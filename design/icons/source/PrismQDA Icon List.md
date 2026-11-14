# PrismQDA Icon List

Nov 5, 2026 · @Jason Wrench

PrismQDA needs 232 icons in 18 groups, delivered in three batches. Batch 1 covers the screens already prototyped (workspace tabs, command ribbon, coding, AI review, and core status and controls). Batch 2 covers the rest of release 1.0, and Batch 3 covers release 1.5 and later.

## Style brief

Every icon is a single-color outline drawing on a 24 by 24 grid, so the whole set reads as one family at 16, 20, and 24 pixels.

| Rule | Specification |
| --- | --- |
| Grid | 24 x 24 viewBox, 2 px padding (20 x 20 live area) |
| Stroke | 1.75 px, round caps, round joins, no fill |
| Color | stroke="currentColor" only; the app supplies color, including status colors |
| Corners | 2 px radius on rectangles; circles and arcs drawn as paths |
| Detail | Must read at 16 px: no more than 3 or 4 strokes of detail, no letters or numbers unless a row says so |
| Active variant | Workspace tab icons also need a filled version for the selected tab |
| Family motif | A subtle prism facet (a 60 degree angle or triangle notch) on brand, workspace, and AI icons only, never on every icon |
| Direction | Draw one chevron or arrow; left, up, and down come from rotation in code |
| Code colors | Never inside icons; code colors appear only in swatches, highlights, and stripes |
| Originality | New drawings, not traced or copied from other research software or icon sets. If an icon is adapted from Lucide (ISC license), mark it so it can be credited |
| File format | Hand-written SVG code, one file per icon, paths only, no embedded images, metadata, or transforms |
| Naming | Lowercase file names exactly as listed (group prefix, then name), for example ws-home.svg and ws-home-active.svg |

Prompt to give Chat with each batch:

> Draw the following icons for a desktop research application called PrismQDA as hand-written SVG code, one code block per icon, using the style rules below. Keep the set consistent: same stroke weight, corner radius, and level of detail. Do not copy icons from other software. \[paste the style rules table and the rows for this batch\]

Ask for SVG code rather than images. Image generators produce pictures of icons, not usable vector files.

## Brand, workspaces, and command ribbon

### Brand and app (5)

These are the only full-color pieces in the set: the prism with its spectrum, on navy #151A3D where a background is needed.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Logo mark | brand-mark.svg | Faceted prism splitting light into a spectrum; works alone at 16 px | 1 |
| Full logo | brand-logo.svg | Logo mark plus the PrismQDA wordmark (letters allowed); no ® | 1 |
| App icon | app-icon.svg (plus .ico at 16, 24, 32, 48, 64, 128, 256) | Logo mark on a navy rounded square | 1 |
| Project file | file-project.svg | Document sheet with a small prism in the corner | 2 |
| Exchange package | file-package.svg | Box or bundle with a small prism, for archive and exchange exports | 2 |

### Workspace tabs (12, each with an active variant)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Home | ws-home.svg | House | 1 |
| Data | ws-data.svg | Stacked database cylinder | 1 |
| Code | ws-code.svg | Tag | 1 |
| Explore | ws-explore.svg | Magnifying glass | 1 |
| Mixed Methods | ws-mixed.svg | Two overlapping circles | 1 |
| Text Analytics | ws-text.svg | Lines of text of varying length | 1 |
| Statistics | ws-stats.svg | Bar chart on a baseline | 1 |
| Synthesize | ws-synth.svg | Stacked layers | 1 |
| Report | ws-report.svg | Document with text lines | 1 |
| Team | ws-team.svg | Two people | 1 |
| AI | ws-ai.svg | Sparkle (a four-point star with a small companion) | 1 |
| Settings | ws-settings.svg | Gear | 1 |

### Command ribbon (17)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Snapshot | cmd-snapshot.svg | Camera | 1 |
| Undo | cmd-undo.svg | Arrow curving back to the left | 1 |
| Redo | cmd-redo.svg | Arrow curving forward to the right | 1 |
| Import | cmd-import.svg | Arrow down into a tray | 1 |
| New memo | cmd-memo.svg | Sticky note with a folded corner | 1 |
| Code (apply) | cmd-code.svg | Tag with a small check or plus | 1 |
| In vivo | cmd-invivo.svg | Quotation marks | 1 |
| Quick code | cmd-quick-code.svg | Lightning bolt | 1 |
| New code | cmd-new-code.svg | Tag with a plus | 1 |
| Link | cmd-link.svg | Two chain links | 1 |
| Search | cmd-search.svg | Magnifying glass (may reuse ws-explore) | 1 |
| Retrieve | cmd-retrieve.svg | List of rows with bullet dots | 1 |
| Scope | cmd-scope.svg | Funnel | 1 |
| Reset scope | cmd-reset-scope.svg | Funnel with a small x | 1 |
| AI assist | cmd-ai-assist.svg | Sparkle (may reuse ws-ai) | 1 |
| Add to report | cmd-add-to-report.svg | Document with a plus | 1 |
| Export | cmd-export.svg | Arrow up out of a tray | 1 |

