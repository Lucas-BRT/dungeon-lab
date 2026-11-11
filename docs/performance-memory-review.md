# Benchmarks

How to measure the generator natively.

The native benchmark measures generation (`generate`), generation with validation and attempts (`stage_with_validation`), isolated validation and the metrics computation separately. It includes levels 1–5, a scenario with an irregular grid, rotated shapes and W routes, and a backbone scenario with keys, a switch and pacing enabled.

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
