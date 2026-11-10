# Tapiti

Tapiti digs dungeon floors from a seed, in Rust and WebAssembly: rooms of many shapes (rect, disc, octagon, L, truncated star), continuous doors,
octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator (V1 to V5) and the graph's measures (β, δ, bridges, Menger routes, λ₂), with a
desktop app and a page to generate any level and edit every parameter of Λ.

The same seed and Λ always draw the same stage (the rng is Mulberry32). Every optimization below was checked against that: `cargo run --release -p tapiti-core --example dump`
hashes 150 stages (5 levels × 30 seeds) and the hashes did not move, single-threaded or parallel.

## Speed

Generation cost depends on Λ: the current model evaluates corridor candidates,
βmax with junction actions, topology constraints, progression, and leaf chambers.
Do not compare it with measurements from the earlier, smaller model.

Recent native release measurements on the development machine (14 Rayon threads)
show level 5 generation around 22–28 ms and generation plus validation around
24–27 ms. The `scale` benchmark measured:

| workload | 1 thread | 14 threads |
|---|---:|---:|
| 10×8 grid, about 54 rooms | 448 ms/map | 396 ms/map |
| 16×12 grid, about 130 rooms | 4,668 ms/map | 966 ms/map |
| batch of 200 level-5 maps | 12,958 ms | 2,114 ms |

The small default map has limited independent work, so its speedup is modest.
Large maps and batches expose enough work to benefit from the Rayon pool. Use
Criterion for comparable single-map measurements and `scale` for throughput:

```bash
cargo bench --release -p tapiti-core --bench generator
cargo test --release -p tapiti-core scale -- --nocapture
cargo test --release -p tapiti-core --no-default-features scale -- --nocapture
```

### What it took, in the order it paid

1. **A distance query with a limit (`shape_within`, `region_within`).** The profile said 77% of a stage was `seg_seg`, 1.66 million calls per
   stage, almost all asking "is this closer than σ?". The query skips a pair of edges whose boxes are σ apart and stops at the first pair that
   is closer. `seg_seg` now runs ~2,000 times per stage (1% of it). This alone is the 10×.
2. **Corridor candidates computed up front, in parallel.** They depend only on the rooms and every pair is needed anyway, so one batch
   split over rayon's threads (`par.rs`) gives the same list in the same order: 21 ms → 5 ms on 14 threads in the first port.
3. **T branches indexed by axis.** A door only meets segments perpendicular to it; filing the segments by axis (in edge order, so the choices
   made are the same) halved the cycles phase on big grids.
4. Remembering for good what can never fit again (corridors and doors only accumulate), and doors checked after the cheap geometry.

### What did not pay (measured)

* **SIMD.** `+simd128` made no difference (6.47 vs 6.47 ms; the binary does not even change size): after step 1 the geometry kernel is 1% of the
  time, so there is nothing for 2-lane `f64` vectors to speed up. `./build.sh simd` and `node bench.mjs` keep the comparison.
* **WebAssembly threads** (rayon over Web Workers sharing one memory, `wasm-bindgen-rayon`). It built and ran in the browser, and was about
  16× *slower* than one thread (106 ms vs 6.5 ms per stage, level 5): the generator allocates many small `Vec`s, and the allocator's global lock
  serializes the threads. It also needs a cross-origin isolated page. Removed.
* **Threads inside one small stage, natively.** At 3 ms a stage there is too little work to split (2.7 ms vs 3.1 ms); the threads pay on big
  floors and batches, which is why `par.rs` keeps a size threshold on each split.

So the parallelism is: rayon natively (`parallel` feature, on by default; also used by the tests), and in the browser a **pool of Web Workers,
each with its own single-threaded instance** (`web/pool.js`): no shared memory, no special headers, each worker with its own allocator.

## Run

