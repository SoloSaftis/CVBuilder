//! «Koble til AI»: pick a service, paste a key, test it. The key is kept in the computer's
//! keychain. What the AI may do with the CV is in the chat (`chat_ui.rs`).
use std::sync::mpsc::{Receiver, channel};

use cv_ai::{AiFailure, Endpoint, Keys, SERVICES, Service};
use egui::RichText;

use crate::CvApp;
use crate::i18n;

pub enum Outcome {
    Models(Service, Result<Vec<String>, AiFailure>),
    /// Local servers that answered: (name, address, model count).
    Detected(Vec<(String, String, usize)>),
}

#[derive(Default)]
pub struct AiState {
    pub keys: Keys,
    pub connect_open: bool,
    pub choice: Option<Service>,
    pub key_input: String,
    pub models: Vec<String>,
    pub understood: bool,
    pub busy: Option<String>,
    pub inbox: Option<Receiver<Outcome>>,
    pub note: String,
    /// The address typed for a local server.
    pub local_url: String,
    pub detected: Option<Vec<(String, String, usize)>>,
}

impl CvApp {
    /// The connected service, its model and an endpoint with the stored key.
    pub(crate) fn ai_endpoint(&self) -> Option<(Endpoint, String)> {
        let service = self.settings.ai_service?;
        if service == Service::Local {
            // A local server: the saved address, and a key only if one was given.
            let key = self.ai.keys.get(service).unwrap_or_else(|| cv_ai::providers::Secret::new(""));
            let base_url = if self.settings.local_url.is_empty() { service.base_url().to_string() } else { self.settings.local_url.clone() };
            return Some((Endpoint { service, base_url, key }, self.settings.ai_model.clone()));
        }
        let key = self.ai.keys.get(service)?;
        Some((Endpoint::new(service, key), self.settings.ai_model.clone()))
    }

    pub(crate) fn ai_poll(&mut self, ctx: &egui::Context) {
        let Some(inbox) = &self.ai.inbox else { return };
        match inbox.try_recv() {
            Ok(Outcome::Models(service, result)) => {
                self.ai.busy = None;
                self.ai.inbox = None;
                match result {
                    Ok(models) => {
                        self.ai.models = models;
                        self.ai.note = i18n::tf("Tilkoblet. {service} fant {n} modeller; den anbefalte er valgt.", &[("service", service.name()), ("n", &self.ai.models.len().to_string())]);
                        self.settings.ai_model = self.ai.models.first().cloned().unwrap_or_default();
                        if service == Service::Local {
                            self.settings.local_url = self.ai.local_url.trim().trim_end_matches('/').to_string();
                            self.settings.chat_backend = "api".into();
                            self.ai.note.push(' ');
                            self.ai.note.push_str(i18n::t("Små modeller kan streve med å bruke verktøy; modeller som qwen3 eller llama3.1 fungerer best."));
                        }
                        let saved = if service.needs_key() || !self.ai.key_input.trim().is_empty() { self.ai.keys.set(service, &self.ai.key_input) } else { true };
                        self.ai.key_input.clear();
                        self.settings.ai_service = Some(service);
                        crate::storage::save_settings(&self.settings);
                        if !saved {
                            self.ai.note.push(' ');
                            self.ai.note.push_str(i18n::t("Denne maskinen har ingen nøkkelring, så nøkkelen huskes bare til du lukker programmet."));
                        }
                    }
                    Err(e) => self.ai.note = i18n::tdyn(&e.to_string()),
                }
            }
            Ok(Outcome::Detected(found)) => {
                self.ai.busy = None;
                self.ai.inbox = None;
                if let Some((_, url, _)) = found.first() {
                    self.ai.local_url = url.clone();
                }
                self.ai.note =
                    if found.is_empty() { i18n::t("Fant ingen lokal AI-server på de vanlige adressene. Start Ollama eller LM Studio, eller skriv inn adressen selv.").into() } else { String::new() };
                self.ai.detected = Some(found);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(200)),
            Err(_) => {
                self.ai.busy = None;
                self.ai.inbox = None;
            }
        }
    }

