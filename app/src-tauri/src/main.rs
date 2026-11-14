//! The PrismQDA desktop app: a window around the React interface, with one door to the engine.
//!
//! The interface calls `invoke('call', { cmd, args })`; this file hands it to the same command layer the
//! development bridge and the tests use, so what ships is what was tested. Nothing here decides anything.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod updates;

use prismqda_api::{Api, ApiError, MediaReply};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

/// The engine, shared by every command. One project is open at a time, so one lock is enough.
struct Engine(Arc<Mutex<Api>>);

/// Runs one engine command. Async so a long import never freezes the window. The few commands that start with `update.` are the
/// window's own (the Check for updates button, see `updates.rs`) and never reach the engine, which makes no network connection.
#[tauri::command]
async fn call<R: tauri::Runtime>(app: tauri::AppHandle<R>, cmd: String, args: Option<Value>, engine: State<'_, Engine>) -> Result<Value, ApiError> {
    if let Some(what) = cmd.strip_prefix("update.") {
        return handle_update(&app, what, &engine).await;
    }
    let mut api = engine.0.lock().map_err(|_| ApiError::new("internal", "the engine stopped after an earlier failure; close and reopen PrismQDA"))?;
    api.call(&cmd, args.unwrap_or(Value::Null))
}

/// `update.state`, `update.check` and `update.install`.
async fn handle_update<R: tauri::Runtime>(app: &tauri::AppHandle<R>, what: &str, engine: &Engine) -> Result<Value, ApiError> {
    let feed = updates::Feed::built_in();
    let locked = updates::offline_lock_of(&engine.0);
    let portable = is_portable();
    match what {
        "state" => Ok(updates::state(app, &feed, locked, portable)),
        "check" => updates::check(app, &feed, locked).await,
        "install" => updates::install(app, &feed, locked, portable, Arc::clone(&engine.0)).await,
        other => Err(ApiError::new("unknown_command", format!("there is no command update.{other}"))),
    }
}

/// True for a portable copy (the marker file is beside the program), which keeps to itself and is never updated in place.
fn is_portable() -> bool {
    std::env::current_exe().ok().and_then(|exe| exe.parent().and_then(portable_data_dir)).is_some()
}

/// The project file the program was started with, or was sent by a second start (a double click on a `.pqda` file in Windows
/// Explorer while PrismQDA is already open). It is held until the window asks for it, so a file sent before the window has
/// drawn its first screen is not lost.
struct LaunchPath(Mutex<Option<String>>);

/// The first of the program's arguments (the first of all is the program itself) that names an existing `.pqda` file. A relative
/// path is read from `base`, the folder the person started the program from. Anything else on the command line is left alone.
fn project_path_from_args(args: &[String], base: &Path) -> Option<PathBuf> {
    args.iter().skip(1).find_map(|arg| {
        let path = Path::new(arg);
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pqda")) {
            return None;
        }
        let full = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
        full.is_file().then_some(full)
    })
}

/// Hands the window the project file it was started with, once. The window asks when it starts and again each time a second
/// start sends one (it is told by the `launch-path` event).
#[tauri::command]
fn launch_path(launch: State<'_, LaunchPath>) -> Option<String> {
    launch.0.lock().ok().and_then(|mut slot| slot.take())
}

/// The name recorded on every change this person makes, until they choose one in Settings: "Researcher" and four letters or digits, made once
/// for this installation and kept in a small file in the program's data folder. It is never the Windows account name, which a project or a
/// copy that is shared would carry on every change; and it differs from one computer to the next, because a merge tells coders apart by name,
/// so two people who both started with "Researcher" would be taken for one coder.
fn person() -> String {
    let dir = std::env::current_exe().ok().and_then(|exe| exe.parent().and_then(portable_data_dir)).or_else(profile_data_dir);
    match dir {
        Some(dir) => automatic_name_in(&dir),
        None => "Researcher".to_string(),
    }
}

/// Where the program keeps its own small files for the person in their Windows profile (the window's data is there too).
fn profile_data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty()).map(|p| PathBuf::from(p).join("io.github.jasonswrench-beep.prismqda"))
}

/// "Researcher" and the four characters kept in `history-name.txt` in `dir`, made now when there are none (or none that are right).
fn automatic_name_in(dir: &Path) -> String {
    let file = dir.join("history-name.txt");
    if let Ok(text) = std::fs::read_to_string(&file) {
        let id = text.trim();
        if id.len() == 4 && id.chars().all(|c| c.is_ascii_hexdigit()) {
            return format!("Researcher {}", id.to_ascii_uppercase());
        }
    }
    let id = random_id();
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&file, &id);
    format!("Researcher {id}")
}

