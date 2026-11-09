//! The look: calm neutral surfaces, white cards with soft borders, one blue accent, rounded
//! controls and DM Sans throughout. Light and dark follow the computer's setting. Widgets read
//! [`Tokens::get`] instead of hard-coding colours, so both themes stay consistent.
use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, Visuals};

#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    /// Behind the CV pages.
    pub canvas: Color32,
    /// Side panels and the top bar.
    pub panel: Color32,
    pub card: Color32,
    pub border: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub faint: Color32,
    pub accent: Color32,
    #[allow(dead_code)]
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub user_bubble: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub shadow: Color32,
}

pub const LIGHT: Tokens = Tokens {
    canvas: Color32::from_rgb(0xee, 0xed, 0xea),
    panel: Color32::from_rgb(0xfa, 0xfa, 0xf8),
    card: Color32::WHITE,
    border: Color32::from_rgb(0xe4, 0xe2, 0xdd),
    field: Color32::WHITE,
    hover: Color32::from_rgb(0xf1, 0xf0, 0xec),
    text: Color32::from_rgb(0x1c, 0x1b, 0x19),
    dim: Color32::from_rgb(0x5f, 0x5c, 0x57),
    faint: Color32::from_rgb(0x8f, 0x8b, 0x84),
    accent: Color32::from_rgb(0x2b, 0x55, 0xd9),
    accent_hover: Color32::from_rgb(0x22, 0x47, 0xbd),
    accent_soft: Color32::from_rgb(0xe9, 0xef, 0xfd),
    on_accent: Color32::WHITE,
    user_bubble: Color32::from_rgb(0xef, 0xee, 0xea),
    success: Color32::from_rgb(0x1f, 0x7a, 0x45),
    warning: Color32::from_rgb(0xa1, 0x5c, 0x07),
    danger: Color32::from_rgb(0xb4, 0x2b, 0x2b),
    info: Color32::from_rgb(0x2b, 0x55, 0xd9),
    shadow: Color32::from_black_alpha(22),
};

pub const DARK: Tokens = Tokens {
    canvas: Color32::from_rgb(0x12, 0x12, 0x14),
    panel: Color32::from_rgb(0x1a, 0x1a, 0x1d),
    card: Color32::from_rgb(0x21, 0x21, 0x25),
    border: Color32::from_rgb(0x32, 0x32, 0x38),
    field: Color32::from_rgb(0x17, 0x17, 0x1a),
    hover: Color32::from_rgb(0x2a, 0x2a, 0x2f),
    text: Color32::from_rgb(0xec, 0xec, 0xee),
    dim: Color32::from_rgb(0xb0, 0xb0, 0xb8),
    faint: Color32::from_rgb(0x84, 0x84, 0x8c),
    accent: Color32::from_rgb(0x5b, 0x7f, 0xf5),
    accent_hover: Color32::from_rgb(0x74, 0x94, 0xff),
    accent_soft: Color32::from_rgb(0x24, 0x2c, 0x48),
    on_accent: Color32::WHITE,
    user_bubble: Color32::from_rgb(0x2c, 0x2c, 0x32),
    success: Color32::from_rgb(0x5c, 0xc4, 0x8a),
    warning: Color32::from_rgb(0xe8, 0xa8, 0x4a),
    danger: Color32::from_rgb(0xf0, 0x74, 0x6c),
    info: Color32::from_rgb(0x8a, 0xa6, 0xff),
    shadow: Color32::from_black_alpha(90),
};

pub const R_SM: u8 = 6;
pub const R: u8 = 8;
pub const R_LG: u8 = 12;

impl Tokens {
    pub fn get(ctx: &egui::Context) -> Tokens {
        if ctx.theme() == Theme::Dark { DARK } else { LIGHT }
    }
}

pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    add("DMSans-Regular", include_bytes!("../../../assets/fonts/DMSans-Regular.ttf"));
    add("DMSans-Medium", include_bytes!("../../../assets/fonts/DMSans-Medium.ttf"));
    add("DMSans-Bold", include_bytes!("../../../assets/fonts/DMSans-Bold.ttf"));
    // egui's own fonts stay as fallbacks for symbols DM Sans doesn't have, then the bundled
    // faces for Japanese, Chinese, Korean, Arabic, Hebrew, Thai, Devanagari and Georgian (from
    // craft-fonts, see assets/craft-fonts). Japanese comes first, so kana and kanji in one
    // line share a style.
    // Phosphor's icons (MIT, private-use code points) come before egui's own emoji font, which
    // uses some of the same code points; see `ph` below.
    add("Phosphor", include_bytes!("../../../assets/fonts/Phosphor.ttf"));
    let mut fallbacks = vec!["Phosphor".to_string()];
    fallbacks.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    let mut bold_fallbacks = Vec::new();
    for (i, font) in wordcraft_fonts::japanese_ui_fonts(true).into_iter().filter(|f| f.style == "Bold").take(1).enumerate() {
        let name = format!("bundled-bold-{i}");
        fonts.font_data.insert(name.clone(), Arc::new(FontData::from_static(font.bytes)));
        bold_fallbacks.push(name);
    }
    let others = ["Arab", "Hebr", "Thai", "Deva", "Geor", "Zsym"];
    let extra = wordcraft_fonts::ui_cjk_fonts(false).into_iter().chain(wordcraft_fonts::CRAFT_FONTS.iter().filter(|f| f.style == "Regular" && f.scripts.iter().any(|s| others.contains(s))));
    for (i, font) in extra.enumerate() {
        let name = format!("bundled-{i}");
        fonts.font_data.insert(name.clone(), Arc::new(FontData::from_static(font.bytes)));
        fallbacks.push(name.clone());
        fonts.families.entry(FontFamily::Monospace).or_default().push(name);
    }
    let with = |first: &str, before: &[String]| std::iter::once(first.to_string()).chain(before.iter().cloned()).chain(fallbacks.iter().cloned()).collect::<Vec<_>>();
    let mut families = BTreeMap::new();
    families.insert(FontFamily::Proportional, with("DMSans-Regular", &[]));
    families.insert(medium(), with("DMSans-Medium", &[]));
    families.insert(bold(), with("DMSans-Bold", &bold_fallbacks));
    families.insert(FontFamily::Monospace, fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default());
    families.insert(icons(), vec!["Phosphor".to_string()]);
    fonts.families = families;
    fonts
}

pub fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// Icons from Phosphor (regular weight), by name: https://phosphoricons.com
pub mod ph {
    pub const FILE_PLUS: &str = "\u{e236}";
    pub const LIGHTBULB: &str = "\u{e2dc}";
    pub const FOLDER_OPEN: &str = "\u{e256}";
    pub const FLOPPY_DISK: &str = "\u{e248}";
    pub const ARROW_SQUARE_IN: &str = "\u{e5dc}";
    pub const COPY: &str = "\u{e1ca}";
    pub const FILE_PDF: &str = "\u{e702}";
    pub const FILE_DOC: &str = "\u{eb1e}";
    pub const CLIPBOARD_TEXT: &str = "\u{e198}";
    pub const LINK: &str = "\u{e2e2}";
    pub const FILE_ARROW_UP: &str = "\u{e61e}";
    pub const UNDO: &str = "\u{e038}";
    pub const TRANSLATE: &str = "\u{e4a2}";
    pub const BRIEFCASE: &str = "\u{e0ee}";
    pub const GRADUATION_CAP: &str = "\u{e62c}";
    pub const CERTIFICATE: &str = "\u{e766}";
    pub const LIST_PLUS: &str = "\u{e2f8}";
    pub const STAR: &str = "\u{e46a}";
    pub const HEART: &str = "\u{e2a8}";
    pub const CHAT_TEXT: &str = "\u{e16e}";
    pub const ADDRESS_BOOK: &str = "\u{e6f8}";
    pub const USER_CIRCLE: &str = "\u{e4c4}";
    pub const FRAME_CORNERS: &str = "\u{e626}";
    pub const COLUMNS: &str = "\u{e546}";
    pub const FILE: &str = "\u{e230}";
    pub const ROWS: &str = "\u{e5a2}";
    pub const TEXT_AA: &str = "\u{e6ee}";
    pub const ARROWS_VERTICAL: &str = "\u{eb04}";
    pub const PALETTE: &str = "\u{e6c8}";
    pub const UPLOAD: &str = "\u{e4c0}";
    pub const EYE: &str = "\u{e220}";
    pub const SIDEBAR: &str = "\u{ec24}";
    pub const SQUARES_FOUR: &str = "\u{e464}";
    pub const ZOOM: &str = "\u{e310}";
    pub const PLUGS_CONNECTED: &str = "\u{eb5a}";
    pub const PLUG: &str = "\u{e946}";
    pub const CHAT: &str = "\u{e168}";
    pub const CHAT_DOTS: &str = "\u{e16c}";
    pub const LIST_CHECKS: &str = "\u{eadc}";
    pub const INFO: &str = "\u{e2ce}";
    pub const FILE_CODE: &str = "\u{e914}";
    pub const GEAR: &str = "\u{e270}";
    pub const NOTEPAD: &str = "\u{e63e}";
    pub const MINUS: &str = "\u{e32a}";
    pub const PLUS: &str = "\u{e3d4}";
    pub const CARET_DOWN: &str = "\u{e136}";

