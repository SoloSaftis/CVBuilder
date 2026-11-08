//! The AI parts of the window: «Koble til AI» (pick a service, paste a key, test it) and the
//! review window every AI suggestion goes through. Nothing the AI writes is used until the
//! person presses «Bruk forslaget», and the usual fact approval still applies at download.
use std::sync::mpsc::{Receiver, channel};

use cv_ai::{AiFailure, Endpoint, Keys, SERVICES, Service};
use cv_core::Project;
use egui::RichText;

use crate::CvApp;

pub enum Suggestion {
    Bullets { job_id: String, text: String },
    Summary(String),
    Keywords(String),
    Translation(Box<Project>),
}

pub enum Outcome {
    Models(Service, Result<Vec<String>, AiFailure>),
    Suggestion(Result<Suggestion, AiFailure>),
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
    pub review: Option<Suggestion>,
    pub note: String,
}

impl CvApp {
    /// The connected service, its model and an endpoint with the stored key.
    pub(crate) fn ai_endpoint(&self) -> Option<(Endpoint, String)> {
        let service = self.settings.ai_service?;
        let key = self.ai.keys.get(service)?;
        Some((Endpoint::new(service, key), self.settings.ai_model.clone()))
    }

    pub(crate) fn ai_label(&self) -> String {
        match self.ai_endpoint() {
            Some((e, _)) => format!("AI: {}", e.service.name()),
            None => "Koble til AI…".into(),
        }
    }

