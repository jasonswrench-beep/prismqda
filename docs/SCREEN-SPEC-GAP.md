# Screen Specifications Book, Part 1: where the build differs, and what changes

Source: `PrismQDA_Screen_Specifications_Book_Part_1_Revised.docx` (three screens: Splash, Welcome and Open Project, Main Application Layout; plus shared interaction and the SVG assignments). The book's three pictures are the visual reference. This file is the working list: every difference, what the book asks for, and the change. Items are ticked as they are built. Written 2026-11-06 against alpha 9 (main d520aa8).

## Status (2026-11-06, second pass)

The author sent the Complete book (Parts 1 to 5) after the first pass. Part 1 is unchanged; Parts 2 and 3 add eighteen more screens and Part 5 the icon catalog. They agree with the shell built here, so nothing already built is undone. What each part now asks of the build:

| Book part | Screen | State |
|-----------|--------|-------|
| 1 | Splash, Welcome, Main layout (title bar, tabs, Home ribbon, Documents, Codes, reader, Retrieved Segments, Segment Details, status bar) | Built in this pass (see the table below for what is still short of the picture) |
| 2 | New Project Setup | Built: name, save location with the final path shown before creating, Blank or Method template, optional study details kept as a memo, Cancel and Create Project |
| 2 | Import and Prepare Data | Built (alpha 11, kinds added in alpha 12) as the Import Data page: drop zone, the six kinds, the Import Queue, a preview with the first words, destination folder, case, duplicate choices, per-file outcomes, and retry. Documents, Reference Library (RIS, BibTeX, CSL JSON), Web / News (saved pages and feeds; no fetching) and Email (EML, MBOX) work. Linking a newer version of a file works from alpha 17, Audio / Video from alpha 18 (through caption files), and Images from alpha 19. |
| 2 | Codebook Editor | Built (alpha 11; properties in alpha 12): toolbar, tree, Code Details, Examples (with counterexamples), Usage, History, and Compare. Mutually Exclusive Group and Weighted / Rated (with its described scale) work. AI Suggestible is shown, not built |
| 2 | Query Builder, Code Matrix | Built (alpha 13) as the Explore page: Query Builder with a readback in words, groups, "also coded", text, document, case, case variable, coder and rating clauses, stale-result marking and results that open at their source; Code Matrix with an explicit metric, descendants rule, group size N and evidence behind every cell. Not built: saved and frozen queries, dates, percentages, Crosstabs, Concept Map |
| 3 | Project Dashboard | Built (alpha 11): four counts, Project Activity, Data by Type, last saved, Project Details |
| 3 | Media coding, Text Analytics, Statistics, Crosstabs, Mixed Methods, Joint Displays, Synthesis grids, Team, AI, Report Builder | Not built; their ribbon tabs say so |
| 3 | Settings and Preferences | Built (alpha 11): all eleven sections are listed; General, Interface, Project Defaults, Privacy & Security, Accessibility, Updates, and About have content, and Coding, Analysis, AI Assistant, and Team & Sharing say they are not built |

First-pass items, as built: A1 to A3 (splash, startup status, blocked state with Retry and Close), B1 to B6 (rail, welcome copy, three cards, recent projects with times and menus, Get Started, sample projects through the engine), C1 to C19 except the ones listed next.

Still short of the picture after this pass: the title bar is below the system's own title bar (the window controls are the system's, not merged into our bar); a sample copy keeps the sample's name as its project name (the name chosen for the copy is its file name); protected projects (unlock) do not exist yet; the Documents tree has no durations because sources carry none; "Case" appears once a document is in a case; split view is a disabled control with a reason; Auto Code is built in alpha 14 as rule-based coding with a review queue and is named "Auto Code" without the book's "(AI)", because it has no model (decision 80); Code Summary, Code Relations, Query, Visualize and Compare Cases are built in alpha 13.

## Decisions taken to follow the book (author can overrule)

1. **Navigation follows the book.** The open-project window is one workspace (Documents over Codes, reader over Retrieved Segments, Segment Details at the right). The fourteen tabs (Home, Import, Code, Memos, Explore, Mixed Methods, Visual Tools, Synthesize, Text Analytics, Statistics, Report, Team, AI, View) switch the **ribbon's command group only**; the project, document tabs, and selection stay. Pages that exist today (project overview, data table, memos, report, settings) stay reachable as commands inside those ribbons and from the title bar gear, and open in the work area with a clear way back. Tabs for parts that are not built show their group with a plain "not built yet" notice instead of silently missing, so the row matches the picture.
2. **Names follow the book:** Documents (was Sources), Codes (was Codebook), Retrieved Segments (was Passages), Segment Details (was Details).
3. **File extension stays `.pqda`.** The book says the picker must use the supported formats, not the picture's `.ppda`.
4. **Rating and case become real data.** The reference case shows a 4 / 5 rating and "Case: Urban Residents", so the engine gets a nullable rating on a coding (first schema migration, version 2, the original kept as a copy as the engine already does) and commands to make cases and put sources in them.
5. **Light surfaces everywhere except the splash.** The workspace in the picture is white cards on a pale lavender-gray page, with purple as the one accent. Dark navy belongs to the splash.

