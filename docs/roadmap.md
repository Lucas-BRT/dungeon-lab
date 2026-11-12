# Roadmap

Tapiti is young. This page says plainly what is unfinished, so you know what to rely on.

## Growth layout

Besides the default layout (one room per grid block), the generator can *grow* rooms one from another, off a parent's door, in free space (`layout: growth`). It gives an organic, non-lattice floor, with an entrance that can stand in the middle or against a border (`growth.start`), leaf chambers, and the topology profiles. Known limits: it leaves room for the leaf chambers by growing fewer main rooms (about 77% of the grid's), and makes about 60% of the chambers asked for (the grid layout makes 85 to 95%); with several islands the bridges are single corridors, so independent routes to the exit are left out; `growth.cluster`, `growth.islands` and `growth.reach` above 3 are experimental.

## Other open items

- **Junction conductance** (`junction`) is a parameter of the reference model that the generator does not use yet.
- **Delaunay proximity** is in the model but not in the generator (RNG, Gabriel and Near are).
- **Global scoring of topology profiles** and **multi-floor campaigns** are described in the [model](model) as proposals.
- **The lab as the desktop app.** The browser lab is a Topcoat page with its own canvas, written in English only. Compiling the desktop interface (egui) to WebAssembly would unify them, and is postponed.
- **Publishing the crates** on crates.io and tagged releases with desktop binaries.

## Done

- Rust generator, validator (V1 to V5) and graph measures, byte-for-byte deterministic.
- Native desktop app with progressive drawing.
- The growth layout, with a test suite (determinism, validity, profiles, leaf chambers, the side of the entrance).
- This website.

Found something missing? [Open an issue](https://github.com/Lucas-BRT/tapiti/issues).
