//! The first start: which language the menus and the CV should be in, with the computer's own
//! language already chosen. Shown once; both can be changed later (Innstillinger, and the
//! language list on Hjem).
use egui::RichText;

use crate::i18n::{self, Choice};
use crate::menu_ui::Action;
use crate::theme::{self, Tokens};
use crate::translate_ui::{cv_language_list, set_cv_language, shown_name, system_cv_language};
use crate::{CvApp, storage};

/// The welcome window's three ways to start.
enum Start {
    Import,
    Empty,
    Example,
}

/// The welcome window's choices while it is open.
pub struct Welcome {
    /// «no», «en» or a pack's code.
    pub ui_language: String,
    /// The CV language's own name.
    pub cv_language: String,
}

impl Welcome {
    /// The computer's languages, for the menus and for the CV.
    pub fn for_this_computer(packs: &[i18n::Pack]) -> Welcome {
        let ui_language = match i18n::choose("auto", &i18n::locale(), packs) {
            Choice::No => "no".to_string(),
            Choice::En => "en".to_string(),
            Choice::Pack(i) => packs[i].code.clone(),
        };
        Welcome { ui_language, cv_language: system_cv_language().to_string() }
    }
}

impl CvApp {
    pub(crate) fn welcome_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.welcome.take() else {
            return;
        };
        let t = Tokens::get(ctx);
        // How to start, once the languages are chosen.
        let mut start: Option<Start> = None;
        let mut ui_changed = false;
        egui::Modal::new(egui::Id::unique("cvbuilder.welcome")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(RichText::new(i18n::t("Velkommen til CVBuilder")).size(19.0).family(theme::bold()));
            ui.add_space(4.0);
            theme::hint(ui, i18n::t("Velg språk. Språket på datamaskinen din er valgt, og du kan endre begge senere."));
            ui.add_space(10.0);

            theme::label(ui, i18n::t("Språk i menyene"));
            let packs = &self.langs.packs;
            let name_of = |code: &str| match code {
                "no" => i18n::menu_language_name("no", "Norsk (bokmål)"),
                "en" => i18n::menu_language_name("en", "English (US)"),
                c => packs.iter().find(|p| p.code == c).map(|p| i18n::menu_language_name(c, &p.name)).unwrap_or_else(|| c.to_string()),
            };
            let mut menu: Vec<(String, String)> = vec![("no".into(), name_of("no")), ("en".into(), name_of("en"))];
            menu.extend(packs.iter().map(|p| (p.code.clone(), name_of(&p.code))));
            menu.sort_by_key(|(_, n)| theme::sort_key(n));
            egui::ComboBox::from_id_salt("welcome-ui-language")
                .selected_text(name_of(&w.ui_language))
                .width(300.0)
                .height(2000.0)
                .show_ui(ui, |ui| {
                    for (code, name) in &menu {
                        if ui.selectable_label(w.ui_language == *code, name.as_str()).on_hover_text(i18n::t("Menyer og knapper på dette språket")).clicked() && w.ui_language != *code {
                            w.ui_language = code.clone();
                            ui_changed = true;
                        }
                    }
                })
                .response
                .on_hover_text(i18n::t("Språket i menyene og knappene"));
            ui.add_space(8.0);

            theme::label(ui, i18n::t("Språket i CV-en"));
            egui::ComboBox::from_id_salt("welcome-cv-language")
                .selected_text(shown_name(&w.cv_language))
                .width(300.0)
                .height(2000.0)
                .show_ui(ui, |ui| {
                    let (own, rest) = cv_language_list();
                    let has_own = own.is_some();
                    for (i, name) in own.into_iter().chain(rest).enumerate() {
                        if i == 1 && has_own {
                            ui.separator();
                        }
                        if ui.selectable_label(w.cv_language == name, shown_name(name)).on_hover_text(i18n::t("Overskriftene finnes ferdig på dette språket")).clicked() {
                            w.cv_language = name.to_string();
                        }
                    }
                })
                .response
                .on_hover_text(i18n::t("Overskrifter og datoer i CV-en. Teksten skriver du selv, eller AI-en oversetter den."));
            ui.add_space(14.0);
            theme::label(ui, i18n::t("Hvordan vil du starte?"));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                if theme::primary(ui, i18n::t("Hent inn CV-en jeg har"), i18n::t("Les inn en CV du allerede har: Word, PDF, tekst og mer, uten internett"), true).clicked() {
                    start = Some(Start::Import);
                }
                if ui.button(i18n::t("Start med en tom CV")).on_hover_text(i18n::t("Et tomt skjema på språket du valgte")).clicked() {
                    start = Some(Start::Empty);
                }
                if ui.button(i18n::t("Se et eksempel først")).on_hover_text(i18n::t("Et ferdig utfylt eksempel med en oppdiktet person. Du kan starte din egen når som helst.")).clicked() {
                    start = Some(Start::Example);
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new(i18n::t("Endres under Innstillinger og i språklisten på Hjem.")).size(12.0).color(t.faint));
        });
        // The menus switch while choosing, so the window itself is read in the chosen language.
        if ui_changed {
            i18n::apply(&w.ui_language, &self.langs.packs);
        }
        let Some(start) = start else {
            self.welcome = Some(w);
            return;
        };
        self.settings.ui_language = w.ui_language.clone();
        self.settings.cv_language = w.cv_language.clone();
        self.settings.welcomed = true;
        i18n::apply(&self.settings.ui_language, &self.langs.packs);
        storage::save_settings(&self.settings);
        // The example is shown in the chosen CV language (English text for languages other than
        // Norwegian); a CV that came back keeps its text and gets the chosen headings and dates.
        if self.is_example {
            self.project = crate::translate_ui::example_in(&w.cv_language);
        } else {
            set_cv_language(&mut self.project, &w.cv_language);
        }
        self.buffers.clear();
        self.preview.changed();
        match start {
            Start::Import => self.run_action(ctx, Action::ImportCv),
            Start::Empty => self.run_action(ctx, Action::New),
            Start::Example => {}
        }
    }

    /// The language a new CV starts in: the one chosen at the first start, else the computer's.
    pub(crate) fn new_cv_language(&self) -> String {
        if self.settings.cv_language.is_empty() { system_cv_language().to_string() } else { self.settings.cv_language.clone() }
    }
}
