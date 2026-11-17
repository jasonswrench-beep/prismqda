# Updates: the Check for updates button

PrismQDA is offline by design, with one exception that a person starts: **Settings > Updates > Check for updates** (decision 189). This page says
what the button does, why it can be trusted, and what the maintainer does to make a release that the button will offer.

## What a person sees

- Nothing is checked at start, in the background, or on a schedule. The program never looks for a new version by itself.
- Pressing **Check for updates** asks one address (the update feed, a small file `latest.json` on github.com) whether a newer version has been
  published. The request carries what every web request carries (the version of the program and the kind of computer) and nothing from any project.
- If there is a newer version, the window shows its number and its notes. **Download and install** is a second step, and it asks for confirmation.
  The installer is downloaded, **its signature is checked against the key built into the program**, the open project is saved and closed, and
  the installer runs for the current user (no administrator rights). A file whose signature does not match is refused and nothing is installed.
- Nothing is checked while the open project has the **offline lock** on. A **portable copy** is never updated in place: it says where to download
  the new zip. A window that is not the installed program (a browser, the development bridge) says that updates can only be checked in the program.
- A build made before the update key was set up says so ("not set up") and cannot connect to anything.

## How it is built

- The shell (`app/src-tauri/src/updates.rs`) uses Tauri's updater plugin, but never lets it connect unless asked: `update.state`, `update.check`
  and `update.install` are the window's own commands, handled in `main.rs` before the engine, which has no network code. The address (`ENDPOINT`) and
  the public key (`PUBKEY`) are constants in `updates.rs`; while `PUBKEY` is empty the updater is not set up.
- The release build only accepts an `https` address. The check has a 20 second timeout, and a failure is shown in words and changes nothing.
- The update signature is a separate key from the code-signing certificate: the Azure signature says who made the setup file to Windows
  (SmartScreen), and the update signature says to PrismQDA itself that this file is the maintainer's. The update signature is made **after** the code
  signature, over the exact bytes people download, and checked by the program before it runs anything.
- Tests (`cargo test` in `app/src-tauri`): a feed on this computer announces a newer, an older and the same version; a missing feed and an address nobody
  listens on fail in words; a download passes only when it matches its signature (a changed byte and a signature by another key are refused); the
  offline lock and a portable copy are refused before any connection; and a build with no key makes no connection. A test that is run by hand
  (`PRISMQDA_REAL_SETUP_DIR=<folder of a setup built with -Updates>`, `cargo test a_real_built_setup -- --ignored`, in a build whose `PUBKEY` is the
  public half of that key) takes a really built, Azure-signed setup file through the same check.

## Making the key (once)

```
powershell -ExecutionPolicy Bypass -File tools\make-update-key.ps1
```
It asks for a password (twice) and makes `prismqda-update.key` and `.pub` in `PrismQDA-update-key` in your profile folder, outside the repository,
and puts the public half into `PUBKEY` in `updates.rs`. **Back up that folder and the password.** If the key or the password is lost, the programs that
are already installed can never be updated again (a new setup file would have to be installed by hand once, and carry a new key). If someone else
gets both, make a new key at once and publish a setup built with it.

**The key was replaced once (2026-11-16, decision 204).** The password of the first key could not be found, so a second key was made and beta 3 carries its public half. Copies of beta 1 and beta 2 cannot install beta 3 through the button (the signature does not match the key built into them, and nothing is installed); they download the setup from the releases page once. After making a key, check its password at once by signing a small test file (`tauri signer sign -f <the .key file> <a small file>`, with `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` set), and keep the password in a password manager.

## Making a release that the button offers

1. Build with the key: set `TAURI_SIGNING_PRIVATE_KEY_PATH` to the `.key` file and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to its password, then
   `powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates -NotesFile <the notes, as text>`. The script stops if the key is
   missing or if `PUBKEY` is empty or is not the public half of the key. It signs the finished setup file with the update key and writes `latest.json`
   (the version, the notes, the day, the signature, and the address of the setup file: `https://github.com/jasonswrench-beep/prismqda/releases/download/v<version>/<setup file>`).
2. Publish the release `v<version>` with the setup file, its `.sha256.txt`, and the portable zip.
3. Publish the feed: the release called `update-feed` (a pre-release made once, titled "Update feed: not a download") holds one asset, `latest.json`.
   Replace it with the new one: `gh release upload update-feed latest.json --clobber --repo jasonswrench-beep/prismqda`. The address in `ENDPOINT` is that
   asset's. (It is not `releases/latest`, because GitHub does not count a pre-release as the latest release.)
4. Check from a computer with an older installed version: press Check for updates, read the notes, install, and see the version change.

**Versions** are compared the way semantic versioning orders them: `0.1.0-beta.1` is older than `0.1.0-beta.2`, which is older than `0.1.0`. Every
release must have a number higher than the one before it (`version` in `app/src-tauri/tauri.conf.json` and the same in `app/src-tauri/Cargo.toml`),
or the button will not offer it.

**Withdrawing a release**: put the previous `latest.json` back in the feed. People who already installed the bad version install the earlier setup
file by hand (the installer lets an older version replace a newer one).

## What it does not do

No automatic checks, no downloads without a go-ahead, no telemetry, no account, no update of a portable copy, no update of a project (a project's
file format is brought up to date when it is opened, with a copy kept), and no check while the offline lock is on.
