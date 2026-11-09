//! The MCP connection. `CVBuilder --mcp` is an MCP server on stdin/stdout that AI apps start
//! (Claude Code, Claude Desktop, and CVBuilder's own chat when it uses Claude Code). It answers
//! the protocol itself and hands each tool call to the CVBuilder window that is open, so the
//! change happens in that window and shows at once.
//!
//! The window listens on 127.0.0.1 only, on a port the system picks, and writes the port and a
//! random token to `mcp-link.json` in its data folder; a call without the token is refused.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use cv_mcp::Outcome;
use serde_json::{Value, json};

/// A tool call that arrived over MCP, for the window to run.
pub struct LinkCall {
    pub name: String,
    pub arguments: Value,
    pub reply: Sender<Outcome>,
}

pub struct Link {
    pub calls: Receiver<LinkCall>,
    file: PathBuf,
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

fn link_file() -> PathBuf {
    crate::storage::data_dir().join("mcp-link.json")
}

fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    // Two independently seeded hashes of the time and process: unguessable for this purpose.
    let mut out = String::new();
    for _ in 0..2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

impl Link {
    /// Starts listening. `None` when the computer refuses a local port (then MCP is off).
    pub fn start(ctx: egui::Context) -> Option<Link> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        let token = token();
        let file = link_file();
        std::fs::create_dir_all(file.parent()?).ok()?;
        std::fs::write(&file, json!({"port": port, "token": token, "pid": std::process::id()}).to_string()).ok()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (tx, ctx, token) = (tx.clone(), ctx.clone(), token.clone());
                std::thread::spawn(move || serve(stream, &tx, &ctx, &token));
            }
        });
        Some(Link { calls: rx, file })
    }
}

fn serve(stream: TcpStream, tx: &Sender<LinkCall>, ctx: &egui::Context, token: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(600)));
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            break;
        };
        if v["token"].as_str() != Some(token) {
            let _ = writeln!(writer, "{}", json!({"text": "Refused: wrong token.", "is_error": true}));
            break;
        }
        let (reply, answer) = channel();
        let call = LinkCall { name: v["name"].as_str().unwrap_or_default().to_string(), arguments: v["arguments"].clone(), reply };
        if tx.send(call).is_err() {
            break;
        }
        ctx.request_repaint();
        let outcome = answer.recv_timeout(Duration::from_secs(60)).unwrap_or(Outcome { text: "CVBuilder did not answer in time.".into(), is_error: true, change: None });
        if writeln!(writer, "{}", json!({"text": outcome.text, "is_error": outcome.is_error})).is_err() {
            break;
        }
    }
}

/// The bridge's side: sends each tool call to the open window.
struct Bridge {
    connection: Option<(BufReader<TcpStream>, TcpStream, String)>,
}

const NOT_OPEN: &str = "CVBuilder is not open. Ask the person to open CVBuilder, then try again.";

impl Bridge {
    fn connect(&mut self) -> Result<(), String> {
        if self.connection.is_some() {
            return Ok(());
        }
        let text = std::fs::read_to_string(link_file()).map_err(|_| NOT_OPEN.to_string())?;
        let v: Value = serde_json::from_str(&text).map_err(|_| NOT_OPEN.to_string())?;
        let port = v["port"].as_u64().and_then(|p| u16::try_from(p).ok()).ok_or(NOT_OPEN)?;
        let stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(3)).map_err(|_| NOT_OPEN.to_string())?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(90)));
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        self.connection = Some((reader, stream, v["token"].as_str().unwrap_or_default().to_string()));
        Ok(())
    }

    fn send(&mut self, name: &str, arguments: &Value) -> Result<Outcome, String> {
        self.connect()?;
        let Some((reader, writer, token)) = self.connection.as_mut() else {
            return Err(NOT_OPEN.into());
        };
        writeln!(writer, "{}", json!({"token": token, "name": name, "arguments": arguments})).map_err(|e| e.to_string())?;
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err(NOT_OPEN.into());
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        Ok(Outcome { text: v["text"].as_str().unwrap_or_default().to_string(), is_error: v["is_error"].as_bool().unwrap_or(true), change: None })
    }
}

impl cv_mcp::Host for Bridge {
    fn call(&mut self, name: &str, arguments: &Value) -> Outcome {
        match self.send(name, arguments) {
            Ok(o) => o,
            Err(_) => {
                // The window may have been restarted: try once more with a fresh connection.
                self.connection = None;
                self.send(name, arguments).unwrap_or_else(|e| Outcome { text: e, is_error: true, change: None })
            }
        }
    }
}

/// `CVBuilder --mcp`: an MCP server on stdin/stdout.
pub fn serve_stdio() {
    let mut bridge = Bridge { connection: None };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = cv_mcp::handle(&mut bridge, &line) {
            if writeln!(stdout, "{reply}").and_then(|()| stdout.flush()).is_err() {
                break;
            }
        }
    }
}

/// Where Claude Code's chat config is written: CVBuilder's own data folder.
pub fn write_claude_code_config() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = crate::storage::data_dir().join("claude-code");
    std::fs::create_dir_all(&dir).ok()?;
    let file = dir.join("cvbuilder-mcp.json");
    std::fs::write(&file, serde_json::to_string_pretty(&cv_ai::chat::mcp_config(&exe)).ok()?).ok()?;
    Some(file)
}

/// The snippet a person pastes into an AI app's MCP settings.
pub fn client_snippet() -> String {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "CVBuilder".into());
    serde_json::to_string_pretty(&json!({"mcpServers": {"cvbuilder": {"command": exe, "args": ["--mcp"]}}})).unwrap_or_default()
}
