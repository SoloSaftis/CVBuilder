//! The ribbon at the top, laid out like Word's: a row of tabs (Fil, Hjem, Sett inn, Design, Vis,
//! AI, Hjelp) over a band of tool groups with their names underneath. The band can be folded
//! away (^). «Om CVBuilder» under Hjelp shows the version and the exact build.
use cv_core::Project;
use cv_core::model::{Certification, ContactField, CustomItem, CustomSection, Education, Interest, Job, Skill, SpokenLanguage};
use egui::{CornerRadius, Margin, RichText, Stroke};

use crate::i18n;
use crate::theme::{self, Tokens};
use crate::{CvApp, Export, Side, Views};

/// The version and the exact source it was built from (set by build.rs).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD: &str = env!("CVBUILDER_BUILD");

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Insert,
    Page,
    Design,
    View,
    Ai,
    Help,
}

enum Action {
    New,
    Example,
    Open,
    Save,
    Export(Export),
    Undo,
    /// Switch the CV's headings and dates to this language (offline).
    Headings(String),
    Translate,
    Add(&'static str),
    AddSection(&'static str),
    Photo,
    ImportCv,
    PasteAd,
    AdLink,
    AdFile,
    Import,
    SaveDesign,
    ConnectAi,
    Mcp,
    NewChat,
    Chat,
    Checks,
    About,
    DesignFormat,
    ShowAll,
    Settings,
}

/// A tool button in the ribbon: a label, and a short hint on hover (also while it is off).
fn tool(ui: &mut egui::Ui, icon: &str, text: &str, tip: &str, enabled: bool) -> bool {
    debug_assert!(!tip.trim().is_empty(), "the ribbon tool «{text}» needs a tip");
    let b = theme::tool_button(ui, icon, text, enabled, false);
    ui.add_enabled(enabled, b).on_hover_text(tip).on_disabled_hover_text(tip).clicked()
}

/// An item in the Fil menu: icon and text.
fn menu_item<'a>(ui: &egui::Ui, icon: &str, text: &str) -> egui::Button<'a> {
    let t = Tokens::get(ui.ctx());
    egui::Button::new((RichText::new(icon).family(theme::icons()).size(15.0).color(t.accent), RichText::new(text)))
}

/// A ribbon tool that opens a menu: icon, label and a small ▾.
pub(crate) fn tool_menu(ui: &mut egui::Ui, icon: &str, text: &str, tip: &str, body: impl FnOnce(&mut egui::Ui)) {
    debug_assert!(!tip.trim().is_empty(), "the ribbon menu «{text}» needs a tip");
    let b = theme::tool_button(ui, icon, text, true, true);
    egui::containers::menu::MenuButton::from_button(b).ui(ui, body).0.on_hover_text(tip);
}

/// What a download does, and what is missing while it cannot be done.
fn export_tip(kind: Export, blocked: bool) -> String {
    let what = match kind {
        Export::Pdf => i18n::t("Lagre CV-en som PDF med ekte, søkbar tekst – den formen søknadssystemer (ATS) leser sikrest."),
        Export::Word => i18n::t("Lagre CV-en som Word-fil (.docx), for portaler og rekrutterere som ber om Word."),
        Export::Text => i18n::t("Til portaler som ber deg lime inn CV-en som tekst"),
    };
    if blocked { format!("{what}\n\n{}", i18n::t("Se «Sjekkliste» for hva som mangler før nedlasting.")) } else { what.to_string() }
}

/// One group of tools with its name underneath and a thin line after it, like Word.
pub(crate) fn group(ui: &mut egui::Ui, name: &str, body: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.vertical(|ui| {
        // Tools on top, the group's name at the same height in every group.
        let top = ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            body(ui);
        });
        ui.add_space((58.0 - top.response.rect.height()).max(0.0));
        let w = top.response.rect.width();
        ui.allocate_ui_with_layout(egui::vec2(w, 16.0), egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(RichText::new(name).size(11.5).color(t.faint)).wrap_mode(egui::TextWrapMode::Extend));
        });
    });
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, 74.0), egui::Sense::hover());
    ui.painter().line_segment([rect.center_top() + egui::vec2(0.0, 6.0), rect.center_bottom()], Stroke::new(1.0, t.border));
}

