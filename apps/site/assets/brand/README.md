# Brand assets

- `mark.svg`: the Tapiti mark (a rabbit-shaped burrow with a carrot-coloured door). It adapts to the visitor's colour scheme and doubles as the SVG favicon.
- `favicon.ico`, `apple-touch-icon.png`: made from a 1024 px render of the mark on the dark soil colour (`#15110e`) with the `web-asset-generator` skill's `generate_favicons.py`.
- `og.png`: the 1200×630 social card. A real floor (`cargo run -p tapiti-render --example export_svg -- 5 26 floor.svg dark`) beside the wordmark in Fraunces, composed in HTML and captured with headless Firefox.

Colours and the carrot accent come from `../styles.css`.
- `tapiti-real.svg`, `tapiti-real.png` (277×257): the hand-drawn tapiti sketch, traced to one centre line, smoothed (Gaussian, σ = 7 points) to remove hand tremor, cropped to the drawing with 24 px of margin, closed and filled with the carrot colour (`#f08a3c`) under a soil-coloured outline (`#1a1410`). The raw sketch is in git history (commit `a17946d`).