## Sources and code system

### Source types (20)

Source icons share one document-sheet outline with a small distinguishing mark, so they read as a set in the sources tree.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Folder | src-folder.svg | Folder | 1 |
| Text document | src-text.svg | Sheet with text lines | 1 |
| PDF | src-pdf.svg | Sheet with a page-corner fold and a small frame | 1 |
| Image | src-image.svg | Frame with a mountain and sun | 1 |
| Audio | src-audio.svg | Sound wave or speaker | 1 |
| Video | src-video.svg | Film frame with a play triangle | 1 |
| Table | src-table.svg | Grid of cells | 1 |
| Survey | src-survey.svg | Clipboard with check boxes | 1 |
| Transcript | src-transcript.svg | Sheet with a speech bubble | 2 |
| Focus group | src-focus-group.svg | Three people around a speech bubble | 2 |
| Reference | src-reference.svg | Book or bookmark | 2 |
| Web page | src-web.svg | Browser window or globe | 2 |
| Email | src-email.svg | Envelope | 2 |
| Field note | src-field-note.svg | Notebook with a pencil | 2 |
| Case | src-case.svg | Single person in a frame (a participant, site, or organization) | 2 |
| Document set | src-set.svg | Two stacked sheets | 2 |
| Linked file | src-linked.svg | Sheet with a small chain link | 2 |
| Missing file | src-missing.svg | Sheet with a dashed outline | 2 |
| Social media post | src-social.svg | Speech bubble with a short thread line | 3 |
| Geodata | src-geo.svg | Map pin | 3 |

### Code system (14)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Code | code-code.svg | Tag | 1 |
| Parent code | code-parent.svg | Tag with two small branches below | 1 |
| In vivo code | code-invivo.svg | Tag with quotation marks | 1 |
| AI-suggested marker | code-ai-suggested.svg | Small sparkle badge, used beside codes accepted from AI | 1 |
| Activated | code-activated.svg | Small filled dot or check, used beside activated codes and sources | 1 |
| Code set | code-set.svg | Two overlapping tags | 2 |
| Smart code | code-smart.svg | Tag with a funnel or gear (built from a query) | 2 |
| Weighted code | code-weighted.svg | Tag with a small scale or bar | 2 |
| Mutually exclusive group | code-exclusive.svg | Two tags with a divider between | 2 |
| Favorite | code-favorite.svg | Star | 2 |
| Highlighter | code-highlighter.svg | Highlighter pen | 2 |
| Inactive code | code-inactive.svg | Tag with a dashed outline | 2 |
| Merge codes | code-merge.svg | Two arrows joining into one | 2 |
| Split code | code-split.svg | One arrow dividing into two | 2 |

## Memos, Explore, and Mixed Methods

### Memos and links (12)

Memo types share the sticky-note outline with a small inner mark.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| General memo | memo-general.svg | Sticky note with a folded corner | 1 |
| Code memo | memo-code.svg | Sticky note with a small tag | 2 |
| Document memo | memo-document.svg | Sticky note with a small sheet | 2 |
| Theory memo | memo-theory.svg | Sticky note with a lightbulb | 2 |
| Method memo | memo-method.svg | Sticky note with a small compass or ruler | 2 |
| Literature memo | memo-literature.svg | Sticky note with a small book | 2 |
| Question memo | memo-question.svg | Sticky note with a question mark (mark allowed) | 2 |
| Logbook entry | memo-logbook.svg | Bound journal with a date tab | 2 |
| Comment | memo-comment.svg | Speech bubble | 2 |
| Paraphrase | memo-paraphrase.svg | Two lines with a curved arrow between (restating) | 2 |
| Summary | memo-summary.svg | Three lines condensing into one | 2 |
| Link marker | link-marker.svg | Small chain link badge shown in the text margin | 2 |

