#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate webrust;

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
            },
            _ => {
                Response::new()
                .with_status(StatusCode::NotFound)
            }
        })
    }
}


fn main() {
    pretty_env_logger::init().unwrap();
    let addr = "0.0.0.0:8080".parse().unwrap();

    let server = Http::new().bind(&addr, || Ok(Echo)).unwrap();
    println!("Listening on http://{} with 1 thread.", server.local_addr().unwrap());
    server.run().unwrap();
}
