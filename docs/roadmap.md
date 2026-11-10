# Roadmap

Tapiti is young. This page says plainly what is unfinished, so you know what to rely on.

## Growth layout (prototype)

Besides the default layout (one room per grid block), the generator can *grow* rooms one from another, off a parent's door, in free space. It gives an organic, non-lattice floor and scales well to large worlds (about 165 ms for 81 rooms on a 16×12 grid), but it is marked as a prototype because:

- **It often stops short of the room count you ask for.** On the default parameters it reaches roughly 75 to 80% of the rooms the grid layout would give.
- **Too many floors are rejected for the stairs being too close to the entrance** (5 to 10% fail after all draws on levels 1 and 5, and floors cost two to four draws instead of one). Growth should favour depth, and the interface should check `depth` against `minHops`.
- **Topology profiles mostly do not fit it** (`backbone`, `redundantExit` and `longLoop` rarely validate); `hub` works.
- **Leaf chambers are not generated**, although their parameters stay visible.
- **It has no automated tests** yet, and the old `frontier-growth` experiment still sits in the generator behind a feature flag.
- **The browser lab has no controls for it.**

## Other open items

- **Junction conductance** (`junction`) is a parameter of the reference model that the generator does not use yet.
- **Delaunay proximity** is in the model but not in the generator (RNG, Gabriel and Near are).
- **Global scoring of topology profiles**, **mission objectives** and **multi-floor campaigns** are described in the [model](model) as proposals.
- **The lab as the desktop app.** The browser lab is the original page. Compiling the desktop interface (egui) to WebAssembly would unify them, and is postponed.
- **Publishing the crates** on crates.io and tagged releases with desktop binaries.

## Done

- Rust generator, validator (V1 to V5) and graph measures, byte-for-byte deterministic.
- Native desktop app with progressive drawing, English and Portuguese.
- This website, in English and Portuguese.

Found something missing? [Open an issue](https://github.com/Lucas-BRT/tapiti/issues).