### Explore (13)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Query builder | exp-query.svg | Two connected blocks (logic pieces) | 1 |
| Code matrix | exp-code-matrix.svg | Grid with some cells filled | 1 |
| Crosstab | exp-crosstab.svg | Table with a highlighted header row and column | 1 |
| Co-occurrence | exp-cooccurrence.svg | Two overlapping squares | 1 |
| Keyword in context | exp-kwic.svg | Three text lines with one word aligned in the center | 1 |
| Concept map | exp-concept-map.svg | Three nodes joined by lines | 2 |
| Document portrait | exp-portrait.svg | Sheet filled with small colored-block pattern (drawn as outlines) | 2 |
| Codeline | exp-codeline.svg | Horizontal timeline with segments | 2 |
| Code relations | exp-relations.svg | Two tags joined by a line | 2 |
| Trends | exp-trends.svg | Rising line chart | 2 |
| Saved query | exp-saved-query.svg | Magnifying glass with a bookmark | 2 |
| Random sample | exp-sample.svg | Die or scattered dots | 2 |
| Similarity | exp-similarity.svg | Two sheets with an approximately-equal sign | 3 |

### Mixed Methods (7)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Joint display | mix-joint-display.svg | Split panel: a bar on one side, a quote mark on the other | 1 |
| Quantitize | mix-quantitize.svg | Tag with an arrow pointing to a number column | 2 |
| Qualitize | mix-qualitize.svg | Bar chart with an arrow pointing to a tag | 2 |
| Case comparison | mix-case-compare.svg | Two people side by side with a divider | 2 |
| Typology | mix-typology.svg | 2 by 2 grid with a dot in one quadrant | 2 |
| Integration log | mix-integration-log.svg | Notebook with two merging lines | 2 |
| Design template | mix-design-template.svg | Flow of three boxes with arrows | 2 |

## Text Analytics, Statistics, Synthesize, and Report

### Text Analytics (11)

Keyword in context reuses exp-kwic.svg.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Corpus | txt-corpus.svg | Stack of sheets with text lines | 2 |
| Frequencies | txt-frequencies.svg | Horizontal bars of decreasing length | 2 |
| N-grams | txt-ngrams.svg | Two or three linked word blocks | 2 |
| Dictionary | txt-dictionary.svg | Book with an index tab | 2 |
| Stop list | txt-stoplist.svg | Text line with a strike-through | 2 |
| Word cloud | txt-word-cloud.svg | Cloud outline with lines of mixed length inside | 2 |
| Topics | txt-topics.svg | Three clusters of dots | 3 |
| Classifier | txt-classifier.svg | Sheet with an arrow pointing into labeled bins | 3 |
| Sentiment | txt-sentiment.svg | Plus and minus scale or slider | 3 |
| Named entities | txt-entities.svg | Word inside a rounded box with a small pin | 3 |
| Keyness | txt-keyness.svg | Two columns with one word raised | 3 |

### Statistics (16)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Data editor | sta-data-editor.svg | Spreadsheet grid with a pencil | 2 |
| Variable | sta-variable.svg | Column header with a small ruler | 2 |
| Transform | sta-transform.svg | Grid with a circular arrow | 2 |
| Describe | sta-describe.svg | Bell curve | 2 |
| Compare groups | sta-compare.svg | Two bars with a bracket above | 2 |
| Correlate | sta-correlate.svg | Scatter of dots along a line | 2 |
| Regression | sta-regression.svg | Scatter with a fitted line | 2 |
| Nonparametric | sta-nonparametric.svg | Ranked steps (1, 2, 3 bars as a staircase) | 2 |
| Bootstrap | sta-bootstrap.svg | Circular arrows around dots | 2 |
| Test chooser | sta-test-chooser.svg | Branching decision tree | 2 |
| Assumptions check | sta-assumptions.svg | Bell curve with a check mark | 2 |
| Output notebook | sta-output.svg | Notebook with a small chart | 2 |
| Chart | sta-chart.svg | Simple bar chart in a frame | 2 |
| Scales and reliability | sta-reliability.svg | Three aligned items with a bracket | 3 |
| Factor analysis | sta-factor.svg | Many dots grouped under two arrows | 3 |
| Cluster analysis | sta-cluster.svg | Two circles of dots | 3 |

### Synthesize (4)

The Memos view reuses memo-general.svg.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Summary grid | syn-summary-grid.svg | Grid with text lines in each cell | 2 |
| Framework matrix | syn-framework.svg | Table with a highlighted first column | 2 |
| Case profile | syn-case-profile.svg | Person beside text lines | 2 |
| Evidence board | syn-evidence-board.svg | Board with pinned cards | 2 |

