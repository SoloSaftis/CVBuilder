//! The AI chat beside the CV. The person writes what they want; the AI changes the CV through
//! CVBuilder's tools and the preview updates while it works. Every change made in one answer
//! is listed under it with «Angre», and the usual fact approval still applies at download.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use cv_ai::chat::{Backend, ChatEvent, History, ToolRequest};
use cv_core::Project;
use cv_mcp::Outcome;
use egui::{CornerRadius, Margin, RichText, Stroke};
use serde_json::Value;

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    Running,
    Ok,
    Failed,
}

pub enum Item {
    User(String),
    Assistant(String),
    Tool {
        id: String,
        name: String,
        state: ToolState,
    },
    /// What one answer changed, with the CV as it was before (for «Angre»).
    Changes {
        lines: Vec<String>,
        before: Option<Box<(Project, bool)>>,
        external: bool,
    },
    Note {
        text: String,
        error: bool,
    },
}

#[derive(Default)]
pub struct ChatState {
    pub items: Vec<Item>,
    pub input: String,
    history: History,
    /// Which backend `history` belongs to; another backend starts a new conversation.
    history_owner: String,
    events: Option<Receiver<ChatEvent>>,
    tools: Option<Receiver<ToolRequest>>,
    stop: Arc<AtomicBool>,
    pub running: bool,
    turn_lines: Vec<String>,
    turn_before: Option<(Project, bool)>,
    pub claude_code: Option<PathBuf>,
    /// The MCP config Claude Code is started with, written once at start.
    pub mcp_config: Option<PathBuf>,
    pub mcp_help_open: bool,
    pub focus_input: bool,
}

/// What a tool call looks like in the chat, while it runs and after.
fn tool_text(name: &str, state: ToolState) -> String {
    let (doing, done) = match name {
        "cv_read" => (i18n::t("Leser CV-en"), i18n::t("Leste CV-en")),
        "cv_check" => (i18n::t("Sjekker CV-en"), i18n::t("Sjekket CV-en")),
        "cv_update_profile" => (i18n::t("Endrer profilen"), i18n::t("Endret profilen")),
        "cv_update_target" => (i18n::t("Endrer stillingen"), i18n::t("Endret stillingen")),
        "cv_job_save" => (i18n::t("Skriver arbeidserfaring"), i18n::t("Skrev arbeidserfaring")),
        "cv_job_remove" => (i18n::t("Fjerner en jobb"), i18n::t("Fjernet en jobb")),
        "cv_skills_set" => (i18n::t("Oppdaterer kompetanse"), i18n::t("Oppdaterte kompetanse")),
        "cv_education_set" => (i18n::t("Oppdaterer utdanning"), i18n::t("Oppdaterte utdanning")),
        "cv_certifications_set" => (i18n::t("Oppdaterer kurs"), i18n::t("Oppdaterte kurs")),
        "cv_languages_set" => (i18n::t("Oppdaterer språk"), i18n::t("Oppdaterte språk")),
        "cv_interests_set" => (i18n::t("Oppdaterer interesser"), i18n::t("Oppdaterte interesser")),
        "cv_list_designs" => (i18n::t("Ser på designene"), i18n::t("Så på designene")),
        "cv_set_layout" => (i18n::t("Bytter design"), i18n::t("Byttet design")),
        "cv_contact_set" => (i18n::t("Endrer kontaktlinjen"), i18n::t("Endret kontaktlinjen")),
        "cv_section_order" => (i18n::t("Endrer rekkefølgen"), i18n::t("Endret rekkefølgen")),
        "cv_set_colors" => (i18n::t("Endrer fargene"), i18n::t("Endret fargene")),
        "cv_page_setup" => (i18n::t("Endrer sideoppsettet"), i18n::t("Endret sideoppsett")),
        "cv_custom_section_save" => (i18n::t("Skriver egen seksjon"), i18n::t("Skrev egen seksjon")),
        "cv_custom_section_remove" => (i18n::t("Sletter en seksjon"), i18n::t("Slettet en seksjon")),
        "cv_set_language" => (i18n::t("Bytter språk"), i18n::t("Byttet språk")),
        other => (other, other),
    };
    match state {
        ToolState::Running => format!("{doing} …"),
        ToolState::Ok => done.to_string(),
        ToolState::Failed => i18n::tf("{doing}: gikk ikke", &[("doing", doing)]),
    }
}

