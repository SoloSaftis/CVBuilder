//! Where the CV goes and what it costs, said before the AI gets it. The chat shows it under the
//! AI picker; Translate, Import and «Dette må plasseres» show it next to their AI button; the
//! one-click AI buttons in the form ask once per kind of work and AI in a session (to-do 030).
//! A model on this computer sends nothing and costs nothing, so it never asks.
use cv_ai::Service;
use cv_ai::chat::Backend;
use cv_ai::cost::{self, Work};
use egui::RichText;

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

/// Where the text goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Destination {
    /// A model on this computer.
    Here,
    /// The person's own server elsewhere (their network or a rented machine).
    Server(String),
    /// A company, with the person's API key: paid per use.
    Provider(Service),
    /// Claude Code with the person's own sign-in: their Claude plan or credit.
    ClaudeCode,
}

impl Destination {
    pub fn of(backend: &Backend) -> Destination {
        match backend {
            Backend::Api { endpoint, .. } if endpoint.service == Service::Local && cv_ai::is_local_address(&endpoint.base_url) => Destination::Here,
            Backend::Api { endpoint, .. } if endpoint.service == Service::Local => Destination::Server(endpoint.base_url.clone()),
            Backend::Api { endpoint, .. } => Destination::Provider(endpoint.service),
            Backend::ClaudeCode { .. } => Destination::ClaudeCode,
        }
    }

    /// The text leaves this computer.
    pub fn leaves(&self) -> bool {
        *self != Destination::Here
    }

    /// The name on a «Send to …» button.
    pub fn name(&self) -> String {
        match self {
            Destination::Here => i18n::tdyn(Service::Local.name()),
            Destination::Server(url) => url.trim_start_matches("https://").trim_start_matches("http://").split('/').next().unwrap_or(url).to_string(),
            Destination::Provider(s) => s.company().to_string(),
            Destination::ClaudeCode => "Claude Code".to_string(),
        }
    }
}

/// A sum of US dollars, shown the same in every language («US$ 0.04»).
fn usd(x: f64) -> String {
    format!("US$ {x:.2}")
}

/// The rough cost, said after «roughly»: «under 1 US cent», «up to US$ 0.12», «US$ 0.02–0.50».
pub fn cost_words(low: f64, high: f64) -> String {
    if high < 0.01 {
        i18n::t("under 1 amerikansk cent").to_string()
    } else if low < 0.01 {
        i18n::tf("opptil {sum}", &[("sum", &usd(high))])
    } else {
        format!("{}–{high:.2}", usd(low))
    }
}

/// Where the text goes and what it may cost, in one or two plain sentences. `cv_chars` is the
/// size of the CV, `extra_chars` of other text sent along (an old CV to import).
pub fn notice(destination: &Destination, work: Work, cv_chars: usize, extra_chars: usize) -> String {
    match destination {
        Destination::Here => i18n::t("Blir på denne maskinen: den lokale modellen kjører her, og det koster ingenting.").to_string(),
        Destination::Server(url) => i18n::tf("CV-teksten sendes til AI-serveren din på {url}. CVBuilder tar ikke betalt for noe.", &[("url", url)]),
        Destination::Provider(service) => {
            let (low, high) = cost::dollars(work, cv_chars, extra_chars);
            let args = [("company", service.company()), ("service", &i18n::tdyn(service.name())), ("cost", &cost_words(low, high))];
            if work == Work::Chat {
                i18n::tf("CV-teksten sendes til {company} ({service}) med din API-nøkkel. {company} tar betalt på kontoen din, anslagsvis {cost} per melding, etter hvilken modell du bruker.", &args)
            } else {
                i18n::tf("CV-teksten sendes til {company} ({service}) med din API-nøkkel. {company} tar betalt på kontoen din, anslagsvis {cost} for dette, etter hvilken modell du bruker.", &args)
            }
        }
        Destination::ClaudeCode => i18n::t("CV-teksten sendes til Anthropic gjennom Claude Code, med kontoen du er logget inn med. Det bruker av kvoten i Claude-abonnementet ditt, eller kreditt hvis kontoen betaler per bruk.").to_string(),
    }
}

/// A request for the AI that waits for the person to say yes.
pub struct Pending {
    pub work: Work,
    pub text: String,
    pub destination: Destination,
    /// «La AI-en sortere kravene» instead of a chat message; `text` is then the ad.
    pub sort_requirements: bool,
}

impl CvApp {
    /// The size of the open CV as the AI reads it.
    pub(crate) fn cv_chars(&self) -> usize {
        self.project.to_json().map_or(4_000, |j| j.len())
    }

