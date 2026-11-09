//! SHARED by Windows, Linux and macOS: the CVBuilder desktop window. Same rules, layouts and
//! project files as the HTML builder, with Word and PDF written by WordCraft from one layout and
//! a preview of those exact pages, and an AI chat that edits the CV through MCP tools while the
//! person watches. The platform folders only start it with [`run`].

pub use eframe::Result;

mod ai_ui;
mod chat_ui;
mod checklist_ui;
mod colors_ui;
mod designs_ui;
mod form;
mod i18n;
mod import_ui;
mod langpack_ui;
mod link;
mod menu_ui;
mod page_ui;
mod photo;
mod preview;
mod settings_ui;
mod storage;
mod theme;
#[cfg(test)]
mod tooltips;
mod translate_ui;
#[cfg(test)]
mod ui_qa;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use cv_core::checks::{self, Level};
use cv_core::{Project, document};
use egui::{CornerRadius, Margin, RichText, Stroke};
use theme::Tokens;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Export {
    Word,
    Pdf,
    Text,
}

enum Picked {
    OpenProject(PathBuf),
    SaveProject(PathBuf),
    Export(Export, PathBuf),
    Photo(PathBuf),
    ImportCv(PathBuf),
    AdFile(PathBuf),
    DesignImport(PathBuf),
    ChecklistLoad(PathBuf),
    ChecklistSave(PathBuf),
    DesignSave(PathBuf),
}

struct PhotoDialog {
    edit: photo::Edit,
    texture: Option<egui::TextureHandle>,
    stale: bool,
}

/// Which of the three views are shown, and the preview's zoom (0 = fit the view).
#[derive(Clone, Copy)]
struct Views {
    form: bool,
    preview: bool,
    side: bool,
    zoom: f32,
}

