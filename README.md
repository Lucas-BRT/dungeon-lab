# dungeon-lab-wasm

The dungeon model of the game (`docs/dungeon-model.md` in `pokemon-survivors`) rewritten in Rust and compiled to WebAssembly: rooms of
many shapes (rect, disc, octagon, L, truncated star), continuous doors, octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator
(V1 to V5) and the graph's measures (β, δ, bridges, Menger routes, λ₂), with a page to generate any level and edit every parameter of Λ.

The generator is a faithful port of the TypeScript prototype (`src/dungeonLab` there): the same seed and Λ draw the same stage (the rng
is the game's Mulberry32), and the measures over 25 seeds per level match. What changed is the cost:

| level | TypeScript (per stage) | Rust native | WASM in the browser |
|---|---|---|---|
| 1 | 58 ms | 3.4 ms | |
| 3 | 238 ms | 9.9 ms | |
| 5 | 692 ms | 31.5 ms | ~54 ms |

Per stage: generation, validation and measures. The page's "benchmark" button measures the module alone. Beyond the port, every shape
carries its bounding box, so far-apart shapes skip their exact distance test, and rooms keep theirs.

## Run

```bash
./build.sh                          # cargo build --release --target wasm32-unknown-unknown + wasm-bindgen → web/pkg
python3 -m http.server 8090 -d web  # then http://localhost:8090/
cargo test --release                # validity of the 5 levels, determinism, density; add `bench -- --nocapture` for the timings
```

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI at the version in `Cargo.toml` (`=0.2.130`).

## Layout

| file | what |
|---|---|
| `src/geom.rs` | directions Θ₈, shapes, distance between shapes (with boxes), corridor bands and joints |
| `src/params.rs` | Λ (serde, camelCase JSON as in the prototype) and the five levels |
| `src/generate.rs` | the generator, valid by construction: scaffold, shapes, doors, candidates, tree, cycles with T/X, leaf chambers |
| `src/validate.rs` | V1 to V5 from the stage alone |
| `src/metrics.rs` | β, δ, bridges, articulation rooms, Menger routes, diameter, λ₂ |
| `src/stage.rs` | a valid stage: redraws from a derived seed when the validator objects |
| `src/lib.rs` | the WASM API: `default_params(level)`, `stage_json(seed, params)`, `bench(level, n)` |
| `web/` | the page (`index.html`, `main.js`), no bundler |

Floats: only `+ − × ÷`, `sqrt` and the libm inside the module, so every browser rounds the same.