    pub(crate) fn connect_dialog(&mut self, ctx: &egui::Context) {
        if !self.ai.connect_open {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("cvbuilder.ai")).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.heading(i18n::t("Koble til AI"));
            ui.add(egui::Label::new(i18n::t("Valgfritt. Med AI kan du be om endringer i chatten, og se CV-en endre seg mens AI-en jobber. Alt kan angres.")).wrap());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for s in SERVICES {
                    let tip = if s == Service::Local { i18n::t("En modell på din egen maskin, f.eks. med Ollama. Ingen nøkkel og ingen kostnad.").to_string() } else { i18n::tf("Bruk {service} med din egen API-nøkkel fra {company}", &[("service", s.name()), ("company", s.company())]) };
                    if ui.selectable_label(self.ai.choice == Some(s), s.name()).on_hover_text(tip).clicked() {
                        self.ai.choice = Some(s);
                        self.ai.models.clear();
                        self.ai.note.clear();
                    }
                }
            });
            let Some(service) = self.ai.choice else {
                ui.label(RichText::new(i18n::t("Velg tjenesten du har en API-nøkkel til.")).weak());
                return;
            };
            ui.add_space(4.0);
            if service == Service::Local {
                self.local_steps(ui);
                return;
            }
            // The link on its own line: wrapped after the text it was split in two and hard to hit.
            ui.label(i18n::tf("1. Lag en API-nøkkel hos {company}:", &[("company", service.company())]));
            ui.indent("key-link", |ui| {
                ui.hyperlink_to(service.key_page(), service.key_page()).on_hover_text(i18n::t("Åpner siden i nettleseren, der du lager nøkkelen"));
            });
            ui.label(i18n::t("2. Lim inn nøkkelen her:"));
            ui.add(egui::TextEdit::singleline(&mut self.ai.key_input).password(true).hint_text(i18n::t("API-nøkkel")).desired_width(f32::INFINITY)).on_hover_text(i18n::t("Lim inn nøkkelen. Den lagres i maskinens nøkkelring, aldri i CV-filen."));
            ui.add_space(4.0);
            ui.label(RichText::new(i18n::tf(
                "Når du bruker AI-chatten, sendes meldingene dine, CV-teksten og stillingsannonsen til {company}. {company} tar betalt for bruken på din nøkkel. Nøkkelen lagres i maskinens egen nøkkelring, aldri i CV-filen.",
                &[("company", service.company())],
            ))
            .small());
            ui.checkbox(&mut self.ai.understood, i18n::t("Jeg forstår og vil koble til")).on_hover_text(i18n::t("Bekreft at du har lest hva som sendes, og hvem som tar betalt"));
            ui.horizontal(|ui| {
                let ready = self.ai.understood && self.ai.key_input.trim().len() > 10 && self.ai.busy.is_none();
                if ui.add_enabled(ready, egui::Button::new(RichText::new(i18n::t("3. Test og koble til")).strong())).on_hover_text(i18n::t("Prøver nøkkelen og henter modellene du kan velge")).on_disabled_hover_text(i18n::t("Lim inn nøkkelen og kryss av først")).clicked() {
                    let endpoint = Endpoint::new(service, craft_ai_core_secret(&self.ai.key_input));
                    let (tx, rx) = channel();
                    self.ai.inbox = Some(rx);
                    self.ai.busy = Some(i18n::tf("Tester nøkkelen hos {service} …", &[("service", service.name())]));
                    std::thread::spawn(move || {
                        let _ = tx.send(Outcome::Models(service, cv_ai::test_connection(&endpoint)));
                    });
                }
                if self.settings.ai_service == Some(service) && ui.button(i18n::t("Koble fra og slett nøkkelen")).on_hover_text(i18n::t("Slå av AI og slett nøkkelen fra denne maskinen")).clicked() {
                    self.ai.keys.forget(service);
                    self.settings.ai_service = None;
                    crate::storage::save_settings(&self.settings);
                    self.ai.note = i18n::tf("Koblet fra {service}. Nøkkelen er slettet fra denne maskinen.", &[("service", service.name())]);
                }
            });
            if let Some(busy) = &self.ai.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(busy);
                });
            }
            if !self.ai.note.is_empty() {
                ui.label(&self.ai.note);
            }
            if self.settings.ai_service == Some(service) && !self.ai.models.is_empty() {
                egui::ComboBox::from_label(i18n::t("Modell")).selected_text(self.settings.ai_model.clone()).show_ui(ui, |ui| {
                    for m in &self.ai.models {
                        if ui.selectable_value(&mut self.settings.ai_model, m.clone(), m).on_hover_text(i18n::t("Bruk denne modellen i chatten")).changed() {
                            crate::storage::save_settings(&self.settings);
                        }
                    }
                })
                .response
                .on_hover_text(i18n::t("Modellen chatten bruker. Større modeller svarer ofte bedre, men tregere."));
            }
            ui.add_space(6.0);
            if ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk vinduet (Esc)")).clicked() {
                close = true;
            }
        });
        if close || response.should_close() {
            self.ai.connect_open = false;
            self.ai.key_input.clear();
        }
    }

    /// Connecting a model on this computer (or the network) through an OpenAI-compatible server.
    fn local_steps(&mut self, ui: &mut egui::Ui) {
        let service = Service::Local;
        if self.ai.local_url.is_empty() {
            self.ai.local_url = if self.settings.local_url.is_empty() { service.base_url().to_string() } else { self.settings.local_url.clone() };
        }
        ui.add(egui::Label::new(i18n::t("Bruk en AI-modell som kjører på din egen maskin, for eksempel med Ollama eller LM Studio. Ingen nøkkel og ingen kostnad, og CV-en forlater ikke maskinen. Modellen endrer CV-en med de samme verktøyene (MCP-verktøyene til CVBuilder) som de andre tjenestene.")).wrap());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(i18n::t("Har du ikke en lokal server?"));
            ui.hyperlink_to(i18n::t("Last ned Ollama"), "https://ollama.com/download").on_hover_text(i18n::t("Åpner nedlastingssiden til Ollama i nettleseren"));
            ui.label(i18n::t("og kjør for eksempel «ollama pull qwen3»."));
        });
        ui.add_space(6.0);
        ui.label(i18n::t("1. Adressen til serveren (OpenAI-kompatibel, slutter vanligvis på /v1):"));
        ui.add(egui::TextEdit::singleline(&mut self.ai.local_url).hint_text("http://localhost:11434/v1").desired_width(f32::INFINITY))
            .on_hover_text(i18n::t("Adressen til den lokale serveren. Ollama bruker http://localhost:11434/v1."));
        ui.horizontal_wrapped(|ui| {
            for (name, url) in cv_ai::LOCAL_SERVERS {
                if ui.small_button(name).on_hover_text(i18n::tf("Bruk standardadressen til {name}: {url}", &[("name", name), ("url", url)])).clicked() {
                    self.ai.local_url = url.to_string();
                }
            }
            if ui.small_button(i18n::t("Finn lokale servere")).on_hover_text(i18n::t("Ser etter Ollama, LM Studio og andre på de vanlige adressene")).clicked() && self.ai.busy.is_none() {
                let (tx, rx) = channel();
                self.ai.inbox = Some(rx);
                self.ai.busy = Some(i18n::t("Ser etter lokale AI-servere …").into());
                std::thread::spawn(move || {
                    let _ = tx.send(Outcome::Detected(cv_ai::detect_local()));
                });
            }
        });
        if let Some(found) = &self.ai.detected {
            for (name, url, count) in found {
                ui.label(RichText::new(i18n::tf("Fant {name} på {url} ({count} modeller)", &[("name", name), ("url", url), ("count", &count.to_string())])).small());
            }
        }
        ui.add_space(4.0);
        ui.label(i18n::t("2. Nøkkel (bare hvis serveren din krever det):"));
        ui.add(egui::TextEdit::singleline(&mut self.ai.key_input).password(true).hint_text(i18n::t("Valgfri")).desired_width(f32::INFINITY))
            .on_hover_text(i18n::t("Bare hvis serveren krever nøkkel. Ollama og LM Studio gjør det ikke."));
        let local = cv_ai::is_local_address(&self.ai.local_url);
        ui.label(
            RichText::new(if local {
                i18n::t("Adressen er på denne maskinen: meldingene og CV-teksten blir her.")
            } else {
                i18n::t("Merk: adressen er ikke på denne maskinen. Meldingene og CV-teksten sendes dit.")
            })
            .small(),
        );
        ui.horizontal(|ui| {
            let ready = self.ai.local_url.starts_with("http") && self.ai.busy.is_none();
            if ui
                .add_enabled(ready, egui::Button::new(RichText::new(i18n::t("3. Test og koble til")).strong()))
                .on_hover_text(i18n::t("Kobler til serveren og henter modellene den har"))
                .on_disabled_hover_text(i18n::t("Skriv en adresse som begynner med http"))
                .clicked()
            {
                let endpoint = Endpoint { service, base_url: self.ai.local_url.trim().trim_end_matches('/').to_string(), key: craft_ai_core_secret(&self.ai.key_input) };
                let (tx, rx) = channel();
                self.ai.inbox = Some(rx);
                self.ai.busy = Some(i18n::t("Kobler til den lokale serveren …").into());
                std::thread::spawn(move || {
                    let _ = tx.send(Outcome::Models(service, cv_ai::test_connection(&endpoint)));
                });
            }
            if self.settings.ai_service == Some(service) && ui.button(i18n::t("Koble fra")).on_hover_text(i18n::t("Slå av den lokale modellen i CVBuilder")).clicked() {
                self.ai.keys.forget(service);
                self.settings.ai_service = None;
                crate::storage::save_settings(&self.settings);
                self.ai.note = i18n::t("Koblet fra den lokale modellen.").into();
            }
        });
        if let Some(busy) = &self.ai.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(busy);
            });
        }
        if !self.ai.note.is_empty() {
            ui.add(egui::Label::new(&self.ai.note).wrap());
        }
        if self.settings.ai_service == Some(service) && !self.ai.models.is_empty() {
            egui::ComboBox::from_label(i18n::t("Modell"))
                .selected_text(self.settings.ai_model.clone())
                .show_ui(ui, |ui| {
                    for m in &self.ai.models {
                        if ui.selectable_value(&mut self.settings.ai_model, m.clone(), m).on_hover_text(i18n::t("Bruk denne modellen i chatten")).changed() {
                            crate::storage::save_settings(&self.settings);
                        }
                    }
                })
                .response
                .on_hover_text(i18n::t("Modellen chatten bruker. Større modeller svarer ofte bedre, men tregere."));
        }
    }
}

fn craft_ai_core_secret(key: &str) -> cv_ai::providers::Secret {
    cv_ai::providers::Secret::new(key.trim())
}