## A. Splash (book section 01)

| # | Book | Build today | Change |
|---|------|-------------|--------|
| A1 | Dark navy splash: faceted prism, PrismQDA wordmark, "Qualitative • Quantitative • Mixed Methods", "A complete research platform for coding, analysis, statistics, and synthesis.", spectrum along the lower edge (decorative) | While starting, a blank page with "PrismQDA" and "Starting…" | New splash view in that look, using `brand-logo.svg` on navy; the spectrum is decorative and hidden from assistive technology |
| A2 | One accessible name for the brand; status text separate; no artificial minimum display time | n/a | Splash shows only while starting and goes away the moment the launcher can take input; status line "Starting PrismQDA" appears only if startup takes noticeable time, indeterminate, honors reduced motion |
| A3 | Startup blocked: actionable error naming the failed step, Retry and Close, keyboard focus, details kept apart from the summary | A notice above the start screen | Startup-blocked view with Retry and Close and an expandable "Details" |
| A4 | A failed optional startup task reaches Welcome with a concise notice (damaged recent list falls back to the empty state) | Recent list read from storage without a guard | Guard and notice |

## B. Welcome and Open Project (section 02)

| # | Book | Build today | Change |
|---|------|-------------|--------|
| B1 | Window with the PrismQDA mark in the title bar, and a **left rail**: Home (current), Data, Code, Explore, Text Analytics, Statistics, Synthesize, Report, Team, Settings, with the project-dependent entries visible and saying a project is needed | One centered column, no rail | Add the rail; project entries disabled with the reason; Settings opens before a project exists (Settings page works without a project) |
| B2 | "Welcome to PrismQDA" and "Qualitative, quantitative, and mixed methods analysis in one project." | "Code interviews, surveys, and documents on your own computer…" | Use the book's two lines |
| B3 | Three action cards: New Project (purple, primary, plus icon), Open Project (folder icon), Sample Projects (graduation cap) | Two form cards with typed file paths | Three cards; New Project opens a guided dialog (name, destination with Browse, template or blank, optional metadata, confirm destination); Open Project opens the file picker (typed path stays as a fallback in the dialog); Sample Projects opens a chooser |
| B4 | Recent Projects: name with extension, "Modified today, 2:14 PM", an overflow menu per row ("More options for X"; Locate, Remove from recent list), row and menu are separate focus targets | Raw file paths as links, no times, no menu | Rows with name, modified time, overflow menu; a missing project offers Locate and checks identity before saving the new path; Remove changes only the list; recent entries store the modified time |
| B5 | Get Started panel: Import your data, Create a codebook, Code your first document, Explore and analyze, Build a report; progress only when the action is done | None | Checklist; before a project opens each item offers create, open, or sample; ticks come from real project facts (sources, codes, codings) |
| B6 | Sample Projects: bundled studies, an editable copy for practice, the original never changed, the sample identity visible in the open project | None | Bundle the Urban Mobility Study as the first sample (made-up data), copy on open, "Sample" tag in the title bar |
| B7 | Validation before the workspace opens; read-only offer; older schema: keep the original and check the migrated copy; unlock for protected projects | Engine validates, keeps an original on migration, opens read-only when newer; no unlock | Show working state and plain messages for each case; unlock is not possible until project encryption is built (stated, not faked) |
| B8 | Icons: ui-plus, src-folder, ui-more, ws-home/data/code/explore/text/stats/synth/report/team/settings; graduation cap needs an approved match | Not used here | Use them; graduation cap: ask for an approved asset (a plain placeholder is not shipped silently) |

## C. Main application layout (section 03), visual elements

Reference: one 1586 x 992 picture. The fixture for comparing and for acceptance is the Urban Mobility Study (INT-001 – Alex, 12 retrieved segments).

### Title bar
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C1 | Light bar: mark and wordmark, a divider, **Urban Mobility Study ⌄** (project menu), a wide "Search project (Ctrl+K)" field, bell with an unread dot, gear, **RM** avatar with a chevron (actor menu), native window controls | Dark navy bar: wordmark, project name and two tags, search button, Shortcuts, Close project | Rebuild in the book's order and look; Close project and Shortcuts move into the project menu and the help menu; the actor menu shows who changes are attributed to; notices list behind the bell |

### Ribbon tabs and the Home ribbon
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C2 | Text-only tab row: Home, Import, Code, Memos, Explore, Mixed Methods, Visual Tools, Synthesize, Text Analytics, Statistics, Report, Team, AI, View; current tab underlined in purple | Icon workspace tabs: Home, Data, Code, Synthesize, Report, Settings | Replace per decision 1 |
| C3 | Home ribbon, left to right: **Project** (New Project, Open Project, Save ⌘S) · **Add Documents** with a chevron · **Add** (New Code, New Memo, New Case) · **Coding** (Code Selection, Auto Code (AI), Code In Vivo, Quick Code ⌘D) · **Explore** (Code Summary, Code Relations, Query) · **Analyze & Report** (Visualize, Compare Cases, Report Builder with a chevron); large icons, two-line labels, group captions under, thin dividers | Undo, Backup · Import, Folder, Memo · Code, In vivo, Repeat, New code · Search, Passages · scope · Export | Same command set in the book's order and look; today's Undo, Backup, Folder import, Repeat, Passages and scope commands move to their tabs (Import, Code, View) so nothing is lost; Auto Code, Code Summary, Code Relations, Query, Visualize and Compare Cases are shown unavailable with "not built yet" |
| C4 | A command that cannot run says why at its point of use | Done (ribbon says why) | Keep |

