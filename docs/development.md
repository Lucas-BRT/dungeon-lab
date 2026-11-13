# Development

How to run, build and check Tapiti, and where things are in the repository. To install a release instead, see [getting started](getting-started#install).

## Run

```bash
cargo run                           # opens the native desktop app (egui) (workspace default; fast dev build)
cargo run --release                 # optimized desktop build
scripts/build-wasm.sh && topcoat dev -p tapiti-site     # the website (landing, docs, lab) on http://127.0.0.1:3000/, rebuilt on every change (`cargo install topcoat-cli`)
docker compose -f compose.local.yml up --build   # the same site as a container on http://localhost:8090/
node scripts/lab-smoke.mjs                   # smoke test of the lab in headless Firefox (site running)
node scripts/site-smoke.mjs                  # the landing page's 3D walk, the 404, reduced motion (site running; AXE_JS=… adds axe-core)
node scripts/site-shots.mjs shots            # screenshots of the pages at desktop and phone width
node scripts/og-card.mjs                     # remakes the social card from the 3D scene
scripts/build-wasm.sh                          # only the module: cargo build --release --target wasm32-unknown-unknown + wasm-bindgen → apps/site/pkg
cargo test --release -p tapiti-core   # validity of the 5 levels, determinism, density; add `profile`, `scale`, `bench -- --nocapture` for timings
cargo test --release -p tapiti-core --no-default-features   # the same, single-threaded
cargo bench --release -p tapiti-core --bench generator  # Criterion benchmarks
cargo run --release -p tapiti-core --example dump  # a hash per stage, to compare two builds
scripts/build-wasm.sh simd && node scripts/bench-wasm.mjs 5 100      # SIMD vs not, in Node
```

## Workspace

The code is split into workspace packages so frontend changes do not require rebuilding the generator, WASM bindings, web server, and desktop app together.
Cargo keeps one shared `target/` cache: `cargo run` builds only the default desktop app and its core dependency; `scripts/build-wasm.sh` builds the WASM package and core; the site is compiled only when selected. The desktop (egui/eframe, glow renderer) enables only the Wayland and X11 backends. For everyday UI work, use the incremental development profile with plain `cargo run`; release builds retain optimization and thin LTO for performance measurements.

The root virtual workspace sets only `apps/desktop` as its default member, so plain `cargo run` still opens the desktop app. To build a package directly, use `cargo build -p tapiti-core`, `cargo build -p tapiti-wasm --target wasm32-unknown-unknown`, or `cargo build -p tapiti-site`. Cargo compiles the selected package and its dependency graph, rather than every workspace member. Development builds omit debug symbols and optimize only the generator core; release builds use thin LTO and multiple codegen units to shorten link times while retaining native optimization.

## The website

The website is a [Topcoat](https://github.com/tokio-rs/topcoat) app (`apps/site`): server-rendered pages in English, with the browser lab at `/lab` (a Topcoat page; its script, `apps/site/assets/lab/`, drives the WebAssembly module served from `/lab/pkg/`). `HOST` and `PORT` set the address; `TAPITI_SCENE=off` turns off the 3D scene behind the landing page (the dimmed map stays); `compose.yml` is the Dokploy deployment.

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at the version in `crates/tapiti-wasm/Cargo.toml` (`=0.2.130`).

## Determinism

The same seed and Λ always draw the same stage (the rng is Mulberry32). Every optimization was checked against that: `cargo run --release -p tapiti-core --example dump`
hashes 150 stages (the four sizes × 30 seeds, then 30 grown floors) and the hashes did not move, single-threaded or parallel.

## Layout of the repository

| file | what |
|---|---|
| `crates/tapiti-core/src/geom.rs` | directions Θ₈, shapes, distances (exact and with a limit), corridor bands and joints |
| `crates/tapiti-core/src/params.rs` | Λ (serde, camelCase JSON) and the five levels |
| `crates/tapiti-core/src/generate.rs` | the generator, valid by construction: scaffold, shapes, doors, candidates, tree, cycles with T/X, leaf chambers; per-phase times |
| `crates/tapiti-core/src/validate.rs` | V1 to V5 from the stage alone |
| `crates/tapiti-core/src/metrics.rs` | β, δ, bridges, articulation rooms, Menger routes, diameter, λ₂ |
| `crates/tapiti-core/src/stage.rs` | a valid stage: redraws from a derived seed when the validator objects |
| `crates/tapiti-core/src/par.rs` | the one place that decides parallel or sequential (rayon, or plain loops) |
| `crates/tapiti-core/src/clock.rs` | `performance.now()` in the browser, the system clock natively |
| `apps/desktop/` | native desktop application (egui); selected by bare `cargo run` |
| `apps/site/` | the website: landing page, docs and the lab (Topcoat; `topcoat dev -p tapiti-site`) |
| `crates/tapiti-render/` | SVG rendering of floors for the site, docs and social images |
| `crates/tapiti-schema/` | the parameters of Λ as data (groups, ranges, help), shared by the desktop app and the docs |
| `docs/` | the documentation (Markdown, math as `$…$`), rendered by the site and readable on GitHub |
| `crates/tapiti-wasm/` | the WASM API: `default_params(size)`, `rich_params(size)`, `stage_json(seed, params)`, `bench(size, from, count)`, `threads()` |
| `crates/tapiti-core/tests/`, `benches/`, `examples/` | model tests, Criterion benchmarks, profiling and deterministic dumps |

Floats: only `+ − × ÷`, `sqrt` and the libm inside the module, so every browser rounds the same.
