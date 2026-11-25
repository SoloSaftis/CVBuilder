//! «Forbered intervjuet» (to-do 051): the sheet cv-core builds from the CV and the job ad –
//! the claims an interviewer is likely to ask about, what the ad asks for and where the CV
//! shows it, the gaps – with a notes box under each question. Copy it as text or save it as a
//! PDF to print. It works without AI; «Øv i AI-chatten» hands the questions to the chat only
//! when the person clicks it, and never the notes.
//!
//! The notes are the person's own preparation: they live in the data folder on this computer
//! (`intervju-notater.json`), never in the CV file and never in the CV, and apart for each CV and job
//! (`cv_core::interview::notes_scope`), so a note for one application never shows on another.
use std::time::{Duration, Instant};

use cv_core::interview::{self, Notes, Part, Sheet};
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

/// The notes on this computer, for every CV and job. A file that cannot be read is moved aside,
/// never written over.
fn load(path: &std::path::Path) -> (Notes, String) {
    match std::fs::read_to_string(path) {
        Ok(text) => match Notes::from_json(&text) {
            Ok(notes) => (notes, String::new()),
            Err(e) => {
                let _ = set_aside(path);
                (Notes::default(), i18n::details(i18n::t("Intervjunotatene på denne maskinen kunne ikke leses og er lagt til side."), &e))
            }
        },
        Err(_) => (Notes::default(), String::new()),
    }
}

/// Moves a file that cannot be read to `<name>.bad`, or `<name>.bad-2`, `-3` … when an earlier
/// one is there, so no copy set aside before is written over.
fn set_aside(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for n in 1..10_000 {
        let to = path.with_file_name(if n == 1 { format!("{name}.bad") } else { format!("{name}.bad-{n}") });
        if !to.exists() {
            std::fs::rename(path, &to)?;
            return Ok(to);
        }
    }
    Err(std::io::Error::other("too many copies set aside"))
}

/// Writes then renames; no notes, no file.
fn save(path: &std::path::Path, notes: &Notes) -> Result<(), String> {
    if notes.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, notes.to_json()?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[derive(Default)]
pub struct InterviewState {
    pub open: bool,
    /// For every CV and job; the sheet shows only those of `interview::notes_scope`.
    pub notes: Notes,
    /// The notes are read from disk the first time the window opens.
    loaded: bool,
    copied: Option<Instant>,
    confirm_clear: bool,
    pub message: String,
}

impl InterviewState {
    /// The note on item `id` of this CV's and job's sheet.
    pub fn note(&self, project: &cv_core::Project, id: &str) -> String {
        self.notes.get(&interview::notes_scope(project), id).to_string()
    }

    pub fn open(&mut self) {
        if !self.loaded {
            let (notes, message) = load(&file());
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
        let scope = interview::notes_scope(&self.project);
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
                            let note = self.interview.notes.entry(&scope, &item.id);
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
                    let notes = |id: &str| self.interview.notes.get(&scope, id).to_string();
                    ctx.copy_text(interview::to_text(&self.project, &sheet, &notes, &tr));
                    self.interview.copied = Some(Instant::now());
                }
                if ui.button(i18n::t("Lagre som PDF …")).on_hover_text(i18n::t("Lagre arket som PDF, med en linje å skrive på under hvert spørsmål")).clicked() {
                    self.pick("interview_pdf", None);
                }
                if has_ai && ui.button(i18n::t("Øv i AI-chatten")).on_hover_text(i18n::t("AI-en stiller spørsmålene ett om gangen og gir kort tilbakemelding. Notatene dine sendes ikke, og CV-en endres ikke.")).clicked() {
                    practise = true;
                }
                let any = self.interview.notes.any(&scope);
                if any && !self.interview.confirm_clear && ui.button(i18n::t("Slett notatene")).on_hover_text(i18n::t("Slett notatene på dette arket, for denne CV-en og denne jobben")).clicked() {
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
                ui.label(i18n::t("Slette notatene på dette arket? Notatene for andre CV-er og jobber blir liggende."));
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("Slett")).on_hover_text(i18n::t("Slett notatene på dette arket, for denne CV-en og denne jobben")).clicked() {
                        self.interview.notes.clear(&scope);
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
            self.interview.notes.tidy();
            match save(&file(), &self.interview.notes) {
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
        let notes = |id: &str| self.interview.note(&self.project, id);
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

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cvbuilder-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_that_cannot_be_read_is_set_aside_without_writing_over_an_earlier_one() {
        let dir = temp_dir("interview-bad");
        let path = dir.join("intervju-notater.json");
        for (i, text) in ["first {", "second [", "third"].iter().enumerate() {
            std::fs::write(&path, text).unwrap();
            let (notes, message) = load(&path);
            assert!(notes.is_empty() && !message.is_empty());
            assert!(!path.exists(), "moved aside, round {i}");
        }
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(read("intervju-notater.json.bad"), "first {");
        assert_eq!(read("intervju-notater.json.bad-2"), "second [");
        assert_eq!(read("intervju-notater.json.bad-3"), "third");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_first_notes_file_is_upgraded_and_its_notes_kept() {
        let dir = temp_dir("interview-old");
        let path = dir.join("intervju-notater.json");
        std::fs::write(&path, r#"{"why": "Fordi"}"#).unwrap();
        let (mut notes, message) = load(&path);
        assert!(message.is_empty(), "{message}");
        let p = cv_core::Project::example();
        let mut other = p.clone();
        other.target.company = "Et annet firma".into();
        let (scope, elsewhere) = (interview::notes_scope(&p), interview::notes_scope(&other));
        assert_eq!(notes.get(&scope, "why"), "", "an old note is not shown on any sheet");
        *notes.entry(&scope, "ask") = "Om teamet".into();
        *notes.entry(&elsewhere, "ask") = "Om lønn".into();
        save(&path, &notes).unwrap();
        // «Slett notatene» on this sheet: the other job's notes and the old ones stay.
        notes.clear(&scope);
        save(&path, &notes).unwrap();
        let (again, _) = load(&path);
        assert_eq!(again.unassigned.get("why").map(String::as_str), Some("Fordi"));
        assert!(!again.any(&scope));
        assert_eq!(again.get(&elsewhere, "ask"), "Om lønn");
        // With nothing at all left, there is no file.
        save(&path, &Notes::default()).unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
