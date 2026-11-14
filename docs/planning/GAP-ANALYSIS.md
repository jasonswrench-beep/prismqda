# Planning package against what is built

> Historical record, 2026-11-05. It was written before the author decided which documents govern (decision 43), so where it says "repository specification" it means the retired earlier one, and "new specification" is now [../SPECIFICATION.md](../SPECIFICATION.md). The interface document is now [../INTERFACE.md](../INTERFACE.md) and the roadmap was rewritten from section 2 of it ([../ROADMAP.md](../ROADMAP.md)). Sections 6 to 8 say what was settled.

Written 2026-11-05, after the planning package arrived (five zip files). It compares the package's specification, roadmap, and interface direction with the repository as it stands. The comparison is by section and feature, not line by line: nobody has yet read the new specification against the old one sentence by sentence, and the review in the package itself says that merge needs a careful pass.

## 1. What arrived

| File | What it holds | Used here |
| --- | --- | --- |
| PrismQDA_Planning_Package.zip | Six documents (current specification, missing tools and interface design, icon list, a review of an outside specification, the earlier specification marked superseded, a background scan) and three reference images | Documents 1 and 2 became `docs/SPECIFICATION.md` and `docs/INTERFACE.md`. The icon list came again inside the icon packs |
| PrismQDA_Batch_1_Complete_v1.0.zip | The final logo and app icon, 93 outline icons, 12 selected-tab icons, registry, standards, review gallery | Brought into the repository (section 5) |
| PrismQDA_Main_Logo_SVG_Pack.zip | The logo files alone | Identical to the brand folder inside Batch 1 (same hashes), so nothing extra |
| PrismQDA_Icon_Foundation_Pack_v0.1.zip | The first 36 icons and the earlier "Aperture" logo candidates | Superseded: Batch 1 keeps all 36 icons unchanged and replaces the logo |
| Prism.zip | Eight Concept 03 screen images, five mockup boards, an HTML screen guide, and copies of the other four zips | Nothing new beyond the Concept 03 screens. The images are not in the repository |

Not imported on purpose: the superseded specification (it is what `docs/SPECIFICATION.md` was made from), and documents 4 and 6, which review outside material and are not needed to build (the repository describes the product on its own terms).

## 2. What changed in scope

| | Repository specification (`docs/SPECIFICATION.md`) | Current specification (`PrismQDA-Specification.md`) |
| --- | --- | --- |
| Size | 23 sections, about 8,300 words | 30 sections, about 12,400 words |
| Workspaces | Not defined | Eight workspaces in the text, eleven tabs plus Settings in the interface document |
| Images (region coding) | 2.0 | 1.0, and PDF regions and table cells are 1.0 too |
| Reference libraries (RIS, BibTeX) with PDFs | 2.0 | 1.0 |
| Scanned PDF (OCR) | 2.0 | 1.5 |
| Audio and video | 2.0 (offline transcription, synced playback) | 1.5, coded directly on a timeline |
| Keyword in context | 2.0 | 1.0 (the interface document says it moved up); the text analytics module as a whole is 1.5 |
| Mixed methods, joint displays, summaries and synthesis | Light | Whole sections (11, 14), milestone 3 |
| Statistics | Core tests | Full package; 1.0 is about two dozen procedures |
| Method templates | Five, plus free mode | Ten, plus free mode |
| New areas | Absent | Literature review workspace, survey grid, web capture, geodata, scripting and local API, plug-ins, Team Server (sections 20 to 22 and 25) |
| macOS | Windows only | 1.5 |
| Milestones | Six, my reconstruction in `docs/ROADMAP.md` | Five, drawn in the specification (`images/spec-roadmap.png`) |

The review document in the package judges 1.0 as too large (202 of 267 requirements in an outside specification) and suggests a realistic 1.0 of 40 to 60 requirements. The same worry applies to this specification: its roadmap puts statistics, teamwork, AI, reporting, teaching, a literature review workspace, and an accessibility pass all before 1.0.

## 3. Built against the five milestones

"Built" means in the engine and tested. "Screen" means the interface has it. The engine is complete for more than the interface shows.

