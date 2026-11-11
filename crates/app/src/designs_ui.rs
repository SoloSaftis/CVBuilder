//! Choosing a design (a drop-down list grouped by origin) and importing one's own
//! (`.cvdesign.json`, the format shown in the import window). Imported designs are kept in the
//! data folder's `designs/` and loaded at start.
use cv_core::layouts::{self, Design};
use egui::{CornerRadius, Margin, RichText, Stroke};

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

#[derive(Default)]
pub struct DesignState {
    pub import_open: bool,
    pub message: String,
    pub message_error: bool,
}

fn designs_dir() -> std::path::PathBuf {
    crate::storage::data_dir().join("designs")
}

/// Loads the person's imported designs. Files that no longer pass the checks are skipped.
pub fn load_imported() {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(designs_dir()) {
        for e in entries.flatten() {
            if e.path().to_string_lossy().ends_with(".cvdesign.json") {
                if let Ok(d) = std::fs::read_to_string(e.path()).map_err(|e| e.to_string()).and_then(|t| layouts::parse(&t)) {
                    found.push(d);
                }
            }
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    layouts::set_imported(found);
}

fn group(d: &Design) -> String {
    if d.imported { i18n::t("Dine designs").to_string() } else { i18n::tdyn(layouts::category_label(&d.category)) }
}

/// The designs in list order: by kind of job, one column before two, then the person's own.
fn ordered(all: Vec<Design>) -> Vec<Design> {
    let rank = |d: &Design| {
        if d.imported { usize::MAX } else { layouts::CATEGORIES.iter().position(|(c, _)| *c == d.category).unwrap_or(layouts::CATEGORIES.len()) }
    };
    let mut out = all;
    out.sort_by_key(|d| (rank(d), d.columns.is_some()));
    out
}

fn swatch(ui: &mut egui::Ui, d: &Design) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let c = layouts::rgb(&d.accent);
    let fill = d.header_fill.as_deref().map(layouts::rgb).unwrap_or(c);
    ui.painter().rect_filled(rect, CornerRadius::same(4), egui::Color32::from_rgb(fill.0, fill.1, fill.2));
    let line = egui::Rect::from_min_size(rect.left_bottom() - egui::vec2(0.0, 5.0), egui::vec2(18.0, 5.0));
    ui.painter().rect_filled(line, CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, egui::Color32::from_rgb(c.0, c.1, c.2));
}

impl CvApp {
    /// The design drop-down in the top bar.
    pub(crate) fn design_picker(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let all = ordered(layouts::all());
        let current = layouts::design(&self.project.options.layout);
        let mut chosen: Option<String> = None;
        let mut import = false;
        let mut save_template = false;
        ui.horizontal(|ui| {
            swatch(ui, &current);
            egui::ComboBox::from_id_salt("design")
                .selected_text(RichText::new(i18n::tf("Design: {name}", &[("name", &current.name)])).size(13.5).family(theme::medium()))
                .width(210.0)
                .height(520.0)
                .show_ui(ui, |ui| {
                    ui.set_min_width(380.0);
                    let mut last = String::new();
                    for d in &all {
                        let g = group(d);
                        if g != last {
                            ui.add_space(if last.is_empty() { 2.0 } else { 8.0 });
                            ui.label(RichText::new(&g).size(11.5).family(theme::medium()).color(t.faint));
                            last = g;
                        }
                        let on = d.id == current.id;
                        let r = ui
                            .horizontal(|ui| {
                                swatch(ui, d);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&d.name).family(theme::medium()).color(if on { t.accent } else { t.text }));
                                        if d.columns.is_some() {
                                            theme::pill(ui, i18n::t("To spalter"), t.warning.gamma_multiply(0.14), t.warning);
                                        }
                                    });
                                    ui.add(egui::Label::new(RichText::new(i18n::tdyn(&d.description)).size(11.5).color(t.dim)).wrap());
                                });
                            })
                            .response;
                        let r = ui.interact(r.rect, ui.scope_id().with(("design", &d.id)), egui::Sense::click());
                        if r.hovered() {
                            ui.painter().rect_stroke(r.rect.expand(2.0), CornerRadius::same(theme::R_SM), Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
                        }
                        let tip = [i18n::tdyn(&d.credit), i18n::tdyn(d.ats_warning().unwrap_or(""))].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n");
                        let tip = if tip.is_empty() { i18n::t("Bytter bare utseendet. Teksten i CV-en er den samme.").to_string() } else { tip };
                        let r = r.on_hover_text(tip);
                        if r.clicked() {
                            chosen = Some(d.id.clone());
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.selectable_label(false, i18n::t("Importer eget design …")).on_hover_text(i18n::t("Fra en .cvdesign.json-fil")).clicked() {
                        import = true;
                    }
                    if ui.selectable_label(false, i18n::t("Lagre dette designet som mal-fil …")).on_hover_text(i18n::t("Dette designet som fil du kan endre")).clicked() {
                        save_template = true;
                    }
                })
                .response
                .on_hover_text(i18n::t("Utseendet på CV-en: skrift, farger og overskrifter. Teksten er lik i alle design."));
        });
        if let Some(id) = chosen {
            if id != self.project.options.layout {
                self.project.options.layout = id;
                self.preview.changed();
                self.save_at = Some(std::time::Instant::now());
            }
        }
        if import {
            self.designs.import_open = true;
            self.designs.message.clear();
        }
        if save_template {
            self.pick("design_save", None);
        }
    }

    /// Reads a picked design file and keeps a copy in the data folder.
    pub(crate) fn import_design(&mut self, path: &std::path::Path) {
        let result = std::fs::read_to_string(path).map_err(|e| i18n::tf("Filen kunne ikke leses: {e}", &[("e", &e.to_string())])).and_then(|t| layouts::parse(&t));
        match result {
            Ok(d) => {
                let dir = designs_dir();
                let saved = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(format!("{}.cvdesign.json", d.id)), layouts::to_file(&d)));
                match saved {
                    Ok(()) => {
                        load_imported();
                        self.project.options.layout = d.id.clone();
                        self.preview.changed();
                        self.save_at = Some(std::time::Instant::now());
                        self.designs.message = i18n::tf("«{name}» er importert og valgt. Den ligger under «Dine designs».", &[("name", &d.name)]);
                        self.designs.message_error = false;
                    }
                    Err(e) => {
                        self.designs.message = i18n::tf("Designet er gyldig, men kunne ikke lagres: {e}", &[("e", &e.to_string())]);
                        self.designs.message_error = true;
                    }
                }
            }
            Err(e) => {
                let problems = e.lines().map(i18n::tdyn).collect::<Vec<_>>().join("\n");
                self.designs.message = i18n::tf("Kunne ikke importere «{file}»:\n{e}", &[("file", &path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), ("e", &problems)]);
                self.designs.message_error = true;
            }
        }
    }

    /// Saves the current design as a file to edit and import again.
    pub(crate) fn save_design_template(&mut self, path: &std::path::Path) {
        let mut d = layouts::design(&self.project.options.layout);
        if !d.imported {
            d.id = format!("{}-min", d.id).chars().take(40).collect();
            d.name = format!("{} (min versjon)", d.name);
        }
        self.status = match std::fs::write(path, layouts::to_file(&d)) {
            Ok(()) => i18n::tf("Mal-filen er lagret: {path}. Endre den og importer den igjen.", &[("path", &path.display().to_string())]),
            Err(e) => i18n::tf("Kunne ikke lagre mal-filen: {e}", &[("e", &e.to_string())]),
        };
    }

    /// The import window: what the file must look like, and the button to pick one.
    pub(crate) fn import_dialog(&mut self, ctx: &egui::Context) {
        if !self.designs.import_open {
            return;
        }
        let t = Tokens::get(ctx);
        let mut close = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.import-design")).show(ctx, |ui| {
            ui.set_max_width(620.0);
            ui.heading(i18n::t("Importer eget design"));
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(i18n::t(
                    "Et design bestemmer skrift, størrelser, farger, toppfelt og overskrifter. Teksten og den ene spalten er lik i alle design, så CV-en leses like godt av søknadssystemer (ATS).",
                ))
                .wrap(),
            );
            ui.add_space(6.0);
            egui::Frame::new().fill(t.accent_soft).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(i18n::t("Filformat: .cvdesign.json")).family(theme::bold()));
                ui.add(
                    egui::Label::new(i18n::t("En JSON-fil (UTF-8) med feltene under. «format» må være «cvbuilder-design/1». Lag en ved å lagre et design du liker som mal-fil og endre den.")).wrap(),
                );
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for (field, what) in fields() {
                    ui.horizontal_top(|ui| {
                        ui.add_sized([150.0, 16.0], egui::Label::new(RichText::new(field).monospace().size(12.0)));
                        ui.add(egui::Label::new(RichText::new(what).size(12.5).color(t.dim)).wrap());
                    });
                }
                ui.add_space(4.0);
                theme::hint(
                    ui,
                    i18n::t(
                        "Ikke mulig, med vilje: to spalter, sidefelt, tabeller, tekstbokser, ikoner, ferdighetsstolper og tekst i topp- eller bunntekst. Det gjør CV-er vanskeligere å lese for ATS.",
                    ),
                );
            });
            ui.add_space(8.0);
            if !self.designs.message.is_empty() {
                ui.add(egui::Label::new(RichText::new(&self.designs.message).color(if self.designs.message_error { t.danger } else { t.success })).wrap());
                ui.add_space(6.0);
            }
            ui.horizontal(|ui| {
                if theme::primary(ui, i18n::t("Velg fil …"), i18n::t("Velg en .cvdesign.json-fil å importere"), true).clicked() {
                    self.pick("design_import", None);
                }
                if ui.button(i18n::t("Lagre mal-fil å starte fra …")).on_hover_text(i18n::t("Dette designet som fil du kan endre")).clicked() {
                    self.pick("design_save", None);
                }
                if ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk vinduet (Esc)")).clicked() {
                    close = true;
                }
            });
        });
        if close || r.should_close() {
            self.designs.import_open = false;
        }
    }
}

