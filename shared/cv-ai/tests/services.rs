//! The AI layer against a local stand-in server: no real keys, no network, no cost.
//! Each test checks what is sent (address, key header, safeguards) and how replies are read.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use craft_ai_core::SecretString;
use cv_ai::providers::{AiFailure, Endpoint, Service, complete, parse_claude, test_connection};
use cv_ai::tasks;
use cv_core::Project;

#[derive(Clone, Debug, Default)]
struct Seen {
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_default()
    }
}

/// Answers every request with `status` and `body`, and records what it received.
fn server(status: u16, body: &'static str) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://127.0.0.1:{}/v1", listener.local_addr().expect("addr").port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut line = String::new();
            reader.read_line(&mut line).ok();
            let path = line.split_whitespace().nth(1).unwrap_or_default().to_string();
            let mut headers = Vec::new();
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
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            let mut buf = vec![0; length];
            reader.read_exact(&mut buf).ok();
            log.lock().expect("lock").push(Seen { path, headers, body: String::from_utf8_lossy(&buf).into() });
            let mut stream = stream;
            let _ = write!(stream, "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
        }
    });
    (url, seen)
}

fn endpoint(service: Service, url: &str) -> Endpoint {
    Endpoint { service, base_url: url.to_string(), key: SecretString::new("test-key-123") }
}

#[test]
fn claude_request_carries_key_version_and_fallback_safeguard() {
    let (url, seen) = server(200, r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"{\"summary\":\"Hei\"}"}]}"#);
    let text = complete(&endpoint(Service::Claude, &url), "claude-opus-5-5", "regler", "spørsmål").expect("reply");
    assert!(text.contains("Hei"));
    let request = seen.lock().expect("lock")[0].clone();
    assert_eq!(request.path, "/v1/messages");
    assert_eq!(request.header("x-api-key"), Some("test-key-123"));
    assert_eq!(request.header("anthropic-version"), Some("2026-10-21"));
    assert_eq!(request.header("anthropic-beta"), Some("server-side-fallback-2026-07-01"));
    let body = request.json();
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["fallbacks"], "default");
    assert_eq!(body["system"], "regler");
}

#[test]
fn claude_refusal_and_bad_key_are_explained() {
    assert!(matches!(parse_claude(r#"{"stop_reason":"refusal","content":[]}"#), Err(AiFailure::Refused(_))));
    let (url, _) = server(401, r#"{"error":{"message":"invalid x-api-key test-key-123"}}"#);
    let error = complete(&endpoint(Service::Claude, &url), "claude-opus-5-5", "s", "u").expect_err("401");
    assert!(matches!(error, AiFailure::BadKey));
    assert!(!error.to_string().contains("test-key-123"), "the key never appears in a message");
}

#[test]
fn openai_compatible_services_send_a_bearer_key_and_ask_for_json() {
    let (url, seen) = server(200, r#"{"choices":[{"message":{"content":"{\"bullets\":[\"Ledet idriftsettelse av [tall] anlegg.\",\"Feilsøkte Modbus.\"]}"}}],"usage":{"prompt_tokens":3,"completion_tokens":2}}"#);
    let project = Project::example();
    let bullets = tasks::improve_bullets(&endpoint(Service::Grok, &url), "grok-4", &project, 0).expect("bullets");
    assert_eq!(bullets.len(), 2);
    let request = seen.lock().expect("lock")[0].clone();
    assert_eq!(request.path, "/v1/chat/completions");
    assert_eq!(request.header("authorization"), Some("Bearer test-key-123"));
    let body = request.json();
    assert_eq!(body["model"], "grok-4");
    assert_eq!(body["response_format"]["type"], "json_object");
    assert_eq!(body["messages"][0]["role"], "system", "the honesty rules go first");
    assert!(body["messages"][0]["content"].as_str().unwrap_or_default().contains("Finn aldri på fakta"));
}

#[test]
fn an_ai_number_placeholder_blocks_export_until_filled_in() {
    let mut project = Project::example();
    project.experience[0].bullets[0] = "Ledet idriftsettelse av [tall] anlegg.".into();
    assert!(cv_core::checks::blocking(&project).iter().any(|e| e.contains("[tall]")));
}

#[test]
fn model_list_puts_the_recommended_model_first() {
    let (url, _) = server(200, r#"{"data":[{"id":"claude-haiku-5-5"},{"id":"claude-opus-5-5"},{"id":"claude-sonnet-5-5"}]}"#);
    let models = test_connection(&endpoint(Service::Claude, &url)).expect("models");
    assert_eq!(models[0], "claude-opus-5-5");
}

#[test]
fn a_translation_that_drops_parts_is_rejected() {
    let (url, _) = server(200, r#"{"stop_reason":"end_turn","content":[{"type":"text","text":"{\"headline\":\"H\",\"summary\":\"S\",\"jobs\":[],\"skills\":[],\"education\":[],\"languages\":[]}"}]}"#);
    let project = Project::example();
    assert!(tasks::translate(&endpoint(Service::Claude, &url), "claude-opus-5-5", &project).is_err());
}