| Milestone in the new roadmap | Built | Partly | Not started |
| --- | --- | --- | --- |
| **1. Engine foundation** (gate: engine and crash-safety tests pass) | Project file, schema, activity log, backups, command-line harness; importers for text, Word, OpenDocument, RTF, HTML, Markdown, PDF text, spreadsheets, SPSS, transcripts and captions, social exports, whole folders. Crash test and end-to-end test pass | Scanned PDFs are not read (no OCR). The REFI codebook (QDC) is exported but not imported | Reference libraries (RIS, BibTeX), images, QDPX |
| **2. Qualitative core** (gate: a real project coded end to end) | Free-span and whole-unit coding, codebook tree with merge, split, retire, versions, memos (four kinds, seven attachment points), search language with autocoding into a review queue, undo, text edits that carry codings, cases and attributes, six method templates as data. Screens for all of these | Groups (folders only, no sets), variables (attributes only, no data editor), codebook version comparison and restore (versions are saved and listed) | PDF region, table cell, and image coding; highlighter pens; segment weights and ratings; segment links; comments; activation and the global filter bar; the four-pane Concept 03 workspace |
| **3. Analysis and mixed methods** (gate: every count opens its segments and matches hand-checked totals) | Coded-segment export by code and coder (Excel, Word, CSV) | The search language is a text query, not the visual query builder | Query builder, smart codes and sets, matrices, visual tools, concept maps, summaries, paraphrases, joint displays |
| **4. Statistics and teamwork** (gate: every procedure and coefficient matches R) | Agreement coefficients (percent, Cohen, Scott, Fleiss, Brennan and Prediger, Gwet, Krippendorff) with bootstrap intervals, checked against R. Coders (human and model), samples, reliability runs and reports, unit lock | Adjudication (a final layer exists, no queue or screens). No screens for reliability | Data editor, descriptive and inferential procedures, output notebook, APA output; users, roles, merging, coder packets, training center |
| **5. AI, reporting, teaching** (gate: verifier rejects every invented quote) | Method templates and phase checklists; model coders and the review queue are the groundwork for AI | Accessibility: keyboard use and high-contrast, reduced-motion, and 200 percent text checks in a headless browser, no screen-reader pass yet | AI routes and verifier, MCP server, methods appendix, literature review workspace, teaching mode, classroom pilot |
| **Release 1.0** | Unsigned installer, published as pre-releases | | Signed installer, read-only viewer, user guide, validation report, DOI, sample projects |

## 4. The interface

`app/ui` was built before the interface document. Concept 03 asks for:

- a navy title bar with the prism mark, project name, current workspace, and a Ctrl+K command palette;
- workspace tabs (Home, Data, Code, Explore, Mixed Methods, Text Analytics, Statistics, Synthesize, Report, Team, AI, and Settings as a gear);
- one slim command ribbon that never changes between workspaces;
- a left side panel that changes with the workspace, a right inspector, a lower pane for retrieved segments, and a status bar with save state, active scope, coder layer, and AI mode.

Today the window has a header with three areas (Coding, Memos, History), a Sources pane, a document or grid in the middle, a Codebook pane and Details panel on the right, and no ribbon, lower pane, or status bar. Coding, the codebook, memos, history, import, export, and help are all there as working screens, so the redesign is a reshaping of what exists plus the screens that do not exist yet, not a rewrite of the engine calls.

Rules in the interface document that the interface must follow, because the mockups break them:

- Every count names its unit (codings, segments, documents, or cases). A statistic must refuse rows that are not independent, for example a chi-square on codings from 45 participants, and offer to count each participant once.
- Show the benchmark the researcher chose for agreement, with its citation, and no verdict word.
- Report p = .053 as not statistically significant, with the effect size and its interval, never as "marginal".
- Every figure on a joint display is computed from the project and shown with its n. The mockup's percentages do not add up.
- Changes save at once; scheduled backups stay. No "auto-save every 5 minutes" setting.
- No registered-trademark sign after the name until the name is cleared.

## 5. Logo and icons

Brought into the repository today:

