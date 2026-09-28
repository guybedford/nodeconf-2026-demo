//! Tokio on bare Emscripten in Node.js. The runtime is a hosted
//! `LocalEventLoop`: its wait *is* the Node event loop, so `tokio::spawn`,
//! timers, sync primitives, TCP and a hyper server work as they do natively, with no
//! `#[tokio::main]`, no stack switching and no blocking. `main` returns with
//! the server in flight and Node keeps running while it has tasks.

use bytes::Bytes;
use http_body_util::channel::Channel;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{header, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::future::Future;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::runtime::{Builder, LocalEventLoop, LocalOptions};
use tokio::sync::mpsc;
use tokio::time::sleep;

thread_local! {
    // The host loop drives the runtime after main returns.
    static EVENT_LOOP: LocalEventLoop = Builder::new_current_thread()
        .enable_all()
        .build_hosted_local_event_loop(LocalOptions::default())
        .expect("failed to build event loop");
}

const INDEX: &str = "Tokio on Emscripten\n\n\
GET /countdown   stream a countdown from 10, one tick per second\n\
GET /parallel    3 spawned countdowns at different rates, interleaved\n\
GET /sleep       tokio::time::sleep for 3s\n\n\
Open one, then another a few seconds later: both progress on the one Node thread.\n";

type Body = BoxBody<Bytes, Infallible>;

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);
    EVENT_LOOP.with(|rt| rt.spawn_local(serve(port)));
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

async fn handle(req: Request<Incoming>) -> Result<Response<Body>, Infallible> {
    println!("{} {}", req.method(), req.uri());
    Ok(match req.uri().path() {
        "/" => text(StatusCode::OK, INDEX),
        "/countdown" => countdown(),
        "/parallel" => parallel(),
        "/sleep" => sleep_for().await,
        _ => text(StatusCode::NOT_FOUND, "not found\n"),
    })
}

fn text(status: StatusCode, body: &'static str) -> Response<Body> {
    plain(status, Full::new(Bytes::from_static(body.as_bytes())).boxed())
}

fn plain(status: StatusCode, body: Body) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header("x-content-type-options", "nosniff")
        .body(body)
        .unwrap()
}

/// A streamed text response fed by `f` through an `mpsc` of lines.
fn streamed<F>(f: impl FnOnce(mpsc::Sender<String>) -> F) -> Response<Body>
where
    F: Future<Output = ()> + Send + 'static,
{
    let (lines, mut rx) = mpsc::channel::<String>(16);
    let (mut body_tx, body) = Channel::<Bytes, Infallible>::new(4);
    tokio::spawn(f(lines));
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            // The client went away.
            if body_tx.send_data(Bytes::from(line)).await.is_err() {
                return;
            }
        }
    });
    plain(StatusCode::OK, body.boxed())
}

async fn tick(lines: &mpsc::Sender<String>, label: &str, from: u32, interval: Duration) {
    for i in (1..=from).rev() {
        sleep(interval).await;
        if lines.send(format!("{label}{i}\n")).await.is_err() {
            return;
        }
    }
}

fn countdown() -> Response<Body> {
    let from: u32 = 10;
    let ms: u64 = 1000;
    streamed(move |lines| async move {
        let start = Instant::now();
        let _ = lines
            .send(format!("counting down from {from}, one tick every {ms}ms\n"))
            .await;
        tick(&lines, "", from, Duration::from_millis(ms)).await;
        let _ = lines
            .send(format!("done in {:.0}ms\n", start.elapsed().as_secs_f64() * 1000.0))
            .await;
    })
}

fn parallel() -> Response<Body> {
    let n: usize = 3;
    let from: u32 = 10;
    streamed(move |lines| async move {
        let start = Instant::now();
        let _ = lines
            .send(format!(
                "{n} spawned countdowns from {from}; task k ticks every k x 1000ms\n"
            ))
            .await;
        let handles: Vec<_> = (1..=n)
            .map(|k| {
                let lines = lines.clone();
                let label = format!("{}: ", (b'A' + k as u8 - 1) as char);
                tokio::spawn(async move {
                    tick(&lines, &label, from, Duration::from_millis(k as u64 * 1000)).await;
                    let _ = lines.send(format!("{label}done\n")).await;
                })
            })
            .collect();
        for h in handles {
            let _ = h.await;
        }
        let _ = lines
            .send(format!("all done in {:.0}ms\n", start.elapsed().as_secs_f64() * 1000.0))
            .await;
    })
}

async fn sleep_for() -> Response<Body> {
    let ms: u64 = 3000;
    let start = Instant::now();
    sleep(Duration::from_millis(ms)).await;
    plain(
        StatusCode::OK,
        Full::new(Bytes::from(format!(
            "slept {ms}ms (measured {:.0}ms)\n",
            start.elapsed().as_secs_f64() * 1000.0
        )))
        .boxed(),
    )
}
