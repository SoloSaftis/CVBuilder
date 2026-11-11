//! A small MCP server (JSON-RPC 2.0, one message per line, as the stdio transport sends them).
//! It answers `initialize`, `ping`, `tools/list` and `tools/call`; the work of a tool call is
//! done by a [`Host`], so the same server runs inside the app (on the open CV) and in the
//! `--mcp` bridge (which hands calls to the running app).
use serde_json::{Value, json};

use crate::tools::{INSTRUCTIONS, Outcome, tools};

/// The protocol revisions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Runs tool calls.
pub trait Host {
    fn call(&mut self, name: &str, arguments: &Value) -> Outcome;
}

/// Runs tools directly on a project.
impl Host for cv_core::Project {
    fn call(&mut self, name: &str, arguments: &Value) -> Outcome {
        crate::tools::call(self, name, arguments)
    }
}

/// The `tools/list` result.
pub fn tool_list() -> Value {
    // The hints let a client allow reading tools without asking, and ask before removing things.
    let tools: Vec<Value> = tools()
        .into_iter()
        .map(|t| {
            let annotations = json!({
                "readOnlyHint": !t.edits,
                "destructiveHint": t.name.ends_with("_remove"),
                "idempotentHint": !t.edits || !t.name.ends_with("_save"),
                "openWorldHint": false,
            });
            json!({"name": t.name, "description": t.description, "inputSchema": t.schema, "annotations": annotations})
        })
        .collect();
    json!({ "tools": tools })
}

fn result(id: &Value, result: Value) -> Option<String> {
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string())
}

fn error(id: &Value, code: i64, message: &str) -> Option<String> {
    Some(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string())
}

/// Handles one JSON-RPC message. Returns the reply line, or `None` for a notification.
pub fn handle(host: &mut dyn Host, line: &str) -> Option<String> {
    let message: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return error(&Value::Null, -32700, "Parse error"),
    };
    if message.is_array() {
        return error(&Value::Null, -32600, "Batches are not supported");
    }
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    // A notification (no id) never gets a reply; a reply sent to us (no method) is ignored.
    let id = match id {
        Some(id) if !method.is_empty() => id,
        // An id but no method and no result: a broken request, which still gets an answer.
        Some(id) if message.get("result").is_none() && message.get("error").is_none() => return error(&id, -32600, "Invalid request"),
        _ => return None,
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
            let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[1]);
            result(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "cvbuilder", "title": "CVBuilder", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => result(&id, json!({})),
        "tools/list" => result(&id, tool_list()),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return error(&id, -32602, "tools/call needs a tool name");
            };
            if !tools().iter().any(|t| t.name == name) {
                return error(&id, -32602, &format!("Unknown tool: {name}"));
            }
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let outcome = host.call(name, &arguments);
            let mut body = json!({"content": [{"type": "text", "text": outcome.text}], "isError": outcome.is_error});
            // JSON answers (cv_read, cv_check …) also go as structured content, for clients that use it.
            if let Ok(object @ Value::Object(_)) = serde_json::from_str::<Value>(&outcome.text) {
                body["structuredContent"] = object;
            }
            result(&id, body)
        }
        "resources/list" => result(&id, json!({"resources": []})),
        "prompts/list" => result(&id, json!({"prompts": []})),
        _ => error(&id, -32601, &format!("Method not found: {method}")),
    }
}