    /// True for a character from the icon font (Unicode's private-use area).
    pub fn is_icon(c: char) -> bool {
        ('\u{e000}'..='\u{f8ff}').contains(&c)
    }
}

/// A small flat button with only an icon (the caller adds the hover text).
pub fn flat_icon(ui: &mut egui::Ui, icon: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let b = egui::Button::new(egui::RichText::new(icon).family(icons()).size(14.0).color(if enabled { t.text } else { t.border }))
        .frame_when_inactive(false)
        .corner_radius(CornerRadius::same(R_SM))
        .min_size(egui::vec2(26.0, 26.0));
    ui.add_enabled(enabled, b)
}

/// A ribbon tool: an icon in the accent colour and a label, flat until hovered, like Word's.
pub fn tool_button<'a>(ui: &egui::Ui, icon: &str, text: &str, enabled: bool, caret: bool) -> egui::Button<'a> {
    let t = Tokens::get(ui.ctx());
    let icon = egui::RichText::new(icon).family(icons()).size(17.0).color(if enabled { t.accent } else { t.border });
    let label = egui::RichText::new(text).size(13.0).color(if enabled { t.text } else { t.faint });
    let b = if caret {
        egui::Button::new((icon, label, egui::RichText::new(ph::CARET_DOWN).family(icons()).size(11.0).color(t.faint)))
    } else {
        egui::Button::new((icon, label))
    };
    b.wrap_mode(egui::TextWrapMode::Extend).frame_when_inactive(false).corner_radius(CornerRadius::same(R_SM)).min_size(egui::vec2(0.0, 26.0))
}

fn visuals(t: &Tokens, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let r = CornerRadius::same(R);
    v.panel_fill = t.panel;
    v.window_fill = t.card;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(R_LG);
    v.menu_corner_radius = CornerRadius::same(R);
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: t.shadow };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: t.shadow };
    v.extreme_bg_color = t.field;
    v.text_edit_bg_color = Some(t.field);
    v.faint_bg_color = t.hover;
    v.code_bg_color = t.hover;
    v.hyperlink_color = t.accent;
    v.selection.bg_fill = if dark { Color32::from_rgb(0x2f, 0x40, 0x7a) } else { Color32::from_rgb(0xcf, 0xdc, 0xfb) };
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.override_text_color = None;
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.card;
    w.noninteractive.weak_bg_fill = t.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = t.hover;
    w.inactive.weak_bg_fill = t.card;
    w.inactive.bg_stroke = Stroke::new(1.0, t.border);
    w.inactive.fg_stroke = Stroke::new(1.0, t.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = t.hover;
    w.hovered.weak_bg_fill = t.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, t.faint);
    w.hovered.fg_stroke = Stroke::new(1.0, t.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = t.accent_soft;
    w.active.weak_bg_fill = t.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, t.accent);
    w.active.fg_stroke = Stroke::new(1.0, t.text);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open.bg_fill = t.hover;
    w.open.weak_bg_fill = t.hover;
    w.open.bg_stroke = Stroke::new(1.0, t.border);
    w.open.fg_stroke = Stroke::new(1.0, t.text);
    w.open.corner_radius = r;
    v.text_cursor.stroke = Stroke::new(1.5, t.accent);
    v
}

/// Sets fonts, sizes, spacing and both themes. Call once at start.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, medium())),
            (TextStyle::Heading, FontId::new(19.0, bold())),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.window_margin = Margin::same(20);
        s.spacing.menu_margin = Margin::same(6);
        s.spacing.indent = 14.0;
        // Scroll bars get their own strip: floating ones lay over the right edge of full-width
        // buttons and took the click there (found by the click check in ui_qa.rs).
        s.spacing.scroll.bar_width = 8.0;
        s.spacing.scroll.floating = false;
        s.spacing.scroll.bar_inner_margin = 2.0;
        // Clear of the panels' resize edges, which reach a few points into the neighbour.
        s.spacing.scroll.bar_outer_margin = 4.0;
        // Plain text is not selectable: a selectable title inside a clickable header (a form
        // section's fold area) took the click as a text selection instead of folding. Text in
        // fields stays selectable.
        s.interaction.selectable_labels = false;
    });
}

/// A white card with a soft border, for one section of the form.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(R_LG)).inner_margin(Margin::same(16))
}

/// The filled main action (download PDF, send), with its tip on hover – also while it is off.
pub fn primary(ui: &mut egui::Ui, text: &str, tip: &str, enabled: bool) -> egui::Response {
    debug_assert!(!tip.trim().is_empty(), "the button «{text}» needs a tip");
    let t = Tokens::get(ui.ctx());
    let label = egui::RichText::new(text).color(t.on_accent).family(medium());
    ui.add_enabled(enabled, egui::Button::new(label).fill(if enabled { t.accent } else { t.faint }).stroke(Stroke::NONE).corner_radius(CornerRadius::same(R)))
        .on_hover_text(tip)
        .on_disabled_hover_text(tip)
}

