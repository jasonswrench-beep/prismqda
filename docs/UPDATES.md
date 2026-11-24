# Updates: the Check for updates button

PrismQDA is offline by design, with one exception that a person starts: **Settings > Updates > Check for updates** (decision 189). This page says
what the button does, why it can be trusted, and what the maintainer does to make a release that the button will offer.

## What a person sees

- Nothing is checked at start, in the background, or on a schedule. The program never looks for a new version by itself.
- Pressing **Check for updates** asks one address (the update feed, a small file `latest.json` on github.com) whether a newer version has been
  published. The request carries what every web request carries (the version of the program and the kind of computer) and nothing from any project.
- If there is a newer version, the window shows its number and its notes. **Download and install** is a second step, and it asks for confirmation.
  The installer is downloaded, **its signature is checked against the key built into the program**, the open project is saved and closed, and
  the installer runs for the current user (no administrator rights). A file whose signature does not match is refused and nothing is installed. So is a
  file whose signature was made for another file than the version announced (an older installer offered under a newer number): see "The version is bound
  to the signed file" below.
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
- **The version is bound to the signed file** (decision 205). The feed is not signed, only the installer is, so whoever could change the feed could pair an
  older installer, genuinely signed, with a newer version number. The signature's *trusted comment* (minisign's global signature covers it, so it cannot be
  changed without failing the signature check) names the signed file, for example `file:PrismQDA_0.1.0-beta.4_x64-setup.exe`. The program refuses an update, at
  Check and again at Install and before anything is downloaded, unless that name is exactly `PrismQDA_<version>_x64-setup.exe` for the announced version and the
  version is newer than the running program. Tauri's own `require_signed_version` option is not used: the signer in use (the Tauri command line 2.12.4) does not
  write a version into the comment yet, so it would refuse every release.
- **The backup key** (decision 205): see "The backup key, and rotating the keys" below. `Feed` holds both public keys; `fetch_verified` in `updates.rs` is the
  one place that downloads and checks, and retries with the backup key only after a failed *signature* check (`Error::Minisign`), never after a network error.
- Tests (`cargo test` in `app/src-tauri`): a feed on this computer announces a newer, an older and the same version; a missing feed and an address nobody
  listens on fail in words; a download passes only when it matches its signature (a changed byte and a signature by another key are refused); a file signed by
  the backup key installs when the main key does not match, a file signed by a third key is refused, and an empty backup key behaves exactly as before; a failed
  download is not retried with the backup key; a genuine signature of an older installer offered under a new version is refused before any download, and a
  signature whose trusted comment was changed to name the right file is refused by the signature check; the offline lock and a portable copy are refused
  before any connection; and a build with no key makes no connection. The signatures are made with throwaway keys (`app/src-tauri/tests/fixtures/README.md`). A test that is run by hand
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

## The backup key, and rotating the keys

One key signs every release, and the programs already installed trust only that key. If its file or its password is lost, or someone else gets both, nothing new
could be signed that people's programs would accept. The **backup key** is the way out (decision 205; finding M2 of the security review of beta 3).

- **What it is.** A second key, made once by the owner and kept **offline** (a USB stick in a drawer, its password in the password manager). Its public half is built
  into the program as `BACKUP_PUBKEY` in `updates.rs`. Its private half is never used for a normal release. While `BACKUP_PUBKEY` is empty the program trusts the
  main key alone, exactly as before.
- **What the program does with it.** A download is checked against the main key first. Only if that signature check fails, and only then, it is checked once more
  against the backup key. A network failure, a version that is not newer, or a file name that does not match is never retried with the backup key.
- **Making it (the owner, once).** `powershell -ExecutionPolicy Bypass -File tools\make-update-key.ps1 -Backup`. It asks for a password twice, makes the key in
  `PrismQDA-update-key-backup` in your profile folder, and puts the public half into `BACKUP_PUBKEY` (never into `PUBKEY`). It refuses a folder inside the
  repository, the main key's own folder, a folder that already holds a key, and a run while `PUBKEY` is empty. Review the change with `git diff`, commit it, and
  release as usual, signed with the **main** key. That release is the first that carries the backup key, and only programs that carry it can be rescued by it
  (beta 3 and earlier cannot). Then take the key folder off the computer, as the script says, and test its password once by signing a small file.
- **Rotating the keys** (only when the main key is lost, or may have been stolen):
  1. Make a new main key, then a new backup key, each in a new folder (`make-update-key.ps1 -Folder <new folder>`, then `make-update-key.ps1 -Backup -Folder <another
     new folder>`). The scripts put both new public halves into `updates.rs`; commit that.
  2. Raise the version, commit everything, and tag the commit (the usual rule).
  3. Build with the **old backup key**: `TAURI_SIGNING_PRIVATE_KEY_PATH` is that key's file and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` its password, then
     `powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates -Rotation -PreviousTag v<the last release people have installed> -NotesFile <the notes>`.
     The script checks (`tools\check-update-keys.ps1`) that the key about to sign is exactly the `BACKUP_PUBKEY` of that release (read from git), and that the new
     source brings in a new `PUBKEY` and a new `BACKUP_PUBKEY`, neither of them the key that signs.
  4. Publish as usual (steps 2 to 4 of "Making a release"). An installed program finds that the main key does not match, accepts the backup key, installs the
     release, and from then on trusts the new pair.
  5. Retire the old backup key: keep it offline, never sign anything with it again. Keep the new backup key offline too, and test both new passwords.
  A normal release signed with the backup key is refused by the build script, and so is a rotation signed with anything else.

## Making a release that the button offers

0. Raise the version (`app/src-tauri/tauri.conf.json`, `Cargo.toml` and `Cargo.lock`), commit everything, and tag the commit
   (`git tag -a v<version> -m "PrismQDA <version>"`). The build script refuses to run on anything else (`tools/check-release-commit.ps1`, `CLAUDE.md` rule 3).
1. Build with the key: set `TAURI_SIGNING_PRIVATE_KEY_PATH` to the `.key` file and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to its password, then
   `powershell -ExecutionPolicy Bypass -File app\BUILD-INSTALLER.ps1 -Sign -Updates -NotesFile <the notes, as text>`. The script stops if the key is
   missing or if `PUBKEY` is empty or is not the public half of the key (it also warns while no backup key is built in, and refuses the backup key for a normal release). It signs the finished setup file with the update key and writes `latest.json`
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
