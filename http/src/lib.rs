use std::{
    collections::BTreeMap,
    io::Read,
    str::FromStr,
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
const APPLICATION_JSON: &str = "application/json";
const MULTIPART_FORM_DATA: &str = "multipart/form-data";
const FORM_URL_ENCODED: &str = "application/x-www-form-urlencoded";

use adana_script_core::{
    primitive::{Compiler, Json, LibData, NativeFunctionCallResult, Primitive},
    Value,
};
use anyhow::anyhow;
use multipart2::server::Multipart;
use tiny_http::{Header, Method, Request, Response, Server};
use url::Url;
pub struct HttpServer {
    server: Server,
    server_addr: String,
}
pub enum PathSegment {
    Root,
    String(String),
    Variable { position: usize, name: String },
}
pub struct Middleware {
    path_segments: Vec<PathSegment>,
    function: Vec<Value>,
    method: Method,
}
pub struct HttpHandle {
    handle: Arc<Mutex<Option<JoinHandle<anyhow::Result<()>>>>>,
    tx: Arc<Sender<bool>>,
}

#[no_mangle]
pub fn new(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    let server_addr = if params.len() == 1 {
        params[0].to_string()
    } else {
        "0.0.0.0:8000".into()
    };
    match Server::http(&server_addr) {
        Ok(server) => Ok(Primitive::LibData(LibData {
            data: Arc::new(Box::new(HttpServer {
                server,
                server_addr,
            })),
        })),
        Err(e) => Err(anyhow::anyhow!("could not start server: {e}")),
    }
}

#[no_mangle]
pub fn start(mut params: Vec<Primitive>, compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.len() != 3 {
        return Err(anyhow::anyhow!(
            "invalid param (e.g start(server, middlewares, ctx))"
        ));
    }

    let Primitive::LibData(lib_data) = params.remove(0) else {
        return Err(anyhow::anyhow!("first param must be the http server"));
    };
    let Primitive::Array(middlewares) = params.remove(1) else {
        return Err(anyhow::anyhow!(
            "second param must be an array of middlewares"
        ));
    };
    let Primitive::Struct(ctx) = params.remove(0) else {
        return Err(anyhow::anyhow!(
            "third parameter must be the context (struct)"
        ));
    };

    let middlewares = compile_middlewares(middlewares)?;

    let (tx, rx) = mpsc::channel();

    let handle: JoinHandle<anyhow::Result<()>> = std::thread::spawn(move || {
        if let Some(server) = lib_data.data.downcast_ref::<HttpServer>() {
            println!("server running at {}", server.server_addr);
            loop {
                let shutdown = rx.try_recv().ok().unwrap_or(false);
                if shutdown {
                    return Ok(());
                }
                if let Some(mut request) = server
                    .server
                    .recv_timeout(Duration::from_millis(50)) // fixme the duration could be a
                    // parameter
                    .ok()
                    .flatten()
                {
                    let (req, middleware) = request_to_primitive(&mut request, &middlewares)?;
                    if let Some(middleware) = middleware {
                        todo!()
                    } else {
                        request
                            .respond(Response::from_string("NOT FOUND").with_status_code(404))
                            .map_err(|e| anyhow!("could not respond: {e}"))?;
                    }
                }
            }
        } else {
            Err(anyhow!("invalid libData value. Must be an HttpServer"))
        }
    });
    Ok(Primitive::LibData(LibData {
        data: Arc::new(Box::new(HttpHandle {
            handle: Arc::new(Mutex::new(Some(handle))),
            tx: Arc::new(tx),
        })),
    }))
}

fn get_content_type(req: &Request) -> Option<String> {
    req.headers().iter().find_map(|h| {
        if h.field.equiv("Content-Type") {
            Some(h.field.to_string())
        } else {
            None
        }
    })
}

fn request_to_primitive<'a, 'b>(
    req: &'b mut Request,
    middlewares: &'a [Middleware],
) -> anyhow::Result<(Primitive, Option<&'a Middleware>)> {
    let headers = headers_to_primitive(req.headers());
    let url = Url::parse(req.url())?;
    let query_params = Primitive::Struct(
        url.query_pairs()
            .map(|(k, v)| (k.to_string(), Primitive::String(v.to_string())))
            .collect::<BTreeMap<_, _>>(),
    );
    let path = Primitive::String(url.path().to_string());
    let (path_variables, middleware) = {
        let path_segments = if url.path() == "/" || url.path().is_empty() {
            url.path()
                .split('/')
                .filter(|p| !p.is_empty())
                .map(|s| PathSegment::String(s.to_string()))
                .collect::<Vec<_>>()
        } else {
            vec![PathSegment::Root]
        };

        let mut res = (BTreeMap::new(), None);
        // determine which middleware
        'middlewareLoop: for middleware in middlewares {
            if &middleware.method != req.method() {
                continue 'middlewareLoop;
            }
            if middleware.path_segments.len() == path_segments.len() {
                for (segment_from_req, segment_from_middleware) in
                    middleware.path_segments.iter().zip(path_segments.iter())
                {
                    match (segment_from_req, segment_from_middleware) {
                        (PathSegment::Root, PathSegment::Root) => {
                            res.1 = Some(middleware);
                            break 'middlewareLoop;
                        }
                        (PathSegment::Root, PathSegment::String(_))
                        | (PathSegment::Root, PathSegment::Variable { .. })
                        | (PathSegment::String(_), PathSegment::Root) => continue 'middlewareLoop,
                        (PathSegment::String(s), PathSegment::String(s2)) => {
                            if s != s2 {
                                continue 'middlewareLoop;
                            }
                        }
                        (PathSegment::String(value), PathSegment::Variable { name, .. }) => {
                            res.0
                                .insert(name.to_string(), Primitive::String(value.to_string()));
                        }
                        (PathSegment::Variable { .. }, _) => {
                            return Err(anyhow!("BUG. Cannot be a PathSegment::Variable here"))
                        }
                    }
                }
            }
        }
        res
    };

    if middleware.is_none() {
        return Ok((Primitive::Null, middleware));
    }

    let ct = get_content_type(req);
    let method = Primitive::String(req.method().to_string());

    let mut req_p = BTreeMap::from([
        ("headers".to_string(), headers),
        ("query".to_string(), query_params),
        ("body".to_string(), Primitive::Null),
        ("form".to_string(), Primitive::Null),
        ("path".to_string(), path),
        ("method".to_string(), method),
        ("params".to_string(), Primitive::Struct(path_variables)),
    ]);
    if ct == Some(APPLICATION_JSON.to_string()) {
        let mut body = String::new();
        req.as_reader().read_to_string(&mut body)?;
        req_p.insert("body".to_string(), Primitive::from_json(&body)?);
    } else if ct == Some(MULTIPART_FORM_DATA.to_string()) {
        let mut multipart =
            Multipart::from_request(req).map_err(|_| anyhow!("could not parse multipart"))?;
        let mut body = BTreeMap::new();
        while let Ok(Some(mut field)) = multipart.read_entry() {
            let mut data = String::new();
            field.data.read_to_string(&mut data)?;
            let key = field.headers.name.to_string();
            if let Some(file_name) = field.headers.filename {
                body.insert(
                    key,
                    Primitive::Struct(BTreeMap::from([
                        ("file_name".to_string(), Primitive::String(file_name)),
                        (
                            "content_type".to_string(),
                            Primitive::String(
                                field
                                    .headers
                                    .content_type
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "".to_string()),
                            ),
                        ),
                        ("content".to_string(), Primitive::String(data)),
                    ])),
                );
            } else {
                body.insert(key, Primitive::String(data));
            };
        }
        req_p.insert("form".to_string(), Primitive::Struct(body));
    } else if ct == Some(FORM_URL_ENCODED.to_string()) {
        let mut data = String::new();
        let mut body = BTreeMap::new();
        req.as_reader().read_to_string(&mut data)?;
        for (k, v) in form_urlencoded::parse(data.as_bytes()).into_owned() {
            body.insert(k, Primitive::String(v));
        }
        req_p.insert("form".to_string(), Primitive::Struct(body));
    };

    Ok((Primitive::Struct(req_p), middleware))
}

