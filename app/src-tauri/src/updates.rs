//! Program updates (decision 189): a button, never a timer.
//!
//! PrismQDA makes no network connection of its own, with one exception that the person starts: pressing **Check for updates** in Settings.
//! That asks one address (the update feed, a small JSON file) for the newest version, and sends nothing but what any web request sends
//! (the program's version and the computer's kind, in the request's headers). Nothing is checked in the background, at start, or on a schedule.
//! Downloading and installing is a second step that needs the person's go-ahead, and the download is checked against a signature made with
//! the maintainer's update-signing key (its public half is in this program) before anything runs; a file that does not match is refused.
//!
//! The feed address and the public key are set here when a release is published. While the key is empty the updater is "not set up" and
//! cannot make a connection at all. The check is also refused while the open project has the offline lock on (the lock promises that
//! nothing leaves the computer through PrismQDA), when this is a portable copy (it is never updated in place; a new zip is downloaded
//! instead), and in any window that is not the installed program.
//!
//! Two more protections (decision 205, from the security review of beta 3):
//!
//! * **A backup key.** The program can carry a second public key, `BACKUP_PUBKEY`, whose private half is kept offline and signs exactly one
//!   release: the one that rotates the keys (docs/UPDATES.md). If a download does not match the main key's signature, and only then, it is tried
//!   once against the backup key. A network failure is never retried this way. While `BACKUP_PUBKEY` is empty the program behaves as it did before.
//! * **The version is bound to the signed file.** The feed is not signed, only the installer is, so a feed could pair an older, genuinely signed
//!   installer with a newer version number. The signature's trusted comment (which the signature covers) names the signed file, and an update is
//!   refused unless that name is exactly `PrismQDA_<version>_x64-setup.exe` and the version is newer than the running program.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_updater::{Error as UpdaterError, Update, Updater, UpdaterExt};

/// Where the newest version is announced: a small JSON file in the project's public repository (the `update-feed` release holds a
/// `latest.json` that each release replaces). The updater adds nothing to this address.
pub const ENDPOINT: &str = "https://github.com/jasonswrench-beep/prismqda/releases/download/update-feed/latest.json";

/// The public half of the update-signing key (the text of the `.pub` file `tools/make-update-key.ps1` makes). **Empty until that is done**,
/// and then the updater is "not set up": it cannot connect to anything.
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDhGMTAyRjRCNTRCNDE3M0EKUldRNkY3UlVTeThRanhLandFU0Z4anBQQkhDVmZJQUMwUHpwTUpxbmJFZTVNOFRLK3RYdjQzYmYK";

/// The public half of the **backup** update-signing key (the text of the `.pub` file that `tools/make-update-key.ps1 -Backup` makes). The private
/// half is kept offline and signs one release only: the one that rotates the keys (docs/UPDATES.md, "Rotating the keys"). **Empty until the owner has
/// made that key**, and while it is empty the program trusts the main key alone, exactly as before.
pub const BACKUP_PUBKEY: &str = "";

/// Where to look and what to trust. The built-in one is `Feed::built_in()`; the tests make their own, pointed at a server on this computer.
#[derive(Clone, Debug)]
pub struct Feed {
    pub endpoint: String,
    pub pubkey: String,
    /// The backup key, or empty for none. Used only after the main key's signature check has failed.
    pub backup_pubkey: String,
}

impl Feed {
    pub fn built_in() -> Feed {
        Feed { endpoint: ENDPOINT.to_string(), pubkey: PUBKEY.to_string(), backup_pubkey: BACKUP_PUBKEY.to_string() }
    }

    /// True when both the address and the key are present. Without them nothing is ever requested.
    pub fn is_set_up(&self) -> bool {
        !self.endpoint.trim().is_empty() && !self.pubkey.trim().is_empty()
    }

    /// True when a backup key is built in.
    pub fn has_backup(&self) -> bool {
        !self.backup_pubkey.trim().is_empty()
    }

    /// The name of the computer the address is on, for saying who is asked.
    pub fn host(&self) -> String {
        tauri::Url::parse(&self.endpoint).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
    }
}

/// Why an update cannot be checked or installed now. The window words each one itself (so its words go through the language catalog).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// No feed address or no public key in this build.
    NotSetUp,
    /// The open project has the offline lock on.
    OfflineLock,
}

impl Blocked {
    fn code(self) -> &'static str {
        match self {
            Blocked::NotSetUp => "not_set_up",
            Blocked::OfflineLock => "offline_lock",
        }
    }
}

