# Brand assets

- `logo-sketch.png` (1000×1000): the hand-drawn rabbit head, the source of the logo.
- `logo.svg`: the logo, the sketch traced to centre lines (skeleton of the strokes, gaps closed, Gaussian σ = 6 points to remove hand tremor, cubic Béziers), drawn as 36-unit round strokes in `#ff2e63` and cropped to the drawing. It has no background, so it works on dark and light pages. Used by the site header and footer, and by the README.
- `icon.svg`: the same head on the soil colour (`#15110e`), simplified to the outline and the pupil (the whiskers and the eye ring blur at small sizes), with a thicker line. It is the SVG favicon and the source of the raster icons.
- `favicon.ico` (16, 32 and 48 px), `apple-touch-icon.png` (180 px) and `../../../desktop/assets/icon.png` (256 px, the desktop window icon), `icon.ico` (16 to 256 px, embedded in `tapiti-desktop.exe` by `apps/desktop/build.rs`) and `icon.icns` (128 to 1024 px, in `Tapiti.app`): renders of `icon.svg`.
- `og.png`: the 1200×630 social card: the bird's-eye frame of the landing page's walk (seed 279, level 5), captured from the 3D scene itself in headless Firefox, with the wordmark in Grenze and the claim in Alegreya Sans. Made by `node scripts/og-card.mjs` against a running site.

Colours and the carrot accent come from `../styles.css` (`--carrot`).
- `tapiti-real.svg`, `tapiti-real.png` (277×257): the hand-drawn tapiti sketch, traced to one centre line, smoothed (Gaussian, σ = 7 points) to remove hand tremor, cropped to the drawing with 24 px of margin, closed and filled with the carrot colour (`#f08a3c`) under a soil-coloured outline (`#1a1410`). The raw sketch is in git history (commit `a17946d`).