fn suggestions() -> [&'static str; 6] {
    [
        i18n::t("Skreddersy CV-en til stillingsannonsen"),
        i18n::t("Gjør punktene i arbeidserfaringen mer resultatorienterte"),
        i18n::t("Skriv en kort profiltekst"),
        i18n::t("Hvilke fagord fra annonsen mangler i CV-en?"),
        i18n::t("Få CV-en ned på to sider"),
        i18n::t("Oversett CV-en til engelsk"),
    ]
}

impl ChatState {
    /// Starts a new conversation (the CV is not changed).
    pub fn clear(&mut self) {
        self.items.clear();
        self.history = History::default();
    }

    pub fn new(claude_code: Option<PathBuf>) -> ChatState {
        let mcp_config = claude_code.as_ref().and_then(|_| crate::link::write_claude_code_config());
        ChatState { claude_code, mcp_config, ..Default::default() }
    }

    /// Stops a running answer (the window is closing).
    pub fn stop_now(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl CvApp {
    /// The backend the chat uses now, if any.
    pub(crate) fn chat_backend(&self) -> Option<Backend> {
        let api = || self.ai_endpoint().map(|(endpoint, model)| Backend::Api { endpoint, model });
        let claude_code = || {
            let program = self.chat.claude_code.clone()?;
            let mcp_config = self.chat.mcp_config.clone()?;
            Some(Backend::ClaudeCode { program, mcp_config, model: None })
        };
        if self.settings.chat_backend == "claude-code" { claude_code().or_else(api) } else { api().or_else(claude_code) }
    }

    pub(crate) fn chat_send(&mut self, text: &str) {
        let text = text.trim().to_string();
        if text.is_empty() || self.chat.running {
            return;
        }
        let Some(backend) = self.chat_backend() else {
            self.ai.connect_open = true;
            self.ai.choice = Some(cv_ai::Service::Claude);
            return;
        };
        let owner = backend.name();
        if owner != self.chat.history_owner {
            self.chat.history = History::default();
            self.chat.history_owner = owner;
        }
        if matches!(backend, Backend::ClaudeCode { .. }) && self.link.is_none() {
            self.chat.items.push(Item::Note { text: i18n::t("MCP-forbindelsen kunne ikke startes på denne maskinen, så Claude Code kan ikke endre CV-en.").into(), error: true });
        }
        self.chat.items.push(Item::User(text.clone()));
        let (event_tx, event_rx) = channel();
        let (tool_tx, tool_rx) = channel();
        self.chat.events = Some(event_rx);
        self.chat.tools = Some(tool_rx);
        self.chat.stop = Arc::new(AtomicBool::new(false));
        self.chat.running = true;
        self.chat.turn_lines.clear();
        self.chat.turn_before = None;
        let stop = self.chat.stop.clone();
        let history = self.chat.history.clone();
        let today = chrono_like_today();
        let example = if self.is_example { " The CV now open is CVBuilder's fictional example; say so if the person seems to think it is theirs." } else { "" };
        let system = cv_ai::chat::system_prompt(&format!("Today is {today}. The CV is open in CVBuilder's window next to this chat.{example}"));
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            // Wake the window for every event, so text and tool calls appear as they happen.
            let (relay_tx, relay_rx) = channel::<ChatEvent>();
            let ctx2 = ctx.clone();
            let forward = std::thread::spawn(move || {
                for e in relay_rx {
                    if event_tx.send(e).is_err() {
                        break;
                    }
                    ctx2.request_repaint();
                }
            });
            let (tool_relay_tx, tool_relay_rx) = channel::<ToolRequest>();
            let ctx3 = ctx.clone();
            std::thread::spawn(move || {
                for r in tool_relay_rx {
                    if tool_tx.send(r).is_err() {
                        break;
                    }
                    ctx3.request_repaint();
                }
            });
            cv_ai::chat::run_turn(&backend, history, &text, &system, &relay_tx, &tool_relay_tx, &stop);
            drop(relay_tx);
            let _ = forward.join();
        });
        self.chat.focus_input = true;
    }