/// Four hexadecimal digits that differ from one run to the next (the hasher is seeded by the operating system).
fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    hasher.write_u32(std::process::id());
    format!("{:04X}", hasher.finish() & 0xFFFF)
}

/// What the window's players ask for: a recording, by the number of its document, at `http://media.localhost/<number>` (and a part
/// of it when the player sends a `Range`). Only a recording linked to a document of the open project can be asked for.
fn media_response(api: &Mutex<Api>, path: &str, range: Option<&str>) -> tauri::http::Response<Vec<u8>> {
    let gone = |status: u16, words: &str| MediaReply { status, content_type: "text/plain; charset=utf-8".to_string(), content_range: None, body: words.as_bytes().to_vec(), cacheable: false };
    let wanted = path.trim_matches('/');
    // `pdf/<number>/<page>/<width>` is one page of a PDF, drawn.
    if let Some(rest) = wanted.strip_prefix("pdf/") {
        let reply = match prismqda_api::parse_pdf_page_path(rest) {
            Some((id, page, width)) => match api.lock() {
                Ok(mut api) => api.pdf_page_reply(id, page, width),
                Err(_) => gone(500, "the engine stopped after an earlier failure"),
            },
            None => gone(404, "a page is asked for as pdf/<document>/<page>/<width>"),
        };
        return reply_to_response(reply);
    }
    // `image/<number>` is a picture; a bare number is a recording.
    let (picture, number) = match wanted.strip_prefix("image/") {
        Some(rest) => (true, rest),
        None => (false, wanted),
    };
    let reply = match number.parse::<i64>() {
        Ok(id) => match api.lock() {
            Ok(mut api) => {
                if picture {
                    api.image_reply(id)
                } else {
                    api.media_reply(id, range)
                }
            }
            Err(_) => gone(500, "the engine stopped after an earlier failure"),
        },
        Err(_) => gone(404, "a recording or a picture is asked for by its document's number"),
    };
    reply_to_response(reply)
}

/// What the window is sent for a reply of the engine's.
fn reply_to_response(reply: MediaReply) -> tauri::http::Response<Vec<u8>> {
    let mut response = tauri::http::Response::builder()
        .status(reply.status)
        .header("Content-Type", reply.content_type)
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", if reply.cacheable { "private, max-age=3600" } else { "no-store" })
        .header("Access-Control-Allow-Origin", "*");
    if let Some(range) = reply.content_range {
        response = response.header("Content-Range", range);
    }
    response.body(reply.body).unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

/// Specification 4.1: a backup on open (the engine does that) and one every 15 minutes while a project is open.
fn keep_backing_up(api: Arc<Mutex<Api>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15 * 60));
        if let Ok(mut api) = api.lock() {
            // No project open, or a read-only one, simply has nothing to back up.
            let _ = api.call("project.backup", Value::Null);
        }
    });
}

/// Closes the open project, if there is one. Closing is what makes SQLite fold its write-ahead log back into the
/// project file, so after this the one `.pqda` file holds everything. Without it, work from the last session sits
/// in a `-wal` file beside the project, and a person who copies, emails, or uploads just the `.pqda` loses it.
fn close_project(api: &Mutex<Api>) {
    // A lock poisoned by an earlier failure still guards a usable engine; the point here is to close the file.
    let mut api = api.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // No project open simply has nothing to close.
    let _ = api.call("project.close", Value::Null);
}

/// The marker that makes a copy of the program portable: a file with this name beside the program.
const PORTABLE_MARKER: &str = "portable.txt";

/// Where a portable copy keeps what the window would otherwise keep for the person in their Windows profile (their settings, the list
/// of recent projects, translations they added): a folder beside the program. `None` for an installed program, which uses the profile.
fn portable_data_dir(program_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    program_dir.join(PORTABLE_MARKER).is_file().then(|| program_dir.join("PrismQDAData"))
}