    /// Runs one AI task on a background thread; the window stays responsive.
    pub(crate) fn ai_run(&mut self, what: &str, task: impl FnOnce(&Endpoint, &str) -> Result<Suggestion, AiFailure> + Send + 'static) {
        let Some((endpoint, model)) = self.ai_endpoint() else {
            self.ai.connect_open = true;
            self.status = "Koble til en AI-tjeneste først.".into();
            return;
        };
        if self.ai.busy.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.ai.inbox = Some(rx);
        self.ai.busy = Some(format!("{} jobber: {what} …", endpoint.service.name()));
        std::thread::spawn(move || {
            let _ = tx.send(Outcome::Suggestion(task(&endpoint, &model)));
        });
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
                        self.ai.note = format!("Tilkoblet. {} fant {} modeller; den anbefalte er valgt.", service.name(), self.ai.models.len());
                        self.settings.ai_model = self.ai.models.first().cloned().unwrap_or_default();
                        let saved = self.ai.keys.set(service, &self.ai.key_input);
                        self.ai.key_input.clear();
                        self.settings.ai_service = Some(service);
                        crate::storage::save_settings(&self.settings);
                        if !saved {
                            self.ai.note.push_str(" Denne maskinen har ingen nøkkelring, så nøkkelen huskes bare til du lukker programmet.");
                        }
                    }
                    Err(e) => self.ai.note = e.to_string(),
                }
            }
            Ok(Outcome::Suggestion(result)) => {
                self.ai.busy = None;
                self.ai.inbox = None;
                match result {
                    Ok(s) => self.ai.review = Some(s),
                    Err(e) => self.status = e.to_string(),
                }
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
            ui.heading("Koble til AI");
            ui.label("Valgfritt. AI kan foreslå bedre formuleringer, en profiltekst, fagord fra annonsen og en oversatt kopi. Du ser alltid forslaget og velger selv om det skal brukes.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for s in SERVICES {
                    if ui.selectable_label(self.ai.choice == Some(s), s.name()).clicked() {
                        self.ai.choice = Some(s);
                        self.ai.models.clear();
                        self.ai.note.clear();
                    }
                }
            });
            let Some(service) = self.ai.choice else {
                ui.label(RichText::new("Velg tjenesten du har en API-nøkkel til.").weak());
                return;
            };
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("1. Lag en API-nøkkel hos {}:", service.company()));
                ui.hyperlink_to(service.key_page(), service.key_page());
            });
            ui.label("2. Lim inn nøkkelen her:");
            ui.add(egui::TextEdit::singleline(&mut self.ai.key_input).password(true).hint_text("API-nøkkel").desired_width(f32::INFINITY));
            ui.add_space(4.0);
            ui.label(RichText::new(format!(
                "Når du bruker AI-knappene, sendes CV-teksten og stillingsannonsen til {} for å lage forslaget. {} tar betalt for bruken på din nøkkel. Nøkkelen lagres i maskinens egen nøkkelring, aldri i CV-filen.",
                service.company(),
                service.company()
            ))
            .small());
            ui.checkbox(&mut self.ai.understood, "Jeg forstår og vil koble til");
            ui.horizontal(|ui| {
                let ready = self.ai.understood && self.ai.key_input.trim().len() > 10 && self.ai.busy.is_none();
                if ui.add_enabled(ready, egui::Button::new(RichText::new("3. Test og koble til").strong())).clicked() {
                    let endpoint = Endpoint::new(service, craft_ai_core_secret(&self.ai.key_input));
                    let (tx, rx) = channel();
                    self.ai.inbox = Some(rx);
                    self.ai.busy = Some(format!("Tester nøkkelen hos {} …", service.name()));
                    std::thread::spawn(move || {
                        let _ = tx.send(Outcome::Models(service, cv_ai::test_connection(&endpoint)));
                    });
                }
                if self.settings.ai_service == Some(service) && ui.button("Koble fra og slett nøkkelen").clicked() {
                    self.ai.keys.forget(service);
                    self.settings.ai_service = None;
                    crate::storage::save_settings(&self.settings);
                    self.ai.note = format!("Koblet fra {}. Nøkkelen er slettet fra denne maskinen.", service.name());
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
                egui::ComboBox::from_label("Modell").selected_text(self.settings.ai_model.clone()).show_ui(ui, |ui| {
                    for m in &self.ai.models {
                        if ui.selectable_value(&mut self.settings.ai_model, m.clone(), m).changed() {
                            crate::storage::save_settings(&self.settings);
                        }
                    }
                });
            }
            ui.add_space(6.0);
            if ui.button("Lukk").clicked() {
                close = true;
            }
        });
        if close || response.should_close() {
            self.ai.connect_open = false;
            self.ai.key_input.clear();
        }
    }

    pub(crate) fn review_dialog(&mut self, ctx: &egui::Context) {
        let Some(suggestion) = self.ai.review.as_mut() else { return };
        let mut decision: Option<bool> = None;
        egui::Modal::new(egui::Id::unique("cvbuilder.review")).show(ctx, |ui| {
            ui.set_max_width(620.0);
            ui.heading("Forslag fra AI");
            ui.label("Les nøye. Rett det som ikke stemmer. Fyll inn eller fjern [tall] før du laster ned – CV-en kan ikke eksporteres med plassholdere.");
            ui.add_space(6.0);
            match suggestion {
                Suggestion::Bullets { text, .. } => {
                    ui.label("Nye punkter (ett per linje):");
                    ui.add(egui::TextEdit::multiline(text).desired_rows(6).desired_width(f32::INFINITY));
                }
                Suggestion::Summary(text) => {
                    ui.label("Ny profiltekst:");
                    ui.add(egui::TextEdit::multiline(text).desired_rows(5).desired_width(f32::INFINITY));
                }
                Suggestion::Keywords(text) => {
                    ui.label("Fagord fra annonsen (ett per linje). Fjern det som ikke passer:");
                    ui.add(egui::TextEdit::multiline(text).desired_rows(8).desired_width(f32::INFINITY));
                }
                Suggestion::Translation(p) => {
                    ui.label(format!("En oversatt kopi er klar ({}). Den erstatter teksten i vinduet; bruk Angre eller «Lagre som fil» for å beholde begge versjonene.", if p.is_english() { "engelsk" } else { "bokmål" }));
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        ui.label(RichText::new(&p.candidate.headline).strong());
                        ui.label(&p.candidate.summary);
                        for j in &p.experience {
                            ui.label(RichText::new(&j.title).strong());
                            for b in &j.bullets {
                                ui.label(format!("• {b}"));
                            }
                        }
                    });
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Bruk forslaget").strong()).clicked() {
                    decision = Some(true);
                }
                if ui.button("Avvis").clicked() {
                    decision = Some(false);
                }
            });
        });
        let Some(apply) = decision else { return };
        let suggestion = self.ai.review.take();
        if !apply {
            self.status = "Forslaget er avvist. Ingenting er endret.".into();
            return;
        }
        let before = (self.project.clone(), self.is_example);
        let lines = |t: &str| t.lines().map(str::trim).map(|l| l.trim_start_matches(['-', '•']).trim()).filter(|l| !l.is_empty()).map(String::from).collect::<Vec<_>>();
        match suggestion {
            Some(Suggestion::Bullets { job_id, text }) => {
                if let Some(job) = self.project.experience.iter_mut().find(|j| j.id == job_id) {
                    job.bullets = lines(&text);
                    self.buffers.remove(&format!("{job_id}.bullets"));
                }
            }
            Some(Suggestion::Summary(text)) => self.project.candidate.summary = text.trim().to_string(),
            Some(Suggestion::Keywords(text)) => {
                self.project.target.keywords = lines(&text);
                self.buffers.remove("keywords");
            }
            Some(Suggestion::Translation(p)) => {
                self.project = *p;
                self.buffers.clear();
            }
            None => return,
        }
        self.undo = Some(before);
        self.touched();
        self.status = "Forslaget er brukt. Angre hvis du ombestemmer deg.".into();
    }
}

fn craft_ai_core_secret(key: &str) -> cv_ai::providers::Secret {
    cv_ai::providers::Secret::new(key.trim())
}
