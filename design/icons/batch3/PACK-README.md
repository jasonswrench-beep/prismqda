# PrismQDA Icon Library — Batch 3 v1.0

The final 14 SVG files in the supplied icon inventory. This is a **delta pack**, not a replacement for Batches 1 and 2.

## Open first

Open `START_HERE.html` in a browser. It is self-contained: no server, online fonts or network access are required. Review actual SVG artwork at 16, 20 and 24 pixels, switch themes, filter by family, inspect source, save individual SVGs, and export Ready/Revise notes. Local review persistence depends on your browser's local-file storage permissions; export your notes to keep a portable copy.

## Inventory

| Family | Icon | File |
| --- | --- | --- |
| Source types | Social media post | `src-social.svg` |
| Source types | Geodata | `src-geo.svg` |
| Explore | Similarity | `exp-similarity.svg` |
| Text Analytics | Topics | `txt-topics.svg` |
| Text Analytics | Classifier | `txt-classifier.svg` |
| Text Analytics | Sentiment | `txt-sentiment.svg` |
| Text Analytics | Named entities | `txt-entities.svg` |
| Text Analytics | Keyness | `txt-keyness.svg` |
| Statistics | Scales and reliability | `sta-reliability.svg` |
| Statistics | Factor analysis | `sta-factor.svg` |
| Statistics | Cluster analysis | `sta-cluster.svg` |
| Media playback and regions | Foot pedal | `med-foot-pedal.svg` |
| Status indicators | Synchronized | `st-sync.svg` |
| Status indicators | Offline | `st-offline.svg` |

## Artwork

All 14 use a 24 × 24 viewBox, 1.75-unit round strokes, `fill="none"`, and `stroke="currentColor"`. All visible shapes are hand-authored paths. There are no embedded bitmap images, live text, transforms, scripts, external references, or font files. The app supplies the color. The established main logo is not redesigned.

## Contents

- `icons/outline/`: the 14 new SVG files with exact inventory filenames.
- `START_HERE.html`: self-contained interactive review gallery.
- `previews/`: light/dark Batch 3 sheets, gallery screenshots, and a **single 4000 × 7793 PNG showing all 244 canonical icons**.
- `registry/`: new-asset manifest and complete inventory in JSON/CSV, with SHA-256 hashes.
- `docs/`: design notes, integration instructions and validation records.
- `source/`: unchanged original Markdown inventory.
- `tools/validate_pack.py`: repeatable standard-library structural/hash checks.

## Completion and review

All 14 new SVGs passed structural, render and painted-margin checks. The complete library contains 244 canonical targets: 227 ordinary interface glyphs, 12 workspace active variants, and 5 brand assets. The previously delivered 230 assets are preserved byte-for-byte in the separate consolidated library. Two optional brand derivatives and the Windows ICO are additional and excluded from the 244 count.

Drawing the full inventory does not mean the icons have user design approval or that the finished application meets accessibility requirements. New assets remain `draft-for-review`.

## Integration

Copy `icons/outline/*.svg` alongside the earlier interface files, or use the separately supplied complete-library ZIP. No Batch 3 filenames collide with earlier canonical assets. See `docs/Integration_Guide.md` for themeable inline SVG usage.