impl Default for Views {
    fn default() -> Self {
        Views { form: true, preview: true, side: true, zoom: 0.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Chat,
    Checks,
}

struct CvApp {
    project: Project,
    is_example: bool,
    settings: storage::Settings,
    /// Text being typed into list and date fields, committed to the project as it becomes valid.
    buffers: HashMap<String, String>,
    bad_dates: HashSet<String>,
    preview: preview::Preview,
    status: String,
    undo: Option<(Project, bool)>,
    confirm: Option<Export>,
    picked_tx: Sender<Picked>,
    picked_rx: Receiver<Picked>,
    photo: Option<PhotoDialog>,
    save_at: Option<Instant>,
    sequence: u64,
    ai: ai_ui::AiState,
    chat: chat_ui::ChatState,
    link: Option<link::Link>,
    egui_ctx: egui::Context,
    side: Side,
    /// Form sections the person has folded away.
    folded: HashSet<String>,
    views: Views,
    /// The CV as it was before its language was switched, shown under the fields to translate from.
    reference: Option<Project>,
    /// The «Oversett CV-en» window, while it is open.
    translate: Option<translate_ui::TranslateState>,
    designs: designs_ui::DesignState,
    portrait: Option<(u64, egui::TextureHandle)>,
    about_open: bool,
    import: import_ui::ImportState,
    settings_open: bool,
    /// Added interface languages, and the translation that may be running.
    langs: langpack_ui::LangState,
    checklist: checklist_ui::ChecklistState,
    /// The name being typed for a new section, while that window is open.
    new_section: Option<String>,
    /// Where the new section goes in the CV: `None` at the bottom, `Some(i)` at position i.
    new_section_at: Option<usize>,
    ribbon_tab: menu_ui::Tab,
    /// The ribbon folded to its row of tabs.
    ribbon_folded: bool,
}

impl CvApp {
    fn new(cc: &eframe::CreationContext<'_>) -> CvApp {
        theme::install(&cc.egui_ctx);
        let settings = storage::load_settings();
        i18n::set(i18n::from_setting(&settings.ui_language));
        // Added languages (packs) too: the setting may name one, or «auto» may pick one.
        let langs = langpack_ui::LangState::load(&settings.ui_language);
        cc.egui_ctx.set_theme(settings_ui::theme_preference(&settings.theme));
        let (picked_tx, picked_rx) = channel();
        let mut app = CvApp {
            project: Project::example(),
            is_example: true,
            settings,
            buffers: HashMap::new(),
            bad_dates: HashSet::new(),
            preview: preview::Preview::new(cc.egui_ctx.clone()),
            status: i18n::t("Dette er et eksempel. Velg «Ny CV» for å starte på din egen, eller skriv rett inn i eksemplet.").into(),
            undo: None,
            confirm: None,
            picked_tx,
            picked_rx,
            photo: None,
            save_at: None,
            sequence: 0,
            ai: ai_ui::AiState::default(),
            chat: chat_ui::ChatState::new(cv_ai::chat::find_claude_code()),
            link: link::Link::start(cc.egui_ctx.clone()),
            egui_ctx: cc.egui_ctx.clone(),
            side: Side::Chat,
            folded: ["certifications", "languages", "interests"].into_iter().map(String::from).collect(),
            views: Views::default(),
            reference: None,
            translate: None,
            designs: designs_ui::DesignState::default(),
            portrait: None,
            about_open: false,
            import: import_ui::ImportState::default(),
            settings_open: false,
            langs,
            checklist: checklist_ui::ChecklistState::load(),
            new_section: None,
            new_section_at: None,
            ribbon_tab: menu_ui::Tab::default(),
            ribbon_folded: false,
        };
        designs_ui::load_imported();
        // Developer aids for screenshots and manual testing.
        match std::env::var("CVBUILDER_OPEN").as_deref() {
            Ok("ai") => {
                app.ai.connect_open = true;
                app.ai.choice = Some(cv_ai::Service::Claude);
            }
            Ok("mcp") => app.chat.mcp_help_open = true,
            Ok("checks") => app.side = Side::Checks,
            Ok("about") => app.about_open = true,
            Ok("insert") => app.ribbon_tab = menu_ui::Tab::Insert,
            Ok("design") => app.ribbon_tab = menu_ui::Tab::Design,
            Ok("view") => app.ribbon_tab = menu_ui::Tab::View,
            Ok("page") => app.ribbon_tab = menu_ui::Tab::Page,
            Ok("settings") => app.settings_open = true,
            Ok("section") => app.new_section = Some(String::new()),
            _ => {}
        }
        if let Ok(theme) = std::env::var("CVBUILDER_THEME") {
            cc.egui_ctx.set_theme(if theme == "dark" { egui::Theme::Dark } else { egui::Theme::Light });
        }
        if app.settings.remember {
            if let Some(text) = storage::load_draft() {
                match Project::from_json(&text) {
                    Ok(project) => {
                        app.load(project, false);
                        app.status = i18n::t("Velkommen tilbake. Utkastet ditt er hentet fra denne maskinen.").into();
                    }
                    Err(_) => {
                        storage::save_backup(&text);
                        app.status = i18n::t("Det lagrede utkastet kunne ikke åpnes, men er tatt vare på som sikkerhetskopi. Du ser nå eksemplet.").into();
                    }
                }
            }
        }
        // Developer aid: CVBUILDER_ASK="…" sends one chat message at start (for testing the chat).
        if let Ok(ask) = std::env::var("CVBUILDER_ASK") {
            app.chat_send(&ask);
        }
        app
    }

    fn load(&mut self, project: Project, example: bool) {
        self.project = project;
        self.is_example = example;
        self.buffers.clear();
        self.bad_dates.clear();
        self.preview.changed();
        if !example {
            self.persist();
        }
    }

    /// Any edit: approval must be given again, the preview refreshes and the draft is saved soon.
    fn touched(&mut self) {
        self.project.approvals.facts_confirmed = false;
        self.preview.changed();
        if self.is_example {
            self.is_example = false;
            self.status = i18n::t("Nå er dette din CV. Den huskes på denne maskinen mens du skriver.").into();
        }
        self.save_at = Some(Instant::now());
    }

    fn persist(&self) {
        if self.settings.remember && !self.is_example {
            if let Ok(json) = self.project.to_json() {
                storage::save_draft(&json);
            }
        }
    }

    fn replace(&mut self, project: Project, example: bool, text: &str) {
        let before = (self.project.clone(), self.is_example);
        if !before.1 {
            if let Ok(json) = before.0.to_json() {
                storage::save_backup(&json);
            }
        }
        self.load(project, example);
        self.undo = Some(before);
        self.status = text.into();
    }

    fn next_id(&mut self, prefix: &str) -> String {
        self.sequence += 1;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        format!("{prefix}_{now:x}_{}", self.sequence)
    }

    fn file_stem(&self) -> String {
        let clean = |s: &str| {
            let t: String = s
                .chars()
                .map(|c| match c {
                    'æ' => "ae".to_string(),
                    'Æ' => "Ae".to_string(),
                    'ø' => "o".to_string(),
                    'Ø' => "O".to_string(),
                    'å' => "a".to_string(),
                    'Å' => "A".to_string(),
                    c if c.is_ascii_alphanumeric() => c.to_string(),
                    _ => "_".to_string(),
                })
                .collect();
            t.split('_').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("_")
        };
        let lang = if self.project.is_english() { "en" } else { "no" };
        [clean(&self.project.candidate.name), "CV".into(), clean(&self.project.target.company), lang.into()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
    }

    /// The page count recruiters would advise for this CV.
    pub(crate) fn recommended_pages(&self) -> usize {
        let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 2_629_746).unwrap_or(0) as i32 + 1970 * 12;
        checks::recommended_pages(&self.project, today)
    }

    /// Sends a request to the chat and shows it.
    fn ask_ai(&mut self, text: &str) {
        self.side = Side::Chat;
        self.chat_send(text);
    }

    // ---------- file dialogs run on their own thread so the window stays responsive ----------

    fn pick(&self, kind: &'static str, export: Option<Export>) {
        let tx = self.picked_tx.clone();
        let stem = self.file_stem();
        std::thread::spawn(move || {
            let picked = match (kind, export) {
                ("open", _) => rfd::FileDialog::new().add_filter(i18n::t("CVBuilder-utkast"), &["json"]).pick_file().map(Picked::OpenProject),
                ("save", _) => rfd::FileDialog::new().add_filter(i18n::t("CVBuilder-utkast"), &["json"]).set_file_name(format!("{stem}_utkast.json")).save_file().map(Picked::SaveProject),
                ("import_cv", _) => rfd::FileDialog::new().add_filter(i18n::t("CV (PDF, Word, tekst, nettside, CVBuilder)"), &cv_core::import::FILE_TYPES).pick_file().map(Picked::ImportCv),
                ("ad_file", _) => rfd::FileDialog::new().add_filter(i18n::t("Annonse (PDF, Word, tekst, nettside)"), &["pdf", "docx", "txt", "md", "html", "htm"]).pick_file().map(Picked::AdFile),
                ("checklist_load", _) => rfd::FileDialog::new().add_filter("CVBuilder-sjekkliste", &["json"]).pick_file().map(Picked::ChecklistLoad),
                ("checklist_save", _) => rfd::FileDialog::new().add_filter("CVBuilder-sjekkliste", &["json"]).set_file_name("min-sjekkliste.cvchecklist.json").save_file().map(Picked::ChecklistSave),
                ("design_import", _) => rfd::FileDialog::new().add_filter(i18n::t("CVBuilder-design"), &["json"]).pick_file().map(Picked::DesignImport),
                ("design_save", _) => rfd::FileDialog::new().add_filter(i18n::t("CVBuilder-design"), &["json"]).set_file_name("min-design.cvdesign.json").save_file().map(Picked::DesignSave),
                ("photo", _) => rfd::FileDialog::new().add_filter(i18n::t("Bilde"), &["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"]).pick_file().map(Picked::Photo),
                ("export", Some(Export::Word)) => rfd::FileDialog::new().add_filter("Word", &["docx"]).set_file_name(format!("{stem}.docx")).save_file().map(|p| Picked::Export(Export::Word, p)),
                ("export", Some(Export::Pdf)) => rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{stem}.pdf")).save_file().map(|p| Picked::Export(Export::Pdf, p)),
                _ => None,
            };
            if let Some(p) = picked {
                let _ = tx.send(p);
            }
        });
    }

    fn handle_picked(&mut self, ctx: &egui::Context) {
        while let Ok(picked) = self.picked_rx.try_recv() {
            match picked {
                Picked::OpenProject(path) => match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Project::from_json(&t).map_err(|e| e.to_string())) {
                    Ok(project) => self.replace(project, false, i18n::t("Utkastet er åpnet. Bekreft opplysningene når du laster ned.")),
                    Err(_) => {
                        self.status = i18n::tf(
                            "«{file}» kunne ikke åpnes. Her åpner du bare .json-utkast lagret med «Lagre som fil».",
                            &[("file", &path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())],
                        )
                    }
                },
                Picked::SaveProject(path) => {
                    self.status = match self.project.to_json().map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string())) {
                        Ok(()) => i18n::tf("Utkastet er lagret i {path}. Filen inneholder dine personopplysninger.", &[("path", &path.display().to_string())]),
                        Err(e) => i18n::tf("Kunne ikke lagre: {e}", &[("e", &e)]),
                    }
                }
                Picked::Export(kind, path) => self.write_export(kind, &path),
                Picked::ImportCv(path) => self.import_cv_file(path),
                Picked::AdFile(path) => self.import_ad_file(path),
                Picked::DesignImport(path) => self.import_design(&path),
                Picked::ChecklistLoad(path) => self.load_checklist(&path),
                Picked::ChecklistSave(path) => self.save_checklist_file(&path),
                Picked::DesignSave(path) => self.save_design_template(&path),
                Picked::Photo(path) => match photo::Edit::open(&path) {
                    Ok(edit) => self.photo = Some(PhotoDialog { edit, texture: None, stale: true }),
                    Err(e) => self.status = e,
                },
            }
            ctx.request_repaint();
        }
    }

    fn write_export(&mut self, kind: Export, path: &std::path::Path) {
        let doc = document::build(&self.project);
        let result = match kind {
            Export::Word => document::to_docx(&doc),
            Export::Pdf => document::to_pdf(&doc),
            Export::Text => return,
        };
        self.status = match result.map_err(|e| e.to_string()).and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string())) {
            Ok(()) => match kind {
                Export::Word => i18n::tf("Word-filen er lagret: {path}. Åpne den og se over sidebrytingen før du sender den.", &[("path", &path.display().to_string())]),
                _ => i18n::tf("PDF-en er lagret med ekte, søkbar tekst: {path}", &[("path", &path.display().to_string())]),
            },
            Err(e) => i18n::tf("Kunne ikke lagre filen: {e}", &[("e", &e)]),
        };
    }

    fn start_export(&mut self, ctx: &egui::Context, kind: Export) {
        // Downloading always works. While something important is missing, the window before it
        // says what, and the person can download anyway or go and fill it in.
        if !self.project.approvals.facts_confirmed || !checks::blocking(&self.project).is_empty() {
            self.confirm = Some(kind);
            return;
        }
        self.export_now(ctx, kind);
    }

    fn export_now(&mut self, ctx: &egui::Context, kind: Export) {
        match kind {
            Export::Text => {
                ctx.copy_text(checks::plain_text(&self.project));
                self.status = i18n::t("CV-teksten er kopiert. Lim den inn der søknadsportalen ber om CV som tekst.").into();
            }
            other => self.pick("export", Some(other)),
        }
    }

    /// A small header over a view, with the button that hides it to the ribbon.
    fn view_header(ui: &mut egui::Ui, title: &str, shown: &mut bool) {
        let t = Tokens::get(ui.ctx());
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(12.0).family(theme::medium()).color(t.faint));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button(ui, theme::Icon::Hide, i18n::t("Skjul (vis igjen fra «Vis:» øverst)"), true).clicked() {
                    *shown = false;
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        // The fixed things on the right first; the message gets what is left and is shortened
        // with «…» (whole on hover), so a long message in a long language can't run into them.
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut remember = self.settings.remember;
            if ui
                .checkbox(&mut remember, RichText::new(i18n::t("Husk utkastet på denne maskinen")).size(12.5))
                .on_hover_text(i18n::t("Utkastet lagres her mens du skriver. Slå av på en delt maskin."))
                .changed()
            {
                self.set_remember(remember);
            }
            if storage::portable() {
                theme::pill(ui, i18n::t("Bærbar modus"), t.hover, t.dim).on_hover_text(i18n::t("Bærbar modus: alt lagres i mappen ved siden av programmet."));
            }
            if self.link.is_some() {
                theme::pill(ui, i18n::t("MCP på"), t.hover, t.dim).on_hover_text(i18n::t("AI-apper på denne maskinen kan koble til CVBuilder. Se «MCP» i AI-panelet."));
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if let Some((project, example)) = self.undo.clone() {
                    if ui.small_button(i18n::t("Angre")).on_hover_text(i18n::t("Hent tilbake CV-en fra før siste store endring")).clicked() {
                        self.load(project, example);
                        self.undo = None;
                        self.status = if example { i18n::t("Eksemplet er tilbake.").into() } else { i18n::t("CV-en din er tilbake.").into() };
                    }
                }
                ui.add(egui::Label::new(RichText::new(&self.status).size(12.5).color(t.dim)).truncate());
            });
        });
    }

    // ---------- right side: chat and checklist ----------

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let blocking = checks::blocking(&self.project);
        egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(3)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                let w = ((ui.available_width() - 10.0) / 2.0).floor();
                for (side, name, tip) in [
                    (Side::Chat, i18n::t("AI-assistent").to_string(), i18n::t("Be AI-en om endringer i CV-en med egne ord")),
                    (
                        Side::Checks,
                        if blocking.is_empty() { i18n::t("Sjekkliste").to_string() } else { i18n::tf("Sjekkliste · {n}", &[("n", &blocking.len().to_string())]) },
                        i18n::t("Hva som mangler før nedlasting, og råd fra rekrutterere og ATS"),
                    ),
                ] {
                    let on = self.side == side;
                    let b = egui::Button::new(RichText::new(name.as_str()).size(13.0).family(theme::medium()).color(if on { t.text } else { t.dim }))
                        .fill(if on { t.card } else { egui::Color32::TRANSPARENT })
                        .stroke(if on { Stroke::new(1.0, t.border) } else { Stroke::NONE })
                        .corner_radius(CornerRadius::same(theme::R_SM))
                        .truncate();
                    // Exactly half each, the name shortened if it is longer (one long Icelandic
                    // word widened the panel; found by ui_qa.rs).
                    if ui.add_sized([w, 28.0], b).on_hover_text(format!("{name}: {tip}")).clicked() {
                        self.side = side;
                    }
                }
            });
        });
        ui.add_space(10.0);
        match self.side {
            Side::Chat => self.chat_panel(ui),
            Side::Checks => {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.checklist(ui, &blocking));
            }
        }
    }

    fn checklist(&mut self, ui: &mut egui::Ui, blocking: &[String]) {
        let t = Tokens::get(ui.ctx());
        self.checklist_choice(ui);
        ui.add_space(6.0);
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t("Før nedlasting"));
            let design = cv_core::layouts::effective(&self.project);
            if let Some(w) = design.ats_warning() {
                status_line(ui, t.warning, &i18n::tdyn(w));
            }
            let (pages, advice) = (self.preview.page_count, self.recommended_pages());
            if pages > advice {
                let color = if pages > advice + 1 { t.danger } else { t.warning };
                status_line(
                    ui,
                    color,
                    &i18n::tf(
                        "CV-en er {pages} sider. Vi anbefaler sterkt {advice}: rekrutterere leser raskt, og det viktigste må komme først. Kort ned, eller velg «Kompakt».",
                        &[("pages", &pages.to_string()), ("advice", &advice.to_string())],
                    ),
                );
            }
            for w in design.color_warnings() {
                status_line(ui, t.warning, &i18n::tdyn(&w));
            }
            if blocking.is_empty() {
                status_line(ui, t.success, i18n::t("Alt nødvendig er fylt ut. Du bekrefter opplysningene når du laster ned."));
            } else {
                for e in blocking.iter().take(12) {
                    status_line(ui, t.danger, &i18n::tdyn(e));
                }
            }
        });
        ui.add_space(10.0);
        if self.settings.own_checklist {
            self.own_checklist(ui);
            return;
        }
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t("Rekrutterersjekk"));
            theme::hint(ui, i18n::t("Råd fra rekrutterere og søknadssystemer (ATS). Veiledning, ikke en poengsum."));
            ui.add_space(4.0);
            let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 2_629_746).unwrap_or(0) as i32 + 1970 * 12;
            let (list, matched) = checks::audit(&self.project, self.preview.page_count.max(1), today);
            for c in list {
                let color = match c.level {
                    Level::Pass => t.success,
                    Level::Warn => t.warning,
                    Level::Info => t.info,
                };
                status_line(ui, color, &i18n::tdyn(&c.text));
                if c.level != Level::Pass && !c.tip.is_empty() {
                    egui::Frame::new().inner_margin(Margin { left: 16, right: 0, top: 0, bottom: 0 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        theme::hint(ui, &i18n::tdyn(&c.tip));
                    });
                }
            }
            if !matched.found.is_empty() || !matched.missing.is_empty() {
                ui.add_space(6.0);
                theme::label(ui, i18n::t("Fagord fra annonsen"));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for k in &matched.found {
                        theme::pill_wrapped(ui, k, t.success.gamma_multiply(0.14), t.success);
                    }
                    for k in &matched.missing {
                        theme::pill_wrapped(ui, k, t.warning.gamma_multiply(0.14), t.warning).on_hover_text(i18n::t("Står i annonsen, men ikke i CV-en. Ta det med bare hvis det stemmer."));
                    }
                });
                if !matched.missing.is_empty()
                    && ui.button(i18n::t("Spør AI-en om de manglende fagordene")).on_hover_text(i18n::t("AI-en spør hvilke av fagordene du faktisk kan stå for, før noe legges inn")).clicked()
                {
                    self.ask_ai(i18n::t("Hvilke av fagordene fra annonsen som mangler i CV-en kan jeg ta med på en ærlig måte? Spør meg om det du trenger å vite."));
                }
            }
        });
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            Self::view_header(ui, i18n::t("FORHÅNDSVISNING"), &mut self.views.preview);
        });
        let (pages, advice) = (self.preview.page_count, self.recommended_pages());
        if self.is_example || pages > advice {
            ui.vertical_centered(|ui| {
                if self.is_example {
                    theme::pill(ui, i18n::t("Du ser et eksempel. Personen og firmaene er oppdiktet."), t.warning.gamma_multiply(0.14), t.warning);
                }
                if pages > advice {
                    let color = if pages > advice + 1 { t.danger } else { t.warning };
                    theme::pill_wrapped(
                        ui,
                        &i18n::tf(
                            "CV-en er {pages} sider. Vi anbefaler sterkt {advice}: rekrutterere leser raskt, og det viktigste må komme først. Kort ned, eller velg «Kompakt».",
                            &[("pages", &pages.to_string()), ("advice", &advice.to_string())],
                        ),
                        color.gamma_multiply(0.14),
                        color,
                    );
                }
            });
        }
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let fit = (ui.available_width() - 64.0).clamp(240.0, 860.0);
            // 100 % is the A4 page at its real size on screen (595 points wide).
            let width = if self.views.zoom > 0.0 { 595.0 * self.views.zoom } else { fit };
            ui.add_space(16.0);
            // While the pages are made, the note sits in the visible width, not the wide canvas.
            if self.preview.textures.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.label(RichText::new(i18n::t("Lager forhåndsvisning …")).color(t.faint));
                });
                return;
            }
            let full = ui.available_width().max(width + 64.0);
            ui.allocate_ui_with_layout(egui::vec2(full, 0.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                for texture in &self.preview.textures {
                    let size = texture.size_vec2();
                    let scaled = egui::vec2(width, width * size.y / size.x);
                    egui::Frame::new()
                        .fill(egui::Color32::WHITE)
                        .corner_radius(CornerRadius::same(3))
                        .shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: t.shadow })
                        .stroke(Stroke::new(1.0, t.border))
                        .show(ui, |ui| ui.add(egui::Image::new((texture.id(), scaled))));
                    ui.add_space(24.0);
                }
            });
        });
    }

    /// The portrait as a texture for the form, made once per picture.
    pub(crate) fn portrait_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let url = &self.project.candidate.photo;
        if url.is_empty() {
            return None;
        }
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        let key = h.finish();
        if let Some((k, tex)) = &self.portrait {
            if *k == key {
                return Some(tex.clone());
            }
        }
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.decode(url.split_once(',')?.1).ok()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        let tex = ctx.load_texture("portrait-form", color, egui::TextureOptions::LINEAR);
        self.portrait = Some((key, tex.clone()));
        Some(tex)
    }

    fn form_view(&mut self, ui: &mut egui::Ui) {
        Self::view_header(ui, i18n::t("SKJEMA"), &mut self.views.form);
        egui::ScrollArea::vertical().id_salt("form-scroll").auto_shrink([false, false]).show(ui, |ui| {
            // The form follows the panel's width, like the AI and checklist panel: rows stack when narrow.
            ui.set_width(ui.available_width() - 8.0);
            self.form(ui);
        });
    }

    fn side_view(&mut self, ui: &mut egui::Ui) {
        Self::view_header(ui, i18n::t("AI OG SJEKKLISTE"), &mut self.views.side);
        self.side_panel(ui);
    }

    // ---------- dialogs ----------

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.confirm else { return };
        let missing = checks::blocking(&self.project);
        let t = Tokens::get(ctx);
        // 0 = cancel, 1 = download, 2 = show the checklist.
        let response = egui::Modal::new(egui::Id::unique("cvbuilder.confirm")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            if missing.is_empty() {
                ui.heading(i18n::t("Stemmer alt i CV-en?"));
            } else {
                ui.heading(RichText::new(i18n::t("Noe viktig mangler i CV-en")).color(t.warning));
                ui.add(egui::Label::new(i18n::t("Du kan laste ned likevel, men dette bør fylles ut først:")).wrap());
                ui.add_space(4.0);
                for e in missing.iter().take(6) {
                    ui.add(egui::Label::new(RichText::new(format!("• {}", i18n::tdyn(e))).color(t.warning)).wrap());
                }
                if missing.len() > 6 {
                    ui.label(RichText::new(i18n::tf("… og {n} til", &[("n", &(missing.len() - 6).to_string())])).color(t.dim));
                }
                ui.add_space(6.0);
            }
            if !self.project.approvals.facts_confirmed {
                ui.add(egui::Label::new(i18n::t("Les gjennom en siste gang: navn, kontaktinfo, datoer, og at alt som står der er sant – også det AI-en har skrevet.")).wrap());
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let (label, tip) = if missing.is_empty() {
                    (i18n::t("Ja, last ned"), i18n::t("Du bekrefter at alt i CV-en stemmer, og blir ikke spurt igjen for denne CV-en"))
                } else {
                    (i18n::t("Last ned likevel"), i18n::t("Lagre filen selv om noe mangler. Du bekrefter også at det som står i CV-en er sant."))
                };
                if theme::primary(ui, label, tip, true).clicked() {
                    return Some(1u8);
                }
                if !missing.is_empty() && ui.button(i18n::t("Vis sjekkliste")).on_hover_text(i18n::t("Lukk og se hva som mangler i sjekklisten")).clicked() {
                    return Some(2);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Gå tilbake uten å laste ned (Esc)")).clicked() {
                    return Some(0);
                }
                None
            })
            .inner
        });
        let answer = response.inner.or(response.should_close().then_some(0));
        if let Some(answer) = answer {
            self.confirm = None;
            match answer {
                1 => {
                    self.project.approvals.facts_confirmed = true;
                    self.export_now(ctx, kind);
                }
                2 => {
                    self.side = Side::Checks;
                    self.views.side = true;
                    self.status = i18n::t("Ingenting er lastet ned.").into();
                }
                _ => self.status = i18n::t("Ingenting er lastet ned.").into(),
            }
        }
    }

    fn photo_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.photo.as_mut() else {
            return;
        };
        if dialog.stale {
            let img = dialog.edit.render(320);
            let image = egui::ColorImage::from_rgb([img.width() as usize, img.height() as usize], img.as_raw());
            dialog.texture = Some(ctx.load_texture("portrait-preview", image, egui::TextureOptions::LINEAR));
            dialog.stale = false;
        }
        let mut done: Option<bool> = None;
        egui::Modal::new(egui::Id::unique("cvbuilder.photo")).show(ctx, |ui| {
            ui.heading(i18n::t("Tilpass portrettet"));
            theme::hint(ui, i18n::t("Beskjær og gjør små justeringer. Originalfilen endres ikke, og posisjonsdata fra kameraet blir ikke med."));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if let Some(t) = &dialog.texture {
                    egui::Frame::new()
                        .corner_radius(CornerRadius::same(theme::R))
                        .show(ui, |ui| ui.add(egui::Image::new((t.id(), egui::vec2(200.0, 256.0))).corner_radius(CornerRadius::same(theme::R))));
                }
                ui.vertical(|ui| {
                    let e = &mut dialog.edit;
                    let mut moved = false;
                    moved |= ui.add(egui::Slider::new(&mut e.zoom, 1.0..=3.0).text("Zoom")).on_hover_text(i18n::t("Forstørr for å beskjære tettere rundt ansiktet")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.x, -1.0..=1.0).text(i18n::t("Venstre–høyre"))).on_hover_text(i18n::t("Flytt utsnittet mot venstre eller høyre")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.y, -1.0..=1.0).text(i18n::t("Opp–ned"))).on_hover_text(i18n::t("Flytt utsnittet opp eller ned")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.straighten, -10.0..=10.0).text(i18n::t("Rett opp (grader)"))).on_hover_text(i18n::t("Drei bildet litt hvis det heller")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.brightness, -0.3..=0.3).text(i18n::t("Lysstyrke"))).on_hover_text(i18n::t("Gjør bildet lysere eller mørkere")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.contrast, -0.3..=0.3).text(i18n::t("Kontrast"))).on_hover_text(i18n::t("Mer eller mindre forskjell mellom lyst og mørkt")).changed();
                    if ui.button(i18n::t("Tilbakestill")).on_hover_text(i18n::t("Tilbake til utsnittet og justeringene fra start")).clicked() {
                        (e.zoom, e.x, e.y, e.straighten, e.brightness, e.contrast) = (1.0, 0.0, -0.3, 0.0, 0.0, 0.0);
                        moved = true;
                    }
                    dialog.stale |= moved;
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::primary(ui, i18n::t("Bruk bildet"), i18n::t("Legg portrettet inn i CV-en. Originalfilen endres ikke."), true).clicked() {
                    done = Some(true);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre portrettet (Esc)")).clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(apply) = done {
            if apply {
                match dialog.edit.to_data_url() {
                    Ok(url) => {
                        self.project.candidate.photo = url;
                        self.project.options.photo = true;
                        self.photo = None;
                        self.touched();
                        self.status = i18n::t("Portrettet er lagt inn.").into();
                    }
                    Err(e) => self.status = i18n::tf("Bildet kunne ikke lagres: {e}", &[("e", &e)]),
                }
            } else {
                self.photo = None;
            }
        }
    }
}