    /// Runs a tool on the open CV, as the AI asked. Edits count as a change by the person.
    pub(crate) fn apply_tool(&mut self, name: &str, arguments: &Value, external: bool) -> Outcome {
        let before = (self.project.clone(), self.is_example);
        let outcome = cv_mcp::call(&mut self.project, name, arguments);
        if let Some(change) = outcome.change.clone() {
            self.buffers.clear();
            self.bad_dates.clear();
            self.touched();
            if self.chat.running {
                // Part of the answer being written (a Claude Code turn's calls arrive over MCP too).
                self.chat.turn_before.get_or_insert(before);
                self.chat.turn_lines.push(i18n::tdyn(&change));
            } else {
                // Another AI app connected over MCP changed the CV.
                self.chat.items.push(Item::Changes { lines: vec![i18n::tdyn(&change)], before: Some(Box::new(before)), external });
            }
        }
        outcome
    }

    pub(crate) fn chat_poll(&mut self) {
        // Calls over MCP (Claude Code in the chat, or another AI app).
        let link_calls: Vec<_> = self.link.as_ref().map(|l| l.calls.try_iter().collect()).unwrap_or_default();
        for call in link_calls {
            let outcome = self.apply_tool(&call.name, &call.arguments, true);
            let _ = call.reply.send(outcome);
        }
        let requests: Vec<ToolRequest> = self.chat.tools.as_ref().map(|t| t.try_iter().collect()).unwrap_or_default();
        for r in requests {
            let outcome = self.apply_tool(&r.name, &r.arguments, false);
            let _ = r.reply.send(outcome);
        }
        let Some(events) = self.chat.events.as_ref() else {
            return;
        };
        let mut finished = None;
        let mut new_items = Vec::new();
        loop {
            match events.try_recv() {
                Ok(ChatEvent::Text(t)) => new_items.push(Item::Assistant(t)),
                Ok(ChatEvent::ToolStarted { id, name }) => new_items.push(Item::Tool { id, name, state: ToolState::Running }),
                Ok(ChatEvent::ToolDone { id, ok }) => {
                    let state = if ok { ToolState::Ok } else { ToolState::Failed };
                    let mut found = false;
                    for item in self.chat.items.iter_mut().chain(new_items.iter_mut()) {
                        if let Item::Tool { id: i, state: s, .. } = item {
                            if *i == id {
                                *s = state;
                                found = true;
                            }
                        }
                    }
                    let _ = found;
                }
                Ok(ChatEvent::Done { history }) => {
                    finished = Some((history, None));
                    break;
                }
                Ok(ChatEvent::Failed { message, history }) => {
                    finished = Some((history, Some(message)));
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = Some((self.chat.history.clone(), Some(i18n::t("Samtalen stoppet uventet.").into())));
                    break;
                }
            }
        }
        self.chat.items.extend(new_items);
        if let Some((history, error)) = finished {
            self.chat.history = history;
            self.chat.running = false;
            self.chat.events = None;
            self.chat.tools = None;
            for item in &mut self.chat.items {
                if let Item::Tool { state, .. } = item {
                    if *state == ToolState::Running {
                        *state = ToolState::Failed;
                    }
                }
            }
            if self.chat.stop.load(Ordering::Relaxed) {
                self.chat.items.push(Item::Note { text: i18n::t("Stoppet.").into(), error: false });
            }
            if let Some(e) = error {
                self.chat.items.push(Item::Note { text: i18n::tdyn(&e), error: true });
            }
            if !self.chat.turn_lines.is_empty() {
                let lines = std::mem::take(&mut self.chat.turn_lines);
                let before = self.chat.turn_before.take().map(Box::new);
                self.chat.items.push(Item::Changes { lines, before, external: false });
            }
        }
    }

