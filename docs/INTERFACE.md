# PrismQDA Missing Tools and Interface Design

> The interface design, adopted 2026-11-05 (decision 43 in [DECISIONS.md](DECISIONS.md)). The window is being rebuilt to it (decision 48). "As built" at the end says what exists today.

The first draft of the specification covered the coding core but left out more than 80 tools researchers expect, and it said almost nothing about how the program should look. This document lists what was missing, ranks it, and sets an interface direction with a familiar researcher layout and a modern, calm finish.

## 1. Tools the first draft missed

The gaps fall into eleven areas. Priority means: Must, needed for a credible 1.0; Should, expected within the 1.5 release; Later, valuable but not blocking. Each tool is now written into the PrismQDA Specification at the section shown.

### 1.1 Data types and import

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| PDF region coding (figures, tables) | Researchers code articles and reports in their original layout | Must | 8.2 |
| Image coding in 1.0 (was 2.0) | Photo elicitation and visual data are common in student work | Must | 8.2 |
| Table documents with cell coding | Spreadsheets of notes and logs are coded cell by cell | Must | 5 |
| Reference manager import with PDFs | Literature reviews start from a reference library | Must | 20 |
| Structured-text preprocessor | Splits and codes long files in one step | Should | 5 |
| Email, news archive, and web page capture | Document analysis and digital ethnography | Should | 5, 21.3 |
| Watched folders | Ongoing data collection imports itself | Later | 5.1 |

### 1.2 Preparing data

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Manual transcription tools (foot pedal, text snippets, speed, rewind) | Many researchers still transcribe by hand | Should | 6.2 |
| OCR for scanned PDFs | Archival and historical documents | Should | 6.3 |
| Spell check and unknown-word correction | Cleaner text means better search and counts | Should | 6.1 |
| Translation layer | Multilingual studies code across languages | Later | 6.5 |
| Re-identification check | Ethics boards ask how identifiability was assessed | Later | 6.4 |

### 1.3 Coding

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Highlighter pens and symbol coding | Fast first reading before a codebook exists | Must | 8.1 |
| Open coding mode | Rapid inductive coding with on-the-spot names | Must | 8.1 |
| Segment weights and per-code rating scales | Intensity and rating studies; needed for weight-based charts | Must | 8.3 |
| Segment comments and names | Notes on a single coding decision | Must | 8.3 |
| Segment review table | Edit many segments without opening documents | Must | 8.5 |
| Mutually exclusive code groups | Required for clean content analysis and agreement | Should | 8.3 |
| Redundant-coding cleanup | Keeps counts honest after merges | Should | 8.5 |

### 1.4 Codebook and organization

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Smart and snapshot codes | Queries that behave like codes | Must | 13.2 |
| Smart document and code sets | Groups that update themselves | Must | 7.1 |
| Global filter bar | One filter across every open view | Must | 13.4 |
| Code variables, groundedness, density | Describe codes, not only documents | Should | 9.1 |
| Creative coding canvas | Sorts a long open-code list into categories visually | Should | 15.4 |
| Codebook version comparison and restore | Documenting how the codebook evolved | Should | 9.3 |
| Dynamic (time-varying) variables | Longitudinal and repeated-interview designs | Should | 7.3 |
| Project templates and named snapshots | Reuse a study setup; safe points before big changes | Should | 4.1 |

### 1.5 Memos, links, and synthesis

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Typed memos with icons and a memo manager | Memoing is central to most methods | Must | 10.1 |
| Segment-to-segment links with named relations | Captures contradictions, causes, and sequences that codes cannot | Must | 10.3 |
| Logbook | A dated research journal that feeds the methods section | Must | 10.4 |
| Paraphrasing and paraphrase matrix | A core step in qualitative content analysis | Should | 11 |
| Summary grid, tables, and explorer | Case-by-theme condensation | Must | 11 |
| Framework matrix | Framework analysis is widely taught | Should | 11 |
| Case profiles | One-page summaries for case study work | Should | 11 |
| Evidence board | Moves from findings to a written argument | Should | 11 |
| Report manager | A scrapbook of results that becomes a report | Should | 11 |

### 1.6 Search and computer-assisted coding

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Keyword in context in 1.0 (was 2.0) | The most-used exploration tool after search | Must | 12.1 |
| Section retrieval | Pull every answer to one interview question | Must | 12.2 |
| Similar-to-code search | Finds what coders missed | Should | 12.2 |
| Query by example | Finds passages like a sample passage | Should | 12.2 |
| Cluster extraction | Finds repeated stories and boilerplate | Later | 12.2 |
| Named entities, sentiment, concept lists | Quick orientation in large corpora | Should | 12.2 |
| Random segment sampling | Consistency checks on a code | Should | 12.2 |

### 1.7 Queries, mixed methods, and visual tools

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Visual query builder with position and hierarchy operators | Complex questions without syntax | Must | 13.1 |
| Code patterns and code sequence analysis | Which codes appear together and in what order | Should | 13.3 |
| Segment matrix, typology table, side-by-side display | Standard joint displays for mixed methods | Must | 14.2 |
| Qualitizing and code-by-variable bubble plots | Moves from numbers to themes and shows four dimensions at once | Should | 14 |
| Deviation tables and similarity analysis | What is typical of each group | Should | 14.2 |
| Mixed-methods design templates | Sets up convergent and sequential designs | Should | 14.3 |
| Full concept maps (layers, templates, synced nodes, relations) | The first spec had only a simple canvas | Must | 15.3 |
| Document portrait, codeline, document and code maps, trends, Sankey | Standard visual overviews of coding | Should | 15.1 |
| Dendrograms, MDS, correspondence analysis, heatmaps | Statistical maps of codes and words | Should | 15.2 |
| Project history heatmap | Shows work over time for audit and supervision | Later | 15.1 |

