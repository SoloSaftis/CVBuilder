//! «Oversett CV-en»: the CV into any language. The headings come from CVBuilder's own table
//! for common languages, or the person types them, or the AI writes them; the CV's text is
//! translated by the AI, or by the person with the original shown under each field.
use cv_core::Language;
use cv_core::headings::{self, Headings};
use egui::{CornerRadius, Margin, RichText};

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

/// The translate window's state.
pub struct TranslateState {
    /// The language's own name, from the list or typed.
    pub language: String,
    pub headings: Headings,
    /// The heading fields are shown.
    pub edit_headings: bool,
}

fn is_norwegian(l: &str) -> bool {
    matches!(l.trim().to_lowercase().as_str(), "bokmål" | "bokmal" | "norsk" | "norwegian")
}

fn is_english(l: &str) -> bool {
    matches!(l.trim().to_lowercase().as_str(), "english" | "engelsk")
}

impl TranslateState {
    pub fn new(language: &str) -> TranslateState {
        let mut s = TranslateState { language: String::new(), headings: Headings::default(), edit_headings: false };
        s.choose(language);
        s
    }

    fn choose(&mut self, language: &str) {
        self.language = language.to_string();
        self.headings = headings::built_in(language).unwrap_or_else(|| Headings { language: language.to_string(), ..Default::default() });
        self.edit_headings = !(is_norwegian(language) || is_english(language) || headings::built_in(language).is_some());
    }
}

impl CvApp {
    /// Opens the window, with the language the person picked (or the other one of bokmål/English).
    pub(crate) fn open_translate(&mut self, language: Option<Language>) {
        let name = match language {
            Some(Language::No) => "Bokmål",
            Some(Language::En) => "English",
            None if self.project.is_english() || self.project.options.headings.is_some() => "Bokmål",
            None => "English",
        };
        self.translate = Some(TranslateState::new(name));
    }

    /// From the ribbon's language list: the headings and dates change at once, offline; the
    /// text stays, with the original shown under the fields, until it is translated.
    pub(crate) fn switch_cv_language(&mut self, name: &str) {
        let state = TranslateState::new(name);
        self.apply_headings(&state);
        self.status = i18n::tf("CV-en har nå overskrifter på {language}. Trykk «Oversett» for å oversette teksten også.", &[("language", name)]);
    }

    /// Puts the headings in the chosen language; keeps the text as it was, as the original to
    /// translate from.
    fn apply_headings(&mut self, state: &TranslateState) {
        let before = (self.project.clone(), self.is_example);
        self.reference = Some(self.project.clone());
        if is_norwegian(&state.language) {
            self.project.options.headings = None;
            self.project.language = Language::No;
        } else if is_english(&state.language) {
            self.project.options.headings = None;
            self.project.language = Language::En;
        } else {
            self.project.language = Language::En;
            self.project.options.headings = Some(Headings { language: state.language.clone(), ..state.headings.clone() });
        }
        self.buffers.clear();
        self.undo = Some(before);
        self.touched();
    }

