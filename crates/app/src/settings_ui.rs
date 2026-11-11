//! «Innstillinger» under Fil: the app's own settings, set once and kept – menu language, theme,
//! remembering the draft, the AI connection and where the data is. Plus the window that asks
//! for a new section's name.
use cv_core::model::{CustomItem, CustomSection};
use egui::{CornerRadius, Margin, RichText};

use crate::i18n;
use crate::theme::{self, Tokens};
use crate::{CvApp, storage};

/// The theme setting as egui's preference.
pub fn theme_preference(value: &str) -> egui::ThemePreference {
    match value {
        "light" => egui::ThemePreference::Light,
        "dark" => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    }
}

impl CvApp {
    pub(crate) fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let t = Tokens::get(ctx);
        let mut close = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.settings")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading(i18n::t("Innstillinger"));
            ui.add_space(8.0);
            // Scrolls on a small screen, with «Lukk» always in view below.
            let height = (ui.ctx().content_rect().height() - 200.0).max(160.0);
            egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
                theme::section_title(ui, i18n::t("Språk i menyene"));
                theme::hint(ui, i18n::t("Språket i menyene og knappene. CV-ens eget språk velger du under Hjem."));
                // The choices, added languages and their buttons: langpack_ui.rs.
                self.language_settings(ui);
                ui.add_space(10.0);

                theme::section_title(ui, i18n::t("Tema"));
                ui.horizontal(|ui| {
                    for (value, name, tip) in [
                        ("system", i18n::t("Følg systemet"), i18n::t("Lyst eller mørkt, slik datamaskinen er innstilt")),
                        ("light", i18n::t("Lyst"), i18n::t("Lys bakgrunn")),
                        ("dark", i18n::t("Mørkt"), i18n::t("Mørk bakgrunn, skånsomt for øynene om kvelden")),
                    ] {
                        if ui.radio(self.settings.theme == value, name).on_hover_text(tip).clicked() && self.settings.theme != value {
                            self.settings.theme = value.to_string();
                            ctx.set_theme(theme_preference(value));
                            storage::save_settings(&self.settings);
                        }
                    }
                });
                ui.add_space(10.0);

                theme::section_title(ui, i18n::t("Utkast"));
                let mut remember = self.settings.remember;
                if ui.checkbox(&mut remember, i18n::t("Husk utkastet på denne maskinen")).on_hover_text(i18n::t("Utkastet lagres her mens du skriver. Slå av på en delt maskin.")).changed() {
                    self.set_remember(remember);
                }
                if ui
                    .checkbox(&mut self.settings.autosave, i18n::t("Lagre automatisk hvert 5. minutt"))
                    .on_hover_text(i18n::t("Når CV-en er lagret som fil, lagres endringene i den filen hvert 5. minutt, som i Word."))
                    .changed()
                {
                    storage::save_settings(&self.settings);
                }
                ui.add_space(10.0);

                theme::section_title(ui, "AI");
                ui.horizontal(|ui| {
                    let now = match self.chat_backend() {
                        Some(b) => i18n::tf("Tilkoblet: {name}", &[("name", &b.name())]),
                        None => i18n::t("Ingen AI tilkoblet").to_string(),
                    };
                    ui.label(RichText::new(now).color(t.dim));
                    if ui.button(i18n::t("Koble til AI")).on_hover_text(i18n::t("Velg en AI-tjeneste og legg inn API-nøkkelen din")).clicked() {
                        self.ai.connect_open = true;
                        self.ai.choice = self.settings.ai_service.or(Some(cv_ai::Service::Claude));
                        close = true;
                    }
                });
                ui.add_space(10.0);