    pub(crate) fn chat_panel(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let backend = self.chat_backend();
        // Header: who the person is talking to.
        ui.horizontal(|ui| {
            theme::label(ui, i18n::t("Du snakker med"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::ghost(ui, "MCP", i18n::t("Bruk CVBuilder fra Claude Code, Claude Desktop eller en annen AI-app")).clicked() {
                    self.chat.mcp_help_open = true;
                }
                if !self.chat.items.is_empty() && !self.chat.running && theme::ghost(ui, i18n::t("Ny samtale"), i18n::t("Start en ny samtale. CV-en endres ikke.")).clicked() {
                    self.chat.items.clear();
                    self.chat.history = History::default();
                }
            });
        });
        self.backend_picker(ui, backend.as_ref());
        ui.add_space(4.0);
        let composer_h = ui.data(|d| d.get_temp::<f32>(egui::Id::unique("cvbuilder.composer-h"))).unwrap_or(120.0);
        let feed_h = (ui.available_height() - composer_h - 8.0).max(80.0);
        let mut send: Option<String> = None;
        let mut undo: Option<usize> = None;
        egui::ScrollArea::vertical().id_salt("chat-feed").max_height(feed_h).min_scrolled_height(feed_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.chat.items.is_empty() {
                self.greeting(ui, backend.is_some(), &mut send);
            }
            for (i, item) in self.chat.items.iter().enumerate() {
                ui.push_id(i, |ui| item_ui(ui, item, i, &mut undo));
            }
            if self.chat.running {
                let last_is_tool = matches!(self.chat.items.last(), Some(Item::Tool { state: ToolState::Running, .. }));
                if !last_is_tool {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(i18n::t("Tenker …")).color(t.faint));
                    });
                }
            }
            ui.add_space(6.0);
        });
        let top = ui.cursor().top();
        self.composer(ui, &mut send);
        let used = ui.cursor().top() - top;
        ui.data_mut(|d| d.insert_temp(egui::Id::unique("cvbuilder.composer-h"), used));
        if (used - composer_h).abs() > 0.5 {
            ui.ctx().request_discard("chat composer height changed");
        }
        if let Some(i) = undo {
            if let Some(Item::Changes { before, .. }) = self.chat.items.get_mut(i) {
                if let Some(b) = before.take() {
                    let (project, example) = *b;
                    self.load(project, example);
                    self.status = i18n::t("AI-endringene er angret.").into();
                    self.chat.items.push(Item::Note { text: i18n::t("Endringene er angret.").into(), error: false });
                }
            }
        }
        if let Some(text) = send {
            self.chat.input.clear();
            self.chat_send(&text);
        }
    }

    fn backend_picker(&mut self, ui: &mut egui::Ui, backend: Option<&Backend>) {
        let t = Tokens::get(ui.ctx());
        let api = self.ai_endpoint();
        let current = match backend {
            Some(Backend::Api { endpoint, model }) => {
                format!("{} · {model}", endpoint.service.name())
            }
            Some(Backend::ClaudeCode { .. }) => i18n::t("Claude Code · din egen innlogging").into(),
            None => i18n::t("Ingen AI tilkoblet").into(),
        };
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("chat-backend")
                .selected_text(RichText::new(current).size(13.0))
                .width((ui.available_width() - 36.0).max(120.0))
                .show_ui(ui, |ui| {
                    if let Some((endpoint, model)) = &api {
                        if ui
                            .selectable_label(matches!(backend, Some(Backend::Api { .. })), format!("{} · {model}", endpoint.service.name()))
                            .on_hover_text(i18n::t("Bruk AI-tjenesten du har koblet til med API-nøkkel"))
                            .clicked()
                        {
                            self.settings.chat_backend = "api".into();
                            crate::storage::save_settings(&self.settings);
                        }
                    }
                    if self.chat.claude_code.is_some()
                        && ui
                            .selectable_label(matches!(backend, Some(Backend::ClaudeCode { .. })), i18n::t("Claude Code · din egen innlogging, via MCP"))
                            .on_hover_text(i18n::t("Bruk Claude Code på denne maskinen, med kontoen du er logget inn med"))
                            .clicked()
                    {
                        self.settings.chat_backend = "claude-code".into();
                        crate::storage::save_settings(&self.settings);
                    }
                    ui.separator();
                    if ui.selectable_label(false, i18n::t("Koble til AI-tjeneste med API-nøkkel …")).on_hover_text(i18n::t("Velg en AI-tjeneste og legg inn API-nøkkelen din")).clicked() {
                        self.ai.connect_open = true;
                        self.ai.choice = self.settings.ai_service.or(Some(cv_ai::Service::Claude));
                    }
                })
                .response
                .on_hover_text(i18n::t("Hvilken AI du snakker med. Du kan bytte når som helst."));
        });
        let privacy = match backend {
            Some(Backend::Api { endpoint, .. }) if endpoint.service == cv_ai::Service::Local && cv_ai::is_local_address(&endpoint.base_url) => {
                i18n::tf("Kjører på denne maskinen ({url}). CV-en forlater ikke maskinen.", &[("url", &endpoint.base_url)])
            }
            Some(Backend::Api { endpoint, .. }) if endpoint.service == cv_ai::Service::Local => i18n::tf("Sendes til din AI-server på {url}.", &[("url", &endpoint.base_url)]),
            Some(Backend::Api { endpoint, .. }) => i18n::tf("Meldinger og CV-tekst sendes til {company} med din nøkkel.", &[("company", endpoint.service.company())]),
            Some(Backend::ClaudeCode { .. }) => i18n::t("Kjører i Claude Code på denne maskinen, med kontoen du er logget inn med.").into(),
            None => String::new(),
        };
        if !privacy.is_empty() {
            ui.label(RichText::new(privacy).size(11.5).color(t.faint));
        }
    }

    fn greeting(&mut self, ui: &mut egui::Ui, connected: bool, send: &mut Option<String>) {
        let t = Tokens::get(ui.ctx());
        ui.add_space(18.0);
        ui.label(RichText::new(i18n::t("Hva vil du gjøre med CV-en?")).size(18.0).family(theme::bold()).color(t.text));
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                RichText::new(i18n::t("Skriv med egne ord. AI-en endrer CV-en direkte, du ser endringene med en gang, og kan angre dem. Den finner aldri på fakta: der et tall mangler, skriver den [tall] som du fyller inn."))
                    .color(t.dim),
            )
            .wrap(),
        );
        ui.add_space(12.0);
        if !connected {
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(i18n::t("Koble til en AI først")).family(theme::medium()));
                theme::hint(ui, i18n::t("Bruk Claude, ChatGPT, Gemini eller Grok med din egen API-nøkkel. Har du Claude Code installert, kan du bruke det i stedet."));
                ui.add_space(4.0);
                if theme::primary(ui, i18n::t("Koble til AI …"), i18n::t("Velg en AI-tjeneste og legg inn API-nøkkelen din"), true).clicked() {
                    self.ai.connect_open = true;
                    self.ai.choice = Some(cv_ai::Service::Claude);
                }
            });
            return;
        }
        for s in suggestions() {
            let b = egui::Button::new(RichText::new(s).color(t.text).size(13.5))
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(theme::R_LG))
                .min_size(egui::vec2((ui.available_width() - 4.0).floor(), 34.0));
            if ui.add(b).on_hover_text(i18n::t("Send dette til AI-en. Du ser endringene og kan angre dem.")).clicked() {
                *send = Some(s.to_string());
            }
        }
    }

    fn composer(&mut self, ui: &mut egui::Ui, send: &mut Option<String>) {
        let t = Tokens::get(ui.ctx());
        let running = self.chat.running;
        egui::Frame::new().fill(t.field).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(14)).inner_margin(Margin { left: 12, right: 8, top: 10, bottom: 8 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let edit = egui::TextEdit::multiline(&mut self.chat.input)
                .id_salt("chat-input")
                .frame(egui::Frame::NONE)
                .hint_text(i18n::t("Be om en endring, f.eks. «gjør profilen kortere»"))
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .return_key(Some(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Enter)));
            let r = ui.add(edit).on_hover_text(i18n::t("Skriv med egne ord hva du vil endre. Enter sender."));
            if self.chat.focus_input {
                r.request_focus();
                self.chat.focus_input = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) && !running {
                *send = Some(self.chat.input.clone());
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::t("Enter sender · Shift+Enter gir ny linje")).size(11.5).color(t.faint));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if running {
                        let b = egui::Button::new(RichText::new(i18n::t("Stopp")).color(t.text).size(12.5)).fill(t.hover).corner_radius(CornerRadius::same(255));
                        if ui.add(b).on_hover_text(i18n::t("Stopp AI-en. Endringer den alt har gjort, blir værende og kan angres.")).clicked() {
                            self.chat.stop.store(true, Ordering::Relaxed);
                        }
                    } else {
                        let ready = !self.chat.input.trim().is_empty();
                        let b = egui::Button::new(RichText::new("Send").color(t.on_accent).size(12.5).family(theme::medium()))
                            .fill(if ready { t.accent } else { t.faint })
                            .corner_radius(CornerRadius::same(255));
                        if ui.add_enabled(ready, b).on_hover_text(i18n::t("Send meldingen (Enter)")).on_disabled_hover_text(i18n::t("Skriv en melding først")).clicked() {
                            *send = Some(self.chat.input.clone());
                        }
                    }
                });
            });
        });
    }

    pub(crate) fn mcp_help(&mut self, ctx: &egui::Context) {
        if !self.chat.mcp_help_open {
            return;
        }
        let t = Tokens::get(ctx);
        let snippet = crate::link::client_snippet();
        let mut close = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.mcp")).show(ctx, |ui| {
            ui.set_max_width(560.0);
            ui.heading(i18n::t("Bruk CVBuilder fra en annen AI-app"));
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    i18n::t("CVBuilder er en MCP-server. AI-apper som støtter MCP, som Claude Code og Claude Desktop, kan lese og endre CV-en som er åpen her, og du ser endringene med en gang. Hver endring kan angres i chatten."),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            theme::label(ui, i18n::t("Claude Code: kjør denne kommandoen i en terminal"));
            let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let command = format!("claude mcp add cvbuilder -- \"{exe}\" --mcp");
            code_box(ui, &command, &t);
            ui.add_space(6.0);
            theme::label(ui, i18n::t("Andre apper (f.eks. Claude Desktop): legg dette inn i MCP-innstillingene"));
            code_box(ui, &snippet, &t);
            ui.add_space(6.0);
            theme::hint(ui, i18n::t("CVBuilder må være åpent mens AI-appen jobber. Forbindelsen går bare på denne maskinen (127.0.0.1) og krever en nøkkel som bare CVBuilder kjenner."));
            ui.add_space(8.0);
            if ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk vinduet (Esc)")).clicked() {
                close = true;
            }
        });
        if close || r.should_close() {
            self.chat.mcp_help_open = false;
        }
    }
}

