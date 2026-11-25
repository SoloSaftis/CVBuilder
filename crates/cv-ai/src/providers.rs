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
    /// A model running on the person's own computer (or network) behind an OpenAI-compatible
    /// server: Ollama, LM Studio, llama.cpp, Jan, vLLM, LocalAI …
    Local,
}

pub const SERVICES: [Service; 5] = [Service::Claude, Service::ChatGpt, Service::Gemini, Service::Grok, Service::Local];

impl Service {
    pub fn name(self) -> &'static str {
        match self {
            Service::Claude => "Claude",
            Service::ChatGpt => "ChatGPT",
            Service::Gemini => "Gemini",
            Service::Grok => "Grok",
            Service::Local => "Lokal modell",
        }
    }
    pub fn company(self) -> &'static str {
        match self {
            Service::Claude => "Anthropic",
            Service::ChatGpt => "OpenAI",
            Service::Gemini => "Google",
            Service::Grok => "xAI",
            Service::Local => "din lokale AI-server",
        }
    }
    /// Where a person creates an API key.
    pub fn key_page(self) -> &'static str {
        match self {
            Service::Claude => "https://console.anthropic.com/settings/keys",
            Service::ChatGpt => "https://platform.openai.com/api-keys",
            Service::Gemini => "https://aistudio.google.com/app/apikey",
            Service::Grok => "https://console.x.ai",
            Service::Local => "https://ollama.com/download",
        }
    }
    pub fn base_url(self) -> &'static str {
        match self {
            Service::Claude => "https://api.anthropic.com/v1",
            Service::ChatGpt => "https://api.openai.com/v1",
            Service::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Service::Grok => "https://api.x.ai/v1",
            Service::Local => "http://localhost:11434/v1",
        }
    }
    /// The model chosen when the service's list offers it; otherwise the first preferred match.
    pub fn preferred(self) -> &'static [&'static str] {
        match self {
            Service::Claude => &["claude-opus-5-5"],
            Service::ChatGpt => &["gpt-5", "gpt-4.1", "gpt-4o"],
            Service::Gemini => &["gemini-2.5-pro", "gemini-pro", "gemini"],
            Service::Grok => &["grok-4", "grok-3", "grok"],
            // Models that are good at tool calls come first.
            Service::Local => &["qwen3", "qwen2.5", "llama3.3", "llama3.1", "mistral-small", "mistral-nemo", "gpt-oss", "gemma3", "command-r"],
        }
    }
    pub fn secret_name(self) -> &'static str {
        match self {
            Service::Claude => "cvbuilder-claude",
            Service::ChatGpt => "cvbuilder-chatgpt",
            Service::Gemini => "cvbuilder-gemini",
            Service::Grok => "cvbuilder-grok",
            Service::Local => "cvbuilder-local",
        }
    }
    /// Local servers usually need no key.
    pub fn needs_key(self) -> bool {
        self != Service::Local
    }
}

/// Well-known local AI servers and their OpenAI-compatible addresses.
pub const LOCAL_SERVERS: [(&str, &str); 6] = [
    ("Ollama", "http://localhost:11434/v1"),
    ("LM Studio", "http://localhost:1234/v1"),
    ("llama.cpp", "http://localhost:8080/v1"),
    ("Jan", "http://localhost:1337/v1"),
    ("vLLM", "http://localhost:8000/v1"),
    ("LocalAI", "http://localhost:8081/v1"),
];

/// An AI address read by a real URL parser: only `http` and `https`, with a host and without a
/// user name or password (`http://localhost:x@evil.example` is a way to hide the real host).
fn parse_address(url: &str) -> Option<url::Url> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    let plain = matches!(parsed.scheme(), "http" | "https") && parsed.username().is_empty() && parsed.password().is_none() && parsed.host().is_some();
    plain.then_some(parsed)
}

/// Whether an address stays on this computer: `localhost`, a name under `.localhost`,
/// `127.0.0.0/8` or `[::1]`. Anything else, a user name in the address, or an address that does
/// not parse, is not local, so the person is told where the text goes.
pub fn is_local_address(url: &str) -> bool {
    let Some(parsed) = parse_address(url) else {
        return false;
    };
    match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip == std::net::Ipv6Addr::LOCALHOST,
        // The parser has already lowered the case. A trailing dot or an empty label is not
        // taken as local.
        Some(url::Host::Domain(name)) => name == "localhost" || name.strip_suffix(".localhost").is_some_and(|rest| !rest.is_empty() && rest.split('.').all(|label| !label.is_empty())),
        None => false,
    }
}

