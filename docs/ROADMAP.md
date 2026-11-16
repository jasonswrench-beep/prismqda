# PrismQDA roadmap

Version 2 · 2026-11-05 · follows [SPECIFICATION.md](SPECIFICATION.md) section 29 and replaces the six-milestone draft of 2026-11-04

This is the order to build PrismQDA in, what each stage delivers, and the test that must pass before the next stage starts. It sets no dates. A milestone ends only when its gate passes.

**The *Where it stands* column below was written on 2026-11-05 and has not been kept up.** [TODO.md](../TODO.md) is the current list, and its section *Not built yet from the 1.0 cut* was checked against the code on 2026-11-10.

## 1. At a glance

| # | Milestone | A researcher can | Gate | Where it stands, 2026-11-05 |
|---|---|---|---|---|
| 1 | Engine foundation | Bring material in and have it kept safely | Engine and crash-safety tests pass | **Built.** Project file, data model, activity log, backups, command-line harness, 14 importers. Missing: reference libraries, images, QDPX |
| 2 | Qualitative core (alpha) | Code a real project from start to finish in a window that is easy to use | A real project coded end to end | **Engine mostly built; the window is being rebuilt** to the interface design (decision 48). Missing: PDF regions, table cells and images as codable data, links, comments, sets |
| 3 | Analysis and mixed methods | Ask questions across the coding and see joint displays | Every count opens its segments and matches hand-checked totals | **Not started** |
| 4 | Statistics and teamwork | Run the core statistics and check agreement with a team | Every procedure and coefficient matches R | **Part built.** Agreement coefficients, samples, reliability runs and reports match R. The data editor makes and changes variables of cases; later steps of it (date parts, sorting, weights, merging, restructuring) and other procedures are not built; adjudication has a layer but no queue |
| 5 | AI, reporting, teaching (beta) | Use AI that proposes and a person approves; write up; teach | The verifier rejects every invented quote; pilot feedback addressed | **Not started.** The review queue, model coders, method templates, and Excel, Word, CSV and QDC exports exist |
| 1.0 | Windows release | Install it, use it, and cite it | Every test layer passes | Not started |

Release 1.5 adds media, text analytics, advanced statistics, and macOS. Release 2.0 adds geodata, the Team Server, plug-ins, and Linux.

## 2. The 1.0 cut (confirmed 2026-11-05)

The author confirmed this cut on 2026-11-05 (decision 44). Where it moves a function to 1.5, the cut governs over the version labels in the specification. The specification puts about everything in sections 5 to 27 into 1.0, and its own review judged that too large: statistics, teamwork, AI, reporting, teaching, a literature workspace, and an accessibility pass all before one release. This is the smallest 1.0 that is still a complete tool for one researcher or a small team, with the rest moved to 1.5 in the order a researcher would ask for it.

**The test for what stays:** can someone take a study from raw material to a documented, publishable result without another program, and trust it? Anything past that moves to 1.5.

| Area | Stays in 1.0 | Moves to 1.5 | Later |
|---|---|---|---|
| Bring in | Text, Word, OpenDocument, RTF, PDF text, transcripts, spreadsheets and surveys, SPSS, social exports, folders (all built); reference libraries; images; QDPX and QDC | Scanned PDFs (OCR), web capture, email, news | Geodata, direct social pulls |
| Prepare | Edit with codings kept in place (built); pseudonymize and redact | Spell check, transcription tools, foot pedals | Translation layer, handwriting |
| Organize | Groups, sets, cases, variables, activation and the global filter | Dynamic (time-varying) variables | |
| Code | Free-span and unit coding, drag, palette, in-vivo, quick code, favorites, highlighter, open coding, comments, weights, segment review table; PDF regions and table cells | Rating scales, mutually exclusive groups, symbol coding | |
| Codebook | Tree, merge, split, retire, definitions, versions with compare and restore, import and export | Creative coding canvas | |
| Memos and links | Typed memos, comments, segment links with named relations, logbook | | |
| Find | Text search, keyword in context, section retrieval, autocoding through the review queue | Query by example, similar-to-code, entities, sentiment | Cluster extraction |
| Query | Visual query builder, smart codes and sets, code matrix, co-occurrence | Code patterns and sequences | |
| Mixed methods | Quantitizing and qualitizing, crosstab, segment matrix, typology table, side-by-side display, design templates | Bubble plots, deviation tables, similarity, triangulation matrix | |
| Visual | Code matrix, bar and treemap, document portrait, codeline, code relations | Concept maps, dendrograms, correspondence analysis, Sankey | |
| Statistics | Data editor, descriptives, crosstabs (chi-square, Fisher, effect sizes), agreement (built), t-tests, one-way ANOVA, correlation, linear regression, Mann-Whitney, Wilcoxon, Kruskal-Wallis, bootstrap; output notebook and APA tables; every one checked against R | Factorial ANOVA, logistic regression, scales, factor and cluster analysis, remaining nonparametric tests, power | Time series, ROC |
| Teamwork | Reliability runs and reports (built), adjudication, coder packets, project merge | Users with passwords and roles, the coder training center | Team Server |
| AI | Prompt packets, the verifier, the review queue, provenance, AI-use disclosure | Own-key routes, the MCP server; **a local model route is not planned while decision 91 stands (decision 166)** | |
| Reporting | Coded segments, codebook, methods appendix, reliability report, statistics output, AI-use disclosure, decision timeline | Report builder, interactive HTML report | |
| Teaching and learning | The built method templates, a guided first project, sample projects, help in every dialog | Teaching mode (instructor packs, coach, class review), more templates | |
| Literature | Library import, metadata as variables, page-aware quotes | Synthesis matrix, screening | Open paper search, sync |
| Privacy | Encryption at rest, offline lock, do-not-share flags, ethics summary | | |
| Accessibility | WCAG 2.2 AA, keyboard-complete, NVDA and Narrator passes, high-contrast and dark themes, 200 percent text | Other screen readers | |
| Release | Signed installer, user guide, validation report, DOI, portable mode | Read-only viewer | |

