# Performance

## Speed

Generation cost depends on Λ: the current model evaluates corridor candidates,
βmax with junction actions, topology constraints and leaf chambers.
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

Profile first: one distance query with a limit (instead of testing every segment pair) gave the 10×. SIMD and WebAssembly threads were measured and did not pay.

So the parallelism is: rayon natively (`parallel` feature, on by default; also used by the tests), and in the browser a **pool of Web Workers,
each with its own single-threaded instance** (`apps/site/assets/lab/lab.js`): no shared memory, no special headers, each worker with its own allocator.

## Benchmarks

How to measure the generator natively.

The native benchmark measures generation (`generate`), generation with validation and attempts (`stage_with_validation`), isolated validation and the metrics computation separately. It includes the four sizes with every option on, a scenario with an irregular grid, rotated shapes and W routes, and a backbone scenario.

```sh
cargo bench --bench generator
```

To save and compare a local baseline:

```sh
cargo bench --bench generator -- --save-baseline before
# after an optimization:
cargo bench --bench generator -- --baseline before
```

Criterion stores its data and reports under `target/criterion`. The benchmark measures native Rust code; it does not measure WebAssembly serialization, JSON transfer or drawing in the browser. For heap memory and allocation counts, the next step is to add a profiler such as DHAT or heaptrack and compare it with these same scenarios.

With the library published as `cdylib` and `rlib`, the current Cargo shows an artifact-name collision warning when building the benchmark. The run finishes normally. If a future Cargo version turns that warning into an error, the benchmark output will need to move to a separate native package.
