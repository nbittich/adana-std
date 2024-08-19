use std::{
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use adana_script_core::{
    primitive::{Compiler, LibData, NativeFunctionCallResult, Primitive},
    Value,
};
use anyhow::anyhow;
use tiny_http::{Response, Server};
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
}
pub struct HttpHandle {
    handle: Arc<Mutex<Option<JoinHandle<Result<(), String>>>>>,
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

    let handle: JoinHandle<Result<(), String>> = std::thread::spawn(move || {
        if let Some(server) = lib_data.data.downcast_ref::<HttpServer>() {
            println!("server running at {}", server.server_addr);
            loop {
                let shutdown = rx.try_recv().ok().unwrap_or(false);
                if shutdown {
                    return Ok(());
                }
                if let Some(request) = server
                    .server
                    .recv_timeout(Duration::from_millis(50)) // fixme the duration could be a
                    // parameter
                    .ok()
                    .flatten()
                {
                    let response = Response::from_string("Hello, World!");
                    request
                        .respond(response)
                        .map_err(|e| format!("could not respond: {e}"))?;
                }
            }
        } else {
            Err("invalid libData value. Must be an HttpServer".into())
        }
    });
    Ok(Primitive::LibData(LibData {
        data: Arc::new(Box::new(HttpHandle {
            handle: Arc::new(Mutex::new(Some(handle))),
            tx: Arc::new(tx),
        })),
    }))
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
                    for (pos, segment) in path
                        .split('/')
                        .into_iter()
                        .filter(|p| !p.is_empty())
                        .enumerate()
                    {
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
                    return Err(anyhow::anyhow!("missing handler pram i middleware"));
                };
                if parameters.len() != 1 {
                    return Err(anyhow!("Middleware must have exactly one parameter (req)"));
                }

                Ok(Middleware {
                    path_segments: segments,
                    function: exprs,
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
