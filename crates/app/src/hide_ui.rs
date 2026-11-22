//! One full CV, many tailored copies (041): items hidden in this copy stay in the form and the
//! file but are left out of the CV, the downloads, the checks and the page count. An eye button
//! on every job, bullet, education, skill, language, course and entry; a line at the top of the
//! form saying what is hidden and which CV this is a copy of; «Lagre tilpasset kopi …» in the File
//! menu.
use std::path::{Path, PathBuf};

use egui::{CornerRadius, Margin, RichText, Stroke};

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Icon, Tokens};

/// The eye button: open when the item is in this CV. Returns true when clicked.
pub(crate) fn eye(ui: &mut egui::Ui, hidden: bool) -> bool {
    let (icon, tip) = if hidden { (Icon::Hidden, i18n::t("Vis i denne CV-en igjen")) } else { (Icon::Shown, i18n::t("Ta ut av denne CV-en. Det blir stående i skjemaet og i filen.")) };
    theme::icon_button(ui, icon, tip, true).clicked()
}

/// Greys the text of what follows in `ui` when the item is hidden.
pub(crate) fn tint(ui: &mut egui::Ui, hidden: bool) {
    if hidden {
        let t = Tokens::get(ui.ctx());
        ui.visuals_mut().override_text_color = Some(t.faint);
    }
}

/// «Skjult i denne CV-en», after the title of a hidden job or entry.
pub(crate) fn hidden_pill(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    theme::pill(ui, i18n::t("Skjult i denne CV-en"), t.hover, t.dim).on_hover_text(i18n::t("Står i skjemaet og i filen, men ikke i CV-en, nedlastingene eller sjekkene"));
}

/// The width of [`hidden_pill`], so a hidden item's title leaves room for it.
pub(crate) fn pill_width(ui: &egui::Ui) -> f32 {
    let font = egui::FontId::new(12.0, theme::medium());
    ui.fonts_mut(|f| f.layout_no_wrap(i18n::t("Skjult i denne CV-en").to_string(), font, egui::Color32::WHITE).size().x) + 24.0
}

/// Where the full CV of a tailored copy is: `copy_of` is a file name next to the copy, or a path.
pub(crate) fn full_cv_path(copy: &Path, copy_of: &str) -> PathBuf {
    let named = Path::new(copy_of);
    if named.is_absolute() { named.to_path_buf() } else { copy.parent().map(|p| p.join(named)).unwrap_or_else(|| named.to_path_buf()) }
}

/// What a copy saved at `copy` records as its full CV, `full`: the file name when both are in the
/// same folder (so the pair can be moved together), else the whole path.
pub(crate) fn copy_of_value(copy: &Path, full: &Path) -> String {
    if copy.parent() == full.parent() { full.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() } else { full.display().to_string() }
}