pub fn blocked(feed: &Feed, offline_lock: bool) -> Option<Blocked> {
    if !feed.is_set_up() {
        Some(Blocked::NotSetUp)
    } else if offline_lock {
        Some(Blocked::OfflineLock)
    } else {
        None
    }
}

/// Whether the open project has the offline lock on (a window with no project open has none).
pub fn offline_lock_of(engine: &Mutex<Api>) -> bool {
    let Ok(mut api) = engine.lock() else { return false };
    api.call("project.info", Value::Null).ok().and_then(|info| info["offline_lock"].as_bool()).unwrap_or(false)
}

fn problem(code: &str, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message)
}

/// What the Updates part of Settings shows before anything is asked: the version, who would be asked, and why it cannot be asked now.
pub fn state<R: Runtime>(app: &AppHandle<R>, feed: &Feed, offline_lock: bool, portable: bool) -> Value {
    json!({
        "version": app.package_info().version.to_string(),
        "set_up": feed.is_set_up(),
        "host": feed.host(),
        "portable": portable,
        "blocked": blocked(feed, offline_lock).map(Blocked::code),
    })
}

/// What to do just before the installer replaces the program (closing the open project). Shared by the main and the backup updater.
type BeforeExit = Arc<dyn Fn() + Send + Sync>;

/// Builds the updater for a feed and one public key: the address, the key, a timeout, and what to do just before the installer replaces the program.
fn updater<R: Runtime>(app: &AppHandle<R>, feed: &Feed, pubkey: &str, before_exit: BeforeExit) -> Result<Updater, ApiError> {
    let url = tauri::Url::parse(&feed.endpoint).map_err(|e| problem("update_failed", format!("the update address is not valid ({e})")))?;
    app.updater_builder()
        .pubkey(pubkey.trim().to_string())
        .endpoints(vec![url])
        .map_err(|e| problem("update_failed", e.to_string()))?
        .timeout(Duration::from_secs(20))
        .on_before_exit(move || before_exit())
        .build()
        .map_err(|e| problem("update_failed", e.to_string()))
}

/// The name Tauri's signer gives the setup file of a version, and so the name in the signature's trusted comment (`file:`).
pub fn setup_name(version: &str) -> String {
    format!("PrismQDA_{version}_x64-setup.exe")
}

/// The `file:` value in the trusted comment of a signature, given the text of a `.sig` file (base64 of a minisign signature).
/// This only *reads* the comment. It is the download's signature check (made by the updater plugin against the main or the backup key) that
/// proves the comment is genuine: minisign's global signature covers it, so a changed comment fails that check.
pub fn signed_file_name(signature: &str) -> Result<String, String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(signature.trim())
        .map_err(|_| "the signature in the update feed is not valid".to_string())?;
    let text = String::from_utf8(decoded).map_err(|_| "the signature in the update feed is not valid".to_string())?;
    let parsed = minisign_verify::Signature::decode(&text).map_err(|_| "the signature in the update feed is not valid".to_string())?;
    parsed
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("file:"))
        .map(str::to_string)
        .ok_or_else(|| "the signature in the update feed does not say which file it is for".to_string())
}

/// Whether an offered update is the one it says it is: a version newer than the running one, whose signature was made for exactly the setup file
/// of that version. So an older installer, genuinely signed, cannot be offered under a newer version number. (Plain words: they go into the
/// window as "The update was not installed (...)".)
pub fn bound(offered_version: &str, running: &semver::Version, signature: &str) -> Result<(), String> {
    let offered = semver::Version::parse(offered_version.trim_start_matches('v'))
        .map_err(|_| format!("the version in the update feed ({offered_version}) is not a version number"))?;
    if offered <= *running {
        return Err(format!("the feed offers version {offered}, which is not newer than this program ({running})"));
    }
    let signed = signed_file_name(signature)?;
    let expected = setup_name(offered_version);
    if signed != expected {
        return Err(format!("the file offered is not the one announced: its signature is for {signed}, but version {offered_version} would be {expected}"));
    }
    Ok(())
}

fn check_bound<R: Runtime>(app: &AppHandle<R>, update: &Update) -> Result<(), ApiError> {
    bound(&update.version, &app.package_info().version, &update.signature).map_err(|why| problem("update_failed", why))
}

fn noop() -> BeforeExit {
    Arc::new(|| {})
}

