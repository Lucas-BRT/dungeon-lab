# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Site redesign: a torchlit stone floor (dark only: wet-stone base, bone text, one carrot accent; Grenze, Alegreya Sans and JetBrains Mono), docs as a calm document with copy buttons and a live table of contents, and a canonical link on every page.
- A social card made from the 3D scene, and browser checks in CI (the landing walk, the lab, the 404, axe-core) with screenshots kept as an artifact.
- The Tapiti website: landing page, documentation (with math as MathML, a generated parameter reference and search), the browser lab, sitemap and robots.txt.
- `tapiti-render` (SVG floors) and `tapiti-schema` (the parameters of Λ as data).
- "Dig again" on the landing page: dig the floor of any seed on the server (cached, with a bounded budget).
- MIT license, contribution guide, CI (format, lints, tests, WebAssembly build, cached multi-stage Docker build) and a release workflow for desktop binaries.

### Changed

- The lab follows the site's stone and carrot look: a segmented level control, the reading of the floor in a drawer below 1280 px and as a tab below 1000 px (the map first), and an optional Plan / 3D switch that turns the generated floor in three dimensions, rebuilt on every regenerate.
- The landing page is seven rooms along one real floor (level 5, seed 279), each with its own composition and no boxes around the text: the entrance title large at the bottom left, the five shapes along the floor, the checks as a map table (a floor kept beside a floor thrown away), the steps of the dig across the bottom, the numbers carved large, the stairs centred. A labelled route at the right edge (minimap with a "you are here" dot, every room by name, the pause control) replaces the depth gauge and the floating minimap; on the landing page the header lies over the scene and scrolls away. A three.js scene behind the page walks a torchlit camera along the floor as you scroll (the floor is built in the browser by the same WebAssembly module as the lab); without WebGL2 or scripts, or with `TAPITI_SCENE=off`, the floor stays as a dimmed map. "Dig again" is gone from the landing page: digging any seed lives in the lab.
- The scene's light: warm torches (about 2700 K) on warm-grey stone against a cool fill, softer falloff, thin fog in the overview so the whole floor reads as a lit model, torches spread over the floor in the overview, and the picture moved away from the side that holds the text. Below 1100 px the scene stays behind the entrance only, and stops drawing once scrolled away.
- Docs: page titles in Grenze, section headings in Alegreya Sans, a search field with an icon, an outline without boxed rows, the page list as one scrolling row on phones; headings no longer slide under the sticky header (one `--header-h` token drives the header, the scroll padding, the docs' sticky columns and the lab's height). The header marks the current section with `aria-current`.
- The browser lab is a Topcoat page (`/lab`) whose form is built from `tapiti-schema`; the generator still runs in the browser with WebAssembly.
- The room layout is a `layout` parameter (`grid` or `growth`), shown as a drop-down in the desktop app and the lab; it replaces the `growth.enabled` checkbox. A Λ without `layout` is a grid, and one that set `growth.enabled` must set `layout` instead. The generator output is unchanged.
- `Dungeon` is now `Floor`; `generate.rs` is split into one file per phase. The generator output is unchanged.
- Accessibility pass on the website: axe-core reports no violations in light and dark, and no page scrolls sideways at 360 px.
- Renamed the project to Tapiti.
- The desktop app now uses egui instead of libcosmic, with a new layout: the map always visible, and tabs for the floor, the parameters and the analysis.
- Everything the project ships is written in English first.

### Removed

- Portuguese everywhere: the website (`/pt-BR`, the language prefix; old `/en/*` and `/pt-BR/*` URLs redirect), `docs/pt-BR`, the desktop language switch and the Portuguese texts of `tapiti-schema`. The project is English only.
- The plain `web/` lab page, the `frontier-growth` experiment and the `gpu_proximity` example (with the `wgpu` dev-dependency).
- The `apps/web-server` development server: the website serves the lab.
