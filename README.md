# dungeon-lab-wasm

The dungeon model of the game (`docs/dungeon-model.md` in `pokemon-survivors`) rewritten in Rust and compiled to WebAssembly: rooms of
many shapes (rect, disc, octagon, L, truncated star), continuous doors, octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator
(V1 to V5) and the graph's measures (β, δ, bridges, Menger routes, λ₂), with a page to generate any level and edit every parameter of Λ.

It is a faithful port of the TypeScript prototype (`src/dungeonLab` there): the same seed and Λ draw the same stage (the rng is the game's
Mulberry32). Every optimization below was checked against it: `cargo run --release --example dump` hashes 150 stages (5 levels × 30 seeds)
and the hashes did not move, single-threaded or parallel.

## Speed

Per stage (generate + validate), level 1 to 5 of the page's Λ:

| level | TypeScript | Rust, first port | Rust now (1 thread) | WASM now (browser, 1 thread) |
|---|---|---|---|---|
| 3 | 238 ms | 9.9 ms | 1.9 ms | ~2.2 ms |
| 5 | 692 ms | 31.5 ms | 3.1 ms | ~6.5 ms (8.7 ms in the page) |

Larger floors (level 5 Λ on a bigger grid), native: 10×8 grid (~56 rooms) 20.8 ms on 1 thread, 11.0 ms on 14; 16×12 (~132 rooms) 208 ms and 74 ms.
A batch of 200 level-5 stages: 749 ms on 1 thread, 113 ms on 14 (6.6×). In the browser, 800 stages of level 5 take 8.65 s on one worker and
1.09 s on a pool of 14 workers (8×).

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
./build.sh                          # cargo build --release --target wasm32-unknown-unknown --no-default-features + wasm-bindgen → web/pkg
python3 -m http.server 8090 -d web  # then http://localhost:8090/
cargo test --release                # validity of the 5 levels, determinism, density; add `profile`, `scale`, `bench -- --nocapture` for timings
cargo test --release --no-default-features   # the same, single-threaded
cargo run --release --example dump  # a hash per stage, to compare two builds
./build.sh simd && node bench.mjs 5 100      # SIMD vs not, in Node
```

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at the version in `Cargo.toml` (`=0.2.130`).

## Layout

| file | what |
|---|---|
| `src/geom.rs` | directions Θ₈, shapes, distances (exact and with a limit), corridor bands and joints |
| `src/params.rs` | Λ (serde, camelCase JSON as in the prototype) and the five levels |
| `src/generate.rs` | the generator, valid by construction: scaffold, shapes, doors, candidates, tree, cycles with T/X, leaf chambers; per-phase times |
| `src/validate.rs` | V1 to V5 from the stage alone |
| `src/metrics.rs` | β, δ, bridges, articulation rooms, Menger routes, diameter, λ₂ |
| `src/stage.rs` | a valid stage: redraws from a derived seed when the validator objects |
| `src/par.rs` | the one place that decides parallel or sequential (rayon, or plain loops) |
| `src/clock.rs` | `performance.now()` in the browser, the system clock natively |
| `src/lib.rs` | the WASM API: `default_params(level)`, `stage_json(seed, params)`, `bench(level, from, count)`, `threads()` |
| `web/` | the page (`index.html`, `main.js`), the worker pool (`pool.js`, `worker.js`), no bundler |
| `tests/`, `examples/` | model tests, `profile` (time by phase), `scale` (big floors, batch), `dump` (hashes) |

Floats: only `+ − × ÷`, `sqrt` and the libm inside the module, so every browser rounds the same.
