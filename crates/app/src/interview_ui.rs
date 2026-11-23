//! «Forbered intervjuet» (to-do 051): the sheet cv-core builds from the CV and the job ad –
//! the claims an interviewer is likely to ask about, what the ad asks for and where the CV
//! shows it, the gaps – with a notes box under each question. Copy it as text or save it as a
//! PDF to print. It works without AI; «Øv i AI-chatten» hands the questions to the chat only
//! when the person clicks it, and never the notes.
//!
//! The notes are the person's own preparation: they live in the data folder on this computer
//! (`intervju-notater.json`), never in the CV file and never in the CV.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use cv_core::interview::{self, Part, Sheet};
use egui::{CornerRadius, Margin, RichText};

use crate::checklist_ui::Status;
use crate::i18n;
use crate::theme::{self, Tokens};
use crate::{CvApp, status_line, storage};

const COPIED_FOR: Duration = Duration::from_millis(1600);

/// The menu language's words for a key from cv-core.
pub fn tr(key: &'static str, args: &[(&str, &str)]) -> String {
    i18n::tf(key, args)
}

/// This month as `year * 12 + month - 1`, the way cv-core counts months.
pub fn today_index() -> i32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 2_629_746).unwrap_or(0) as i32 + 1970 * 12
}

fn file() -> std::path::PathBuf {
    storage::data_dir().join("intervju-notater.json")
}

/// The notes on this computer, by item id. A file that cannot be read is moved aside, not
/// written over.
fn load() -> (BTreeMap<String, String>, String) {
    match std::fs::read_to_string(file()) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(notes) => (notes, String::new()),
            Err(e) => {
                let _ = std::fs::rename(file(), file().with_extension("json.bad"));
                (BTreeMap::new(), i18n::details(i18n::t("Intervjunotatene på denne maskinen kunne ikke leses og er lagt til side."), &e.to_string()))
            }
        },
        Err(_) => (BTreeMap::new(), String::new()),
    }
}

