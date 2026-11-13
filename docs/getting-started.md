# Getting started

Tapiti is a Rust workspace. You need a recent stable Rust toolchain ([rustup](https://rustup.rs)); nothing else to try the generator. The desktop app also needs the usual graphics libraries of your platform (on Linux: xkbcommon, Wayland and/or X11, and OpenGL).

```sh
git clone https://github.com/Lucas-BRT/tapiti
cd tapiti
```

## The desktop app

```sh
cargo run --release
```

Pick a layout (grid, or growth, where rooms grow one from another) and a size (small, medium, large or huge) in the top bar, next to the seed and the Generate button; the grid, the leaf chambers and, for growth, where it starts are in their group of the Parameters tab. Change the seed and edit any parameter: the floor is dug again a moment after each change. A very large floor can be stopped with the Cancel button (or <kbd>Esc</kbd>), and the floor on the screen stays. The side panel has two tabs: the parameters (one collapsible group each: Grid and size and Room shapes open, then Corridors and junctions…, plus Growth with the growth layout, with the number of changed ones in the title), and the analysis (charts and plain explanations of what the floor is). A search box opens the groups that have a match. The menu bar has File (copy the JSON, copy the floor as SVG, quit), Edit (reset all to the size), View (the theme), Tools (the guided experiments and the JSON editor, in a window) and Help (the shortcuts); the status bar shows how many parameters changed, with a Reset.

Shortcuts: <kbd>Alt</kbd>+<kbd>1</kbd> to <kbd>4</kbd> switch the size, <kbd>Alt</kbd>+<kbd>R</kbd> draws a random seed, <kbd>Alt</kbd>+<kbd>←</kbd> and <kbd>→</kbd> step the seed. Drag the map to pan, scroll to zoom, double-click to fit.

## The website and the lab

The website you are reading is a [Topcoat](https://github.com/tokio-rs/topcoat) app. To run it with live reload:

```sh
cargo install topcoat-cli
scripts/build-wasm.sh                      # the WebAssembly module of the lab
topcoat dev -p tapiti-site      # http://127.0.0.1:3000
```

The lab (`/lab`) runs the generator in your browser, compiled to WebAssembly. Its form is built from the same parameter schema as the desktop app. The lab is in English only.

## Docker

```sh
docker compose -f compose.local.yml up --build
```

serves the same site on <http://localhost:8090>. `compose.yml` is the version meant for a Dokploy deployment; set `TAPITI_SITE_URL` to the public address so canonical links and social previews point to it.

## Using the crate

`tapiti-core` is the generator, with no UI. Add it as a git dependency:

```toml
[dependencies]
tapiti-core = { git = "https://github.com/Lucas-BRT/tapiti" }
```

```rust
use tapiti_core::{
    params::{params_of_size, Size},
    stage::stage,
};

fn main() {
    let params = params_of_size(Size::Medium);
    // Up to 8 draws: a floor the validator rejects is dug again from a derived seed.
    let staged = stage(42, &params, 8);
    assert!(staged.violations.is_empty());
    println!("{} rooms, {} corridors", staged.d.rooms.len(), staged.d.edges.len());
}
```

The same seed and parameters always give the same floor, native or WebAssembly.

## Command-line tools

```sh
cargo run --release -p tapiti-core --example dump               # a hash per stage, to compare two builds
cargo run -p tapiti-render --example export_svg -- medium 26 floor.svg dark   # size, seed, output, theme
cargo bench -p tapiti-core --bench generator                    # Criterion benchmarks
```

## Where to go next

- [The mathematical model](model): what a floor is, how it is validated and measured.
- [Parameters](parameters): every parameter of Λ, with its range.
- [Benchmarks](performance): how to measure the generator.
- [Roadmap](roadmap): what is unfinished.
