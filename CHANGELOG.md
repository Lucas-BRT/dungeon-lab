# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Site redesign: a drawing sheet (paper by day, blueprint by night), docs as a calm document with copy buttons and a live table of contents; canonical link and `hreflang` alternates on every page.
- The Tapiti website: landing page, documentation (with math as MathML, a generated parameter reference and search), the browser lab, sitemap and robots.txt.
- `tapiti-render` (SVG floors) and `tapiti-schema` (the parameters of Λ as data).
- "Dig again" on the landing page: dig the floor of any seed on the server (cached, with a bounded budget).
- MIT license, contribution guide, CI (format, lints, tests, WebAssembly build, cached multi-stage Docker build) and a release workflow for desktop binaries.

### Changed

- The landing page is seven rooms along one real floor (level 5, seed 279), with a depth gauge and a minimap that link between the rooms; the floor is the dimmed backdrop. "Dig again" is gone from the landing page: digging any seed lives in the lab.
- The browser lab is a Topcoat page (`/lab`) whose form is built from `tapiti-schema`; the generator still runs in the browser with WebAssembly.
- `Dungeon` is now `Floor`; `generate.rs` is split into one file per phase. The generator output is unchanged.
- Accessibility pass on the website: axe-core reports no violations in light and dark, and no page scrolls sideways at 360 px.
- Renamed the project to Tapiti.
- The desktop app now uses egui instead of libcosmic, with a new layout: the map always visible, and tabs for the floor, the parameters and the analysis.
- Everything the project ships is written in English first.

### Removed

- Portuguese everywhere: the website (`/pt-BR`, the language prefix; old `/en/*` and `/pt-BR/*` URLs redirect), `docs/pt-BR`, the desktop language switch and the Portuguese texts of `tapiti-schema`. The project is English only.
- The plain `web/` lab page, the `frontier-growth` experiment and the `gpu_proximity` example (with the `wgpu` dev-dependency).
- The `apps/web-server` development server: the website serves the lab.
