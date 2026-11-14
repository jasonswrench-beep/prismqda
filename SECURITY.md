# Security policy

PrismQDA is software for working with research data, some of it about people. A flaw that exposes or corrupts that data matters, so
reports are welcome and are taken seriously.

## Supported versions

Only the newest release is supported. PrismQDA is early software (see [TODO.md](TODO.md) and [TESTING-TODO.md](TESTING-TODO.md) for what has
and has not been checked), and fixes go into a new release, not into old ones.

## Reporting a vulnerability

Please report it **privately**, not in a public issue:

1. On the repository's page choose **Security**, then **Report a vulnerability** (GitHub's private reporting).
2. Say what version you used (**Help > About**, or the name of the installer), what you did, what you expected, and what happened.
3. If a file is needed to reproduce it, attach a file made of **invented data only**. Never send real participant data, a real project,
   or anything you are not allowed to share.

What to expect: a first answer within about a week, a fix or a plain statement of why not, and credit in the release notes if you want it.
This is a small project with one maintainer, so these are goals, not guarantees.

## What counts

Most welcome are problems in how PrismQDA reads files it did not make, because a person may open a file from anyone:

- Word, OpenDocument, Excel, RTF, PDF, SPSS, CSV, caption and QDPX files, pictures, and recordings (a file that crashes the program, uses all the
  memory, runs for ever, writes outside its own folders, or makes the program say something false about the file).
- Encrypted projects (a way to read a project without its passphrase, or to damage one without the program saying so).
- The words an AI prompt packet may hold (material marked as flagged, or names a person asked to hide, that gets into a packet or an export).
- The installer, the portable copy, and the file type registration.
- A network request: PrismQDA makes exactly one kind, when a person presses Check for updates in Settings (one address over https; the request carries only the version of the program and the kind of computer; the download is checked against a signature before anything runs; decision 189). Any other connection it opens, or any way to make it connect to an address a file names, is a serious report.

What that one request reveals: Check for updates contacts github.com, which sees the computer's IP address, as any web server does. Nothing else leaves the computer, and nothing from a project is sent.

Not in scope: a problem that needs the attacker to already control the computer or the person's Windows account, and software that PrismQDA only
talks to by files (a chatbot a person pastes a packet into, or another analysis program).

## Advisories that do not apply

- **RUSTSEC-2026-0459** (a memory disclosure in SQLite's write-ahead log, fixed in SQLite 3.54.0) is reported against `libsqlite3-sys` 0.38.2, and scanners will keep reporting it until that crate moves to 0.39. It does not apply to PrismQDA: the program builds SQLite from the `sqlite3mc-src` crate, pinned to `=206.1.0`, which bundles SQLite 3.54.0 (`vendor/libsqlite3-sys/PATCHES.md`). When `sqlite3mc-src` for SQLite3 Multiple Ciphers 2.7.0 is published, the program moves to it, as that file describes.

## Verifying a download

Each release has a `.sha256.txt` file beside each download, and the Windows files are signed. Right-click the setup file, choose
**Properties**, then **Digital Signatures**: the signer should be the name shown on the release page, with a timestamp. If it is not,
do not run the file, and tell the maintainer.
