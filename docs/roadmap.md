# Roadmap

Tapiti is young. This page says plainly what is unfinished, so you know what to rely on.

## Growth layout (prototype)

Besides the default layout (one room per grid block), the generator can *grow* rooms one from another, off a parent's door, in free space. It gives an organic, non-lattice floor and scales well to large worlds (about 165 ms for 81 rooms on a 16×12 grid), but it is marked as a prototype: it often stops short of the room count you ask for, many floors are rejected for the stairs being too close to the entrance, most topology profiles do not fit it, and leaf chambers are not generated. It has no automated tests yet.

## Other open items

- **Junction conductance** (`junction`) is a parameter of the reference model that the generator does not use yet.
- **Delaunay proximity** is in the model but not in the generator (RNG, Gabriel and Near are).
- **Global scoring of topology profiles**, **mission objectives** and **multi-floor campaigns** are described in the [model](model) as proposals.
- **The lab as the desktop app.** The browser lab is a Topcoat page with its own canvas, written in English only. Compiling the desktop interface (egui) to WebAssembly would unify them, and is postponed.
- **Publishing the crates** on crates.io and tagged releases with desktop binaries.

## Done

- Rust generator, validator (V1 to V5) and graph measures, byte-for-byte deterministic.
- Native desktop app with progressive drawing, English and Portuguese.
- This website, in English and Portuguese.

Found something missing? [Open an issue](https://github.com/Lucas-BRT/tapiti/issues).
