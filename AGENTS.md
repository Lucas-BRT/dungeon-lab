# Agent instructions

Tapiti is a Rust workspace: the generator (`crates/tapiti-core`), its WebAssembly bindings (`crates/tapiti-wasm`), a native egui app (`apps/desktop`), SVG rendering of floors (`crates/tapiti-render`) and the public website (`apps/site`, a Topcoat app: landing page, docs, and the browser lab at `/lab`, which is still the plain `web/` page).

## Rules

- **English first, Portuguese supported.** Everything the project ships (UI, docs, comments, commit messages, issue text) is written in English. Portuguese (pt-BR) is a
  translation, never the source. Desktop: `t!("English text")` / `tf!(...)` in `apps/desktop/src/i18n.rs`, with the pt table in `i18n/pt.rs`; a unit test fails when a key has no
  entry. Site: Fluent `.ftl` files (`en` default, `pt-BR`), checked by `topcoat l10n check`.
- Unsafe code is not allowed in new crates (`unsafe_code = "forbid"`).
- Determinism is a feature: a seed and a Λ must draw the same stage on every platform. Changes to `tapiti-core` must keep
  `cargo run --release -p tapiti-core --example dump` hashes unchanged, unless the change is meant to alter the output (say so in the commit).
- Run `cargo fmt`, `cargo test -p tapiti-core -p tapiti-desktop` and `cargo clippy` for the crates you touched.
- Do not reintroduce references to the external game or prototype this model came from.

## Documentation

- `docs/*.md` (English) and `docs/pt-BR/*.md` are the single source: GitHub shows them and `apps/site` renders them (included at compile time, see `apps/site/src/docs.rs`). A page missing from `docs/pt-BR/` falls back to English with a notice.
- Write math as `$…$` and `$$…$$` (own lines, blank line around), never `\(…\)`. Do not put a `|` inside a formula in a table cell. The site turns formulas into MathML on the server; its tests fail when a formula does not parse, a link or anchor is broken, or a heading id repeats.
- Link other pages by slug (`[model](model)`, `[x](model#section)`), and the repository by relative path (`../crates/...`): the site rewrites both.
- The parameter reference is generated from `crates/tapiti-schema`; add a parameter there (English and Portuguese text) and the desktop app and the docs follow. `web/main.js` still carries its own copy: the lab is frozen.

## Website (`apps/site`, Topcoat)

Read Topcoat's [`llms.txt`](https://raw.githubusercontent.com/tokio-rs/topcoat/refs/heads/main/llms.txt) before changing site code, and the guide it links for the feature you touch.

- Handlers and components take `cx: &Cx` first, even when unused.
- Internal links use `href!`, never string paths.
- After `cargo fmt`, run `topcoat fmt` (macro bodies such as `view!`); CI uses `topcoat fmt --check`.
- Pin Topcoat to an exact version: it is experimental and breaks between releases.
- Run `topcoat l10n check` after touching `.ftl` files.