/// Asks the feed whether a version newer than this one exists. This is the one request, and it is made only when this is called.
pub async fn check_update<R: Runtime>(app: &AppHandle<R>, feed: &Feed) -> Result<Option<Update>, ApiError> {
    let up = updater(app, feed, &feed.pubkey, noop())?;
    up.check().await.map_err(|e| problem("update_failed", e.to_string()))
}

/// What the window is told about a newer version: its number, its notes, and the day.
pub fn describe(update: &Update) -> Value {
    json!({
        "available": true,
        "version": update.version,
        "notes": update.body.clone().unwrap_or_default(),
        "date": update.date.map(|d| d.to_string()).unwrap_or_default(),
    })
}

/// The Check button: refused while blocked; otherwise one request to the feed.
pub async fn check<R: Runtime>(app: &AppHandle<R>, feed: &Feed, offline_lock: bool) -> Result<Value, ApiError> {
    if let Some(why) = blocked(feed, offline_lock) {
        return Err(refusal(why));
    }
    match check_update(app, feed).await? {
        Some(update) => {
            // An offer that could not be installed is not announced: the same check the Install button makes, made before the person is asked.
            check_bound(app, &update)?;
            Ok(describe(&update))
        }
        None => Ok(json!({ "available": false })),
    }
}

fn refusal(why: Blocked) -> ApiError {
    match why {
        Blocked::NotSetUp => problem("not_set_up", "this version of PrismQDA is not set up to check for updates"),
        Blocked::OfflineLock => problem("offline_lock", "the offline lock is on for the open project, so PrismQDA does not connect to anything"),
    }
}

/// The Install button: checks again (the feed may have moved on), downloads, verifies the signature, closes the open project, and runs the
/// installer, which replaces the program. The program ends when the installer starts. A portable copy is never updated this way.
pub async fn install<R: Runtime>(app: &AppHandle<R>, feed: &Feed, offline_lock: bool, portable: bool, engine: Arc<Mutex<Api>>) -> Result<Value, ApiError> {
    if let Some(why) = blocked(feed, offline_lock) {
        return Err(refusal(why));
    }
    if portable {
        return Err(problem("portable", "a portable copy is not updated in place"));
    }
    let closer = Arc::clone(&engine);
    let before_exit: BeforeExit = Arc::new(move || {
        // The installer replaces the program: the project is closed first, so the one file holds everything.
        let mut api = closer.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = api.call("project.close", Value::Null);
    });
    let handle = app.clone();
    let progress = move |received: u64, total: Option<u64>| {
        let _ = handle.emit("update-progress", json!({ "received": received, "total": total }));
    };
    let Some(found) = fetch_verified(app, feed, before_exit, progress).await? else {
        return Ok(json!({ "installed": false }));
    };
    found.update.install(&found.bytes).map_err(|e| problem("update_failed", e.to_string()))?;
    Ok(json!({ "installed": true, "version": found.update.version }))
}

/// A setup file that passed its signature check, with the update it belongs to.
pub struct Found {
    pub update: Update,
    pub bytes: Vec<u8>,
    /// True when the main key did not match and the backup key did (a rotation release).
    pub used_backup: bool,
}

