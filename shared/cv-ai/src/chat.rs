//! The chat beside the CV. A turn runs on a worker thread: the AI answers, may call CVBuilder's
//! tools (the same ones the MCP server offers), sees their results and goes on until it is done.
//!
//! Two kinds of backend:
//! - **API** (Claude, ChatGPT, Gemini, Grok with the person's own key). Tool calls are sent to the
//!   window as [`ToolRequest`]s; the window runs them on the open CV, so changes show at once.
//! - **Claude Code**, the person's own installed `claude` program with their own sign-in. It is
//!   started in headless mode with CVBuilder as its only MCP server (`CVBuilder --mcp`), and no
//!   other tools; its tool calls reach the window through that MCP connection.
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use cv_mcp::Outcome;
use serde_json::{Value, json};

use crate::providers::{AiFailure, Endpoint, Service};

/// Who the person is chatting with.
#[derive(Clone)]
pub enum Backend {
    Api { endpoint: Endpoint, model: String },
    ClaudeCode { program: PathBuf, mcp_config: PathBuf, model: Option<String> },
}

impl Backend {
    pub fn name(&self) -> String {
        match self {
            Backend::Api { endpoint, .. } => endpoint.service.name().to_string(),
            Backend::ClaudeCode { .. } => "Claude Code".to_string(),
        }
    }
}

/// What the window hears while a turn runs.
#[derive(Debug)]
pub enum ChatEvent {
    /// Text from the AI (a whole block).
    Text(String),
    /// The AI called a tool (the name without any `mcp__cvbuilder__` prefix).
    ToolStarted { id: String, name: String },
    ToolDone { id: String, ok: bool },
    /// The turn ended. `history` is what the next turn continues from.
    Done { history: History },
    Failed { message: String, history: History },
}

/// A tool call the window must run on the open CV.
pub struct ToolRequest {
    pub name: String,
    pub arguments: Value,
    pub reply: Sender<Outcome>,
}

/// The conversation so far, in the backend's own format.
#[derive(Debug, Clone, Default)]
pub struct History {
    /// API backends: the messages (without the system prompt).
    pub messages: Vec<Value>,
    /// Claude Code: the session to resume.
    pub session: Option<String>,
}

/// How many model calls one turn may make. A CV edit rarely needs more than a dozen.
const MAX_STEPS: usize = 24;

/// The system prompt: the rules, plus where the person is.
pub fn system_prompt(extra: &str) -> String {
    format!("{}\n\n{extra}", cv_mcp::INSTRUCTIONS)
}

/// Runs one turn. Blocks until the turn is over; call it on a worker thread.
pub fn run_turn(backend: &Backend, mut history: History, user: &str, system: &str, events: &Sender<ChatEvent>, tools: &Sender<ToolRequest>, stop: &Arc<AtomicBool>) {
    let result = match backend {
        Backend::Api { endpoint, model } if endpoint.service == Service::Claude => claude_turn(endpoint, model, &mut history, user, system, events, tools, stop),
        Backend::Api { endpoint, model } => openai_turn(endpoint, model, &mut history, user, system, events, tools, stop),
        Backend::ClaudeCode { program, mcp_config, model } => claude_code_turn(program, mcp_config, model.as_deref(), &mut history, user, system, events, stop),
    };
    let _ = events.send(match result {
        Ok(()) => ChatEvent::Done { history },
        Err(message) => ChatEvent::Failed { message, history },
    });
}

/// Asks the window to run a tool and waits for the result.
fn run_tool(tools: &Sender<ToolRequest>, name: &str, arguments: Value) -> Outcome {
    let (reply, answer) = channel();
    if tools.send(ToolRequest { name: name.to_string(), arguments, reply }).is_err() {
        return Outcome { text: "CVBuilder was closed.".into(), is_error: true, change: None };
    }
    answer.recv_timeout(Duration::from_secs(60)).unwrap_or(Outcome { text: "CVBuilder did not answer.".into(), is_error: true, change: None })
}

const STOPPED: &str = "The person stopped this before the tool ran.";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(300))).max_redirects(0).http_status_as_error(false).build().new_agent()
}

