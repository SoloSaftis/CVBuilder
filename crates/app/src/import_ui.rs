//! Bringing things in: an existing CV (PDF, Word, text, web page or a CVBuilder file), with or
//! without AI, and a job ad from the clipboard, a link or a file. Reading runs on a worker, so
//! the window never stops.
use std::sync::mpsc::{Receiver, channel};

use cv_core::Project;
use cv_core::import::{self, AdInfo};
use egui::{CornerRadius, Margin, RichText};

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Tokens};

pub enum Outcome {
    /// The text of an existing CV, and whether the AI should place it.
    Cv(Result<String, String>, bool),
    Ad(Result<AdInfo, String>),
}

#[derive(Default)]
pub struct ImportState {
    pub cv_open: bool,
    pub use_ai: bool,
    pub busy: Option<String>,
    inbox: Option<Receiver<Outcome>>,
    /// What could not be filled in without AI, shown for the person to place.
    pub rest: Option<String>,
    /// The last result of reading an ad, shown on the ad card.
    pub ad_note: String,
    pub focus_ad: bool,
}

const AI_LIMIT: usize = 20_000;

impl CvApp {
    fn run_import(&mut self, what: &str, job: impl FnOnce() -> Outcome + Send + 'static) {
        let (tx, rx) = channel();
        self.import.inbox = Some(rx);
        self.import.busy = Some(what.to_string());
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
    }