### Report (9)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Report outline | rep-outline.svg | Numbered list with indents (numbers allowed) | 2 |
| Evidence library | rep-evidence.svg | Bookshelf or stacked cards with a quote mark | 2 |
| Template | rep-template.svg | Sheet with dashed blocks | 2 |
| Quotation | rep-quote.svg | Large quotation marks | 2 |
| Table | rep-table.svg | Small table | 2 |
| Figure | rep-figure.svg | Frame with a small chart | 2 |
| Methods appendix | rep-methods.svg | Sheet with a paperclip | 2 |
| AI disclosure | rep-ai-disclosure.svg | Sheet with a small sparkle | 2 |
| Preview | rep-preview.svg | Eye over a sheet | 2 |

## Team, AI, and media

### Team (9)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Coder | team-coder.svg | Person with a small tag | 2 |
| Roles | team-roles.svg | Person with a small badge | 2 |
| Coder packet | team-packet.svg | Parcel or envelope with a tag | 2 |
| Merge work | team-merge.svg | Two lines joining into one | 2 |
| Agreement | team-agreement.svg | Two overlapping check marks | 2 |
| Adjudicate | team-adjudicate.svg | Gavel or balance scale | 2 |
| Blind coding | team-blind.svg | Eye with a slash | 2 |
| Training | team-training.svg | Graduation cap | 2 |
| Answer key | team-answer-key.svg | Key | 2 |

### AI (14)

AI icons may carry the small prism facet. Accept and reject must differ in shape, not only color.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Code suggestions | ai-suggest-codes.svg | Tag with a sparkle | 1 |
| Review queue | ai-review-queue.svg | Stack of cards with a sparkle | 1 |
| Local model | ai-local.svg | Desktop computer with a sparkle on the screen | 1 |
| Cloud model | ai-cloud.svg | Cloud with a sparkle | 1 |
| Payload preview | ai-payload.svg | Eye over an outgoing arrow (what will be sent) | 1 |
| Accept proposal | ai-accept.svg | Check mark in a circle | 1 |
| Reject proposal | ai-reject.svg | X in a circle | 1 |
| Summarize | ai-summarize.svg | Lines condensing, with a sparkle | 2 |
| Find themes | ai-themes.svg | Clustered dots with a sparkle | 2 |
| Ask your data | ai-ask.svg | Speech bubble with a sparkle | 2 |
| Writing help | ai-writing.svg | Pen with a sparkle | 2 |
| Methods help | ai-methods.svg | Book with a sparkle | 2 |
| Model | ai-model.svg | Chip or processor outline | 2 |
| AI log | ai-log.svg | List with a small sparkle and a clock | 2 |

### Media playback and regions (11)

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Play | med-play.svg | Triangle | 2 |
| Pause | med-pause.svg | Two bars | 2 |
| Skip back | med-back.svg | Circular arrow back (no number; the app adds the seconds label) | 2 |
| Skip forward | med-forward.svg | Circular arrow forward | 2 |
| Playback speed | med-speed.svg | Gauge | 2 |
| Insert timestamp | med-timestamp.svg | Clock with a plus | 2 |
| Waveform | med-waveform.svg | Vertical bars of varied height | 2 |
| Speaker | med-speaker.svg | Person with a speech bubble | 2 |
| Rectangle region | med-region-rect.svg | Dashed rectangle with corner handles | 2 |
| Polygon region | med-region-polygon.svg | Dashed polygon with point handles | 2 |
| Foot pedal | med-foot-pedal.svg | Angled pedal | 3 |

## Settings, status, and common controls

### Settings sections (11)

The AI and Team sections reuse ws-ai.svg and ws-team.svg.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| General | set-general.svg | Sliders | 2 |
| Appearance | set-appearance.svg | Half-filled circle (light and dark) | 2 |
| Data and storage | set-storage.svg | Hard drive | 2 |
| Backups | set-backup.svg | Clock with a circular arrow | 2 |
| Import and export | set-import-export.svg | Two opposing arrows | 2 |
| Privacy and security | set-privacy.svg | Shield | 2 |
| Accessibility | set-accessibility.svg | Person with outstretched arms in a circle | 2 |
| Language | set-language.svg | Globe with a speech bubble | 2 |
| Keyboard shortcuts | set-shortcuts.svg | Keyboard | 2 |
| Updates | set-updates.svg | Circular arrow with a down arrow | 2 |
| About | set-about.svg | Letter i in a circle (letter allowed) | 2 |

