//! The dev server of the page: `cargo run` serves `web/` (the page, the worker pool and the WebAssembly module in `web/pkg`) on
//! http://localhost:8090/. If the module is not built yet it runs `build.sh` first.
//!
//! `cargo run -- --port 9000`, `--host 0.0.0.0` (to open it from a phone), `--build` (rebuild the module before serving). `PORT` also sets the port.

use axum::http::{header, HeaderValue};
use axum::Router;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

struct Options {
    host: String,
    port: u16,
    build: bool,
}

fn options() -> Result<Options, String> {
    let mut o = Options { host: "127.0.0.1".into(), port: std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8090), build: false };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" | "-p" => o.port = args.next().and_then(|v| v.parse().ok()).ok_or("--port needs a number")?,
            "--host" => o.host = args.next().ok_or("--host needs an address")?,
            "--build" => o.build = true,
            "--help" | "-h" => return Err("usage: cargo run -- [--port N] [--host ADDR] [--build]".into()),
            other => return Err(format!("unknown option {other} (try --help)")),
        }
    }
    Ok(o)
}

/// Runs `build.sh` (cargo build for wasm32 + wasm-bindgen into `web/pkg`).
fn build_module(root: &Path) -> Result<(), String> {
    println!("building the WebAssembly module (./build.sh)…");
    let status = Command::new("bash").arg("build.sh").current_dir(root).status().map_err(|e| format!("could not run build.sh: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("build.sh failed (it needs the wasm32-unknown-unknown target and wasm-bindgen 0.2.130)".into())
    }
}

#[tokio::main]
async fn main() {
    let o = match options() {
        Ok(o) => o,
        Err(m) => {
            eprintln!("{m}");
            std::process::exit(2);
        }
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let web = root.join("web");
    if o.build || !web.join("pkg/dungeon_lab_bg.wasm").exists() {
        if let Err(m) = build_module(&root) {
            eprintln!("{m}");
            std::process::exit(1);
        }
    }

    // ServeDir answers 404 for a missing file and guesses each type from its extension (.wasm is application/wasm); the module and the page
    // change while developing, so nothing is cached.
    let app = Router::new().fallback_service(ServeDir::new(&web).append_index_html_on_directories(true)).layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("no-store")));

    let addr: SocketAddr = match format!("{}:{}", o.host, o.port).parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("bad address {}:{}: {e}", o.host, o.port);
            std::process::exit(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("dungeon lab: http://localhost:{}/  (Ctrl-C stops)", o.port);
    let stop = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(stop).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