    pub(crate) fn translate_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.translate.take() else {
            return;
        };
        let t = Tokens::get(ctx);
        let has_ai = self.chat_backend().is_some();
        let mut choice: Option<u8> = None;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.translate")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading(i18n::t("Oversett CV-en"));
            ui.add_space(4.0);
            theme::label(ui, i18n::t("Språk"));
            let mut picked: Option<String> = None;
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("translate-language")
                    .selected_text(state.language.clone())
                    .width(200.0)
                    .height(420.0)
                    .show_ui(ui, |ui| {
                        for name in ["Bokmål", "English"].into_iter().chain(headings::built_in_languages()) {
                            if ui.selectable_label(state.language == name, name).on_hover_text(i18n::t("Overskriftene finnes ferdig på dette språket")).clicked() {
                                picked = Some(name.to_string());
                            }
                        }
                    })
                    .response
                    .on_hover_text(i18n::t("Velg et språk fra listen, eller skriv et hvilket som helst språk i feltet ved siden av"));
                let mut typed = if headings::built_in(&state.language).is_some() || is_norwegian(&state.language) || is_english(&state.language) { String::new() } else { state.language.clone() };
                if ui
                    .add(egui::TextEdit::singleline(&mut typed).hint_text(i18n::t("… eller skriv et språk")).desired_width(200.0).margin(Margin::symmetric(8, 6)))
                    .on_hover_text(i18n::t("Et hvilket som helst språk, skrevet på det språket, f.eks. Kiswahili eller 日本語"))
                    .changed()
                {
                    picked = Some(typed);
                }
            });
            if let Some(p) = picked {
                state.choose(&p);
            }
            ui.add_space(6.0);
            let custom = !(is_norwegian(&state.language) || is_english(&state.language));
            if custom {
                let known = headings::built_in(&state.language).is_some();
                ui.label(
                    RichText::new(if known {
                        i18n::t("Overskriftene finnes ferdig på dette språket.")
                    } else {
                        i18n::t("Overskriftene må skrives: AI-en gjør det, eller du fyller dem inn under.")
                    })
                    .size(12.5)
                    .color(t.dim),
                );
                if ui.selectable_label(state.edit_headings, i18n::t("Overskrifter i CV-en")).on_hover_text(i18n::t("Se og endre overskriftene CV-en får på dette språket")).clicked() {
                    state.edit_headings = !state.edit_headings;
                }
                if state.edit_headings {
                    egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(8)).show(ui, |ui| {
                        egui::Grid::new("translate-headings").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                            let h = &mut state.headings;
                            for (label, value) in [
                                (i18n::t("Profil"), &mut h.profile),
                                (i18n::t("Kjernekompetanse"), &mut h.skills),
                                (i18n::t("Arbeidserfaring"), &mut h.experience),
                                (i18n::t("Utdanning"), &mut h.education),
                                (i18n::t("Kurs og sertifiseringer"), &mut h.certifications),
                                (i18n::t("Språk"), &mut h.languages),
                                (i18n::t("Interesser"), &mut h.interests),
                                (i18n::t("Kontakt"), &mut h.contact),
                                (i18n::t("«nå» (nåværende jobb)"), &mut h.present),
                            ] {
                                ui.label(RichText::new(label).size(12.5).color(t.dim));
                                ui.add(egui::TextEdit::singleline(value).desired_width(280.0).margin(Margin::symmetric(6, 4)))
                                    .on_hover_text(i18n::t("Overskriften slik den skal stå i CV-en på dette språket"));
                                ui.end_row();
                            }
                        });
                    });
                }
            }
            ui.add_space(6.0);
            if has_ai {
                ui.add(egui::Label::new(i18n::t("AI-en oversetter all teksten og overskriftene. Du ser endringene mens den jobber og kan angre dem.")).wrap());
            } else {
                egui::Frame::new().fill(t.warning.gamma_multiply(0.14)).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(i18n::t("AI er ikke koblet til")).family(theme::medium()).color(t.warning));
                    ui.add(egui::Label::new(i18n::t("Da oversetter du teksten selv. Overskriftene byttes nå, og originalteksten vises under hvert felt mens du skriver.")).wrap());
                });
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                let target = state.language.trim().to_string();
                let ok = !target.is_empty();
                if has_ai && theme::primary(ui, i18n::t("Oversett med AI"), i18n::t("AI-en oversetter hele CV-en og overskriftene"), ok).clicked() {
                    choice = Some(1);
                }
                if !has_ai && theme::primary(ui, i18n::t("Koble til AI"), i18n::t("Koble til en AI-tjeneste eller en lokal modell for å oversette automatisk"), true).clicked() {
                    choice = Some(3);
                }
                if ui
                    .add_enabled(ok, egui::Button::new(i18n::t("Bare bytt overskrifter")))
                    .on_hover_text(i18n::t("Overskriftene byttes; teksten oversetter du selv med originalen som hjelp"))
                    .on_disabled_hover_text(i18n::t("Velg eller skriv et språk først"))
                    .clicked()
                {
                    choice = Some(2);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre noe")).clicked() {
                    choice = Some(0);
                }
            });
        });
        match choice.or(r.should_close().then_some(0)) {
            None => self.translate = Some(state),
            Some(1) => {
                self.reference = Some(self.project.clone());
                let lang = state.language.trim().to_string();
                let headings = if is_norwegian(&lang) || is_english(&lang) {
                    String::new()
                } else {
                    let h = &state.headings;
                    let given = [&h.profile, &h.skills, &h.experience, &h.education, &h.certifications, &h.languages, &h.interests, &h.contact, &h.present].iter().any(|s| !s.trim().is_empty());
                    if given {
                        format!(
                            " Bruk disse overskriftene: profile «{}», skills «{}», experience «{}», education «{}», certifications «{}», languages «{}», interests «{}», contact «{}», present «{}» (fyll inn de som mangler).",
                            h.profile, h.skills, h.experience, h.education, h.certifications, h.languages, h.interests, h.contact, h.present
                        )
                    } else {
                        String::new()
                    }
                };
                let ask = i18n::tf(
                    "Oversett hele CV-en til {language}: overskrift, profil, alle jobbpunkter og titler, kompetanse, utdanning, kurs, språk, interesser og egne seksjoner. Sett CV-ens overskrifter med cv_set_headings (language «{language}»).{headings} Behold alle fakta, navn på firma og tall.",
                    &[("language", &lang), ("headings", &headings)],
                );
                self.ask_ai(&ask);
            }
            Some(2) => {
                self.apply_headings(&state);
                self.status = i18n::tf("CV-en har nå overskrifter på {language}. Originalteksten vises under feltene mens du oversetter.", &[("language", state.language.trim())]);
            }
            Some(3) => {
                self.ai.connect_open = true;
                self.ai.choice = Some(cv_ai::Service::Claude);
            }
            Some(_) => {}
        }
    }
}
