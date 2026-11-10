# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Site: canonical link and `hreflang` alternates on every page.
- The Tapiti website: landing page, documentation (English and Portuguese, with math as MathML, a generated parameter reference and search), the browser lab, sitemap and robots.txt.
- `tapiti-render` (SVG floors) and `tapiti-schema` (the parameters of Λ as data, in English and Portuguese).
- "Dig again" on the landing page: dig the floor of any seed on the server (cached, with a bounded budget).
- English and Portuguese in the desktop app, with a language switch.
- MIT license, contribution guide, CI (format, lints, tests, WebAssembly build, cached multi-stage Docker build) and a release workflow for desktop binaries.

### Changed

- Accessibility pass on the website: axe-core reports no violations in light and dark, and no page scrolls sideways at 360 px.
- Renamed the project to Tapiti.
- The desktop app now uses egui instead of libcosmic, with a new layout: the map always visible, and tabs for the floor, the parameters and the analysis.
- Everything the project ships is written in English first.

### Removed

- The `apps/web-server` development server: the website serves the lab.
