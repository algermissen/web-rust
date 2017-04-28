#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate webrust;

//extern crate futures;
//extern crate hyper;
extern crate net2;
//extern crate num_cpus;
//extern crate reader_writer;
extern crate tokio_core;

use futures::Stream;
//use hyper::{Delete, Get, Put, StatusCode};
//use hyper::header::ContentLength;
//use hyper::server::{Http, Service, Request, Response};
use net2::unix::UnixTcpBuilderExt;
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
//
use std::thread;
//use std::path::Path;
use std::net::SocketAddr;
use std::sync::Arc;
//use std::io::{Read, Write};

use futures::future::FutureResult;
use hyper::{Get, StatusCode};
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};
use futures::Future;

use webrust::cpu_intensive_work;
use std::env;

extern crate prometheus;
use prometheus::{Registry, Gauge, Opts, Histogram, HistogramOpts, Encoder, TextEncoder};


#[derive(Clone)]
struct Counted {
    registry: Registry,
    histogram: Histogram,
    gauge: Gauge
}

impl Service for Counted {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
            (&Get, "/data") => {
                self.histogram.start_timer();
                let b = cpu_intensive_work().into_bytes();
                Response::new()
                .with_header(ContentLength(b.len() as u64))
                .with_body(b)
            },
            (&Get, "/metrics") => {
                let encoder = TextEncoder::new();
                let metric_familys = self.registry.gather();
                let mut buffer = vec![];
                encoder.encode(&metric_familys, &mut buffer).unwrap();

                Response::new()
                .with_body(buffer)
            },
            _ => {
                println!("_____ {}", req.path());
                Response::new()
                .with_status(StatusCode::NotFound)
            }
        })
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.gauge.dec();
        println!("Disconnect");
    }
}


fn serve(addr: &SocketAddr, protocol: &Http, registry: &Registry, histogram: &Histogram, gauge: &Gauge) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let listener = net2::TcpBuilder::new_v4().unwrap()
    .reuse_port(true).unwrap()
    .bind(addr).unwrap()
    .listen(128).unwrap();
    let listener = TcpListener::from_listener(listener, addr, &handle).unwrap();
    core.run(listener.incoming().for_each(|(socket, addr)| {
        let s = Counted { registry: registry.clone(), histogram: histogram.clone(), gauge: gauge.clone() };
        s.gauge.inc();
        println!("xConnect");
        protocol.bind_connection(&handle, socket, addr, s);

        Ok(())
    }).or_else(|e| -> FutureResult<(), ()> {
        panic!("TCP listener failed: {}",e);
    })
    ).unwrap();
}

fn start_server(nb_instances: usize, addr: &str) {
    let opts = HistogramOpts::new("http_request_duration_milliseconds", "request duration milliseconds help");
    let histogram = Histogram::with_opts(opts).unwrap();
    let gauge_opts = Opts::new("connections_count", "connections count help");
    //.const_label("a", "1").const_label("b", "2");
    let gauge = Gauge::with_opts(gauge_opts).unwrap();


    let reg = Registry::new();
    reg.register(Box::new(histogram.clone())).unwrap();
    reg.register(Box::new(gauge.clone())).unwrap();


    let addr = addr.parse().unwrap();

    let protocol = Arc::new(Http::new());
    {
        for _ in 0..nb_instances - 1 {
            let protocol = protocol.clone();
            let r = reg.clone();
            let h = histogram.clone();
            let g = gauge.clone();
            thread::spawn(move || serve(&addr, &protocol, &r, &h, &g));
        }
    }
    serve(&addr, &protocol, &reg, &histogram, &gauge);
}


fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Please state the number of threads to start");
    }
    let n = usize::from_str_radix(&args[1], 10).unwrap();

    start_server(n, "0.0.0.0:8080");
}