### Status indicators (15)

Used in the status bar, job list, and on results. Each status differs in shape, so none depends on color.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Saved | st-saved.svg | Check mark inside a small disk outline (saved on this computer) | 1 |
| Saving | st-saving.svg | Partial circle (spinner frame) | 1 |
| Save failed | st-save-failed.svg | Disk outline with an exclamation mark | 1 |
| Background job | st-job.svg | Stacked bars with a small clock | 1 |
| Information | st-info.svg | Circle with an i | 1 |
| Success | st-success.svg | Circle with a check | 1 |
| Warning | st-warning.svg | Triangle with an exclamation mark | 1 |
| Error | st-error.svg | Octagon with an x | 1 |
| Stale result | st-stale.svg | Circular arrow with a small dot (needs refresh) | 2 |
| Frozen snapshot | st-frozen.svg | Snowflake or pinned sheet | 2 |
| Restricted | st-restricted.svg | Padlock | 2 |
| Encrypted | st-encrypted.svg | Padlock with a key notch | 2 |
| Unit check | st-unit-check.svg | Person outline with a ruler (unit of analysis flagged) | 2 |
| Synchronized | st-sync.svg | Two circular arrows | 3 |
| Offline | st-offline.svg | Plug or link with a slash | 3 |

### Common controls (32)

These are standard interface glyphs. Drawing them fresh keeps the set consistent, but Lucide equivalents (ISC license) are an acceptable shortcut here.

| Icon | File name | What it shows | Batch |
| --- | --- | --- | --- |
| Chevron down | ui-chevron-down.svg | Chevron (rotated in code for up) | 1 |
| Chevron right | ui-chevron-right.svg | Chevron (rotated in code for left) | 1 |
| Close | ui-close.svg | X | 1 |
| More | ui-more.svg | Three horizontal dots | 1 |
| Add | ui-plus.svg | Plus | 1 |
| Remove | ui-minus.svg | Minus | 1 |
| Check | ui-check.svg | Check mark | 1 |
| Drag handle | ui-drag.svg | Six dots in two columns | 1 |
| Collapse pane | ui-pane-collapse.svg | Panel with an inward arrow | 1 |
| Float pane | ui-pane-float.svg | Window with an outward arrow | 1 |
| Pin | ui-pin.svg | Pushpin | 1 |
| Filter | ui-filter.svg | Funnel (may reuse cmd-scope) | 1 |
| Sort | ui-sort.svg | Up and down arrows | 1 |
| Help | ui-help.svg | Question mark in a circle | 1 |
| Command palette | ui-command.svg | Keyboard key with a caret | 1 |
| Show or hide | ui-eye.svg | Eye (eye with a slash comes from team-blind) | 1 |
| Copy | ui-copy.svg | Two overlapping sheets | 1 |
| Paste | ui-paste.svg | Clipboard | 1 |
| Delete | ui-delete.svg | Trash can | 1 |
| Edit or rename | ui-edit.svg | Pencil | 1 |
| Duplicate | ui-duplicate.svg | Two sheets with a plus | 1 |
| Restore | ui-restore.svg | Circular arrow back around a clock | 1 |
| Open externally | ui-external.svg | Box with an arrow leaving the corner | 1 |
| Refresh | ui-refresh.svg | Circular arrow | 1 |
| Zoom in | ui-zoom-in.svg | Magnifier with a plus | 1 |
| Zoom out | ui-zoom-out.svg | Magnifier with a minus | 1 |
| Fit to window | ui-fit.svg | Four corner brackets | 1 |
| Full screen | ui-fullscreen.svg | Four outward arrows | 1 |
| Collapse ribbon | ui-ribbon-toggle.svg | Chevron in a thin bar | 1 |
| Calendar | ui-calendar.svg | Calendar page | 2 |
| User | ui-user.svg | Person in a circle | 2 |
| Notifications | ui-bell.svg | Bell | 2 |

### Totals

| Batch | Covers | Icons |
| --- | --- | --- |
| 1 | Screens already prototyped: brand, workspace tabs, command ribbon, core sources and codes, AI review, core status and controls | 96 |
| 2 | The rest of release 1.0 | 122 |
| 3 | Release 1.5 and later | 14 |
| All |  | 232 |

The 12 workspace tabs also need filled active versions, so Batch 1 produces 108 files.
