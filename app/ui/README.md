# PrismQDA interface

The screens of PrismQDA: a React, TypeScript, and Vite web interface that talks to the Rust engine
through one function (`call(cmd, args)`, see `src/bridge.ts`). A separate step will wrap it in a
Tauri window. Everything here follows `docs/SPECIFICATION.md` sections 7, 14.1, and 18 and the
commands in `docs/API.md`.

This is slice 1: a usable hand-coding alpha. Search, undo, text editing, templates, reliability, and
the other parts of the specification are not here yet (the engine does not have them yet either).

## Run it

You need Node 20.19 or later and, for the browser check, an existing Playwright install (the
check does not install one).

```
npm install --ignore-scripts --no-audit --no-fund
npm run typecheck        # TypeScript
npm test                 # unit tests (node --test, TypeScript through tsx)
npm run licenses         # license rules for the JavaScript packages
npm run build            # production build into dist/
npm run browser-check    # headless browser check against the real engine (needs PLAYWRIGHT_DIR)
npm run check            # all of the above in order
```

`npm run dev` serves the interface with Vite and forwards `/rpc` to a development bridge
(`target/debug/prismqda serve --port 4891`, built with
`cargo build -p prismqda-cli --features dev-server`). Set `PRISMQDA_BRIDGE` to use another address.

The browser check starts the bridge and a headless Chromium itself, drives the screens, and stops
exactly the two processes it started. It never opens a window and never plays sound. Set
`PLAYWRIGHT_DIR` to the folder of an installed `playwright` package, and set `PRISMQDA_EXE` to an
already built `prismqda` program to skip the cargo build.

Some processors crash long-running programs on their performance cores. `tools/run-checks.ps1`
pins the whole run to the efficiency cores (CPUs 16 to 31) and runs every check.

## Where things are

```
src/bridge.ts          call(cmd, args): browser POST /rpc, or Tauri invoke('call'); ApiError {code, message}
src/types.ts           a type for every command and response in docs/API.md
src/pickFile.ts        the ONE place that asks for a file path (typed today; wire the native dialog here)
src/offsets.ts         UTF-16 <-> scalar offsets, in one tested module
src/lib/               pure logic, all unit tested: segments, lanes, labels and colors, codebook tree and
                       moves, shortcuts, event wording, coverage, units, threads, import mapping, targets
src/state/             the store, the announcer, and actions.ts (everything a person can do)
src/surface/           the text surface: DOM building, the offset map, the component
src/components/        screens and dialogs
tools/                 the browser check (harness, scenarios, servers, synthetic test data)
scripts/               the license check
```

## The text surface: how a keyboard or screen-reader user selects words

This was the hardest requirement, so it was tried first.

**The problem.** Coding needs a caret and Shift+arrow selection in running text, plus margin
stripes, headings, unit marks, and speaker names drawn around that text. Plain text on a page has no
caret unless the person turns on the browser's caret browsing. A text area has a caret but cannot
draw anything inside itself.

**The spike.** A throwaway page with `contenteditable="true"`, `role="textbox"`,
`aria-multiline="true"`, `aria-readonly="true"`, and every `beforeinput` event cancelled was driven in
headless Chromium (the engine inside WebView2). Results:

- The caret appears and moves with the arrow keys, Ctrl+Left/Right (by word), Home/End.
- Shift+arrows, Ctrl+Shift+arrows, and Shift+Home/End select, one grapheme at a time (an emoji,
  or a letter with a combining accent, is one step).
- Typing, Backspace, Delete, Enter, Ctrl+B, and Ctrl+X do not change the text.
- Tab leaves the element, so there is no keyboard trap.
- A mouse drag selects.

**The decision.** The document is a `contenteditable` element that cannot be edited
(`src/surface/TextSurface.tsx`):

- `role="textbox" aria-multiline="true" aria-readonly="true"`, named by the source heading, and
  described by a hidden sentence that explains the keys.
- `beforeinput`, `paste`, `cut`, `drop`, and `dragstart` are cancelled. If an input method still
  changes the text, an `input` handler puts the text back from the model.
- The text is drawn as one `<span>` per piece, each holding one text node, with nothing else between
  them and `white-space: pre-wrap`. So every character in the DOM is one character of the source, in
  order, and DOM positions map to offsets exactly (`src/surface/domMap.ts`).
- The stripes live in a separate margin element that is hidden from screen readers. Their text
  equivalents are the "Codings here" list (codings in the current paragraph or unit, as plain
  buttons), a spoken note when the caret enters or leaves coded text ("In Welcome."), and the Grid
  view, which is plain rows of text.
- Single-key shortcuts (1 to 9, I, R, N, E, U, Enter) work only while the text has the focus, so
  they never interfere with typing in a box (WCAG 2.1.4).
- The selection is read again at the moment a key is pressed and when the text loses the focus,
  because the browser reports selection changes a moment late.

**Alternatives rejected.**