    /// A picked existing CV. CVBuilder files open as they are; other files are read as text.
    pub(crate) fn import_cv_file(&mut self, path: std::path::PathBuf) {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Project::from_json(&t).map_err(|e| e.to_string())) {
                Ok(p) => self.replace(p, false, i18n::t("CV-en er hentet inn. Den forrige kan hentes tilbake med Angre.")),
                Err(_) => self.status = i18n::t("JSON-filen er ikke et CVBuilder-utkast.").into(),
            }
            return;
        }
        let use_ai = self.import.use_ai && self.chat_backend().is_some();
        self.run_import(i18n::t("Leser CV-en …"), move || Outcome::Cv(import::text_from_file(&path), use_ai));
    }

    pub(crate) fn import_ad_file(&mut self, path: std::path::PathBuf) {
        self.run_import(i18n::t("Leser annonsen …"), move || Outcome::Ad(import::ad_from_file(&path)));
    }

    pub(crate) fn fetch_ad(&mut self) {
        let url = self.project.target.url.trim().to_string();
        if url.is_empty() {
            self.import.ad_note = i18n::t("Lim inn lenken til annonsen i feltet over først.").into();
            return;
        }
        self.run_import(i18n::t("Henter annonsen …"), move || Outcome::Ad(cv_ai::fetch_page(&url).and_then(|html| import::ad_from_page(&html))));
    }

    /// Reads the ad text in the form (pasted by the person) and fills in what it can.
    /// `previous` is the text the box had before a paste: what CVBuilder filled from that ad
    /// may then make way for the new one, while what the person typed stays.
    pub(crate) fn read_pasted_ad(&mut self, previous: Option<String>) {
        if self.project.target.description.trim().is_empty() {
            self.import.ad_note = i18n::t("Lim inn annonseteksten i feltet først, eller hent den fra lenken eller en fil.").into();
            return;
        }
        let mut ad = import::read_ad(&self.project.target.description);
        // The box keeps the text exactly as the person pasted it.
        ad.description = self.project.target.description.clone();
        self.apply_ad(ad, previous);
    }

    fn apply_ad(&mut self, ad: AdInfo, previous: Option<String>) {
        // Undo goes back to just before the filling in; a pasted text stays.
        let before = (self.project.clone(), self.is_example);
        if let Some(previous) = previous {
            self.project.target.description = previous;
        }
        let note = i18n::tdyn(&import::apply_ad(&mut self.project, &ad));
        self.undo = Some(before);
        self.buffers.remove("keywords");
        self.folded.remove("target");
        self.import.ad_note = note.clone();
        self.status = i18n::tf("{note} Angre hvis noe ble feil.", &[("note", &note)]);
        self.touched();
    }

    pub(crate) fn import_poll(&mut self) {
        let Some(inbox) = &self.import.inbox else {
            return;
        };
        let outcome = match inbox.try_recv() {
            Ok(outcome) => outcome,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            // The worker stopped without an answer (a reader that crashed): never stay busy.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                let cv = self.import.busy.as_deref() == Some(i18n::t("Leser CV-en …"));
                self.import.inbox = None;
                self.import.busy = None;
                let e = i18n::t("Noe gikk galt under lesingen. Prøv igjen, eller kopier teksten og lim den inn.").to_string();
                if cv {
                    self.status = e;
                } else {
                    self.import.ad_note = e;
                }
                return;
            }
        };
        self.import.inbox = None;
        self.import.busy = None;
        match outcome {
            Outcome::Ad(Ok(ad)) => self.apply_ad(ad, None),
            Outcome::Ad(Err(e)) => {
                // The message is on the ad card, so the card must be open to show it.
                self.folded.remove("target");
                self.import.ad_note = i18n::tdyn(&e);
            }
            Outcome::Cv(Err(e), _) => self.status = i18n::tdyn(&e),
            Outcome::Cv(Ok(text), true) => {
                let mut blank = Project::default();
                blank.options = self.project.options.clone();
                blank.options.section_order.clear();
                blank.options.section_colors.clear();
                blank.target = self.project.target.clone();
                self.replace(blank, false, i18n::t("Ny CV fra filen. AI-en fyller den ut nå; den forrige kan hentes tilbake med Angre."));
                let cut: String = text.chars().take(AI_LIMIT).collect();
                let ask = i18n::tf(
                    "Her er teksten fra en CV jeg har fra før. Fyll ut CV-en i CVBuilder med disse opplysningene: kontaktinfo, overskrift, profil, alle jobber med datoer og punkter, utdanning, kompetanse, kurs, språk og interesser. Bruk bare det som står i teksten, og skriv punktene kort med verb først. Behold alle tall. Si til slutt hva jeg bør sjekke.\n\n---\n{cut}",
                    &[("cut", &cut)],
                );
                self.ask_ai(&ask);
            }
            Outcome::Cv(Ok(text), false) => {
                let read = import::read_cv(&text);
                let mut p = read.project;
                p.options = self.project.options.clone();
                p.options.section_order.clear();
                p.options.section_colors.clear();
                p.target = self.project.target.clone();
                let found = if read.found.is_empty() { i18n::t("lite").to_string() } else { read.found.iter().map(|f| i18n::tdyn(f)).collect::<Vec<_>>().join(", ") };
                self.replace(p, false, &i18n::tf("Hentet inn {found}. Den forrige CV-en kan hentes tilbake med Angre.", &[("found", &found)]));
                if !read.rest.trim().is_empty() {
                    self.import.rest = Some(read.rest);
                }
            }
        }
    }

    /// «Importer CV»: which files, with or without AI.
    pub(crate) fn import_cv_dialog(&mut self, ctx: &egui::Context) {
        if !self.import.cv_open {
            return;
        }
        let t = Tokens::get(ctx);
        let has_ai = self.chat_backend().is_some();
        if !has_ai {
            self.import.use_ai = false;
        }
        let mut close = false;
        let mut pick = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.import-cv")).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.heading(i18n::t("Hent inn en CV du har fra før"));
            ui.add_space(4.0);
            egui::Frame::new().fill(t.accent_soft).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(i18n::t("Filer som kan leses")).family(theme::medium()));
                ui.add(egui::Label::new(i18n::t("PDF med ekte tekst (ikke skannet) · Word (.docx, .doc) · OpenDocument (.odt) · RTF · Pages · tekst (.txt, .md) · nettside (.html, .mht) · CVBuilder-utkast (.json)")).wrap());
            });
            ui.add_space(8.0);
            ui.add_enabled_ui(has_ai, |ui| {
                ui.radio_value(&mut self.import.use_ai, true, i18n::t("Med AI – fyller ut alt, også jobber og utdanning"))
                    .on_hover_text(i18n::t("AI-en leser hele CV-en og fyller ut alle delene. Teksten sendes til AI-tjenesten."))
                    .on_disabled_hover_text(i18n::t("Koble til AI under AI-fanen for å bruke dette."));
            });
            if !has_ai {
                ui.label(RichText::new(i18n::t("Koble til AI under AI-fanen for å bruke dette.")).size(12.0).color(t.faint));
            }
            ui.radio_value(&mut self.import.use_ai, false, i18n::t("Uten AI – på denne maskinen: kontaktinfo, profil, jobber, utdanning, kompetanse, språk, kurs og interesser."))
                .on_hover_text(i18n::t("Leses på denne maskinen, og ingenting sendes. Sjekk resultatet; det som ikke kunne plasseres, vises til slutt."));
            ui.add_space(6.0);
            theme::hint(ui, i18n::t("Du får en ny CV med designet og fargene du har valgt. Den du har nå kan hentes tilbake med Angre. Med AI sendes teksten til tjenesten du har koblet til."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::primary(ui, i18n::t("Velg fil"), i18n::t("Velg CV-filen du vil hente inn"), true).clicked() {
                    pick = true;
                    close = true;
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å endre noe (Esc)")).clicked() {
                    close = true;
                }
            });
        });
        if pick {
            self.pick("import_cv", None);
        }
        if close || r.should_close() {
            self.import.cv_open = false;
        }
    }

    /// What reading without AI could not place, for the person to copy or hand to the AI.
    pub(crate) fn import_rest_dialog(&mut self, ctx: &egui::Context) {
        let Some(rest) = self.import.rest.clone() else {
            return;
        };
        let has_ai = self.chat_backend().is_some();
        let mut close = false;
        let mut to_ai = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.import-rest")).show(ctx, |ui| {
            ui.set_max_width(600.0);
            ui.heading(i18n::t("Dette må plasseres"));
            ui.add(
                egui::Label::new(i18n::t(
                    "Det meste er fylt inn. Dette kunne ikke plasseres sikkert – legg det inn selv i skjemaet (kopier fra teksten under), eller la AI-en gjøre det.",
                ))
                .wrap(),
            );
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(&rest).size(12.5)).wrap().selectable(true));
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if has_ai && theme::primary(ui, i18n::t("La AI-en legge det inn"), i18n::t("AI-en legger jobbene, utdanningen og resten inn i skjemaet"), true).clicked() {
                    to_ai = true;
                }
                if ui.button(i18n::t("Kopier teksten")).on_hover_text(i18n::t("Kopier teksten, så du kan lime den inn i skjemaet selv")).clicked() {
                    ui.ctx().copy_text(rest.clone());
                }
                if ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk vinduet (Esc)")).clicked() {
                    close = true;
                }
            });
        });
        if to_ai {
            let cut: String = rest.chars().take(AI_LIMIT).collect();
            self.ask_ai(&i18n::tf(
                "Legg inn disse delene fra CV-en min (jobber med datoer og punkter, utdanning, kurs og annet). Bruk bare det som står her og behold alle tall:\n\n---\n{cut}",
                &[("cut", &cut)],
            ));
            close = true;
        }
        if close || r.should_close() {
            self.import.rest = None;
        }
    }
}
