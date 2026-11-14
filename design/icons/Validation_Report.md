# Validation report
## PrismQDA Icon Foundation Pack · v0.1 · 5 November 2026

**Asset-check result: PASS.** Technical validation is separate from design approval. All canonical assets remain drafts.

| Check actually performed | Result |
| --- | --- |
| SVG count | 42 files |
| Canonical inventory reconciliation | 244 targets; 39 drafted, 205 not started |
| Supplied Markdown checksum | Preserved |
| Delivered canonical asset hashes | 39 checked |
| SVG structure | Only svg/path; no raster content, transforms, scripts or external resources |
| Regular style | 24 square, 1.75 rounded currentColor strokes; one declared filled-cell exception |
| Active states | 12 separately drawn currentColor-filled paths |
| Interface raster test combinations | 324 renders: 36 glyphs × 3 sizes × 3 theme colors |
| Brand raster test combinations | 18 renders: 6 brand/candidate files × 3 sizes |
| Painted interface-margin checks | 36 checked at 8× sampling; all within x/y 2–22 |
| ICO structure and decoder | All 7 requested sizes present and decoded |
| Font binaries | None in the pack |
| External script/style resources in gallery | None |

## Browser interaction checks

Chromium was used at **1440 × 1080** and **390 × 844**. Checked filtering, active-state selection, light/dark/high-contrast switches, SVG inspection, individual SVG saving with exact source round-trip, review-note entry, review JSON export, example navigation and clean initial review state. No JavaScript errors or external network requests were observed. The narrow viewport had no document-wide horizontal overflow.

The test environment blocks both file and localhost navigation. The gallery's HTML was therefore injected into the browser using Playwright's `set_content` for interaction and rendering tests. **Direct file-origin navigation and persistent local-storage behavior were not verified here.** The gallery has a guarded, in-memory review fallback; JSON export was tested with that fallback. The package is self-contained and does not require network resources to render.

## Visual inspection

Reviewed the full glyph sheet, gallery overview, three brand directions and a dark-theme interface placement. Enlarged previews do not establish that every icon's meaning is immediately clear to researchers. In particular, Parent code, Crosstab and Joint display should receive actual-size recognition testing with their labels.

## Boundaries of this report

This is not a comprehensive accessibility conformance report, a multi-browser certification, an operating-system installation test, a trademark clearance or proof of scientific/software functionality. The small painted-bound checks are sampled raster bounds rather than symbolic curve proofs. Some corner radii and the filled-cell rule are declared design decisions; see the decision log.

## Reproduction and machine-readable evidence

Run `python source/validate_icons.py --root .` from the extracted directory. Raster checks require installed CairoSVG and Pillow; otherwise the checker records them as skipped. Read `registry/asset-validation.json` and `registry/gallery-validation.json` for the detailed results. This report does not change any asset's review state to approved.
