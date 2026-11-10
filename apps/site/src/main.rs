//! The Tapiti website. `cargo run -p tapiti-site` serves it on http://127.0.0.1:3000 (`HOST` and `PORT` override).

mod app;
mod docs;
mod floor;

#[tokio::main]
async fn main() {
    if std::env::args().any(|a| a == "--healthcheck") {
        std::process::exit(if app::healthcheck() { 0 } else { 1 });
    }
    topcoat::start(app::router()).await.unwrap();
}
