//! A rough idea of what one AI request costs on the person's own API key, so the window can say
//! it before anything is sent. It is an estimate from the size of what is sent, not a bill:
//! providers charge per token (about four characters of text), and each step of a chat turn
//! sends the rules, the tool list and the conversation so far again.

/// What the person asks the AI to do. Each kind sends the CV and gets a different amount back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Work {
    /// A message typed in the chat, or a small request (fagord, a profile, the bullets of one job).
    Chat,
    /// Tailoring the whole CV to the job ad.
    Tailor,
    /// Translating the whole CV.
    Translate,
    /// Filling in a new CV from the text of an old one.
    Import,
}

/// US dollars per million tokens: (input, output). The cheap end is a small model (Claude
/// Haiku 5.5, GPT mini, Gemini Flash), the dear end a top model like the ones CVBuilder picks
/// first (Claude Opus 5.5, GPT-5, Gemini Pro).
const CHEAP: (f64, f64) = (0.10, 0.50);
const DEAR: (f64, f64) = (4.0, 20.0);

/// Characters per token, roughly, for CV text and JSON.
const CHARS_PER_TOKEN: f64 = 4.0;

/// The tokens a request sends and gets back: (input, output).
pub fn tokens(work: Work, cv_chars: usize, extra_chars: usize) -> (f64, f64) {
    // The rules and the tool list go with every model call.
    let base = (cv_mcp::INSTRUCTIONS.len() + serde_json::to_string(&cv_mcp::tools().iter().map(|t| (&t.name, &t.description, &t.schema)).collect::<Vec<_>>()).map_or(0, |s| s.len())) as f64 / CHARS_PER_TOKEN;
    let cv = cv_chars as f64 / CHARS_PER_TOKEN;
    let extra = extra_chars as f64 / CHARS_PER_TOKEN;
    // How many model calls a turn usually takes, and how much text comes back.
    let (steps, out) = match work {
        Work::Chat => (4.0, 600.0 + cv * 0.2),
        Work::Tailor => (6.0, 800.0 + cv * 0.6),
        Work::Translate => (8.0, 400.0 + cv * 1.3),
        Work::Import => (8.0, 400.0 + extra * 1.2),
    };
    // Every call sends the rules again; the CV (read once with a tool) and the extra text stay in
    // the conversation from the first call on.
    let input = steps * base + steps * (cv + extra) + out * steps / 2.0;
    (input, out)
}

/// What the request costs, in US dollars, from a small model to a top one: (low, high).
pub fn dollars(work: Work, cv_chars: usize, extra_chars: usize) -> (f64, f64) {
    let (input, output) = tokens(work, cv_chars, extra_chars);
    let price = |(i, o): (f64, f64)| (input * i + output * o) / 1_000_000.0;
    (price(CHEAP), price(DEAR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_message_costs_cents_not_dollars() {
        let (low, high) = dollars(Work::Chat, 6_000, 0);
        assert!(low > 0.0 && low < 0.01, "{low}");
        assert!(high > 0.05 && high < 1.0, "{high}");
    }

    #[test]
    fn bigger_work_costs_more() {
        let chat = dollars(Work::Chat, 6_000, 0).1;
        let tailor = dollars(Work::Tailor, 6_000, 0).1;
        let translate = dollars(Work::Translate, 6_000, 0).1;
        assert!(chat < tailor && tailor < translate, "{chat} {tailor} {translate}");
        assert!(dollars(Work::Translate, 12_000, 0).1 > translate);
        assert!(dollars(Work::Import, 0, 20_000).1 > dollars(Work::Import, 0, 2_000).1);
    }

    #[test]
    fn the_range_is_honest_about_the_model() {
        let (low, high) = dollars(Work::Translate, 8_000, 0);
        assert!(high > low * 10.0, "a top model costs far more than a small one: {low} {high}");
        // A whole translation with a top model is well under a few dollars.
        assert!(high < 3.0, "{high}");
    }
}
