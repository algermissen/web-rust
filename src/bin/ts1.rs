#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate webrust;
extern crate net2;
extern crate tokio_core;

use futures::Stream;
use net2::unix::UnixTcpBuilderExt;
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use std::thread;
use std::net::SocketAddr;
use std::sync::Arc;
use futures::future::FutureResult;
use hyper::{Get, StatusCode};
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

use webrust::cpu_intensive_work;

#[derive(Clone, Copy)]
struct Echo;

impl Service for Echo {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
                                (&Get, "/data") => {
                                    let b = cpu_intensive_work().into_bytes();
                                    Response::new()
                                        .with_header(ContentLength(b.len() as u64))
                                        .with_body(b)
                                }
                                _ => Response::new().with_status(StatusCode::NotFound),
                            })
    }
}

fn serve(addr: &SocketAddr, protocol: &Http) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let listener = net2::TcpBuilder::new_v4()
        .unwrap()
        .reuse_port(true)
        .unwrap()
        .bind(addr)
        .unwrap()
        .listen(128)
        .unwrap();
    let listener = TcpListener::from_listener(listener, addr, &handle).unwrap();
    core.run(listener
                 .incoming()
                 .for_each(|(socket, addr)| {
                               protocol.bind_connection(&handle, socket, addr, Echo);
                               Ok(())
                           }))
        .unwrap();
}

fn start_server(nb_instances: usize, addr: &str) {
    let addr = addr.parse().unwrap();

    let protocol = Arc::new(Http::new());
    {
        for _ in 0..nb_instances - 1 {
            let protocol = protocol.clone();
            thread::spawn(move || serve(&addr, &protocol));
        }
    }
    serve(&addr, &protocol);
}


fn main() {
    start_server(4, "0.0.0.0:8080");
}
