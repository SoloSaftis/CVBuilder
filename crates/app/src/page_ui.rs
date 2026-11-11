//! «Sideoppsett» (page layout), like Word's Layout tab: margins, size and columns; page breaks
//! and the page target; text size and spacing. Everything is saved with the CV.
use cv_core::model::{ColumnsChoice, Margins, PageSetup, PageSize};
use cv_core::text as cvtext;
use egui::RichText;

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

/// Section types people add most, with their headings in Norwegian and English.
pub const SECTION_TYPES: [(&str, &str); 10] = [
    ("Prosjekter", "Projects"),
    ("Frivillig arbeid", "Volunteering"),
    ("Verv og styreverv", "Positions of trust"),
    ("Publikasjoner", "Publications"),
    ("Priser og utmerkelser", "Awards"),
    ("Foredrag og presentasjoner", "Talks and presentations"),
    ("Referanser", "References"),
    ("Militærtjeneste", "Military service"),
    ("Patenter", "Patents"),
    ("Annen erfaring", "Other experience"),
];

fn margins_label(m: Margins) -> &'static str {
    match m {
        Margins::Normal => i18n::t("Normal · 17 mm"),
        Margins::Narrow => i18n::t("Smal · 12,7 mm"),
        Margins::Moderate => i18n::t("Moderat · 19 / 25 mm"),
        Margins::Wide => i18n::t("Bred · 25,4 mm"),
        Margins::Custom => i18n::t("Egendefinert"),
    }
}

fn size_label(s: PageSize) -> &'static str {
    match s {
        PageSize::A4 => "A4 · 210 × 297 mm",
        PageSize::Letter => i18n::t("US Letter · 8,5 × 11 in"),
        PageSize::Legal => i18n::t("US Legal · 8,5 × 14 in"),
    }
}

