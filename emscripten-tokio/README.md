# Tokio on Emscripten in Node.js

A Rust [hyper](https://hyper.rs) HTTP server on Tokio, compiled to
`wasm32-unknown-emscripten` and run directly by Node, with no wasm-bindgen and
no runtime shim.

```sh
npm run build
npm run start
```

The first build installs the pinned Emscripten SDK into `../.work` and applies
the patches in `../patches/emscripten` (see `../setup.sh`).

Tokio runs as a hosted [`LocalEventLoop`]: a `current_thread` runtime whose
wait *is* the Node event loop. Where a native runtime parks its thread, the
event loop returns to Node and schedules its own drives for timer deadlines and
socket readiness (via `epoll` listeners). There is no `#[tokio::main]`, no
`block_on`, no JSPI or Asyncify: `main` spawns the server and returns, and Node
stays alive while the runtime has tasks.

```rs
let rt = Builder::new_current_thread()
    .enable_all()
    .build_hosted_local_event_loop(LocalOptions::default())?;
rt.spawn_local(serve(port));
std::mem::forget(rt);
```

`TcpListener`, `tokio::spawn`, `tokio::time`, `tokio::sync` and `tokio::join!`
then work inside plain `async fn`s as they do natively, and hyper serves each
accepted connection over `TokioIo` unmodified.

| Route | Shows |
| --- | --- |
| `curl -N localhost:8787/countdown` | a streamed countdown from 10, one chunk per `tokio::time::sleep` tick |
| `curl -N localhost:8787/parallel` | 3 `tokio::spawn`ed countdowns at different rates, interleaved through an `mpsc` into one streamed body |
| `curl localhost:8787/sleep` | `tokio::time::sleep` for 3s |

Start a countdown, then open another URL a few seconds later: both progress,
since every wait yields to the Node event loop. Set `PORT` to change the
listening port.

## Patchset

Upstream PRs are in progress; until they land, `Cargo.toml` patches in:

* Tokio: https://github.com/tokio-rs/tokio/pull/8484 (the event loop, Emscripten `net`)
* mio: https://github.com/tokio-rs/mio/pull/1969 (Emscripten epoll reactor)
* libc: the `libc-0.2` branch (Emscripten socket definitions, pending 0.2.190)

And on the Emscripten side (`../patches/emscripten`):

* `epoll-listeners.patch`: `emscripten_epoll_add_listener`, host-macrotask readiness delivery ([emscripten#27547](https://github.com/emscripten-core/emscripten/pull/27547))
* `noderawsockets-dns.patch`: `emscripten_dns_lookup_async`, non-blocking hostname lookup under `-sNODERAWSOCKETS` ([emscripten#27742](https://github.com/emscripten-core/emscripten/pull/27742))

[`LocalEventLoop`]: https://github.com/tokio-rs/tokio/pull/8484