### 1.8 Text analytics

The first spec deferred all text analytics to 2.0 and listed only basics. Missing: rule-based dictionaries with proximity conditions, dictionary-building aids, included open dictionaries, word trees, readability and style measures, factor-analysis topic models with stability checks, classifiers with cross-validation, and reference-corpus comparison. Priority: Should (1.5), spec section 16.

### 1.9 Statistics

The first spec had core tests only. Missing: a full data editor with transformations, merging, aggregation, and restructuring; factorial ANOVA and ANCOVA; logistic and curve regression; scales and item analysis; factor and cluster analysis; ROC; time series and single-case designs; permutation tests and power; a probability calculator; and a script recorder. Priority: Must for the data editor, Should for the rest, spec section 17.

### 1.10 Teamwork and training

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| User roles with passwords | Protects a shared project from accidental changes | Must | 18.1 |
| Agreement by code presence, frequency, or position with overlap threshold | Matches how different methods define agreement | Must | 18.3 |
| Holsti, Gwet's AC1, and Krippendorff's cu-alpha and Cu-alpha | Coefficients reviewers ask for | Should | 18.3 |
| Semantic domains | Agreement measured within groups of exclusive codes | Should | 18.3 |
| Coder training center | Trains new coders against expert coding before they code real data | Must | 18.5 |
| Self-hosted Team Server | Live collaboration without a commercial cloud | Later | 18.6 |

### 1.11 Platform, reporting, and extensibility

| Tool | Why it matters | Priority | Spec |
| --- | --- | --- | --- |
| Free read-only viewer | Supervisors review without installing a full tool | Must | 3.1 |
| Literature review workspace with screening | One of the most common student projects | Must (core), Should (screening) | 20 |
| Survey grid workspace | Open-ended answers beside closed answers | Must | 21.1 |
| Interactive HTML report and report builder | Shareable results for readers without the software | Should | 23.1 |
| Coded-segment report with title page and contents | The standard printed deliverable | Must | 23.1 |
| Macro recorder, scripting, local API, plug-ins | Automation and growth by others | Should (scripting), Later (plug-ins) | 25 |
| Additional method templates (framework, discourse, case study, quantitative content analysis, literature review) | Covers what methods courses teach | Should | 24.1 |
| Guided first project, sample projects, contextual help | Learning in the tool | Must | 24.2 |
| macOS, Linux, portable mode | Many students use Macs | Should (Mac), Later (Linux) | 3.1 |
| High-contrast theme and interface translations | Accessibility and reach | Should | 26 |

## 2. The first twelve to build

These twelve close the gaps a new user would notice in the first hour, in the order they should be built, because each one leans on the one before it.

1. The four-pane main workspace with ribbon, docking, and tabs (section 4 of this document). Everything else lives inside it.
2. Coding on PDFs, images, and tables, with margin stripes and highlighting, so every common data type works on day one.
3. Highlighter pens, open coding mode, and favorite-code keys, for fast first-pass coding.
4. Segment weights, rating scales, comments, and the segment review table.
5. Typed memos, memo manager, segment links with named relations, and the logbook.
6. Activation and the global filter bar, which every analysis view depends on.
7. Keyword in context and section retrieval, alongside text search and autocoding.
8. The visual query builder with smart codes and smart sets.
9. Summary grid and summary tables, the main bridge from coding to writing.
10. Joint displays: crosstab, segment matrix, typology table, side-by-side display.
11. Full concept maps with relations, layers, and data-synced nodes.
12. User roles, the coder training center, and agreement settings (presence, frequency, position).

## 3. Interface direction

PrismQDA should look and behave in a way researchers find familiar (a ribbon across the top, a four-pane coding window, colored coding stripes in the margin), with clean type, calm color, and fast keyboard paths.

### Adopted direction: Concept 03

PrismQDA adopts Concept 03 (chosen November 5): a navy prism brand on the startup and welcome screens, and calm, near-white working screens where the researcher's data carries the color. It is combined with one structural decision: a slim command ribbon across the top that never changes, and a left side panel that changes with the workspace.

*[Figure not copied: Concept 03: brand panel, welcome screen, project dashboard, coding workspace, Explore code matrix, Statistics, AI assistant, report builder, and icon strip. The mockup images are in the planning package, not in this repository.]*

The ribbon answers "what can I always do?" and the side panel answers "what can I do here?". Workspace tabs sit on the ribbon's top row, but switching tabs changes the side panel and main area, not the commands.

