//! A local development bridge: `prismqda serve` answers the interface's commands over HTTP so the
//! screens can be tested in a headless browser against the real engine. It listens on 127.0.0.1 only,
//! refuses requests from other web pages, and exists only when built with `--features dev-server`.
//! The shipped desktop app does not use it: it calls the same command layer directly.

use std::io::Read;

use prismqda_api::Api;
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

fn json_response(status: u16, body: &Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut r = Response::from_string(body.to_string()).with_status_code(status);
    r.add_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").expect("header"));
    r.add_header(Header::from_bytes("Cache-Control", "no-store").expect("header"));
    r
}

/// The answer to a player: the recording, or the part of it that was asked for.
fn media_response(reply: prismqda_api::MediaReply) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut r = Response::from_data(reply.body).with_status_code(reply.status);
    r.add_header(Header::from_bytes("Content-Type", reply.content_type).expect("header"));
    r.add_header(Header::from_bytes("Accept-Ranges", "bytes").expect("header"));
    r.add_header(Header::from_bytes("Cache-Control", if reply.cacheable { "private, max-age=3600" } else { "no-store" }).expect("header"));
    if let Some(range) = reply.content_range {
        r.add_header(Header::from_bytes("Content-Range", range).expect("header"));
    }
    r
}

/// Only pages served from this computer may call the bridge, so a web page the person happens to
/// visit cannot reach their project.
fn origin_allowed(request: &Request) -> bool {
    let header = |name: &str| {
        request
            .headers()
            .iter()
            .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str().to_string())
    };
    let local = |value: &str| {
        let rest = value.strip_prefix("http://").unwrap_or(value);
        let host = rest.split('/').next().unwrap_or("");
        let host = host.split(':').next().unwrap_or("");
        host == "127.0.0.1" || host == "localhost"
    };
    match header("Origin") {
        Some(origin) => local(&origin),
        None => header("Host").map(|h| local(&h)).unwrap_or(false),
    }
}

pub fn run(port: u16, actor: &str, project: Option<&std::path::Path>) -> Result<(), String> {
    let server = Server::http(("127.0.0.1", port)).map_err(|e| format!("cannot listen on port {port}: {e}"))?;
    let mut api = Api::new(actor).without_backup_on_open();
    if let Some(path) = project {
        api.call("project.open", json!({ "path": path.display().to_string(), "backup": false })).map_err(|e| e.to_string())?;
    }
    println!("PrismQDA development bridge listening on http://127.0.0.1:{port}");
    for mut request in server.incoming_requests() {
        let response = if !origin_allowed(&request) {
            json_response(403, &json!({ "ok": false, "error": { "code": "forbidden", "message": "this bridge only answers pages served from this computer" } }))
        } else if *request.method() == Method::Get && request.url() == "/health" {
            json_response(200, &json!({ "ok": true }))
        } else if *request.method() == Method::Post && request.url() == "/rpc" {
            let mut body = String::new();
            let _ = request.as_reader().take(64 * 1024 * 1024).read_to_string(&mut body);
            match serde_json::from_str::<Value>(&body) {
                Ok(v) => {
                    let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
                    let args = v.get("args").cloned().unwrap_or(Value::Null);
                    match api.call(&cmd, args) {
                        Ok(result) => json_response(200, &json!({ "ok": true, "result": result })),
                        Err(e) => json_response(200, &json!({ "ok": false, "error": { "code": e.code, "message": e.message } })),
                    }
                }
                Err(e) => json_response(400, &json!({ "ok": false, "error": { "code": "bad_request", "message": e.to_string() } })),
            }
        } else if *request.method() == Method::Get && request.url().starts_with("/media/") {
            // A player asking for a recording, or a part of one, by the number of its document.
            let range = request.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("range")).map(|h| h.value.as_str().to_string());
            match request.url()["/media/".len()..].split(['?', '#']).next().unwrap_or("").parse::<i64>() {
                Ok(id) => media_response(api.media_reply(id, range.as_deref())),
                Err(_) => json_response(404, &json!({ "ok": false, "error": { "code": "not_found", "message": "a recording is asked for by its document's number" } })),
            }
        } else if *request.method() == Method::Get && request.url().starts_with("/pdf/") {
            // One page of a PDF, drawn at a width: `/pdf/<document>/<page>/<width>`.
            match prismqda_api::parse_pdf_page_path(request.url()["/pdf/".len()..].split(['?', '#']).next().unwrap_or("")) {
                Some((id, page, width)) => media_response(api.pdf_page_reply(id, page, width)),
                None => json_response(404, &json!({ "ok": false, "error": { "code": "not_found", "message": "a page is asked for as pdf/<document>/<page>/<width>" } })),
            }
        } else if *request.method() == Method::Get && request.url().starts_with("/image/") {
            // A picture, by the number of its document.
            match request.url()["/image/".len()..].split(['?', '#']).next().unwrap_or("").parse::<i64>() {
                Ok(id) => media_response(api.image_reply(id)),
                Err(_) => json_response(404, &json!({ "ok": false, "error": { "code": "not_found", "message": "a picture is asked for by its document's number" } })),
            }
        } else {
            json_response(404, &json!({ "ok": false, "error": { "code": "not_found", "message": "there is nothing here" } }))
        };
        let _ = request.respond(response);
    }
    Ok(())
}