```bash
cargo run                           # opens the native desktop app (egui) (workspace default; fast dev build)
cargo run --release                 # optimized desktop build
./build.sh && topcoat dev -p tapiti-site     # the website (landing, docs, lab) on http://127.0.0.1:3000/, rebuilt on every change (`cargo install topcoat-cli`)
docker compose -f compose.local.yml up --build   # the same site as a container on http://localhost:8090/
./build.sh                          # only the module: cargo build --release --target wasm32-unknown-unknown + wasm-bindgen → web/pkg
cargo test --release -p tapiti-core   # validity of the 5 levels, determinism, density; add `profile`, `scale`, `bench -- --nocapture` for timings
cargo test --release -p tapiti-core --no-default-features   # the same, single-threaded
cargo bench --release -p tapiti-core --bench generator  # Criterion benchmarks
cargo run --release -p tapiti-core --example dump  # a hash per stage, to compare two builds
./build.sh simd && node bench.mjs 5 100      # SIMD vs not, in Node
```

The code is split into workspace packages so frontend changes do not require rebuilding the generator, WASM bindings, web server, and desktop app together.
Cargo keeps one shared `target/` cache: `cargo run` builds only the default desktop app and its core dependency; `./build.sh` builds the WASM package and core; the site is compiled only when selected. The desktop (egui/eframe, glow renderer) enables only the Wayland and X11 backends. For everyday UI work, use the incremental development profile with plain `cargo run`; release builds retain optimization and thin LTO for performance measurements.

The root virtual workspace sets only `apps/desktop` as its default member, so plain `cargo run` still opens the desktop app. To build a package directly, use `cargo build -p tapiti-core`, `cargo build -p tapiti-wasm --target wasm32-unknown-unknown`, or `cargo build -p tapiti-site`. Cargo compiles the selected package and its dependency graph, rather than every workspace member. Development builds omit debug symbols and optimize only the generator core; release builds use thin LTO and multiple codegen units to shorten link times while retaining native optimization.

The website is a [Topcoat](https://github.com/tokio-rs/topcoat) app (`apps/site`): server-rendered pages in English and Portuguese (`/en`, `/pt-BR`), with the browser lab at `/lab` (the WebAssembly module and `web/`, served from fixed URLs). `HOST` and `PORT` set the address; `compose.yml` is the Dokploy deployment.

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at the version in `crates/tapiti-wasm/Cargo.toml` (`=0.2.130`).

The native egui frontend is the workspace's default application. It calls the shared Rust generator directly with Rayon enabled and renders the map
without WebAssembly. The map fills the window; a right-hand panel has three tabs (floor reading and layout, every Λ parameter, analysis). Controls: all level
presets, Λ parameters with per-field reset and a filter, guided experiments, JSON editing and copying, five map overlays and a legend,
pan/zoom/hover inspection, validation, and metrics. Edits regenerate the map after a short pause. Each interactive request generates one floor;
a slow generation shows rooms and corridor connections as the generator creates them. A separate benchmark compares single-thread and
multi-thread throughput over many independent maps. Shortcuts: Alt+1–5 level, Alt+R random seed, Alt+left/right seed.
The UI is in English by default; the language selector in the bottom bar switches to Portuguese. Texts use `t!("English text")` / `tf!("… {name}", name = x)`
(`apps/desktop/src/i18n.rs`), and a language is one table of `(english, translation)` pairs, e.g. `i18n/pt.rs`; a unit test fails when a text has no
entry, an entry is unused, or the placeholders differ. The choice is not persisted between runs.
Building needs the usual eframe system libraries on Linux (xkbcommon, Wayland and/or X11, OpenGL).

The desktop's single-map latency and progressive-rendering design is documented in [docs/desktop-generation-rendering.md](docs/desktop-generation-rendering.md).

## Layout

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
| `crates/tapiti-schema/` | the parameters of Λ as data (groups, ranges, help in English and Portuguese), shared by the desktop app and the docs |
| `docs/`, `docs/pt-BR/` | the documentation (Markdown, math as `$…$`), rendered by the site and readable on GitHub |
| `crates/tapiti-wasm/` | the WASM API: `default_params(level)`, `stage_json(seed, params)`, `bench(level, from, count)`, `threads()` |
| `crates/tapiti-core/tests/`, `benches/`, `examples/` | model tests, Criterion benchmarks, profiling and deterministic dumps |
| `web/` | the page (`index.html`, `main.js`), the worker pool (`pool.js`, `worker.js`), no bundler |

Floats: only `+ − × ÷`, `sqrt` and the libm inside the module, so every browser rounds the same.

## License

[MIT](LICENSE).