fn headers_to_primitive(headers: &[Header]) -> Primitive {
    let mut prim_headers = BTreeMap::new();
    for header in headers {
        prim_headers.insert(
            header.field.to_string(),
            Primitive::String(header.value.to_string()),
        );
    }
    Primitive::Struct(prim_headers)
}

fn compile_middlewares(middlewares: Vec<Primitive>) -> anyhow::Result<Vec<Middleware>> {
    fn compile_middleware(middleware: Primitive) -> anyhow::Result<Middleware> {
        match middleware {
            Primitive::Ref(p) => {
                let p = p
                    .read()
                    .map_err(|e| anyhow::anyhow!("could not acquire lock {e}"))?;
                compile_middleware(p.clone())
            }
            Primitive::Struct(mut middleware) => {
                let Some(Primitive::String(path)) = middleware.remove("path") else {
                    return Err(anyhow::anyhow!("missing path param in middleware"));
                };
                if !path.starts_with("/") {
                    return Err(anyhow!("path of middleware must start with /"));
                }

                let segments = if path.len() == 1 {
                    vec![PathSegment::Root]
                } else {
                    let mut segments = vec![];
                    for (pos, segment) in path.split('/').filter(|p| !p.is_empty()).enumerate() {
                        if segment.starts_with(":") {
                            let segment = segment[1..].to_string();
                            segments.push(PathSegment::Variable {
                                position: pos,
                                name: segment,
                            })
                        } else {
                            segments.push(PathSegment::String(segment.to_string()))
                        }
                    }
                    segments
                };

                let Some(Primitive::Function { parameters, exprs }) = middleware.remove("handler")
                else {
                    return Err(anyhow::anyhow!("missing handler param i middleware"));
                };
                if parameters.len() != 1 {
                    return Err(anyhow!("Middleware must have exactly one parameter (req)"));
                }
                let Some(Primitive::String(method)) = middleware.remove("method") else {
                    return Err(anyhow!("missing method"));
                };

                Ok(Middleware {
                    path_segments: segments,
                    function: exprs,
                    method: Method::from_str(&method).map_err(|e| anyhow!("bad method {e:?}"))?,
                })
            }
            _ => Err(anyhow::anyhow!("invalid middleware")),
        }
    }
    let mut compiled = Vec::with_capacity(middlewares.len());
    for middleware in middlewares {
        compiled.push(compile_middleware(middleware)?);
    }
    Ok(compiled)
}

#[no_mangle]
pub fn stop(mut params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.len() != 1 {
        return Err(anyhow::anyhow!("invalid param"));
    } else if let Some(Primitive::LibData(lib_data)) = params.get_mut(0) {
        let data = lib_data.data.clone();

        if let Some(server) = data.downcast_ref::<HttpHandle>() {
            match server.handle.lock() {
                Ok(mut http_handle) => {
                    if let Some(handle) = http_handle.take() {
                        server.tx.send(true)?;
                        match handle.join() {
                            Ok(r) => {
                                let _ = r.map_err(|e| anyhow::anyhow!("{e}"))?;
                                println!("server stopped");
                                return Ok(Primitive::Unit);
                            }
                            Err(e) => return Err(anyhow::anyhow!("could not join handle {e:?}")),
                        }
                    } else {
                        return Err(anyhow::anyhow!("server already stopped"));
                    }
                }
                Err(_) => {
                    return Err(anyhow::anyhow!("cannot acquire lock for handle"));
                }
            }
        } else {
            return Err(anyhow::anyhow!("cannot downcast http handle"));
        }
    } else {
        Err(anyhow::anyhow!("invalid param"))
    }
}
