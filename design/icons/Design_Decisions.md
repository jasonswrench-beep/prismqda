# Design decisions and open approvals

These decisions make the first draft buildable. They do not silently amend the supplied brief. **S1** is the unchanged `PrismQDA Icon List.md` in `source/`.

| ID | Issue | Source lines | Pilot treatment | Status |
| --- | --- | --- | --- | --- |
| D-01 | Outline versus active/brand fill | S1 14–18, 34–42 | Separate regular, filled workspace and full-color brand contracts. | Source-consistent interpretation |
| D-02 | Visible padding | S1 13–14 | Measure the painted stroke, not only its path centerline, inside x/y 2–22. | Proposed clarification |
| D-03 | All sources as sheets versus Table as grid | S1 89, 94–99 | Text/PDF share a page; Table uses the row-specified grid silhouette. | Approval requested |
| D-04 | Code matrix filled cells versus global no fill | S1 14–15, 159 | Permit two currentColor filled cells only in exp-code-matrix.svg. | Approval requested |
| D-05 | Horizontal wordmark and app canvas | S1 13, 40–42 | Keep 24 square for glyphs; allow a horizontal wordmark and 256-square app icon. | Proposed clarification |
| D-06 | Rectangle radius on narrow elements | S1 16 | Use radius 2 on standard frames; cap capsules to fit; use 0.7 for filled matrix cells and 1.6 for active bars as declared optical departures. | Proposed clarification |
| D-07 | Detail count | S1 17 | Count semantic details, not the number of path commands forming a curve. | Proposed clarification |
| D-08 | Apply code versus New code | S1 72, 75 | Use a check for Apply and a plus for New on the same tag base. | Within source options |
| D-09 | Prism motif placement | S1 19 | Use it selectively in brand/layer geometry; leave familiar controls undecorated. | Within source options |
| D-10 | AI origin versus review state | S1 121, 268–278 | Keep origin separate from Pending/Accepted/Rejected. The origin marker is not yet drawn. | Proposed semantic clarification |
| D-11 | Working brand recommendation | S1 36, 40–42 | Aperture is used for canonical review files; alternatives remain visible. | Brand approval pending |
| D-12 | Wordmark typography | S1 41 | Use outlined Inter Display Semibold/Medium letters, with provenance; no font files. | Typography approval pending |
| D-13 | File delivery | S1 23–30 | Provide named SVG files, machine-readable registry and HTML review instead of chat code blocks. | User-approved foundation-pack workflow |
| D-14 | Lifecycle and licensing | S1 22; source master-spec context | Mark assets draft until approval; owner confirms artwork release license before distribution. | Release decision pending |

## Recommended decision order

Approve a brand direction first, then the regular/active workspace pair treatment. Review the source family and D-04 together at 16 px. After those decisions, remaining Batch 1 work can reuse the approved page, tag, grid, sparkle and modifier scaffolds.

## Not included or implied

No new product features are specified by these drawings. The remaining icon inventory is not drawn, the generated mockup statistics are not reused as evidence, and no third-party icon shapes have been traced. The mark has not undergone trademark clearance. The geometry may resemble common interface conventions because a house, tag or magnifying glass must remain recognizable.
