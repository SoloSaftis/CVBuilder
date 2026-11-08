//! Colours, independent of the design: a palette for the whole CV (ready-made or your own
//! colour for each role), and a colour per section from its card in the form.
use cv_core::layouts;
use cv_core::model::Palette;
use egui::{CornerRadius, RichText};

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

/// A colour button big enough to hit easily (egui's own is a small swatch).
fn color_button(ui: &mut egui::Ui, c: &mut [u8; 3], tip: &str) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size = egui::vec2(36.0, 26.0);
        ui.color_edit_button_srgb(c).on_hover_text(tip)
    })
    .inner
}

pub fn to_rgb(hex: &str) -> [u8; 3] {
    let c = layouts::rgb(hex);
    [c.0, c.1, c.2]
}

pub fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn chip(ui: &mut egui::Ui, colors: &[[u8; 3]]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0 * colors.len() as f32, 16.0), egui::Sense::hover());
    for (i, c) in colors.iter().enumerate() {
        let r = egui::Rect::from_min_size(rect.min + egui::vec2(14.0 * i as f32, 0.0), egui::vec2(14.0, 16.0));
        ui.painter().rect_filled(r, CornerRadius::same(3), egui::Color32::from_rgb(c[0], c[1], c[2]));
    }
}

/// One colour role: a picker, and «Design» to go back to the design's own colour.
fn role(ui: &mut egui::Ui, label: &str, tip: &str, shown: &str, value: &mut Option<String>) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new(RichText::new(label).size(13.0)));
        let mut c = to_rgb(shown);
        if color_button(ui, &mut c, tip).changed() {
            *value = Some(to_hex(c));
            changed = true;
        }
        if value.is_some() && theme::ghost(ui, i18n::t("Design"), i18n::t("Bruk designets egen farge")).clicked() {
            *value = None;
            changed = true;
        }
        if value.is_none() {
            ui.label(RichText::new(i18n::t("designets")).size(11.5).color(t.faint));
        }
    });
    changed
}

/// A band (header or headings): on/off and its colour.
fn band(ui: &mut egui::Ui, label: &str, tip: &str, current: Option<&str>, value: &mut Option<String>, default: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new(RichText::new(label).size(13.0)));
        let mut on = current.is_some();
        if ui.checkbox(&mut on, "").on_hover_text(tip).changed() {
            *value = Some(if on { default.to_string() } else { "none".into() });
            changed = true;
        }
        if let Some(cur) = current {
            let mut c = to_rgb(cur);
            if color_button(ui, &mut c, tip).changed() {
                *value = Some(to_hex(c));
                changed = true;
            }
        }
        if value.is_some() && theme::ghost(ui, i18n::t("Design"), i18n::t("Som i designet")).clicked() {
            *value = None;
            changed = true;
        }
    });
    changed
}

impl CvApp {
    /// «Farger» in the top bar.
    pub(crate) fn color_menu(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let d = layouts::effective(&self.project);
        let custom = !self.project.options.palette.is_empty() || !self.project.options.section_colors.is_empty();
        let label = if custom { i18n::t("Farger · egne") } else { i18n::t("Farger") };
        let mut changed = false;
        ui.menu_button(RichText::new(label).size(13.5).family(theme::medium()), |ui| {
            ui.set_min_width(330.0);
            ui.label(RichText::new(i18n::t("Fargene er uavhengige av designet. Bytter du design, beholdes fargene du har valgt.")).size(12.0).color(t.dim));
            ui.add_space(4.0);
            theme::label(ui, i18n::t("Ferdige paletter"));
            for (name, palette) in layouts::palettes() {
                let preview = {
                    let mut x = layouts::design(&self.project.options.layout);
                    layouts::apply_palette(&mut x, &palette);
                    x
                };
                let on = self.project.options.palette == palette;
                let r = ui.horizontal(|ui| {
                    chip(ui, &[to_rgb(&preview.accent), to_rgb(&preview.ink), to_rgb(&preview.rule)]);
                    ui.label(RichText::new(i18n::tdyn(name)).color(if on { t.accent } else { t.text }));
                });
                if ui
                    .interact(r.response.rect, ui.scope_id().with(("palette", name)), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(i18n::t("Bruk paletten på hele CV-en. Den beholdes når du bytter design."))
                    .clicked()
                {
                    self.project.options.palette = palette;
                    changed = true;
                }
            }
            ui.separator();
            theme::label(ui, i18n::t("Dine farger"));
            let p = &mut self.project.options.palette;
            changed |= role(ui, i18n::t("Hovedfarge"), i18n::t("Fargen på overskrifter og detaljer som skiller seg ut"), &d.accent, &mut p.accent);
            changed |= role(ui, i18n::t("Brødtekst"), i18n::t("Fargen på vanlig tekst. Mørk tekst på lys bakgrunn leses best."), &d.ink, &mut p.ink);
            changed |= role(ui, i18n::t("Datoer og detaljer"), i18n::t("Fargen på datoer, steder og annen hjelpetekst"), &d.muted, &mut p.muted);
            changed |= role(ui, i18n::t("Linjer"), i18n::t("Fargen på skillelinjene"), &d.rule, &mut p.rule);
            changed |= band(ui, i18n::t("Fargefelt øverst"), i18n::t("Et farget felt bak navnet og kontaktinformasjonen øverst"), d.header_fill.as_deref(), &mut p.header_fill, &d.accent);
            if let Some(text) = d.header_text.as_deref() {
                changed |= role(ui, i18n::t("Tekst i fargefeltet"), i18n::t("Tekstfargen i fargefeltet. Sørg for god kontrast mot feltet."), text, &mut p.header_text);
            }
            let heading_band = (d.heading_style == layouts::HeadingStyle::Band).then(|| d.heading_fill.clone().unwrap_or_default());
            changed |= band(ui, i18n::t("Bånd bak overskrifter"), i18n::t("Et farget bånd bak hver seksjonsoverskrift"), heading_band.as_deref(), &mut p.heading_fill, "#EEF1F5");
            ui.add_space(4.0);
            theme::hint(ui, i18n::t("Hver seksjon kan også få sin egen farge: bruk fargeknappen på seksjonen i skjemaet."));
            if custom && ui.button(i18n::t("Tilbake til designets farger")).on_hover_text(i18n::t("Fjern alle egne farger, også seksjonenes")).clicked() {
                self.project.options.palette = Palette::default();
                self.project.options.section_colors.clear();
                changed = true;
            }
        })
        .response
        .on_hover_text(i18n::t("Farger for hele CV-en, uavhengig av designet"));
        if changed {
            self.preview.changed();
            self.save_at = Some(std::time::Instant::now());
        }
    }

    /// The colour button on a section's card.
    pub(crate) fn section_color(&mut self, ui: &mut egui::Ui, key: &str) {
        let d = layouts::effective(&self.project);
        let own = self.project.options.section_colors.contains_key(key);
        let mut c = to_rgb(d.section_accent(key));
        let r = color_button(ui, &mut c, if own { i18n::t("Seksjonens egen farge. Høyreklikk for å bruke hovedfargen igjen.") } else { i18n::t("Gi seksjonen sin egen farge") });
        let mut changed = false;
        if r.changed() {
            self.project.options.section_colors.insert(key.to_string(), to_hex(c));
            changed = true;
        }
        if own && r.secondary_clicked() {
            self.project.options.section_colors.remove(key);
            changed = true;
        }
        if changed {
            self.preview.changed();
            self.save_at = Some(std::time::Instant::now());
        }
    }
}
