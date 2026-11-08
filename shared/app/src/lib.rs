//! SHARED by Windows, Linux and macOS: the CVBuilder desktop window. Same rules, layouts and
//! project files as the HTML builder, with Word and PDF written by WordCraft from one layout and
//! a preview of those exact pages. The platform folders only start it with [`run`].

pub use eframe::Result;

mod ai_ui;
mod photo;
mod preview;
mod storage;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use cv_core::checks::{self, Level};
use cv_core::layouts::{LAYOUTS, layout};
use cv_core::model::{Certification, Education, Interest, InterestStatus, Job, Skill, SpokenLanguage};
use cv_core::{Language, Project, document};
use egui::{Color32, RichText};

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
}

struct PhotoDialog {
    edit: photo::Edit,
    texture: Option<egui::TextureHandle>,
    stale: bool,
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
}

impl CvApp {
    fn new(cc: &eframe::CreationContext<'_>) -> CvApp {
        let settings = storage::load_settings();
        let (picked_tx, picked_rx) = channel();
        let mut app = CvApp {
            project: Project::example(),
            is_example: true,
            settings,
            buffers: HashMap::new(),
            bad_dates: HashSet::new(),
            preview: preview::Preview::new(cc.egui_ctx.clone()),
            status: "Dette er et eksempel. Velg «Ny tom CV» for å starte på din egen, eller skriv rett inn i eksemplet.".into(),
            undo: None,
            confirm: None,
            picked_tx,
            picked_rx,
            photo: None,
            save_at: None,
            sequence: 0,
            ai: ai_ui::AiState::default(),
        };
        // Developer aid for screenshots and manual testing: CVBUILDER_OPEN=ai opens «Koble til AI».
        if std::env::var("CVBUILDER_OPEN").is_ok_and(|v| v == "ai") {
            app.ai.connect_open = true;
            app.ai.choice = Some(cv_ai::Service::Claude);
        }
        if app.settings.remember {
            if let Some(text) = storage::load_draft() {
                match Project::from_json(&text) {
                    Ok(project) => {
                        app.load(project, false);
                        app.status = "Velkommen tilbake. Utkastet ditt er hentet fra denne maskinen.".into();
                    }
                    Err(_) => {
                        storage::save_backup(&text);
                        app.status = "Det lagrede utkastet kunne ikke åpnes, men er tatt vare på som sikkerhetskopi. Du ser nå eksemplet.".into();
                    }
                }
            }
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
            self.status = "Nå er dette din CV. Den huskes på denne maskinen mens du skriver.".into();
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

    // ---------- file dialogs run on their own thread so the window stays responsive ----------

    fn pick(&self, kind: &'static str, export: Option<Export>) {
        let tx = self.picked_tx.clone();
        let stem = self.file_stem();
        std::thread::spawn(move || {
            let picked = match (kind, export) {
                ("open", _) => rfd::FileDialog::new().add_filter("CVBuilder-utkast", &["json"]).pick_file().map(Picked::OpenProject),
                ("save", _) => rfd::FileDialog::new().add_filter("CVBuilder-utkast", &["json"]).set_file_name(format!("{stem}_utkast.json")).save_file().map(Picked::SaveProject),
                ("photo", _) => rfd::FileDialog::new().add_filter("Bilde", &["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"]).pick_file().map(Picked::Photo),
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
                    Ok(project) => self.replace(project, false, "Utkastet er åpnet. Bekreft opplysningene når du laster ned."),
                    Err(_) => self.status = format!("«{}» kunne ikke åpnes. Her åpner du bare .json-utkast lagret med «Lagre som fil».", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                },
                Picked::SaveProject(path) => {
                    self.status = match self.project.to_json().map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string())) {
                        Ok(()) => format!("Utkastet er lagret i {}. Filen inneholder dine personopplysninger.", path.display()),
                        Err(e) => format!("Kunne ikke lagre: {e}"),
                    }
                }
                Picked::Export(kind, path) => self.write_export(kind, &path),
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
                Export::Word => format!("Word-filen er lagret: {}. Åpne den og se over sidebrytingen før du sender den.", path.display()),
                _ => format!("PDF-en er lagret med ekte, søkbar tekst: {}", path.display()),
            },
            Err(e) => format!("Kunne ikke lagre filen: {e}"),
        };
    }

    fn start_export(&mut self, ctx: &egui::Context, kind: Export) {
        if !self.project.approvals.facts_confirmed {
            self.confirm = Some(kind);
            return;
        }
        match kind {
            Export::Text => {
                ctx.copy_text(checks::plain_text(&self.project));
                self.status = "CV-teksten er kopiert. Lim den inn der søknadsportalen ber om CV som tekst.".into();
            }
            other => self.pick("export", Some(other)),
        }
    }

    // ---------- panels ----------

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("CVBuilder");
            ui.separator();
            if ui.button("Ny tom CV").clicked() {
                let mut blank = Project::default();
                blank.options.layout = self.project.options.layout;
                let text = if self.is_example { "Tom CV klar. Start med kontaktinformasjonen." } else { "Tom CV klar. Den forrige kan hentes tilbake med Angre." };
                self.replace(blank, false, text);
            }
            if ui.button("Vis eksempel").clicked() {
                self.replace(Project::example(), true, "Du ser eksemplet. CV-en din kan hentes tilbake med Angre.");
            }
            if ui.button("Åpne fil…").clicked() {
                self.pick("open", None);
            }
            if ui.button("Lagre som fil…").clicked() {
                self.pick("save", None);
            }
            ui.separator();
            if ui.button(self.ai_label()).on_hover_text("Valgfri hjelp fra Claude, ChatGPT, Gemini eller Grok med din egen API-nøkkel").clicked() {
                self.ai.connect_open = true;
                self.ai.choice = self.settings.ai_service.or(Some(cv_ai::Service::Claude));
            }
            if self.ai_endpoint().is_some() && ui.button(if self.project.is_english() { "AI: Oversett til bokmål" } else { "AI: Oversett til engelsk" }).clicked() {
                let project = self.project.clone();
                self.ai_run("oversetter", move |e, m| cv_ai::tasks::translate(e, m, &project).map(|p| ai_ui::Suggestion::Translation(Box::new(p))));
            }
            ui.separator();
            ui.label("Utforming:");
            for l in &LAYOUTS {
                if ui.selectable_label(self.project.options.layout == l.id, l.name).on_hover_text(l.description).clicked() && self.project.options.layout != l.id {
                    self.project.options.layout = l.id;
                    self.preview.changed();
                    self.save_at = Some(Instant::now());
                }
            }
            ui.separator();
            let before = self.project.language;
            egui::ComboBox::from_id_salt("language").selected_text(if self.project.is_english() { "English" } else { "Bokmål" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.project.language, Language::No, "Bokmål");
                ui.selectable_value(&mut self.project.language, Language::En, "English");
            });
            if before != self.project.language {
                self.touched();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let blocked = !checks::blocking(&self.project).is_empty() || self.preview.page_count > 2;
                let ctx = ui.ctx().clone();
                if ui.add_enabled(!blocked, egui::Button::new(RichText::new("Last ned PDF").strong())).clicked() {
                    self.start_export(&ctx, Export::Pdf);
                }
                if ui.add_enabled(!blocked, egui::Button::new("Last ned Word")).clicked() {
                    self.start_export(&ctx, Export::Word);
                }
                if ui.add_enabled(!blocked, egui::Button::new("Kopier tekst")).on_hover_text("Til portaler som ber deg lime inn CV-en som tekst").clicked() {
                    self.start_export(&ctx, Export::Text);
                }
                let pages = self.preview.page_count;
                ui.label(format!("{pages} A4-side{}", if pages == 1 { "" } else { "r" }));
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(&self.status);
            if let Some((project, example)) = self.undo.clone() {
                if ui.button("Angre").clicked() {
                    self.load(project, example);
                    self.undo = None;
                    self.status = if example { "Eksemplet er tilbake.".into() } else { "CV-en din er tilbake.".into() };
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut remember = self.settings.remember;
                if ui.checkbox(&mut remember, "Husk utkastet på denne maskinen").changed() {
                    self.settings.remember = remember;
                    storage::save_settings(&self.settings);
                    if remember {
                        self.persist();
                    } else {
                        storage::forget_draft();
                        self.status = "Utkastet huskes ikke lenger. Lagre som fil før du lukker.".into();
                    }
                }
                if storage::portable() {
                    ui.label(RichText::new("Bærbar modus").weak());
                }
            });
        });
    }

    fn checklist(&mut self, ui: &mut egui::Ui) {
        let blocking = checks::blocking(&self.project);
        ui.heading("Før nedlasting");
        if blocking.is_empty() {
            ui.colored_label(Color32::from_rgb(0x2f, 0x8a, 0x52), "Alt nødvendig er fylt ut. Du bekrefter opplysningene når du laster ned.");
        } else {
            for e in blocking.iter().take(12) {
                ui.colored_label(Color32::from_rgb(0xc0, 0x45, 0x45), format!("• {e}"));
            }
        }
        ui.add_space(10.0);
        ui.heading("Rekrutterersjekk");
        ui.label(RichText::new("Råd fra rekrutterere og søknadssystemer. Veiledning, ikke en poengsum.").weak());
        let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 2_629_746).unwrap_or(0) as i32 + 1970 * 12;
        let (list, matched) = checks::audit(&self.project, self.preview.page_count.max(1), today);
        for c in list {
            let (mark, color) = match c.level {
                Level::Pass => ("•", Color32::from_rgb(0x2f, 0x8a, 0x52)),
                Level::Warn => ("!", Color32::from_rgb(0xc0, 0x7a, 0x1e)),
                Level::Info => ("i", Color32::from_rgb(0x4a, 0x7d, 0xb5)),
            };
            ui.colored_label(color, format!("{mark} {}", c.text));
            if c.level != Level::Pass && !c.tip.is_empty() {
                ui.label(RichText::new(c.tip).small());
            }
        }
        if !matched.found.is_empty() || !matched.missing.is_empty() {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for k in &matched.found {
                    ui.label(RichText::new(k).small().color(Color32::from_rgb(0x2f, 0x8a, 0x52)));
                }
                for k in &matched.missing {
                    ui.label(RichText::new(k).small().strikethrough().color(Color32::from_rgb(0xc0, 0x7a, 0x1e)));
                }
            });
        }
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        if self.is_example {
            ui.colored_label(Color32::from_rgb(0xc0, 0x7a, 0x1e), "Du ser et eksempel. Personen og firmaene er oppdiktet.");
        }
        if self.preview.page_count > 2 {
            let fits: Vec<&str> = LAYOUTS.iter().filter(|l| l.id != self.project.options.layout).map(|l| l.name).collect();
            ui.colored_label(Color32::from_rgb(0xc0, 0x45, 0x45), format!("CV-en blir {} sider. Maks to: kort ned teksten, eller prøv {}.", self.preview.page_count, fits.join(" eller ")));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 24.0).clamp(200.0, 900.0);
            ui.vertical_centered(|ui| {
                for texture in &self.preview.textures {
                    let size = texture.size_vec2();
                    let scaled = egui::vec2(width, width * size.y / size.x);
                    ui.add(egui::Image::new((texture.id(), scaled)));
                    ui.add_space(16.0);
                }
                if self.preview.textures.is_empty() {
                    ui.label("Lager forhåndsvisning …");
                }
            });
        });
    }

    // ---------- the form ----------

    fn form(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        egui::CollapsingHeader::new(RichText::new("Kontakt og profil").strong()).default_open(true).show(ui, |ui| {
            let c = &mut self.project.candidate;
            changed |= field(ui, "Fullt navn", &mut c.name);
            changed |= field(ui, "Faglig overskrift · bruk stillingens ord", &mut c.headline);
            changed |= field(ui, "E-post", &mut c.email);
            changed |= field(ui, "Telefon med landskode (+47 …)", &mut c.phone);
            changed |= field(ui, "By og land", &mut c.city);
            changed |= field(ui, "LinkedIn · full adresse (anbefalt)", &mut c.linkedin);
            changed |= area(ui, "Kort profil · 30–70 ord", &mut c.summary, 5);
            if ui.button("AI: Skriv forslag til profil").on_hover_text("Lager en profil ut fra det som allerede står i CV-en").clicked() {
                let project = self.project.clone();
                self.ai_run("skriver profil", move |e, m| cv_ai::tasks::write_summary(e, m, &project).map(ai_ui::Suggestion::Summary));
            }
            ui.horizontal(|ui| {
                if ui.button(if self.project.candidate.photo.is_empty() { "Legg til portrett…" } else { "Bytt portrett…" }).clicked() {
                    self.pick("photo", None);
                }
                if !self.project.candidate.photo.is_empty() {
                    changed |= ui.checkbox(&mut self.project.options.photo, "Vis portrett i CV-en").changed();
                }
            });
        });
        egui::CollapsingHeader::new(RichText::new("Stillingen").strong()).default_open(true).show(ui, |ui| {
            let t = &mut self.project.target;
            changed |= field(ui, "Stilling du søker", &mut t.role);
            changed |= field(ui, "Arbeidsgiver", &mut t.company);
            changed |= field(ui, "Annonselenke (https://…)", &mut t.url);
            changed |= area(ui, "Lim inn hele stillingsannonsen", &mut t.description, 5);
            changed |= lines(ui, &mut self.buffers, "keywords", "Viktige fagord fra annonsen · ett per linje", &mut self.project.target.keywords);
            if ui.button("AI: Finn fagord i annonsen").clicked() {
                let project = self.project.clone();
                self.ai_run("leser annonsen", move |e, m| cv_ai::tasks::keywords(e, m, &project).map(|k| ai_ui::Suggestion::Keywords(k.join("\n"))));
            }
            if ui.button("Foreslå fagord fra annonsen").clicked() {
                let mut merged = self.project.target.keywords.clone();
                for k in checks::suggest_keywords(&self.project.target.description, 25) {
                    if !merged.contains(&k) {
                        merged.push(k);
                    }
                }
                merged.truncate(30);
                self.project.target.keywords = merged;
                self.buffers.remove("keywords");
                self.status = "Fagord foreslått. Fjern ord som ikke er relevante.".into();
                changed = true;
            }
        });
        egui::CollapsingHeader::new(RichText::new("Kjernekompetanse").strong()).default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Bruk annonsens ordlyd der den stemmer. Bare det du kan stå for.").weak());
            let mut remove = None;
            for (i, s) in self.project.skills.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= ui.add(egui::TextEdit::singleline(&mut s.group).hint_text("Gruppe").desired_width(140.0)).changed();
                    changed |= ui.add(egui::TextEdit::singleline(&mut s.name).hint_text("Kompetanse eller verktøy").desired_width(f32::INFINITY)).changed();
                    if ui.small_button("✕").on_hover_text("Fjern").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.project.skills.remove(i);
                changed = true;
            }
            if ui.button("+ Legg til kompetanse").clicked() {
                let group = self.project.skills.last().map(|s| s.group.clone()).unwrap_or_default();
                self.project.skills.push(Skill { group, name: String::new(), evidence: String::new(), confirmed: true });
                changed = true;
            }
        });
        egui::CollapsingHeader::new(RichText::new("Arbeidserfaring").strong()).default_open(true).show(ui, |ui| {
            ui.label(RichText::new("2–4 korte punkter per jobb. Start med et verb og vis resultatet: «Oppnådde X, målt ved Y, ved å gjøre Z».").weak());
            let mut remove = None;
            let mut ai_job: Option<usize> = None;
            for i in 0..self.project.experience.len() {
                let title = {
                    let j = &self.project.experience[i];
                    if j.company.is_empty() { format!("Jobb {}", i + 1) } else { format!("{} · {}", j.title, j.company) }
                };
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(title);
                        if ui.small_button("Fjern").clicked() {
                            remove = Some(i);
                        }
                    });
                    let key = self.project.experience[i].id.clone();
                    let j = &mut self.project.experience[i];
                    changed |= field(ui, "Stillingstittel", &mut j.title);
                    changed |= field(ui, "Arbeidsgiver", &mut j.company);
                    changed |= field(ui, "Sted · by", &mut j.location);
                    changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &format!("{key}.start"), "Fra · måned og år (09.2021 eller 2010)", &mut j.start, false);
                    let mut current = j.end.is_none();
                    if ui.checkbox(&mut current, "Nåværende jobb").changed() {
                        j.end = if current { None } else { Some(String::new()) };
                        changed = true;
                    }
                    let end_key = format!("{key}.end");
                    if self.buffers.get(&end_key).is_some_and(|b| checks::is_now(b)) {
                        // «nå» typed as an end date: this is the current job.
                        j.end = None;
                        self.buffers.remove(&end_key);
                        self.bad_dates.remove(&end_key);
                        changed = true;
                    }
                    if let Some(end) = j.end.as_mut() {
                        changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &end_key, "Til · måned og år (eller «nå»)", end, false);
                    }
                    changed |= lines(ui, &mut self.buffers, &format!("{key}.bullets"), "Hva du gjorde og oppnådde · ett punkt per linje", &mut j.bullets);
                    if ui.small_button("AI: Forbedre punktene").clicked() {
                        ai_job = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.project.experience.remove(i);
                changed = true;
            }
            if let Some(i) = ai_job {
                let project = self.project.clone();
                let job_id = project.experience[i].id.clone();
                self.ai_run("forbedrer punktene", move |e, m| cv_ai::tasks::improve_bullets(e, m, &project, i).map(|b| ai_ui::Suggestion::Bullets { job_id, text: b.join("\n") }));
            }
            if ui.button("+ Legg til jobb").clicked() {
                let id = self.next_id("job");
                self.project.experience.push(Job { id, end: Some(String::new()), confirmed: true, ..Default::default() });
                changed = true;
            }
        });
        egui::CollapsingHeader::new(RichText::new("Utdanning").strong()).default_open(false).show(ui, |ui| {
            let mut remove = None;
            for (i, e) in self.project.education.iter_mut().enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    changed |= field(ui, "Grad, fagbrev eller studium", &mut e.qualification);
                    changed |= field(ui, "Lærested", &mut e.institution);
                    ui.horizontal(|ui| {
                        changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &format!("edu{i}.start"), "Fra · år", &mut e.start, true);
                        changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &format!("edu{i}.end"), "Til · år", &mut e.end, true);
                    });
                    if ui.small_button("Fjern").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.project.education.remove(i);
                self.buffers.retain(|k, _| !k.starts_with("edu"));
                changed = true;
            }
            if ui.button("+ Legg til utdanning").clicked() {
                self.project.education.push(Education::default());
                changed = true;
            }
        });
        egui::CollapsingHeader::new(RichText::new("Kurs og sertifiseringer").strong()).default_open(false).show(ui, |ui| {
            let mut remove = None;
            for (i, c) in self.project.certifications.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= ui.add(egui::TextEdit::singleline(&mut c.name).hint_text("Kurs eller sertifisering").desired_width(200.0)).changed();
                    changed |= ui.add(egui::TextEdit::singleline(&mut c.issuer).hint_text("Utsteder").desired_width(120.0)).changed();
                    changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &format!("cert{i}.year"), "År", &mut c.year, true);
                    if ui.small_button("✕").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.project.certifications.remove(i);
                self.buffers.retain(|k, _| !k.starts_with("cert"));
                changed = true;
            }
            if ui.button("+ Legg til kurs").clicked() {
                self.project.certifications.push(Certification::default());
                changed = true;
            }
        });
        egui::CollapsingHeader::new(RichText::new("Språk").strong()).default_open(false).show(ui, |ui| {
            let mut remove = None;
            for (i, l) in self.project.languages.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= ui.add(egui::TextEdit::singleline(&mut l.name).hint_text("Språk").desired_width(120.0)).changed();
                    changed |= ui.add(egui::TextEdit::singleline(&mut l.level).hint_text("Nivå: morsmål, flytende, B2 …").desired_width(f32::INFINITY)).changed();
                    if ui.small_button("✕").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.project.languages.remove(i);
                changed = true;
            }
            if ui.button("+ Legg til språk").clicked() {
                self.project.languages.push(SpokenLanguage::default());
                changed = true;
            }
        });
        egui::CollapsingHeader::new(RichText::new("Interesser").strong()).default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Valgfritt. Ta med høyst tre korte, ekte interesser.").weak());
            changed |= ui.checkbox(&mut self.project.options.include_interests, "Ta med interesser i CV-en").changed();
            let selected = self.project.options.interest_ids.clone();
            let mut remove = None;
            for (i, item) in self.project.interests.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let mut on = selected.contains(&item.id);
                    if ui.add_enabled(on || selected.len() < 3, egui::Checkbox::without_text(&mut on)).on_hover_text("Ta med i CV-en").changed() {
                        if on {
                            self.project.options.interest_ids.push(item.id.clone());
                        } else {
                            self.project.options.interest_ids.retain(|x| x != &item.id);
                        }
                        changed = true;
                    }
                    changed |= ui.add(egui::TextEdit::singleline(&mut item.label).desired_width(180.0)).changed();
                    egui::ComboBox::from_id_salt(("status", i))
                        .selected_text(match item.status {
                            InterestStatus::Current => "Nå",
                            InterestStatus::Former => "Tidligere",
                            InterestStatus::Interest => "Interesse for",
                        })
                        .show_ui(ui, |ui| {
                            changed |= ui.selectable_value(&mut item.status, InterestStatus::Current, "Nå").changed();
                            changed |= ui.selectable_value(&mut item.status, InterestStatus::Former, "Tidligere").changed();
                            changed |= ui.selectable_value(&mut item.status, InterestStatus::Interest, "Interesse for").changed();
                        });
                    if ui.small_button("✕").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                let id = self.project.interests.remove(i).id;
                self.project.options.interest_ids.retain(|x| x != &id);
                changed = true;
            }
            if ui.button("+ Legg til interesse").clicked() {
                let id = self.next_id("interest");
                self.project.interests.push(Interest { id, confirmed: true, category: "other".into(), specificity: 1, ..Default::default() });
                changed = true;
            }
        });
        if changed {
            self.touched();
        }
    }

    // ---------- dialogs ----------

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.confirm else { return };
        let response = egui::Modal::new(egui::Id::unique("cvbuilder.confirm")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Stemmer alt i CV-en?");
            ui.label("Les gjennom en siste gang: navn, kontaktinfo, datoer, og at alt du skriver er sant.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Ja, last ned").strong()).clicked() {
                    return Some(true);
                }
                if ui.button("Avbryt").clicked() {
                    return Some(false);
                }
                None
            })
            .inner
        });
        let answer = response.inner.or(response.should_close().then_some(false));
        if let Some(yes) = answer {
            self.confirm = None;
            if yes {
                self.project.approvals.facts_confirmed = true;
                self.start_export(ctx, kind);
            } else {
                self.status = "Ingenting er lastet ned.".into();
            }
        }
    }

    fn photo_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.photo.as_mut() else { return };
        if dialog.stale {
            let img = dialog.edit.render(320);
            let image = egui::ColorImage::from_rgb([img.width() as usize, img.height() as usize], img.as_raw());
            dialog.texture = Some(ctx.load_texture("portrait-preview", image, egui::TextureOptions::LINEAR));
            dialog.stale = false;
        }
        let mut done: Option<bool> = None;
        egui::Modal::new(egui::Id::unique("cvbuilder.photo")).show(ctx, |ui| {
            ui.heading("Tilpass portrettet");
            ui.label(RichText::new("Beskjær og gjør små justeringer. Originalfilen endres ikke, og posisjonsdata fra kameraet blir ikke med.").weak());
            ui.horizontal(|ui| {
                if let Some(t) = &dialog.texture {
                    ui.add(egui::Image::new((t.id(), egui::vec2(200.0, 256.0))));
                }
                ui.vertical(|ui| {
                    let e = &mut dialog.edit;
                    let mut moved = false;
                    moved |= ui.add(egui::Slider::new(&mut e.zoom, 1.0..=3.0).text("Zoom")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.x, -1.0..=1.0).text("Venstre–høyre")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.y, -1.0..=1.0).text("Opp–ned")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.straighten, -10.0..=10.0).text("Rett opp (grader)")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.brightness, -0.3..=0.3).text("Lysstyrke")).changed();
                    moved |= ui.add(egui::Slider::new(&mut e.contrast, -0.3..=0.3).text("Kontrast")).changed();
                    if ui.button("Tilbakestill").clicked() {
                        (e.zoom, e.x, e.y, e.straighten, e.brightness, e.contrast) = (1.0, 0.0, -0.3, 0.0, 0.0, 0.0);
                        moved = true;
                    }
                    dialog.stale |= moved;
                });
            });
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Bruk bildet").strong()).clicked() {
                    done = Some(true);
                }
                if ui.button("Avbryt").clicked() {
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
                        self.status = "Portrettet er lagt inn.".into();
                    }
                    Err(e) => self.status = format!("Bildet kunne ikke lagres: {e}"),
                }
            } else {
                self.photo = None;
            }
        }
    }
}