fn code_box(ui: &mut egui::Ui, text: &str, t: &Tokens) {
    egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0)).wrap().selectable(true));
        });
        if ui.small_button(i18n::t("Kopier")).on_hover_text(i18n::t("Kopier til utklippstavlen")).clicked() {
            ui.ctx().copy_text(text.to_string());
        }
    });
}

fn item_ui(ui: &mut egui::Ui, item: &Item, index: usize, undo: &mut Option<usize>) {
    let t = Tokens::get(ui.ctx());
    match item {
        Item::User(text) => {
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                egui::Frame::new().fill(t.user_bubble).corner_radius(CornerRadius::same(16)).inner_margin(Margin::symmetric(14, 9)).show(ui, |ui| {
                    ui.set_max_width(ui.available_width() * 0.85);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.add(egui::Label::new(RichText::new(text).color(t.text)).wrap());
                    });
                });
            });
            ui.add_space(4.0);
        }
        Item::Assistant(text) => {
            assistant_text(ui, text, &t);
            ui.add_space(2.0);
        }
        Item::Tool { name, state, .. } => {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                match state {
                    ToolState::Running => {
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                    ToolState::Ok => dot(ui, t.success),
                    ToolState::Failed => dot(ui, t.warning),
                }
                ui.label(RichText::new(tool_text(name, *state)).size(12.5).color(t.dim));
            });
        }
        Item::Changes { lines, before, external } => {
            ui.add_space(4.0);
            egui::Frame::new().fill(t.accent_soft).corner_radius(CornerRadius::same(theme::R_LG)).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let title = if *external { i18n::t("Endret av en AI-app over MCP") } else { i18n::t("Endringer i CV-en") };
                ui.label(RichText::new(title).family(theme::medium()).color(t.text));
                for l in lines {
                    // Indented, with the bullet in the margin, so long lines wrap to the panel.
                    let r = egui::Frame::new().inner_margin(Margin { left: 14, right: 0, top: 0, bottom: 0 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(RichText::new(l).size(13.0).color(t.text)).wrap());
                    });
                    let top = r.response.rect.left_top();
                    ui.painter().circle_filled(egui::pos2(top.x + 4.0, top.y + 9.0), 2.5, t.accent);
                }
                if before.is_some() {
                    ui.add_space(2.0);
                    if ui.button(i18n::t("Angre disse endringene")).on_hover_text(i18n::t("Sett CV-en tilbake til slik den var før disse endringene")).clicked() {
                        *undo = Some(index);
                    }
                } else {
                    ui.label(RichText::new(i18n::t("Angret")).size(12.0).color(t.faint));
                }
            });
            ui.add_space(4.0);
        }
        Item::Note { text, error } => {
            ui.add(egui::Label::new(RichText::new(text).size(12.5).color(if *error { t.danger } else { t.faint })).wrap());
        }
    }
}