/// Where requests to an address go, for remembering a yes: scheme, host and port, normalised
/// («HTTP://LocalHost:80/v1» and «http://localhost/» are the same place). None when the address
/// does not parse or names a user.
pub fn endpoint_origin(url: &str) -> Option<String> {
    Some(parse_address(url)?.origin().ascii_serialization())
}

/// The address to send to, exactly as the check read it («http://localhost:11434/v1»), so the
/// HTTP client cannot read a different host from the same text. None when it does not parse or
/// names a user.
pub fn normalized_address(url: &str) -> Option<String> {
    Some(parse_address(url)?.as_str().trim_end_matches('/').to_string())
}

/// Where an address really points, to show the person: scheme, host and port, never a user name
/// or password («http://evil.example» for «http://localhost:x@evil.example/v1»).
pub fn shown_origin(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    parsed.host()?;
    Some(parsed.origin().ascii_serialization())
}

/// The host (and port, if not the default) to show for an address, without any user name or
/// password: «192.168.1.20:11434».
pub fn address_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    let host = parsed.host_str()?.to_string();
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

/// Plain `http://` to a computer other than this one: anyone on the way can read the text.
pub fn is_unencrypted(url: &str) -> bool {
    !is_local_address(url) && url::Url::parse(url.trim()).is_ok_and(|u| u.scheme() == "http")
}

