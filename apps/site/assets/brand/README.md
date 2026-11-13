# Brand assets

- `mark.svg`: the Tapiti mark (a rabbit-shaped burrow with a carrot-coloured door). It adapts to the visitor's colour scheme and doubles as the SVG favicon.
- `favicon.ico`, `apple-touch-icon.png`: made from a 1024 px render of the mark on the dark soil colour (`#15110e`) with the `web-asset-generator` skill's `generate_favicons.py`.
- `og.png`: the 1200×630 social card: the bird's-eye frame of the landing page's walk (seed 279, level 5), captured from the 3D scene itself in headless Firefox, with the wordmark in Grenze and the claim in Alegreya Sans. Made by `node scripts/og-card.mjs` against a running site.

Colours and the carrot accent come from `../styles.css` (`--carrot`).
- `tapiti-real.svg`, `tapiti-real.png` (277×257): the hand-drawn tapiti sketch, traced to one centre line, smoothed (Gaussian, σ = 7 points) to remove hand tremor, cropped to the drawing with 24 px of margin, closed and filled with the carrot colour (`#f08a3c`) under a soil-coloured outline (`#1a1410`). The raw sketch is in git history (commit `a17946d`).