/// Writes then renames; no notes, no file.
fn save(notes: &BTreeMap<String, String>) -> Result<(), String> {
    let path = file();
    if notes.values().all(|n| n.trim().is_empty()) {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(notes).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[derive(Default)]
pub struct InterviewState {
    pub open: bool,
    pub notes: BTreeMap<String, String>,
    /// The notes are read from disk the first time the window opens.
    loaded: bool,
    copied: Option<Instant>,
    confirm_clear: bool,
    pub message: String,
}

impl InterviewState {
    pub fn note(&self, id: &str) -> String {
        self.notes.get(id).cloned().unwrap_or_default()
    }

    pub fn open(&mut self) {
        if !self.loaded {
            let (notes, message) = load();
            self.notes = notes;
            self.message = message;
            self.loaded = true;
        }
        self.open = true;
    }
}

/// What the chat gets when the person asks to practise: the questions (never the notes) and
/// the rules for the AI.
pub fn practice_prompt(project: &cv_core::Project, sheet: &Sheet) -> String {
    let questions = interview::to_text(project, sheet, &|_| String::new(), &tr);
    i18n::tf(
        "Øv på intervjuet med meg. Still meg ett spørsmål om gangen fra arket under, vent på svaret mitt, og gi kort og ærlig tilbakemelding før neste spørsmål. Ikke endre CV-en, og ikke finn på fakta om meg.\n\n{sheet}",
        &[("sheet", questions.trim())],
    )
}

impl CvApp {
    pub(crate) fn interview_sheet(&self) -> Sheet {
        interview::sheet(&self.project, today_index())
    }

    pub(crate) fn interview_dialog(&mut self, ctx: &egui::Context) {
        if !self.interview.open {
            return;
        }
        let t = Tokens::get(ctx);
        let sheet = self.interview_sheet();
        let has_ai = self.chat_backend().is_some();
        let mut close = false;
        let mut practise = false;
        let mut changed = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.interview")).show(ctx, |ui| {
            let width = (ctx.content_rect().width() - 80.0).clamp(320.0, 640.0);
            ui.set_width(width);
            ui.add(egui::Label::new(RichText::new(interview::title(&self.project, &tr)).heading()).wrap());
            theme::hint(ui, &tr("Bygd fra CV-en og annonsen. Kan du ikke snakke om en linje, bør den kanskje ut av CV-en.", &[]));
            if !self.interview.message.is_empty() {
                status_line(ui, Status::ToDo, &self.interview.message.clone());
            }
            if sheet.items.iter().all(|i| i.part == Part::Always) {
                status_line(ui, Status::ToDo, &tr("Ingenting å vise ennå. Skriv jobbene dine og lim inn stillingsannonsen, så fylles arket.", &[]));
            }
            ui.add_space(4.0);
            let height = (ctx.content_rect().height() - 260.0).max(160.0);
            egui::ScrollArea::vertical().id_salt("interview-scroll").max_height(height).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for part in Part::ALL {
                    let items: Vec<&interview::Item> = sheet.part(part).collect();
                    if items.is_empty() {
                        continue;
                    }
                    ui.add_space(6.0);
                    theme::section_title(ui, &tr(part.title(), &[]));
                    for item in items {
                        egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(10)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(egui::Label::new(RichText::new(interview::headline(item, &tr)).family(theme::medium()).color(t.text)).wrap());
                            if !item.subject.is_empty() {
                                ui.add(egui::Label::new(RichText::new(item.question.render(&tr)).color(t.text)).wrap());
                            }
                            if let Some(w) = interview::where_line(item, &tr) {
                                ui.add(egui::Label::new(RichText::new(w).size(12.5).color(t.dim)).wrap());
                            }
                            ui.add_space(2.0);
                            let w = ui.available_width();
                            let note = self.interview.notes.entry(item.id.clone()).or_default();
                            changed |= ui
                                .add(egui::TextEdit::multiline(note).hint_text(i18n::t("Notater")).desired_rows(1).desired_width(w))
                                .on_hover_text(i18n::t("Det du vil si. Lagres bare på denne maskinen, ikke i CV-filen."))
                                .changed();
                        });
                        ui.add_space(4.0);
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let just = self.interview.copied.is_some_and(|at| at.elapsed() < COPIED_FOR);
                if just {
                    ctx.request_repaint_after(COPIED_FOR);
                }
                let copy = if just { i18n::t("Kopiert") } else { i18n::t("Kopier som tekst") };
                if theme::primary(ui, copy, i18n::t("Kopier arket med notatene, til et dokument eller en e-post til deg selv"), true).clicked() {
                    let notes = |id: &str| self.interview.note(id);
                    ctx.copy_text(interview::to_text(&self.project, &sheet, &notes, &tr));
                    self.interview.copied = Some(Instant::now());
                }
                if ui.button(i18n::t("Lagre som PDF …")).on_hover_text(i18n::t("Lagre arket som PDF, med en linje å skrive på under hvert spørsmål")).clicked() {
                    self.pick("interview_pdf", None);
                }
                if has_ai && ui.button(i18n::t("Øv i AI-chatten")).on_hover_text(i18n::t("AI-en stiller spørsmålene ett om gangen og gir kort tilbakemelding. Notatene dine sendes ikke, og CV-en endres ikke.")).clicked() {
                    practise = true;
                }
                let any = self.interview.notes.values().any(|n| !n.trim().is_empty());
                if any && !self.interview.confirm_clear && ui.button(i18n::t("Slett notatene")).on_hover_text(i18n::t("Slett intervjunotatene fra denne maskinen")).clicked() {
                    self.interview.confirm_clear = true;
                }
                let default = ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk arket. Notatene er lagret."));
                crate::shortcuts::default_button(ui, &default);
                if default.clicked() {
                    close = true;
                }
            });
            if self.interview.confirm_clear {
                ui.add_space(4.0);
                ui.label(i18n::t("Slette alle intervjunotatene på denne maskinen?"));
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("Slett")).on_hover_text(i18n::t("Slett intervjunotatene fra denne maskinen")).clicked() {
                        self.interview.notes.clear();
                        self.interview.confirm_clear = false;
                        changed = true;
                    }
                    if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Behold notatene")).clicked() {
                        self.interview.confirm_clear = false;
                    }
                });
            }
        });
        if changed {
            self.interview.notes.retain(|_, n| !n.is_empty());
            match save(&self.interview.notes) {
                Ok(()) => self.interview.message.clear(),
                Err(e) => self.interview.message = i18n::failed(i18n::t("Intervjunotatene ble ikke lagret."), &e, cv_core::plain::TRY_AGAIN),
            }
        }
        if practise {
            self.interview.open = false;
            let ask = practice_prompt(&self.project, &sheet);
            self.ask_ai(cv_ai::cost::Work::Chat, &ask);
            return;
        }
        if close || r.should_close() {
            self.interview.open = false;
            self.interview.confirm_clear = false;
        }
    }

    pub(crate) fn write_interview_pdf(&mut self, path: &std::path::Path) {
        let sheet = self.interview_sheet();
        let notes = |id: &str| self.interview.note(id);
        let doc = interview::to_document(&self.project, &sheet, &notes, &tr, &i18n::current_code());
        let result = cv_core::document::to_pdf(&doc).map_err(|e| e.to_string()).and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
        let message = match result {
            Ok(()) => i18n::tf("Intervjuarket er lagret: {path}", &[("path", &path.display().to_string())]),
            Err(e) => i18n::failed(i18n::t("Filen ble ikke lagret."), &e, cv_core::plain::TRY_ANOTHER_PLACE),
        };
        self.status = message.clone();
        self.interview.message = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sheet_text_has_english() {
        for key in interview::KEYS {
            assert!(i18n::has_english(key), "«{key}» has no English");
        }
    }

    #[test]
    fn practising_sends_the_questions_but_not_the_notes() {
        let p = cv_core::Project::example();
        let sheet = interview::sheet(&p, 2026 * 12 + 10);
        // Other tests switch the menu language while this runs, so compare with the same words.
        let ask = practice_prompt(&p, &sheet);
        let questions = interview::to_text(&p, &sheet, &|_| String::new(), &tr);
        assert!(ask.ends_with(questions.trim()), "{ask}");
        // The claims go along; the notes cannot (`practice_prompt` is never given them).
        assert!(sheet.part(interview::Part::Claims).count() > 0);
        assert!(sheet.part(interview::Part::Claims).all(|i| ask.contains(&i.subject)));
    }
}
