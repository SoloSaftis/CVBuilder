//! «Legg til språk»: interface languages beyond Norwegian and English. The connected AI
//! translates every interface text into the chosen language and saves it as a language pack
//! (`<code>.cvlang.json` in the data folder's `languages`), which can then be chosen like Norsk
//! and English, made again, edited, exported and imported. Without AI, a template with the
//! English text can be exported, translated by hand and imported.
//!
//! The translation runs on its own thread in batches, saving the pack after each, so a stop
//! keeps what is done. The pack format and lookups are in [`crate::i18n`].
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use cv_ai::chat::Backend;
use egui::{Margin, RichText};
use serde_json::{Map, Value};

use crate::i18n::{self, Pack, PackError};
use crate::theme::{self, Tokens};
use crate::{CvApp, storage};

/// Languages offered when adding one, as (own name, code). All written in scripts the app's
/// fonts can show (Latin, Greek, Cyrillic).
const SUGGESTIONS: &[(&str, &str)] = &[
    ("Bahasa Indonesia", "id"),
    ("Bosanski", "bs"),
    ("Čeština", "cs"),
    ("Dansk", "da"),
    ("Davvisámegiella", "se"),
    ("Deutsch", "de"),
    ("Eesti", "et"),
    ("Español", "es"),
    ("Français", "fr"),
    ("Hrvatski", "hr"),
    ("Íslenska", "is"),
    ("Italiano", "it"),
    ("Kiswahili", "sw"),
    ("Kurdî", "ku"),
    ("Latviešu", "lv"),
    ("Lietuvių", "lt"),
    ("Magyar", "hu"),
    ("Nederlands", "nl"),
    ("Norsk nynorsk", "nn"),
    ("Polski", "pl"),
    ("Português", "pt"),
    ("Português (Brasil)", "pt-BR"),
    ("Română", "ro"),
    ("Shqip", "sq"),
    ("Slovenčina", "sk"),
    ("Slovenščina", "sl"),
    ("Soomaali", "so"),
    ("Srpski", "sr"),
    ("Suomi", "fi"),
    ("Svenska", "sv"),
    ("Tagalog", "tl"),
    ("Tiếng Việt", "vi"),
    ("Türkçe", "tr"),
    ("Ελληνικά", "el"),
    ("Български", "bg"),
    ("Русский", "ru"),
    ("Українська", "uk"),
];

/// At most this many strings in one request …
pub(crate) const BATCH_SIZE: usize = 60;
/// … and at most about this much English text.
const BATCH_CHARS: usize = 6000;

pub(crate) struct LangState {
    /// The installed packs, sorted by name.
    pub packs: Vec<Pack>,
    adding: Option<Adding>,
    job: Option<Job>,
    message: Option<String>,
    confirm_delete: bool,
    editor: Option<Editor>,
    picks_tx: Sender<LangPick>,
    picks_rx: Receiver<LangPick>,
}

#[derive(Default)]
struct Adding {
    name: String,
    code: String,
}

struct Job {
    code: String,
    name: String,
    done: usize,
    total: usize,
    rx: Receiver<Progress>,
    stop: Arc<AtomicBool>,
}

enum Progress {
    Saved { done: usize, total: usize },
    Finished { pack: Pack, report: Report, save_error: Option<String> },
    Failed(String),
}

/// How a translation went.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Report {
    pub translated: usize,
    /// Strings the AI did not translate usably, also after one retry.
    pub failed: usize,
    pub stopped: bool,
}

enum LangPick {
    Import(PathBuf),
    Template(PathBuf),
    Export(String, PathBuf),
}

struct Editor {
    code: String,
    /// (key, English, translation) for every text.
    rows: Vec<(&'static str, &'static str, String)>,
    filter: String,
}

impl LangState {
    /// Reads the installed packs and sets the interface language from the saved setting.
    pub fn load(setting: &str) -> LangState {
        let (picks_tx, picks_rx) = channel();
        let state = LangState { packs: load_packs(), adding: None, job: None, message: None, confirm_delete: false, editor: None, picks_tx, picks_rx };
        i18n::apply(setting, &state.packs);
        state
    }
}

include!(concat!(env!("OUT_DIR"), "/bundled_languages.rs"));

/// The language ships with CVBuilder (it comes back if the person's own copy is deleted).
pub fn is_bundled(code: &str) -> bool {
    BUNDLED_LANGUAGES.iter().any(|(c, _)| *c == code)
}

/// The packs: the person's own files first (an edited or newer copy wins), then the bundled ones.
fn load_packs() -> Vec<Pack> {
    let mut packs: Vec<Pack> = Vec::new();
    for text in storage::language_files().iter().map(String::as_str).chain(BUNDLED_LANGUAGES.iter().map(|(_, text)| *text)) {
        if let Ok(checked) = i18n::parse_pack(text) {
            if !packs.iter().any(|p| p.code == checked.pack.code) {
                packs.push(checked.pack);
            }
        }
    }
    packs.sort_by_key(|p| p.name.to_lowercase());
    packs
}

fn pack_error(e: &PackError) -> String {
    match e {
        PackError::NotJson(e) => i18n::tf("Filen er ikke gyldig JSON: {e}", &[("e", e)]),
        PackError::Format => i18n::tf("Filen er ikke en CVBuilder-språkfil (feltet «format» må være «{format}»).", &[("format", i18n::PACK_FORMAT)]),
        PackError::NoName => i18n::t("Fyll inn språkets navn i feltet «name».").to_string(),
        PackError::BadCode(c) if c.is_empty() => i18n::t("Fyll inn språkkoden i feltet «code», for eksempel de eller pt-BR.").to_string(),
        PackError::BadCode(c) => i18n::tf("«{code}» er ikke en gyldig språkkode. Bruk 2–8 små bokstaver, gjerne med region, for eksempel de eller pt-BR.", &[("code", c)]),
        PackError::Reserved(c) => i18n::tf("Språkkoden «{code}» brukes av de innebygde valgene. Velg en annen.", &[("code", c)]),
    }
}