    /// The notice for the AI in use now, or None when no AI is connected.
    pub(crate) fn ai_notice(&self, work: Work, extra_chars: usize) -> Option<(Destination, String)> {
        let destination = Destination::of(&self.chat_backend()?);
        let text = notice(&destination, work, self.cv_chars(), extra_chars);
        Some((destination, text))
    }

    /// The notice as a small line in a dialog, with an icon that says it is about sending.
    pub(crate) fn notice_line(&self, ui: &mut egui::Ui, work: Work, extra_chars: usize) {
        if let Some((destination, text)) = self.ai_notice(work, extra_chars) {
            let t = Tokens::get(ui.ctx());
            let icon = if destination.leaves() { theme::ph::UPLOAD } else { theme::ph::INFO };
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(icon).size(14.0).color(t.dim));
                ui.add(egui::Label::new(RichText::new(text).size(12.5).color(t.dim)).wrap());
            });
        }
    }

    /// The person has seen where this kind of work goes, with the AI in use now: no need to ask.
    pub(crate) fn ai_seen(&mut self, work: Work) {
        if let Some(backend) = self.chat_backend() {
            self.ai_ok.insert((work, backend.name()));
        }
    }

    /// Whether a one-click request must ask first: the text leaves this computer and the person
    /// has not said yes to this kind of work with this AI in this session.
    pub(crate) fn must_ask(&self, work: Work) -> Option<Destination> {
        let backend = self.chat_backend()?;
        let destination = Destination::of(&backend);
        (destination.leaves() && !self.ai_ok.contains(&(work, backend.name()))).then_some(destination)
    }

    /// «Før AI-en starter»: where the CV goes and what it costs, with Send and Cancel.
    pub(crate) fn ai_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.ai_pending.take() else {
            return;
        };
        let mut answer: Option<bool> = None;
        let text = notice(&pending.destination, pending.work, self.cv_chars(), pending.text.len());
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.ai-confirm")).show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.heading(i18n::t("Før AI-en starter"));
            ui.add_space(4.0);
            ui.add(egui::Label::new(text).wrap());
            ui.add_space(4.0);
            theme::hint(ui, i18n::t("Du blir spurt én gang for hver slags oppgave. Alt AI-en endrer kan angres."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let send = i18n::tf("Send til {name}", &[("name", &pending.destination.name())]);
                if theme::primary(ui, &send, i18n::t("Send CV-teksten og spørsmålet til AI-en"), true).clicked() {
                    answer = Some(true);
                }
                let cancel = ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Ingenting sendes (Esc)"));
                crate::shortcuts::default_button(ui, &cancel);
                if cancel.clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer.or(r.should_close().then_some(false)) {
            None => self.ai_pending = Some(pending),
            Some(true) => {
                self.ai_seen(pending.work);
                if pending.sort_requirements {
                    self.sort_requirements_ai();
                } else {
                    self.ask_ai(pending.work, &pending.text);
                }
            }
            Some(false) => self.status = i18n::t("Ingenting ble sendt til AI-en.").into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Other tests switch the interface language at the same time, so these check only what
    // every language keeps: names, addresses and sums.

    #[test]
    fn a_model_here_stays_here_and_costs_nothing() {
        let text = notice(&Destination::Here, Work::Translate, 8_000, 0);
        assert!(!text.contains("US$"), "{text}");
        assert!(!Destination::Here.leaves());
    }

    #[test]
    fn a_provider_is_named_with_a_rough_cost() {
        let text = notice(&Destination::Provider(Service::ChatGpt), Work::Translate, 8_000, 0);
        assert!(text.contains("OpenAI") && text.contains("ChatGPT") && text.contains("US$"), "{text}");
        assert!(Destination::Provider(Service::Claude).leaves());
        assert_eq!(Destination::Provider(Service::Gemini).name(), "Google");
    }

    #[test]
    fn claude_code_and_own_servers_are_named() {
        assert!(notice(&Destination::ClaudeCode, Work::Chat, 1, 0).contains("Anthropic"));
        let server = Destination::Server("http://192.168.1.20:11434/v1".into());
        assert!(notice(&server, Work::Chat, 1, 0).contains("192.168.1.20"));
        assert_eq!(server.name(), "192.168.1.20:11434");
    }

    #[test]
    fn cost_words_cover_small_and_large_sums() {
        assert!(!cost_words(0.0001, 0.004).contains("US$"));
        let up_to = cost_words(0.001, 0.12);
        assert!(up_to.contains("US$ 0.12") && !up_to.contains("0.00"), "{up_to}");
        let range = cost_words(0.02, 0.5);
        assert_eq!(range, "US$ 0.02–0.50");
    }
}
