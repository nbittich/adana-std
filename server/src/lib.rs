use std::{
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use adana_script_core::primitive::{Compiler, LibData, NativeFunctionCallResult, Primitive};
use tiny_http::{Response, Server};
pub struct HttpServer {
    server: Server,
    server_addr: String,
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
pub fn start(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
    if params.len() != 1 {
        return Err(anyhow::anyhow!("invalid param"));
    } else if let Some(Primitive::LibData(lib_data)) = params.get(0) {
        let data = lib_data.data.clone();
        let (tx, rx) = mpsc::channel();

        let handle: JoinHandle<Result<(), String>> = std::thread::spawn(move || {
            if let Some(server) = data.downcast_ref::<HttpServer>() {
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
                        println!("received smth");
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
    } else {
        Err(anyhow::anyhow!("invalid param"))
    }
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