/// Finds the newest version, makes sure it is the one it claims to be (`bound`), downloads it and checks its signature. The main key is tried
/// first. Only if that check fails *as a signature check* (not a network error, not a bad version) and a backup key is built in, the same is done
/// once more with the backup key. The setup file is not run here: `install` does that with the bytes returned.
pub async fn fetch_verified<R: Runtime, P: FnMut(u64, Option<u64>) + Send>(
    app: &AppHandle<R>,
    feed: &Feed,
    before_exit: BeforeExit,
    mut progress: P,
) -> Result<Option<Found>, ApiError> {
    let up = updater(app, feed, &feed.pubkey, Arc::clone(&before_exit))?;
    let Some(update) = up.check().await.map_err(|e| problem("update_failed", e.to_string()))? else {
        return Ok(None);
    };
    check_bound(app, &update)?;
    let mut received: u64 = 0;
    let first = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                progress(received, total);
            },
            || {},
        )
        .await;
    match first {
        Ok(bytes) => Ok(Some(Found { update, bytes, used_backup: false })),
        Err(UpdaterError::Minisign(_)) if feed.has_backup() => {
            let up = updater(app, feed, &feed.backup_pubkey, before_exit)?;
            let Some(update) = up.check().await.map_err(|e| problem("update_failed", e.to_string()))? else {
                return Err(problem("update_failed", "the update is no longer offered"));
            };
            check_bound(app, &update)?;
            let mut received: u64 = 0;
            let bytes = update
                .download(
                    |chunk, total| {
                        received += chunk as u64;
                        progress(received, total);
                    },
                    || {},
                )
                .await
                .map_err(|e| problem("update_failed", format!("the download is not signed by a key this program trusts ({e})")))?;
            Ok(Some(Found { update, bytes, used_backup: true }))
        }
        Err(e) => Err(problem("update_failed", e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

    // Test-only keys, made for these tests and then discarded (only the public halves and the signatures are kept; tests/fixtures/README.md).
    // PUB is the "main" key, BACKUP_PUB the backup key, OTHER_PUB a third key that neither trusts. All four signatures are of SETUP.
    const PUB: &str = include_str!("../tests/fixtures/update-test.pub");
    const BACKUP_PUB: &str = include_str!("../tests/fixtures/update-backup.pub");
    const OTHER_PUB: &str = include_str!("../tests/fixtures/update-other.pub");
    /// Main key, for the file `PrismQDA_9.9.9_x64-setup.exe`.
    const SIGNATURE: &str = include_str!("../tests/fixtures/update-setup.bin.sig");
    /// Main key, but for the file `PrismQDA_0.1.0-beta.3_x64-setup.exe`: a genuine signature of an older installer.
    const SIGNATURE_OLD: &str = include_str!("../tests/fixtures/update-setup-old.bin.sig");
    /// Backup key, for `PrismQDA_9.9.9_x64-setup.exe`.
    const SIGNATURE_BACKUP: &str = include_str!("../tests/fixtures/update-setup-backup.bin.sig");
    /// A third key, for `PrismQDA_9.9.9_x64-setup.exe`.
    const SIGNATURE_THIRD: &str = include_str!("../tests/fixtures/update-setup-third.bin.sig");
    const SETUP: &[u8] = include_bytes!("../tests/fixtures/update-setup.bin");

    fn app() -> tauri::App<MockRuntime> {
        // The same plugin configuration as `tauri.conf.json`: the address and the key are given in code (`Feed`), not here.
        let mut context = mock_context(noop_assets());
        context.config_mut().plugins.0.insert("updater".into(), json!({ "pubkey": "", "endpoints": [], "windows": { "installMode": "passive" } }));
        mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).expect("the mock app builds")
    }

    /// A feed on this computer: `/latest.json` announces `version` with the fixture's signature, and `/setup.exe` gives `bytes`.
    /// The server answers requests until the test ends and counts the ones it gets.
    struct Server {
        base: String,
        requests: Arc<Mutex<Vec<String>>>,
    }

    fn serve(version: &'static str, bytes: &'static [u8], status_for_feed: u16) -> Server {
        serve_signed(version, bytes.to_vec(), SIGNATURE.trim().to_string(), status_for_feed)
    }

    /// The same, for a file and a signature given at run time (a setup file that was really built).
    fn serve_signed(version: &str, bytes: Vec<u8>, signature: String, status_for_feed: u16) -> Server {
        serve_full(version, bytes, signature, status_for_feed, 200)
    }

    /// The same, and the setup file itself is answered with `status_for_setup` (200 gives the file; anything else is a failed download).
    fn serve_full(version: &str, bytes: Vec<u8>, signature: String, status_for_feed: u16, status_for_setup: u16) -> Server {
        let version = version.to_string();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", server.server_addr().to_ip().unwrap().port());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        let manifest = json!({
            "version": version,
            "notes": "Fixes for the test.",
            "pub_date": "2026-11-13T12:00:00Z",
            "platforms": { "windows-x86_64": { "signature": signature, "url": format!("{base}/setup.exe") } },
        })
        .to_string();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let url = request.url().to_string();
                log.lock().unwrap().push(url.clone());
                let response = if url == "/latest.json" && status_for_feed == 200 {
                    tiny_http::Response::from_string(manifest.clone()).with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap()).boxed()
                } else if url == "/setup.exe" && status_for_setup == 200 {
                    tiny_http::Response::from_data(bytes.clone()).boxed()
                } else if url == "/setup.exe" {
                    tiny_http::Response::from_string("the download failed").with_status_code(status_for_setup).boxed()
                } else {
                    tiny_http::Response::from_string("not here").with_status_code(status_for_feed.max(404)).boxed()
                };
                let _ = request.respond(response);
            }
        });
        Server { base, requests }
    }

    fn feed(server: &Server, pubkey: &str) -> Feed {
        Feed { endpoint: format!("{}/latest.json", server.base), pubkey: pubkey.trim().to_string(), backup_pubkey: String::new() }
    }

    /// A feed that also carries a backup key.
    fn feed_with_backup(server: &Server, pubkey: &str, backup: &str) -> Feed {
        Feed { backup_pubkey: backup.trim().to_string(), ..feed(server, pubkey) }
    }

    fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(f)
    }

    #[test]
    fn a_build_with_no_key_or_no_address_is_not_set_up_and_never_connects() {
        let app = app();
        let server = serve("9.9.9", SETUP, 200);
        for feed in [
            Feed { endpoint: String::new(), pubkey: PUB.trim().to_string(), backup_pubkey: BACKUP_PUB.trim().to_string() },
            Feed { endpoint: format!("{}/latest.json", server.base), pubkey: String::new(), backup_pubkey: BACKUP_PUB.trim().to_string() },
            Feed { endpoint: "  ".into(), pubkey: "  ".into(), backup_pubkey: String::new() },
        ] {
            assert!(!feed.is_set_up());
            let state = state(app.handle(), &feed, false, false);
            assert_eq!(state["blocked"], "not_set_up");
            let error = block_on(check(app.handle(), &feed, false)).unwrap_err();
            assert_eq!(error.code, "not_set_up");
        }
        assert!(server.requests.lock().unwrap().is_empty(), "nothing was asked of the server");
        // The built-in feed is not set up until the maintainer's key is put in (the first release does it); this says so while it is empty.
        assert_eq!(Feed::built_in().is_set_up(), !PUBKEY.is_empty());
        assert!(Feed::built_in().endpoint.starts_with("https://"), "the address is a secure one");
    }

    #[test]
    fn the_offline_lock_stops_a_check_before_any_connection_is_made() {
        let app = app();
        let server = serve("9.9.9", SETUP, 200);
        let feed = feed(&server, PUB);
        assert_eq!(state(app.handle(), &feed, true, false)["blocked"], "offline_lock");
        assert_eq!(block_on(check(app.handle(), &feed, true)).unwrap_err().code, "offline_lock");
        assert_eq!(block_on(install(app.handle(), &feed, true, false, Arc::new(Mutex::new(Api::new("t"))))).unwrap_err().code, "offline_lock");
        assert!(server.requests.lock().unwrap().is_empty(), "the lock is checked first");
        assert_eq!(state(app.handle(), &feed, false, false)["blocked"], Value::Null);
    }

    #[test]
    fn a_newer_version_is_announced_with_its_notes_and_the_same_or_an_older_one_is_not() {
        let app = app();
        let newer = serve("9.9.9", SETUP, 200);
        let found = block_on(check(app.handle(), &feed(&newer, PUB), false)).unwrap();
        assert_eq!(found["available"], true);
        assert_eq!(found["version"], "9.9.9");
        assert_eq!(found["notes"], "Fixes for the test.");
        assert_eq!(newer.requests.lock().unwrap().as_slice(), ["/latest.json"], "one request, for the feed, and not for the installer");
        let older = serve("0.0.1", SETUP, 200);
        assert_eq!(block_on(check(app.handle(), &feed(&older, PUB), false)).unwrap()["available"], false);
        let current = app.handle().package_info().version.to_string();
        let same: &'static str = Box::leak(current.into_boxed_str());
        assert_eq!(block_on(check(app.handle(), &feed(&serve(same, SETUP, 200), PUB), false)).unwrap()["available"], false, "this very version is not an update");
    }

    #[test]
    fn a_feed_that_is_missing_is_said_in_a_failure_and_changes_nothing() {
        let app = app();
        let missing = serve("9.9.9", SETUP, 404);
        let error = block_on(check(app.handle(), &feed(&missing, PUB), false)).unwrap_err();
        assert_eq!(error.code, "update_failed");
        let nobody = Feed { endpoint: "http://127.0.0.1:1/latest.json".into(), pubkey: PUB.trim().to_string(), backup_pubkey: String::new() };
        assert_eq!(block_on(check(app.handle(), &nobody, false)).unwrap_err().code, "update_failed");
    }

    #[test]
    fn a_download_is_checked_against_the_signature_and_only_the_signed_file_passes() {
        let app = app();
        // The file that was signed, with the key that is built in: accepted, and byte for byte what was published.
        let good = serve("9.9.9", SETUP, 200);
        let update = block_on(check_update(app.handle(), &feed(&good, PUB))).unwrap().expect("a newer version");
        let bytes = block_on(update.download(|_, _| {}, || {})).expect("the signed file passes");
        assert_eq!(bytes, SETUP);
        // The same address, but the installer is not the one that was signed (one byte changed): refused.
        let tampered: &'static [u8] = Box::leak({
            let mut b = SETUP.to_vec();
            b[3] ^= 1;
            b.into_boxed_slice()
        });
        let bad = serve("9.9.9", tampered, 200);
        let update = block_on(check_update(app.handle(), &feed(&bad, PUB))).unwrap().expect("announced");
        assert!(block_on(update.download(|_, _| {}, || {})).is_err(), "a file that does not match its signature is not accepted");
        // The right file but the program trusts another key: refused.
        let update = block_on(check_update(app.handle(), &feed(&good, OTHER_PUB))).unwrap().expect("announced");
        assert!(block_on(update.download(|_, _| {}, || {})).is_err(), "a signature from another key is not accepted");
    }

    /// Run by hand against a setup file that `app\BUILD-INSTALLER.ps1 -Sign -Updates` built (and a build whose `PUBKEY` is the public half of the key
    /// that signed it): the real file, signed by Azure and then by the update key, must pass, and the same file with one byte changed must not.
    /// `PRISMQDA_REAL_SETUP_DIR` is the folder with the setup file and its `latest.json`.
    #[test]
    #[ignore = "needs a setup file built with -Updates; set PRISMQDA_REAL_SETUP_DIR to its folder"]
    fn a_real_built_setup_passes_its_update_signature_and_a_changed_byte_does_not() {
        let Some(dir) = std::env::var_os("PRISMQDA_REAL_SETUP_DIR") else { return };
        assert!(!PUBKEY.is_empty(), "this build has no public key to check with");
        let dir = std::path::PathBuf::from(dir);
        let manifest: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();
        let platform = &manifest["platforms"]["windows-x86_64"];
        let signature = platform["signature"].as_str().unwrap().to_string();
        let name = platform["url"].as_str().unwrap().rsplit('/').next().unwrap().to_string();
        let bytes = std::fs::read(dir.join(&name)).unwrap();
        let app = app();
        let feed = |server: &Server| Feed { endpoint: format!("{}/latest.json", server.base), pubkey: PUBKEY.to_string(), backup_pubkey: BACKUP_PUBKEY.to_string() };

        let good = serve_signed("99.0.0", bytes.clone(), signature.clone(), 200);
        let update = block_on(check_update(app.handle(), &feed(&good))).unwrap().expect("a newer version");
        let got = block_on(update.download(|_, _| {}, || {})).expect("the file that was built passes its signature");
        assert_eq!(got.len(), bytes.len());
        assert_eq!(got, bytes);

        let mut changed = bytes;
        let middle = changed.len() / 2;
        changed[middle] ^= 1;
        let bad = serve_signed("99.0.0", changed, signature, 200);
        let update = block_on(check_update(app.handle(), &feed(&bad))).unwrap().expect("announced");
        assert!(block_on(update.download(|_, _| {}, || {})).is_err(), "one changed byte is refused");
    }

    #[test]
    fn a_portable_copy_is_not_updated_in_place() {
        let app = app();
        let server = serve("9.9.9", SETUP, 200);
        let error = block_on(install(app.handle(), &feed(&server, PUB), false, true, Arc::new(Mutex::new(Api::new("t"))))).unwrap_err();
        assert_eq!(error.code, "portable");
        assert!(server.requests.lock().unwrap().is_empty());
        assert_eq!(state(app.handle(), &feed(&server, PUB), false, true)["portable"], true);
    }

    #[test]
    fn what_a_build_asks_is_one_address_over_a_secure_connection() {
        // The release build refuses a feed that is not https; the built-in address is.
        assert!(ENDPOINT.starts_with("https://github.com/"));
        let f = Feed::built_in();
        assert_eq!(f.host(), "github.com");
    }

    // ---- the backup key (decision 205) ----

    fn fetch(app: &tauri::App<MockRuntime>, feed: &Feed) -> Result<Option<Found>, ApiError> {
        block_on(fetch_verified(app.handle(), feed, noop(), |_, _| {}))
    }

    fn asked(server: &Server) -> Vec<String> {
        server.requests.lock().unwrap().clone()
    }

    #[test]
    fn a_file_signed_by_the_main_key_is_taken_and_the_backup_key_is_never_tried() {
        let app = app();
        let server = serve("9.9.9", SETUP, 200);
        let found = fetch(&app, &feed_with_backup(&server, PUB, BACKUP_PUB)).unwrap().expect("an update");
        assert_eq!(found.bytes, SETUP);
        assert!(!found.used_backup);
        assert_eq!(found.update.version, "9.9.9");
        assert_eq!(asked(&server), ["/latest.json", "/setup.exe"], "one check and one download");
    }

    #[test]
    fn a_file_signed_by_the_backup_key_is_taken_when_the_main_key_does_not_match() {
        let app = app();
        let server = serve_signed("9.9.9", SETUP.to_vec(), SIGNATURE_BACKUP.trim().to_string(), 200);
        let found = fetch(&app, &feed_with_backup(&server, PUB, BACKUP_PUB)).unwrap().expect("an update");
        assert_eq!(found.bytes, SETUP, "byte for byte what was signed");
        assert!(found.used_backup);
        assert_eq!(
            asked(&server),
            ["/latest.json", "/setup.exe", "/latest.json", "/setup.exe"],
            "the main key failed its signature check once, and the backup key was then tried once"
        );
    }

    #[test]
    fn a_file_signed_by_a_third_key_is_refused_even_with_a_backup_key() {
        let app = app();
        let server = serve_signed("9.9.9", SETUP.to_vec(), SIGNATURE_THIRD.trim().to_string(), 200);
        let error = fetch(&app, &feed_with_backup(&server, PUB, BACKUP_PUB)).err().expect("refused");
        assert_eq!(error.code, "update_failed");
        assert!(error.message.contains("not signed by a key this program trusts"), "{}", error.message);
        assert_eq!(asked(&server).len(), 4, "each key was tried once, and no more");
    }

    #[test]
    fn with_no_backup_key_a_file_signed_by_the_backup_key_is_refused_as_before() {
        let app = app();
        let server = serve_signed("9.9.9", SETUP.to_vec(), SIGNATURE_BACKUP.trim().to_string(), 200);
        let feed = feed(&server, PUB);
        assert!(!feed.has_backup());
        let error = fetch(&app, &feed).err().expect("refused");
        assert_eq!(error.code, "update_failed");
        assert!(!error.message.contains("trusts"), "the words are the updater's own: {}", error.message);
        assert_eq!(asked(&server), ["/latest.json", "/setup.exe"], "no second try without a backup key");
    }

    #[test]
    fn a_failed_download_is_not_retried_with_the_backup_key() {
        let app = app();
        let server = serve_full("9.9.9", SETUP.to_vec(), SIGNATURE.trim().to_string(), 200, 500);
        let error = fetch(&app, &feed_with_backup(&server, PUB, BACKUP_PUB)).err().expect("the download failed");
        assert_eq!(error.code, "update_failed");
        assert_eq!(asked(&server), ["/latest.json", "/setup.exe"], "a network failure is not a signature failure");
    }

    #[test]
    fn a_broken_backup_key_cannot_hurt_the_main_path_and_never_lets_anything_through() {
        let app = app();
        let good = serve("9.9.9", SETUP, 200);
        let found = fetch(&app, &feed_with_backup(&good, PUB, "not a key at all")).unwrap().expect("an update");
        assert!(!found.used_backup);
        let backup_signed = serve_signed("9.9.9", SETUP.to_vec(), SIGNATURE_BACKUP.trim().to_string(), 200);
        assert!(fetch(&app, &feed_with_backup(&backup_signed, PUB, "not a key at all")).is_err());
    }

    #[test]
    fn an_empty_backup_key_behaves_exactly_as_today() {
        let app = app();
        let server = serve("9.9.9", SETUP, 200);
        let plain = feed(&server, PUB);
        assert!(plain.is_set_up() && !plain.has_backup());
        let found = fetch(&app, &plain).unwrap().expect("an update");
        assert_eq!((found.bytes.as_slice(), found.used_backup), (SETUP, false));
        // A backup key alone does not set the updater up: the main key is what makes it work.
        let only_backup = Feed { pubkey: String::new(), ..feed_with_backup(&server, PUB, BACKUP_PUB) };
        assert!(!only_backup.is_set_up());
        assert_eq!(state(app.handle(), &only_backup, false, false)["blocked"], "not_set_up");
        // The built-in feed carries whatever is built in, and is the same shape either way.
        assert_eq!(Feed::built_in().has_backup(), !BACKUP_PUBKEY.is_empty());
    }

    #[test]
    fn the_keys_built_in_are_real_minisign_public_keys_and_two_different_ones() {
        for (name, key) in [("PUBKEY", PUBKEY), ("BACKUP_PUBKEY", BACKUP_PUBKEY)] {
            if key.is_empty() {
                continue;
            }
            let text = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(key.trim()).unwrap_or_else(|_| panic!("{name} is not base64"))).unwrap();
            assert!(minisign_verify::PublicKey::decode(&text).is_ok(), "{name} is not a minisign public key");
        }
        assert!(BACKUP_PUBKEY.is_empty() || BACKUP_PUBKEY.trim() != PUBKEY.trim(), "the backup key must not be the main key");
    }

    // ---- the version is bound to the signed file (decision 205) ----

    #[test]
    fn an_old_installer_offered_under_a_new_version_number_is_refused_before_any_download() {
        let app = app();
        // A genuine signature of PrismQDA_0.1.0-beta.3_x64-setup.exe, offered by a feed that calls the file version 9.9.9.
        let server = serve_signed("9.9.9", SETUP.to_vec(), SIGNATURE_OLD.trim().to_string(), 200);
        let error = block_on(check(app.handle(), &feed(&server, PUB), false)).unwrap_err();
        assert_eq!(error.code, "update_failed");
        assert!(error.message.contains("PrismQDA_0.1.0-beta.3_x64-setup.exe") && error.message.contains("PrismQDA_9.9.9_x64-setup.exe"), "{}", error.message);
        let error = fetch(&app, &feed_with_backup(&server, PUB, BACKUP_PUB)).err().expect("refused");
        assert_eq!(error.code, "update_failed");
        assert!(!asked(&server).iter().any(|u| u == "/setup.exe"), "nothing was downloaded: {:?}", asked(&server));
    }

    #[test]
    fn a_signature_whose_comment_was_changed_to_name_the_right_file_is_refused_by_the_signature_check() {
        let app = app();
        let decoded = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(SIGNATURE_OLD.trim()).unwrap()).unwrap();
        let altered = decoded.replace("file:PrismQDA_0.1.0-beta.3_x64-setup.exe", "file:PrismQDA_9.9.9_x64-setup.exe");
        assert_ne!(altered, decoded, "the comment was changed");
        let forged = base64::engine::general_purpose::STANDARD.encode(altered);
        // The name alone would pass the version check ...
        assert_eq!(signed_file_name(&forged).unwrap(), "PrismQDA_9.9.9_x64-setup.exe");
        assert!(bound("9.9.9", &semver::Version::new(0, 1, 0), &forged).is_ok());
        // ... but minisign's global signature covers the comment, so the signature check refuses the file, with either key.
        let server = serve_signed("9.9.9", SETUP.to_vec(), forged, 200);
        assert!(fetch(&app, &feed(&server, PUB)).is_err(), "a changed trusted comment is refused");
        assert!(fetch(&app, &feed_with_backup(&server, PUB, BACKUP_PUB)).is_err(), "and so with a backup key");
    }

    #[test]
    fn the_signed_file_name_and_a_newer_version_are_both_needed() {
        let running = semver::Version::parse("0.1.0-beta.3").unwrap();
        let for_999 = SIGNATURE.trim();
        assert_eq!(signed_file_name(for_999).unwrap(), "PrismQDA_9.9.9_x64-setup.exe");
        assert_eq!(setup_name("0.1.0-beta.4"), "PrismQDA_0.1.0-beta.4_x64-setup.exe");
        assert!(bound("9.9.9", &running, for_999).is_ok(), "the file named for the version, and newer");
        assert!(bound("0.1.0-beta.4", &running, for_999).is_err(), "a signature for another file name");
        assert!(bound("9.9.9", &running, SIGNATURE_OLD.trim()).is_err(), "a genuine signature of an older installer");
        assert!(bound("0.1.0-beta.3", &semver::Version::parse("0.1.0-beta.3").unwrap(), for_999).is_err(), "the same version is not an update");
        assert!(bound("0.1.0-beta.2", &running, for_999).is_err(), "an older version is not an update");
        assert!(bound("not a version", &running, for_999).is_err());
        assert!(bound("9.9.9", &running, "this is not base64 !!").is_err());
        assert!(bound("9.9.9", &running, "").is_err());
        let decoded = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(for_999).unwrap()).unwrap();
        let nameless = base64::engine::general_purpose::STANDARD.encode(decoded.replace("\tfile:", "\tfilx:"));
        assert!(signed_file_name(&nameless).unwrap_err().contains("does not say which file"));
    }
}