fn post(endpoint: &Endpoint, url: &str, headers: &[(&str, &str)], body: &Value) -> Result<Value, String> {
    let service = endpoint.service;
    let mut request = agent().post(url).header("content-type", "application/json");
    for (k, v) in headers {
        request = request.header(*k, *v);
    }
    let mut reply = request.send(body.to_string()).map_err(|_| {
        if service == Service::Local {
            format!("Fikk ikke kontakt med den lokale AI-serveren på {}. Kjører den fortsatt?", endpoint.base_url)
        } else {
            AiFailure::Network(service.name()).to_string()
        }
    })?;
    let status = reply.status().as_u16();
    let text = reply.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        return Err(crate::providers::status_error(service, status, &text).to_string());
    }
    serde_json::from_str(&text).map_err(|_| AiFailure::Unreadable(service.name()).to_string())
}

// ---------- Claude (Messages API with tools) ----------

fn claude_tools() -> Vec<Value> {
    cv_mcp::tools().into_iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.schema})).collect()
}

#[allow(clippy::too_many_arguments)]
fn claude_turn(endpoint: &Endpoint, model: &str, history: &mut History, user: &str, system: &str, events: &Sender<ChatEvent>, tools: &Sender<ToolRequest>, stop: &Arc<AtomicBool>) -> Result<(), String> {
    history.messages.push(json!({"role": "user", "content": user}));
    let url = format!("{}/messages", endpoint.base_url);
    let key = endpoint.key.expose().to_string();
    let headers = [("x-api-key", key.as_str()), ("anthropic-version", crate::providers::ANTHROPIC_VERSION), ("anthropic-beta", "server-side-fallback-2026-07-01")];
    for _ in 0..MAX_STEPS {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let body = json!({
            "model": model,
            "max_tokens": 16000,
            "system": system,
            "messages": history.messages,
            "tools": claude_tools(),
            "thinking": {"type": "adaptive"},
            "fallbacks": "default",
        });
        let reply = post(endpoint, &url, &headers, &body)?;
        let stop_reason = reply["stop_reason"].as_str().unwrap_or_default().to_string();
        if stop_reason == "refusal" {
            history.messages.pop();
            return Err(AiFailure::Refused("Claude").to_string());
        }
        let content = reply["content"].clone();
        // The reply goes back unchanged (thinking blocks included), as the API requires.
        history.messages.push(json!({"role": "assistant", "content": content}));
        let blocks = content.as_array().cloned().unwrap_or_default();
        for b in &blocks {
            if b["type"] == "text" {
                if let Some(t) = b["text"].as_str().filter(|t| !t.trim().is_empty()) {
                    let _ = events.send(ChatEvent::Text(t.to_string()));
                }
            }
        }
        let calls: Vec<&Value> = blocks.iter().filter(|b| b["type"] == "tool_use").collect();
        if calls.is_empty() {
            if stop_reason == "max_tokens" {
                let _ = events.send(ChatEvent::Text("(Svaret ble avbrutt fordi det ble for langt.)".into()));
            }
            return Ok(());
        }
        // Every call gets a result, also when the person stopped the turn, so the next request is valid.
        let mut results = Vec::new();
        for call in calls {
            let id = call["id"].as_str().unwrap_or_default().to_string();
            let name = call["name"].as_str().unwrap_or_default().to_string();
            let outcome = if stop.load(Ordering::Relaxed) {
                Outcome { text: STOPPED.into(), is_error: true, change: None }
            } else {
                let _ = events.send(ChatEvent::ToolStarted { id: id.clone(), name: name.clone() });
                let o = run_tool(tools, &name, call["input"].clone());
                let _ = events.send(ChatEvent::ToolDone { id: id.clone(), ok: !o.is_error });
                o
            };
            results.push(json!({"type": "tool_result", "tool_use_id": id, "content": outcome.text, "is_error": outcome.is_error}));
        }
        history.messages.push(json!({"role": "user", "content": results}));
    }
    Err("AI-en brukte for mange steg på én melding. Del opp oppgaven og prøv igjen.".into())
}

// ---------- ChatGPT, Gemini, Grok (OpenAI-compatible chat completions with tools) ----------

fn openai_tools() -> Vec<Value> {
    cv_mcp::tools().into_iter().map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.schema}})).collect()
}

