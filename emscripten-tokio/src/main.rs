//! Tokio on bare Emscripten in Node.js. The runtime is a hosted
//! `LocalEventLoop`: its wait *is* the Node event loop, so `tokio::spawn`,
//! timers, sync primitives, TCP and a hyper server work as they do natively, with no
//! `#[tokio::main]`, no stack switching and no blocking. `main` returns with
//! the server in flight and Node keeps running while it has tasks.

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{header, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::runtime::{Builder, LocalOptions};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, timeout};

const INDEX: &str = "Tokio on Emscripten\n\n\
GET /spawn      spawn tasks and await their JoinHandles\n\
GET /sleep?ms=  tokio::time::sleep\n\
GET /timeout    tokio::time::timeout\n\
GET /channels   mpsc between spawned producers and the handler\n\
GET /mutex      tokio::sync::Mutex shared across tasks\n\
GET /join       tokio::join! over concurrent futures\n";

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);
    let rt = Builder::new_current_thread()
        .enable_all()
        .build_hosted_local_event_loop(LocalOptions::default())
        .expect("failed to build event loop");
    rt.spawn_local(serve(port));
    // The host loop drives the runtime after main returns.
    std::mem::forget(rt);
}

async fn serve(port: u16) {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("failed to bind");
    println!("Tokio on Emscripten listening on http://127.0.0.1:{port}");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(async move {
                    let conn = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service_fn(handle));
                    if let Err(e) = conn.await {
                        eprintln!("connection error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

#[derive(Serialize)]
struct Demo {
    description: &'static str,
    result: String,
    elapsed_ms: f64,
}

fn demo(description: &'static str, result: String, start: Instant) -> Demo {
    Demo {
        description,
        result,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}

async fn handle(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let query = req.uri().query().unwrap_or("");
    Ok(match req.uri().path() {
        "/" => text(StatusCode::OK, INDEX),
        "/spawn" => json(spawn().await),
        "/sleep" => json(sleep_for(query).await),
        "/timeout" => json(timeouts().await),
        "/channels" => json(channels().await),
        "/mutex" => json(mutex().await),
        "/join" => json(join().await),
        _ => text(StatusCode::NOT_FOUND, "not found\n"),
    })
}

fn text(status: StatusCode, body: &'static str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap()
}

fn json(demo: Demo) -> Response<Full<Bytes>> {
    let mut body = serde_json::to_string_pretty(&demo).unwrap();
    body.push('\n');
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

async fn spawn() -> Demo {
    let start = Instant::now();
    let handles: Vec<_> = (1..=3)
        .map(|i| {
            tokio::spawn(async move {
                sleep(Duration::from_millis(10)).await;
                i * i
            })
        })
        .collect();
    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }
    demo(
        "3 spawned tasks, each sleeping 10ms, run concurrently",
        format!("{results:?}"),
        start,
    )
}

async fn sleep_for(query: &str) -> Demo {
    let ms: u64 = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "ms")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(100);
    let start = Instant::now();
    sleep(Duration::from_millis(ms)).await;
    demo("tokio::time::sleep", format!("slept {ms}ms"), start)
}

async fn timeouts() -> Demo {
    let start = Instant::now();
    let fast = timeout(Duration::from_millis(50), sleep(Duration::from_millis(10))).await;
    let slow = timeout(Duration::from_millis(20), sleep(Duration::from_millis(100))).await;
    demo(
        "a 10ms operation under a 50ms timeout, then a 100ms one under 20ms",
        format!("fast: {}, slow: {}", verdict(fast), verdict(slow)),
        start,
    )
}

fn verdict(r: Result<(), tokio::time::error::Elapsed>) -> &'static str {
    match r {
        Ok(()) => "completed",
        Err(_) => "timed out",
    }
}

async fn channels() -> Demo {
    let start = Instant::now();
    let (tx, mut rx) = mpsc::channel::<u32>(4);
    for range in [1..=5, 6..=10] {
        let tx = tx.clone();
        tokio::spawn(async move {
            for i in range {
                sleep(Duration::from_millis(5)).await;
                let _ = tx.send(i).await;
            }
        });
    }
    drop(tx);
    let mut received = Vec::new();
    while let Some(v) = rx.recv().await {
        received.push(v);
    }
    demo(
        "two producers send 1..=5 and 6..=10 over an mpsc channel",
        format!("{received:?}"),
        start,
    )
}

async fn mutex() -> Demo {
    let start = Instant::now();
    let counter = Arc::new(Mutex::new(0));
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let counter = counter.clone();
            tokio::spawn(async move {
                let mut n = counter.lock().await;
                sleep(Duration::from_millis(1)).await;
                *n += 1;
            })
        })
        .collect();
    for h in handles {
        h.await.unwrap();
    }
    let n = *counter.lock().await;
    demo(
        "10 tasks increment a counter while holding an async Mutex",
        format!("counter = {n}"),
        start,
    )
}

async fn join() -> Demo {
    let start = Instant::now();
    let (a, b, c) = tokio::join!(
        async {
            sleep(Duration::from_millis(30)).await;
            "a"
        },
        async {
            sleep(Duration::from_millis(30)).await;
            "b"
        },
        async {
            sleep(Duration::from_millis(30)).await;
            "c"
        },
    );
    demo(
        "three 30ms futures joined; takes ~30ms, not 90",
        format!("{a}{b}{c}"),
        start,
    )
}
