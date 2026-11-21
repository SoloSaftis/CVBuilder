//! «Se etter oppdateringer»: asks GitHub for CVBuilder's newest release, only when the person
//! clicks. Nothing is downloaded or installed by itself; the app shows the answer and a link.
use std::time::Duration;

/// Where CVBuilder's releases are published.
pub const RELEASES_API: &str = "https://api.github.com/repos/SoloSaftis/CVBuilder/releases/latest";
/// The page with every version, for when the newest one has no page of its own.
pub const RELEASES_PAGE: &str = "https://github.com/SoloSaftis/CVBuilder/releases/latest";

/// The newest published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Like «1.0.1», without the «v».
    pub version: String,
    /// Its page on GitHub, with the downloads.
    pub page: String,
}

/// GitHub could not be reached from here (no connection, or it took too long).
pub const OFFLINE: &str = "Fikk ikke kontakt med GitHub. Sjekk internettforbindelsen og prøv igjen.";
/// GitHub itself has trouble; it usually passes quickly.
pub const DOWN: &str = "GitHub kunne ikke nås akkurat nå. Det går som regel fort over – prøv igjen om noen minutter.";
/// GitHub limits how often one computer may ask (60 times an hour).
pub const LIMITED: &str = "GitHub har fått mange spørsmål fra denne maskinen og svarer ikke en stund. Prøv igjen om en time, eller åpne nedlastingssiden.";
/// GitHub answered, but not with a version.
pub const UNCLEAR: &str = "GitHub svarte, men uten en versjon CVBuilder kan lese. Åpne nedlastingssiden for å se den nyeste versjonen.";

/// What went wrong, in plain words, with GitHub's own answer on a «Detaljer» line. `status` is
/// the HTTP status when GitHub answered at all.
pub fn failure(status: Option<u16>, technical: &str) -> String {
    let text = match status {
        None => OFFLINE,
        Some(403 | 429) => LIMITED,
        Some(500..=599) => DOWN,
        Some(_) => UNCLEAR,
    };
    cv_core::plain::with_details(text, technical)
}

/// Asks GitHub for the newest release (a few seconds at most). The error is a message for the
/// person (see [`failure`]).
pub fn latest() -> Result<Release, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .user_agent(concat!("CVBuilder/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    let mut response = agent.get(RELEASES_API).header("Accept", "application/vnd.github+json").call().map_err(|e| failure(None, &e.to_string()))?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().map_err(|e| failure(None, &e.to_string()))?;
    if status != 200 {
        return Err(failure(Some(status), &format!("GitHub answered {status}")));
    }
    parse(&body).map_err(|e| failure(Some(status), &e))
}

/// The release in GitHub's answer.
pub fn parse(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().filter(|t| !t.is_empty()).ok_or("no version in GitHub's answer")?;
    let page = v["html_url"].as_str().unwrap_or(RELEASES_PAGE).to_string();
    Ok(Release { version: tag.trim_start_matches(['v', 'V']).to_string(), page })
}

/// Whether `newest` is a later version than `current` («1.0.10» is after «1.0.9»; a test
/// version such as «1.1.0-beta.1» counts as its plain number).
pub fn is_newer(newest: &str, current: &str) -> bool {
    let numbers = |s: &str| -> Vec<u64> { s.trim_start_matches(['v', 'V']).split(['-', '+']).next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (numbers(newest), numbers(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("v1.0.10", "1.0.9"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("1.0", "1.0.0"));
    }

    #[test]
    fn reads_githubs_answer() {
        let r = parse(r#"{"tag_name":"v1.2.0","html_url":"https://github.com/SoloSaftis/CVBuilder/releases/tag/v1.2.0","name":"CVBuilder 1.2.0"}"#).expect("release");
        assert_eq!(r.version, "1.2.0");
        assert!(r.page.ends_with("/v1.2.0"));
        assert!(parse("{}").is_err());
    }

    #[test]
    fn failures_are_plain_words_with_details() {
        let say = |status, technical| failure(status, technical);
        assert_eq!(say(None, "io: Connection refused (os error 10061)"), format!("{OFFLINE}\nDetaljer: io: Connection refused (os error 10061)"));
        assert!(say(Some(503), "GitHub answered 503").starts_with(DOWN));
        assert!(say(Some(403), "GitHub answered 403").starts_with(LIMITED));
        assert!(say(Some(429), "GitHub answered 429").starts_with(LIMITED));
        assert!(say(Some(200), "no version in GitHub's answer").starts_with(UNCLEAR));
        for text in [OFFLINE, DOWN, LIMITED, UNCLEAR] {
            assert!(!text.contains("503") && !text.contains("JSON") && !text.contains("HTTP"), "{text}");
        }
    }
}