/// A quiet button without a frame until hovered, with its tip on hover.
pub fn ghost(ui: &mut egui::Ui, text: &str, tip: &str) -> egui::Response {
    debug_assert!(!tip.trim().is_empty(), "the button «{text}» needs a tip");
    let t = Tokens::get(ui.ctx());
    ui.add(egui::Button::new(egui::RichText::new(text).color(t.dim)).frame_when_inactive(false).corner_radius(CornerRadius::same(R_SM))).on_hover_text(tip)
}

/// A small rounded label such as «2 A4-sider» or «3».
pub fn pill(ui: &mut egui::Ui, text: &str, fill: Color32, color: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| ui.add(egui::Label::new(egui::RichText::new(text).size(12.0).family(medium()).color(color)).wrap_mode(egui::TextWrapMode::Extend)))
        .response
}

/// A small heading above a group of fields.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    // Shortened with «…» (whole on hover) rather than widening the panel: a long title in a
    // long language pushed the form panel wider (found by ui_qa.rs in Ukrainian).
    ui.add(egui::Label::new(egui::RichText::new(text).size(15.5).family(bold()).color(t.text)).truncate());
}

/// The dim label above a field.
pub fn label(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).size(12.5).family(medium()).color(t.dim));
}

pub fn hint(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.5).color(t.faint)).wrap());
}

/// The «delete» button: a drawn bin (the font has no such glyph), saying on hover what it deletes.
pub fn remove_button(ui: &mut egui::Ui, tip: &str) -> egui::Response {
    icon_button(ui, Icon::Trash, tip, true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Trash,
    Up,
    Down,
    Hide,
}

/// A small icon button drawn with lines, so it looks the same in every font.
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tip: &str, enabled: bool) -> egui::Response {
    debug_assert!(!tip.trim().is_empty(), "an icon button needs a tip");
    let t = Tokens::get(ui.ctx());
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    // 30 points: comfortable to hit with a mouse or a finger.
    let (rect, r) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), sense);
    let hot = enabled && r.hovered();
    if hot {
        ui.painter().rect_filled(rect, CornerRadius::same(R_SM), t.hover);
    }
    let color = if !enabled {
        t.border
    } else if hot && icon == Icon::Trash {
        t.danger
    } else if hot {
        t.text
    } else {
        t.faint
    };
    let s = Stroke::new(1.4, color);
    let p = ui.painter();
    let c = rect.center();
    let v = egui::vec2;
    match icon {
        Icon::Trash => {
            // Lid, handle and a can with two lines.
            p.line_segment([c + v(-6.0, -4.0), c + v(6.0, -4.0)], s);
            p.line_segment([c + v(-2.0, -6.0), c + v(2.0, -6.0)], s);
            p.line_segment([c + v(-2.0, -6.0), c + v(-2.0, -4.0)], s);
            p.line_segment([c + v(2.0, -6.0), c + v(2.0, -4.0)], s);
            p.line_segment([c + v(-4.5, -4.0), c + v(-3.5, 6.5)], s);
            p.line_segment([c + v(4.5, -4.0), c + v(3.5, 6.5)], s);
            p.line_segment([c + v(-3.5, 6.5), c + v(3.5, 6.5)], s);
            p.line_segment([c + v(-1.2, -1.5), c + v(-1.0, 4.0)], s);
            p.line_segment([c + v(1.2, -1.5), c + v(1.0, 4.0)], s);
        }
        Icon::Up | Icon::Down => {
            let d = if icon == Icon::Up { -1.0 } else { 1.0 };
            p.line_segment([c + v(-4.5, -2.0 * d), c + v(0.0, 2.5 * d)], s);
            p.line_segment([c + v(4.5, -2.0 * d), c + v(0.0, 2.5 * d)], s);
        }
        Icon::Hide => {
            p.line_segment([c + v(-5.0, 0.0), c + v(5.0, 0.0)], s);
        }
    }
    let r = if enabled { r.on_hover_cursor(egui::CursorIcon::PointingHand) } else { r };
    r.on_hover_text(tip)
}

/// A pill in a wrapping row: starts a new row when it would not fit, so the row never pushes its
/// panel wider.
pub fn pill_wrapped(ui: &mut egui::Ui, text: &str, fill: Color32, color: Color32) -> egui::Response {
    let font = egui::FontId::new(12.0, medium());
    let width = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, color).size().x) + 20.0;
    // In a wrapping row, what is left on this line (available_width is the whole row).
    if width > ui.available_size_before_wrap().x {
        ui.end_row();
    }
    pill(ui, text, fill, color)
}
