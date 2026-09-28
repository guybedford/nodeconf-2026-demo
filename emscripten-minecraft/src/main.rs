//! Pumpkin under Node.js through plain Emscripten: Tokio's hosted
//! `LocalEventLoop` is driven by Node's own event loop, the Java listener is a
//! Node `net.Server` through NODERAWSOCKETS and the world lives on the host
//! filesystem through NODERAWFS. `main` spawns the server and returns; the
//! process lives until the server has stopped and saved.

mod config;
mod settings;

use pumpkin::{data::VanillaData, PumpkinServer};
use std::cell::Cell;
use tokio::runtime::{Builder, LocalEventLoop};

thread_local! {
    static EVENT_LOOP: Cell<Option<&'static LocalEventLoop>> = const { Cell::new(None) };
}

fn event_loop() -> &'static LocalEventLoop {
    EVENT_LOOP.with(|slot| slot.get().expect("event loop not started"))
}

extern "C" {
    fn emscripten_run_script(script: *const std::ffi::c_char);
}

fn run_js(script: &str) {
    let script = std::ffi::CString::new(script).unwrap();
    // SAFETY: a NUL-terminated script evaluated on the host.
    unsafe { emscripten_run_script(script.as_ptr()) }
}

fn main() {
    std::panic::set_hook(Box::new(|info| eprintln!("RUST PANIC: {info}")));
    // The event loop outlives `main`: the drives it schedules on the host are
    // what run the server.
    let el: &'static LocalEventLoop = Box::leak(Box::new(
        Builder::new_current_thread()
            .enable_all()
            .build_hosted_local_event_loop(Default::default())
            .expect("tokio event loop"),
    ));
    EVENT_LOOP.with(|slot| slot.set(Some(el)));
    // There are no Rayon workers: spawned jobs wait until this thread yields
    // to Rayon. A wake fires once per idle-to-pending transition, so the
    // driver runs jobs until the queue is idle, one per turn of the event
    // loop, and stops until the next wake.
    let _ = rayon::set_fallback_wake_hook(drive_rayon);
    el.spawn_local(async {
        if let Err(error) = run().await {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    });
    // Ctrl+C stops the server, which saves and exits; a second one is forced.
    run_js(
        "process.on('SIGINT', () => { \
           if (Module.stopping) process.exit(130); \
           Module.stopping = true; \
           console.error('stopping...'); \
           Module._stop(); \
         })",
    );
}

/// Called from the host to request a clean stop.
#[no_mangle]
pub extern "C" fn stop() {
    pumpkin::stop_server();
}

async fn run() -> Result<(), String> {
    let settings = settings::Settings::from_env()?;
    std::fs::create_dir_all(&settings.data_dir).map_err(|e| e.to_string())?;
    std::env::set_current_dir(&settings.data_dir).map_err(|e| e.to_string())?;
    pumpkin::reset_stop();
    let (basic, advanced) = config::configuration(&settings);
    pumpkin::init_logger(&advanced);
    let started = std::time::Instant::now();
    let server = PumpkinServer::new(basic, advanced, VanillaData::load()).await;
    server.init_plugins().await;
    println!(
        "Minecraft server listening on port {} ({} ms startup)",
        settings.port,
        started.elapsed().as_millis()
    );
    // Returns after the final save once stopped.
    server.start().await;
    println!("done");
    Ok(())
}

/// Runs one queued Rayon job per turn of the event loop until the fallback
/// queue is idle.
fn drive_rayon() {
    event_loop().spawn_local(async {
        if rayon::yield_now() == Some(rayon::Yield::Executed) {
            drive_rayon();
        }
    });
}