impl CvApp {
    /// The line at the top of the form: which CV this is a copy of, and what is hidden in it.
    pub(crate) fn hidden_banner(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let count = self.project.hidden_count();
        let copy_of = self.project.options.copy_of.clone();
        if count == 0 && copy_of.is_empty() {
            return;
        }
        let t = Tokens::get(ui.ctx());
        egui::Frame::new().fill(t.accent_soft).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(theme::R_LG)).inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !copy_of.is_empty() {
                let name = Path::new(&copy_of).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| copy_of.clone());
                ui.add(egui::Label::new(RichText::new(i18n::tf("Tilpasset kopi av «{file}»", &[("file", &name)])).family(theme::medium()).size(13.5)).wrap());
                ui.add(egui::Label::new(RichText::new(i18n::t("Endringer her gjelder bare denne kopien. Hele CV-en er uendret.")).size(12.5).color(t.dim)).wrap());
            }
            if count > 0 {
                let text = if count == 1 {
                    i18n::t("1 del er skjult i denne CV-en. Den står i skjemaet og i filen, men ikke i CV-en, nedlastingene eller sjekkene.").to_string()
                } else {
                    i18n::tf("{n} deler er skjult i denne CV-en. De står i skjemaet og i filen, men ikke i CV-en, nedlastingene eller sjekkene.", &[("n", &count.to_string())])
                };
                ui.add(egui::Label::new(RichText::new(text).size(12.5)).wrap());
            }
            ui.horizontal_wrapped(|ui| {
                if count > 0 {
                    if ui.button(i18n::t("Vis alt igjen")).on_hover_text(i18n::t("Ta med alt i CV-en igjen")).clicked() {
                        self.remember(crate::step::SHOW_ALL);
                        self.project.options.hidden.clear();
                        self.status = i18n::t("Alt står i CV-en igjen. Angre hvis det var feil.").into();
                        *changed = true;
                    }
                    ui.checkbox(&mut self.show_hidden, i18n::t("Vis skjulte deler i skjemaet")).on_hover_text(i18n::t("Slå av for å se bare det som står i CV-en"));
                }
                if !copy_of.is_empty() && self.file.is_some() && theme::ghost(ui, i18n::t("Åpne hele CV-en"), i18n::t("Åpne CV-en denne kopien ble laget fra")).clicked() {
                    self.open_full_cv();
                }
            });
        });
        ui.add_space(10.0);
    }

    /// Under a job's or an entry's bullets: which of them are in this CV. Folded to one line
    /// until opened, and open while some are hidden.
    pub(crate) fn bullet_picker(&mut self, ui: &mut egui::Ui, owner: &str, bullets: &[String], ids: &[String], changed: &mut bool) {
        if bullets.len() < 2 && !ids.iter().any(|id| self.project.is_hidden(id)) {
            return;
        }
        let shown = ids.iter().filter(|id| !self.project.is_hidden(id)).count();
        let open = self.bullet_pick.contains(owner) || shown < ids.len();
        let label = i18n::tf("Punkter i denne CV-en: {shown} av {all}", &[("shown", &shown.to_string()), ("all", &ids.len().to_string())]);
        let tip = i18n::t("Velg hvilke punkter som står i denne CV-en. De andre blir stående her, men skjult.");
        if theme::ghost(ui, &format!("{} {label}", if open { "–" } else { "+" }), tip).clicked() {
            if !self.bullet_pick.remove(owner) {
                self.bullet_pick.insert(owner.to_string());
            }
        }
        if !open {
            return;
        }
        let t = Tokens::get(ui.ctx());
        let mut toggle = None;
        for (id, text) in ids.iter().zip(bullets) {
            let hidden = self.project.is_hidden(id);
            ui.horizontal(|ui| {
                let mut on = !hidden;
                if ui.add(egui::Checkbox::without_text(&mut on)).on_hover_text(i18n::t("Med i denne CV-en")).changed() {
                    toggle = Some(id.clone());
                }
                let color = if hidden { t.faint } else { t.text };
                ui.add(egui::Label::new(RichText::new(text).size(12.5).color(color)).truncate()).on_hover_text(text);
            });
        }
        if let Some(id) = toggle {
            let hidden = self.project.is_hidden(&id);
            self.project.set_hidden(&id, !hidden);
            *changed = true;
        }
    }

    /// Shows or hides `id` in this CV.
    pub(crate) fn toggle_hidden(&mut self, id: &str) {
        let hidden = self.project.is_hidden(id);
        self.project.set_hidden(id, !hidden);
    }

    /// The file name a tailored copy is offered under: the CV's usual name (it has the employer
    /// in it), never the full CV's own.
    pub(crate) fn copy_file_name(&self) -> String {
        let name = format!("{}_utkast.json", self.file_stem());
        let own = self.file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().to_string());
        if own.as_deref() == Some(name.as_str()) { format!("{}_kopi_utkast.json", self.file_stem()) } else { name }
    }

    /// «Lagre tilpasset kopi …» chose `path`: the CV with its hide choices, ad and keywords goes
    /// there and is the open CV from now on; the full CV's file stays as it is.
    pub(crate) fn save_copy_to(&mut self, path: &Path) {
        let Some(full) = self.file.clone() else { return };
        if full == path {
            self.status = i18n::t("Velg et annet filnavn: kopien skal ikke erstatte hele CV-en.").into();
            return;
        }
        // A copy of a copy belongs to the same full CV.
        let full_cv = if self.project.options.copy_of.is_empty() { full.clone() } else { full_cv_path(&full, &self.project.options.copy_of) };
        let before = self.project.options.copy_of.clone();
        self.project.options.copy_of = copy_of_value(path, &full_cv);
        self.save_to(path, false);
        if self.file.as_deref() == Some(path) {
            let name = full_cv.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            self.status = i18n::tf("Den tilpassede kopien er lagret i {path}. Hele CV-en i «{file}» er uendret.", &[("path", &path.display().to_string()), ("file", &name)]);
        } else {
            self.project.options.copy_of = before;
        }
    }

    /// The banner's «Åpne hele CV-en»: asks to save this copy first when it has changes.
    pub(crate) fn open_full_cv(&mut self) {
        let Some(file) = self.file.clone() else { return };
        let full = full_cv_path(&file, &self.project.options.copy_of);
        if !full.is_file() {
            let name = full.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            self.status = i18n::tf("Fant ikke «{file}». Den kan være flyttet eller ha fått nytt navn.", &[("file", &name)]);
            return;
        }
        self.open_after = Some(full);
        if self.unsaved() {
            self.ask_to_save(crate::Leave::OpenFull);
        } else {
            self.open_full_now();
        }
    }

    pub(crate) fn open_full_now(&mut self) {
        if let Some(path) = self.open_after.take() {
            self.open_path(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_finds_its_full_cv_next_to_it_or_by_path() {
        let dir = std::env::temp_dir();
        let full = dir.join("Kari_CV_utkast.json");
        let copy = dir.join("Kari_CV_Acme_utkast.json");
        assert_eq!(copy_of_value(&copy, &full), "Kari_CV_utkast.json");
        assert_eq!(full_cv_path(&copy, "Kari_CV_utkast.json"), full);
        let elsewhere = dir.join("søknader").join("Kari_CV_Acme_utkast.json");
        let value = copy_of_value(&elsewhere, &full);
        assert_eq!(PathBuf::from(&value), full);
        assert_eq!(full_cv_path(&elsewhere, &value), full);
    }
}