| Alternative | Why not |
| --- | --- |
| `<textarea readonly>` | Best screen-reader support and trivial offsets, but nothing can be drawn inside it. Stripes, headings, unit marks, and speaker names would need an overlay that has to be kept in step with its wrapping and scrolling. It is the fallback to try first if screen-reader testing shows trouble with the chosen design. |
| A custom caret (a focusable `div` with our own key handling and a live region) | Total control, but it rebuilds caret movement by word, by line, and through right-to-left text, and screen readers would get no native reading at all. |
| Plain selectable text | A keyboard user cannot select without turning on caret browsing in the browser. |
| `contenteditable="plaintext-only"` | The same as the chosen design for our purposes; it works only in Chromium. `true` with every input cancelled is the more widely supported spelling. |
| A canvas or a hidden-textarea editor component | Heavy, and re-creates what the browser already does. |

**Limits and risks, stated honestly.**

- I could not run a screen reader (NVDA or Narrator). A `role="textbox"` with `aria-readonly` should make
  them switch to focus mode and read caret movement and selection natively, but that is unverified.
- Input methods (Chinese, Japanese, Korean) can start a composition inside a contenteditable
  element before anything can be cancelled. The restore handler is tested by simulating a change,
  not with a real input method.
- Windows high contrast and the WebView2 wrapper were checked in headless Chromium only.

## Offsets

The engine counts Unicode scalar values; JavaScript counts UTF-16 units. `OffsetIndex`
(`src/offsets.ts`) remembers where the characters above U+FFFF sit and converts both ways with a
binary search, so a document with none costs nothing. A position between the two halves of one
character snaps to its start or its end. `src/surface/domMap.ts` is the only code that turns a DOM
selection into scalar offsets and back. Tests use emoji, letters with combining accents that have no
single-character form, CJK (including characters above U+FFFF), and Hebrew, Arabic, and Adlam
(right to left); the browser check also codes an emoji, a combining accent, CJK, and Hebrew text
through the real UI and compares the engine's stored offsets with offsets computed independently.

Selections are trimmed of spaces and line breaks at both ends before they are applied (a double click
on Windows also selects the space after a word).

## Large sources

A source with more than 1,000 units or 400,000 characters opens in the **Grid** view, a windowed list
(`src/components/GridView.tsx`): only the rows on screen are in the page, the grid is a real
`role="grid"` with `aria-rowcount`, and the active row is announced through `aria-activedescendant`.
The Document view asks before drawing such a source. Replies are listed in the Thread view
(`role="treegrid"`, `aria-level` per row, depth-first order from `parent_id`).

Measured by the browser check (headless Chromium, pinned to the efficiency cores of the author's PC,
engine in a debug build):

| Case | Time |
| --- | --- |
| Open a 15,000-unit source in the grid, after the dialog closes | about 80 to 260 ms |
| End key to the last of 15,000 rows | under 50 ms |
| Code 4 selected rows of 15,000 (number key to "Coded 4 units") | about 270 to 360 ms |
| Document of about 49,000 characters with 267 stripes | about 130 to 150 ms |
| Document of about 376,000 characters with 300 codings (the biggest that opens as a document) | about 200 ms |
| Code a unit in that document | about 100 ms |

## Accessibility

- Landmarks: a banner (with the main-area navigation), a Sources area, one `main` (the source), a
  Codebook-and-Details area with a Codebook region and a Details region, and a status line. Every
  control sits inside one of them (the dividing lines are inside their areas). A skip link is the
  first Tab stop.
- Every area has a heading. Heading levels never skip.
- Dividing lines are focusable `role="separator"` controls: arrow keys move them (Shift for bigger
  steps, Home and End for the limits), and the sizes are remembered in `localStorage` (inside
  `try`/`catch`).
- F6 and Shift+F6 move between the areas; F1 opens the printable shortcut sheet.
- Dialogs (React Aria `Modal`) trap the focus, hide the page behind them, close with Escape, and give
  the focus back.
- Results and errors go to two live regions (polite and assertive) that are also the visible status
  line. Errors clear after 12 seconds. A note about the caret is held back after an action is
  announced so it never talks over it.
- Nothing carries meaning by color alone. Each code has a color from a color-blind-safe palette
  (Okabe-Ito plus gray), a pattern chosen from its color, and a short text label (`WEL` for
  "Welcome"; labels are unique and stable). Selected things also have a bar and bold text. Coded
  text is underlined.
- Light and dark follow the system. Windows high contrast (`forced-colors`) was checked
  with the browser's forced-colors emulation: selection uses thick borders instead of fills (the
  browser puts a plain plate behind text in forced colors, which hid text on filled buttons), and
  stripes keep their patterns with system colors. `prefers-reduced-motion` removes the transitions.
- Text zoom to 200% (root font size) and a 683 and a 320 pixel wide window: the three areas stack
  and the area scrolls; nothing needs sideways scrolling.

### axe

