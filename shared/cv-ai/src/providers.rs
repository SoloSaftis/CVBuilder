//! The four AI services a user can connect with their own API key.
//!
//! Claude is called through Anthropic's own Messages API (there is no official Rust SDK, so this
//! is plain HTTPS). ChatGPT, Gemini and Grok all offer an OpenAI-compatible endpoint and are
//! called through the Craft `craft-ai-openai` adapter, which sends the key only to the configured
//! address, never follows redirects and removes the key from error messages.
use std::time::{Duration, Instant};

use craft_ai_core::{CapabilityId, Event, InputRole, JobState, ModelId, Payload, Provider as _, Request, SecretString};
use craft_ai_openai::{OpenAiConfig, OpenAiProvider};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// An API key that never prints itself.
pub use craft_ai_core::SecretString as Secret;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Service {
    Claude,
    ChatGpt,
    Gemini,
    Grok,
}

pub const SERVICES: [Service; 4] = [Service::Claude, Service::ChatGpt, Service::Gemini, Service::Grok];

impl Service {
    pub fn name(self) -> &'static str {
        match self {
            Service::Claude => "Claude",
            Service::ChatGpt => "ChatGPT",
            Service::Gemini => "Gemini",
            Service::Grok => "Grok",
        }
    }
    pub fn company(self) -> &'static str {
        match self {
            Service::Claude => "Anthropic",
            Service::ChatGpt => "OpenAI",
            Service::Gemini => "Google",
            Service::Grok => "xAI",
        }
    }
    /// Where a person creates an API key.
    pub fn key_page(self) -> &'static str {
        match self {
            Service::Claude => "https://console.anthropic.com/settings/keys",
            Service::ChatGpt => "https://platform.openai.com/api-keys",
            Service::Gemini => "https://aistudio.google.com/app/apikey",
            Service::Grok => "https://console.x.ai",
        }
    }
    pub fn base_url(self) -> &'static str {
        match self {
            Service::Claude => "https://api.anthropic.com/v1",
            Service::ChatGpt => "https://api.openai.com/v1",
            Service::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Service::Grok => "https://api.x.ai/v1",
        }
    }
    /// The model chosen when the service's list offers it; otherwise the first preferred match.
    pub fn preferred(self) -> &'static [&'static str] {
        match self {
            Service::Claude => &["claude-opus-5-5"],
            Service::ChatGpt => &["gpt-5", "gpt-4.1", "gpt-4o"],
            Service::Gemini => &["gemini-2.5-pro", "gemini-pro", "gemini"],
            Service::Grok => &["grok-4", "grok-3", "grok"],
        }
    }
    pub fn secret_name(self) -> &'static str {
        match self {
            Service::Claude => "cvbuilder-claude",
            Service::ChatGpt => "cvbuilder-chatgpt",
            Service::Gemini => "cvbuilder-gemini",
            Service::Grok => "cvbuilder-grok",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AiFailure {
    #[error("Nøkkelen ble ikke godtatt. Sjekk at du har limt inn hele API-nøkkelen.")]
    BadKey,
    #[error("{0} er opptatt eller har nådd en grense. Vent litt og prøv igjen.")]
    Busy(&'static str),
    #[error("Fikk ikke kontakt med {0}. Sjekk internettforbindelsen.")]
    Network(&'static str),
    #[error("{0} avslo forespørselen. Prøv en annen formulering eller en annen tjeneste.")]
    Refused(&'static str),
    #[error("{0} svarte, men ikke i formatet CVBuilder trenger. Prøv igjen.")]
    Unreadable(&'static str),
    #[error("{0}: {1}")]
    Other(&'static str, String),
}

/// Overridable addresses, so tests can point at a local server.
#[derive(Clone)]
pub struct Endpoint {
    pub service: Service,
    pub base_url: String,
    pub key: SecretString,
}

impl Endpoint {
    pub fn new(service: Service, key: SecretString) -> Endpoint {
        Endpoint { service, base_url: service.base_url().to_string(), key }
    }
}

const ANTHROPIC_VERSION: &str = "2026-10-21";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).max_redirects(0).http_status_as_error(false).build().new_agent()
}

fn status_error(service: Service, status: u16, body: &str) -> AiFailure {
    match status {
        401 | 403 => AiFailure::BadKey,
        429 | 529 | 503 => AiFailure::Busy(service.name()),
        _ => {
            // Never echo a body that could contain the key; keep only the service's own message.
            let message = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["error"]["message"].as_str().map(str::to_string)).unwrap_or_else(|| format!("HTTP {status}"));
            AiFailure::Other(service.name(), message.chars().take(240).collect())
        }
    }
}

/// Checks the key by listing the service's models. Returns model ids, best guess first.
pub fn test_connection(endpoint: &Endpoint) -> Result<Vec<String>, AiFailure> {
    let service = endpoint.service;
    let mut ids: Vec<String> = if service == Service::Claude {
        let mut reply = agent()
            .get(format!("{}/models", endpoint.base_url))
            .header("x-api-key", endpoint.key.expose())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .call()
            .map_err(|_| AiFailure::Network(service.name()))?;
        let status = reply.status().as_u16();
        let body = reply.body_mut().read_to_string().unwrap_or_default();
        if status != 200 {
            return Err(status_error(service, status, &body));
        }
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|_| AiFailure::Unreadable(service.name()))?;
        value["data"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect()).unwrap_or_default()
    } else {
        let config = OpenAiConfig::new(endpoint.base_url.clone()).with_key(endpoint.key.clone());
        let listing = craft_ai_openai::probe::list_models(&config).map_err(|e| openai_error(service, &e))?;
        listing.ids.into_iter().map(|id| id.trim_start_matches("models/").to_string()).collect()
    };
    ids.sort_by_key(|id| service.preferred().iter().position(|p| id.starts_with(p)).unwrap_or(usize::MAX));
    if ids.is_empty() {
        return Err(AiFailure::Other(service.name(), "fant ingen modeller for denne nøkkelen".into()));
    }
    Ok(ids)
}

fn openai_error(service: Service, error: &craft_ai_core::AiError) -> AiFailure {
    use craft_ai_core::ErrorKind;
    match error.kind {
        ErrorKind::Auth => AiFailure::BadKey,
        ErrorKind::RateLimited | ErrorKind::Quota => AiFailure::Busy(service.name()),
        ErrorKind::Network => AiFailure::Network(service.name()),
        _ => AiFailure::Other(service.name(), error.message.chars().take(240).collect()),
    }
}

/// One instruction plus one message; returns the model's text answer.
pub fn complete(endpoint: &Endpoint, model: &str, system: &str, user: &str) -> Result<String, AiFailure> {
    let service = endpoint.service;
    if service == Service::Claude {
        return complete_claude(endpoint, model, system, user);
    }
    let config = OpenAiConfig::new(endpoint.base_url.clone()).with_key(endpoint.key.clone());
    let provider = OpenAiProvider::new(config).map_err(|e| openai_error(service, &e))?;
    let model_id = ModelId::new(model).map_err(|e| AiFailure::Other(service.name(), e.to_string()))?;
    let capability = CapabilityId::parse("text.complete").map_err(|e| AiFailure::Other(service.name(), e.to_string()))?;
    let request = Request::new(capability)
        .with_input(InputRole::System, Payload::text(system))
        .with_input(InputRole::Prompt, Payload::text(user))
        .with_param("response_format", json!({"type": "json_object"}));
    let job = provider.submit(&model_id, &request).map_err(|e| openai_error(service, &e))?;
    let started = Instant::now();
    let mut text = String::new();
    loop {
        let snapshot = provider.poll(&job).map_err(|e| openai_error(service, &e))?;
        for event in snapshot.events {
            match event {
                Event::TextDelta { text: t } => text.push_str(&t),
                Event::Output(output) => match output.payload {
                    Payload::Text { text: t } => text.push_str(&t),
                    Payload::Inline { data, .. } => text.push_str(&String::from_utf8_lossy(&data)),
                    _ => {}
                },
                _ => {}
            }
        }
        match snapshot.state {
            JobState::Succeeded => return Ok(text),
            JobState::Failed { error } => return Err(openai_error(service, &error)),
            JobState::Cancelled => return Err(AiFailure::Other(service.name(), "avbrutt".into())),
            _ if started.elapsed() > Duration::from_secs(180) => {
                let _ = provider.cancel(&job);
                return Err(AiFailure::Network(service.name()));
            }
            _ => std::thread::sleep(Duration::from_millis(250)),
        }
    }
}

/// Claude's Messages API. `fallbacks: "default"` lets a request that a safety classifier
/// declines by mistake be answered by Anthropic's recommended fallback model instead.
fn complete_claude(endpoint: &Endpoint, model: &str, system: &str, user: &str) -> Result<String, AiFailure> {
    let service = Service::Claude;
    let body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": system,
        "messages": [{"role": "user", "content": user}],
        "fallbacks": "default",
    });
    let mut reply = agent()
        .post(format!("{}/messages", endpoint.base_url))
        .header("x-api-key", endpoint.key.expose())
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|_| AiFailure::Network(service.name()))?;
    let status = reply.status().as_u16();
    let text = reply.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        return Err(status_error(service, status, &text));
    }
    parse_claude(&text)
}

/// Reads a Messages API reply. The stop reason is checked before the content is used.
pub fn parse_claude(body: &str) -> Result<String, AiFailure> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| AiFailure::Unreadable("Claude"))?;
    match value["stop_reason"].as_str() {
        Some("refusal") => return Err(AiFailure::Refused("Claude")),
        Some("max_tokens") => return Err(AiFailure::Other("Claude", "svaret ble for langt".into())),
        _ => {}
    }
    let text: String = value["content"].as_array().map(|blocks| blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect()).unwrap_or_default();
    if text.trim().is_empty() {
        return Err(AiFailure::Unreadable("Claude"));
    }
    Ok(text)
}