### Documents and Codes (left column)
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C5 | Cards on a pale page, white with a thin border and rounded corners; pane title row with an icon, Add, Search, more | Flat panes separated by lines | Card look for all four panes |
| C6 | **Documents:** project root "Urban Mobility Study (52)" with folders "Interviews (18)" expanded, rows "INT-001 – Alex (45:12)" (lavender when current), further folders with counts; folder and document icons | Flat list with import coverage bars | Tree with folders from the engine's folder field, counts, document icons, durations when the source has them; search within the pane; the coverage bar becomes a thin line under the name |
| C7 | **Codes:** hierarchy with colored dots, bold group names, counts in parentheses; parents show a group count | Tree with dots, counts at the right | Counts in parentheses beside names, bold group names; count rule documented (a group counts the distinct codings of itself and its children) |

### Reader
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C8 | Tab strip: document icon, name, close, plus | None (one source at a time) | Open-source tabs per source, remembered scroll and selection |
| C9 | Header: large document icon, title, "Interview | 45:12 | Participant: Alex | Case: Urban Residents", "‹ 1 of 18 ›", split-view icon, more menu | Title, facts line, previous and next | Header as in the book; split view arrives as a disabled control with a reason until built |
| C10 | Transcript: line numbers at the left, **speaker labels in a bold column**, wrapped lines aligned with the text after the label, colored highlights, thin colored markers at the far right edge | Line numbers (done), highlights (done), labels inline, markers wider with text labels | Speaker column; thin markers (the short label moves into the marker's tooltip and the Codings here list, so it still is not color alone) |
| C11 | No coding toolbar row above the text | Apply box, Repeat, In vivo, Previous and Next uncoded, selection line | Move to the ribbon (Code Selection opens the picker with the apply box; Quick Code repeats); keep keyboard keys; "Codings here" stays as a collapsible text equivalent |

### Retrieved Segments
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C12 | "Retrieved Segments (12)": filter dropdowns All Codes, All Sources, All Cases, a search box, more menu; **table** with checkbox, #, Source, Code (colored chip), Case, Excerpt (preview), Notes; first row highlighted | A header with a code chooser and two toggles, and a list of cards that is empty until codes are chosen | Table as in the book, filled by default with every coding in scope; sortable headers; row check boxes with a header check box; "selection is outside the current results" message |

### Segment Details
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C13 | Header "Segment Details" with a layers icon and a close control (View ribbon reopens it) | "Details" with Coding, Code, Memos tabs | Single Segment Details panel; Code and Memos tabs move out (code definition opens from the Codes pane, memos from the Memos ribbon) |
| C14 | Selected Segment quote card; Source with a document icon; Lines "007 – 009 (3 lines)"; **Case** with an icon | Source, Lines, Where; no case | Add Case (engine); drop "Where" to the tooltip |
| C15 | Applied Codes (1): pill chip with dot, a cross, a chevron, and a plus | Chip with cross; plus | Add the chevron (assignment options) |
| C16 | Memo (1): row "M-014: Service reliability and daily stress" with an open icon | Titles only | Memo numbers (M-0xx) and an open action that opens that memo |
| C17 | **Weight / Rating:** info icon, four filled stars of five, "(4 / 5)", chevron | Missing | Rating on a coding (engine migration), stars with keyboard, "unset" distinct from zero, scale text behind the info icon |
| C18 | Provenance: Coded by RM (Researcher), Coded on, **Last modified**, **History: 1 code added, 0 removed** | Coded by, how, coded on | Last modified and history counts from the event log |

### Status bar
| # | Book | Build today | Change |
|---|------|-------------|--------|
| C19 | Filters: None · Jobs: 1 running (...) 62% · Saved ✓ date and time · Local Project ✓ Synced | Message line, "Every change is saved…", "Coding as…", "AI is off" | Four fixed facts: Filters (the scope), Jobs, Saved with pending / saving / saved / failed, Local Project and Synced (offline shown as such); the live message region stays |

## D. Shared (sections 04, 05)

- Project lifecycle messages for open, create, switch, close, read-only (D1); focus rules (D2); the seven end-to-end scenarios become browser-check scenarios (D3); icon assignments per section 05 (D4: all named icons exist in the library except the graduation cap, the New, Open and Save project glyphs, the rating stars, split view, and local project storage, which need approved artwork).

## What already matches

Documents over Codes, reader over results, details at the right, one status bar (structure); highlighted words with a marker column at the right (alpha 9); line numbers; Segment Details content except case, rating, last modified, and history; chips for applied codes; previous and next source buttons; commands say why they are unavailable; F6 moves between regions; keyboard coding; the search box (Ctrl+K).