`@axe-core/playwright` runs on every screen and dialog in the browser check with the tags
`wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa`, and `best-practice`. The final run covered
37 screens and dialogs (start screen, workspace, every dialog step, code details, memos, history,
the shortcut sheet, the grid, the thread view, a large coded document, dark mode, forced colors,
200% text, a 683 pixel window, and error messages) with 0 violations. The rules switched off, and why:

- `color-contrast` is switched off only in the forced-colors run, where axe cannot judge
  system colors.
- The `region` rule is not applied to the list of matches under the "Apply a code" box. React Aria
  places that pop-up at the end of the page; it belongs to the box through `aria-controls`.

Finding axe turned up: a tab panel with an accessible name counted as content outside a landmark
(the main-area tabs became a navigation), splitters outside landmarks (moved inside their areas),
heading levels that skipped, `aria-level` on grid rows (the thread now uses `treegrid`).

## Tests

- `npm test`: 89 unit tests: offsets (round trips for every sample, random text, speed), segments,
  stripe lanes, palette and labels, codebook tree, filtering and moves (up, down, indent, outdent, drop),
  shortcuts, event wording, coverage, units, threads, import mapping, favorites, targets (what a code
  applies to in free-span and unit projects), scope (paragraph or unit at the caret), in-vivo names,
  and the bridge (browser and Tauri paths, errors).
- `npm run browser-check`: 56 steps and about 300 checks against the real engine. It creates
  projects, imports a text file, a CSV (with the column wizard), a transcript, a Markdown file, a
  15,000-row CSV, a 376,000-character document, and problem files; codes by mouse, by
  keyboard alone (Shift+arrows), with number keys, the type-ahead box, in-vivo, and repeat; changes,
  annotates, marks, and removes codings; edits, reorganizes (buttons, Alt+arrows, and a real
  drag), merges, splits, retires, and restores codes; writes memos of every kind and scope; reads the
  history; opens the shortcut sheet; works in a unit project; and checks themes, forced colors,
  reduced motion, zoom, layout memory, Tab order, focus traps, the skip link, and an unreachable
  engine. Screenshots of failures go to `test-results/`.

## API gaps and oddities found

- `coding.list` does not say whether a coding is an anchor example. The interface replays the
  `anchor.add` and `anchor.remove` events (up to 20,000) when a project opens and keeps the set
  itself afterwards. A field on the coding row would be simpler.
- Codings can be listed only one source at a time, and the source list has no coded amount. The
  coded bar beside each source is built from `coding.list` calls made one by one in the background.
  A "coded characters or units" count in `source.list` would remove that.
- The split dialog can only offer the codings of the open source, because of the same limit.
- Threaded posts cannot be imported yet (`social_mapping` and the JSON profiles return "not built
  yet"), so the Thread view is checked by changing the unit list in flight so that each unit replies to
  the one before it. The ordering and indentation logic are also unit tested.
- A `coding.add` event does not hold the quoted words or the code's name, so the history looks them
  up (it fetches the text of up to 12 sources and reads code names from the codebook as it is now).
- `project.info` has no research questions or method fields to show.
- Error messages from the engine are shown as they are. Some begin with a technical prefix such as
  "file error:".
- `memo.add` for the project needs `target_id` to be absent or null (the engine says so); this is not
  in docs/API.md.
- Not exercised: a read-only project (one made by a newer version). The interface disables every
  change in that case, but the check has no way to make such a file.

## Packages and licenses

Shipped (reachable from `dependencies`): `react`, `react-dom`, `react-aria-components`
(Apache-2.0, with its React Aria and React Stately packages), and `@tauri-apps/api` (Apache-2.0 or
MIT; loaded only inside the Tauri window, as its own chunk). 17 packages in all: Apache-2.0, MIT,
and 0BSD.

Build and test only: `vite`, `@vitejs/plugin-react`, `typescript`, `tsx`, `@types/*`,
`@axe-core/playwright`, `axe-core`. `scripts/check-licenses.mjs` reads every installed package's
license, fails on anything that is not permissive, and lists the only exceptions, all build or test
tools that never ship: `@axe-core/playwright` and `axe-core` (MPL-2.0, used by the browser check
and never imported by the interface) and `lightningcss` with its Windows binary (MPL-2.0, a CSS
processor that Vite 8 installs and uses at build time; its code is not in the built files). The check
also fails if an exception becomes reachable from `dependencies`, if `src/` mentions axe, or if the
built JavaScript contains axe.

`.npmrc` sets `legacy-peer-deps=true` so that `@axe-core/playwright` does not pull in its peer
`playwright-core`; the browser check imports Playwright from `PLAYWRIGHT_DIR`.

## Not verified

- Any screen reader (NVDA, Narrator): not available to the author of this slice.
- A real input method editor.
- The Tauri window and WebView2 (the Tauri path of the bridge is unit tested with a fake `invoke`).
- Printing the shortcut sheet to paper: only the print style sheet is checked.
- Touch input.