// ---------- the translation itself, without any UI (tested with a fake AI) ----------

/// The instruction for one batch.
pub(crate) fn system_prompt(language: &str) -> String {
    format!(
        "You translate the user interface strings of CVBuilder, a CV/resume app, from English into {language}.\n\
         The user message is a JSON object of numbered strings. Reply with a JSON object only: the same keys, each with its translation.\n\
         Rules:\n\
         - Keep every {{placeholder}} in curly braces exactly as it is, untranslated.\n\
         - Keep «» quotation marks, keyboard key names (Enter, Esc, Ctrl, Tab …), file extensions and product names unchanged: CVBuilder, ATS, MCP, PDF, Word, LinkedIn, Claude, Claude Code, ChatGPT, Gemini, Grok, Ollama, LM Studio, Reactive Resume, RenderCV.\n\
         - Use short, natural wording as in a well-made app in {language}; buttons stay short. Keep the tone friendly and plain.\n\
         - Keep ellipses (…), line breaks and the punctuation style of each string.\n\
         - Do not add explanations, notes or Markdown."
    )
}

/// Splits the strings into batches: at most [`BATCH_SIZE`] strings and about [`BATCH_CHARS`]
/// characters of English each (one long string still gets a batch of its own).
pub(crate) fn batches(items: &[(&str, &str)]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = 0;
    for (i, (_, en)) in items.iter().enumerate() {
        let len = en.chars().count();
        if i > start && (i - start >= BATCH_SIZE || chars + len > BATCH_CHARS) {
            out.push(start..i);
            start = i;
            chars = 0;
        }
        chars += len;
    }
    if start < items.len() {
        out.push(start..items.len());
    }
    out
}

/// One batch as the user message: `{"1": "…", "2": "…"}` in order.
pub(crate) fn request_text(texts: &[&str]) -> String {
    let lines: Vec<String> = texts.iter().enumerate().map(|(i, s)| format!("  \"{}\": {}", i + 1, serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()))).collect();
    format!("{{\n{}\n}}", lines.join(",\n"))
}

/// The first JSON object in a reply, also when it is wrapped in prose or ```json fences.
pub(crate) fn extract_json(reply: &str) -> Option<Map<String, Value>> {
    for (at, _) in reply.match_indices('{') {
        let mut values = serde_json::Deserializer::from_str(&reply[at..]).into_iter::<Value>();
        if let Some(Ok(Value::Object(map))) = values.next() {
            return Some(map);
        }
    }
    None
}

