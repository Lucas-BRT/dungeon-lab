# dungeon-lab-wasm

The dungeon model of the game (`docs/dungeon-model.md` in `pokemon-survivors`) rewritten in Rust and compiled to WebAssembly: rooms of
many shapes (rect, disc, octagon, L, truncated star), continuous doors, octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator
(V1 to V5) and the graph's measures (β, δ, bridges, Menger routes, λ₂), with a page to generate any level and edit every parameter of Λ.

It is a faithful port of the TypeScript prototype (`src/dungeonLab` there): the same seed and Λ draw the same stage (the rng is the game's
Mulberry32). Every optimization below was checked against it: `cargo run --release -p dungeon-lab-core --example dump` hashes 150 stages (5 levels × 30 seeds)
and the hashes did not move, single-threaded or parallel.

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
cargo bench --release -p dungeon-lab-core --bench generator
cargo test --release -p dungeon-lab-core scale -- --nocapture
cargo test --release -p dungeon-lab-core --no-default-features scale -- --nocapture
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
cargo run                           # opens the native COSMIC desktop app (workspace default; fast dev build)
cargo run --release                 # optimized desktop build
cargo run --release -p dungeon-lab-web-server  # serves the web frontend on http://localhost:8090/
cargo run --release -p dungeon-lab-web-server -- --port 9000 --host 0.0.0.0 --build   # another port, reachable from a phone, rebuild the module first
./build.sh                          # only the module: cargo build --release --target wasm32-unknown-unknown + wasm-bindgen → web/pkg
cargo test --release -p dungeon-lab-core   # validity of the 5 levels, determinism, density; add `profile`, `scale`, `bench -- --nocapture` for timings
cargo test --release -p dungeon-lab-core --no-default-features   # the same, single-threaded
cargo bench --release -p dungeon-lab-core --bench generator  # Criterion benchmarks
cargo run --release -p dungeon-lab-core --example dump  # a hash per stage, to compare two builds
./build.sh simd && node bench.mjs 5 100      # SIMD vs not, in Node
```

The code is split into workspace packages so frontend changes do not require rebuilding the generator, WASM bindings, web server, and desktop app together.
Cargo keeps one shared `target/` cache: `cargo run` builds only the default desktop app and its core dependency; `./build.sh` builds the WASM package and core; the web server is compiled only when selected. The desktop enables only the libcosmic features it uses: Tokio, accessibility, X11, and Wayland. For everyday UI work, use the incremental development profile with plain `cargo run`; release builds retain optimization and thin LTO for performance measurements.

The root virtual workspace sets only `apps/cosmic` as its default member, so plain `cargo run` still opens the desktop app. To build a package directly, use `cargo build -p dungeon-lab-core`, `cargo build -p dungeon-lab-wasm --target wasm32-unknown-unknown`, or `cargo build -p dungeon-lab-web-server`. Cargo compiles the selected package and its dependency graph, rather than every workspace member. Development builds omit debug symbols and optimize only the generator core; release builds use thin LTO and multiple codegen units to shorten link times while retaining native optimization.

The server is `apps/web-server/src/main.rs` (axum + tower-http's `ServeDir`): it serves `web/` with the right types (`.wasm` as `application/wasm`) and no cache,
so an edit to the page or a rebuilt module shows on reload. `PORT` also sets the port.

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at the version in `crates/dungeon-wasm/Cargo.toml` (`=0.2.130`).

The native COSMIC frontend is the workspace's default application. It calls the shared Rust generator directly with Rayon enabled and renders the map
without WebAssembly. Its controls include all level presets and Λ parameters, guided experiments, JSON editing and copying, five map overlays,
pan/zoom/hover inspection, validation, and metrics. Each interactive request generates one floor; a 2D Canvas preview draws rooms and corridor
connections as the generator creates them. A separate benchmark remains for comparing single-thread and multi-thread throughput over many independent maps.
`libcosmic` is fetched from the upstream COSMIC toolkit repository, so building this binary needs network access on the first build and the platform
libraries listed in the [libcosmic README](https://github.com/pop-os/libcosmic#dependencies).

The desktop's single-map latency and progressive-rendering design is documented in [docs/desktop-generation-rendering.md](docs/desktop-generation-rendering.md).

## Layout

| file | what |
|---|---|
| `crates/dungeon-core/src/geom.rs` | directions Θ₈, shapes, distances (exact and with a limit), corridor bands and joints |
| `crates/dungeon-core/src/params.rs` | Λ (serde, camelCase JSON as in the prototype) and the five levels |
| `crates/dungeon-core/src/generate.rs` | the generator, valid by construction: scaffold, shapes, doors, candidates, tree, cycles with T/X, leaf chambers; per-phase times |
| `crates/dungeon-core/src/validate.rs` | V1 to V5 from the stage alone |
| `crates/dungeon-core/src/metrics.rs` | β, δ, bridges, articulation rooms, Menger routes, diameter, λ₂ |
| `crates/dungeon-core/src/stage.rs` | a valid stage: redraws from a derived seed when the validator objects |
| `crates/dungeon-core/src/par.rs` | the one place that decides parallel or sequential (rayon, or plain loops) |
| `crates/dungeon-core/src/clock.rs` | `performance.now()` in the browser, the system clock natively |
| `apps/cosmic/` | native COSMIC desktop application; selected by bare `cargo run` |
| `apps/web-server/` | local HTTP server for the browser frontend (`cargo run -p dungeon-lab-web-server`) |
| `crates/dungeon-wasm/` | the WASM API: `default_params(level)`, `stage_json(seed, params)`, `bench(level, from, count)`, `threads()` |
| `crates/dungeon-core/tests/`, `benches/`, `examples/` | model tests, Criterion benchmarks, profiling and deterministic dumps |
| `web/` | the page (`index.html`, `main.js`), the worker pool (`pool.js`, `worker.js`), no bundler |

Floats: only `+ − × ÷`, `sqrt` and the libm inside the module, so every browser rounds the same.