**Statistics since 2026-11-15 (decision 193).** The Statistics page describes to begin with, and the tests of difference, association, and prediction in the row above are offered only when a person asks (a setting on this computer); they are PrismStatistics's work, and PrismQDA sends it the case table (decision 194). The checked code is kept, and the author confirmed on 2026-11-16 that the tests stay off by default.

**Teaching mode is in 1.5, first in line.** The author teaches methods and it is the strongest reason for an instructor to adopt the tool, which is why it goes first after 1.0. Keeping it out of 1.0 keeps milestone 5 about a third smaller.

## 3. Order and what can run in parallel

```
1 ──► 2 ──► 3 ──► 5 ──► 1.0
       └──► 4 (statistics and teamwork) ──┘
```

- The critical path is 1, 2, 3, 5. Each needs the one before it.
- Statistics and teamwork (4) needs only variables and code counts, which exist at the end of 2, so it can run beside 3.
- **The window comes first inside milestone 2.** Every later workspace hangs off the frame in [INTERFACE.md](INTERFACE.md): the tabs, the command row, the side panel, the inspector, the lower pane, and the status bar. A tool people cannot find their way around does not get used, so the frame is built and tested with people before more is added to it.
- Each milestone builds its engine part first and its screens second, so what is on screen is what was tested.

## 4. Milestones

### 1. Engine foundation (built)

Built and tested: the project file with a published schema, write-ahead logging, dated backups, read-only opening of newer files, the append-only event log, undo, importers for text, Markdown, HTML, Word, OpenDocument, RTF, PDF text, spreadsheets, SPSS, transcripts and captions, and social exports, whole-folder import (500 Word files in about a second), duplicate detection, import reports, and the license rule. The crash test kills the process mid-write six times and the project reopens intact with every coding matched by its event.

To build: reference libraries (RIS and BibTeX), images, and QDPX exchange.

### 2. Qualitative core (alpha)

**The window.** Rebuild to the design in [INTERFACE.md](INTERFACE.md). See that document's "As built" section for what exists.

**Built in the engine:** free-span and whole-unit coding, the codebook (tree, merge, split, retire, numbered versions, anchor examples), memos of four kinds attached to seven things, text editing that moves codings along (with property tests), search (words, phrases, wildcards, regular expressions, AND, OR, NOT, NEAR) with autocoding into a review queue, cases and attributes, six method templates as data, coded-segment retrieval, exports as Excel, Word, CSV and QDC.

**To build:** coding of table cells, PDF regions and images; highlighter pens, open coding, segment weights, comments, links with named relations, the segment review table; groups and sets; variables with a data editor; the logbook. Built since: choosing codes to look at, with the Passages pane and the scope chip (decision 50).

**Gate.** A real project coded end to end; applying or removing a coding under 100 ms; offset-remapping property tests pass; a person can finish a coding task with the keyboard alone and a screen reader finds it usable.

### 3. Analysis and mixed methods

The visual query builder with smart codes and sets; matrices and co-occurrence, every cell opening its segments; summaries, paraphrases, and the evidence board; quantitizing and qualitizing; the joint displays in the 1.0 column above.

**Gate.** Queries and matrices match hand-computed fixtures; every figure on a joint display is computed from the project and shown with its n; charts have text descriptions and carry no meaning by color alone.

### 4. Statistics and teamwork

**Built:** the agreement coefficients (percent agreement, Cohen, Scott, Fleiss, Brennan and Prediger, Gwet, Krippendorff) with bootstrap intervals, checked against R; the random and stratified sample drawer with a saved seed; reliability runs, per-code agreement, confusion tables, reporting labels, and the unit lock until a run is archived.

**To build:** the data editor and the procedures in the 1.0 column, the guided test chooser that offers only valid tests and refuses rows that are not independent, assumption checks, the output notebook with APA tables, and a validation report. For teamwork: blind sessions, coder packets, project merge, adjudication with a disagreement queue and a separate final layer.

