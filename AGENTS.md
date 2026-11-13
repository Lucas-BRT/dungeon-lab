# Agent instructions

Tapiti is a Rust workspace: the generator (`crates/tapiti-core`), its WebAssembly bindings (`crates/tapiti-wasm`), a native egui app (`apps/desktop`), SVG rendering of floors (`crates/tapiti-render`) and the public website (`apps/site`, a Topcoat app: landing page, docs, and the browser lab at `/{locale}/lab`: a Topcoat page whose form comes from `tapiti-schema`, with `apps/site/assets/lab/` driving the WebAssembly module).

## Rules

- **English only.** Everything the project ships (UI, docs, comments, commit messages, issue text) is written in English: the website, the lab, the desktop app and the docs. The site has no `/{locale}` prefix; its Fluent `en.ftl` files are checked by `topcoat l10n check -p tapiti-site` (unused or missing messages). `/en/*` and `/pt-BR/*` redirect permanently to the flat URLs.
- Everything is pinned, never `stable`, `latest` or a range: the Rust toolchain (`rust-toolchain.toml`, the same in CI and the Docker image), tools installed by hand (`cargo install --version`, `axe-core@x.y.z`), base image tags, Topcoat and wasm-bindgen (exact), and the lockfile. Pinned is not forgotten: Dependabot (crates, actions, image) and the weekly `Pins` workflow (`scripts/check-pins.sh` for the toolchain, Topcoat, wasm-bindgen, cargo-chef and axe-core and for pins that disagree between files, plus `cargo audit` on the lockfile) open an issue when something is behind or has an advisory. Bump on purpose, in a commit of its own with the reason, and move the versions kept in several places together.
- Unsafe code is not allowed in new crates (`unsafe_code = "forbid"`).
- Determinism is a feature: a seed and a Λ must draw the same stage on every platform. Changes to `tapiti-core` must keep
  `cargo run --release -p tapiti-core --example dump` hashes unchanged, unless the change is meant to alter the output (say so in the commit).
- Run `cargo fmt`, `cargo test -p tapiti-core -p tapiti-desktop` and `cargo clippy` for the crates you touched.
- Do not reintroduce references to the external game or prototype this model came from.

## Documentation

- `docs/*.md` is the single source: GitHub shows it and `apps/site` renders it (included at compile time, see `apps/site/src/docs.rs`).
- Write math as `$…$` and `$$…$$` (own lines, blank line around), never `\(…\)`. Do not put a `|` inside a formula in a table cell. The site turns formulas into MathML on the server; its tests fail when a formula does not parse, a link or anchor is broken, or a heading id repeats.
- Link other pages by slug (`[model](model)`, `[x](model#section)`), and the repository by relative path (`../crates/...`): the site rewrites both.
- The parameter reference is generated from `crates/tapiti-schema`; add a parameter there (its label and help text) and the desktop app and the docs follow. The lab form is generated from it too.

## Website (`apps/site`, Topcoat)

Read Topcoat's [`llms.txt`](https://raw.githubusercontent.com/tokio-rs/topcoat/refs/heads/main/llms.txt) before changing site code, and the guide it links for the feature you touch.

- Handlers and components take `cx: &Cx` first, even when unused.
- Internal links use `href!`, never string paths.
- After `cargo fmt`, run `topcoat fmt` (macro bodies such as `view!`); CI uses `topcoat fmt --check`.
- Pin Topcoat to an exact version: it is experimental and breaks between releases.
- Run `topcoat l10n check` after touching `.ftl` files.
