# Roadmap

Tapiti is young. This page says plainly what is unfinished, so you know what to rely on.

## Growth layout (prototype)

Besides the default layout (one room per grid block), the generator can *grow* rooms one from another, off a parent's door, in free space. It gives an organic, non-lattice floor, but it is still marked as a prototype: on large grids it stops short of the room count you ask for, leaf chambers fit only in part, the topology profiles that need side branches or independent routes (backbone, redundant exit) often fail with it, and a large floor takes much longer to dig than the grid layout. It has few automated tests.

## Other open items

- **Junction conductance** (`junction`) is a parameter of the reference model that the generator does not use yet.
- **Delaunay proximity** is in the model but not in the generator (RNG, Gabriel and Near are).
- **Global scoring of topology profiles** and **multi-floor campaigns** are described in the [model](model) as proposals.
- **The lab as the desktop app.** The browser lab is a Topcoat page with its own canvas, written in English only. Compiling the desktop interface (egui) to WebAssembly would unify them, and is postponed.
- **Publishing the crates** on crates.io and tagged releases with desktop binaries.

## Done

- Rust generator, validator (V1 to V5) and graph measures, byte-for-byte deterministic.
- Native desktop app with progressive drawing.
- This website.

Found something missing? [Open an issue](https://github.com/Lucas-BRT/tapiti/issues).