| Region | What it holds | Changes by workspace |
| --- | --- | --- |
| Title bar | Navy bar with the prism mark, project name, current workspace, search (Ctrl+K), window controls | No |
| Workspace tabs | Home, Data, Code, Explore, Mixed Methods, Text Analytics, Statistics, Synthesize, Report, Team, AI, with Settings as a gear at the right end | No (active tab highlighted) |
| Command ribbon | One row of the most important commands, in the same order everywhere (section 5) | No |
| Side panel (left) | The lists and tools for the current workspace (section 5) | Yes |
| Main area | The document, matrix, dataset, editor, or canvas, in tabs | Yes |
| Inspector (right) | Details of whatever is selected: segment, code, case, variable, or result | Content only |
| Bottom pane | Retrieved segments in Code, output log in Statistics, result preview in Explore | Yes |
| Status bar | Save state, active scope, coder layer, AI mode (off, local, or cloud), background jobs | No |

Changes made to Concept 03 before it becomes the specification:

- The same frame on every screen. The Code and AI screens keep the title bar, tabs, ribbon, and side panel used everywhere else.
- One color per code everywhere: tree swatch, text highlight, margin stripe, matrix cell, chart bar, and report label.
- Margin coding stripes beside the text and a retrieved-segments pane under the document. The Coding Inspector stays on the right.
- Every count names its unit (codings, segments, documents, or cases). Statistics refuse tests whose rows are not independent: the concept's chi-square on 131 codings from 45 participants would be blocked, with an offer to count each participant once.
- The AI screen gets its review stage: model, local or cloud status, a preview of what is sent, cited sources, and accept, edit, or reject.
- Gradients appear only on the startup and welcome screens.
- Reference mockups for import, codebook editor, media coding, text analytics, synthesis matrices, joint displays, team reliability, and settings follow below.

### Reference mockups for the remaining screens

Two further boards (added November 5) show the screens Concept 03 left out. They set the look and content for import, the codebook editor, media coding, text analytics, synthesis matrices, joint displays, team reliability, and settings. The table lists what to keep and what to change before each screen is built.

*[Figure not copied: Board 1: dashboard, import, coding, codebook editor, media coding, text analytics, synthesis matrix, joint display, code matrix, statistics, team reliability, and settings. The mockup images are in the planning package, not in this repository.]*

*[Figure not copied: Board 2: expanded import, codebook editor, media coding, text analytics, synthesis grid, joint display, team reliability, and settings. The mockup images are in the planning package, not in this repository.]*

