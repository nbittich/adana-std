use std::sync::Arc;

use adana_script_core::primitive::{Compiler, LibData, NativeFunctionCallResult, Primitive};
use tiny_http::{Response, Server};
pub struct HttpServer {
    server: Server,
    server_addr: String,
}

#[no_mangle]
pub fn new_server(params: Vec<Primitive>, _compiler: Box<Compiler>) -> NativeFunctionCallResult {
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
        std::thread::spawn(move || {
            if let Some(server) = data.downcast_ref::<HttpServer>() {
                println!("server running at {}", server.server_addr);
                for request in server.server.incoming_requests() {
                    println!("received smth");
                    let response = Response::from_string("Hello, World!");
                    request.respond(response).unwrap();
                }
            } else {
                panic!("invalid libData value. Must be an HttpServer");
            }
        });
        Ok(Primitive::Unit)
    } else {
        todo!()
    }
}
