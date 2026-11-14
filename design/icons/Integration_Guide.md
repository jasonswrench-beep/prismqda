# Integration guide
## Use the SVGs as application assets

### Basic labeled control

Inline the supplied SVG body so `currentColor` inherits from the component. A button's visible label can provide its accessible name. An icon-only button needs an explicit action name. The following is an example, not a framework dependency:

```html
<button type="button" class="tool-button" aria-label="Accept proposal">
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
       width="20" height="20" fill="none" stroke="currentColor"
       stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
       aria-hidden="true" focusable="false">
    <path d="M3.3 12a8.7 8.7 0 1 0 17.4 0a8.7 8.7 0 1 0 -17.4 0Z"/>
    <path d="M7.6 12L10.6 15.1L16.5 9.2"/>
  </svg>
</button>
```

```css
.tool-button {
  color: var(--icon-color);
  min-width: 36px;
  min-height: 36px;
}
.tool-button svg { display: block; flex: none; }
.tool-button:focus-visible { outline: 2px solid var(--focus-color); outline-offset: 3px; }
```

Those example target dimensions are design defaults, not a claim of standards conformance. Validate the implemented component in its actual context.

### Selected workspaces

Render `ws-code.svg` in the unselected item and `ws-code-active.svg` in the selected item. Preserve the label and a selected/current programmatic state appropriate to the navigation component. Do not just set `fill` on the outline file: its open paths were not designed for that operation.

### Asset loading caveat

An external SVG used through an HTML `img` element does not inherit the containing element's `color` as an inline SVG does. Use your application's trusted inline SVG loader or component wrapper for themeable glyphs. The source files intentionally have no scripts or external dependencies. Do not use an unsanitized loader for arbitrary user-provided SVG content.

The brand mark and app icon have fixed colors. `brand-logo.svg` has fixed facet colors with `currentColor` letters, so an inline instance can adapt the wordmark to light or dark backgrounds. Its horizontal viewBox is intentional.

### Sizing

Use 16, 20 or 24 px for the interface instances. Larger sizes are supported by the vector files. Preserve aspect ratio. Do not snap all coordinates to integers or rewrite the stroke width during optimization without inspecting the small-size previews again. Do not replace path-based circles or letter outlines with raster images.

Use labels for domain-specific tools such as the joint display, code matrix and crosstab. Color belongs to a containing selection state or separate code swatch, not to a new colored copy of the glyph.

### Reuse

The tag base is shared intentionally between the two coding commands. Source pages share their scaffold. Do not alias a future generic icon to a workspace icon if doing so would import a workspace-only facet or change its meaning. Keep the filename requested by the inventory even when geometry is shared.

### App icon

`brand/app-icon.ico` contains the requested 16, 24, 32, 48, 64, 128 and 256 px entries. Matching PNGs are included as previews. The 256-square SVG is the source. Installation, taskbar behavior, per-platform packaging and operating-system high-contrast behavior still require testing in the application.

### Suggested repository layout

```text
assets/icons/regular/
assets/icons/active/
assets/brand/
design/icon-standards/
design/icon-registry/
```

Keep `foundation-manifest.json` and the review decisions with the assets. The full registry retains every canonical target, including those not started. Its `status` and `review_status` are separate; a technical pass must not mark an asset design-approved.

### Rerun checks

From the extracted pack directory:

```sh
python source/validate_icons.py --root .
```

The core XML/naming/manifest/ICO checks use the Python standard library. Optional rendering checks use installed CairoSVG and Pillow; the checker reports explicitly when these are unavailable. The delivered validation report records the checks actually run for this pack. No rendered preview replaces the editable SVG source.