// ---------- small helpers ----------

/// The app's mark: a rounded accent tile with a document line drawn in it.
fn logo(ui: &mut egui::Ui, t: &Tokens, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let p = ui.painter();
    let k = size / 24.0;
    p.rect_filled(rect, CornerRadius::same((6.0 * k) as u8), t.accent);
    let c = rect.center();
    let stroke = Stroke::new(1.6 * k, t.on_accent);
    for (dy, w) in [(-5.0, 9.0), (0.0, 9.0), (5.0, 6.0)] {
        p.line_segment([egui::pos2(c.x - 4.5 * k, c.y + dy * k), egui::pos2(c.x + (w - 4.5) * k, c.y + dy * k)], stroke);
    }
}

pub(crate) fn status_line(ui: &mut egui::Ui, color: egui::Color32, text: &str) {
    let t = Tokens::get(ui.ctx());
    // Indented text with the dot drawn in the margin: the text always wraps to the panel.
    let r = egui::Frame::new().inner_margin(Margin { left: 16, right: 0, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).size(13.5).color(t.text)).wrap());
    });
    let top = r.response.rect.left_top();
    ui.painter().circle_filled(egui::pos2(top.x + 5.0, top.y + 9.0), 3.5, color);
}

impl eframe::App for CvApp {
    /// Work that must go on while the window is minimized or covered: an AI app connected over
    /// MCP, or a chat answer, keeps changing the CV, and the draft is still saved.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_picked(ctx);
        self.chat_poll();
        self.import_poll();
        if self.import.busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        if let Some(at) = self.save_at {
            if at.elapsed() > Duration::from_millis(800) {
                self.save_at = None;
                self.persist();
            } else {
                ctx.request_repaint_after(Duration::from_millis(850));
            }
        }
        if self.chat.running {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let t = Tokens::get(&ctx);
        self.preview.tick(&ctx, &self.project);
        egui::Panel::top("toolbar").frame(egui::Frame::new().fill(t.panel).inner_margin(Margin { left: 14, right: 14, top: 10, bottom: 6 })).show(ui, |ui| {
            self.ribbon(ui);
        });
        egui::Panel::bottom("status").frame(egui::Frame::new().fill(t.panel).inner_margin(Margin::symmetric(14, 5)).stroke(Stroke::new(1.0, t.border))).show(ui, |ui| self.status_bar(ui));
        let side_frame = |m: Margin| egui::Frame::new().fill(t.panel).inner_margin(m).stroke(Stroke::new(1.0, t.border));
        let v = self.views;
        // The preview fills what is left; without it, the next shown view does.
        let center = if v.preview {
            1
        } else if v.side {
            2
        } else if v.form {
            0
        } else {
            3
        };
        if v.form && center != 0 {
            egui::Panel::left("form")
                .default_size(460.0)
                .min_size(320.0)
                .max_size(1100.0)
                .resizable(true)
                .frame(side_frame(Margin { left: 14, right: 6, top: 8, bottom: 0 }))
                .show(ui, |ui| self.form_view(ui));
        }
        if v.side && center != 2 {
            egui::Panel::right("side").default_size(400.0).min_size(300.0).max_size(1000.0).resizable(true).frame(side_frame(Margin::same(14))).show(ui, |ui| self.side_view(ui));
        }
        match center {
            0 => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.panel).inner_margin(Margin { left: 14, right: 6, top: 8, bottom: 0 })).show(ui, |ui| self.form_view(ui));
            }
            1 => {
                // A gap at each side keeps the preview's scroll bar clear of the panels' resize edges.
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.canvas).inner_margin(Margin { left: 6, right: 6, top: 0, bottom: 0 })).show(ui, |ui| self.preview_panel(ui));
            }
            2 => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.panel).inner_margin(Margin::same(14))).show(ui, |ui| self.side_view(ui));
            }
            _ => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.canvas)).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(RichText::new(i18n::t("Alle visningene er skjult")).size(18.0).family(theme::bold()));
                        theme::hint(ui, i18n::t("Velg hva du vil se under «Vis:» øverst."));
                        if theme::primary(ui, i18n::t("Vis alt"), i18n::t("Vis skjema, forhåndsvisning og AI-panelet igjen"), true).clicked() {
                            self.views = Views::default();
                        }
                    });
                });
            }
        }
        self.confirm_dialog(&ctx);
        self.photo_dialog(&ctx);
        self.translate_dialog(&ctx);
        self.import_dialog(&ctx);
        self.about_dialog(&ctx);
        self.settings_dialog(&ctx);
        self.language_packs(&ctx);
        self.section_name_dialog(&ctx);
        self.checklist_editor(&ctx);
        self.import_cv_dialog(&ctx);
        self.import_rest_dialog(&ctx);
        if let Some(busy) = self.import.busy.clone() {
            egui::Area::new(egui::Id::unique("cvbuilder.import-busy")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -48.0]).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(busy);
                    });
                });
            });
        }
        self.ai_poll(&ctx);
        self.connect_dialog(&ctx);
        self.mcp_help(&ctx);
    }

    fn on_exit(&mut self) {
        self.chat.stop_now();
        self.persist();
    }
}

fn window_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../../../assets/cvbuilder-256.png")).map(|i| i.to_rgba8()).unwrap_or_default();
    egui::IconData { width: image.width(), height: image.height(), rgba: image.into_raw() }
}

/// Starts CVBuilder. Called by each platform's launcher. With `--mcp` it runs as an MCP server
/// on stdin/stdout instead of opening a window (that is how AI apps start it).
pub fn run() -> eframe::Result {
    if std::env::args().any(|a| a == "--mcp") {
        link::serve_stdio();
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("CVBuilder").with_icon(window_icon()).with_inner_size([1440.0, 900.0]).with_min_inner_size([1100.0, 620.0]).with_maximized(true),
        ..Default::default()
    };
    eframe::run_native("CVBuilder", options, Box::new(|cc| Ok(Box::new(CvApp::new(cc)))))
}
