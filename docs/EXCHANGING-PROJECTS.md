# Exchanging projects with other programs

A running list, kept as problems turn up, of what happens when other programs read the project files PrismQDA writes (QDPX, the standard for exchanging qualitative projects between programs) and when PrismQDA reads theirs. It is listed under the program that was tried, and described by what the program does and what the standard says. Started 2026-11-12.

Each entry says how sure we are, because they are not the same thing:

- **Found**: seen happen, with the date and what was done.
- **Expected**: written in that program's own documentation or in someone else's test of it, and not yet seen by us. Treat it as a place to look, not a fact about our files.
- **To check**: a risk in what PrismQDA writes or reads, from reading our own code and the standard. Nothing has been tried with the other program yet.

When a test is run, add the program's name and version, the date, who ran it, which direction (PrismQDA to it, it to PrismQDA, or round trip), and what was found. Move an entry from Expected or To check to Found, or strike it out with the reason.

---

## PrismQDA (the program, its installer, its portable copy)

**Found**

- 2026-11-12, installer test: **no `.pqda` file association**. Double-clicking a project in Windows Explorer does not open PrismQDA. **Fixed in alpha 52** (decision 178): the installer registers the file type, the program opens the path it is given, and an installed program is one window that a second start hands its file to. Checked by script in the real installed window (`tools/installer-check`); not by a person in Explorer.
- 2026-11-12: **every alpha says version 0.1.0** in Windows (Settings, Apps; the file's properties), so the installed version cannot be told from Windows; I had to compare a file hash to know which alpha was installed. Consider a version that changes with each alpha (for example 0.1.0-alpha.52 in the installer's display version and the file version).
- 2026-11-12: the **Report page said "Statistics output arrives in a later release"**, which was untrue (the notebook exports to Word, Excel, a web page, or an R script). Fixed (commit 23b5b5e) and released in alpha 52; the alpha 51 installer still has the old line.
- 2026-11-12: after the uninstaller ran, the install folder and an empty folder named `prismqda-app.exe.WebView2` were left. Harmless. The window's settings and recent-projects folder in `AppData\Local\io.github.jasonswrench-beep.prismqda` is left on purpose (it is not the program); the website should say so (suggested to the website session, not yet confirmed).
- 2026-11-12: the count sentence on Team > Disagreements reads "1 of 1 unit are not settled yet" (and "2 of 2 units are not settled yet"): the verb does not agree. The new areas line was reworded to "Not settled yet: 1 of 2 areas and stretches" to avoid it; the older sentence still has it.
- 2026-11-12: in a 1600 by 1000 window the gallery of coded areas (Retrieved Segments, Gallery) shows only its first row, because that pane is short; making the window taller does not help.
- 2026-11-12: in the browser check, one whole run had its test server time out once (`connect ETIMEDOUT`, a 502 from the proxy), which failed the Themes step and the console step; the next whole run passed 351 of 351. A fault of the check's own server, not seen in the program.

**To check**

- At a 1600 pixel window the right end of the View tab's list of pages is cut off ("M..."): see whether it scrolls, and whether a keyboard or screen-reader user can reach every page.
- The installer's own wizard pages and its run-after-install option (only the silent route was run), a second computer, Windows 10, a computer without WebView2, the SmartScreen warning (a file built here has no download mark).
- Whether a person can tell, in Settings > About, which alpha they have.

---

## QDPX files PrismQDA writes and reads (our side, before any other program)

Checked against **QualCoder** on 2026-11-12 where it says so; everything else is still **To check**: it comes from reading our code and the standard, and has not been tried with another program.

