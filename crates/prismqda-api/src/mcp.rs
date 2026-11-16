//! The assistant route (decision 203), as `prismqda mcp <project>` and `PrismQDA.exe mcp <project>`. An assistant application the researcher
//! already uses (one that speaks the Model Context Protocol) starts the program as a child process and talks to it over its standard input
//! and output, one JSON message to a line. There is no port and no network connection: PrismQDA does not reach out to the assistant, and
//! nothing can reach in except the application that started the program. Nothing runs until the researcher sets their application to start it.
//!
//! What the assistant may see and do is decided by the rules in the engine (`Project::assistant_call`), not here; this file only speaks the
//! protocol: `initialize`, `ping`, `tools/list` and `tools/call`.

use std::io::{BufRead, Write};
use std::path::Path;

use crate::Api;
use serde_json::{json, Value};

const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub struct Server {
    api: Api,
    permission: String,
    client: String,
    calls: usize,
    suggested: usize,
}

fn error(id: Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

impl Server {
    /// Opens the project and checks that it allows an assistant at all. `actor` is the name the person codes under; suggestions that are accepted
    /// later are coded by them, so a session that proposes must be given it.
    pub fn open(project: &Path, actor: Option<&str>, passphrase: Option<String>, permission: &str) -> Result<Server, String> {
        if !matches!(permission, "read" | "propose") {
            return Err(format!("the permission is read or propose, not '{permission}'"));
        }
        let actor = actor.map(str::trim).filter(|a| !a.is_empty());
        if permission == "propose" && actor.is_none() {
            return Err("say whose coding the suggestions would become: add --actor \"<your name>\", the name you code under in PrismQDA".to_string());
        }
        let mut api = Api::new(actor.unwrap_or("assistant")).without_backup_on_open();
        api.call("project.open", json!({ "path": project, "backup": false, "passphrase": passphrase })).map_err(|e| e.to_string())?;
        api.call("assistant.check", json!({})).map_err(|e| e.to_string())?;
        Ok(Server { api, permission: permission.to_string(), client: String::new(), calls: 0, suggested: 0 })
    }

    /// One line in, the reply (if there is one) out. A message with no id is a notification and gets no reply.
    pub fn handle(&mut self, line: &str) -> Option<String> {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return Some(error(Value::Null, -32700, "the message is not JSON")),
        };
        if !v.is_object() {
            return Some(error(Value::Null, -32600, "the message is not a request"));
        }
        let id = v.get("id").filter(|i| !i.is_null()).cloned()?;
        let method = v["method"].as_str().unwrap_or("");
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        let reply: Result<Value, (i64, String)> = match method {
            "initialize" => {
                self.client = params["clientInfo"]["name"].as_str().unwrap_or("").to_string();
                let asked = params["protocolVersion"].as_str().unwrap_or("");
                let version = PROTOCOLS.iter().find(|p| **p == asked).unwrap_or(&"2024-11-05");
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "prismqda", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "This is a qualitative research project. You can read its documents, codebook and codings, search them, and count words. Passages and codes the researcher marked do not share are withheld. You may not change anything; if you were given the permission to propose, you can suggest codings, which wait for the researcher's review.",
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => self.api.call("assistant.tools", json!({ "permission": self.permission })).map_err(|e| (-32603, e.to_string())),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                if name.is_empty() {
                    Err((-32602, "a tool call needs a name".to_string()))
                } else {
                    self.calls += 1;
                    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                    let outcome = self.api.call("assistant.call", json!({ "tool": name, "arguments": arguments, "permission": self.permission, "client": self.client }));
                    Ok(match outcome {
                        Ok(value) => {
                            if name == "propose_codings" {
                                self.suggested += value["suggested"].as_u64().unwrap_or(0) as usize;
                            }
                            json!({ "content": [{ "type": "text", "text": value.to_string() }], "isError": false })
                        }
                        Err(e) => json!({ "content": [{ "type": "text", "text": e.to_string() }], "isError": true }),
                    })
                }
            }
            other => Err((-32601, format!("there is no method named '{other}'"))),
        };
        Some(match reply {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
            Err((code, message)) => error(id, code, &message),
        })
    }

    /// Reads messages until the assistant's application closes the pipe, then says on standard error what the session did.
    pub fn run(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle(&line) {
                writeln!(output, "{reply}")?;
                output.flush()?;
            }
        }
        eprintln!(
            "assistant session over: {} tool call{}, {} suggestion{} sent to the review queue (permission: {})",
            self.calls,
            if self.calls == 1 { "" } else { "s" },
            self.suggested,
            if self.suggested == 1 { "" } else { "s" },
            self.permission
        );
        Ok(())
    }
}