/// The fields of a `.cvdesign.json` file and what each one holds.
fn fields() -> [(&'static str, &'static str); 15] {
    [
        ("format", "«cvbuilder-design/1»"),
        ("id", i18n::t("2–40 tegn: a–z, 0–9 og bindestrek. Ikke det samme som et innebygd design.")),
        ("name, description", i18n::t("Navn og én setning, vises i listen.")),
        ("credit", i18n::t("Hvem designet bygger på (valgfritt).")),
        (
            "body_font, head_font",
            i18n::t("Arial, Calibri, Cambria, Georgia, Garamond, Times New Roman, Verdana, Trebuchet MS, Tahoma, Palatino Linotype, Book Antiqua, Century Gothic, Segoe UI eller Aptos."),
        ),
        ("size", i18n::t("Brødtekst 9,5–12 pt.")),
        ("name_size", i18n::t("Navnet 16–34 pt. name_bold, name_upper: true/false. name_spacing: 0–3.")),
        ("accent, ink, muted, rule", i18n::t("Farger som «#2F4A66»: overskrifter, brødtekst, datoer, linjer. Brødteksten må ha kontrast minst 4,5:1.")),
        ("header_align", i18n::t("left eller center.")),
        ("header_fill, header_text", i18n::t("Fargebånd bak toppfeltet og tekstfargen på det, eller null.")),
        ("heading_style", i18n::t("rule, rule_above, double, bar, band eller plain. band trenger heading_fill.")),
        ("heading_upper, heading_small_caps", "true/false. heading_spacing: 0–3. heading_size: 9–16 pt."),
        ("date_style", i18n::t("right (samme linje) eller below (egen linje).")),
        ("company_italic", "true/false."),
        ("gap", i18n::t("Luft mellom delene, 0,75–1,3.")),
    ]
}