| Screen | Keep | Change |
| --- | --- | --- |
| Import | Side panel of import sources (files, references, web, email, OCR, transcription, survey) and an import queue with each file's status | Already matches the adopted frame; it becomes the Data workspace's import view |
| Codebook editor | Definition, inclusion and exclusion criteria, examples, usage, and history; properties for mutually exclusive groups, weights, active, and AI-suggestible | Show each code in its one assigned color |
| Media coding | Video with a coded timeline under it, a timed segment list, and a synced transcript | Nothing structural |
| Text analytics | Stop words, lemmatizing, frequency bars, topics | The word cloud is an optional view, not the default; frequencies carry the analysis |
| Synthesis matrices | Summary grid of cases by themes; framework matrix | The High, Moderate, and Low dots need a stated rule (for example, share of a group's cases coded), shown in a legend |
| Joint display | Quantitative result, illustrative quote, and integration verdict side by side | The figures disagree: 78% of 18, 50% of 22, and 25% of 8 make 27 participants, not the 37 of 48 shown, and 37 of 48 is 77%. Every figure must be computed from the project and shown with its n |
| Statistics | Setup beside results; results, assumptions, and output tabs | It calls p = .053 a "marginal association." Report it as not statistically significant, with the effect size and its interval. Primary Need must be one value per participant, never a count of codings |
| Team reliability | Choice of coefficient (Cohen's kappa, Krippendorff's alpha, percent agreement) and results by code | The boards label .82 both "Substantial" and "Excellent," and one says coding is "ready for full analysis." Show the benchmark the researcher picked, with its citation, and no verdict |
| Settings | Appearance, AI, team, import and export, accessibility, and light, dark, and high-contrast themes | "Auto-save every 5 minutes" conflicts with the specification, where every committed change saves at once; keep scheduled backups instead |
| All screens | Calm neutral working screens and the workspace icon set, now including Mixed Methods | Navigation varies between boards (a left workspace list on some, top tabs on others). Use the adopted frame everywhere: top workspace tabs, the fixed command row, and a context side panel. Drop the ® after PrismQDA until the name is cleared and registered |

### 3.1 Conventions the field shares

Researchers share a handful of layout conventions. PrismQDA adopts all of them so a researcher trained elsewhere is productive in minutes.

| Convention | What users expect |
| --- | --- |
| Ribbon with labeled tabs | Commands grouped by task (import, code, analyze, report), with large icons for common actions and small ones for the rest |
| Four-pane main window | Documents top left, codes bottom left, the open document top right, retrieved segments bottom right |
| Navigation, list, and detail | A tree to choose a category, a list of its items, and a detail view of the selected item |
| Margin coding display | Colored brackets or stripes beside the text, one per code, with names on hover |
| Activation by click | Click documents and codes to activate them; retrieved segments update at once |
| Drag and drop everywhere | Drag text onto codes, codes onto text, documents into groups, items onto maps |
| Context menus | Right-click any item for its full set of actions |
| Docked and floating windows with tabs | Any pane can float onto a second monitor or join a tab group |
| Welcome screen | Recent projects, sample projects, and getting-started links |
| Home dashboard | Project overview with counts and starter charts |
| Status bar | What is active, what filter is on, background jobs, and save state |
| Click-through charts | Every chart element opens the excerpts behind it |

### 3.2 Why the first design felt clunky

The first specification described only a left list, a center document, a right codebook, and a detail panel. With no ribbon, no lower panes, and no layout rules, that design ends up feeling clunky in predictable ways:

- Every command lives in dialogs and menus instead of a visible toolbar, so common actions take three clicks instead of one.
- Coding results appear nowhere near the coding, because there is no retrieved-segments pane.
- Panes have no visual hierarchy, so the document competes with lists for attention.
- Default system fonts, gray boxes, and heavy borders make it look like a form, not a reading tool.
- Analysis views open as separate windows with their own styles, so the program feels like several programs.

### 3.3 PrismQDA layout rules

1. The document is the hero: it gets the most space, the best typography (serif or humanist reading font, 1.5 line spacing, 72-character measure), and the quietest background.
2. Panes are light: thin dividers, no heavy borders, subtle headers with a pane title, a count, and two or three icon actions.
3. One accent color for selection and focus; code colors carry meaning, so chrome stays neutral.
4. Every analysis opens as a tab in the main window by default, styled the same way, and can be popped out.
5. Saved layouts: Coding (four panes), Reading (document full width with codes on the right), Analysis (results tab large, codes and documents small), Teaching (student view), switched from the View tab or a status-bar menu.
6. Density setting (comfortable, compact) for large monitors and laptops.

## 4. Main workspace

The main window is the four-pane layout researchers expect, with the document given the largest pane and the retrieved segments directly below it, so the result of every coding action is visible without switching views.

Note (November 5): this mockup shows the earlier full-ribbon direction. The Concept 03 layout in section 3 replaces it.

*[Figure not copied: main workspace mockup · ribbon, four panes, coding stripes, floating coding bar. The mockup images are in the planning package, not in this repository.]*

main workspace mockup · ribbon, four panes, coding stripes, floating coding bar

What the mockup shows, top to bottom:

- Title bar with the project name, a command palette (Ctrl+K), and quick access buttons.
- Ribbon with study-order tabs; the Code tab is open, showing coding commands and a weight slider.
- Documents pane (top left): folders with counts; the open document is marked.
- Codes pane (bottom left): color-coded tree with counts and a filter box; the activated code is tinted.
- Document browser (top right): tabs for open documents, paragraph numbers, a reading font, highlighted coded text, margin stripes with code names, and the floating coding bar under a fresh selection.
- Retrieved segments (bottom right): every segment of the activated code, with its source, updating as coding happens.
- Status bar: what is active, the global filter badge, background jobs, and save state.

## 5. Ribbon, workspaces, and side panel

The ribbon is one slim row of commands that stays the same in every workspace, so the most important actions are always in the same place. Workspace tabs above it change the side panel and main area, never the commands.

| Group | Commands | Notes |
| --- | --- | --- |
| Project | Save snapshot, Undo, Redo | Autosave always runs; a snapshot names a restore point |
| Bring in | Import, New memo | Import opens one wizard for any file type |
| Code | Code, In vivo, Quick code, New code | Code applies a code to the selection (type to search); Quick code reapplies the last code |
| Connect | Link | Links a segment to another segment, memo, case, or source |
| Find | Search, Retrieve | Retrieve opens retrieved segments for the activated codes and documents |
| Scope | Active scope chip, Reset | Shows what is activated, for example "3 documents, 2 codes"; one click resets |
| Assist | AI Assist | Grey when AI is off; shows whether the model is local or cloud |
| Output | Add to report, Export | Add to report sends the selection, table, or chart to the report outline |

Ribbon rules:

- Labels sit under icons by default; the ribbon collapses to icons only (Ctrl+F1) for more reading height.
- Commands that cannot run right now stay visible but grey, and the tooltip says why, for example "Select text to code it."
- Users can pin up to four extra commands at the right end; method templates can suggest pins.
- Every ribbon command has a keyboard shortcut and appears in the Ctrl+K command palette.
- Right-click menus and the floating coding bar repeat the same commands, and every route does the same thing.

The side panel changes with the workspace:

| Workspace | Side panel | Main area |
| --- | --- | --- |
| Home | Project overview, recent activity, snapshots, logbook, project details | Project dashboard |
| Data | Sources tree, cases, variables, import queue, preparation tools | Document preview, case grid, or variable editor |
| Code | Sources tree above the code system, with a splitter between | Document reader with margin stripes; retrieved segments below |
| Explore | Query builder, code matrix, crosstabs, co-occurrence, keyword in context, concept map, saved queries | The chosen view with its controls |
| Mixed Methods | Joint displays, quantitize, qualitize, codes by variables, case comparison, integration log, design templates | The chosen integration view, with every figure and quote linked to its source |
| Text Analytics | Corpus, frequencies, keyword in context, dictionaries, topics, classifier | The chosen analysis |
| Statistics | Dataset and variables, then procedures by family: describe, compare groups, correlate, regression, nonparametric, scales | Procedure setup beside results, with an output notebook |
| Synthesize | Memos, summary grid, framework matrix, case profiles, evidence board | The chosen synthesis view |
| Report | Report outline, evidence library, templates | Report editor with preview |
| Team | Coders, coder packets, agreement, adjudication, training | The chosen team view |
| AI | Tasks (code suggestions, summarize, themes, ask your data, writing help, methods help), review queue, model and privacy, AI log | Request, then review with sources and accept, edit, or reject |

In Code, Concept 03 gave the code system its own column. Stacking it under the sources in the side panel leaves more width for the document; users can drag it out into a column if they prefer.

## 6. Start screen and project dashboard

The start screen gets a researcher into work in one click: recent projects as cards, method templates as chips, and a short getting-started list for new users.

*[Figure not copied: start screen mockup · recent projects, method templates, getting started. The mockup images are in the planning package, not in this repository.]*

start screen mockup · recent projects, method templates, getting started

Once a project is open, the Home tab shows a project dashboard in the document area until a document is opened:

- Counts of documents, words, codes, segments, memos, and cases, each clickable.
- Coding progress: share of documents with at least one segment, and documents not yet opened.
- Three starter charts: top codes, codes by document group, and coding activity over the last 30 days.
- The current method phase and its exit checklist, when a method template is in use.
- Recent memos and logbook entries, and any background jobs in progress.

## 7. Coding interactions

Coding should take one gesture and give instant feedback: the stripe appears, the count in the code tree ticks up, and the retrieved-segments pane shows the new segment, all within a tenth of a second.

| Moment | Interaction |
| --- | --- |
| Select text | A small floating coding bar appears above the selection: last code, favorites, highlighter colors, in-vivo, and a search box |
| Apply a code | Drag the selection to a code, drag a code onto the selection, press 1 to 9, or type in the floating bar and press Enter |
| Create while coding | Type a new name in the floating bar and press Shift+Enter; the code lands at the top level or under the selected code |
| Confirm | The stripe animates in (skipped under reduced motion), the code's count updates, and a small toast offers Undo for five seconds |
| Inspect | Hover a stripe for code, weight, coder, and comment; click it to select the segment; double-click to edit its details |
| Adjust | Drag the ends of a selected segment to resize; drop another code on a stripe to replace it; Delete removes the coding |
| Weight and rate | A weight slider and rating chips in the segment details popover, with keyboard arrows |
| Unit mode | The current unit is outlined; number keys code it and Tab moves to the next uncoded unit |
| Media | Drag along the waveform to select a span; the same floating bar appears |
| Images and PDFs | Draw a rectangle or polygon; the floating bar appears at the shape |

Margin stripes follow fixed rules so they stay readable with many codes:

- Stripes sit in a margin column beside the text, ordered by the code tree, with the code name written vertically when space allows.
- More than eight overlapping codes collapse into a "+n" chip that expands on click.
- A margin filter limits stripes to a code set, coder, or origin, and shows which filter is on.

## 8. Code system and codebook editor

The code pane is a working tree for coding; the codebook editor is a full-page view for writing definitions. Both edit the same codes.

### 8.1 Code pane

- Each row: color dot, name, segment count on the right, and a memo icon when the code has a memo; smart codes show a lightning mark and snapshot codes a pin.
- Activation by checkbox or click; active codes are tinted with the accent color.
- Quick filter box at the top of the pane; typing narrows the tree and expands matches.
- Drag to reorder and reparent; drop one code on another with Alt to merge (with a confirmation that states the number of segments).
- Columns can be added (documents, cases, weight average, last coded) for a table view of the tree.

### 8.2 Codebook editor

- Opens as a tab: the tree on the left, the selected code's page on the right.
- Code page fields: name, alias, color, type, definition, inclusion criteria, exclusion criteria, anchor examples (live segments with source), counter-examples, rating scale, code variables, memo.
- A completeness meter shows which codes lack definitions or examples, which is useful in content analysis and teaching.
- Version bar: current codebook version, a button to freeze a new version, and a compare view that shows added, removed, renamed, merged, and split codes between any two versions.
- Print preview of the codebook as it will export to Word.

## 9. Query, matrix, and joint display builders

All three builders share one pattern, so learning one teaches the others: choose items on the left, set conditions in the middle, and see live results on the right, with a plain-language sentence at the top that reads the setup back.

| Builder | Left (ingredients) | Middle (setup) | Right (live result) |
| --- | --- | --- | --- |
| Query builder | Codes, documents, variables, coders | Blocks joined by operators (AND, OR, NOT, near, within, follows), each block a pill that can be dragged and nested | Segment list with counts by document, plus a mini chart |
| Matrix builder | Codes for rows; documents, cases, groups, or variables for columns | Cell content (count, percent, coverage, segments, summaries), normalization, totals | Heat-shaded table; clicking a cell opens its segments in the lower pane |
| Joint display builder | A qualitative strand (codes, summaries) and a quantitative strand (variables, statistics) | Display type (crosstab, segment matrix, typology, side-by-side, bubble plot), grouping variable | The display, with a convergence column the researcher fills in |

Shared behavior:

- Results update as the setup changes, with a short delay on large projects and a visible "updating" state.
- A Save button turns the setup into a named query, smart code, or saved display in the Explore library.
- Send to: Statistics (run the matching test), Report (add to report manager), Map (draw as nodes), Set (save the documents).
- Every table has the same toolbar: sort, transpose, show as chart, export to Excel or Word, copy.

## 10. Statistics and text analytics views

The statistics and text analytics workspaces use the same window, ribbon, and pane rules as coding, so they feel like part of one program rather than add-ons.

*[Figure not copied: statistics view mockup · variables, output notebook, test chooser. The mockup images are in the planning package, not in this repository.]*

statistics view mockup · variables, output notebook, test chooser

Statistics view:

- Left: the variable list with level-of-measurement tags; variables made from codes carry the code's color dot.
- Center: the output notebook, one page per analysis, with APA 7 tables, a results sentence, and a chart; every cell and bar opens the cases and segments behind it.
- Right: the test chooser, which asks what the researcher wants to do, offers only valid tests, and shows assumption checks before running.

Text analytics view (same three-pane pattern):

- Left: corpus settings (scope, language, stop lists, dictionaries) as a stack of collapsible cards.
- Center: tabs for Frequencies, Keyword in context, Co-occurrence, Crosstab, Topics, Entities, and Classifier, each a sortable table with a chart toggle.
- Right: a context panel that shows the passages behind the selected word, category, or topic, with a button to add the word to a dictionary or turn a topic into a code.

## 11. Visual language

PrismQDA's look is calm and professional: neutral chrome, one indigo accent, a twelve-color code spectrum (the "prism"), and type chosen for long reading sessions. All fonts and icons below use licenses compatible with Apache 2.0.

### 11.1 Color tokens

| Token | Light theme | Dark theme | Use |
| --- | --- | --- | --- |
| Canvas | #FFFFFF | #16181D | Document reading area |
| Surface | #F6F7F9 | #1E2128 | Panes, ribbon background |
| Surface raised | #FFFFFF | #262A33 | Popovers, menus, dialogs |
| Border | #E3E6EB | #343947 | Pane dividers, table rules |
| Text primary | #1C1F26 | #E8EAEE | Body text |
| Text secondary | #5B6372 | #A3AAB8 | Labels, counts, captions |
| Accent | #5B45E0 | #A594FF | Selection, focus ring, active tab, primary buttons |
| Accent tint | #F1EEFF | #2C2552 | Active rows, activated codes |
| Brand navy | #151A3D | #151A3D | Title bar, startup screen, welcome brand panel |
| Success, warning, danger | #1F8A4C, #B7791F, #C53030 | #4CC27F, #E3B341, #F06B6B | Status only, never code colors |

### 11.2 Code spectrum

Twelve default code colors, chosen for color-blind separation and 3 to 1 contrast against both themes (to be verified with a contrast checker before release): indigo, sky, teal, green, lime, amber, orange, red, rose, magenta, violet, slate. New top-level codes take the next color in order; subcodes take lighter or darker steps of their parent's color. Users can pick any color, and every code always shows its name or short label next to its color.

A code keeps one color everywhere it appears: the swatch in the code system, the highlight tint in the text, the margin stripe, matrix cells, chart bars, and labels in reports.

### 11.3 Typography

| Role | Font | Size |
| --- | --- | --- |
| Interface | Inter Variable, bundled (Segoe UI Variable, then Segoe UI, as the fallback) | 14 px, with a 12 px step for secondary text |
| Document reading | Source Serif 4, or Atkinson Hyperlegible as an accessibility option | 15 px, 1.55 line height, 72-character measure |
| Data and tables | Inter with tabular numbers | 12.5 px |
| Headings in panes | Inter semibold | 13 px, sentence case |

### 11.4 Icons, shape, and spacing

- PrismQDA's own icon set (`assets/icons`): a 24 px grid with 1.75 px strokes, drawn for every Concept 03 navigation icon; filled icons only for the selected workspace tab.
- Corner radius 6 px on controls and 8 px on cards and popovers; no drop shadows on panes, a soft shadow only on floating popovers.
- 4 px spacing grid; pane padding 12 px comfortable, 8 px compact.
- Motion between 120 and 200 milliseconds, turned off under the system's reduced-motion setting.

### 11.5 Themes

Light, dark, and high-contrast themes, following the Windows setting by default; a sepia reading mode for the document pane only.

## 12. Interaction patterns

A small set of patterns, used the same way everywhere, keeps PrismQDA predictable.

| Pattern | Rule |
| --- | --- |
| Selection then action | Select first, then act from the ribbon, the floating bar, the context menu, or a key; all four do the same thing |
| Drag and drop | Anything that can be dragged shows a grab cursor and a drop preview; dropping onto an invalid target does nothing and says why |
| Context menus | Every item has one, with the most common action first and destructive actions last, in red, with a confirmation |
| Popovers over dialogs | Small edits (rename, color, weight, comment) happen in popovers next to the item; modal dialogs only for imports, exports, and destructive actions |
| Undo everywhere | Every change can be undone; a toast with Undo follows bulk changes such as merges and autocoding |
| Live counts | Counts in trees and tables update as coding happens, never after a refresh |
| Click-through | Any number, cell, bar, node, or point opens its segments in the lower pane |
| Background jobs | Long tasks show in a jobs tray on the status bar with progress, pause, and cancel; the user keeps working |
| Empty states | Every empty pane explains what goes there and offers the one action that fills it |
| Errors | Plain sentences that say what happened and what to do, never codes alone |
| Keyboard | Every command has a shortcut or palette entry; F6 cycles panes; shortcuts are editable and listed in a printable sheet |
| Second monitor | Any pane or tab can float to another screen and remembers where it was |

## 13. Onboarding and help

A new user should code a sample interview and see a chart within ten minutes, without reading a manual.

- First launch asks three questions: role (student, researcher, instructor, team lead), main data type, and method (or "not sure"). The answers set the starting layout, ribbon tabs, and sample project.
- A guided first project overlays short tips on the real interface: import a file, make a code, code a passage, retrieve it, open a chart. Each tip can be skipped, and the tour can be restarted from Help.
- Sample projects for every method template, each with a finished codebook, memos, and a report, open read-only from the start screen.
- A help button on every dialog and pane opens the matching manual page; hovering any ribbon command shows a one-sentence tooltip with its shortcut.
- A searchable glossary of methods terms (code, theme, case, saturation, kappa) written for students.
- "What's new" notes after each update, shown once, with a link to each new feature in the interface.

## 14. Status

The engine foundation and most of the qualitative core are built; the window follows an earlier layout and is being rebuilt to this design. [ROADMAP.md](ROADMAP.md) has the order, and the "As built" section below records what the window does today.

## 15. As built

What the window does today (2026-11-05). Everything below is in `app/ui`, covered by the unit tests and the headless browser check.

**The frame, on every screen once a project is open**

- **Title bar.** The prism and the name, the project name with what mode it codes in, the search box (Ctrl+K), Shortcuts (F1), and Close project.
- **Workspace tabs.** Home, Data, Code, Synthesize, Report. A tab for a workspace that does not exist yet is not shown (decision 48's rule: no dead ends). The current tab uses the filled icon and is marked as the current page.
- **Command ribbon.** One row in the same order everywhere: Project (Undo, Backup), Bring in (Import, Folder, Memo), Code (Code, In vivo, Repeat, New code), Find (Search, Passages), Scope (a chip that says which codes are chosen, and Reset), Output (Export). The row is a single tab stop and the arrow keys move along it. A command that cannot run now stays where it is, is marked unavailable, and says why when pressed or described by a screen reader ("Go to Code and open a source first."). There is no hidden or silently dead button.
- **Status bar.** The spoken status line and the error line, then what is true of the project: every change is saved as it is made, who is coding, and that AI is off.

**The workspaces**

- **Home.** Overview: counts that open the place they count, one suggested next step, coding progress per source, the most used codes, and recent activity. Activity is the full history with the note box.
- **Data.** The sources as a table (kind, folder, units, characters, codings, how much is coded) with Import a file and Import a whole folder, and the cases. Choosing a source opens it in Code.
- **Code.** The side panel holds the sources above the codebook, with a dividing line between them. The middle is the source as a document, a grid, or a thread, with the apply-a-code box and the codings at the caret under it, and the Passages pane below it (see below). The inspector on the right holds Details (the selected coding, the selected code, memos). The codebook header holds only a plus button (Add code) and a More button, so the rare actions (apply to selection, sub-codes, favorites, show passages, show retired codes, reorder, merge, split, retire, save a version) are all there and none of them clutters the panel.
- **Synthesize.** Memos.
- **Report.** Export the codebook (Excel, Word, CSV, QDC) or the coded passages (Excel, Word, CSV); the dialog opens on the one chosen.

**The look (the streamlined pass, 2026-11-05).** One typeface (Inter, bundled, so the window looks the same on every computer) and one scale: 14 px for everything read or pressed, 12 px for counts, captions, and ribbon labels, 15 px semibold for the name of a pane, 18 px for the title of a page, and 15 px with a 1.7 line height for the document being coded. Three weights only: 400 for reading, 500 for controls and labels, 600 for headings and the current item. Chrome is quiet: panes are white and separated by one-pixel lines, buttons have a thin border and a light hover, the workspace tabs and inspector tabs show the current page with an underline and color rather than a filled box, splitters show only a line until hovered or focused, and the codebook rows show a color swatch and the name (the short abbreviation stays in the margin stripes). The coding toolbar is one row: the apply-a-code box, a few quiet quick actions, and favorites when there are any.

**Pictures (alpha 19).** A document that is a picture opens in the Code workspace as the picture itself, in place of the text. A row of tools sits above it: Select (V), Rectangle (B), Ellipse (O), and Outline (P); zoom out, the zoom, zoom in, Fit, 100%, and Turn (a quarter turn clockwise, T). The picture is drawn at a zoom that fits the window and scrolls when it is larger; Ctrl and the wheel zoom it. Coded areas are drawn over it in the code's color with a halo and the code's short label at the corner, so an area is told by its label and not by color alone; the chosen area is drawn heavier and, for a rectangle or ellipse, shows four corner handles. A rectangle or ellipse is dragged out; an outline is built by clicking its corners (a click near the first corner closes it, as does Enter or the Finish outline button) or drawn freehand in one drag; an area is moved by dragging it. A new area is dashed and waits for a code, and the bar of codes (the same one that floats over selected words) appears under it. The picture is a focusable region named by its description; the arrow keys, Space, Alt, Shift, Enter, the square bracket keys, Delete, and Escape do what the pointer does (decision 105), and every change is announced. Under the picture a list of the coded areas gives each a small picture, its codes, and its place in words; below it the picture's description can be rewritten. Segment Details shows a small picture of the chosen area instead of a quotation, Retrieved Segments and the Explore evidence lists show a small picture beside the words that tell the area, and the Import Data page enables Images and explains that the picture is kept inside the project.

**Languages (alpha 20).** Every word the window says goes through a language catalog ([LOCALIZATION.md](LOCALIZATION.md)). Settings > Language lists the translations a person has brought in (a label, how much is translated, how it was made, who made it) with Add a translation, Save a file for a translator, Save a copy, and Remove; it is one fixed choice, English, until one has been added. With one added, the choice offers "Same as Windows" and each language by its label, and a change redraws the open page in the new language without closing the project or leaving the page. The page's `lang` and `dir` follow the language. Numbers, lists, dates, and the order of names follow the language too. A page address ending in `?lang=en-XA` shows the test language, in which every word that is in the catalog is bent and framed in brackets, so a word that is still plain English can be found by eye.

**Settings.** A gear at the right end of the workspace tabs (and Ctrl+K, "Settings"). The settings belong to the person and not to a study: they are kept on this computer, never in a project file, and every project opens with them. **Colors:** follow Windows, light, dark, or high contrast (white on black, a yellow accent, white outlines; Windows' own high contrast mode is followed whatever is chosen). **Text size:** 90, 100 (14 pixels, the standard), 115, 130, or 150 percent; the whole window scales because every size is in rem, and a sample shows the effect. **Screen reader:** whether the caret says which codes it is in as it moves through the text (on by default; until now it could not be turned off). **Panels and settings:** put the panels in Code back to their standard sizes, or every setting back to the standard. The theme and size are put on the page before anything is drawn, so nothing flashes. The panels in Code grow with the text size too (their sizes are kept as they are at the standard size and multiplied), so they hold the same amount of text at any size; on a small window a large size still leaves the document little room, and the panels can be dragged narrower.

**The floating coding bar.** Words selected with the mouse (a drag, a double-click, a triple-click) raise a small bar just above them, or under them when there is no room above. It holds the code used last (when it is not already a favorite), up to six favorites with their key numbers, In vivo, and Other code, which moves to the apply box. Pressing a code does what its key does: the words get the code, stay selected, and keep the focus, so a second code can follow with a click or a key. The bar is a convenience for the mouse and takes nothing away from the keyboard: its buttons are not Tab stops and never take the focus, words selected with the keyboard never raise it, and the same actions are the keys 1 to 9, R, I, and Enter. It goes away when the selection changes some other way, when the text loses the focus, when the caret is placed, or on Escape (the words stay selected). It is not shown in a read-only project. Its names for a screen reader start "Code the selection as ..." so they cannot be mistaken for the toolbar's.

**Passages and the scope.** The lower pane in Code lists every passage coded with the codes you choose, with the source it came from, and follows your coding as you work. You choose codes in three ways: the Choose codes menu in the pane (several at once, with a check mark on each), the "Show passages of this code" check-mark item in the codebook's More menu, or the Passages command. A code that is chosen carries a mark in the codebook and says so to a screen reader. A parent code brings its sub-codes. "Every source" or "The open source" says where to look. Choosing a passage opens its source with those words selected. The scope chip in the ribbon says what is chosen ("2 codes"), takes you to the pane when pressed, and Reset clears it. Choosing a code to look at is kept apart from selecting one to edit. Until a code is chosen the pane is a compact strip (its controls and one line, with no resize bar), so the text above it gets the room; choosing a code gives it its full height and brings the bar back.

**Search and commands (Ctrl+K).** One box finds a command by what it does ("export" finds Export) and finds words in the text of every source. Choosing a passage opens that source in Code with the words selected.

**Undo.** The Undo command and Ctrl+Z take back the newest change and say what it was. Ctrl+Z inside a text box still undoes typing.

**The reading surface (2026-11-06, after the author tried alpha 8).** Coded words look highlighted: each code's color is a see-through tint behind its words (words with several codes show bands of each color), with no dotted or dashed lines; the margin stripes with their short labels stay at the right edge of the text, so codes are still told apart without color, and in Windows high contrast the coded words are underlined. The text fills the middle of the window (Settings can limit it to a comfortable line length), and the lines are numbered at the left as they are drawn. The color of a code is saved the moment it is chosen; there are 22 colors, and the codebook and the floating bar show them as plain dots. The floating bar offers the last code, the favorites, and then codes in codebook order, up to eight. **Segment Details** (the Coding tab of Details): the selected words in a card edged in the code's color, source, lines, and place, the applied codes as chips (a plus to add another, a cross to take one off), the memos about the coding, the note, the code, the anchor example box, and provenance (coded by, how, coded on). **The start screen** is a centered column with the prism above the name.

**Not built yet.** The case a source belongs to and a weight or rating on a coding in Segment Details (they need new data in the project file); the Documents folder tree, the document tabs with "1 of 18", the table of retrieved segments with its columns, and the top bar's project menu, notifications, and account, all of which the mockup shows; the Mixed Methods, Text Analytics, Statistics, and Team tabs, and the AI tab's model that PrismQDA itself calls (its Auto Code and Review Queue buttons work, with rules and no model, and the AI Assistant works by prompt packet: a packet of text to take to a chatbot, an answer pasted back and checked, and a log of every run; Explore was built in alpha 13); the Settings that belong to other workspaces (AI, team, import and export), and Settings on the start screen (before a project is open); the scope reaching every view (today it filters the Passages pane only, not tables or charts, which do not exist yet), choosing several sources, and a rule-based scope; the highlighter colors and a search box on the floating coding bar (it offers the last code, favorites, In vivo, and Other code today); saved layouts and density; the Connect and Assist groups of the ribbon; the welcome screen's cards and the guided first project.