**Validation setup, free.** R 4.5.3 is on the author's computer. The free CRAN packages needed for reference values are listed in the notes of the statistics work; install them only with the author's yes. SPSS 24 is also there for a second opinion.

**Gate.** No procedure ships until its R comparison passes. A tolerance table per procedure says where results are looser than 1e-8 and why (some of R's own results are numerical approximations).

### 5. AI, reporting, teaching

Prompt packets with a strict parser that rejects invented unit IDs and codes, and an anchor verifier that rejects quotes not in the source; provenance for every AI run; the preview of exactly what leaves the computer; the methods appendix and AI-use disclosure drafted only from project records; encryption at rest with a separate pseudonymization mapping; sample projects and the guided first project; the accessibility pass and a classroom pilot.

**Decisions due.** The encrypted file format (two candidates: SQLCipher Community Edition and SQLite3 Multiple Ciphers, to check on license and a Windows build with no OpenSSL dependency). How the research-pipeline plugin connects: through prompt packets, through the MCP server, or both.

**Gate.** Parser tests on recorded model answers with invented IDs and misquotes (no live model calls in automated tests); flagged material never appears in a packet or export; nothing is sent with the offline lock on.

### 1.0

A signed installer under 60 MB and signed updates; a DOI for each release; the user guide; the validation report; the accessibility conformance report; a dependency security audit; the performance table on a machine like the reference laptop (Windows 11, 8 GB, 4 cores, SSD). A Windows virtual machine limited to 4 cores and 8 GB stands in for it, because the author's computer is far faster and its performance cores are unstable, so tests run on the efficiency cores.

## 5. Tracks that run the whole way

| Track | What it means |
|---|---|
| Accessibility | WCAG 2.2 AA. Automated checks run on every build (41 screens and dialogs today); manual screen-reader passes at each release |
| Testing | Engine tests through `TEST PRISMQDA.bat`, the interface unit tests, the headless browser check, the window test over its debug port |
| Performance | The targets in section 28.1 of the specification, measured on a generated large project |
| Clean-room | The product is described on its own terms. No other program's source code is read, and no specification text names another QDA program (decision 47) |
| Legal | A trademark search before the first public release; third-party notices kept current |

## 6. Decisions

Closed: the file extension is `.pqda` (provisional until the file format is final); the interface is React with React Aria; PrismQDA is its own project with no tie to any other product line; the artwork is released under the project license (decision 46); the new specification governs (decision 43); the 1.0 cut above, with teaching mode in 1.5 (decision 44).

Open: the encrypted format; the default term for model coders in an agreement run ("intercoder reliability" or "concordance"); the name clearance; macOS timing (1.5 in the specification).

## 7. Costs

Every route has a free option. The places money could come in:

- **Code signing:** free through SignPath Foundation for qualifying open-source projects, which need a public repository, an open-source license, and automated builds. A paid certificate is the alternative.
- **Paid screen readers:** the specification asks for testing with the screen readers people use, and some are paid software. Look for a trial first. Skipping it is a choice to make knowingly.
- **Domain:** prismqda.com, .org, .app, and .io were all unregistered on 2026-11-04. Registering one costs a small yearly fee.
- **Trademark:** searching is free. Filing is not, and is optional.
- **DOI per release:** free through Zenodo.

## 8. Risks

| Risk | What it would do | What to do about it |
|---|---|---|
| The scope is a coding tool, a statistics package, an AI layer, and a teaching tool | 1.0 never arrives | Gate every milestone; use the 1.0 cut; release alphas early |
| An interface people cannot find their way around | Nobody uses it | Build the frame first; test it with people; keep every command in the same place on every screen |
| A custom text surface is hard to make accessible | The headline accessibility promise fails late | Test with a screen reader before building more on it |
| Statistics are wrong in a way nobody notices | Published results are wrong | R comparison before any procedure ships; tolerance table; validation report |
| Importers meet hostile or odd real files | Crashes or lost data | Property-based tests, import reports, never silently skip |
| Name confusion with similarly named research software | Trademark trouble | Search before the first public release; always write "PrismQDA" in full |
| One person building and maintaining all of it | A bus factor of one | Keep the engine well tested and documented; keep gates small |

**If time runs short, cut in this order.** First: tagged PDF output, the concept canvas, social profiles beyond the generic one. Next: extra method templates. Last, and only with a plain note to users: encryption (it can follow 1.0). **Must stay for 1.0:** hand coding with a full codebook, honest reliability, QDPX exchange, the methods appendix, and accessibility.

## 9. Next steps

1. Rebuild the window to [INTERFACE.md](INTERFACE.md), screen by screen, keeping the engine untouched and every check passing.
2. Try the new window with the author, then with two or three other researchers, before adding more to it.
3. Finish milestone 2's missing coding features inside the new frame.
