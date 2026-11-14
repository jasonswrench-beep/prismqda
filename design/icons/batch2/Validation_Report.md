# Batch 2 validation report

**Date:** 5 November 2026. **Result:** structural and rendering checks passed. **Design status:** draft-for-review.

## Scope

| Check | Result |
| --- | --- |
| Exact Batch 2 filename coverage | 122 / 122 |
| Canonical asset split | 120 themeable interface SVGs + 2 branded file SVGs |
| Valid XML / SVG namespace | 122 / 122 |
| 24 × 24 viewBox; 1.75 rounded root stroke | 122 / 122 |
| No scripts, external resources, images, live text, metadata, or transforms in SVGs | 122 / 122 |
| Regular glyph children are paths only | 120 / 120 |
| Brand-only local gradient definitions | 2 / 2 |
| Expected fill exception | set-appearance.svg only, outside the brand family |
| Nonempty rendering at 16, 20 and 24 px | 366 / 366 render checks |
| Painted live-area checks | 122 / 122 |
| Gallery cards | 122 |
| Search and family filtering | Passed; Statistics contains 13 assets |
| Theme switching | Light, dark and high-contrast previews passed |
| Inspector and Escape-to-close | Passed |
| Individual SVG export | Exported bytes match source file |
| Review JSON export/import | Passed |
| Unique inline IDs, including repeated brand previews | Passed |
| JavaScript exceptions during tests | 0 |
| External network requests during tests | 0 |
| Horizontal overflow at 780 px viewport | None detected |

## Painted bounds method

SVGs were rendered with CairoSVG at 480 × 480. The alpha-channel bounds above a threshold of 16/255 were mapped back to the 24-unit grid. The allowed painted bounds were 1.95–22.05, permitting a 0.05-unit antialias tolerance around the target 2–22 live area. The final per-file results are recorded in `Render_Checks.json`. This is a rendering check, not a complete mathematical proof about every SVG renderer.

Initial checks identified three margin issues; the link-marker, word-cloud, and bell paths were adjusted before the final passing run. Open-tag edge continuity was also corrected in Smart code and Weighted code. The four category preview sheets were visually inspected after rendering.

## Gallery test environment and limitation

HTML loaded into isolated Chromium via set_content; administrator policy blocks file and loopback navigation. Direct local-file launch not tested.

The gallery's complete HTML was tested in Chromium with embedded assets and no external requests. Session review, source inspection, theme switching, search, downloads and review-file handling passed. Local-file browser-storage persistence varies by browser and was not established by these tests. Export JSON is the portable backup route.

## Not established

This report does not establish user approval, perfect metaphor recognition, screen-reader conformance in the production app, operating-system high-contrast compatibility, trademark clearance, or scientific validity of any analysis. Actual UI labels, state announcements, contrast, focus, hit targets and assistive-technology workflows need application-level tests.

## Repeatability

`python tools/validate_pack.py` repeats the package/structure/hash checks using the standard library. Raster results and browser interaction results are stored as JSON. The PNG sheets are previews; SVGs are the authoritative artwork.
