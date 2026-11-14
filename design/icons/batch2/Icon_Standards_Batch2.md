# Batch 2 icon standards

## Source authority

The unchanged inventory in `source/PrismQDA Icon List.md` supplies meanings, filenames, families, batches and baseline style. The existing Batch 1 SVG files supply the established page, memo, tag and information silhouettes. These are design inputs; this document does not replace the product specification.

## Construction

All new icons have a 24 × 24 viewBox. Root stroke is 1.75, with round line caps and joins and `stroke="currentColor"`. Painted bounds are measured inside the 2–22 live region, with an explicit raster antialias tolerance in the validation report. Regular drawing elements are paths only. Circles, rounded rectangles, letters and numbers are expressed as paths, not SVG text or font resources.

The two brand-file glyphs are a declared brand exception: their drawing paths use local linear-gradient definitions and full-color fills. They contain no embedded bitmap, live text, script, filter or external reference. The approved compact prism geometry is flattened into the 24-unit canvas, without transform attributes.

## Families

Document-like sources retain the folded sheet. Email, browser, reference book, participant and focus-group sources follow the distinct silhouettes requested by their inventory rows. Code-family tags match the existing angle and hole vocabulary. Memo types share the established lower-corner-fold note, with one clear inner mark. Analytical tools use different structural metaphors rather than interchangeable grids. Media controls keep plain outlines. AI actions may combine the base tool with a small sparkle, but neither sparkle nor color communicates review approval.

## Exceptions and optical refinements

Appearance uses a currentColor half-filled circle because its row explicitly asks for one. Standard frames use a radius of 2 where the size allows it; compact inner frames cap the radius to fit. Region-selection handles are deliberately square. Typographic marks explicitly allowed or implied by inventory rows are drawn as paths: question mark, information i, report-outline numbers and the Quantitize number column. No font files are included.

The three/four-detail brief is treated as a semantic-density target, not a count of SVG path commands. Compound tools such as focus group, keyboard and selection regions necessarily require multiple marks. Their usability must be reviewed at actual size with labels.

## Color and state

Regular glyphs receive one application-controlled color; code colors remain separate swatches and highlights. Selection, hover, keyboard focus and disabled styling belong to the UI component, not a new colored SVG file. No active workspace variants are required in Batch 2. The prior 12 active variants remain in Batch 1.

## Approval

Technical validation and design approval are recorded separately. This pack marks new assets draft-for-review. No icon glyph by itself establishes security, correct statistics, trustworthy AI, or accessibility conformance.