/// Translates `items` (key, English) into `language` with `translate(system, user)`, batch by
/// batch. A batch that fails (or the strings in it that come back unusable) is tried once more;
/// what still fails is counted and left out. `on_batch` gets each batch's translations with the
/// number of strings done so far and in all. Stops between batches when `stop` is set.
/// An error only when the very first batch gets no answer at all.
pub(crate) fn translate_all(
    items: &[(&'static str, &'static str)],
    language: &str,
    translate: &mut dyn FnMut(&str, &str) -> Result<String, String>,
    stop: &AtomicBool,
    on_batch: &mut dyn FnMut(&[(&'static str, String)], usize, usize),
) -> Result<Report, String> {
    let system = system_prompt(language);
    let mut report = Report::default();
    let total = items.len();
    let mut done = 0;
    for range in batches(items) {
        if stop.load(Ordering::Relaxed) {
            report.stopped = true;
            break;
        }
        let mut todo: Vec<usize> = range.clone().collect();
        let mut got: Vec<(&'static str, String)> = Vec::new();
        let mut error = None;
        let mut answered = false;
        for _attempt in 0..2 {
            if todo.is_empty() {
                break;
            }
            let texts: Vec<&str> = todo.iter().map(|&i| items[i].1).collect();
            let map = match translate(&system, &request_text(&texts)) {
                Ok(reply) => match extract_json(&reply) {
                    Some(map) => map,
                    None => {
                        error = Some(i18n::t("AI-en svarte ikke med JSON.").to_string());
                        continue;
                    }
                },
                Err(e) => {
                    error = Some(e);
                    continue;
                }
            };
            answered = true;
            let mut rest = Vec::new();
            for (n, &i) in todo.iter().enumerate() {
                match map.get(&(n + 1).to_string()).and_then(Value::as_str) {
                    Some(text) if i18n::valid_translation(items[i].0, text) => got.push((items[i].0, text.to_string())),
                    _ => rest.push(i),
                }
            }
            todo = rest;
        }
        if !answered && done == 0 {
            return Err(error.unwrap_or_default());
        }
        report.translated += got.len();
        report.failed += todo.len();
        done += range.len();
        on_batch(&got, done, total);
    }
    Ok(report)
}

/// One request to the chat's backend.
fn ask(backend: &Backend, system: &str, user: &str) -> Result<String, String> {
    match backend {
        Backend::Api { endpoint, model } => cv_ai::providers::complete(endpoint, model, system, user).map_err(|e| i18n::tdyn(&e.to_string())),
        Backend::ClaudeCode { program, .. } => cv_ai::chat::claude_code_once(program, system, user).map_err(|e| i18n::tdyn(&e)),
    }
}

// ---------- the window ----------

impl CvApp {
    /// The «Språk i menyene» part of the settings window.
    pub(crate) fn language_settings(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let mut choose: Option<String> = None;
        let mut add: Option<&'static str> = None;
        let busy = self.langs.job.is_some();
        let fixed = [
            ("auto", i18n::t("Følg systemet"), i18n::t("Samme språk som datamaskinen: norsk hvis den er på norsk, et tilleggsspråk hvis du har lagt til datamaskinens språk, ellers engelsk")),
            ("no", "Norsk (bokmål)", i18n::t("Menyer og knapper på norsk bokmål")),
            ("en", "English", i18n::t("Menyer og knapper på engelsk")),
        ];
        let current = fixed
            .iter()
            .find(|(v, _, _)| *v == self.settings.ui_language)
            .map(|(_, n, _)| n.to_string())
            .or_else(|| self.langs.packs.iter().find(|p| p.code == self.settings.ui_language).map(|p| p.name.clone()))
            .unwrap_or_else(|| i18n::t("Følg systemet").to_string());
        egui::ComboBox::from_id_salt("ui-language")
            .selected_text(current)
            .width(240.0)
            .height(460.0)
            .show_ui(ui, |ui| {
                for (value, name, tip) in fixed {
                    if ui.selectable_label(self.settings.ui_language == value, name).on_hover_text(tip).clicked() {
                        choose = Some(value.to_string());
                    }
                }
                ui.separator();
                for p in &self.langs.packs {
                    let tip = i18n::tf("Menyer og knapper på {name} ({code}). Laget av {who}.", &[("name", &p.name), ("code", &p.code), ("who", if p.made_by.is_empty() { "?" } else { &p.made_by })]);
                    if ui.selectable_label(self.settings.ui_language == p.code, p.name.as_str()).on_hover_text(tip).clicked() {
                        choose = Some(p.code.clone());
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(!busy, egui::Button::new(i18n::t("Legg til språk med AI …")).frame(false))
                    .on_hover_text(i18n::t("Velg eller skriv et språk, så oversetter AI-en alle tekstene i CVBuilder til det"))
                    .on_disabled_hover_text(i18n::t("Vent til oversettelsen er ferdig"))
                    .clicked()
                {
                    add = Some("ai");
                }
                if ui.add(egui::Button::new(i18n::t("Importer språkfil …")).frame(false)).on_hover_text(i18n::t("Legg til et språk fra en .cvlang.json-fil")).clicked() {
                    add = Some("file");
                }
            })
            .response
            .on_hover_text(i18n::t("Språket i menyene og knappene. Mangler språket ditt, kan du legge det til med AI eller fra en språkfil."));
        match add {
            Some("ai") => self.langs.adding = Some(Adding::default()),
            Some(_) => self.pick_language_file("import", None),
            None => {}
        }
        if let Some(value) = choose.filter(|v| *v != self.settings.ui_language) {
            self.settings.ui_language = value;
            i18n::apply(&self.settings.ui_language, &self.langs.packs);
            storage::save_settings(&self.settings);
            self.langs.confirm_delete = false;
        }
        let active = i18n::active_pack();
        if active.is_some() {
            theme::hint(ui, i18n::t("Meldinger med tall og navn i, for eksempel fra sjekklisten, vises på engelsk i tilleggsspråk."));
        }

        if let Some(job) = &self.langs.job {
            let share = if job.total == 0 { 0.0 } else { job.done as f32 / job.total as f32 };
            let text = i18n::tf("Oversetter {done} av {total} …", &[("done", &job.done.to_string()), ("total", &job.total.to_string())]);
            ui.add(egui::ProgressBar::new(share).text(text).desired_width(ui.available_width()));
            let stopping = job.stop.load(Ordering::Relaxed);
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::tf("Språk: {name}", &[("name", &job.name)])).color(t.dim));
                if stopping {
                    ui.label(RichText::new(i18n::t("Stopper etter denne delen …")).color(t.dim));
                } else if ui.button(i18n::t("Stopp")).on_hover_text(i18n::t("Stopp oversettelsen. Det som er oversatt, beholdes.")).clicked() {
                    job.stop.store(true, Ordering::Relaxed);
                }
            });
        }

        let busy = self.langs.job.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new(i18n::t("Legg til språk med AI …")))
                .on_hover_text(i18n::t("Velg eller skriv et språk, så oversetter AI-en alle tekstene i CVBuilder til det"))
                .on_disabled_hover_text(i18n::t("Vent til oversettelsen er ferdig"))
                .clicked()
            {
                self.langs.adding = Some(Adding::default());
            }
            let has = active.is_some();
            if ui
                .add_enabled(has && !busy, egui::Button::new(i18n::t("Lag på nytt")))
                .on_hover_text(i18n::t("La AI-en oversette det valgte språket på nytt. Det gamle beholdes til hver del er ferdig."))
                .on_disabled_hover_text(i18n::t("Velg et tilleggsspråk først, og vent til en oversettelse er ferdig"))
                .clicked()
            {
                if let Some((code, name)) = &active {
                    self.start_translation(code.clone(), name.clone());
                }
            }
            if ui
                .add_enabled(has && !busy, egui::Button::new(i18n::t("Rediger …")))
                .on_hover_text(i18n::t("Rett tekstene i det valgte språket selv"))
                .on_disabled_hover_text(i18n::t("Velg et tilleggsspråk først, og vent til en oversettelse er ferdig"))
                .clicked()
            {
                if let Some((code, _)) = &active {
                    self.open_editor(code);
                }
            }
            if ui
                .add_enabled(has, egui::Button::new(i18n::t("Eksporter språket …")))
                .on_hover_text(i18n::t("Lagre det valgte språket som fil, for å dele det eller rette det i et annet program"))
                .on_disabled_hover_text(i18n::t("Velg et tilleggsspråk først"))
                .clicked()
            {
                if let Some((code, _)) = &active {
                    self.pick_language_file("export", Some(code.clone()));
                }
            }
            if ui.button(i18n::t("Eksporter mal for oversettelse …")).on_hover_text(i18n::t("Lagre en fil med alle tekstene på engelsk, som du kan oversette selv og importere")).clicked() {
                self.pick_language_file("template", None);
            }
            if ui.button(i18n::t("Importer språkfil …")).on_hover_text(i18n::t("Legg til et språk fra en .cvlang.json-fil")).clicked() {
                self.pick_language_file("import", None);
            }
            // A bundled language can't be deleted; a copy the person changed can (back to as shipped).
            let own = active.as_ref().is_some_and(|(code, _)| storage::has_language(code));
            let (label, tip) = if active.as_ref().is_some_and(|(code, _)| is_bundled(code)) {
                (i18n::t("Tilbakestill språk"), i18n::t("Fjern endringene dine, så språket blir slik det fulgte med CVBuilder"))
            } else {
                (i18n::t("Slett språk"), i18n::t("Fjern det valgte språket fra denne maskinen"))
            };
            if ui
                .add_enabled(has && own && !busy, egui::Button::new(label))
                .on_hover_text(tip)
                .on_disabled_hover_text(i18n::t("Språk som følger med CVBuilder kan ikke slettes; bare endringer du har gjort"))
                .clicked()
            {
                self.langs.confirm_delete = true;
            }
        });
        if let (true, Some((code, name))) = (self.langs.confirm_delete, &active) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(i18n::tf("Slette «{name}» fra denne maskinen? Det kan ikke angres, men du kan eksportere det først.", &[("name", name)])).color(t.danger));
                if ui.button(i18n::t("Slett")).on_hover_text(i18n::t("Slett språkfilen. Menyene går tilbake til «Følg systemet».")).clicked() {
                    self.delete_language(code);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre noe (Esc)")).clicked() {
                    self.langs.confirm_delete = false;
                }
            });
        }
        if let Some(message) = &self.langs.message {
            ui.add(egui::Label::new(RichText::new(message).size(12.5).color(t.dim)).wrap());
        }
    }

    /// Everything for language packs outside the settings window: the add and edit windows,
    /// file dialogs, the running translation and its progress while the settings are closed.
    pub(crate) fn language_packs(&mut self, ctx: &egui::Context) {
        self.poll_translation(ctx);
        while let Ok(pick) = self.langs.picks_rx.try_recv() {
            self.handle_language_pick(pick);
        }
        self.add_language_dialog(ctx);
        self.language_editor(ctx);
        if !self.settings_open {
            if let Some(job) = &self.langs.job {
                let share = if job.total == 0 { 0.0 } else { job.done as f32 / job.total as f32 };
                let text = i18n::tf("Oversetter {done} av {total} …", &[("done", &job.done.to_string()), ("total", &job.total.to_string())]);
                let stop = job.stop.clone();
                egui::Area::new(egui::Id::unique("cvbuilder.langpack-busy")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -48.0]).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::ProgressBar::new(share).text(text).desired_width(260.0));
                            if stop.load(Ordering::Relaxed) {
                                ui.spinner();
                            } else if ui.button(i18n::t("Stopp")).on_hover_text(i18n::t("Stopp oversettelsen. Det som er oversatt, beholdes.")).clicked() {
                                stop.store(true, Ordering::Relaxed);
                            }
                        });
                    });
                });
            }
        }
    }

    fn start_translation(&mut self, code: String, name: String) {
        if self.langs.job.is_some() {
            return;
        }
        let Some(backend) = self.chat_backend() else {
            self.langs.message = Some(i18n::t("Koble til en AI først, eller eksporter malen og oversett den selv.").into());
            self.ai.connect_open = true;
            self.ai.choice = self.settings.ai_service.or(Some(cv_ai::Service::Claude));
            return;
        };
        let made_by = match &backend {
            Backend::Api { endpoint, model } => {
                format!("AI ({} · {model})", endpoint.service.name())
            }
            Backend::ClaudeCode { .. } => "AI (Claude Code)".to_string(),
        };
        // Making a language again starts from what it has, so a stop keeps the rest.
        let strings = self.langs.packs.iter().find(|p| p.code == code).map(|p| p.strings.clone()).unwrap_or_default();
        let mut pack = Pack { code: code.clone(), name: name.clone(), made_by, strings };
        let items: Vec<(&'static str, &'static str)> = i18n::all_keys().collect();
        let total = items.len();
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let ctx = self.egui_ctx.clone();
        let language = format!("{name} ({code})");
        std::thread::spawn(move || {
            let mut save_error = None;
            let result = {
                let mut call = |system: &str, user: &str| ask(&backend, system, user);
                let mut on_batch = |got: &[(&'static str, String)], done: usize, total: usize| {
                    for (k, v) in got {
                        pack.strings.insert(k.to_string(), v.clone());
                    }
                    if let Err(e) = storage::save_language(&pack.code, &i18n::pack_to_json(&pack)) {
                        save_error = Some(e);
                    }
                    let _ = tx.send(Progress::Saved { done, total });
                    ctx.request_repaint();
                };
                translate_all(&items, &language, &mut call, &stop_flag, &mut on_batch)
            };
            let _ = tx.send(match result {
                Ok(report) => Progress::Finished { pack, report, save_error },
                Err(e) => Progress::Failed(e),
            });
            ctx.request_repaint();
        });
        self.langs.message = None;
        self.langs.confirm_delete = false;
        self.langs.job = Some(Job { code, name, done: 0, total, rx, stop });
    }

    fn poll_translation(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.langs.job else {
            return;
        };
        let mut finished = None;
        loop {
            match job.rx.try_recv() {
                Ok(Progress::Saved { done, total }) => {
                    job.done = done;
                    job.total = total;
                }
                Ok(other) => finished = Some(other),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // A thread that ended without a word (it panicked) also ends the job.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if finished.is_none() {
                        finished = Some(Progress::Failed(i18n::t("Oversettelsen stoppet uventet.").into()));
                    }
                    break;
                }
            }
        }
        let Some(finished) = finished else { return };
        let (code, name) = (job.code.clone(), job.name.clone());
        self.langs.job = None;
        match finished {
            Progress::Finished { pack, report, save_error } => {
                let mut message = if report.stopped {
                    i18n::tf("Stoppet. {n} tekster ble oversatt til {name} og er lagret.", &[("n", &report.translated.to_string()), ("name", &name)])
                } else {
                    i18n::tf("{name} er klart: {n} tekster ble oversatt.", &[("n", &report.translated.to_string()), ("name", &name)])
                };
                if report.failed > 0 {
                    message.push(' ');
                    message.push_str(&i18n::tf("{n} tekster ble ikke oversatt og vises som før, eller på engelsk.", &[("n", &report.failed.to_string())]));
                }
                if let Some(e) = save_error {
                    message.push(' ');
                    message.push_str(&i18n::tf("Kunne ikke lagre språkfilen: {e}", &[("e", &e)]));
                }
                if !pack.strings.is_empty() {
                    self.langs.packs.retain(|p| p.code != pack.code);
                    self.langs.packs.push(pack);
                    self.langs.packs.sort_by_key(|p| p.name.to_lowercase());
                    self.settings.ui_language = code;
                    storage::save_settings(&self.settings);
                    i18n::apply(&self.settings.ui_language, &self.langs.packs);
                }
                self.langs.message = Some(message);
            }
            Progress::Failed(e) => self.langs.message = Some(i18n::tf("Oversettelsen til {name} feilet: {e}", &[("name", &name), ("e", &e)])),
            Progress::Saved { .. } => {}
        }
        ctx.request_repaint();
    }

    fn add_language_dialog(&mut self, ctx: &egui::Context) {
        if self.langs.adding.is_none() {
            return;
        }
        let backend = self.chat_backend();
        let Some(adding) = &mut self.langs.adding else {
            return;
        };
        let t = Tokens::get(ctx);
        let mut close = false;
        let mut start: Option<(String, String)> = None;
        let mut connect = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.add-language")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading(i18n::t("Legg til språk"));
            ui.add_space(4.0);
            theme::hint(
                ui,
                &i18n::tf(
                    "AI-en oversetter alle de {n} tekstene i CVBuilder til språket du velger. Det tar noen minutter, og du kan bruke programmet imens.",
                    &[("n", &i18n::all_keys().count().to_string())],
                ),
            );
            ui.add_space(6.0);
            theme::label(ui, i18n::t("Språk"));
            let selected = if adding.name.trim().is_empty() { i18n::t("Velg et språk …").to_string() } else { adding.name.clone() };
            egui::ComboBox::from_id_salt("add-language-pick")
                .selected_text(selected)
                .width(260.0)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for (name, code) in SUGGESTIONS {
                        if ui.selectable_label(adding.code == *code, format!("{name}  ·  {code}")).on_hover_text(i18n::t("Bruk dette språket")).clicked() {
                            adding.name = name.to_string();
                            adding.code = code.to_string();
                        }
                    }
                })
                .response
                .on_hover_text(i18n::t("Velg et språk fra listen, eller skriv navnet og koden under"));
            theme::label(ui, i18n::t("Navn på språket, slik det skrives på språket selv"));
            let name_edit = ui
                .add(egui::TextEdit::singleline(&mut adding.name).hint_text("Deutsch").desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6)))
                .on_hover_text(i18n::t("Navnet vises i listen over språk, for eksempel Deutsch eller Tiếng Việt"));
            if name_edit.changed() {
                if let Some((_, code)) = SUGGESTIONS.iter().find(|(n, _)| n.eq_ignore_ascii_case(adding.name.trim())) {
                    adding.code = code.to_string();
                }
            }
            theme::label(ui, i18n::t("Språkkode"));
            ui.add(egui::TextEdit::singleline(&mut adding.code).hint_text("de, pt-BR").desired_width(140.0).margin(Margin::symmetric(8, 6)))
                .on_hover_text(i18n::t("En kort kode for språket (ISO 639), for eksempel de, pl, es, vi eller pt-BR. Den brukes også til å velge språket automatisk."));
            let code = i18n::normalize_code(&adding.code);
            let name = adding.name.trim().to_string();
            if let Err(e) = &code {
                if !adding.code.trim().is_empty() {
                    ui.label(RichText::new(pack_error(e)).size(12.5).color(t.danger));
                }
            }
            theme::hint(
                ui,
                i18n::t(
                    "CVBuilder har skrift for de fleste språk, også japansk, kinesisk og koreansk. Språk som skrives fra høyre mot venstre, som arabisk og hebraisk, vises ennå ikke riktig i menyene.",
                ),
            );
            ui.add_space(4.0);
            match &backend {
                Some(b) => {
                    ui.label(RichText::new(i18n::tf("Oversettes med {name}", &[("name", &b.name())])).color(t.dim));
                }
                None => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(i18n::t("Ingen AI tilkoblet")).color(t.dim));
                        if ui.button(i18n::t("Koble til AI")).on_hover_text(i18n::t("Velg en AI-tjeneste og legg inn API-nøkkelen din")).clicked() {
                            connect = true;
                        }
                    });
                }
            }
            theme::hint(ui, i18n::t("Uten AI: eksporter malen under Innstillinger, oversett den selv og importer den."));
            ui.add_space(8.0);
            let exists = code.as_ref().ok().and_then(|c| self.langs.packs.iter().find(|p| &p.code == c)).is_some();
            if exists {
                theme::hint(ui, i18n::t("Språket finnes allerede. Det oversettes på nytt og erstattes del for del."));
            }
            let ready = code.is_ok() && !name.is_empty() && backend.is_some();
            ui.horizontal(|ui| {
                if theme::primary(ui, i18n::t("Oversett"), i18n::t("Start oversettelsen. Du ser framdriften under Innstillinger og nederst i vinduet."), ready).clicked() {
                    if let Ok(c) = &code {
                        start = Some((c.clone(), name.clone()));
                    }
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre noe (Esc)")).clicked() {
                    close = true;
                }
            });
        });
        if connect {
            self.ai.connect_open = true;
            self.ai.choice = self.settings.ai_service.or(Some(cv_ai::Service::Claude));
            self.langs.adding = None;
            self.settings_open = false;
            return;
        }
        if let Some((code, name)) = start {
            self.langs.adding = None;
            self.start_translation(code, name);
        } else if close || r.should_close() {
            self.langs.adding = None;
        }
    }

    fn open_editor(&mut self, code: &str) {
        let Some(pack) = self.langs.packs.iter().find(|p| p.code == code) else {
            return;
        };
        let rows = i18n::all_keys().map(|(k, en)| (k, en, pack.strings.get(k).cloned().unwrap_or_default())).collect();
        self.langs.editor = Some(Editor { code: code.to_string(), rows, filter: String::new() });
    }

    fn language_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.langs.editor else {
            return;
        };
        let t = Tokens::get(ctx);
        let name = self.langs.packs.iter().find(|p| p.code == editor.code).map(|p| p.name.clone()).unwrap_or_default();
        let mut done: Option<bool> = None;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.language-editor")).show(ctx, |ui| {
            ui.set_width(760.0);
            ui.heading(i18n::tf("Rediger {name}", &[("name", &name)]));
            theme::hint(ui, i18n::t("Engelsk til venstre, oversettelsen til høyre. Et tomt felt vises på engelsk. Behold plassholdere i krøllparenteser som de står."));
            ui.add(egui::TextEdit::singleline(&mut editor.filter).hint_text(i18n::t("Søk …")).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6)))
                .on_hover_text(i18n::t("Vis bare tekster som inneholder dette, på engelsk, norsk eller i oversettelsen"));
            let needle = editor.filter.trim().to_lowercase();
            let shown: Vec<usize> = (0..editor.rows.len())
                .filter(|&i| {
                    let (k, en, v) = &editor.rows[i];
                    needle.is_empty() || en.to_lowercase().contains(&needle) || k.to_lowercase().contains(&needle) || v.to_lowercase().contains(&needle)
                })
                .collect();
            let row_height = ui.spacing().interact_size.y;
            egui::ScrollArea::vertical().max_height(440.0).auto_shrink([false, false]).show_rows(ui, row_height, shown.len(), |ui, range| {
                for &i in &shown[range] {
                    let (key, en, value) = &mut editor.rows[i];
                    ui.horizontal(|ui| {
                        let bad = !value.is_empty() && !i18n::valid_translation(key, value.as_str());
                        let color = if bad { t.danger } else { t.dim };
                        ui.add_sized([300.0, row_height], egui::Label::new(RichText::new(*en).color(color)).truncate()).on_hover_text(*en);
                        ui.add(egui::TextEdit::singleline(value).id_salt(("language-editor", i)).hint_text(*en).desired_width(f32::INFINITY))
                            .on_hover_text(i18n::t("Oversettelsen av teksten til venstre. Rød tekst: plassholderne stemmer ikke."));
                    });
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::primary(ui, i18n::t("Lagre"), i18n::t("Lagre språket og bruk endringene med en gang"), true).clicked() {
                    done = Some(true);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre noe (Esc)")).clicked() {
                    done = Some(false);
                }
            });
        });
        match done.or(r.should_close().then_some(false)) {
            Some(true) => {
                if let Some(editor) = self.langs.editor.take() {
                    self.save_edited(editor);
                }
            }
            Some(false) => self.langs.editor = None,
            None => {}
        }
    }

    fn save_edited(&mut self, editor: Editor) {
        let Some(pack) = self.langs.packs.iter_mut().find(|p| p.code == editor.code) else {
            return;
        };
        let mut skipped = 0;
        pack.strings.clear();
        for (key, _, value) in editor.rows {
            if value.is_empty() {
                continue;
            }
            if i18n::valid_translation(key, &value) {
                pack.strings.insert(key.to_string(), value);
            } else {
                skipped += 1;
            }
        }
        if !pack.made_by.contains(i18n::t("rettet for hånd")) {
            pack.made_by = if pack.made_by.is_empty() { i18n::t("rettet for hånd").to_string() } else { format!("{}, {}", pack.made_by, i18n::t("rettet for hånd")) };
        }
        let saved = storage::save_language(&pack.code, &i18n::pack_to_json(pack));
        let mut message = match saved {
            Ok(_) => i18n::tf("{name} er lagret.", &[("name", &pack.name)]),
            Err(e) => i18n::tf("Kunne ikke lagre språkfilen: {e}", &[("e", &e)]),
        };
        if skipped > 0 {
            message.push(' ');
            message.push_str(&i18n::tf("{n} tekster hadde feil plassholdere og vises på engelsk.", &[("n", &skipped.to_string())]));
        }
        self.langs.message = Some(message);
        i18n::apply(&self.settings.ui_language, &self.langs.packs);
    }

    fn delete_language(&mut self, code: &str) {
        self.langs.confirm_delete = false;
        let name = self.langs.packs.iter().find(|p| p.code == code).map(|p| p.name.clone()).unwrap_or_else(|| code.to_string());
        match storage::delete_language(code) {
            Ok(()) => {
                // A bundled language comes back as shipped; only the person's own copy goes.
                self.langs.packs = load_packs();
                if self.settings.ui_language == code && !is_bundled(code) {
                    self.settings.ui_language = "auto".into();
                    storage::save_settings(&self.settings);
                }
                i18n::apply(&self.settings.ui_language, &self.langs.packs);
                self.langs.message = Some(i18n::tf("{name} er slettet.", &[("name", &name)]));
            }
            Err(e) => self.langs.message = Some(i18n::tf("Kunne ikke slette språkfilen: {e}", &[("e", &e)])),
        }
    }

    /// The file dialogs, on their own thread so the window stays responsive.
    fn pick_language_file(&self, kind: &'static str, code: Option<String>) {
        let tx = self.langs.picks_tx.clone();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let dialog = || rfd::FileDialog::new().add_filter(i18n::t("CVBuilder-språkfil"), &["json"]);
            let picked = match (kind, code) {
                ("import", _) => dialog().pick_file().map(LangPick::Import),
                ("template", _) => dialog().set_file_name("cvbuilder-mal.cvlang.json").save_file().map(LangPick::Template),
                ("export", Some(code)) => dialog().set_file_name(storage::language_file(&code)).save_file().map(|p| LangPick::Export(code, p)),
                _ => None,
            };
            if let Some(p) = picked {
                let _ = tx.send(p);
                ctx.request_repaint();
            }
        });
    }

    fn handle_language_pick(&mut self, pick: LangPick) {
        let message = match pick {
            LangPick::Template(path) => match std::fs::write(&path, i18n::template_json()) {
                Ok(()) => i18n::tf("Malen er lagret: {path}. Oversett verdiene, fyll inn «code» og «name», og importer filen.", &[("path", &path.display().to_string())]),
                Err(e) => i18n::tf("Kunne ikke lagre filen: {e}", &[("e", &e.to_string())]),
            },
            LangPick::Export(code, path) => match self.langs.packs.iter().find(|p| p.code == code) {
                Some(pack) => match std::fs::write(&path, i18n::pack_to_json(pack)) {
                    Ok(()) => i18n::tf("Språket er lagret: {path}", &[("path", &path.display().to_string())]),
                    Err(e) => i18n::tf("Kunne ikke lagre filen: {e}", &[("e", &e.to_string())]),
                },
                None => return,
            },
            LangPick::Import(path) => self.import_language(&path),
        };
        self.langs.message = Some(message);
    }

    fn import_language(&mut self, path: &std::path::Path) -> String {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => return i18n::tf("Kunne ikke lese filen: {e}", &[("e", &e.to_string())]),
        };
        let checked = match i18n::parse_pack(&text) {
            Ok(c) => c,
            Err(e) => return pack_error(&e),
        };
        let pack = checked.pack;
        if let Err(e) = storage::save_language(&pack.code, &i18n::pack_to_json(&pack)) {
            return i18n::tf("Kunne ikke lagre språkfilen: {e}", &[("e", &e)]);
        }
        let mut message = i18n::tf("{name} er importert med {n} tekster.", &[("name", &pack.name), ("n", &pack.strings.len().to_string())]);
        if checked.dropped > 0 {
            message.push(' ');
            message.push_str(&i18n::tf("{n} tekster var tomme eller hadde feil plassholdere og vises på engelsk.", &[("n", &checked.dropped.to_string())]));
        }
        if checked.unknown > 0 {
            message.push(' ');
            message.push_str(&i18n::tf("{n} tekster finnes ikke i denne versjonen av CVBuilder og ble hoppet over.", &[("n", &checked.unknown.to_string())]));
        }
        let code = pack.code.clone();
        self.langs.packs.retain(|p| p.code != code);
        self.langs.packs.push(pack);
        self.langs.packs.sort_by_key(|p| p.name.to_lowercase());
        self.settings.ui_language = code;
        storage::save_settings(&self.settings);
        i18n::apply(&self.settings.ui_language, &self.langs.packs);
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every language that ships with CVBuilder loads in full: right format and code, nothing
    /// dropped for broken placeholders, and (almost) every text translated.
    #[test]
    fn bundled_languages_load_whole() {
        let keys = i18n::all_keys().count();
        for (code, text) in BUNDLED_LANGUAGES {
            let checked = i18n::parse_pack(text).unwrap_or_else(|e| panic!("{code}: {e:?}"));
            assert_eq!(checked.pack.code, *code, "{code}: the file's code");
            assert_eq!(checked.dropped, 0, "{code}: translations dropped (placeholders?)");
            let have = checked.pack.strings.len();
            assert!(have * 100 >= keys * 98, "{code}: only {have} of {keys} texts; run `node shared/packaging/lang-tools.cjs missing {code} …`");
        }
    }

    #[test]
    fn json_is_found_in_fences_and_prose() {
        let fenced = "Here you go:\n```json\n{\"1\": \"Herunterladen\", \"2\": \"Datei {file}\"}\n```\nHope it helps!";
        let map = extract_json(fenced).expect("fenced");
        assert_eq!(map["1"], "Herunterladen");
        assert_eq!(map["2"], "Datei {file}");
        let prose = "Sure {not json} – the result is {\"1\": \"a } b\", \"2\": \"x\"} and that's it.";
        let map = extract_json(prose).expect("prose");
        assert_eq!(map["1"], "a } b");
        assert_eq!(map.len(), 2);
        assert!(extract_json("no json here").is_none());
        assert!(extract_json("[1, 2]").is_none());
    }

    #[test]
    fn batches_respect_count_and_size() {
        let short: Vec<(&str, &str)> = (0..130).map(|_| ("k", "Save")).collect();
        let b = batches(&short);
        assert_eq!(b, vec![0..60, 60..120, 120..130]);
        let long = "x".repeat(4000);
        let mixed: Vec<(&str, &str)> = vec![("a", "Hi"), ("b", long.as_str()), ("c", long.as_str()), ("d", "Ok")];
        assert_eq!(batches(&mixed), vec![0..2, 2..4]);
        let huge = "y".repeat(9000);
        assert_eq!(batches(&[("a", huge.as_str()), ("b", "z")]), vec![0..1, 1..2]);
        assert!(batches(&[]).is_empty());
        // Every item is in exactly one batch, in order.
        let all: Vec<(&'static str, &'static str)> = i18n::all_keys().collect();
        let covered: usize = batches(&all).iter().map(|r| r.len()).sum();
        assert_eq!(covered, all.len());
    }

    #[test]
    fn request_is_numbered_json() {
        let text = request_text(&["Download PDF", "Say \"hi\""]);
        let v: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(v["1"], "Download PDF");
        assert_eq!(v["2"], "Say \"hi\"");
    }

    const ITEMS: &[(&str, &str)] = &[("Angre", "Undo"), ("Kunne ikke lagre: {e}", "Could not save: {e}"), ("Bilde", "Photo")];

    /// A fake AI: «DE:» before each string, and «{e}» dropped on the first try when asked.
    fn fake(calls: &mut usize, break_first: bool, reply: &str, user: &str) -> Result<String, String> {
        *calls += 1;
        let asked: Map<String, Value> = serde_json::from_str(user).map_err(|e| e.to_string())?;
        let mut out = Map::new();
        for (k, v) in asked {
            let mut s = format!("DE:{}", v.as_str().unwrap_or_default());
            if break_first && *calls == 1 {
                s = s.replace("{e}", "");
            }
            out.insert(k, Value::String(s));
        }
        Ok(format!("{reply}\n```json\n{}\n```", Value::Object(out)))
    }

    #[test]
    fn batches_are_translated_and_bad_strings_retried() {
        let stop = AtomicBool::new(false);
        let mut calls = 0;
        let mut seen: Vec<(&'static str, String)> = Vec::new();
        let mut progress = Vec::new();
        let report = translate_all(ITEMS, "Deutsch", &mut |_, user| fake(&mut calls, true, "Here:", user), &stop, &mut |got, done, total| {
            seen.extend(got.iter().cloned());
            progress.push((done, total));
        })
        .expect("translated");
        assert_eq!(report, Report { translated: 3, failed: 0, stopped: false });
        // The string that lost its placeholder was asked for again, alone.
        assert_eq!(calls, 2);
        assert!(seen.contains(&("Kunne ikke lagre: {e}", "DE:Could not save: {e}".to_string())));
        assert!(seen.contains(&("Angre", "DE:Undo".to_string())));
        assert_eq!(progress, vec![(3, 3)]);
    }

    #[test]
    fn failures_fall_back_and_are_counted() {
        let stop = AtomicBool::new(false);
        // Always unusable for the placeholder string: it is counted, the rest kept.
        let mut calls = 0;
        let mut translate = |_: &str, user: &str| {
            calls += 1;
            let mut v: Map<String, Value> = serde_json::from_str(user).map_err(|e| e.to_string())?;
            for value in v.values_mut() {
                if value.as_str().is_some_and(|s| s.contains("{e}")) {
                    *value = Value::String("kaputt".into());
                }
            }
            Ok(Value::Object(v).to_string())
        };
        let report = translate_all(ITEMS, "Deutsch", &mut translate, &stop, &mut |_, _, _| {}).expect("report");
        assert_eq!(report, Report { translated: 2, failed: 1, stopped: false });
        assert_eq!(calls, 2);
        // No answer at all on the first batch is an error.
        let err = translate_all(ITEMS, "Deutsch", &mut |_, _| Err("offline".to_string()), &stop, &mut |_, _, _| {});
        assert_eq!(err, Err("offline".to_string()));
        // Stopped before the first batch: nothing asked.
        stop.store(true, Ordering::Relaxed);
        let mut asked = false;
        let report = translate_all(
            ITEMS,
            "Deutsch",
            &mut |_, _| {
                asked = true;
                Ok(String::new())
            },
            &stop,
            &mut |_, _, _| {},
        )
        .expect("report");
        assert!(report.stopped && !asked);
    }

    #[test]
    fn the_prompt_names_the_language_and_the_rules() {
        let p = system_prompt("Polski (pl)");
        assert!(p.contains("into Polski (pl)"));
        assert!(p.contains("{placeholder}"));
        assert!(p.contains("JSON object only"));
        assert!(p.contains("CVBuilder, ATS, MCP, PDF, Word, LinkedIn, Claude"));
    }

    #[test]
    fn suggestions_have_valid_unique_codes() {
        let mut codes = std::collections::HashSet::new();
        for (name, code) in SUGGESTIONS {
            assert_eq!(i18n::normalize_code(code).as_deref(), Ok(*code), "{name}");
            assert!(codes.insert(*code), "duplicate {code}");
        }
    }
}