#[allow(clippy::too_many_arguments)]
fn openai_turn(endpoint: &Endpoint, model: &str, history: &mut History, user: &str, system: &str, events: &Sender<ChatEvent>, tools: &Sender<ToolRequest>, stop: &Arc<AtomicBool>) -> Result<(), String> {
    history.messages.push(json!({"role": "user", "content": user}));
    let url = format!("{}/chat/completions", endpoint.base_url);
    let auth = format!("Bearer {}", endpoint.key.expose());
    // Local servers often have no key: then no Authorization header at all.
    let with_key = [("authorization", auth.as_str())];
    let headers: &[(&str, &str)] = if endpoint.key.expose().is_empty() { &[] } else { &with_key };
    for _ in 0..MAX_STEPS {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut messages = vec![json!({"role": "system", "content": system})];
        messages.extend(history.messages.iter().cloned());
        let body = json!({"model": model, "messages": messages, "tools": openai_tools()});
        let reply = post(endpoint, &url, headers, &body)?;
        let message = reply["choices"][0]["message"].clone();
        if message.is_null() {
            return Err(AiFailure::Unreadable(endpoint.service.name()).to_string());
        }
        let text = message["content"].as_str().unwrap_or_default().to_string();
        let calls = message["tool_calls"].as_array().cloned().unwrap_or_default();
        let mut kept = json!({"role": "assistant", "content": text});
        if !calls.is_empty() {
            kept["tool_calls"] = Value::Array(calls.clone());
        }
        history.messages.push(kept);
        if !text.trim().is_empty() {
            let _ = events.send(ChatEvent::Text(text));
        }
        if calls.is_empty() {
            return Ok(());
        }
        for call in calls {
            let id = call["id"].as_str().unwrap_or_default().to_string();
            let name = call["function"]["name"].as_str().unwrap_or_default().to_string();
            // Arguments arrive as a JSON string (some services send an object).
            let arguments = match &call["function"]["arguments"] {
                Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({})),
                Value::Null => json!({}),
                other => other.clone(),
            };
            let outcome = if stop.load(Ordering::Relaxed) {
                Outcome { text: STOPPED.into(), is_error: true, change: None }
            } else {
                let _ = events.send(ChatEvent::ToolStarted { id: id.clone(), name: name.clone() });
                let o = run_tool(tools, &name, arguments);
                let _ = events.send(ChatEvent::ToolDone { id: id.clone(), ok: !o.is_error });
                o
            };
            history.messages.push(json!({"role": "tool", "tool_call_id": id, "content": outcome.text}));
        }
    }
    Err("AI-en brukte for mange steg på én melding. Del opp oppgaven og prøv igjen.".into())
}

// ---------- Claude Code (the person's own `claude` program, connected over MCP) ----------

/// Finds an installed Claude Code: on the PATH, or where its installer puts it.
pub fn find_claude_code() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
    }
    if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

/// The MCP configuration that tells Claude Code how to start CVBuilder's MCP server.
pub fn mcp_config(cvbuilder: &Path) -> Value {
    json!({"mcpServers": {"cvbuilder": {"type": "stdio", "command": cvbuilder.to_string_lossy(), "args": ["--mcp"]}}})
}

fn short_tool(name: &str) -> String {
    name.strip_prefix("mcp__cvbuilder__").unwrap_or(name).to_string()
}

/// Turns Claude Code's own failure text into a next step the person can take.
fn claude_code_failure(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("login") || lower.contains("log in") || lower.contains("not logged") || lower.contains("auth") || lower.contains("api key") {
        "Claude Code er ikke logget inn. Åpne en terminal, kjør «claude», logg inn, og prøv igjen.".into()
    } else {
        let short: String = text.trim().chars().take(400).collect();
        format!("Claude Code stoppet: {short}")
    }
}