/// Two short columns of tools inside a group.
pub(crate) fn column(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
        body(ui);
    });
}

impl CvApp {
    pub(crate) fn ribbon(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let mut action: Option<Action> = None;
        let blocked = !cv_core::checks::blocking(&self.project).is_empty();
        // Tabs, with the fold button on the right.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let file = ui
                .add(
                    egui::Button::new(RichText::new(i18n::t("Fil")).size(13.5).color(t.on_accent).family(theme::medium()))
                        .fill(t.accent)
                        .corner_radius(CornerRadius::same(theme::R_SM))
                        .min_size(egui::vec2(46.0, 28.0)),
                )
                .on_hover_text(i18n::t("Ny CV, åpne og lagre utkast, importer, last ned og innstillinger"));
            egui::Popup::menu(&file).show(|ui| {
                ui.set_min_width(230.0);
                if ui.add(menu_item(ui, theme::ph::FILE_PLUS, i18n::t("Ny CV"))).on_hover_text(i18n::t("Start på en tom CV. Den du har nå kan hentes tilbake med Angre.")).clicked() {
                    action = Some(Action::New);
                }
                if ui.add(menu_item(ui, theme::ph::LIGHTBULB, i18n::t("Åpne eksempel"))).on_hover_text(i18n::t("Et fiktivt eksempel med anbefalt oppsett og rekkefølge")).clicked() {
                    action = Some(Action::Example);
                }
                ui.separator();
                if ui.add(menu_item(ui, theme::ph::FOLDER_OPEN, i18n::t("Åpne"))).on_hover_text(i18n::t("Åpne et lagret CVBuilder-utkast (.json)")).clicked() {
                    action = Some(Action::Open);
                }
                if ui.add(menu_item(ui, theme::ph::FLOPPY_DISK, i18n::t("Lagre som"))).on_hover_text(i18n::t("Lagre utkastet som fil")).clicked() {
                    action = Some(Action::Save);
                }
                if ui.add(menu_item(ui, theme::ph::ARROW_SQUARE_IN, i18n::t("Importer CV fra fil"))).on_hover_text(i18n::t("Hent inn en CV du har fra før: PDF, Word, tekst eller nettside – med eller uten AI")).clicked() {
                    action = Some(Action::ImportCv);
                }
                ui.separator();
                for (kind, icon, name) in [(Export::Pdf, theme::ph::FILE_PDF, i18n::t("Last ned PDF")), (Export::Word, theme::ph::FILE_DOC, i18n::t("Last ned Word")), (Export::Text, theme::ph::COPY, i18n::t("Kopier CV-en som tekst"))] {
                    let tip = export_tip(kind, blocked);
                    if ui.add(menu_item(ui, icon, name)).on_hover_text(&tip).clicked() {
                        action = Some(Action::Export(kind));
                    }
                }
                ui.separator();
                if ui.add(menu_item(ui, theme::ph::GEAR, i18n::t("Innstillinger"))).on_hover_text(i18n::t("Språk i menyene, tema og mer")).clicked() {
                    action = Some(Action::Settings);
                }
                if ui.add(menu_item(ui, theme::ph::INFO, i18n::t("Om CVBuilder"))).on_hover_text(i18n::t("Versjon, bygg og lisenser")).clicked() {
                    action = Some(Action::About);
                }
            });
            ui.add_space(6.0);
            let tabs = [
                (Tab::Home, i18n::t("Hjem"), i18n::t("Fil, stillingsannonse, nedlasting, angre og CV-ens språk")),
                (Tab::Insert, i18n::t("Sett inn"), i18n::t("Legg til jobber, utdanning, kurs, kompetanse, språk og egne seksjoner")),
                (Tab::Page, i18n::t("Sideoppsett"), i18n::t("Marger, papirstørrelse, spalter, sideskift og tekststørrelse")),
                (Tab::Design, i18n::t("Design"), i18n::t("Velg utseende og farger, eller importer et eget design")),
                (Tab::View, i18n::t("Vis"), i18n::t("Velg hvilke visninger du ser, og zoom i forhåndsvisningen")),
                (Tab::Ai, "AI", i18n::t("Koble til AI, åpne chatten og se sjekklisten")),
                (Tab::Help, i18n::t("Hjelp"), i18n::t("Om CVBuilder, format for egne design og innstillinger")),
            ];
            for (tab, name, tip) in tabs {
                let on = self.ribbon_tab == tab && !self.ribbon_folded;
                let r = ui
                    .add(
                        egui::Button::new(RichText::new(name).size(13.5).family(if on { theme::bold() } else { theme::medium() }).color(if on { t.text } else { t.dim }))
                            .frame_when_inactive(false)
                            .corner_radius(CornerRadius::same(theme::R_SM))
                            .min_size(egui::vec2(0.0, 28.0)),
                    )
                    .on_hover_text(tip);
                if on {
                    let rect = r.rect;
                    ui.painter().line_segment([rect.left_bottom() + egui::vec2(8.0, 0.0), rect.right_bottom() - egui::vec2(8.0, 0.0)], Stroke::new(2.5, t.accent));
                }
                if r.clicked() {
                    if self.ribbon_folded {
                        self.ribbon_folded = false;
                    }
                    self.ribbon_tab = tab;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if theme::icon_button(
                    ui,
                    if self.ribbon_folded { theme::Icon::Down } else { theme::Icon::Up },
                    if self.ribbon_folded { i18n::t("Vis verktøylinjen") } else { i18n::t("Skjul verktøylinjen") },
                    true,
                )
                .clicked()
                {
                    self.ribbon_folded = !self.ribbon_folded;
                }
            });
        });
        if self.ribbon_folded {
            if let Some(a) = action {
                self.run_action(ui.ctx(), a);
            }
            return;
        }
        ui.add_space(2.0);
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin { left: 10, right: 10, top: 6, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            egui::ScrollArea::horizontal().id_salt("ribbon-scroll").show(ui, |ui| {
                ui.horizontal_top(|ui| match self.ribbon_tab {
                    Tab::Home => self.home_tab(ui, &mut action, blocked),
                    Tab::Insert => self.insert_tab(ui, &mut action),
                    Tab::Page => self.page_tab(ui),
                    Tab::Design => self.design_tab(ui, &mut action),
                    Tab::View => self.view_tab(ui, &mut action),
                    Tab::Ai => self.ai_tab(ui, &mut action),
                    Tab::Help => {
                        group(ui, i18n::t("Hjelp"), |ui| {
                            column(ui, |ui| {
                                if tool(ui, theme::ph::INFO, i18n::t("Om CVBuilder"), i18n::t("Versjon, bygg og lisenser"), true) {
                                    action = Some(Action::About);
                                }
                                if tool(ui, theme::ph::FILE_CODE, i18n::t("Format for egne design"), i18n::t("Hvordan en .cvdesign.json-fil ser ut"), true) {
                                    action = Some(Action::DesignFormat);
                                }
                            });
                            column(ui, |ui| {
                                if tool(ui, theme::ph::GEAR, i18n::t("Innstillinger"), i18n::t("Språk i menyene, tema og mer"), true) {
                                    action = Some(Action::Settings);
                                }
                            });
                        });
                    }
                });
            });
        });
        if let Some(a) = action {
            self.run_action(ui.ctx(), a);
        }
    }

    fn home_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>, blocked: bool) {
        group(ui, i18n::t("Fil"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::FILE_PLUS, i18n::t("Ny CV"), i18n::t("Start på en tom CV. Den du har nå kan hentes tilbake med Angre."), true) {
                    *action = Some(Action::New);
                }
                if tool(ui, theme::ph::LIGHTBULB, i18n::t("Eksempel"), i18n::t("Et fiktivt eksempel med anbefalt oppsett og rekkefølge"), true) {
                    *action = Some(Action::Example);
                }
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::FOLDER_OPEN, i18n::t("Åpne"), i18n::t("Åpne et lagret CVBuilder-utkast (.json)"), true) {
                    *action = Some(Action::Open);
                }
                if tool(ui, theme::ph::FLOPPY_DISK, i18n::t("Lagre som"), i18n::t("Lagre utkastet som fil"), true) {
                    *action = Some(Action::Save);
                }
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::ARROW_SQUARE_IN, i18n::t("Importer CV"), i18n::t("Hent inn en CV du har fra før: PDF, Word, tekst eller nettside – med eller uten AI"), true) {
                    *action = Some(Action::ImportCv);
                }
                if tool(ui, theme::ph::COPY, i18n::t("Kopier tekst"), &export_tip(Export::Text, blocked), true) {
                    *action = Some(Action::Export(Export::Text));
                }
            });
            // Downloading sits with saving; while something is missing, a click shows what.
            column(ui, |ui| {
                if tool(ui, theme::ph::FILE_PDF, i18n::t("Last ned PDF"), &export_tip(Export::Pdf, blocked), true) {
                    *action = Some(Action::Export(Export::Pdf));
                }
                if tool(ui, theme::ph::FILE_DOC, i18n::t("Last ned Word"), &export_tip(Export::Word, blocked), true) {
                    *action = Some(Action::Export(Export::Word));
                }
            });
        });
        group(ui, i18n::t("Stillingsannonse"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::CLIPBOARD_TEXT, i18n::t("Lim inn annonse"), i18n::t("Gå til feltet for annonseteksten øverst i skjemaet"), true) {
                    *action = Some(Action::PasteAd);
                }
                if tool(ui, theme::ph::LINK, i18n::t("Hent fra lenke"), i18n::t("Hent annonsen fra lenken i skjemaet"), true) {
                    *action = Some(Action::AdLink);
                }
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::FILE_ARROW_UP, i18n::t("Fra fil"), i18n::t("Les annonsen fra PDF, Word, tekst eller en lagret nettside"), true) {
                    *action = Some(Action::AdFile);
                }
            });
        });
        group(ui, i18n::t("Angre"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::UNDO, i18n::t("Angre"), i18n::t("Hent tilbake CV-en fra før siste store endring"), self.undo.is_some()) {
                    *action = Some(Action::Undo);
                }
            });
        });
        group(ui, i18n::t("Språk"), |ui| {
            column(ui, |ui| {
                // The CV's language: its headings and dates. Every listed language works offline;
                // «Annet språk» opens the translate window for any other.
                let current = match &self.project.options.headings {
                    Some(h) => h.language.clone(),
                    None if self.project.is_english() => "English".to_string(),
                    None => "Bokmål".to_string(),
                };
                egui::ComboBox::from_id_salt("cv-language")
                    .selected_text(current.clone())
                    .width(150.0)
                    .height(460.0)
                    .show_ui(ui, |ui| {
                        for name in ["Bokmål", "English"].into_iter().chain(cv_core::headings::built_in_languages()) {
                            let tip = match name {
                                "Bokmål" => i18n::t("Overskrifter og datoer på bokmål"),
                                "English" => i18n::t("Overskrifter og datoer på engelsk"),
                                _ => i18n::t("Overskriftene byttes med en gang; teksten oversetter du med «Oversett»"),
                            };
                            if ui.selectable_label(current == name, name).on_hover_text(tip).clicked() && current != name {
                                *action = Some(Action::Headings(name.to_string()));
                            }
                        }
                        ui.separator();
                        if ui
                            .add(egui::Button::new(i18n::t("Annet språk …")).frame(false))
                            .on_hover_text(i18n::t("Et hvilket som helst språk: skriv det inn, så lager AI-en eller du overskriftene"))
                            .clicked()
                        {
                            *action = Some(Action::Translate);
                        }
                    })
                    .response
                    .on_hover_text(i18n::t("Språket i CV-en: overskrifter og datoer"));
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::TRANSLATE, i18n::t("Oversett"), i18n::t("Oversett hele CV-en, med AI eller selv"), true) {
                    *action = Some(Action::Translate);
                }
            });
        });
    }

    fn insert_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let english_cv = self.project.is_english();
        group(ui, i18n::t("Erfaring"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::BRIEFCASE, i18n::t("+ Jobb").trim_start_matches(['+', ' ']), i18n::t("Ny jobb i arbeidserfaringen"), true) {
                    *action = Some(Action::Add("job"));
                }
                if tool(ui, theme::ph::GRADUATION_CAP, i18n::t("+ Utdanning").trim_start_matches(['+', ' ']), i18n::t("Ny utdanning: grad, fagbrev eller studium"), true) {
                    *action = Some(Action::Add("education"));
                }
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::CERTIFICATE, i18n::t("+ Kurs").trim_start_matches(['+', ' ']), i18n::t("Nytt kurs eller ny sertifisering. Sertifiseringer i annonsen er ofte noe ATS ser etter."), true) {
                    *action = Some(Action::Add("certification"));
                }
                tool_menu(ui, theme::ph::LIST_PLUS, i18n::t("+ Seksjon").trim_start_matches(['+', ' ']), i18n::t("Prosjekter, frivillig arbeid, verv, publikasjoner og andre seksjoner"), |ui| {
                    ui.label(RichText::new(i18n::t("Legg til en seksjon. Du kan endre navnet og flytte den i skjemaet.")).size(12.0).weak());
                    for (no, en) in crate::page_ui::SECTION_TYPES {
                        let name = if english_cv { en } else { no };
                        if ui.button(name).on_hover_text(i18n::tf("Ny seksjon «{name}». Du kan endre navnet før den legges til.", &[("name", name)])).clicked() {
                            *action = Some(Action::AddSection(name));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button(i18n::t("Egen seksjon med eget navn")).on_hover_text(i18n::t("En seksjon med et navn du velger selv")).clicked() {
                        *action = Some(Action::Add("section"));
                        ui.close();
                    }
                });
            });
        });
        group(ui, i18n::t("Kompetanse"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::STAR, i18n::t("+ Kompetanse").trim_start_matches(['+', ' ']), i18n::t("Ny kompetanse eller nytt verktøy. Bruk annonsens ord – ATS ser etter dem."), true) {
                    *action = Some(Action::Add("skill"));
                }
                if tool(ui, theme::ph::CHAT_TEXT, i18n::t("+ Språk").trim_start_matches(['+', ' ']), i18n::t("Nytt språk med nivå, f.eks. engelsk – flytende"), true) {
                    *action = Some(Action::Add("language"));
                }
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::HEART, i18n::t("+ Interesse").trim_start_matches(['+', ' ']), i18n::t("Ny interesse. Høyst tre kommer med i CV-en."), true) {
                    *action = Some(Action::Add("interest"));
                }
            });
        });
        group(ui, i18n::t("Kontakt"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::ADDRESS_BOOK, i18n::t("+ Kontaktfelt").trim_start_matches(['+', ' ']), i18n::t("GitHub, portefølje, nettside, førerkort …"), true) {
                    *action = Some(Action::Add("contact"));
                }
                if tool(ui, theme::ph::USER_CIRCLE, if self.project.candidate.photo.is_empty() { i18n::t("Bilde") } else { i18n::t("Bytt bilde") }, i18n::t("Valgfritt portrett"), true) {
                    *action = Some(Action::Photo);
                }
            });
        });
    }

    fn design_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        group(ui, i18n::t("Design"), |ui| {
            ui.vertical(|ui| self.design_picker(ui));
        });
        group(ui, i18n::t("Farger"), |ui| {
            ui.vertical(|ui| self.color_menu(ui));
        });
        group(ui, i18n::t("Egne design"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::UPLOAD, i18n::t("Importer design"), i18n::t("Fra en .cvdesign.json-fil"), true) {
                    *action = Some(Action::Import);
                }
                if tool(ui, theme::ph::FLOPPY_DISK, i18n::t("Lagre som mal-fil"), i18n::t("Dette designet som fil du kan endre"), true) {
                    *action = Some(Action::SaveDesign);
                }
            });
        });
    }

    fn view_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        group(ui, i18n::t("Visninger"), |ui| {
            column(ui, |ui| {
                // Two rows, like the other tabs: each view is a toggle with its icon.
                let t = Tokens::get(ui.ctx());
                let icon = |i: &str| RichText::new(i).family(theme::icons()).size(17.0).color(t.accent);
                let text = |s: &str| RichText::new(s).size(13.0);
                ui.add(egui::Checkbox::new(&mut self.views.form, (icon(theme::ph::NOTEPAD), text(i18n::t("Skjema"))))).on_hover_text(i18n::t("Vis eller skjul skjemaet der du skriver CV-en"));
                ui.add(egui::Checkbox::new(&mut self.views.preview, (icon(theme::ph::EYE), text(i18n::t("Forhåndsvisning"))))).on_hover_text(i18n::t("Vis eller skjul sidene slik de blir i PDF-en"));
            });
            column(ui, |ui| {
                let t = Tokens::get(ui.ctx());
                let icon = |i: &str| RichText::new(i).family(theme::icons()).size(17.0).color(t.accent);
                ui.add(egui::Checkbox::new(&mut self.views.side, (icon(theme::ph::SIDEBAR), RichText::new(i18n::t("AI og sjekkliste")).size(13.0)))).on_hover_text(i18n::t("Vis eller skjul panelet med AI-assistenten og sjekklisten"));
                if tool(ui, theme::ph::SQUARES_FOUR, i18n::t("Vis alt"), i18n::t("Vis skjema, forhåndsvisning og AI-panelet igjen"), true) {
                    *action = Some(Action::ShowAll);
                }
            });
        });
        group(ui, "Zoom", |ui| {
            column(ui, |ui| {
                let zooms = [(0.0, i18n::t("Tilpass")), (0.75, "75 %"), (1.0, "100 %"), (1.5, "150 %")];
                let current = zooms.iter().find(|(z, _)| (self.views.zoom - z).abs() < 0.01).map(|(_, n)| *n).unwrap_or("100 %");
                tool_menu(ui, theme::ph::ZOOM, current, i18n::t("Sidene fyller bredden av forhåndsvisningen"), |ui| {
                    for (z, name) in zooms {
                        let on = (self.views.zoom - z).abs() < 0.01;
                        let tip = if z < 0.01 { i18n::t("Sidene fyller bredden av forhåndsvisningen").to_string() } else { i18n::tf("Siden i {zoom} av virkelig størrelse", &[("zoom", name)]) };
                        if ui.radio(on, name).on_hover_text(tip).clicked() {
                            self.views.zoom = z;
                            ui.close();
                        }
                    }
                });
            });
        });
    }

    fn ai_tab(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        group(ui, i18n::t("Tilkobling"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::PLUGS_CONNECTED, i18n::t("Koble til AI"), i18n::t("Claude, ChatGPT, Gemini, Grok eller en lokal modell"), true) {
                    *action = Some(Action::ConnectAi);
                }
                if tool(ui, theme::ph::PLUG, i18n::t("MCP for andre apper"), i18n::t("Bruk CVBuilder fra Claude Code, Claude Desktop m.fl."), true) {
                    *action = Some(Action::Mcp);
                }
            });
        });
        group(ui, i18n::t("Assistent"), |ui| {
            column(ui, |ui| {
                if tool(ui, theme::ph::CHAT, i18n::t("Åpne chatten"), i18n::t("Be AI-assistenten om endringer i CV-en med egne ord"), true) {
                    *action = Some(Action::Chat);
                }
                if tool(
                    ui,
                    theme::ph::CHAT_DOTS,
                    i18n::t("Ny samtale"),
                    if self.chat.running { i18n::t("Vent til AI-en er ferdig, eller stopp den i chatten.") } else { i18n::t("Start en ny samtale. CV-en endres ikke.") },
                    !self.chat.running,
                ) {
                    *action = Some(Action::NewChat);
                }
            });
            column(ui, |ui| {
                if tool(ui, theme::ph::LIST_CHECKS, i18n::t("Sjekkliste"), i18n::t("Rekrutterersjekk og hva som mangler"), true) {
                    *action = Some(Action::Checks);
                }
            });
        });
    }

    fn run_action(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::New => {
                let mut blank = Project::default();
                blank.options.layout = self.project.options.layout.clone();
                blank.options.palette = self.project.options.palette.clone();
                let text =
                    if self.is_example { i18n::t("Tom CV klar. Start med kontaktinformasjonen, eller be AI-en om hjelp.") } else { i18n::t("Tom CV klar. Den forrige kan hentes tilbake med Angre.") };
                self.reference = None;
                self.replace(blank, false, text);
            }
            Action::Example => {
                self.reference = None;
                self.replace(Project::example(), true, i18n::t("Du ser eksemplet. CV-en din kan hentes tilbake med Angre."));
            }
            Action::Open => self.pick("open", None),
            Action::Save => self.pick("save", None),
            Action::Export(kind) => self.start_export(ctx, kind),
            Action::Undo => {
                if let Some((project, example)) = self.undo.take() {
                    self.load(project, example);
                    self.status = i18n::t("Angret.").into();
                }
            }
            Action::Headings(name) => self.switch_cv_language(&name),
            Action::Translate => self.open_translate(None),
            Action::Add("section") => self.ask_section_name(""),
            Action::Add(what) => self.insert(what),
            Action::AddSection(title) => self.ask_section_name(title),
            Action::Photo => self.pick("photo", None),
            Action::ImportCv => {
                self.import.cv_open = true;
                self.import.use_ai = self.chat_backend().is_some();
            }
            Action::PasteAd => {
                self.views.form = true;
                self.folded.remove("target");
                self.import.focus_ad = true;
            }
            Action::AdLink => {
                self.views.form = true;
                self.folded.remove("target");
                self.fetch_ad();
            }
            Action::AdFile => self.pick("ad_file", None),
            Action::Import => {
                self.designs.import_open = true;
                self.designs.message.clear();
            }
            Action::SaveDesign => self.pick("design_save", None),
            Action::ConnectAi => {
                self.ai.connect_open = true;
                self.ai.choice = self.settings.ai_service.or(Some(cv_ai::Service::Claude));
            }
            Action::Mcp => self.chat.mcp_help_open = true,
            Action::NewChat => self.chat.clear(),
            Action::Chat => {
                self.views.side = true;
                self.side = Side::Chat;
            }
            Action::Checks => {
                self.views.side = true;
                self.side = Side::Checks;
            }
            Action::About => self.about_open = true,
            Action::DesignFormat => {
                self.designs.import_open = true;
                self.designs.message.clear();
            }
            Action::ShowAll => self.views = Views { zoom: self.views.zoom, ..Views::default() },
            Action::Settings => self.settings_open = true,
        }
    }

    /// «Sett inn»: adds an empty entry and opens its section in the form.
    fn insert(&mut self, what: &str) {
        let section = match what {
            "job" => {
                let id = self.next_id("job");
                self.project.experience.push(Job { id, end: Some(String::new()), confirmed: true, ..Default::default() });
                "experience".to_string()
            }
            "education" => {
                self.project.education.push(Education::default());
                "education".into()
            }
            "certification" => {
                self.project.certifications.push(Certification::default());
                "certifications".into()
            }
            "skill" => {
                let group = self.project.skills.last().map(|s| s.group.clone()).unwrap_or_default();
                self.project.skills.push(Skill { group, name: String::new(), evidence: String::new(), confirmed: true });
                "skills".into()
            }
            "language" => {
                self.project.languages.push(SpokenLanguage::default());
                "languages".into()
            }
            "interest" => {
                let id = self.next_id("interest");
                self.project.interests.push(Interest { id, confirmed: true, category: "other".into(), specificity: 1, ..Default::default() });
                "interests".into()
            }
            "section" => {
                let id = self.next_id("section");
                self.project.custom_sections.push(CustomSection { id: id.clone(), title: "Prosjekter".into(), items: vec![CustomItem::default()] });
                id
            }
            _ => {
                let id = self.next_id("contact");
                self.project.candidate.extra.push(ContactField { id, label: String::new(), value: String::new() });
                "contact".into()
            }
        };
        self.folded.remove(&section);
        self.views.form = true;
        self.status = i18n::t("Lagt til. Fyll ut feltene i skjemaet.").into();
        self.touched();
    }

    /// «Om CVBuilder»: name, version and the exact build.
    pub(crate) fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let t = Tokens::get(ctx);
        let mut close = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.about")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.horizontal(|ui| {
                crate::logo(ui, &t, 44.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("CVBuilder").size(22.0).family(theme::bold()));
                    ui.label(RichText::new(i18n::t("CV-er som rekrutterere og søknadssystemer leser riktig")).color(t.dim));
                });
            });
            ui.add_space(10.0);
            egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("about-grid").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new(i18n::t("Versjon")).color(t.dim));
                    ui.label(RichText::new(VERSION).monospace());
                    ui.end_row();
                    ui.label(RichText::new(i18n::t("Bygg (git)")).color(t.dim));
                    ui.label(RichText::new(BUILD).monospace());
                    ui.end_row();
                    ui.label(RichText::new("System").color(t.dim));
                    ui.label(RichText::new(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)).monospace());
                    ui.end_row();
                    ui.label(RichText::new("Data").color(t.dim));
                    ui.label(RichText::new(if crate::storage::portable() { i18n::t("Bærbar modus (ved siden av programmet)") } else { i18n::t("Brukerens egen datamappe") }).size(12.5));
                    ui.end_row();
                });
            });
            ui.add_space(8.0);
            theme::hint(ui, i18n::t("Lisens: MIT eller Apache-2.0. Bygger på WordCraft og Craft AI-integrasjonene (ArtCraft/storytold). Design fra Reactive Resume (MIT, © Amruth Pillai) og RenderCV (MIT, © Sina Atalay og bidragsyterne) er laget om til én spalte. Skrift: DM Sans (SIL OFL). Full liste i THIRD-PARTY.txt."));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(i18n::t("Kopier versjonsinfo")).on_hover_text(i18n::t("Kopier versjon, bygg og system, f.eks. til en feilmelding")).clicked() {
                    ui.ctx().copy_text(format!("CVBuilder {VERSION} ({BUILD}) {} {}", std::env::consts::OS, std::env::consts::ARCH));
                }
                if theme::primary(ui, i18n::t("Lukk"), i18n::t("Lukk vinduet (Esc)"), true).clicked() {
                    close = true;
                }
            });
        });
        if close || r.should_close() {
            self.about_open = false;
        }
    }
}
