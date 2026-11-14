# PrismQDA Icon Library — Batch 2

**Pack version 1.0 · 5 November 2026 · Draft artwork for review**

This is the next installment of the icon inventory, not a redesign of Batch 1.
It contains **122 canonical, editable SVG assets: 120 themeable interface glyphs and 2 branded file glyphs**. Every filename matches the Batch 2 rows of the supplied `PrismQDA Icon List.md`. No Batch 3 work is silently substituted into this release.

## Start here

Extract the ZIP, then open **START_HERE.html** in a browser. The self-contained gallery has no remote dependencies and displays the actual vectors at 16, 20 and 24 CSS pixels, plus enlarged views. It includes search, family filters, light/dark/high-contrast previews, source-code inspection, individual SVG export, and review notes with JSON import/export.

Local browser storage is best-effort. Export review JSON to preserve a portable copy. Container browser policy prevented a direct `file://` opening test; all gallery interaction tests passed when its complete HTML was loaded into an isolated Chromium context. See the validation report for the exact scope.

## Contents

| Family | New SVGs |
| --- | ---: |
| Brand and app | 2 |
| Source types | 10 |
| Code system | 9 |
| Memos and links | 11 |
| Explore | 7 |
| Mixed Methods | 6 |
| Text Analytics | 6 |
| Statistics | 13 |
| Synthesize | 4 |
| Report | 9 |
| Team | 9 |
| AI | 7 |
| Media playback and regions | 10 |
| Settings sections | 11 |
| Status indicators | 5 |
| Common controls | 3 |
| **Total** | **122** |

## Files and folders

- `icons/outline/`: 120 single-color interface glyphs. Appearance includes the half-filled disk expressly described in its source row.
- `brand/`: `file-project.svg` and `file-package.svg`. Both reuse your approved compact prism facets and curved inner seam as a spectrum badge.
- `previews/`: four family sheets, a full inventory sheet, a dark small-size sheet, and gallery/context screenshots.
- `registry/`: Batch 2 manifests and the complete 244-target library register in JSON and CSV.
- `docs/`: style, decisions, integration, validation, and provenance notes.
- `source/`: unchanged supplied icon list.
- `tools/validate_pack.py`: repeatable structural and manifest validation using Python's standard library.

## Continuity

Batch 1 remains unchanged. Its approved main logo, workspace variants and earlier glyphs are not replaced. The gallery embeds selected Batch 1 drawings only as comparison references; they are not additional canonical files in this pack.

Across the two delivered packs, **230 of 244 canonical SVG targets are now drafted**, including the 12 active workspace variants. **14 Batch 3 icons remain**. Optional brand derivatives, ICO sizes and preview images are not included in those counts.

## Implementation

Preserve the 24 × 24 viewBox and 1.75-unit stroke. Inline the SVG through your component layer when it needs to inherit `currentColor`; external image elements do not inherit their parent's text color in the same way. Provide an accessible label on the containing control. Repeated inline brand instances need unique gradient ID prefixes. See `docs/Integration_Guide.md`.

## Review status

All 122 files passed exact inventory, SVG structure, active-content exclusion, hash, rendering and painted-margin checks. The gallery passed the recorded interaction checks. This does **not** establish final visual approval, trademark clearance, or screen-reader/keyboard/contrast conformance inside the production application.