#[allow(clippy::too_many_arguments)]
fn claude_code_turn(program: &Path, mcp_config: &Path, model: Option<&str>, history: &mut History, user: &str, system: &str, events: &Sender<ChatEvent>, stop: &Arc<AtomicBool>) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(["-p", "--output-format", "stream-json", "--verbose"])
        // No built-in tools (files, shell, web): only CVBuilder's tools, through MCP.
        .args(["--tools", ""])
        .args(["--strict-mcp-config", "--mcp-config"])
        .arg(mcp_config)
        .args(["--allowedTools", "mcp__cvbuilder"])
        .arg("--append-system-prompt")
        .arg(system);
    if let Some(session) = &history.session {
        cmd.args(["--resume", session]);
    }
    if let Some(m) = model.filter(|m| !m.is_empty()) {
        cmd.args(["--model", m]);
    }
    // A neutral working folder, so no project's CLAUDE.md or settings are picked up.
    cmd.current_dir(mcp_config.parent().unwrap_or(Path::new(".")));
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing over the app.
    }
    let mut child = cmd.spawn().map_err(|e| format!("Claude Code kunne ikke startes ({e})."))?;
    // The message goes on stdin: it can be long, and a command line has a length limit.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(user.as_bytes());
    }
    let stderr = child.stderr.take();
    let err_text = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = std::io::Read::read_to_string(&mut e, &mut s);
        }
        s
    });
    let (line_tx, line_rx) = channel::<String>();
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let mut result: Option<Result<(), String>> = None;
    let mut idle = Duration::ZERO;
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            result = Some(Ok(()));
            break;
        }
        match line_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                idle = Duration::ZERO;
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                match v["type"].as_str() {
                    Some("system") if v["subtype"] == "init" => {
                        if let Some(s) = v["session_id"].as_str() {
                            history.session = Some(s.to_string());
                        }
                        let connected = v["mcp_servers"].as_array().is_some_and(|a| a.iter().any(|s| s["name"] == "cvbuilder" && s["status"] == "connected"));
                        if !connected {
                            let _ = events.send(ChatEvent::Text("(Claude Code fikk ikke kontakt med CVBuilder over MCP, så det kan ikke endre CV-en i denne runden.)".into()));
                        }
                    }
                    Some("assistant") => {
                        for b in v["message"]["content"].as_array().cloned().unwrap_or_default() {
                            match b["type"].as_str() {
                                Some("text") => {
                                    if let Some(t) = b["text"].as_str().filter(|t| !t.trim().is_empty()) {
                                        let _ = events.send(ChatEvent::Text(t.to_string()));
                                    }
                                }
                                Some("tool_use") => {
                                    let _ = events.send(ChatEvent::ToolStarted { id: b["id"].as_str().unwrap_or_default().into(), name: short_tool(b["name"].as_str().unwrap_or_default()) });
                                }
                                _ => {}
                            }
                        }
                    }
                    Some("user") => {
                        for b in v["message"]["content"].as_array().cloned().unwrap_or_default() {
                            if b["type"] == "tool_result" {
                                let _ = events.send(ChatEvent::ToolDone { id: b["tool_use_id"].as_str().unwrap_or_default().into(), ok: !b["is_error"].as_bool().unwrap_or(false) });
                            }
                        }
                    }
                    Some("result") => {
                        if let Some(s) = v["session_id"].as_str() {
                            history.session = Some(s.to_string());
                        }
                        result = Some(if v["is_error"].as_bool().unwrap_or(false) { Err(claude_code_failure(v["result"].as_str().unwrap_or("ukjent feil"))) } else { Ok(()) });
                    }
                    _ => {}
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                idle += Duration::from_millis(200);
                if idle > Duration::from_secs(600) {
                    let _ = child.kill();
                    result = Some(Err("Claude Code svarte ikke på ti minutter og ble stoppet.".into()));
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = child.wait().ok();
    let errors = err_text.join().unwrap_or_default();
    match result {
        Some(r) => r,
        None if status.is_some_and(|s| s.success()) => Ok(()),
        None => Err(claude_code_failure(if errors.trim().is_empty() { "programmet avsluttet uten svar" } else { &errors })),
    }
}

/// One request to Claude Code without tools or MCP: `system` is appended to its system prompt,
/// `user` goes on stdin, and the text of its answer comes back. Stopped after five minutes.
pub fn claude_code_once(program: &Path, system: &str, user: &str) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(["-p", "--output-format", "json", "--tools", ""]).arg("--append-system-prompt").arg(system);
    // A neutral working folder, so no project's CLAUDE.md or settings are picked up.
    cmd.current_dir(std::env::temp_dir());
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("Claude Code kunne ikke startes ({e})."))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(user.as_bytes());
    }
    let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_string(&mut s);
            }
            s
        })
    };
    let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
    let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > Duration::from_secs(300) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Claude Code svarte ikke på fem minutter og ble stoppet.".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(claude_code_failure(&e.to_string())),
        }
    }
    let text = out.join().unwrap_or_default();
    let errors = err.join().unwrap_or_default();
    match serde_json::from_str::<Value>(text.trim()) {
        Ok(v) if v["is_error"].as_bool().unwrap_or(false) => Err(claude_code_failure(v["result"].as_str().unwrap_or("ukjent feil"))),
        Ok(v) => v["result"].as_str().map(str::to_string).ok_or_else(|| claude_code_failure("svaret manglet tekst")),
        Err(_) => Err(claude_code_failure(if errors.trim().is_empty() { "programmet avsluttet uten svar" } else { &errors })),
    }
}