impl CvApp {
    /// The «Sideoppsett» tab's groups. Returns true when the page setup changed.
    pub(crate) fn page_tab(&mut self, ui: &mut egui::Ui) {
        let before = self.project.options.page.clone();
        let t = Tokens::get(ui.ctx());
        let page = &mut self.project.options.page;
        crate::menu_ui::group(ui, i18n::t("Sideoppsett"), |ui| {
            crate::menu_ui::column(ui, |ui| {
                crate::menu_ui::tool_menu(ui, theme::ph::FRAME_CORNERS, &i18n::tf("Marger: {m}", &[("m", margins_label(page.margins).split(" · ").next().unwrap_or(""))]), i18n::t("Luften rundt teksten på siden. Normal passer de fleste."), |ui| {
                    for (m, tip) in [
                        (Margins::Normal, i18n::t("Passer de fleste CV-er")),
                        (Margins::Narrow, i18n::t("Mer tekst per side, men siden blir tettere")),
                        (Margins::Moderate, i18n::t("Litt mer luft oppe og nede")),
                        (Margins::Wide, i18n::t("Mest luft – fint for en kort CV")),
                    ] {
                        if ui.radio(page.margins == m, margins_label(m)).on_hover_text(tip).clicked() {
                            page.margins = m;
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.radio(page.margins == Margins::Custom, i18n::t("Egendefinerte marger")).on_hover_text(i18n::t("Velg hver marg selv, i millimeter")).clicked() {
                        page.margins = Margins::Custom;
                    }
                    if page.margins == Margins::Custom {
                        for (i, name) in [i18n::t("Topp"), i18n::t("Høyre"), i18n::t("Bunn"), i18n::t("Venstre")].iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_sized([70.0, 20.0], egui::Label::new(*name));
                                ui.add(egui::DragValue::new(&mut page.custom_mm[i]).range(5.0..=40.0).speed(0.5).suffix(" mm")).on_hover_text(i18n::t("Dra eller skriv inn margen, 5–40 mm"));
                            });
                        }
                    }
                });
                crate::menu_ui::tool_menu(ui, theme::ph::FILE, &i18n::tf("Størrelse: {s}", &[("s", size_label(page.size).split(" · ").next().unwrap_or(""))]), i18n::t("Papirstørrelsen: A4 eller amerikansk Letter og Legal"), |ui| {
                    for (s, tip) in [
                        (PageSize::A4, i18n::t("Vanlig i Norge og Europa")),
                        (PageSize::Letter, i18n::t("Vanlig i USA og Canada")),
                        (PageSize::Legal, i18n::t("Lengre amerikansk format, sjelden brukt til CV")),
                    ] {
                        if ui.radio(page.size == s, size_label(s)).on_hover_text(tip).clicked() {
                            page.size = s;
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new(i18n::t("A4 brukes i Norge og Europa, Letter i USA og Canada.")).size(12.0).color(t.faint));
                });
            });
            crate::menu_ui::column(ui, |ui| {
                let label = match page.columns {
                    None => i18n::t("Spalter: designets"),
                    Some(ColumnsChoice::One) => i18n::t("Spalter: én"),
                    Some(ColumnsChoice::Left) => i18n::t("Spalter: sidefelt venstre"),
                    Some(ColumnsChoice::Right) => i18n::t("Spalter: sidefelt høyre"),
                };
                crate::menu_ui::tool_menu(ui, theme::ph::COLUMNS, &label, i18n::t("Én spalte leses best av ATS. To spalter gir et sidefelt."), |ui| {
                    let two = i18n::t("Et sidefelt med kontakt og kompetanse. Eldre ATS kan lese to spalter i feil rekkefølge.");
                    for (choice, name, tip) in [
                        (None, i18n::t("Som designet"), i18n::t("Bruk spaltene designet har")),
                        (Some(ColumnsChoice::One), i18n::t("Én spalte (best for ATS)"), i18n::t("Leses riktig av alle søknadssystemer – anbefalt")),
                        (Some(ColumnsChoice::Left), i18n::t("To spalter, sidefelt til venstre"), two),
                        (Some(ColumnsChoice::Right), i18n::t("To spalter, sidefelt til høyre"), two),
                    ] {
                        if ui.radio(page.columns == choice, name).on_hover_text(tip).clicked() {
                            page.columns = choice;
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.label(RichText::new(i18n::t("Sidefeltet får kontakt, kompetanse, språk, kurs og interesser.\nTo spalter leses dårligere av eldre søknadssystemer.")).size(12.0).color(t.faint));
                });
            });
        });
        let titles: Vec<(String, String)> = cvtext::section_order(&self.project)
            .into_iter()
            .skip(1)
            .map(|k| {
                let t = cvtext::section_title(&self.project, &k);
                (k.clone(), if t.is_empty() { k } else { t })
            })
            .collect();
        let advice = self.recommended_pages();
        let pages = self.preview.page_count;
        let page = &mut self.project.options.page;
        crate::menu_ui::group(ui, i18n::t("Sider"), |ui| {
            crate::menu_ui::column(ui, |ui| {
                let n = page.page_breaks.len() + page.page_breaks_after.len();
                crate::menu_ui::tool_menu(ui, theme::ph::ROWS, &if n == 0 { i18n::t("Ny side …").trim_end_matches(['…', ' ']).to_string() } else { i18n::tf("Ny side … ({n})", &[("n", &n.to_string())]).replace(" …", "") }, i18n::t("Start en seksjon på en ny side, før eller etter den, så ingen deles stygt"), |ui| {
                    ui.label(RichText::new(i18n::t("Velg seksjoner som skal starte en ny side (før), eller som skal avsluttes med et sideskift (etter).")).size(12.0).color(t.faint));
                    egui::Grid::new("page-breaks").num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
                        ui.label(RichText::new(i18n::t("Seksjon")).size(12.0).color(t.dim));
                        ui.label(RichText::new(i18n::t("Før")).size(12.0).color(t.dim));
                        ui.label(RichText::new(i18n::t("Etter")).size(12.0).color(t.dim));
                        ui.end_row();
                        for (key, title) in &titles {
                            ui.label(title.as_str());
                            let mut before = page.page_breaks.contains(key);
                            if ui.checkbox(&mut before, "").on_hover_text(i18n::tf("«{title}» starter øverst på en ny side", &[("title", title.as_str())])).changed() {
                                page.page_breaks.retain(|k| k != key);
                                if before {
                                    page.page_breaks.push(key.clone());
                                }
                            }
                            let mut after = page.page_breaks_after.contains(key);
                            if ui.checkbox(&mut after, "").on_hover_text(i18n::tf("Det som kommer etter «{title}», starter på en ny side", &[("title", title.as_str())])).changed() {
                                page.page_breaks_after.retain(|k| k != key);
                                if after {
                                    page.page_breaks_after.push(key.clone());
                                }
                            }
                            ui.end_row();
                        }
                    });
                    if n > 0 && ui.button(i18n::t("Fjern alle sideskift")).on_hover_text(i18n::t("La seksjonene flyte fritt over sidene igjen")).clicked() {
                        page.page_breaks.clear();
                        page.page_breaks_after.clear();
                    }
                });
            });
            crate::menu_ui::column(ui, |ui| {
                let color = if pages > advice + 1 {
                    t.danger
                } else if pages > advice {
                    t.warning
                } else {
                    t.dim
                };
                let (p, a) = (pages.to_string(), advice.to_string());
                let now = if pages == 1 {
                    i18n::tf("{pages} side · anbefalt {advice}", &[("pages", &p), ("advice", &a)])
                } else {
                    i18n::tf("{pages} sider · anbefalt {advice}", &[("pages", &p), ("advice", &a)])
                };
                ui.label(RichText::new(now).size(13.0).color(color))
                    .on_hover_text(i18n::t("Det er ingen grense, men rekrutterere bruker sekunder på hver CV: én side for kort erfaring, to sider for de fleste."));
                ui.label(RichText::new(if advice == 1 { i18n::t("Kort erfaring: én side holder.") } else { i18n::t("To sider for de fleste. Ingen grense.") }).size(12.0).color(t.faint));
            });
        });
        let page = &mut self.project.options.page;
        crate::menu_ui::group(ui, i18n::t("Tekst og avstand"), |ui| {
            crate::menu_ui::column(ui, |ui| {
                // Text size: icon, −, the size, +; all the same height as the other tools.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add_sized([26.0, 26.0], egui::Label::new(RichText::new(theme::ph::TEXT_AA).family(theme::icons()).size(16.0).color(t.accent)))
                        .on_hover_text(i18n::t("Tekst"));
                    if theme::flat_icon(ui, theme::ph::MINUS, page.text_delta > -1.5)
                        .on_hover_text(i18n::t("Mindre tekst"))
                        .on_disabled_hover_text(i18n::t("Teksten er så liten den kan bli"))
                        .clicked()
                    {
                        page.text_delta = (page.text_delta - 0.5).max(-1.5);
                    }
                    let size = if page.text_delta == 0.0 { i18n::t("normal").to_string() } else { format!("{:+.1} pt", page.text_delta) };
                    ui.add_sized([56.0, 26.0], egui::Label::new(RichText::new(size).size(13.0)));
                    if theme::flat_icon(ui, theme::ph::PLUS, page.text_delta < 2.0).on_hover_text(i18n::t("Større tekst")).on_disabled_hover_text(i18n::t("Teksten er så stor den kan bli")).clicked() {
                        page.text_delta = (page.text_delta + 0.5).min(2.0);
                    }
                });
                let spacings = [
                    (0.85, i18n::t("Tett"), i18n::t("Mindre luft mellom linjer og deler – får plass til mer")),
                    (1.0, i18n::t("Normal"), i18n::t("Vanlig avstand mellom linjer og deler")),
                    (1.15, i18n::t("Luftig"), i18n::t("Mer luft – lettere å lese når CV-en er kort")),
                ];
                let current = spacings.iter().find(|(v, _, _)| (page.spacing - v).abs() < 0.01).map(|(_, n, _)| *n).unwrap_or(i18n::t("Normal"));
                crate::menu_ui::tool_menu(ui, theme::ph::ARROWS_VERTICAL, &format!("{}: {current}", i18n::t("Avstand")), i18n::t("Vanlig avstand mellom linjer og deler"), |ui| {
                    for (v, name, tip) in spacings {
                        if ui.radio((page.spacing - v).abs() < 0.01, name).on_hover_text(tip).clicked() {
                            page.spacing = v;
                            ui.close();
                        }
                    }
                });
            });
            crate::menu_ui::column(ui, |ui| {
                if !page.is_default() && theme::ghost(ui, i18n::t("Tilbakestill"), i18n::t("Tilbake til standard sideoppsett")).clicked() {
                    *page = PageSetup::default();
                }
            });
        });
        if self.project.options.page != before {
            self.preview.changed();
            self.save_at = Some(std::time::Instant::now());
        }
    }
}
