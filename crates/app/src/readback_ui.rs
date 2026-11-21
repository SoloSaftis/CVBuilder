//! «Hva et ATS leser» (040): the CV is exported as it would be downloaded, read back with the
//! app's own importer, and compared field by field, so the person sees what an applicant
//! tracking system gets. Runs on a worker thread: building and reading a PDF takes a moment.
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use cv_core::readback::{self, Field, ReadBack, State};
use cv_core::{Project, document};

use crate::checklist_ui::{Status, status_line};
use crate::i18n;
use crate::theme;
use crate::CvApp;

#[derive(Default)]
pub struct ReadBackState {
    inbox: Option<Receiver<Result<ReadBack, String>>>,
    result: Option<Result<ReadBack, String>>,
    /// The design and file kind the result was read from («PDF», «Word»).
    from: String,
}

impl ReadBackState {
    /// Shows a result at once (the screen check uses it).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn show(&mut self, result: Result<ReadBack, String>, from: &str) {
        self.inbox = None;
        self.result = Some(result);
        self.from = from.to_string();
    }
}

/// Exports `project` as PDF or Word and reads it back.
pub fn run(project: &Project, word: bool) -> Result<ReadBack, String> {
    let doc = document::build(project);
    let (bytes, ext) = if word { (document::to_docx(&doc), "docx") } else { (document::to_pdf(&doc), "pdf") };
    let bytes = bytes.map_err(|e| cv_core::plain::with_details(cv_core::plain::TRY_AGAIN, &e.to_string()))?;
    readback::read_back(project, &bytes, ext)
}

/// The field's name as the form shows it, with the job or education it belongs to.
fn field_name(field: Field) -> String {
    let job = |n: usize| i18n::tf("Jobb {n}", &[("n", &n.to_string())]);
    let study = |n: usize| i18n::tf("Utdanning {n}", &[("n", &n.to_string())]);
    match field {
        Field::Name => i18n::tdyn("Navn"),
        Field::Email => i18n::t("E-post").into(),
        Field::Phone => i18n::t("Telefon").into(),
        Field::City => i18n::t("Sted").into(),
        Field::LinkedIn => i18n::tdyn("LinkedIn"),
        Field::JobTitle(n) => format!("{} · {}", job(n), i18n::t("Stilling")),
        Field::Employer(n) => format!("{} · {}", job(n), i18n::t("Arbeidsgiver")),
        Field::JobDates(n) => format!("{} · {}", job(n), i18n::t("Tidsrom")),
        Field::Qualification(n) => format!("{} · {}", study(n), i18n::t("Grad, fagbrev eller studium")),
        Field::School(n) => format!("{} · {}", study(n), i18n::t("Lærested")),
        Field::StudyDates(n) => format!("{} · {}", study(n), i18n::t("Tidsrom")),
    }
}

impl CvApp {
    fn start_read_back(&mut self, word: bool) {
        let (tx, rx) = channel();
        let project = self.project.clone();
        let ctx = self.egui_ctx.clone();
        self.readback.inbox = Some(rx);
        self.readback.result = None;
        let design = cv_core::layouts::effective(&self.project);
        self.readback.from = format!("{} · {}", i18n::tdyn(&design.name), if word { "Word" } else { "PDF" });
        std::thread::spawn(move || {
            let _ = tx.send(run(&project, word));
            ctx.request_repaint();
        });
    }

    fn read_back_poll(&mut self) {
        let Some(inbox) = &self.readback.inbox else { return };
        match inbox.try_recv() {
            Ok(result) => self.readback.result = Some(result),
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => self.readback.result = Some(Err(i18n::t("Noe gikk galt under lesingen. Prøv igjen, eller kopier teksten og lim den inn.").into())),
        }
        self.readback.inbox = None;
    }

    /// The card in the checklist panel.
    pub(crate) fn read_back_card(&mut self, ui: &mut egui::Ui) {
        self.read_back_poll();
        let busy = self.readback.inbox.is_some();
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t("Hva et ATS leser"));
            theme::hint(
                ui,
                i18n::t("Leser filen tilbake slik et søknadssystem (ATS) gjør, og viser hva som kommer med. Leseren vår er snillere enn mange ekte systemer: det som mangler her, mangler nok der også."),
            );
            ui.add_space(4.0);
            let mut pick = None;
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!busy, egui::Button::new(i18n::t("Les PDF-en tilbake"))).on_hover_text(i18n::t("Lager filen slik den lastes ned, og leser den som et ATS")).clicked() {
                    pick = Some(false);
                }
                if ui.add_enabled(!busy, egui::Button::new(i18n::t("Les Word-filen tilbake"))).on_hover_text(i18n::t("Lager filen slik den lastes ned, og leser den som et ATS")).clicked() {
                    pick = Some(true);
                }
                if busy {
                    ui.spinner();
                    ui.label(i18n::t("Leser filen tilbake …"));
                }
            });
            if let Some(word) = pick {
                self.start_read_back(word);
            }
            match &self.readback.result {
                None => {}
                Some(Err(e)) => status_line(ui, Status::Problem, &i18n::tf("Filen kunne ikke lages eller leses tilbake. {why}", &[("why", &i18n::tdyn(e))])),
                Some(Ok(back)) => {
                    ui.add_space(4.0);
                    theme::label(ui, &self.readback.from);
                    let same = back.count(State::Same);
                    let status = if same == back.fields.len() { Status::Done } else { Status::ToDo };
                    status_line(ui, status, &i18n::tf("{same} av {all} felt ble lest som i CV-en.", &[("same", &same.to_string()), ("all", &back.fields.len().to_string())]));
                    // What went wrong first: that is what the person can fix.
                    let mut fields: Vec<_> = back.fields.iter().collect();
                    fields.sort_by_key(|f| match f.state {
                        State::Missing => 0,
                        State::Different => 1,
                        State::Same => 2,
                    });
                    for f in fields {
                        let name = field_name(f.field);
                        let (status, text) = match f.state {
                            State::Same => (Status::Done, i18n::tf("{field}: «{expected}»", &[("field", &name), ("expected", &f.expected)])),
                            State::Different => {
                                (Status::ToDo, i18n::tf("{field}: «{expected}» ble lest som «{read}»", &[("field", &name), ("expected", &f.expected), ("read", &f.read)]))
                            }
                            State::Missing => (Status::Problem, i18n::tf("{field}: «{expected}» ble ikke funnet", &[("field", &name), ("expected", &f.expected)])),
                        };
                        status_line(ui, status, &text);
                    }
                    egui::CollapsingHeader::new(i18n::t("Teksten slik den leses")).id_salt("readback-text").show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&back.text).monospace().size(12.0)).wrap());
                    });
                }
            }
        });
    }
}
