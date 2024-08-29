use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::PathBuf,
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
const ACCEPT: &str = "Accept";
const CONTENT_TYPE: &str = "Content-Type";
use adana_script_core::{
    primitive::{Compiler, Json, LibData, NativeFunctionCallResult, Primitive, ToNumber},
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
#[derive(Debug)]
pub enum PathSegment {
    Root,
    String(String),
    Variable { position: usize, name: String },
}
#[derive(Debug)]
pub struct Route {
    path_segments: Vec<PathSegment>,
    function: Value,
    method: Method,
}
#[derive(Debug)]
pub struct StaticServe {
    path: String,
    file_path: String,
}

pub struct HttpHandle {
    handle: Arc<Mutex<Option<JoinHandle<anyhow::Result<()>>>>>,
    tx: Arc<Sender<bool>>,
}

fn server_header() -> Header {
    Header::from_bytes(&b"Server"[..], &b"Adana"[..]).unwrap()
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
pub fn start(mut params: Vec<Primitive>, mut compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.len() != 2 {
        return Err(anyhow::anyhow!(
            "invalid param (e.g start(server, settings))"
        ));
    }

    let Primitive::LibData(lib_data) = params.remove(0) else {
        return Err(anyhow::anyhow!("first param must be the http server"));
    };
    let Primitive::Struct(mut settings) = params.remove(0) else {
        return Err(anyhow::anyhow!(
            r#"second param must be an array of settings (e.g struct {{static: [], routes []}})"#
        ));
    };

    let Some(Primitive::Array(routes)) = settings.remove("routes") else {
        return Err(anyhow!("missing routes in settings"));
    };

    let statics = if let Some(Primitive::Array(statics)) = settings.remove("static") {
        statics
    } else {
        vec![]
    };

    let store = match settings
        .remove("store")
        .ok_or_else(|| anyhow!("missing store struct in settings"))?
    {
        Primitive::Ref(s)
            if matches!(
                s.read()
                    .map_err(|e| anyhow!("could not read store {e}"))?
                    .as_ref_ok()?,
                Primitive::Struct(_)
            ) =>
        {
            Primitive::Ref(s)
        }
        v @ Primitive::Struct(_) => Primitive::Ref(v.ref_prim()),
        _ => Primitive::Ref(Primitive::Struct(BTreeMap::new()).ref_prim()),
    };

    let statics = compile_statics(statics)?;

    let routes = compile_routes(routes)?;

    let (tx, rx) = mpsc::channel();

    let handle: JoinHandle<anyhow::Result<()>> = std::thread::spawn(move || {
        if let Some(server) = lib_data.data.downcast_ref::<HttpServer>() {
            println!("server running at {}", server.server_addr);
            loop {
                let shutdown = rx.try_recv().ok().unwrap_or(false);
                if shutdown {
                    println!("server shutting down");
                    return Ok(());
                }
                if let Some(request) = server
                    .server
                    .recv_timeout(Duration::from_millis(50)) // fixme the duration could be a
                    // parameter
                    .ok()
                    .flatten()
                {
                    match handle_request(request, &routes, &statics, &mut compiler, store.clone()) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("could not process request. {e:?}")
                        }
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

fn handle_request(
    mut request: Request,
    routes: &[Route],
    statics: &[StaticServe],
    compiler: &mut Box<Compiler>,
    store: Primitive,
) -> anyhow::Result<()> {
    let (req, route) = match request_to_primitive(&mut request, routes) {
        Ok((r, m)) => (r, m),
        Err(e) => {
            println!("err {e:?}");
            return Err(e);
        }
    };

    if let Some(route) = route {
        let res = compiler(
            Value::FunctionCall {
                parameters: Box::new(Value::BlockParen(vec![
                    Value::Primitive(req),
                    Value::Primitive(store),
                ])),
                function: Box::new(route.function.clone()),
            },
            BTreeMap::new(), // fixme extra ctx is probably no longer useful
        )?;
        handle_response(request, &res)?;
    } else {
        let url = extract_path_from_url(&request)?;
        if let Some(st) = statics.iter().find(|s| url.path().starts_with(&s.path)) {
            let mut p = PathBuf::from(url.path().replacen(&st.path, &st.file_path, 1));
            if p.is_relative() {
                p = p.canonicalize()?;
            }
            if p.is_dir() {
                p.push("index.html"); // if it's a dir, index.html
            }

            match File::open(&p) {
                Ok(f) => {
                    let ct = mime_guess::from_path(&p).first_or_text_plain();
                    request
                        .respond(
                            Response::from_file(f)
                                .with_header(make_header(CONTENT_TYPE, ct.as_ref())?)
                                .with_header(server_header()),
                        )
                        .map_err(|e| anyhow!("could not respond: {e}"))?;
                }
                Err(_) => {
                    request
                        .respond(
                            Response::from_string("NOT FOUND")
                                .with_status_code(404)
                                .with_header(server_header()),
                        )
                        .map_err(|e| anyhow!("could not respond: {e}"))?;
                }
            }
        } else {
            request
                .respond(
                    Response::from_string("NOT FOUND")
                        .with_status_code(404)
                        .with_header(server_header()),
                )
                .map_err(|e| anyhow!("could not respond: {e}"))?;
        }
    }
    Ok(())
}

fn handle_response(req: Request, res: &Primitive) -> anyhow::Result<()> {
    match res {
        Primitive::Ref(r) => {
            let r = r
                .read()
                .map_err(|e| anyhow!("could not acquire lock {e}"))?;
            handle_response(req, &r)
        }
        Primitive::EarlyReturn(s) => handle_response(req, s),
        Primitive::Error(s) => {
            if get_header(&req, ACCEPT) == Some(APPLICATION_JSON.to_string())
                || get_content_type(&req) == Some(APPLICATION_JSON.to_string())
            {
                let mut response = Response::from_string(
                    Primitive::Struct(BTreeMap::from([(
                        "error".to_string(),
                        Primitive::String(s.to_string()),
                    )]))
                    .to_json()?,
                )
                .with_header(server_header())
                .with_status_code(400);
                response.add_header(make_header(CONTENT_TYPE, APPLICATION_JSON)?);
                req.respond(response)
                    .map_err(|e| anyhow!("cannot respond {e}"))
            } else {
                let response = Response::from_string(format!("Error: {res:?}"))
                    .with_status_code(400)
                    .with_header(server_header());
                req.respond(response)
                    .map_err(|e| anyhow!("cannot respond {e}"))
            }
        }

        Primitive::String(s) => {
            let mut response = Response::from_string(s).with_header(server_header());
            if let Some(accept) = get_header(&req, ACCEPT) {
                response.add_header(make_header(CONTENT_TYPE, &accept)?);
            }
            req.respond(response)
                .map_err(|e| anyhow!("cannot respond {e}"))
        }
        v @ Primitive::Array(_)
            if get_header(&req, ACCEPT) == Some(APPLICATION_JSON.to_string())
                || get_content_type(&req) == Some(APPLICATION_JSON.to_string()) =>
        {
            let mut response = Response::from_string(v.to_json()?).with_header(server_header());
            response.add_header(make_header(CONTENT_TYPE, APPLICATION_JSON)?);
            req.respond(response)
                .map_err(|e| anyhow!("cannot respond {e}"))
        }
        Primitive::NativeLibrary(_)
        | Primitive::NativeFunction(_, _)
        | Primitive::LibData(_)
        | Primitive::Function { .. }
        | Primitive::U8(_)
        | Primitive::I8(_)
        | Primitive::Int(_)
        | Primitive::Bool(_)
        | Primitive::Null
        | Primitive::Double(_)
        | Primitive::Array(_)
        | Primitive::NoReturn => {
            let response = Response::from_string(format!("SERVER ERROR: BAD RETURN {res:?}"))
                .with_status_code(500)
                .with_header(server_header());
            req.respond(response)
                .map_err(|e| anyhow!("cannot respond {e}"))
        }

        Primitive::Unit => {
            let response = Response::from_string("")
                .with_status_code(200)
                .with_header(server_header());
            req.respond(response)
                .map_err(|e| anyhow!("cannot respond {e}"))
        }
        Primitive::Struct(res) => {
            let Some(status) = res.get("status") else {
                return Err(anyhow!("missing status in response (e.g 200)"));
            };

            let Some(body) = res.get("body") else {
                return Err(anyhow!("missing body in response"));
            };

            let headers = if let Some(Primitive::Struct(headers)) = res.get("headers") {
                headers.clone()
            } else {
                BTreeMap::new()
            };
            let ct = if let Some(ct) = headers.iter().find_map(|h| {
                if h.0.eq_ignore_ascii_case(CONTENT_TYPE) {
                    Some(h.1.to_string())
                } else {
                    None
                }
            }) {
                ct
            } else if let Some(ct) = get_content_type(&req).or(get_header(&req, ACCEPT)) {
                ct
            } else {
                "text/html".to_string()
            };
            let body = if ct == APPLICATION_JSON {
                body.to_json()?
            } else {
                body.to_string()
            };
            let status = if let Primitive::Int(n) = status.to_int() {
                n as u16
            } else {
                200
            };

            let mut response = Response::from_string(body)
                .with_status_code(status)
                .with_header(server_header());
            for h in headers.iter().map(|(k, v)| make_header(k, &v.to_string())) {
                response.add_header(h?);
            }
            req.respond(response)
                .map_err(|e| anyhow!("cannot respond {e}"))
        }
    }
}
fn make_header(k: &str, v: &str) -> anyhow::Result<tiny_http::Header> {
    tiny_http::Header::from_str(format!("{k}:{v}").as_str())
        .map_err(|e| anyhow!("bad header {v}: {e:?}"))
}
fn get_header(req: &Request, header_name: &'static str) -> Option<String> {
    req.headers().iter().find_map(|h| {
        if h.field.equiv(header_name) {
            Some(h.value.to_string())
        } else {
            None
        }
    })
}
fn get_content_type(req: &Request) -> Option<String> {
    get_header(req, CONTENT_TYPE)
}

fn extract_path_from_url(req: &Request) -> anyhow::Result<Url> {
    let url = match Url::parse(req.url()) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let url = Url::parse("http://dummy.com")?;
            url.join(req.url())
        }
        e @ Err(_) => e,
    }?;
    Ok(url)
}

fn request_to_primitive<'a>(
    req: &mut Request,
    routes: &'a [Route],
) -> anyhow::Result<(Primitive, Option<&'a Route>)> {
    let headers = headers_to_primitive(req.headers());

    let url = extract_path_from_url(req)?;
    let query_params = Primitive::Struct(
        url.query_pairs()
            .map(|(k, v)| (k.to_string(), Primitive::String(v.to_string())))
            .collect::<BTreeMap<_, _>>(),
    );
    let path = Primitive::String(url.path().to_string());
    let (path_variables, route) = {
        let path_segments = if url.path() == "/" || url.path().is_empty() {
            vec![PathSegment::Root]
        } else {
            url.path()
                .split('/')
                .filter(|p| !p.is_empty())
                .map(|s| PathSegment::String(s.to_string()))
                .collect::<Vec<_>>()
        };

        let mut res = (BTreeMap::new(), None);

        // determine which route
        'routeLoop: for route in routes {
            res.1 = None;
            if &route.method != req.method() {
                continue 'routeLoop;
            }
            if route.path_segments.len() == path_segments.len() {
                for (segment_from_req, segment_from_route) in
                    path_segments.iter().zip(route.path_segments.iter())
                {
                    match (segment_from_req, segment_from_route) {
                        (PathSegment::Root, PathSegment::Root) => {
                            res.1 = Some(route);
                            break 'routeLoop;
                        }
                        (PathSegment::Root, PathSegment::String(_))
                        | (PathSegment::Root, PathSegment::Variable { .. })
                        | (PathSegment::String(_), PathSegment::Root) => {
                            continue 'routeLoop;
                        }
                        (PathSegment::String(s), PathSegment::String(s2)) => {
                            if s != s2 {
                                continue 'routeLoop;
                            } else {
                                res.1 = Some(route);
                            }
                        }
                        (PathSegment::String(value), PathSegment::Variable { name, .. }) => {
                            res.1 = Some(route);
                            res.0
                                .insert(name.to_string(), Primitive::String(value.to_string()));
                        }
                        (PathSegment::Variable { .. }, _) => {
                            return Err(anyhow!("BUG. Cannot be a PathSegment::Variable here"))
                        }
                    }
                }
                if res.1.is_some() {
                    break 'routeLoop;
                }
            }
        }

        res
    };

    if route.is_none() {
        return Ok((Primitive::Null, route));
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

    Ok((Primitive::Struct(req_p), route))
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

fn compile_statics(statics: Vec<Primitive>) -> anyhow::Result<Vec<StaticServe>> {
    let mut static_serve = Vec::with_capacity(statics.len());
    for st in statics {
        let Primitive::Struct(mut st) = st else {
            return Err(anyhow!("bad static {st}"));
        };
        let Some(Primitive::String(path)) = st.remove("path") else {
            return Err(anyhow!("missing path in static"));
        };
        let Some(Primitive::String(mut file_path)) = st.remove("file_path") else {
            return Err(anyhow!("missing file_path in static"));
        };

        let pb = PathBuf::from(&file_path);
        if pb.is_relative() || pb.is_symlink() {
            let pb = pb.canonicalize()?;

            file_path = pb.display().to_string();
        }
        if !file_path.ends_with(std::path::MAIN_SEPARATOR) {
            file_path.push(std::path::MAIN_SEPARATOR);
        }

        static_serve.push(StaticServe { path, file_path });
    }
    Ok(static_serve)
}
fn compile_routes(routes: Vec<Primitive>) -> anyhow::Result<Vec<Route>> {
    fn compile_route(route: Primitive) -> anyhow::Result<Route> {
        match route {
            Primitive::Ref(p) => {
                let p = p
                    .read()
                    .map_err(|e| anyhow::anyhow!("could not acquire lock {e}"))?;
                compile_route(p.clone())
            }
            Primitive::Struct(mut route) => {
                let Some(Primitive::String(path)) = route.remove("path") else {
                    return Err(anyhow::anyhow!("missing path param in route"));
                };
                if !path.starts_with("/") {
                    return Err(anyhow!("path of route must start with /"));
                }

                let segments = if path.len() == 1 {
                    vec![PathSegment::Root]
                } else {
                    let mut segments = vec![];
                    for (pos, segment) in path.split('/').filter(|p| !p.is_empty()).enumerate() {
                        if let Some(stripped) = segment.strip_prefix(":") {
                            let segment = stripped.to_string();
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

                let Some(Primitive::Function { parameters, exprs }) = route.remove("handler")
                else {
                    return Err(anyhow::anyhow!("missing handler param i route"));
                };
                if parameters.len() != 2 {
                    return Err(anyhow!(
                        "route must have exactly two parameters (req, store)"
                    ));
                }
                let Some(Primitive::String(method)) = route.remove("method") else {
                    return Err(anyhow!("missing method"));
                };

                Ok(Route {
                    path_segments: segments,
                    function: Primitive::Function { parameters, exprs }.to_value()?,
                    method: Method::from_str(&method).map_err(|e| anyhow!("bad method {e:?}"))?,
                })
            }
            _ => Err(anyhow::anyhow!("invalid route")),
        }
    }
    let mut compiled = Vec::with_capacity(routes.len());
    for route in routes {
        compiled.push(compile_route(route)?);
    }
    Ok(compiled)
}

#[no_mangle]
pub fn stop(mut params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.len() != 1 {
        Err(anyhow::anyhow!("invalid param"))
    } else if let Some(Primitive::LibData(lib_data)) = params.get_mut(0) {
        let data = lib_data.data.clone();

        if let Some(server) = data.downcast_ref::<HttpHandle>() {
            match server.handle.lock() {
                Ok(mut http_handle) => {
                    if let Some(handle) = http_handle.take() {
                        server.tx.send(true)?;
                        match handle.join() {
                            Ok(r) => {
                                r.map_err(|e| anyhow::anyhow!("{e}"))?;
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