- **The end of a selection.** The standard's text says "the last character", but its own example counts the way PrismQDA does (an end that is left out). *Checked with QualCoder 4.0: it counts the same way; every coding arrived on exactly the words sent. Still to check with other REFI-QDA programs.*
- **Line endings.** Programs differ on whether a line break is one character or two. PrismQDA keeps its text with one (a Windows file's CR LF becomes one line break when a document is imported), and writes it that way; a text inside a QDPX file from another program is kept as the file has it, because the file's places count it that way. QualCoder on Windows counts one and saves two; PrismQDA now reads that case correctly (see the QualCoder section). One program shifting codings by a character after each line ending (reported for one commercial program), and another needing an extra character (reported for another), are reported in the notes of QualCoder; *not tried here*.
- **Rectangles on PDF pages.** Written in PDF points from the lower left of the page's media box, as the standard says, with the page's turn and crop worked out, page counted from 0. Proved against our own renderer for four turns and a crop box. *QualCoder reads them from the top left (its own convention), so they would be upside down in it; see the QualCoder section. Still to check with other REFI-QDA programs.*
- **Ellipses and outlines on pictures** are written as the rectangle around them (the format holds rectangles). The export says how many. *Checked with QualCoder: arrive as the rectangle.*
- **Recordings** are written as a pointer to the file on this computer unless the person ticks to put them inside. A program that cannot find the file will show a document with no player. *Checked with QualCoder with the files inside the file: it copies them into its project. The pointer form (without the files) is not tried.*
- **Notes PrismQDA adds for itself**: a sentence on a recording's description that names its document, and a small piece of text on a document's description that holds its units. *Seen in QualCoder: they show as the memo of the document, as clutter, and QualCoder's own export carries them back.*
- **A user named "Adjudicated decisions"** holds decided codings, so another program will show an extra coder. *Not in the QualCoder test.*
- **A theme** is a code that cannot be applied. *Checked with QualCoder through the codebook: a theme becomes a category, with its definition.*
- **What is never written** (the export says so each time): saved queries, the Statistics notebook, the mixed methods design, reliability runs, AI runs, ratings, document folders, unit values, value labels, code settings, project history.
- **What arrives from another program**: read from files PrismQDA made itself, files written by hand to the standard, and (2026-11-12) a file made by QualCoder 4.0. Every program writes the standard a little differently (paths, identifiers, where descriptions go); the QualCoder file showed one real difference in the numbers (line breaks, the origin of PDF rectangles) and several in where things are kept (the coder on the selection, the words of a note as a file). Expect the same of the next program's first file.

---

## QualCoder (free, open source)

Status: **tried 2026-11-12, by script, in both directions** (QualCoder 4.0, released 2026-10-02; its source from github.com/ccbogel/QualCoder, LGPL-3.0, in its own Python 3.12 environment in `qualcoder-test`; nothing was installed system-wide, no account; its settings went to a throwaway folder). The scripts ran **QualCoder's own import and export code with no window**, so what a person sees in its window (areas drawn on a picture, a recording played, a PDF page) is **not checked**. The scripts are in `tools/qualcoder-check/` (read its README) and can be run again. Everything in the test project is invented: the Urban Mobility Study sample, a text with an emoji, accents, Chinese and Hebrew and Windows line endings, a picture with a rectangle, an ellipse and an outline by two coders, a three-page PDF with two rectangles, two silent recordings with stretches coded, a timed transcript, a memo, a link, a concept map.

**What came through unchanged** (PrismQDA to QualCoder, QualCoder's own export, and back into PrismQDA): all 19 codings of words, at the same places, including the ones after the emoji, the Chinese and the Hebrew; both coders; the 15 codes with their tree, colors (QualCoder snaps a color to its own palette) and definitions; the picture areas (rectangles exact; the ellipse and the outline as the rectangle around them, as the file says); the PDF rectangles; the stretches of recordings with their reasons; the memo; a codebook (QDC) both ways, with a theme as a QualCoder category and include/exclude text kept.
**Found in PrismQDA's file, fixed (2026-11-12, released in alpha 52)**

- QualCoder **stopped reading the whole file** at a recording that has a transcript: our `Transcript` had no `creationDateTime` and kept its words inside the element (`PlainTextContent`), where QualCoder reads only a file path (`plainTextPath`). The words are now a file in the package and the transcript has its date. (The standard allows either way; QualCoder needs the path and the date.)
- **Every coding went to a coder named "default", and a second coder's coding of the same words and code was dropped**: QualCoder reads the coder from the selection and ignores the one on the coding inside it, and PrismQDA put the coder only on the coding and put all coders of the same words in one selection. Now each coder has a selection of their own, with the coder and the time on it (the coding keeps them too). The same for picture, PDF and recording selections.
- **Memos were not imported**: QualCoder reads a note's words only from a file path. They are now files in the package.
- **A picture or a recording was lost when QualCoder exported its own copy**: it builds the stored file's name from the source name, and ours had no ending (`Bus stop`, not `Bus stop.png`), so it could not find the file again. Names in the file now carry the ending of the file; PrismQDA takes an ending that is the file's own off again when it reads (so the names come back as they were).
- **QualCoder's own export stopped at a picture area that had no name** (its importer stores nothing for the memo when the area has no `name`, and its exporter cannot write nothing). A picture and a PDF selection now have a `name` (the reason, or empty); QualCoder takes the memo of an area from there and ignores the `Description`.

**Found in reading QualCoder's file, fixed (same date, released in alpha 52)**

- **Every coding after the first line break was one character early** in a text QualCoder had saved on Windows: it counts a line break as one character in everything it writes and saves the text files with two (CR LF). PrismQDA now reads a file whose origin is QualCoder with the breaks counted as QualCoder counted them, and says so in the import report. The silent version of this (codings on the wrong words) is the worst kind of failure, so it is worth a second look in a real window.
- **PDF rectangles from QualCoder were upside down**: it writes the corners in points from the top left of the page; the standard (which PrismQDA follows) says from the bottom left of the media box. Read as the standard says, a rectangle at the top of a page lands at the bottom. A file whose origin is QualCoder is now read from the top left. The other way (PrismQDA's rectangles in QualCoder) is therefore **mirrored top to bottom in QualCoder** by its own reading of the numbers; nothing to fix on our side without leaving the standard. Not seen in its window.
- **Two coders of one name**: a coder from another program's file whose name is the same as the person who opens the project (the same researcher) became a second coder of that name, with all the codings on the placeholder. The person who opens the project is now that coder when the name is the same and the coder came from a file.

**Found in QualCoder (nothing for us to fix; kept so it is not a surprise)**

- Sync points: it reads a sync point's `position` one character late (it adds 1 to the number that its own export writes), so codings on a timed transcript land one character to the right in QualCoder, and the times are dropped ("syncpoints are not stored in QualCoder"). The words themselves arrive.
- Cases arrive **empty**: a case's `SourceRef` (the documents it holds) is ignored. **Case variables are carried** (tried 2026-11-12: four variables on two cases, a word, a whole number, a decimal and a date written as text, are all held by QualCoder as case attributes, the number ones as `numeric` and the others as `character`, and all eight values come back to PrismQDA unchanged; a date is a word in QualCoder, and the measurement level of a variable (nominal, ratio) is not in the file, so a word comes back as a *text* variable).
- Links between passages and concept maps are not imported (it says graphs are not imported). Sets (document sets) not in the test.
- A recording that PrismQDA wrote arrives as three sources (the document, the recording, and the transcript's text file), and the document carries a sentence and a few lines of PrismQDA's own data in its memo (the kind and the units of the document). QualCoder's own export carries that memo back, which is why PrismQDA reads its units again.
- A source's coder is always "default" (PrismQDA does not keep who imported a document), so "default" appears as a coder in QualCoder and comes back to PrismQDA as a third coder.
- The automatic description of a picture ("Picture <file>, 800 by 500 pixels") names the file; after QualCoder renames files it names a long identifier. Cosmetic.

**Expected** (from QualCoder's own documentation, a user test dated February 2021, and no version number given; not seen in the 2026-11-12 test)

- It says it does **not guarantee the full standard**.
- Case attributes did not carry to other programs it was tested with; dates and booleans become text.
- If no coded data appears after import, its coder name in Settings may need changing.
- The extra line-ending character one program asks for, and another program's shift of a character after each line ending, are in the section on other programs below.

**Still to check with QualCoder**: opening the imported project in its window (do the areas, the recording and the PDF show where they should; does a person see the "default" coder); a trial with document sets; a file written by QualCoder from a project it made itself (not one it imported); QualCoder on a computer where its line-ending dialog is answered the way another program asks for (Windows line endings added on purpose).

---

## Other REFI-QDA programs (commercial or web-based): not tried

Status: not tried. Each needs a licence or a trial, and a sign-up for a trial is a person's step; a colleague or an institution that already holds a licence can run a round trip.

**Expected** (the programs' own help pages for REFI-QDA exchange, and the notes of free programs that tried them; none of it seen by us)

- Text coding in a file QualCoder wrote "seems to require an additional line ending character" in one program; another reported codings shifted by a character after each line ending (2021, "not yet resolved").
- A QDPX file holds only what every participating program shares. At least one program's own export leaves out document sets, summaries, paraphrases and code variables (its version 18 help).
- At least one program says QDPX is not a dependable way to move a project between versions of that same program.
- At least one program's help pages say what its import and export carry; read the page for the version used before relying on any summary (a search summary said one lists data files, codes, and cases only, which we have not confirmed).
- One paid program with a trial imports and exports QDPX from its export menu; case attributes were not imported from a QualCoder file in a 2021 user test.

When one is tried, add what was found here, by what the program did, with its version and the date.

## ELAN (free, from the Max Planck Institute for Psycholinguistics)

Status: **tried 2026-11-12, one direction: ELAN to PrismQDA.** ELAN 7.1 can only *write* QDPX (File > Export Multiple Files As > REFI Project File...); it has no QDPX import, so there is no file of ours for it to read.

**How it was tried.** ELAN 7.1 (the Windows zip from its own site, GPL-3, unpacked in its own folder with no installation) was driven without its window: `tools/elan-check/ElanRefiExport.java` calls ELAN's own writer class (`REFIEncoder`) the way the export window's last step does, on an annotation file of invented material (`tools/elan-check/make_eaf.py`: two speakers, a coding tier under one of them from a vocabulary, a coding tier on the recording itself, a vocabulary entry no annotation uses, words with accents, an emoji, and Chinese). Both a one-file and a two-file export were read in PrismQDA. **Not tried: ELAN's window, and ELAN reading anything.**

**Found in PrismQDA's reading of ELAN's file, fixed (2026-11-12, decision 179; in the next alpha)**

- ELAN names the project file `<project name>.qde`, not `project.qde`, and PrismQDA refused the whole file ("the archive holds no project.qde"). A single project file under any name is now read (a file called `project.qde` still wins, then the one nearest the top).
- ELAN names a recording by the last part of the address its annotation file holds, so a space is `%20` in the name (`Bus%20ride%20talk.wav`) as well as in the path. The document was called `Bus%20ride%20talk`. A name that is the last part of the source's own path is now read as the address it is.
- ELAN writes the codings of a transcript as a **second source** (a text source) on the very same transcript file that the recording's transcript is made of, so one transcript became two documents with the same words and the codings on only one of them. A text source on the same file as a recording's transcript is now read as part of that transcript's document, and the import report says so.

**Found in ELAN (nothing for us to fix; kept so it is not a surprise)**

- The file is valid against the standard's schema (`tools/qdpx/validate-qde.ps1`) and a second reader (pyrefiqda) read it. It uses a folder named `Sources` (capital S), which PrismQDA already read.
- The codings of words (a coding tier under a transcript tier) carry **no coder**, although ELAN knows the tier's annotator; they arrive as the coder *Imported (QDPX)*. The coded stretches of the recording (a free coding tier) do carry their coder.
- The transcript text holds the speaker label and the time of each line inside the words (`INT: [00:00:00.000] How do you...`), so a word count counts the labels. The sync points are at the first and last letter of each utterance, so the label of the next speaker is a unit of its own with the times of the gap.
- Each coding tier's vocabulary entries become codes whether or not any annotation uses them, and the vocabulary becomes a set of codes.
- **Bundling the recording**: with *include media files in the bundle*, ELAN 7.1 on Windows (its own Java 26, through the driver above, not the window) logged "The media file has not been found" for a file that was there (it turns the address into `/C:/...`, which Windows cannot open) and wrote the recording's address instead. A recording is therefore only found if it is still at the address on the computer that made the file. ELAN's own manual says the programs it tested did not find media files on import either.

**Still to check with ELAN**: its window (the export dialog, and whether it writes the same file); a video recording; subdivision and included-in tiers; a file with several speakers in one tier; a real project from a person who uses ELAN.

---

## Others that read or write QDPX

**pyrefiqda** (a free Python reader, MIT, whose models are generated from the standard's own schema): **tried 2026-11-12, both files read without error.** `tools/qdpx/second_reader.py` parsed PrismQDA's whole-project file and the file QualCoder wrote from it, and the counts it printed match the project (2 coders, 15 codes, 10 text, 1 picture, 1 PDF and 2 recording sources, 19 + 4 + 2 + 2 selections, 27 codings, 2 cases, a note, a link, a graph). This is a second opinion on the *shape* of the file, in addition to the schema check; it says nothing about how another program would show the file.

Not tried: the commercial and web-based programs listed above (some offer their own converters), **Taguette** (free; a lesson says it exports only the codebook, as QDC, not whole projects; check its current version), **PortableQDA** (on PyPI, LGPL, last release 2021, and its early release notes say QDPX was not yet implemented; pyrefiqda above was used instead).

