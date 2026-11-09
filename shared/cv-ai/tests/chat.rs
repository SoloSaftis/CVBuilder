//! The chat's tool loop against a local stand-in server: no real keys, no network, no cost.
//! The server answers with a tool call first and a text answer second; the test plays the
//! window's part and runs the tool on a real project.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use craft_ai_core::SecretString;
use cv_ai::chat::{Backend, ChatEvent, History, run_turn};
use cv_ai::providers::{Endpoint, Service};
use cv_core::Project;
use serde_json::Value;

/// Replies to the n-th request with `replies[n]`, and records each request body.
fn server(replies: Vec<&'static str>) -> (String, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://127.0.0.1:{}/v1", listener.local_addr().expect("addr").port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().flatten().enumerate() {
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut line = String::new();
            reader.read_line(&mut line).ok();
            let path = line.split_whitespace().nth(1).unwrap_or_default().to_string();
            let mut length = 0usize;
            loop {
                let mut h = String::new();
                reader.read_line(&mut h).ok();
                let h = h.trim_end().to_string();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
            }
            let mut buf = vec![0; length];
            reader.read_exact(&mut buf).ok();
            log.lock().expect("lock").push((path, serde_json::from_slice(&buf).unwrap_or_default()));
            let body = replies.get(n).copied().unwrap_or(r#"{"error":{"message":"no more replies"}}"#);
            let mut stream = stream;
            let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
        }
    });
    (url, seen)
}

/// Runs one turn, acting as the window: tool requests are run on `project`.
fn turn(backend: Backend, project: &mut Project, user: &str) -> (Vec<ChatEvent>, History) {
    let (events_tx, events_rx) = channel();
    let (tools_tx, tools_rx) = channel::<cv_ai::chat::ToolRequest>();
    let stop = Arc::new(AtomicBool::new(false));
    let user = user.to_string();
    let worker = std::thread::spawn(move || run_turn(&backend, History::default(), &user, "system", &events_tx, &tools_tx, &stop));
    let mut events = Vec::new();
    loop {
        if let Ok(r) = tools_rx.recv_timeout(std::time::Duration::from_millis(20)) {
            let _ = r.reply.send(cv_mcp::call(project, &r.name, &r.arguments));
        }
        while let Ok(e) = events_rx.try_recv() {
            let end = matches!(e, ChatEvent::Done { .. } | ChatEvent::Failed { .. });
            events.push(e);
            if end {
                worker.join().expect("worker");
                let history = match events.last() {
                    Some(ChatEvent::Done { history }) | Some(ChatEvent::Failed { history, .. }) => history.clone(),
                    _ => History::default(),
                };
                return (events, history);
            }
        }
    }
}


fn endpoint(service: Service, url: &str) -> Endpoint {
    Endpoint { service, base_url: url.to_string(), key: SecretString::new("test-key-123") }
}

#[test]
fn claude_calls_a_tool_sees_the_result_and_answers() {
    let (url, seen) = server(vec![
        r#"{"stop_reason":"tool_use","content":[{"type":"thinking","thinking":"plan","signature":"sig"},{"type":"text","text":"Jeg bytter utforming."},{"type":"tool_use","id":"tu_1","name":"cv_set_layout","input":{"layout":"compact"}}]}"#,
        r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"Ferdig: Kompakt."}]}"#,
    ]);
    let mut project = Project::example();
    let backend = Backend::Api { endpoint: endpoint(Service::Claude, &url), model: "claude-opus-5-5".into() };
    let (events, history) = turn(backend, &mut project, "Bytt til kompakt");
    assert_eq!(project.options.layout, "compact", "the tool ran on the project");
    let texts: Vec<&str> = events.iter().filter_map(|e| if let ChatEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
    assert_eq!(texts, ["Jeg bytter utforming.", "Ferdig: Kompakt."]);
    assert!(events.iter().any(|e| matches!(e, ChatEvent::ToolDone { ok: true, .. })));
    let requests = seen.lock().expect("lock").clone();
    assert_eq!(requests.len(), 2);
    let first = &requests[0].1;
    assert_eq!(requests[0].0, "/v1/messages");
    assert!(first["tools"].as_array().expect("tools").iter().any(|t| t["name"] == "cv_set_layout" && t["input_schema"]["type"] == "object"));
    assert_eq!(first["thinking"]["type"], "adaptive");
    // The second request repeats the assistant turn unchanged (thinking included) and answers the call.
    let second = &requests[1].1["messages"];
    assert_eq!(second[1]["content"][0]["type"], "thinking");
    assert_eq!(second[2]["content"][0]["type"], "tool_result");
    assert_eq!(second[2]["content"][0]["tool_use_id"], "tu_1");
    assert_eq!(history.messages.len(), 4, "user, assistant, tool results, assistant");
}

#[test]
fn openai_compatible_services_call_tools_the_same_way() {
    for service in [Service::ChatGpt, Service::Gemini, Service::Grok] {
        let (url, seen) = server(vec![
            r#"{"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"cv_update_profile","arguments":"{\"summary\":\"Kort profil.\"}"}}]}}]}"#,
            r#"{"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"Profilen er endret."}}]}"#,
        ]);
        let mut project = Project::example();
        let backend = Backend::Api { endpoint: endpoint(service, &url), model: "m".into() };
        let (events, _) = turn(backend, &mut project, "Kortere profil");
        assert_eq!(project.candidate.summary, "Kort profil.", "{service:?}");
        assert!(events.iter().any(|e| matches!(e, ChatEvent::Text(t) if t == "Profilen er endret.")), "{service:?}");
        let requests = seen.lock().expect("lock").clone();
        assert_eq!(requests[0].0, "/v1/chat/completions");
        assert_eq!(requests[0].1["messages"][0]["role"], "system");
        assert_eq!(requests[0].1["tools"][0]["type"], "function");
        let second = &requests[1].1["messages"];
        let tool = second.as_array().expect("messages").iter().find(|m| m["role"] == "tool").expect("tool result");
        assert_eq!(tool["tool_call_id"], "call_1");
    }
}

#[test]
fn a_bad_tool_call_is_reported_back_to_the_model_not_applied() {
    let (url, seen) = server(vec![
        r#"{"stop_reason":"tool_use","content":[{"type":"tool_use","id":"tu_1","name":"cv_job_save","input":{"id":"no-such-job","start":"soon"}}]}"#,
        r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"Beklager."}]}"#,
    ]);
    let mut project = Project::example();
    let before = project.clone();
    let backend = Backend::Api { endpoint: endpoint(Service::Claude, &url), model: "m".into() };
    let (events, _) = turn(backend, &mut project, "x");
    assert_eq!(project, before, "nothing changed");
    assert!(events.iter().any(|e| matches!(e, ChatEvent::ToolDone { ok: false, .. })));
    let result = &seen.lock().expect("lock")[1].1["messages"][2]["content"][0];
    assert_eq!(result["is_error"], true);
}

#[test]
fn a_refusal_ends_the_turn_with_a_plain_message() {
    let (url, _) = server(vec![r#"{"stop_reason":"refusal","content":[]}"#]);
    let mut project = Project::example();
    let backend = Backend::Api { endpoint: endpoint(Service::Claude, &url), model: "m".into() };
    let (events, history) = turn(backend, &mut project, "x");
    assert!(matches!(events.last(), Some(ChatEvent::Failed { .. })));
    assert!(history.messages.iter().all(|m| m["role"] != "assistant"));
}
