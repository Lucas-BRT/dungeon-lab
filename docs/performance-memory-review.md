# Performance and memory audit

This audit covers the structs and enums in `src/`, prioritizing what lives in vectors, what duplicates geometry, and what can grow with the number of rooms and corridors.

## How to read the sizes

The numbers below come from `std::mem::size_of` on the native 64-bit target. They measure only the inline space of the value: they do not include allocations of `Vec`, `String`, `Box` or maps, nor the reserved capacity of those containers.

| Type | Bytes | Note |
|---|---:|---|
| `Shape` | 32 | Disc or polygon enum; the two largest payloads are 24 bytes. |
| `Bounds` | 32 | Two points, no geometry of its own. |
| `Bounded` | 64 | Inline shape plus bounds; used in collision regions. |
| `ShapeClass` / `Proximity` / `TopologyProfile` / `Role` / `RouteType` | 1 each | Compact enumerations. |
| `ByClass` | 40 | Five `f64`; a fitting size for fixed values. |
| `TopologySpec` | 48 | Five requirements and the profile. |
| `ProgressionSpec` | 16 | Two `usize` counters; active items are capped at eight of each kind. |
| `PacingSpec` | 1 | One boolean option. |
| `LeafRoles` | 24 | Three `f64` weights. |
| `Params` | 488 | Includes a `BTreeMap<String, f64>` for shape compatibility. |
| `Port` | 40 | Position, direction, edge index, room and inset. |
| `Rect` | 32 | The room's allocation rectangle on the grid. |
| `Room` | 184 | Shape, bounds, ports, used ports and role data. |
| `Node` | 16 | A junction's position is indirect through `Box<V>`. |
| `Edge` | 176 | Includes two optional ends, polyline and the collision-region vector. |
| `Candidate` | 168 | Full candidate used by the search, with ports, weights and region. |
| `CandidateView` | 48 | Reduced version sent to the viewer. |
| `Dungeon` | 784 | Aggregates the parameters and the floor's vectors. |
| `Metrics` | 208 | Scalar results and the count per class. |
| `Stage` | 816 | Holds the `Dungeon`, violations and number of attempts. |
| `Mulberry` / `Const` | 4 / 8 | State of the random number generators. |

## Optimizations applied

- **One shape per room.** Before, `Room` stored the shape in `shape` and a second copy inside `bb: Bounded`. Now it stores `shape` and `Bounds`; this saves 32 inline bytes per room and avoids duplicating the polygon's vertex vector.
- **Smaller nodes.** `Node` was 24 bytes because the `[f64; 2]` position of the `Junction` variant was embedded. The position now uses `Box<V>`: each node takes 16 bytes, with one extra allocation per junction. The JSON still presents `p` as a coordinate pair.
- **Smaller output candidates.** The search uses `Candidate`; the `Dungeon` keeps `CandidateView`, which holds only the IDs and the polyline the page uses. The inline type drops from 168 to 48 bytes per retained candidate, and collision regions and auxiliary ports are no longer kept in the result.
- **No candidate clones between phases.** Live candidates are moved from the cache to the static list. After the tree is built, the search's temporary vectors and the pair cache are freed before the cycles phase.
- **T option without a duplicate port.** `Action::Branch` stored the port and also that port's index in the room. The action now keeps the index and recovers the value when it needs it.
- **Contiguous matrix.** The dense matrix used to compute $\lambda_2$ now takes a single `Vec<f64>`, avoiding one allocation and one `Vec` header per row.
- **Smaller JSON payload.** `Edge.region` is still available for collision and validation in Rust, but is not serialized because the viewer uses the `pts` polyline.
- **Shortcuts for the common case.** Validation skips the state-space search when there are no locks; the classic profile skips cut, hub and branch computations when it does not require them. The shortest-cycle metric also returns early when $\beta=0$.

## Review of the other types

### Geometry

`Shape` is not a good target for putting a `Box` only on the polygon: the disc and the point vector take 24 bytes each, so that change would not shrink the enum. Putting both variants behind pointers would reduce the inline size, but would add an allocation and an indirection to every shape, including the many temporary shapes of corridor regions.