// ---------- small form helpers ----------

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY)).changed()
}

fn area(ui: &mut egui::Ui, label: &str, value: &mut String, rows: usize) -> bool {
    ui.label(label);
    ui.add(egui::TextEdit::multiline(value).desired_rows(rows).desired_width(f32::INFINITY)).changed()
}

/// A list typed one item per line. The raw text is kept while typing, so new lines work.
fn lines(ui: &mut egui::Ui, buffers: &mut HashMap<String, String>, key: &str, label: &str, list: &mut Vec<String>) -> bool {
    ui.label(label);
    let buffer = buffers.entry(key.to_string()).or_insert_with(|| list.join("\n"));
    if ui.add(egui::TextEdit::multiline(buffer).desired_rows(3).desired_width(f32::INFINITY)).changed() {
        *list = buffer.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        return true;
    }
    false
}

/// A month or year typed the way people write it. Only a readable date is ever stored.
fn date(ui: &mut egui::Ui, buffers: &mut HashMap<String, String>, bad: &mut HashSet<String>, key: &str, label: &str, value: &mut String, year: bool) -> bool {
    let shown = |v: &str| if year { v.to_string() } else { checks::show_month(v) };
    let mut changed = false;
    ui.vertical(|ui| {
        ui.label(label);
        let buffer = buffers.entry(key.to_string()).or_insert_with(|| shown(value));
        let response = ui.add(egui::TextEdit::singleline(buffer).hint_text(if year { "2021" } else { "09.2021" }).desired_width(if year { 70.0 } else { 120.0 }));
        if response.lost_focus() || (response.changed() && !response.has_focus()) {
            let parsed = if year { checks::parse_year(buffer) } else { checks::parse_month(buffer) };
            match parsed {
                Some(p) => {
                    bad.remove(key);
                    if &p != value {
                        *value = p.clone();
                        changed = true;
                    }
                    *buffer = shown(&p);
                }
                None => {
                    bad.insert(key.to_string());
                }
            }
        }
        if bad.contains(key) {
            ui.colored_label(Color32::from_rgb(0xc0, 0x45, 0x45), if year { "Skriv årstall med fire siffer, for eksempel 2021." } else { "Skriv måned og år, for eksempel 09.2021." });
        }
    });
    changed
}

