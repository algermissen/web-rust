#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate webrust;
extern crate net2;
extern crate tokio_core;
extern crate tk_listen;

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
use tokio_core::reactor::Remote;
use futures::Future;
use futures::Poll;
use futures::future::*;
use std::env;

use webrust::cpu_intensive_work;
extern crate prometheus;
use prometheus::{Registry, Gauge, Opts, Counter, Histogram, HistogramOpts, Encoder, TextEncoder};

// Srv struct bundles instruments for metrics collection and a thread ID to include
// in the metrics.
#[derive(Clone)]
struct Srv {
    remote: Remote,
    thread_id: String,
    registry: Registry,
    request_counter: Counter,
    request_histogram: Histogram,
    connection_gauge: Gauge,
    finished_conn_counter: Counter,
}

impl Srv {
    fn new(rem: &Remote,
           r: &Registry,
           thread_id: &String,
           cg: &Gauge,
           fcc: &Counter,
           rc: &Counter,
           rh: &Histogram)
           -> Srv {
        let s = Srv {
            remote: rem.clone(),
            registry: r.clone(),
            thread_id: thread_id.clone(),
            request_counter: rc.clone(),
            request_histogram: rh.clone(),
            connection_gauge: cg.clone(),
            finished_conn_counter: fcc.clone(),
        };
        s.connection_gauge.inc();
        s
    }
}

pub struct FutureResponse(Box<Future<Item = Response, Error = hyper::Error>>);

impl Future for FutureResponse {
    type Item = Response;
    type Error = hyper::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

impl Service for Srv {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResponse;

    // Serves the dummy data created from our CPU intensive work
    // (and the metrics endpoint)
    fn call(&self, req: Request) -> Self::Future {
        let f = match (req.method(), req.path()) {
            (&Get, "/data") => {
                self.request_counter.inc();
                let timer = self.request_histogram.start_timer();
                let (tx, rx) = futures::sync::oneshot::channel::<Self::Response>();
                self.remote
                    .spawn(|_| {
                               let b = cpu_intensive_work().into_bytes();
                               let res = Response::new()
                                   .with_header(ContentLength(b.len() as u64))
                                   .with_body(b);
                               let _ = tx.send(res);
                               Ok(())
                           });
                FutureResponse(rx.then(|r| {
                                           timer.observe_duration();
                                           r
                                       })
                                   .or_else(|_| -> Result<Response, Self::Error> {
                    Ok(Response::new().with_status(StatusCode::InternalServerError))
                })
                                   .boxed())
            }
            /// Standard Prometheus metrics endpoint
            (&Get, "/metrics") => {
                let encoder = TextEncoder::new();
                let metric_familys = self.registry.gather();
                let mut buffer = vec![];
                encoder.encode(&metric_familys, &mut buffer).unwrap();
                let res = Response::new().with_body(buffer);
                FutureResponse(futures::finished(res).boxed())
            }

            _ => {
                let res = Response::new().with_status(StatusCode::NotFound);
                FutureResponse(futures::finished(res).boxed())
            }
        };
        f
    }
}


impl Drop for Srv {
    fn drop(&mut self) {
        self.connection_gauge.dec();
        self.finished_conn_counter.inc();
    }
}


fn serve(remote: &Remote,
         thread_id: String,
         addr: &SocketAddr,
         protocol: &Http,
         registry: &Registry) {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = net2::TcpBuilder::new_v4()
        .unwrap()
        .reuse_port(true)
        .unwrap()
        .bind(addr)
        .unwrap()
        .listen(4)
        .unwrap();

    // =================== START METRICS SETUP =====================
    let acg_opts = Opts::new("conn_number", "conn number help")
        .const_label("threadId", thread_id.as_str());
    let active_conn_gauge = Gauge::with_opts(acg_opts).unwrap();
    registry
        .register(Box::new(active_conn_gauge.clone()))
        .unwrap();

    let cc_opts = Opts::new("conn_counter", "conn counter help")
        .const_label("threadId", thread_id.as_str());
    let accepted_conn_counter = Counter::with_opts(cc_opts).unwrap();
    registry
        .register(Box::new(accepted_conn_counter.clone()))
        .unwrap();

    let fcc_opts = Opts::new("finished_conn_counter", "finished_conn counter help")
        .const_label("threadId", thread_id.as_str());
    let finished_conn_counter = Counter::with_opts(fcc_opts).unwrap();
    registry
        .register(Box::new(finished_conn_counter.clone()))
        .unwrap();

    let rc_opts = Opts::new("req_counter", "req counter help")
        .const_label("threadId", thread_id.as_str());
    let rc = Counter::with_opts(rc_opts).unwrap();
    registry.register(Box::new(rc.clone())).unwrap();


    let buckets = vec![0.001, 0.002, 0.005, 0.007, 0.010, 0.020, 0.030, 0.040, 0.050, 0.1, 0.2,
                       0.3, 0.5, 1.0, 5.0];
    let h_opts = HistogramOpts::new("http_request_duration_seconds",
                                    "request duration seconds help")
            .const_label("threadId", thread_id.as_str())
            .buckets(buckets);
    let h = Histogram::with_opts(h_opts).unwrap();
    registry.register(Box::new(h.clone())).unwrap();
    // =================== END METRICS SETUP =====================

    let listener = TcpListener::from_listener(listener, addr, &handle).unwrap();
    core.run(listener
                 .incoming()
                 .for_each(|(socket, addr)| {
            let s = Srv::new(remote,
                             registry,
                             &thread_id,
                             &active_conn_gauge,
                             &finished_conn_counter,
                             &rc,
                             &h);
            accepted_conn_counter.inc();
            protocol.bind_connection(&handle, socket, addr, s);
            Ok(())
        })
                 .or_else(|e| -> FutureResult<(), ()> {
                              panic!("TCP listener failed: {}", e);
                          }))
        .unwrap();
}

fn start_server(n: usize, addr: &'static str) {

    let mut worker_core = Core::new().unwrap();
    let remote = worker_core.remote();

    thread::spawn(move || {
        let reg = Registry::new();
        let addr = addr.parse().unwrap();
        let protocol = Arc::new(Http::new());
        for i in 0..n - 1 {
            let protocol = protocol.clone();
            let r = reg.clone();
            let thread_id = format!("thread-{}", i);
            let remote2 = remote.clone();
            thread::spawn(move || serve(&remote2, thread_id, &addr, &protocol, &r));
        }
        let thread_id = format!("thread-{}", n - 1);
        serve(&remote, thread_id, &addr, &protocol, &reg);
    });

    worker_core.run(empty::<(), ()>()).unwrap()
}

static A: &'static str = "0.0.0.0:8080";

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() < 1 {
        panic!("Please state the number of threads to start");
    }

    let n = usize::from_str_radix(&args[1], 10).unwrap();

    start_server(n, A);
}
