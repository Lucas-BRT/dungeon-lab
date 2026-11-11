# Contributing

Thanks for helping. Small fixes can go straight to a pull request; for something bigger, open an issue first so we can agree on the shape of it. The [roadmap](docs/roadmap.md) lists what is unfinished.

## Set up

```sh
git clone https://github.com/Lucas-BRT/tapiti
cd tapiti
cargo test -p tapiti-core -p tapiti-render -p tapiti-schema -p tapiti-site -p tapiti-desktop
```

See [Getting started](docs/getting-started.md) to run the desktop app and the website (the latter needs `cargo install topcoat-cli`).

## Rules of the project

- **Determinism is a feature.** A seed and the parameters must draw the same floor on every platform. Changes to `tapiti-core` must keep the hashes of `cargo run --release -p tapiti-core --example dump` unchanged, unless the change is meant to alter the output; say so in the pull request.
- **English first, Portuguese supported.** Everything is written in English (UI, docs, comments, commits); Portuguese (pt-BR) is a translation. Desktop strings use `t!("English text")` with the table in `apps/desktop/src/i18n/pt.rs`; website strings live in `apps/site/src/l10n/*.ftl`; parameter texts live in `crates/tapiti-schema`. Tests and `topcoat l10n check` fail when a translation is missing.
- **Docs are the single source.** `docs/` is shown on GitHub and rendered by the website. Math is written as `$…$` and `$$…$$`; links between pages use the slug (`[model](model)`). The site's tests catch broken formulas, links and anchors.
- **No unsafe code** in the crates of this repository (`unsafe_code = "forbid"`).
- **The website** is a [Topcoat](https://github.com/tokio-rs/topcoat) app. Read its `llms.txt` before changing it, run `topcoat fmt` after `cargo fmt`, and keep the Topcoat version pinned.

## Before you open the pull request

```sh
cargo fmt -p tapiti-site -p tapiti-render -p tapiti-schema
cargo clippy -p tapiti-site -p tapiti-render -p tapiti-schema -p tapiti-desktop --no-deps -- -D warnings
cargo test --release -p tapiti-core -p tapiti-render -p tapiti-schema -p tapiti-site -p tapiti-desktop
(cd apps/site && topcoat fmt --check $(find src -name '*.rs')) && topcoat l10n check --package tapiti-site
```

The same checks run in CI. By contributing you agree that your work is released under the [MIT license](LICENSE).
