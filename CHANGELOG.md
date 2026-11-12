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

- The grown layout has a `growth.start` option: its first room, the entrance, stands in the middle of the world (the default, as before) or against the north, south, east or west border (north is the top of the map). A border start makes a deeper floor: valid on the first draw on the medium size, 85% from the middle and 93 to 100% from a border.
- The documentation drops the game-design layer: `docs/model.md` speaks of floors, the entrance and the exit; the sections on keys, switches, pacing and the roles are gone (section 10 is now only the multi-floor extension), the V6 furniture rule and the parameter tables follow the code, and the README, the roadmap and the getting-started page use sizes instead of levels.
- The entrance and the exit are options of the Λ (`entrance`, `exitRoom`), in both layouts; both are on by default. "Start" and "stairs" are now "entrance" and "exit" everywhere: `Role::Entrance` and `Role::Exit`, `Floor.entrance` and `Floor.exit` (nullable), `exitDoors`, `exitMinHops`, `routesToExit`, `hopsToExit`, `geometricPathToExit` (the last two nullable), the V5 messages and the CSS classes. Without an entrance there is no exit and no main path, and the side-branch and independent-route requirements of the topology profiles are left out with a log line; the hub and long-loop profiles do not depend on them. With both on, the geometry of every stage is unchanged.
- The room roles are only entrance, stairs and plain: `Boss`, `Rest`, `Key`, `Switch`, `Ambush`, `Trap`, `Prize` and `Empty`, the `ambush` and `leafRoles` parameters and their colours are gone. A stage that has leaf chambers is drawn differently from the same seed, because the leaf role was drawn from the RNG.
- Keys, switches, gated corridors and exploration pacing are gone (`progression`, `pacing`, the P1 to P3 checks, `Room.keys`/`switches`, `Edge.requiresKeys`/`requiresSwitches`): they were game design, not floor generation. The geometry of every stage is unchanged.
- Sizes replace the five levels: small, medium, large and huge only set the grid, the large blocks and the leaf chambers (`params_of_size`); every other parameter keeps its default and can be changed. The desktop app and the lab pick a size (keys 1 to 4), the wasm module takes a size, and `rich_of_size` (every shape, junction and connection option on) serves tests, benchmarks and the landing page. The levels carried game-design settings the generator does not need.
- The lab follows the site's stone and carrot look: a segmented level control, the reading of the floor in a drawer below 1280 px and as a tab below 1000 px (the map first), and an optional Plan / 3D switch that turns the generated floor in three dimensions, rebuilt on every regenerate.
- The landing page is seven rooms along one real floor (level 5, seed 279), with a depth gauge and a minimap that link between the rooms. A three.js scene behind the page walks a torchlit camera along the floor as you scroll (the floor is built in the browser by the same WebAssembly module as the lab); without WebGL2 or scripts, or with `TAPITI_SCENE=off`, the floor stays as a dimmed map. "Dig again" is gone from the landing page: digging any seed lives in the lab.
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
