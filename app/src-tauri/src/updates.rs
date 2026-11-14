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

use std::sync::{Arc, Mutex};
use std::time::Duration;

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Where the newest version is announced: a small JSON file in the project's public repository (the `update-feed` release holds a
/// `latest.json` that each release replaces). The updater adds nothing to this address.
pub const ENDPOINT: &str = "https://github.com/jasonswrench-beep/prismqda/releases/download/update-feed/latest.json";

/// The public half of the update-signing key (the text of the `.pub` file `tools/make-update-key.ps1` makes). **Empty until that is done**,
/// and then the updater is "not set up": it cannot connect to anything.
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUzQkU4RUY1MkMxMURGQkEKUldTNjN4RXM5WTYrNHdWeU9mWkdpUTBWb2w3ek5YMy9YWFpJKzJKSGlxcUtQZTdmK1NDeW5oc28K";

/// Where to look and what to trust. The built-in one is `Feed::built_in()`; the tests make their own, pointed at a server on this computer.
#[derive(Clone, Debug)]
pub struct Feed {
    pub endpoint: String,
    pub pubkey: String,
}

impl Feed {
    pub fn built_in() -> Feed {
        Feed { endpoint: ENDPOINT.to_string(), pubkey: PUBKEY.to_string() }
    }

    /// True when both the address and the key are present. Without them nothing is ever requested.
    pub fn is_set_up(&self) -> bool {
        !self.endpoint.trim().is_empty() && !self.pubkey.trim().is_empty()
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

/// Builds the updater for a feed: the address, the key, a timeout, and what to do just before the installer replaces the program.
fn updater<R: Runtime>(app: &AppHandle<R>, feed: &Feed, before_exit: impl Fn() + Send + Sync + 'static) -> Result<tauri_plugin_updater::Updater, ApiError> {
    let url = tauri::Url::parse(&feed.endpoint).map_err(|e| problem("update_failed", format!("the update address is not valid ({e})")))?;
    app.updater_builder()
        .pubkey(feed.pubkey.clone())
        .endpoints(vec![url])
        .map_err(|e| problem("update_failed", e.to_string()))?
        .timeout(Duration::from_secs(20))
        .on_before_exit(before_exit)
        .build()
        .map_err(|e| problem("update_failed", e.to_string()))
}

/// Asks the feed whether a version newer than this one exists. This is the one request, and it is made only when this is called.
pub async fn check_update<R: Runtime>(app: &AppHandle<R>, feed: &Feed) -> Result<Option<Update>, ApiError> {
    let up = updater(app, feed, || {})?;
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
        Some(update) => Ok(describe(&update)),
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
    let up = updater(app, feed, move || {
        // The installer replaces the program: the project is closed first, so the one file holds everything.
        let mut api = closer.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = api.call("project.close", Value::Null);
    })?;
    let Some(update) = up.check().await.map_err(|e| problem("update_failed", e.to_string()))? else {
        return Ok(json!({ "installed": false }));
    };
    let handle = app.clone();
    let mut received: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                received += chunk as u64;
                let _ = handle.emit("update-progress", json!({ "received": received, "total": total }));
            },
            || {},
        )
        .await
        .map_err(|e| problem("update_failed", e.to_string()))?;
    Ok(json!({ "installed": true, "version": update.version }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

    const PUB: &str = include_str!("../tests/fixtures/update-test.pub");
    const OTHER_PUB: &str = include_str!("../tests/fixtures/update-other.pub");
    const SIGNATURE: &str = include_str!("../tests/fixtures/update-setup.bin.sig");
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
                } else if url == "/setup.exe" {
                    tiny_http::Response::from_data(bytes.clone()).boxed()
                } else {
                    tiny_http::Response::from_string("not here").with_status_code(status_for_feed.max(404)).boxed()
                };
                let _ = request.respond(response);
            }
        });
        Server { base, requests }
    }

    fn feed(server: &Server, pubkey: &str) -> Feed {
        Feed { endpoint: format!("{}/latest.json", server.base), pubkey: pubkey.trim().to_string() }
    }

    fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(f)
    }

    #[test]
    fn a_build_with_no_key_or_no_address_is_not_set_up_and_never_connects() {
        let app = app();
        let server = serve("9.9.9", SETUP, 200);
        for feed in [Feed { endpoint: String::new(), pubkey: PUB.trim().to_string() }, Feed { endpoint: format!("{}/latest.json", server.base), pubkey: String::new() }, Feed { endpoint: "  ".into(), pubkey: "  ".into() }] {
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
        let nobody = Feed { endpoint: "http://127.0.0.1:1/latest.json".into(), pubkey: PUB.trim().to_string() };
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
        let feed = |server: &Server| Feed { endpoint: format!("{}/latest.json", server.base), pubkey: PUBKEY.to_string() };

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
}
