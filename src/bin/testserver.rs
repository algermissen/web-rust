#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate webrust;
extern crate net2;
extern crate tokio_core;
//extern crate futures;
//extern crate hyper;
//extern crate num_cpus;
//extern crate reader_writer;

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
use futures::Future;
use std::env;
//use hyper::{Delete, Get, Put, StatusCode};
//use hyper::header::ContentLength;
//use hyper::server::{Http, Service, Request, Response};

use webrust::cpu_intensive_work;

extern crate prometheus;
use prometheus::{Registry, Gauge, Opts, Histogram, HistogramOpts, Encoder, TextEncoder};


#[derive(Clone)]
struct Srv {
    thread_id: String,
    registry: Registry,
    histogram: Histogram,
    gauge: Gauge
}

impl Service for Srv {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        futures::future::ok(match (req.method(), req.path()) {
            (&Get, "/data") => {
                let timer = self.histogram.start_timer();
                let b = cpu_intensive_work().into_bytes();
                timer.observe_duration();
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
                Response::new()
                .with_status(StatusCode::NotFound)
            }
        })
    }
}

impl Drop for Srv {
    fn drop(&mut self) {
        self.gauge.dec();
    }
}


fn serve(thread_id: String, addr: &SocketAddr, protocol: &Http, registry: &Registry, histogram: &Histogram) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let listener = net2::TcpBuilder::new_v4().unwrap()
    .reuse_port(true).unwrap()
    .bind(addr).unwrap()
    .listen(128).unwrap();

    let opts = Opts::new("conn_number", "conn number help").const_label("threadId", thread_id.as_str());
    let g = Gauge::with_opts(opts).unwrap();
    registry.register(Box::new(g.clone())).unwrap();

    let listener = TcpListener::from_listener(listener, addr, &handle).unwrap();
    core.run(listener.incoming().for_each(|(socket, addr)| {
        let s = Srv {
            thread_id: thread_id.clone(),
            registry: registry.clone(),
            histogram: histogram.clone(),
            gauge: g.clone()
        };
        s.gauge.inc();
        protocol.bind_connection(&handle, socket, addr, s);
        Ok(())
    }).or_else(|e| -> FutureResult<(), ()> {
        panic!("TCP listener failed: {}",e);
    })
    ).unwrap();
}

fn start_server(n: usize, addr: &str) {
    let reg = Registry::new();

    let buckets = vec![0.001,0.002,0.005,0.007,0.010,0.020,0.030,0.040,0.050,0.1,0.2,0.3,0.5,1.0,5.0];
    let opts = HistogramOpts::new("http_request_duration_seconds", "request duration seconds help")
    .buckets(buckets);
    let histogram = Histogram::with_opts(opts).unwrap();
    reg.register(Box::new(histogram.clone())).unwrap();

    let addr = addr.parse().unwrap();
    let protocol = Arc::new(Http::new());
    for i in 0..n - 1 {
        let protocol = protocol.clone();
        let r = reg.clone();
        let h = histogram.clone();
        let thread_id = format!("thread-{}", i);
        thread::spawn(move || serve(thread_id, &addr, &protocol, &r, &h));
    }
    let thread_id = format!("thread-{}", n - 1);
    serve(thread_id, &addr, &protocol, &reg, &histogram);
}


fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Please state the number of threads to start");
    }
    let n = usize::from_str_radix(&args[1], 10).unwrap();

    start_server(n, "0.0.0.0:8080");
}


