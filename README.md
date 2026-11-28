# PrismQDA

A free, offline Windows tool for qualitative and mixed-methods analysis: hand coding, method-guided workflows, honest reliability, inspectable AI, descriptive statistics checked against R (the tests of difference, association, and prediction are done in PrismStatistics, which opens the case table PrismQDA exports), and reporting that drafts its own methods appendix.

What is left to do is in [TODO.md](TODO.md). The full design is in [docs/SPECIFICATION.md](docs/SPECIFICATION.md), and the interface design is [docs/INTERFACE.md](docs/INTERFACE.md). The order to build it in is [docs/ROADMAP.md](docs/ROADMAP.md). Design decisions made while building, and what is still open, are in [docs/DECISIONS.md](docs/DECISIONS.md). The command layer the interface talks to is in [docs/API.md](docs/API.md). What only a person can check is in [TESTING-TODO.md](TESTING-TODO.md).

How the planning package of 2026-11-05 compares with what was built is in [docs/planning](docs/planning). The logo and icons are in [assets](assets) and their design notes in [design/icons](design/icons).

## Where it stands

This is early software with very little use so far. Windows 11 installers (and a portable copy) are on the [Releases page](https://github.com/jasonswrench-beep/prismqda/releases); use the newest one. They are signed (the signer is Jason Wrench), with a SHA-256 file beside each download. Keep copies of your projects. What has and has not been checked is in [TESTING-TODO.md](TESTING-TODO.md): so far the installer, the window and the analyses have been run by scripts and automatic checks, and no screen-reader pass, real class, or second computer has been tried. Found a security problem? See [SECURITY.md](SECURITY.md). The program connects to nothing on its own; the one exception is the **Check for updates** button in Settings, which you press and which is described in [docs/UPDATES.md](docs/UPDATES.md).

Built and tested:

- **The engine** (a Rust library, specification sections 3 and 4): one SQLite file per project with a published schema ([crates/prismqda-engine/schema/v1.sql](crates/prismqda-engine/schema/v1.sql), then each step that follows it, such as [v2.sql](crates/prismqda-engine/schema/v2.sql) for coding ratings; older files are copied aside and migrated on opening), dated backups, read-only opening of files from a newer version, and an append-only audit trail enforced inside the database.
- **Importers** for text, Markdown, HTML, Word, OpenDocument text, RTF, PDF, CSV, TSV, Excel, SPSS (`.sav`, `.zsav`), transcripts and captions, audio and video recordings (read through captions beside them, or coded by stretches of time when there are none), and YouTube, Reddit, and generic social exports. Spreadsheets become sources, cases, and attributes through a mapping the person controls.
- **Encrypted projects**: a project can be encrypted with a passphrase (every page of its file, SQLCipher 4 compatible, with SQLite3 Multiple Ciphers built in, so building needs no OpenSSL or Perl), turned on and off and changed in Settings, asked for when the project opens, and kept on its backups. A lost passphrase cannot be recovered by anyone.
- **Exports**: the codebook as Excel, Word, CSV, or the REFI-QDA codebook format, and the coded passages as Excel, Word, or CSV, with the words, the code, the coder, and the source. A list of names to **pseudonymize or redact** (the Prepare group of the Import tab) can be applied to everything that is exported (Word, Excel, CSV, the report, and the packets taken to a chatbot) or to the project itself for good, after a backup, with the list kept as a separate key file. Whole folders of files import in one step (500 Word files in about a second). Split view shows a second document beside the one being read, read-only, with its codings marked. An audio or video recording comes in through the caption file beside it (PrismQDA does not transcribe): the transcript's turns know when they are spoken, a player sits above the text and says which turn is playing, and the recording is linked, not copied. Pictures (PNG, JPEG, GIF, WebP, BMP) come in as documents kept inside the project: rectangles, ellipses, and outlines are drawn over parts of a picture with the pointer or the keys and coded like words, and Explore, Retrieved Segments, and the exports count them with the words. Every word the window says goes through a language catalog (see [docs/LOCALIZATION.md](docs/LOCALIZATION.md)), so a language can be added by translating one file; a person can bring in a translation made elsewhere and label it (see [docs/TRANSLATOR-GUIDE.md](docs/TRANSLATOR-GUIDE.md)), and nothing is translated by a machine behind anyone's back. A newer version of a file can replace a document's text in place: the codings follow the words that survive, a backup copy is saved first, and anything removed is on record.
- **Coding**: codebook tree, free-span and whole-unit coding, memos, undo, text edits that carry codings along, search (words, phrases, wildcards, regular expressions, AND, OR, NOT, NEAR) and autocoding into a review queue, and the six method templates as data. The interface has Auto Code: passages found by words, by speaker, or by heading are suggested a code, looked at first, and accepted, rejected, or corrected one by one or a whole run at a time, with one undo for a whole decision. The **AI Assistant** works by prompt packet only: PrismQDA builds a packet of text from the codebook and some passages, you take it to any chatbot, and paste the reply back. Every quote is checked against the document word for word, anything invented is refused, and what passes waits in the same review queue. PrismQDA itself never connects to an AI service, and every packet and answer is kept in an AI use log with a drafted methods sentence.
- **Reliability** (section 18.3): random and stratified samples with a saved seed, agreement between any number of coders (percent agreement, Cohen, Scott, Fleiss, Brennan-Prediger, Gwet, Krippendorff) checked against R, per-code agreement, confusion tables, bootstrap intervals, reports that call agreement with a model "concordance", and a lock on the text of sampled units until the run is archived.
- **The interface** ([app/ui](app/ui)): React and React Aria, rebuilt to the design in [docs/INTERFACE.md](docs/INTERFACE.md). A title bar with a search and command box (Ctrl+K), workspace tabs (Home, Data, Code, Synthesize, Report), a command ribbon that says why a command is unavailable, a status bar, Undo, and the PrismQDA prism and icon set. Code has the sources over the codebook, the document, and an inspector. Keyboard coding and accessibility checks run in a headless browser.
- **The desktop shell** ([app/src-tauri](app/src-tauri)): a Tauri window around the interface and engine.
- **The license rule** (section 3.3): `tools/prismqda-licensecheck` fails the test run if any dependency, in the engine or the app, is not under a permissive license or a reviewed exception.

What is built and what is not changes with every release: [TODO.md](TODO.md) is the current list, the [user guide](docs/USER-GUIDE.md) describes the program as it is, and [docs/ROADMAP.md](docs/ROADMAP.md) says where it is going. The statistics are checked against R ([docs/VALIDATION-REPORT.md](docs/VALIDATION-REPORT.md)).

## Building and testing

You need Rust (1.92 or newer; the PDF page drawing library needs it). The interface also needs Node.js. The SQLite that is built in is SQLite3 Multiple Ciphers (it encrypts a project's pages when asked to), compiled from the `sqlite3mc-src` crate by a copy of `libsqlite3-sys` in [vendor/libsqlite3-sys](vendor/libsqlite3-sys/PATCHES.md); it needs no OpenSSL and no Perl, only a C compiler (the Visual Studio build tools on Windows).

```
cargo test --workspace
```

On a Windows PC whose performance cores are unstable (the author's is), `TEST PRISMQDA.bat` runs the same command on the efficiency cores only.

Some tests run only when asked:

- A real-study check, which needs a folder of coded data: set `PRISMQDA_USARK_DIR` to it.
- Performance at the largest supported project size: `cargo test --release -p prismqda-engine --test perf -- --ignored --nocapture`.

The interface (from `app/ui`):

```
npm ci
npm test
npm run build
```

The desktop app and its installer (a Windows installer, built on the efficiency cores):

```
powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1
```

The script builds only from a clean tree whose latest commit already carries the annotated tag `v<version>` (`git tag -a v<version> -m "PrismQDA <version>"`), so each release tag is the exact commit the build came from; `tools/check-release-commit.ps1` makes that check and has no off switch. For a throwaway setup to try something out, `app\BUILD-INSTALLER.ps1 -TestBuild` skips the check, but it can never sign, and it leaves only a file named `...-TEST-<time>.exe` in `app\src-tauri\target\release\bundle\test` that must not be released.

With `-Sign` the program, the installer's plugins, the uninstaller and the setup file are signed and timestamped with Azure Artifact Signing, and the build fails if any program in the setup is not (`tools/sign_windows.py` and `tools/check_signatures.py`; the header of `app/BUILD-INSTALLER.ps1` lists the four settings and the one-time `az login` it needs). `app\BUILD-PORTABLE.ps1 -Sign` does the same for the portable zip. Without `-Sign` the files are not signed.

A plain `cargo run` in `app/src-tauri` will not show the interface: Tauri's command-line tool (installed with the interface's npm packages) builds it the way the installer needs.

Try the harness:

```
cargo run -p prismqda-cli -- new study.pqda --name "My study"
cargo run -p prismqda-cli -- --help
```

The project file extension `.pqda` is provisional; the final one is an open question in section 30 of the specification, and `PROJECT_EXTENSION` in the engine is the one place that changes.

## Shared pieces and what counts as installed

The OpenStage and Prism apps follow one rule for finding things on the computer:

- **Shared pieces live in one fixed folder each, directly under `%ProgramData%`** (found with Windows' known-folder call, not from the environment): the GPU Voice Pack in `%ProgramData%\GPU Voice Pack`, and voice models (speech recognition, voice detection, speaker models and voices) in `%ProgramData%\Voice Models`. Every app and both hubs (OpenStage Hub and Prism Hub) look for them there and nowhere else. A piece counts as installed only when its files are there and its folder can be changed only by the account that installed it, SYSTEM and the Administrators.
- **Apps are found where they are installed**: in Windows' installed apps list, or in the app's own per-user folder (`%LOCALAPPDATA%\<app name>`). Never on `PATH`.
- **Runtimes ship inside the app's own installer** (Node.js, Python, a speech engine). Nothing has to be installed separately, and an installed app never falls back to a copy it finds on `PATH`.

PrismQDA uses no shared pieces today. The GPU Voice Pack is made in this repository.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). Third-party crates and their licenses are listed in [THIRD-PARTY.md](THIRD-PARTY.md).

PrismQDA is designed without reference to any other qualitative analysis program's source code (specification section 3.3).