fn main() {
    let api = Arc::new(Mutex::new(Api::new(person())));
    keep_backing_up(Arc::clone(&api));
    let started_with = std::env::current_dir()
        .ok()
        .and_then(|here| project_path_from_args(&std::env::args().collect::<Vec<_>>(), &here))
        .map(|p| p.to_string_lossy().into_owned());
    let portable = std::env::current_exe().ok().and_then(|exe| exe.parent().and_then(portable_data_dir)).is_some();
    let mut builder = tauri::Builder::default();
    // An installed program is one program however many project files are double clicked: a second start hands its file to the
    // window already open and ends. A portable copy keeps to itself, so it can run beside an installed one.
    if !portable {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            if let Some(path) = project_path_from_args(&args, Path::new(&cwd)) {
                if let Some(launch) = app.try_state::<LaunchPath>() {
                    if let Ok(mut slot) = launch.0.lock() {
                        *slot = Some(path.to_string_lossy().into_owned());
                    }
                }
                // Only a nudge: the window then asks for the path, so a path is never read from the event itself.
                let _ = app.emit("launch-path", ());
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Engine(api))
        .manage(LaunchPath(Mutex::new(started_with)))
        .setup(|app| {
            // The window is made here, not from the configuration alone, so a portable copy can keep the window's own data beside itself.
            let config = app.config().app.windows.first().cloned().expect("the configuration describes the main window");
            let mut window = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?;
            if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().and_then(portable_data_dir)) {
                let data = dir.join("window");
                std::fs::create_dir_all(&data)?;
                window = window.data_directory(data);
            }
            window.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, launch_path])
        .register_uri_scheme_protocol("media", |context, request| {
            let engine = context.app_handle().state::<Engine>();
            media_response(&engine.0, request.uri().path(), request.headers().get("range").and_then(|v| v.to_str().ok()))
        })
        .build(tauri::generate_context!())
        .expect("PrismQDA could not start its window")
        .run(|handle, event| {
            // The process ends without dropping managed state, so the project must be closed here.
            if let tauri::RunEvent::Exit = event {
                close_project(&handle.state::<Engine>().0);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_name_in_the_history_is_made_once_kept_and_never_a_windows_account_name() {
        let dir = tempfile::tempdir().unwrap();
        let first = automatic_name_in(dir.path());
        assert!(first.starts_with("Researcher ") && first.len() == "Researcher ".len() + 4, "{first}");
        assert_eq!(automatic_name_in(dir.path()), first, "the same name the next time the program starts");
        // A file that is not four hexadecimal digits is made again, and not trusted.
        std::fs::write(dir.path().join("history-name.txt"), "jason-pc").unwrap();
        let again = automatic_name_in(dir.path());
        assert!(again.starts_with("Researcher ") && !again.to_lowercase().contains("jason"), "{again}");
        assert_eq!(std::fs::read_to_string(dir.path().join("history-name.txt")).unwrap().trim().len(), 4);
        // And two computers do not start with one name (this can fail once in 65,536 times, so it asks for several).
        let names: std::collections::HashSet<String> = (0..12).map(|_| automatic_name_in(tempfile::tempdir().unwrap().path())).collect();
        assert!(names.len() > 1, "{names:?}");
    }

    #[test]
    fn a_copy_is_portable_only_when_the_marker_is_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(portable_data_dir(dir.path()), None, "an installed program uses the Windows profile");
        std::fs::write(dir.path().join(PORTABLE_MARKER), "portable").unwrap();
        assert_eq!(portable_data_dir(dir.path()), Some(dir.path().join("PrismQDAData")));
        // A folder with that name is not the marker.
        let other = tempfile::tempdir().unwrap();
        std::fs::create_dir(other.path().join(PORTABLE_MARKER)).unwrap();
        assert_eq!(portable_data_dir(other.path()), None);
    }
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::{App, WebviewWindow, WebviewWindowBuilder};

    fn app() -> App<MockRuntime> {
        let api = Api::new("Tester").without_backup_on_open();
        mock_builder()
            .manage(Engine(Arc::new(Mutex::new(api))))
            .manage(LaunchPath(Mutex::new(None)))
            .invoke_handler(tauri::generate_handler![call, launch_path])
            .build(mock_context(noop_assets()))
            .expect("the mock app builds")
    }

    /// Sends one `call` message the way the interface does, and returns what the interface would receive.
    fn ask(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
        let request = InvokeRequest {
            cmd: "call".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(json!({ "cmd": cmd, "args": args })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(webview, request).map(|body| body.deserialize::<Value>().unwrap())
    }

    #[test]
    fn the_window_reaches_the_engine_and_errors_arrive_as_code_and_message() {
        let app = app();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let webview = &window;

        let pong = ask(webview, "ping", json!({})).unwrap();
        assert_eq!(pong["name"], "PrismQDA");

        // No project is open yet: a plain-language error with a code the interface can branch on.
        let e = ask(webview, "project.info", json!({})).unwrap_err();
        assert_eq!(e["code"], "no_project");
        assert!(e["message"].as_str().unwrap().contains("no project is open"), "{e}");

        let e = ask(webview, "no.such.command", json!({})).unwrap_err();
        assert_eq!(e["code"], "unknown_command");
    }

    #[test]
    fn the_update_commands_are_the_windows_own_and_say_when_updates_are_not_set_up() {
        let app = app();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let webview = &window;

        // They never reach the engine, and they answer with no project open.
        let state = ask(webview, "update.state", json!({})).unwrap();
        assert!(state["version"].as_str().is_some_and(|v| !v.is_empty()), "{state}");
        assert_eq!(state["portable"], false);
        assert_eq!(state["set_up"], !updates::PUBKEY.is_empty());
        let blocked = if updates::PUBKEY.is_empty() { json!("not_set_up") } else { Value::Null };
        assert_eq!(state["blocked"], blocked, "until the maintainer's key is built in, nothing can be asked");
        if updates::PUBKEY.is_empty() {
            assert_eq!(ask(webview, "update.check", json!({})).unwrap_err()["code"], "not_set_up");
            assert_eq!(ask(webview, "update.install", json!({})).unwrap_err()["code"], "not_set_up");
        }
        assert_eq!(ask(webview, "update.nothing", json!({})).unwrap_err()["code"], "unknown_command");
    }

    #[test]
    fn a_whole_session_works_through_the_window_door() {
        let dir = tempfile::tempdir().unwrap();
        let app = app();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let webview = &window;

        let project = dir.path().join("study.pqda");
        let info = ask(webview, "project.create", json!({ "path": project, "name": "Through the window" })).unwrap();
        assert_eq!(info["name"], "Through the window");

        let text = dir.path().join("interview.txt");
        std::fs::write(&text, "I felt welcome on the first day.

The fees were a worry.").unwrap();
        let imported = ask(webview, "import.run", json!({ "path": text })).unwrap();
        let source = imported["outcome"]["source_ids"][0].as_i64().unwrap();
        let code = ask(webview, "code.add", json!({ "name": "Welcome" })).unwrap()["id"].as_i64().unwrap();
        let coding = ask(webview, "coding.apply", json!({ "source_id": source, "start": 2, "end": 14, "code_id": code })).unwrap();
        assert_eq!(coding["quote"], "felt welcome");

        let sources = ask(webview, "source.list", json!({})).unwrap();
        assert_eq!(sources[0]["codings"], 1);
        let events = ask(webview, "events.list", json!({ "limit": 50 })).unwrap();
        assert!(events.as_array().unwrap().iter().any(|e| e["action"] == "coding.add"));

        // Closing and opening again from the same door finds the coding.
        ask(webview, "project.close", json!({})).unwrap();
        ask(webview, "project.open", json!({ "path": project, "backup": false })).unwrap();
        let codings = ask(webview, "coding.list", json!({ "source_id": source })).unwrap();
        assert_eq!(codings.as_array().unwrap().len(), 1);
    }

    /// Found by closing the real window with its X button: the exit left a `-wal` file holding the newest work, so a
    /// copy of the `.pqda` alone was missing it. Closing the project at exit puts everything in the one file.
    #[test]
    fn closing_the_app_leaves_the_whole_project_in_its_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Mutex::new(Api::new("Tester").without_backup_on_open());
        let path = dir.path().join("study.pqda");
        {
            let mut api = engine.lock().unwrap();
            api.call("project.create", json!({ "path": path, "name": "Exit" })).unwrap();
            api.call("code.add", json!({ "name": "Cost", "definition": "Talk about money." })).unwrap();
        }

        close_project(&engine);

        let wal = dir.path().join("study.pqda-wal");
        assert!(!wal.exists() || std::fs::metadata(&wal).unwrap().len() == 0, "the write-ahead log still holds work after close");
        // What a person does: copy the project file by itself somewhere else and open that.
        let elsewhere = tempfile::tempdir().unwrap();
        let lone = elsewhere.path().join("copy.pqda");
        std::fs::copy(&path, &lone).unwrap();
        let mut other = Api::new("Tester").without_backup_on_open();
        other.call("project.open", json!({ "path": lone, "backup": false })).unwrap();
        let codes = other.call("code.list", json!({})).unwrap();
        assert!(codes.as_array().unwrap().iter().any(|c| c["name"] == "Cost"), "the copy is missing the newest code: {codes}");
    }

    #[test]
    fn the_windows_players_are_given_the_linked_recording_and_parts_of_it() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Mutex::new(Api::new("Tester").without_backup_on_open());
        let recording = dir.path().join("talk.wav");
        std::fs::write(&recording, (0..5000u32).map(|i| (i % 251) as u8).collect::<Vec<u8>>()).unwrap();
        let transcript = dir.path().join("talk.txt");
        std::fs::write(&transcript, "Ann: Hello there.\nBo: Fine, thanks.").unwrap();
        let source = {
            let mut api = engine.lock().unwrap();
            api.call("project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Media" })).unwrap();
            let id = api.call("import.run", json!({ "path": transcript })).unwrap()["outcome"]["source_ids"][0].as_i64().unwrap();
            api.call("media.link", json!({ "source_id": id, "path": recording })).unwrap();
            id
        };
        let header = |r: &tauri::http::Response<Vec<u8>>, name: &str| r.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);

        let part = media_response(&engine, &format!("/{source}"), Some("bytes=10-19"));
        assert_eq!(part.status().as_u16(), 206);
        assert_eq!(part.body().len(), 10);
        assert_eq!(part.body()[0], 10);
        assert_eq!(header(&part, "content-range").as_deref(), Some("bytes 10-19/5000"));
        assert_eq!(header(&part, "content-type").as_deref(), Some("audio/wav"));
        assert_eq!(header(&part, "accept-ranges").as_deref(), Some("bytes"));

        let whole = media_response(&engine, &format!("/{source}"), None);
        assert_eq!((whole.status().as_u16(), whole.body().len()), (200, 5000), "a small recording asked for with no range is given whole");
        assert_eq!(media_response(&engine, &format!("/{source}"), Some("bytes=9000-")).status().as_u16(), 416);
        assert_eq!(media_response(&engine, "/9999", None).status().as_u16(), 404, "a document with no recording");
        assert_eq!(media_response(&engine, "/not-a-number", None).status().as_u16(), 404);
    }

    #[test]
    fn the_windows_pictures_are_given_whole_with_their_kind_and_only_for_a_picture() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Mutex::new(Api::new("Tester").without_backup_on_open());
        // The first bytes of a PNG that says it is 40 pixels wide and 30 tall.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&40u32.to_be_bytes());
        png.extend_from_slice(&30u32.to_be_bytes());
        png.extend_from_slice(&[8, 2, 0, 0, 0, 1, 2, 3]);
        let picture = dir.path().join("view.png");
        std::fs::write(&picture, &png).unwrap();
        let words = dir.path().join("words.txt");
        std::fs::write(&words, "Some words.").unwrap();
        let (pic_id, words_id) = {
            let mut api = engine.lock().unwrap();
            api.call("project.create", json!({ "path": dir.path().join("p.pqda"), "name": "Pictures" })).unwrap();
            let pic = api.call("import.run", json!({ "path": picture })).unwrap()["outcome"]["source_ids"][0].as_i64().unwrap();
            let txt = api.call("import.run", json!({ "path": words })).unwrap()["outcome"]["source_ids"][0].as_i64().unwrap();
            (pic, txt)
        };
        let ok = media_response(&engine, &format!("/image/{pic_id}"), None);
        assert_eq!((ok.status().as_u16(), ok.body().as_slice()), (200, png.as_slice()), "the picture is given as it was imported");
        assert_eq!(ok.headers().get("content-type").and_then(|v| v.to_str().ok()), Some("image/png"));
        assert!(ok.headers().get("cache-control").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("max-age")), "and may be kept for a while");
        assert_eq!(media_response(&engine, &format!("/image/{words_id}"), None).status().as_u16(), 404, "a document of words has no picture");
        assert_eq!(media_response(&engine, "/image/9999", None).status().as_u16(), 404);
        assert_eq!(media_response(&engine, "/image/not-a-number", None).status().as_u16(), 404);
        assert_eq!(media_response(&engine, &format!("/{pic_id}"), None).status().as_u16(), 404, "a picture is not a recording");
    }

    #[test]
    fn the_windows_pdf_pages_are_drawn_at_the_width_asked_for_and_only_for_a_pdf_that_keeps_its_pages() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Mutex::new(Api::new("Tester").without_backup_on_open());
        let pdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/prismqda-import/tests/fixtures/pdf/three_pages.pdf");
        let words = dir.path().join("words.txt");
        std::fs::write(&words, "Some words.").unwrap();
        let (pdf_id, words_id) = {
            let mut api = engine.lock().unwrap();
            api.call("project.create", json!({ "path": dir.path().join("p.pqda"), "name": "PDFs" })).unwrap();
            let p = api.call("import.run", json!({ "path": pdf })).unwrap()["outcome"]["source_ids"][0].as_i64().unwrap();
            let w = api.call("import.run", json!({ "path": words })).unwrap()["outcome"]["source_ids"][0].as_i64().unwrap();
            (p, w)
        };
        let page = media_response(&engine, &format!("/pdf/{pdf_id}/2/500"), None);
        assert_eq!(page.status().as_u16(), 200);
        assert_eq!(page.headers().get("content-type").and_then(|v| v.to_str().ok()), Some("image/png"));
        assert_eq!(&page.body()[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes([page.body()[16], page.body()[17], page.body()[18], page.body()[19]]), 500, "drawn at the width asked for");
        assert!(page.headers().get("cache-control").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("max-age")), "and may be kept for a while");
        assert_eq!(media_response(&engine, &format!("/pdf/{pdf_id}/4/500"), None).status().as_u16(), 404, "a page the PDF does not have");
        assert_eq!(media_response(&engine, &format!("/pdf/{words_id}/1/500"), None).status().as_u16(), 404, "a document of words keeps no PDF");
        assert_eq!(media_response(&engine, &format!("/pdf/{pdf_id}/1"), None).status().as_u16(), 404, "the width is part of the address");
        assert_eq!(media_response(&engine, "/pdf/x/1/500", None).status().as_u16(), 404);
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_project_file_on_the_command_line_is_found_and_nothing_else_is() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("My Study.pqda");
        std::fs::write(&project, "x").unwrap();
        let upper = dir.path().join("LOUD.PQDA");
        std::fs::write(&upper, "x").unwrap();
        let other = dir.path().join("notes.txt");
        std::fs::write(&other, "x").unwrap();
        let program = "C:\\Program Files\\PrismQDA\\PrismQDA.exe";
        let here = dir.path();

        let full = project.to_string_lossy().into_owned();
        assert_eq!(project_path_from_args(&args(&[program, &full]), here), Some(project.clone()), "a path with a space in it, as Explorer hands it over");
        assert_eq!(project_path_from_args(&args(&[program, &upper.to_string_lossy()]), here), Some(upper.clone()), "the ending in capitals");
        assert_eq!(project_path_from_args(&args(&[program, "My Study.pqda"]), here), Some(project.clone()), "a relative path is read from where the program was started");
        assert_eq!(project_path_from_args(&args(&[program, "--flag", &other.to_string_lossy(), &full]), here), Some(project.clone()), "the first project among other words");

        assert_eq!(project_path_from_args(&args(&[program]), here), None, "no arguments");
        assert_eq!(project_path_from_args(&args(&[program, &other.to_string_lossy()]), here), None, "another kind of file");
        assert_eq!(project_path_from_args(&args(&[program, "gone.pqda"]), here), None, "a project that is not there");
        assert_eq!(project_path_from_args(&args(&[&full]), here), None, "the first word is the program itself, never a project");
        let folder = dir.path().join("a folder.pqda");
        std::fs::create_dir(&folder).unwrap();
        assert_eq!(project_path_from_args(&args(&[program, &folder.to_string_lossy()]), here), None, "a folder with that ending is not a project");
    }

    #[test]
    fn the_window_is_given_the_launch_path_once() {
        let app = app();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let ask_launch = || {
            let request = InvokeRequest {
                cmd: "launch_path".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(json!({})),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            };
            get_ipc_response(&window, request).unwrap().deserialize::<Value>().unwrap()
        };
        assert_eq!(ask_launch(), Value::Null, "started by itself: nothing to open");
        *app.state::<LaunchPath>().0.lock().unwrap() = Some("C:\\Studies\\one.pqda".to_string());
        assert_eq!(ask_launch(), json!("C:\\Studies\\one.pqda"));
        assert_eq!(ask_launch(), Value::Null, "taken: it is not opened twice");
    }

    #[test]
    fn closing_with_no_project_open_is_harmless() {
        let engine = Mutex::new(Api::new("Tester").without_backup_on_open());
        close_project(&engine);
        close_project(&engine);
    }

    #[test]
    fn a_command_sent_without_arguments_is_treated_as_having_none() {
        let app = app();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
        let request = InvokeRequest {
            cmd: "call".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(json!({ "cmd": "ping" })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        let reply = get_ipc_response(&window, request).unwrap().deserialize::<Value>().unwrap();
        assert_eq!(reply["name"], "PrismQDA");
    }
}
