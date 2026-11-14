# PrismQDA — Complete Batch 1 (v1.0)

This package continues the approved PrismQDA Icon Foundation Pack without redrawing the foundational 24 interface icons. It adds **69 newly drawn 24×24 SVGs** and integrates the user's selected luminous prism logo in place of the earlier candidate.

## Inventory
- **93 outline interface icons:** 12 workspace icons and 81 additional commands, sources, code-system, memo, exploration, AI, status and common-control icons.
- **12 separate active workspace icons** for selected tabs.
- **3 canonical brand SVGs:** brand-mark.svg, brand-logo.svg, app-icon.svg; plus brand-logo-light.svg and brand-mark-small.svg derivatives.
- **app-icon.ico** matching the chosen brand.
- **Total canonical Batch 1 SVGs: 108** (93 outline + 12 active + 3 core brand). The two optional brand alternatives are additional.

## Open first
`START_HERE.html` opens a fully offline gallery of actual file-backed SVG icons at 16, 20, 24 and 68 pixels. It is not a picture of icons. `previews` contains contact sheets and size tests.

## SVG integration
Use `icons/outline/*.svg` as toolbar and navigation glyphs; set CSS color on the containing button. Use `icons/active/ws-*-active.svg` for selected workspace tabs. Preserve the standalone prism as full-color brand artwork only. Avoid using the prism gradient inside regular command icons.

## Review gate
All canonical Batch 1 files exist, parse as SVG, use appropriate viewBoxes, and have prohibited active content excluded. **This is a technical validation, not proof of visual signoff or screen-reader behavior inside PrismQDA.** Review these files in context before approving the next batch.

## Production notes
- Based on the exact 2026-11-05 `PrismQDA Icon List.md` filenames.
- Minimal single-color outline geometry uses 1.75 stroke, rounded caps and joins, and `currentColor`.
- Filled workspace active SVGs come from the preserved foundation pack, with matching names.
- The approved user-selected prism logo is built using editable vector paths and gradients; this is the intentional exception to the outline rule.
- `cmd-search` and `ws-explore`, `ui-filter` and `cmd-scope` share an icon metaphor and intentional geometry, while retaining separate filenames.
- Do not claim finished-project accessibility until the UI labels, contrast and keyboard workflows have been tested in the application.