/// The well-known local servers that answer right now, with their model counts.
pub fn detect_local() -> Vec<(String, String, usize)> {
    let handles: Vec<_> = LOCAL_SERVERS
        .iter()
        .map(|(name, url)| {
            let (name, url) = (name.to_string(), url.to_string());
            std::thread::spawn(move || {
                let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(900))).max_redirects(0).http_status_as_error(false).build().new_agent();
                let mut reply = agent.get(format!("{url}/models")).call().ok()?;
                if reply.status().as_u16() != 200 {
                    return None;
                }
                let v: serde_json::Value = serde_json::from_str(&reply.body_mut().read_to_string().ok()?).ok()?;
                let count = v["data"].as_array().map(Vec::len).unwrap_or(0);
                Some((name, url, count))
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
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
    #[error("{0} fant ingen modeller for denne nøkkelen. Sjekk at nøkkelen hører til denne tjenesten, eller velg en annen tjeneste.")]
    NoModels(&'static str),
    #[error("{0} avbrøt svaret før det var ferdig. Prøv igjen.")]
    Cancelled(&'static str),
    #[error("Svaret fra {0} ble for langt og ble kuttet. Prøv igjen med kortere tekst, eller velg en annen modell.")]
    TooLong(&'static str),
    #[error("Oversettelsen fra {0} manglet deler av CV-en. Prøv igjen, eller velg en annen modell.")]
    Incomplete(&'static str),
    #[error("CVBuilder fant ikke jobben. Velg jobben i listen på nytt og prøv igjen.")]
    NoJob,
    #[error("{0}: {1}")]
    Other(&'static str, String),
    /// The service answered with an error of its own; its words go on the «Detaljer» line.
    #[error("{0} svarte med en feil. Prøv igjen om litt, eller velg en annen modell.\nDetaljer: {1}")]
    Failed(&'static str, String),
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

pub(crate) const ANTHROPIC_VERSION: &str = "2026-10-21";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).max_redirects(0).http_status_as_error(false).build().new_agent()
}

pub(crate) fn status_error(service: Service, status: u16, body: &str) -> AiFailure {
    match status {
        401 | 403 => AiFailure::BadKey,
        429 | 529 | 503 => AiFailure::Busy(service.name()),
        _ => {
            // Never echo a body that could contain the key; keep only the service's own message.
            let message = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v["error"]["message"].as_str().map(str::to_string)).unwrap_or_else(|| format!("HTTP {status}"));
            AiFailure::Failed(service.name(), message.chars().take(240).collect())
        }
    }
}

/// Checks the key by listing the service's models. Returns model ids, best guess first.
pub fn test_connection(endpoint: &Endpoint) -> Result<Vec<String>, AiFailure> {
    let service = endpoint.service;
    let mut ids: Vec<String> = if service == Service::Local {
        let mut request = agent().get(format!("{}/models", endpoint.base_url.trim_end_matches('/')));
        if !endpoint.key.expose().is_empty() {
            request = request.header("authorization", &format!("Bearer {}", endpoint.key.expose()));
        }
        let mut reply = request.call().map_err(|_| AiFailure::Other(service.name(), format!("fikk ikke kontakt med {}. Kjører Ollama, LM Studio eller serveren din?", endpoint.base_url)))?;
        let status = reply.status().as_u16();
        let body = reply.body_mut().read_to_string().unwrap_or_default();
        if status != 200 {
            return Err(status_error(service, status, &body));
        }
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|_| AiFailure::Unreadable(service.name()))?;
        value["data"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect()).unwrap_or_default()
    } else if service == Service::Claude {
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
        return Err(AiFailure::NoModels(service.name()));
    }
    Ok(ids)
}

fn openai_error(service: Service, error: &craft_ai_core::AiError) -> AiFailure {
    use craft_ai_core::ErrorKind;
    match error.kind {
        ErrorKind::Auth => AiFailure::BadKey,
        ErrorKind::RateLimited | ErrorKind::Quota => AiFailure::Busy(service.name()),
        ErrorKind::Network => AiFailure::Network(service.name()),
        _ => AiFailure::Failed(service.name(), error.message.chars().take(240).collect()),
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
    let model_id = ModelId::new(model).map_err(|e| AiFailure::Failed(service.name(), e.to_string()))?;
    let capability = CapabilityId::parse("text.complete").map_err(|e| AiFailure::Failed(service.name(), e.to_string()))?;
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
            JobState::Cancelled => return Err(AiFailure::Cancelled(service.name())),
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
        Some("max_tokens") => return Err(AiFailure::TooLong("Claude")),
        _ => {}
    }
    let text: String = value["content"].as_array().map(|blocks| blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect()).unwrap_or_default();
    if text.trim().is_empty() {
        return Err(AiFailure::Unreadable("Claude"));
    }
    Ok(text)
}

/// Fetches a web page (a job ad) as text. Only http and https (a link without one gets
/// https://); follows up to 5 redirects; at most 5 MB and 20 seconds; no keys are sent.
/// Every failure is a message the person can act on.
pub fn fetch_page(url: &str) -> Result<String, String> {
    fetch_page_within(url, Duration::from_secs(20))
}

/// [`fetch_page`] with its own time limit (tests use a short one).
pub fn fetch_page_within(url: &str, limit: Duration) -> Result<String, String> {
    let mut url = url.trim().trim_matches(['<', '>', '"', '\'']).to_string();
    if url.is_empty() {
        return Err("Lim inn lenken til annonsen i feltet over først.".into());
    }
    let lower = url.to_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        // «finn.no/job/…» or «www.nav.no/…» as copied from somewhere without the scheme.
        if lower.contains("://") || url.contains(char::is_whitespace) || !url.split('/').next().is_some_and(|host| host.contains('.')) {
            return Err("Lenken må starte med https://".into());
        }
        url = format!("https://{url}");
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(limit)).max_redirects(5).http_status_as_error(false).user_agent("Mozilla/5.0 (compatible; CVBuilder)").build().new_agent();
    let mut reply = agent.get(&url).header("accept", "text/html,application/xhtml+xml,text/plain;q=0.8,*/*;q=0.5").header("accept-language", "nb-NO,nb;q=0.9,no;q=0.8,en;q=0.7").call().map_err(|e| match e {
        ureq::Error::Timeout(_) => format!("Siden svarte ikke innen {} sekunder. Prøv igjen, eller kopier annonseteksten og lim den inn.", limit.as_secs().max(1)),
        ureq::Error::HostNotFound => "Fant ikke nettstedet. Sjekk at lenken er riktig.".to_string(),
        ureq::Error::TooManyRedirects | ureq::Error::RedirectFailed => "Siden sendte videre for mange ganger. Åpne den i nettleseren, kopier teksten og lim den inn.".to_string(),
        ureq::Error::BadUri(_) => "Lenken er ikke gyldig. Kopier hele lenken fra nettleseren.".to_string(),
        _ => "Fikk ikke hentet siden. Sjekk lenken og internettforbindelsen.".to_string(),
    })?;
    let status = reply.status().as_u16();
    match status {
        200..=299 => {}
        401 | 403 => return Err("Siden krever innlogging, så annonsen kunne ikke hentes. Åpne den i nettleseren, kopier teksten og lim den inn.".into()),
        404 | 410 => return Err("Fant ikke annonsen på lenken (feil 404). Den kan være tatt ned, eller lenken er feil.".into()),
        429 => return Err("Nettstedet vil ikke svare akkurat nå (for mange forespørsler). Vent litt, eller kopier annonseteksten og lim den inn.".into()),
        _ => return Err(format!("Siden svarte med feil {status}. Noen nettsteder krever innlogging; lim da inn annonseteksten selv.")),
    }
    let kind = reply.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or_default().to_lowercase();
    if kind.contains("application/pdf") {
        return Err("Lenken går til en PDF. Last den ned og hent den inn med «Fra fil».".into());
    }
    let text_like = kind.is_empty() || kind.starts_with("text/") || kind.contains("html") || kind.contains("xml") || kind.contains("json");
    if !text_like {
        return Err("Lenken går ikke til en nettside med tekst. Kopier annonseteksten og lim den inn.".into());
    }
    let bytes = reply.body_mut().with_config().limit(5_000_000).read_to_vec().map_err(|e| match e {
        ureq::Error::BodyExceedsLimit(_) => "Siden er for stor (over 5 MB). Kopier annonseteksten og lim den inn.".to_string(),
        ureq::Error::Timeout(_) => format!("Siden svarte ikke innen {} sekunder. Prøv igjen, eller kopier annonseteksten og lim den inn.", limit.as_secs().max(1)),
        _ => "Fikk ikke hentet hele siden. Prøv igjen.".to_string(),
    })?;
    if bytes.starts_with(b"%PDF-") {
        return Err("Lenken går til en PDF. Last den ned og hent den inn med «Fra fil».".into());
    }
    // The page's own encoding (UTF-8, or Windows-1252 on older sites).
    let text = cv_core::import::decode_text(&bytes);
    if kind.starts_with("text/plain") {
        // Plain text is read as an ad as it is; escaping keeps «<» from looking like a tag.
        return Ok(format!("<pre>{}</pre>", text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_error_says_what_to_do() {
        assert!(matches!(status_error(Service::Claude, 401, ""), AiFailure::BadKey));
        assert!(matches!(status_error(Service::Claude, 403, ""), AiFailure::BadKey));
        for busy in [429, 503, 529] {
            assert!(matches!(status_error(Service::Claude, busy, ""), AiFailure::Busy("Claude")), "{busy}");
        }
        // The service's own words go on the «Detaljer» line, under a message with a next step.
        let failed = status_error(Service::Claude, 400, r#"{"error":{"message":"model not found"}}"#).to_string();
        assert_eq!(failed, "Claude svarte med en feil. Prøv igjen om litt, eller velg en annen modell.\nDetaljer: model not found");
        // A body that is not the service's error format is never echoed (it could hold the key).
        let raw = status_error(Service::Claude, 500, "sk-ant-secret <html>").to_string();
        assert!(raw.ends_with("\nDetaljer: HTTP 500"), "{raw}");
        assert!(!raw.contains("sk-ant"), "{raw}");
        // A long message is cut short.
        let long = status_error(Service::Claude, 400, &format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(1000))).to_string();
        assert!(long.len() < 400, "{}", long.len());
    }

    #[test]
    fn failures_say_what_happened_and_what_to_do() {
        for failure in [AiFailure::NoModels("ChatGPT"), AiFailure::Cancelled("ChatGPT"), AiFailure::TooLong("Claude"), AiFailure::Incomplete("Gemini"), AiFailure::NoJob] {
            let text = failure.to_string();
            let lower = text.to_lowercase();
            assert!(lower.contains("prøv igjen") || lower.contains("velg en annen") || lower.contains("sjekk"), "no next step: {text}");
            assert!(!text.contains(": "), "no raw «service: text»: {text}");
        }
        assert!(matches!(parse_claude(r#"{"stop_reason":"max_tokens","content":[]}"#), Err(AiFailure::TooLong("Claude"))));
    }
}