impl eframe::App for CvApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_picked(&ctx);
        self.preview.tick(&ctx, &self.project);
        if let Some(at) = self.save_at {
            if at.elapsed() > Duration::from_millis(800) {
                self.save_at = None;
                self.persist();
            } else {
                ctx.request_repaint_after(Duration::from_millis(850));
            }
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("form").default_size(440.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.form(ui));
        });
        egui::Panel::right("checks").default_size(320.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.checklist(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| self.preview_panel(ui));
        self.confirm_dialog(&ctx);
        self.photo_dialog(&ctx);
        self.ai_poll(&ctx);
        self.connect_dialog(&ctx);
        self.review_dialog(&ctx);
        if let Some(busy) = self.ai.busy.clone() {
            egui::Area::new(egui::Id::unique("cvbuilder.busy")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -40.0]).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(busy);
                    });
                });
            });
        }
    }

    fn on_exit(&mut self) {
        self.persist();
    }
}

fn window_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../../assets/cvbuilder-256.png")).map(|i| i.to_rgba8()).unwrap_or_default();
    egui::IconData { width: image.width(), height: image.height(), rgba: image.into_raw() }
}

/// Opens the CVBuilder window. Called by each platform's launcher.
pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("CVBuilder").with_icon(window_icon()).with_inner_size([1280.0, 800.0]).with_min_inner_size([900.0, 560.0]).with_maximized(true),
        ..Default::default()
    };
    eframe::run_native("CVBuilder", options, Box::new(|cc| Ok(Box::new(CvApp::new(cc)))))
}

#[allow(dead_code)]
fn layout_name(project: &Project) -> &'static str {
    layout(project.options.layout).name
}