fn dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
}

/// The AI's answer: paragraphs, bullet lines and **bold**, without a full Markdown renderer.
fn assistant_text(ui: &mut egui::Ui, text: &str, t: &Tokens) {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            ui.add_space(2.0);
            continue;
        }
        let (bullet, body) = match trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("• ")) {
            Some(rest) => (true, rest),
            None => (false, trimmed),
        };
        let heading = body.starts_with('#');
        let body = body.trim_start_matches('#').trim();
        let mut job = egui::text::LayoutJob::default();
        let base = egui::TextFormat { font_id: egui::FontId::new(14.0, if heading { theme::bold() } else { egui::FontFamily::Proportional }), color: t.text, ..Default::default() };
        let strong = egui::TextFormat { font_id: egui::FontId::new(14.0, theme::bold()), color: t.text, ..Default::default() };
        for (i, part) in body.split("**").enumerate() {
            let part = part.replace('`', "");
            job.append(&part, 0.0, if i % 2 == 1 { strong.clone() } else { base.clone() });
        }
        job.wrap.max_width = ui.available_width() - if bullet { 16.0 } else { 0.0 };
        ui.horizontal_top(|ui| {
            if bullet {
                ui.label(RichText::new("•").color(t.dim));
            }
            ui.add(egui::Label::new(job).wrap());
        });
    }
}

/// «2026-11» for the system prompt, without a date library.
fn chrono_like_today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02}")
}
