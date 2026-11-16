# Requests to the shared core

Things PrismQDA needs from `openstage-core` that the core does not offer yet. PrismQDA does not build its own version of a
shared piece when the core could offer it; where it had to write something to finish its list, the entry says what and says
what replaces it once the core has the piece. Newest last. Core as read: `prismqda-stats-v1.0.0` (commit 13b210c), PSTAT-1
revision 1.2 (`CORE-NOTES.md` as of 2026-11-15).

**Status, 2026-11-15:** PrismQDA now takes `prismqda-stats` from the core at the tag `prismqda-stats-v1.0.0` and no longer has
a copy of it. The case table (PSTAT-1 section 7.3) is written, and checked with `checkPstat` and opened by PrismStatistics's
own program. R1 to R5 below are what the work of that round left open.

## R1. A test set for the case table (section 7.3)

**For:** development list item 3, and what `CORE-NOTES.md` says in the section for PrismQDA (point 5).

The core has the spec, one example file (`specs/pstat/examples/case-table.pstat`) and the layout checks in `checkPstat`, and no
set that holds a writer to the cases listed in the notes: sub-codes counted with their parent and not, a case with no documents,
attributes whose names must change, missing codes, names left out, and a code that was retired or merged.

PrismQDA's own export test (`crates/prismqda-export/tests/pstat.rs`) already covers each of those by hand-worked figures, and
`tools/pstat-check/check.mjs` runs the core's `checkPstat` and `attributeNames` on three files it writes (names kept, names left
out, sub-codes not counted). All three pass the checker and open in PrismStatistics's `prism.exe` (`open_project`).

Asked: the set itself, as the core makes the others: a folder of cases, each a description of a small project (its cases, attributes
with levels, labels and missing codes, codes with their tree, which excerpts carry which codes) and the `.pstat` file the profile
makes of it. PrismQDA will build each project from its description, export it, and match the file on every member except
`exported`, `project.id` and the release in `app_version`. The description's layout is the core's to choose; PrismQDA will write
the reader for it.

## R2. A Rust implementation of reading, writing and checking `.pstat`

**For:** list item 3 (writing the case table) and item 5 (opening a `.pstat` file with open comments).

`@openstage/pstat` is JavaScript. PrismQDA and PrismStatistics are Rust and each needs the container and the dataset, and PrismStatistics
keeps its own reader and writer. So that PrismQDA could finish its list, it has two small pieces of its own:

- `crates/prismqda-export/src/pstat.rs`: writes the profile `PrismQDA case table 1` only (the zip, `project.json`, `case-table.json`, the
  naming rule of section 7.3 item 3).
- `crates/prismqda-import/src/pstat.rs`: reads `project.json`'s dataset into a table (rules Z1, Z3, P1, P2, D1 to D4, D7, and J1), and
  ignores the rest. It writes nothing.

Asked: a `pstat` crate in the core with the container, `project.json`, a profile's part, and the checks `checkPstat` makes, tested on the
same 33 conformance files and the same example files as the package. PrismQDA would then drop both of its pieces for it, as it dropped its
statistics crate.

## R3. Test vectors for the naming rule (section 7.3, item 3)

PrismQDA has a Rust copy of `attributeName` and `attributeNames` (in the writer above). Its unit tests and the checker script compare it
with the package on the names in its own test project, which is a small table. Asked: a JSON file of inputs and the names the rule makes
(with the names already taken), in the package's tests, that the Rust copy can read, so the two are held to one table. It would cover a
name that is empty, one that starts with a digit, one over 64 characters, two that collide ignoring case, a name that looks like
`code_<n>_present`, and a name made only of characters outside ASCII.

## R4. Ids are never given again: the warning in 7.3 can change

**For:** `CORE-NOTES.md`, point 3 of the section for PrismQDA ("Once ids are never given again, tell the core").

Done in PrismQDA project format 16 (`crates/prismqda-engine/schema/v16.sql`, `src/ids.rs`): a table, `id_floor`, keeps the highest number
given to a code and to a case; a new code or case takes the number above the higher of that and the highest it holds, so Undo of "add" no
longer frees a number. Checked: adding a code or a case and undoing it, several times, and closing and opening the file between, never
gives a number twice; a file of an older format starts from the highest number it holds when it is brought forward.

What cannot be known: a number that was given again by a file of format 15 or earlier before it was brought forward. A case table made
before and one made after the upgrade of the same project could still disagree about such a number; two made after cannot.

Asked: a revision of 7.3 that says this in place of the warning: from PrismQDA project format 16 an id is never given again; a table made
from a project that was upgraded may disagree with one made before the upgrade where Undo had freed a number. PrismQDA does not write the
project's format into the file today; if the core wants it, `case-table.json` could carry an optional `project_format` (an integer), and
PrismQDA will write it.

## R5. The exchange file of Prism Reference Manager

**For:** list item 4 ("Read Prism Reference Manager's exchange file").

`prism-family-exchange` version 1.0 is described in five lines of the Reference Manager's `docs/library-format.md`; the layout is in its
code (`export_family_exchange`, `crates/prm-core/src/library/notes.rs`). PrismQDA reads it from that code, with files made by hand: no
file made by a real build of the Reference Manager has been seen.

Asked: the format taken into the core as PSTAT-1 was, so that the Reference Manager and the tools that read it are held to one text:
a specification (the manifest members, `sources[].csl`, `excerpts[]` with `type`, `source_ids`, `exact_text`, `user_text`, `locator`,
`anchor_state`, `origin`), a sample file made by a real build, and a conformance set. Two questions the specification should answer:

1. The manifest note says "receivers keep source ids and return links as separate data". What does the return look like, and does
   PrismQDA write it? PrismQDA keeps each source's id and each excerpt's id as variables and in the document's settings, so that a
   return can name them, and writes nothing back today.
2. How is the file named? PrismQDA recognises it by `manifest.format` and not by its extension (`.json`).

## R6. Which variable of a `.pstat` file is the respondent, and which are comments

**For:** list item 5 (open comments from evaluations and course surveys).

PrismQDA opens a `.pstat` file as a table, and the person says in the import window which variable names the respondent, which hold open
comments to code, and which are values for the cases (the same window as for a spreadsheet). It guesses from the variables (a text
variable with sentences in it is a comment; a variable named like `id`, `respondent` or `participant` is the respondent). Asked: a way for the file
to say so, so that PrismQDA need not guess and a file written by the evaluation module of PrismStatistics (its list, item 7) is opened
as it was meant: optional members of a variable, for example `role` (`id`, `comment`, `value`), in a revision of section 4.

For the way back: when PrismQDA sends a case table for a project made from a `.pstat` file, it writes the respondent's id as an attribute
(`case_name` also holds it unless names are left out), so that PrismStatistics can match the table to its data. If the core prefers it,
`case-table.json` can name the original file and the variable it matched on (`source_file`, `source_key_variable`), and PrismQDA will
write them.