- `assets/brand/`: the selected prism mark (full and small), wordmark in dark and light versions, square app icon, and `app-icon.ico` with seven sizes.
- `assets/icons/regular/` (93) and `assets/icons/active/` (12): all of Batch 1, which is what the interface needs.
- `design/icons/`: the icon standard, the decision log, the integration guide, the review checklist, the registry of all 244 planned icons, an offline review gallery, and the type provenance notice.
- The app icon (`app/src-tauri/icons/`) is now the prism. The earlier generated placeholder is gone.
- In the interface: the prism and icons in the header and start screen. `app/ui/scripts/build-icons.ts` turns the SVG files into a TypeScript module with a strict parser, so an icon with a script, link, image, transform, or stray color stops the build.

Status the pack itself reports, which still holds:

- The logo direction is approved. Every icon is a draft that has not been reviewed. The review checklist lists ten questions to answer at actual size, among them Apply code against New code, the three matrix tools, and Accept against Reject without color.
- The pack asks the owner to confirm the release license for the artwork and notes that the mark has had no trademark clearance. The wordmark letters are outlines from the Inter typeface (open font license, recorded in `design/icons/THIRD_PARTY_NOTICES.txt`).
- 136 of the 244 planned icons were not drawn when this was written: 122 in Batch 2 (release 1.0) and 14 in Batch 3. Batch 2 arrived the same evening and is in (decision 49), and Batch 3 followed (decision 51), so all 244 are drawn.

## 6. Where the new specification disagreed with decisions already made

| Topic | New text | Settled |
| --- | --- | --- |
| Other product lines | Sections 3, 25, and 30 mentioned the author's other tools (stack, a shared engine library, a relationship question) | Removed (decision 45): PrismQDA has no tie to another product line |
| Project file extension | "for example .prism" | `.pqda`, provisional (decision 2) |
| Platforms | Windows only until 1.5 | Same. Whether macOS belongs in 1.0 is open |
| Clean-room rule | Section 3.3 states it | Same, and enforced by the license check |
| Offsets and positions | Coded positions tied to a version of the text, uncertain ones to a review queue (a review idea) | Open. Offsets are code points of NFC text; an edit moves the codings it can and removes, and logs, those it cannot. A review queue for uncertain ones is not built |

## 7. Section crosswalk, old to new

The code comments, the published schema's comments, and the documents cited the earlier specification's section numbers: about 110 references in 36 files. They were remapped to the new numbers on 2026-11-05 (decision 43), using this mapping, section by section:

| Old | Topic | New |
| --- | --- | --- |
| 2 | Name, license, clean-room | 3.3 |
| 3 | Architecture | 3.2 |
| 4 | Project file and data model | 4 |
| 5 | Data import | 5, 6 |
| 6 | Method templates | 24.1 |
| 7 | Coding workspace and codebook | 8, 9 |
| 8 | Cases, attributes, search, queries | 7.2, 7.3, 12, 13 |
| 9 | Coders, reliability, adjudication | 18.3, 18.4 |
| 10 | AI | 19 |
| 11 | Statistics | 17 |
| 12 | Text analysis | 16 |
| 13 | Visualizations | 15 |
| 14 | Audit trail and reporting | 10.4, 23.1 |
| 15 | Teaching mode | 24.3 |
| 16 | Collaboration without a server | 18.2 |
| 17 | Export and interoperability | 23.2 |
| 18 | Accessibility | 26.1 |
| 19 | Privacy, security, ethics | 27 |
| 20 | Performance | 28.1 |
| 21 | Testing | 28.2 |
| 22 | Release plan | 29 |
| 23 | Open questions | 30 |

## 8. Decisions, 2026-11-05

1. **Which specification governs:** the new one. Done (decision 43).
2. **The 1.0 cut:** the author asked for a recommendation. [../ROADMAP.md](../ROADMAP.md) section 2 proposed one, and the author confirmed it with teaching mode in 1.5 (decision 44).
3. **Interface:** rebuild it now, because it is the backbone and must be intuitive (decision 48).
4. **Icon review:** the ten checklist questions still need a person at actual size. Until answered, the icons are drafts.
5. **Artwork license:** the artwork was created with an AI image tool and is released under the project license (decision 46). A trademark search for the name is still open.
6. **Specification wording:** no other product line (decision 45) and no other QDA program named (decision 47).