`Bounded` still takes 64 bytes and appears in vectors of corridor regions. It is one of the next targets to measure. Reducing it requires separating geometric storage from bounds without spreading boxes and clones through every distance operation.

### Parameters

`ByClass`, `LeafRoles`, `ProgressionSpec` and `PacingSpec` are small fixed values. `Params` is large mostly because of `mix`, which stores 25 `String` keys and values in a `BTreeMap`. A `[[f64; 5]; 5]` matrix would reduce allocations and make access more direct, but it would require keeping the `"class-class"` JSON format at the serialization boundary. Since there are only 25 entries and the conversion affects the interface, this should be compared in the benchmark before changing.

### Graph and search

`Port` is copied among possible ports, used ports, edge ends and candidates. The `room` and `edge` fields may be redundant when the port is stored inside a room, but a port can be shifted continuously to align a corridor. Replacing them with indices requires preserving that case without losing the exact position.

`Edge` is large and holds geometry that must stay alive during validation. `ends` can be simplified if the port model is redone as a compact reference, but shifted ports prevent replacing each value with a plain index without a semantic change. `region` is not sent to the browser.

`Candidate` is large by design while the search is active: the weights, ports and regions are needed to evaluate and apply actions. The final list uses the smaller `CandidateView` type. `Dungeon` and `Stage` carry these values by composition; putting the floor behind a `Box` would reduce the parent value's size, but not the total memory, and would add an allocation.

`Action`, `Opt`, `SegRef`, `Changes` and `Block` are temporary helpers. `Action::Branch` lost the duplicated field; `Block` is a `Copy` value of four integers. `Gen` concentrates caches and temporary maps, and freeing between phases avoids keeping that data until serialization.

### Metrics, validation and infrastructure

`Metrics` is mostly numbers and the final map of counts. The Laplacian matrix, which is not part of the result, now uses contiguous memory. `Tarjan` keeps per-node visit vectors; that memory is linear in the graph size and needed for bridges and articulation points.

The residual `Arc` of the flow and the progression `State` are internal. The state space is bounded by eight key bits and eight switch bits; in the worst case there are up to $2^{16}$ combinations per node. That space is only traversed when there is at least one lock.

`SpatialHash` uses a `HashMap` of cells and index vectors. This representation avoids comparing every geometric pair on larger maps; the bucket overhead should be measured on small and large maps before replacing it.

`Answer` holds references to the result and the metrics. `Options` holds the server's host and arguments. `Mulberry` and `Const` are small states. None of them dominates the generator's memory use.

`Rect` takes 32 bytes inside each room and is not used by the page's current code. It still goes out in the JSON as `box` and may be useful to other consumers. A future step could replace it with integer block coordinates or remove it after confirming that no external consumer depends on that field.

## Rust API structure

Associated methods already appear where state or rules belong to a type: `Gen` holds the search and the caches; `SpatialHash` controls its buckets; `TopologySpec::requirements` turns the profile into effective requirements. The operations in `geom` are pure functions over shapes and vectors, so grouping them into methods would not by itself bring more safety.

The most promising point for a typed API is `topology`: today functions such as `shortest_path` receive `node_count`, `edges`, `start` and `goal` separately, and each may rebuild the adjacency. A `NavigationGraph` or `GraphView` could validate the endpoints once, keep the adjacency and expose methods such as `shortest_path`, `edge_connectivity` and `cycle_girth`. `metrics` and `validate` could share that view. It is a larger refactor and should be compared in Criterion, since keeping the adjacency also has a memory cost.

## Criterion benchmarks

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

A short run during this audit, with 10 samples and 0.5 s of collection, estimated: generation of levels 1–5 at 2.60 ms, 4.14 ms, 15.66 ms, 26.19 ms and 21.15 ms; extended scenario at 91.68 ms; progression scenario at level 5 at 34.33 ms; level 5 validation at 573 µs; metrics at 61.5 µs. These numbers are an initial reference for the current native target, not a before/after comparison nor a maximum-precision measurement. For small decisions, use the longer default collection and repeat on an idle machine.
