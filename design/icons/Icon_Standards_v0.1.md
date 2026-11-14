# PrismQDA Icon Standards
## Version 0.1 · proposed foundation baseline

### 1. Source and authority

**S1:** `source/PrismQDA Icon List.md`, supplied by Jason Wrench, dated 5 November 2026. The source copy is unchanged. The full registry preserves its names, group membership, descriptions and release batches. S1 lines 9–24 define the interface style; lines 34–61 define brand and workspace exceptions; lines 266–278 define the AI review controls. Line references refer to the copied Markdown file, not generated pages.

This document distinguishes source requirements from pilot implementation decisions. When the source is ambiguous, `Design_Decisions.md` records the decision and its approval state. A draft design is not proof that a requirement has passed user review.

### 2. The three asset contracts

| Property | Regular interface | Selected workspace | Brand and app |
| --- | --- | --- | --- |
| Coordinate system | 24 × 24 | 24 × 24 | 24 × 24 mark; horizontal wordmark; 256 × 256 app icon |
| Geometry | Outlined paths | Intentionally drawn filled paths | Filled facets and outlined-to-path letterforms |
| Paint | `stroke="currentColor"`, normally no fill | `fill="currentColor"`, no stroke | Fixed brand colors; wordmark letters use `currentColor` |
| Nominal stroke | 1.75 | Not applicable | Not applicable |
| Caps and joins | Round | Shape-defined | Shape-defined |
| Padding | Painted geometry within x/y 2–22 | Painted geometry within x/y 2–22 | Optically fitted; app tile intentionally reaches its canvas edges |
| Detail | Three or four semantic details where practical | Preserve the regular metaphor | Simplify enough to recognize the standalone mark at small sizes |

One source-row-specific exception is present: the code-matrix glyph uses two `currentColor` filled cells within an outlined grid. The brief explicitly describes filled cells, while its global rule says no fill. This prototype prioritizes that row; confirmation remains open.

### 3. SVG construction

Each delivered SVG has one root `svg` and only `path` children. There are no embedded images, `text`, `rect`, `circle`, groups, transforms, masks, filters, gradients, external resources, scripts, metadata or font dependencies. Curves, circles, rounded boxes and wordmark letters are expressed as paths. Standard frame corners use a radius of 2 units. Narrow capsules cap the radius to fit; the two filled matrix cells use a 0.7-unit radius and active chart bars use 1.6-unit corners as explicit optical refinements. These smaller inner/active radii are recorded pilot departures requiring approval, not a claim of literal radius-2 compliance throughout.

The files use the exact lowercase names requested in S1. Active workspace filenames add `-active` before `.svg`. SVGs intentionally omit accessible names internally: the surrounding interface must provide the correct label in its actual context, while decorative icon instances are hidden from assistive technology. This is an implementation pattern, not a completed accessibility audit.

### 4. Optical design

Use the 24-unit coordinate system at CSS sizes of 16, 20 and 24 px. The stroke scales with the icon; do not add `vector-effect="non-scaling-stroke"` or override stroke weight to simulate an active state. Use the real active SVG instead.

The gallery also provides enlarged inspection views. These are not a substitute for reviewing small rendering. The code hierarchy and joint-display glyphs need particular inspection at 16 px because their meaning combines more than one shape. Prefer visible labels for these tools. User testing remains required.

Match apparent mass rather than forcing every metaphor into the same silhouette. Circular glyphs may feel smaller than a page or grid even when their numeric bounds match. Counters, tag eyes and the space between modifier and base must remain visibly open. Focus and selection should be conveyed by the containing control, not only by changing the icon.

### 5. Prism identity

The current working mark is **Aperture**: a faceted triangular frame with an open central aperture. Its five flat colors retain the prism/spectrum idea without translucent layers or a glow effect. It is a fresh vector exploration informed by the chosen third UI concept; it is not a trace of that raster image.

Two alternatives are included: **Refraction**, a prism with outgoing spectrum rays, and **Solid Spectrum**, a simpler faceted silhouette. The directions should be reviewed at 16 px as well as at branding scale. The canonical mark, logo and app icon use Aperture solely to make this pack usable for evaluation. None is a cleared or approved trademark.

| Brand token | Value | Use |
| --- | --- | --- |
| Navy | `#151A3D` | App tile and dark brand field, specified in S1 |
| Violet | `#7B5CE1` | Facet |
| Blue | `#4C79D8` | Facet |
| Cyan | `#3BB7CB` | Facet |
| Rose | `#D777AC` | Facet |
| Amber | `#F1B756` | Facet |

The five facet colors are proposed design choices, not colors prescribed by the source. These fixed colors appear only in brand assets. Interface icons receive a single `currentColor` from the application. Code colors remain separate swatches, highlights or stripes.

### 6. Family logic

**Workspaces:** familiar house, database, tag, lens, overlapping circles, text, bars, layers, report, people, sparkle and gear. The layered Synthesize shape carries a restrained prism-angle reference; no miniature logo is inserted into every glyph.

**Sources:** Text and PDF share a page scaffold. Table follows the inventory row's cell-grid silhouette. Additional media-source families are not yet drawn.

**Coding:** Apply code and New code share a tag base, with a check and a plus respectively. Parent code adds branches below the tag. Do not use code colors to differentiate these functions.

**Analysis:** The Code matrix has occupied cells; Crosstab emphasizes row and column headers; Joint display joins bars and quotation strokes in separate panes. Treat them as different tools, not interchangeable grid icons.

**AI:** Local model combines monitor and sparkle. Accept and Reject use a check and X in a consistent circle. The icon indicates the selected route or action, not that an AI response is correct or that a network connection is secure. AI-origin markers are conceptually separate from review status; that later marker is not included in this pilot.

### 7. States and accessibility integration

The pack supplies regular icons and filled workspace counterparts. Hover, focus, selected-row backgrounds, disabled appearance, busy indicators and tooltips belong to the product's component layer. Do not create one differently colored SVG per state.

Always give an icon-only control an accessible action name. A workspace label should be accompanied by a programmatic current/selected state. Red and green may supplement the distinct Reject and Accept shapes but must not be their only difference. The high-contrast gallery is a visual stress view; it is not a claim of operating-system high-contrast or screen-reader conformance.

The gallery uses keyboard-focusable controls, native labels and a native inspection dialog. Actual application testing must cover screen readers, forced-color behavior, focus, zoom, localization and pointer targets. The preview controls are not a substitute for that testing.

### 8. Authoring and review lifecycle

`not-started → draft → design-approved → technically-validated → integrated` is the proposed asset lifecycle. Technical checks may be completed while design approval is still pending; store these as separate statuses. The current full inventory records 39 canonical assets as drafts and 205 as not started. Exploration files sit outside the canonical target count.

Use the gallery to record review choices, then export the review JSON. Do not treat a browser review choice as a cryptographic signature or automatically overwrite the repository registry. A maintainer should reconcile the comments and record the approving person and date.

### 9. Freeze rules

Before Batch 1 expansion, confirm the preferred mark, active-state treatment, page/table distinction and filled-cell exception. Once approved, reuse base geometry intentionally. New icons must preserve style and small-size behavior; they should not force every research concept into the same grid or tag. Existing filenames must remain stable even if their drawings are refined.