                theme::section_title(ui, i18n::t("Data"));
                egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(if storage::portable() { i18n::t("Bærbar modus: alt lagres i mappen ved siden av programmet.") } else { i18n::t("Lagres i brukerens egen datamappe.") });
                    ui.add(egui::Label::new(RichText::new(storage::data_dir().display().to_string()).monospace().size(12.0)).wrap().selectable(true));
                    theme::hint(ui, i18n::t("API-nøkler lagres i maskinens nøkkelring, aldri i denne mappen."));
                });
            });
            ui.add_space(12.0);
            if theme::primary(ui, i18n::t("Lukk"), i18n::t("Lukk vinduet. Innstillingene er alt lagret."), true).clicked() {
                close = true;
            }
        });
        if close || r.should_close() {
            self.settings_open = false;
        }
    }

    /// Turning «remember the draft» on or off, from the settings or the status bar.
    pub(crate) fn set_remember(&mut self, remember: bool) {
        self.settings.remember = remember;
        storage::save_settings(&self.settings);
        if remember {
            self.persist();
        } else {
            storage::forget_draft();
            self.status = i18n::t("Utkastet huskes ikke lenger. Lagre som fil før du lukker.").into();
        }
    }

    /// Opens the window that asks for a new section's name, pre-filled with a section type.
    pub(crate) fn ask_section_name(&mut self, prefill: &str) {
        self.new_section = Some(prefill.to_string());
        self.new_section_at = None;
    }

    /// «Ny seksjon 1», «Ny seksjon 2» … (in the CV's language): the first number not in use.
    fn next_section_name(&self) -> String {
        let base = if self.project.is_english() { "New section" } else { "Ny seksjon" };
        (1..).map(|n| format!("{base} {n}")).find(|name| !self.project.custom_sections.iter().any(|s| s.title.trim().eq_ignore_ascii_case(name))).unwrap_or_else(|| base.to_string())
    }

    fn create_section(&mut self, name: &str) {
        let title = if name.trim().is_empty() { self.next_section_name() } else { name.trim().to_string() };
        let id = self.next_id("section");
        self.project.custom_sections.push(CustomSection { id: id.clone(), title: title.clone(), items: vec![CustomItem::default()] });
        // At the bottom unless a place was chosen; the whole order is then written down.
        if let Some(at) = self.new_section_at {
            let mut order = cv_core::text::section_order(&self.project);
            order.retain(|k| k != &id);
            order.insert(at.min(order.len()), id.clone());
            self.project.options.section_order = order;
        }
        self.folded.remove(&id);
        self.views.form = true;
        self.status = i18n::tf("Seksjonen «{name}» er lagt til. Fyll den ut i skjemaet.", &[("name", &title)]);
        self.touched();
    }

    pub(crate) fn section_name_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut name) = self.new_section.clone() else {
            return;
        };
        let mut done: Option<bool> = None;
        let placeholder = self.next_section_name();
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.section-name")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(i18n::t("Ny seksjon"));
            ui.add_space(4.0);
            theme::label(ui, i18n::t("Navn på seksjonen, slik det står i CV-en"));
            let edit = ui
                .add(egui::TextEdit::singleline(&mut name).hint_text(placeholder.as_str()).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6)))
                .on_hover_text(i18n::t("Overskriften seksjonen får i CV-en, f.eks. Prosjekter eller Frivillig arbeid"));
            // Focus the name once when the window opens, so the place list can still be used.
            let first = egui::Id::unique("cvbuilder.section-name.focused");
            if !ui.data(|d| d.get_temp::<bool>(first).unwrap_or(false)) {
                edit.request_focus();
                ui.data_mut(|d| d.insert_temp(first, true));
            }
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some(true);
            }
            theme::hint(ui, &i18n::tf("Står feltet tomt, heter den «{name}». Du kan endre navnet senere.", &[("name", &placeholder)]));
            ui.add_space(6.0);
            theme::label(ui, i18n::t("Plassering i CV-en"));
            let order = cv_core::text::section_order(&self.project);
            let title = |k: &str| {
                let t = cv_core::text::section_title(&self.project, k);
                if t.is_empty() { k.to_string() } else { t }
            };
            let current = match self.new_section_at {
                None => i18n::t("Nederst").to_string(),
                Some(0) => i18n::t("Øverst").to_string(),
                Some(i) => i18n::tf("Etter «{name}»", &[("name", &title(&order[(i - 1).min(order.len().saturating_sub(1))]))]),
            };
            egui::ComboBox::from_id_salt("section-place")
                .selected_text(current)
                .width(260.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.new_section_at, None, i18n::t("Nederst")).on_hover_text(i18n::t("Seksjonen kommer sist i CV-en. Du kan flytte den senere."));
                    ui.selectable_value(&mut self.new_section_at, Some(0), i18n::t("Øverst")).on_hover_text(i18n::t("Seksjonen kommer først, før profilen"));
                    for (i, key) in order.iter().enumerate() {
                        ui.selectable_value(&mut self.new_section_at, Some(i + 1), i18n::tf("Etter «{name}»", &[("name", &title(key))])).on_hover_text(i18n::t("Seksjonen kommer rett etter denne"));
                    }
                })
                .response
                .on_hover_text(i18n::t("Hvor i CV-en den nye seksjonen skal stå. Pilene på seksjonen flytter den senere."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::primary(ui, i18n::t("Legg til"), i18n::t("Legg seksjonen til i CV-en og skjemaet (Enter)"), true).clicked() {
                    done = Some(true);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre noe (Esc)")).clicked() {
                    done = Some(false);
                }
            });
        });
        match done.or(r.should_close().then_some(false)) {
            Some(true) => {
                self.new_section = None;
                self.create_section(&name);
                ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique("cvbuilder.section-name.focused")));
            }
            Some(false) => {
                self.new_section = None;
                ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique("cvbuilder.section-name.focused")));
            }
            None => self.new_section = Some(name),
        }
    }
}
