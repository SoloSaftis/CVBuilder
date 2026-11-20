//! «Søknadsskjema» in the side panel, for the online form most applications end in: the CV one
//! field at a time, each with its own copy button (dates in the format the portal wants), and
//! the person's ready answers to the questions that decide whether anyone reads the CV (work
//! permit, driving licence, notice period …). The ready answers describe the person, not the CV:
//! they live in the data folder on this computer only – never in the CV file, never in the CV,
//! never sent to an AI. The job ad is checked for those questions so the answer is at hand.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use cv_core::Project;
use egui::{Margin, RichText};
use serde::{Deserialize, Serialize};

use crate::i18n;
use crate::theme::{self, Tokens};
use crate::{CvApp, status_line, storage};

/// How long a copy button says «Kopiert».
const COPIED_FOR: Duration = Duration::from_millis(1600);

/// The date formats application portals ask for. The example is the label: it needs no words.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DateFormat {
    #[default]
    Slash,
    Iso,
    Dot,
    Year,
}

impl DateFormat {
    pub const ALL: [DateFormat; 4] = [DateFormat::Slash, DateFormat::Iso, DateFormat::Dot, DateFormat::Year];

    pub fn example(self) -> &'static str {
        match self {
            DateFormat::Slash => "09/2021",
            DateFormat::Iso => "2021-09",
            DateFormat::Dot => "09.2021",
            DateFormat::Year => "2021",
        }
    }
}

/// A CV date (`YYYY-MM`, or a bare year) in the chosen format. A bare year stays a year, and
/// anything else is passed on as it is written.
pub fn date(value: &str, format: DateFormat) -> String {
    let value = value.trim();
    let Some((year, month)) = value.split_once('-').filter(|(y, m)| y.len() == 4 && m.len() == 2 && y.chars().chain(m.chars()).all(|c| c.is_ascii_digit())) else {
        return value.to_string();
    };
    match format {
        DateFormat::Slash => format!("{month}/{year}"),
        DateFormat::Iso => value.to_string(),
        DateFormat::Dot => format!("{month}.{year}"),
        DateFormat::Year => year.to_string(),
    }
}

/// Bullet points as one block for a description field: one per line, with «• » if wanted.
pub fn lines(bullets: &[String], dots: bool) -> String {
    bullets.iter().map(|b| b.trim()).filter(|b| !b.is_empty()).map(|b| if dots { format!("• {b}") } else { b.to_string() }).collect::<Vec<_>>().join("\n")
}

/// One field to copy: its label (a Norwegian key, or the person's own label for a contact
/// field) and its value.
#[derive(Debug, PartialEq)]
pub struct Field {
    pub key: &'static str,
    pub own_label: Option<String>,
    pub value: String,
}

/// A group of fields under a heading: the contact details, one job, one education …
#[derive(Debug, PartialEq)]
pub struct Group {
    /// The heading: a Norwegian key, or text from the CV (a job's title and employer).
    pub title: String,
    pub translate: bool,
    pub fields: Vec<Field>,
}

fn field(key: &'static str, value: impl Into<String>) -> Field {
    Field { key, own_label: None, value: value.into() }
}

fn heading(parts: &[&str], fallback: &'static str) -> (String, bool) {
    let text = parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" – ");
    if text.is_empty() { (fallback.to_string(), true) } else { (text, false) }
}

/// The CV as groups of fields to copy, in the order application forms ask for them. Empty
/// fields and empty groups are left out.
pub fn groups(project: &Project, format: DateFormat, dots: bool) -> Vec<Group> {
    let c = &project.candidate;
    let mut out = Vec::new();
    let mut contact = vec![
        field("Fullt navn", &c.name),
        field("E-post", &c.email),
        field("Telefon", &c.phone),
        field("By og land", &c.city),
        Field { key: "Kontaktinformasjon", own_label: Some("LinkedIn".into()), value: c.linkedin.clone() },
        field("Tittel", &c.headline),
    ];
    contact.extend(c.extra.iter().map(|f| {
        let own = f.label.trim();
        Field { key: "Kontaktinformasjon", own_label: (!own.is_empty()).then(|| own.to_string()), value: f.value.clone() }
    }));
    contact.push(field("Profil", &c.summary));
    out.push(Group { title: "Kontaktinformasjon".into(), translate: true, fields: contact });
    let present = cv_core::text::labels(project).present;
    for job in &project.experience {
        let (title, translate) = heading(&[job.title.as_str(), job.company.as_str()], "Arbeidserfaring");
        let end = match &job.end {
            None => present.to_string(),
            Some(end) => date(end, format),
        };
        out.push(Group {
            title,
            translate,
            fields: vec![
                field("Stilling", &job.title),
                field("Arbeidsgiver", &job.company),
                field("Sted", &job.location),
                field("Fra", date(&job.start, format)),
                field("Til", end),
                field("Beskrivelse", lines(&job.bullets, dots)),
            ],
        });
    }
    for e in &project.education {
        let (title, translate) = heading(&[e.qualification.as_str(), e.institution.as_str()], "Utdanning");
        out.push(Group {
            title,
            translate,
            fields: vec![field("Utdanning", &e.qualification), field("Lærested", &e.institution), field("Fra", date(&e.start, format)), field("Til", date(&e.end, format))],
        });
    }
    for cert in &project.certifications {
        let (title, translate) = heading(&[cert.name.as_str()], "Kurs eller sertifisering");
        out.push(Group { title, translate, fields: vec![field("Kurs eller sertifisering", &cert.name), field("Utsteder", &cert.issuer), field("År", &cert.year)] });
    }
    let skills = project.skills.iter().map(|s| s.name.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
    let languages = project
        .languages
        .iter()
        .filter(|l| !l.name.trim().is_empty())
        .map(|l| if l.level.trim().is_empty() { l.name.trim().to_string() } else { format!("{} – {}", l.name.trim(), l.level.trim()) })
        .collect::<Vec<_>>()
        .join("\n");
    out.push(Group { title: "Kjernekompetanse".into(), translate: true, fields: vec![field("Kjernekompetanse", skills), field("Språk", languages)] });
    for g in &mut out {
        g.fields.retain(|f| !f.value.trim().is_empty());
    }
    out.retain(|g| !g.fields.is_empty());
    out
}

// ---------- ready answers ----------

/// The questions portals ask before anyone reads the CV: an id, the Norwegian key, and the
/// words in a job ad that mean the ad asks about it (lower case; Norwegian, English, Swedish,
/// Danish, German). Words that also mean other things («referanse» as in a reference number,
/// Swedish «visa» = show) are left out, so the check does not cry wolf.
pub const QUESTIONS: &[(&str, &str, &[&str])] = &[
    (
        "permit",
        "Arbeidstillatelse eller statsborgerskap",
        &["arbeidstillatelse", "oppholdstillatelse", "statsborger", "work permit", "right to work", "work authori", "sponsorship", "arbetstillstånd", "arbejdstilladelse", "arbeitserlaubnis"],
    ),
    ("licence", "Førerkort", &["førerkort", "sertifikat klasse", "driving licen", "driver's licen", "drivers licen", "driver’s licen", "körkort", "kørekort", "führerschein"]),
    ("notice", "Oppsigelsestid", &["oppsigelsestid", "notice period", "uppsägningstid", "opsigelsesvarsel", "kündigungsfrist"]),
    ("start", "Tidligste oppstart", &["tiltredelse", "start date", "available from", "tillträde", "tiltrædelse", "eintrittstermin", "eintrittsdatum"]),
    ("salary", "Lønnsforventning", &["lønnsforventning", "lønnskrav", "salary expectation", "salary requirement", "löneanspråk", "lønforventning", "gehaltsvorstellung"]),
    ("references", "Referanser", &[]),
    ("clearance", "Sikkerhetsklarering", &["sikkerhetsklarering", "security clearance", "säkerhetsprövning", "sikkerhedsgodkendelse", "sicherheitsüberprüfung"]),
    (
        "travel",
        "Kan flytte eller reise",
        &["reisevirksomhet", "reiseaktivitet", "villig til å reise", "relocat", "willing to travel", "travel required", "resor i tjänsten", "rejseaktivitet", "reisebereitschaft"],
    ),
];

/// The ready questions the job ad asks about, by id, in [`QUESTIONS`] order.
pub fn asked_in(ad: &str) -> Vec<&'static str> {
    let ad = ad.to_lowercase();
    QUESTIONS.iter().filter(|(_, _, words)| words.iter().any(|w| ad.contains(w))).map(|(id, _, _)| *id).collect()
}

/// A question of the person's own, with its answer.
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default)]
pub struct OwnAnswer {
    pub question: String,
    pub answer: String,
}

/// The ready answers, as kept in `faste-svar.json` in the data folder.
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default)]
pub struct ReadyAnswers {
    /// Answers to [`QUESTIONS`], by id.
    pub answers: BTreeMap<String, String>,
    pub own: Vec<OwnAnswer>,
}

impl ReadyAnswers {
    pub fn answer(&self, id: &str) -> &str {
        self.answers.get(id).map(|s| s.trim()).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.answers.values().all(|a| a.trim().is_empty()) && self.own.is_empty()
    }
}

fn file() -> std::path::PathBuf {
    storage::data_dir().join("faste-svar.json")
}

/// The ready answers on this computer. A file that is there but cannot be read is moved aside
/// (`faste-svar.json.bad`) rather than written over, and said out loud.
fn load() -> (ReadyAnswers, String) {
    match std::fs::read_to_string(file()) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(answers) => (answers, String::new()),
            Err(e) => {
                let _ = std::fs::rename(file(), file().with_extension("json.bad"));
                (ReadyAnswers::default(), i18n::tf("De faste svarene kunne ikke leses: {error}", &[("error", &e.to_string())]))
            }
        },
        Err(_) => (ReadyAnswers::default(), String::new()),
    }
}

/// Writes then renames, so a crash never leaves half a file. No answers: no file at all.
fn save(answers: &ReadyAnswers) -> Result<(), String> {
    let path = file();
    if answers.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(answers).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub struct FormHelperState {
    pub ready: ReadyAnswers,
    pub format: DateFormat,
    pub dots: bool,
    /// The field last copied, and when: its button says «Kopiert» for a moment.
    copied: Option<(String, Instant)>,
    /// «Slett alle svarene» was clicked and waits for a yes.
    confirm_clear: bool,
    pub message: String,
}

impl FormHelperState {
    pub fn load() -> FormHelperState {
        let (ready, message) = load();
        FormHelperState { ready, format: DateFormat::default(), dots: false, copied: None, confirm_clear: false, message }
    }
}

impl CvApp {
    /// A copy button for one value; it says «Kopiert» for a moment after a click.
    fn copy_button(&mut self, ui: &mut egui::Ui, id: &str, name: &str, value: &str) {
        let t = Tokens::get(ui.ctx());
        let just = self.form_helper.copied.as_ref().is_some_and(|(c, at)| c.as_str() == id && at.elapsed() < COPIED_FOR);
        if just {
            ui.label(RichText::new(i18n::t("Kopiert")).size(12.5).family(theme::medium()).color(t.success));
            ui.ctx().request_repaint_after(COPIED_FOR);
            return;
        }
        let tip = i18n::tf("Kopier «{field}»", &[("field", name)]);
        let r = theme::named(theme::flat_icon(ui, theme::ph::COPY, !value.trim().is_empty()), &tip).on_hover_text(&tip);
        if r.clicked() {
            ui.ctx().copy_text(value.to_string());
            self.form_helper.copied = Some((id.to_string(), Instant::now()));
        }
    }

    /// One row: the label over the value (one line, whole on hover), the copy button at the end.
    fn copy_row(&mut self, ui: &mut egui::Ui, id: &str, name: &str, value: &str) {
        let t = Tokens::get(ui.ctx());
        ui.horizontal(|ui| {
            let room = (ui.available_width() - 70.0).max(60.0);
            ui.vertical(|ui| {
                ui.set_width(room);
                ui.add(egui::Label::new(RichText::new(name).size(12.0).color(t.dim)).truncate());
                let shown = value.lines().next().unwrap_or("");
                let shown = if value.lines().nth(1).is_some() { format!("{shown} …") } else { shown.to_string() };
                ui.add(egui::Label::new(RichText::new(shown).size(13.5).color(t.text)).truncate()).on_hover_text(value);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.copy_button(ui, id, name, value));
        });
    }

    pub(crate) fn form_helper(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        theme::hint(ui, i18n::t("Kopier ett felt om gangen og lim det inn i søknadsskjemaet på nettet."));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(i18n::t("Datoformat")).size(12.5).color(t.dim));
            for f in DateFormat::ALL {
                if ui.radio(self.form_helper.format == f, f.example()).on_hover_text(i18n::t("Datoformat")).clicked() {
                    self.form_helper.format = f;
                }
            }
        });
        ui.checkbox(&mut self.form_helper.dots, i18n::t("Punkt («•») foran hver linje")).on_hover_text(i18n::t("Beskrivelse"));
        ui.add_space(8.0);
        let groups = groups(&self.project, self.form_helper.format, self.form_helper.dots);
        for (g, group) in groups.iter().enumerate() {
            theme::card(ui).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let title = if group.translate { i18n::tdyn(&group.title) } else { group.title.clone() };
                ui.add(egui::Label::new(RichText::new(title).size(14.0).family(theme::bold()).color(t.text)).truncate());
                for (i, f) in group.fields.iter().enumerate() {
                    let name = f.own_label.clone().unwrap_or_else(|| i18n::t(f.key).to_string());
                    self.copy_row(ui, &format!("cv.{g}.{i}"), &name, &f.value);
                }
            });
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        self.ready_answers(ui);
    }

    fn ready_answers(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let mut changed = false;
        theme::card(ui).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t("Mine faste svar"));
            theme::hint(
                ui,
                i18n::t("Svar på spørsmålene som ofte avgjør om søknaden blir lest. Svarene lagres bare på denne maskinen: ikke i CV-filen, ikke i CV-en, og de sendes ikke til AI. På en delt maskin: slett dem når du er ferdig."),
            );
            if !self.form_helper.message.is_empty() {
                status_line(ui, t.warning, &self.form_helper.message.clone());
            }
            // What the job ad asks about, and whether an answer is ready.
            for id in asked_in(&self.project.target.description) {
                let Some((_, key, _)) = QUESTIONS.iter().find(|(q, _, _)| *q == id) else { continue };
                let what = i18n::t(key);
                let answer = self.form_helper.ready.answer(id);
                if answer.is_empty() {
                    status_line(ui, t.warning, &i18n::tf("Annonsen nevner «{what}», men du har ikke skrevet et svar ennå.", &[("what", what)]));
                } else {
                    status_line(ui, t.success, &i18n::tf("Annonsen nevner «{what}». Svaret ditt: {answer}", &[("what", what), ("answer", answer)]));
                }
            }
            ui.add_space(4.0);
            // Each answer: its question and copy button on one row, the text over the full width
            // under it. The text fields get an exact width so they can never widen the panel.
            for (id, key, _) in QUESTIONS {
                let name = i18n::t(key);
                let value = self.form_helper.ready.answer(id).to_string();
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 26.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.copy_button(ui, &format!("ready.{id}"), name, &value);
                    ui.add(egui::Label::new(RichText::new(name).size(12.0).color(t.dim)).truncate());
                });
                let width = ui.available_width();
                let text = self.form_helper.ready.answers.entry(id.to_string()).or_default();
                changed |= ui.add(egui::TextEdit::multiline(text).desired_rows(1).desired_width(width)).on_hover_text(name).changed();
                ui.add_space(2.0);
            }
            let mut remove = None;
            for i in 0..self.form_helper.ready.own.len() {
                ui.add_space(4.0);
                let own = self.form_helper.ready.own[i].clone();
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::remove_button(ui, i18n::t("Fjern spørsmålet")).clicked() {
                        remove = Some(i);
                    }
                    self.copy_button(ui, &format!("own.{i}"), &own.question, own.answer.trim());
                    let width = ui.available_width();
                    let q = &mut self.form_helper.ready.own[i].question;
                    changed |= ui
                        .add(egui::TextEdit::singleline(q).hint_text(i18n::t("Spørsmålet, f.eks. Kan du jobbe skift?")).desired_width(width))
                        .on_hover_text(i18n::t("Spørsmålet, f.eks. Kan du jobbe skift?"))
                        .changed();
                });
                let width = ui.available_width();
                let answer = &mut self.form_helper.ready.own[i].answer;
                changed |= ui.add(egui::TextEdit::multiline(answer).desired_rows(1).desired_width(width)).on_hover_text(i18n::t("Et spørsmål portalene dine ofte stiller, med svaret ditt")).changed();
            }
            if let Some(i) = remove {
                self.form_helper.ready.own.remove(i);
                changed = true;
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(i18n::t("+ Legg til eget spørsmål")).on_hover_text(i18n::t("Et spørsmål portalene dine ofte stiller, med svaret ditt")).clicked() {
                    self.form_helper.ready.own.push(OwnAnswer::default());
                }
                if !self.form_helper.ready.is_empty() && !self.form_helper.confirm_clear && ui.button(i18n::t("Slett alle svarene")).on_hover_text(i18n::t("Slett de faste svarene fra denne maskinen")).clicked() {
                    self.form_helper.confirm_clear = true;
                }
            });
            if self.form_helper.confirm_clear {
                ui.add_space(4.0);
                ui.label(i18n::t("Slette alle de faste svarene på denne maskinen?"));
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("Slett")).on_hover_text(i18n::t("Slett de faste svarene fra denne maskinen")).clicked() {
                        self.form_helper.ready = ReadyAnswers::default();
                        self.form_helper.confirm_clear = false;
                        changed = true;
                    }
                    if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Behold svarene")).clicked() {
                        self.form_helper.confirm_clear = false;
                    }
                });
            }
        });
        if changed {
            // Questions with no answer are not worth a file.
            self.form_helper.ready.answers.retain(|_, a| !a.is_empty());
            match save(&self.form_helper.ready) {
                Ok(()) => self.form_helper.message.clear(),
                Err(e) => self.form_helper.message = i18n::tf("De faste svarene kunne ikke lagres: {error}", &[("error", &e)]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_core::model::{Certification, ContactField, Education, Job, Skill, SpokenLanguage};

    #[test]
    fn dates_in_every_format() {
        assert_eq!(date("2021-09", DateFormat::Slash), "09/2021");
        assert_eq!(date("2021-09", DateFormat::Iso), "2021-09");
        assert_eq!(date("2021-09", DateFormat::Dot), "09.2021");
        assert_eq!(date("2021-09", DateFormat::Year), "2021");
        // A bare year stays a year; text the person typed some other way is kept as it is.
        assert_eq!(date("2016", DateFormat::Slash), "2016");
        assert_eq!(date(" 2016 ", DateFormat::Dot), "2016");
        assert_eq!(date("  ", DateFormat::Slash), "");
        assert_eq!(date("2019-2021", DateFormat::Slash), "2019-2021");
    }

    #[test]
    fn bullets_become_lines() {
        let bullets = vec!["Led the team".to_string(), "  ".to_string(), " Cut costs 20 % ".to_string()];
        assert_eq!(lines(&bullets, false), "Led the team\nCut costs 20 %");
        assert_eq!(lines(&bullets, true), "• Led the team\n• Cut costs 20 %");
        assert_eq!(lines(&[], true), "");
    }

    fn project() -> Project {
        let mut p = Project::default();
        p.candidate.name = "Kari Nordmann".into();
        p.candidate.email = "kari@example.com".into();
        p.candidate.extra.push(ContactField { id: "c1".into(), label: "GitHub".into(), value: "github.com/kari".into() });
        p.experience.push(Job { company: "Fjellvegg AS".into(), title: "Koordinator".into(), start: "2021-09".into(), end: None, bullets: vec!["Planla drift".into()], ..Default::default() });
        p.experience.push(Job { company: "Bygg AS".into(), title: "".into(), start: "2018-01".into(), end: Some("2021-08".into()), ..Default::default() });
        p.education.push(Education { qualification: "Bachelor".into(), institution: "NTNU".into(), start: "2015".into(), end: "2018".into() });
        p.certifications.push(Certification { name: "Truckførerbevis".into(), issuer: "".into(), year: "2019".into() });
        p.skills.push(Skill { name: "Excel".into(), ..Default::default() });
        p.skills.push(Skill { name: "SAP".into(), ..Default::default() });
        p.languages.push(SpokenLanguage { name: "Engelsk".into(), level: "flytende".into() });
        p
    }

    fn value<'a>(g: &'a Group, key: &str) -> Option<&'a str> {
        g.fields.iter().find(|f| f.key == key).map(|f| f.value.as_str())
    }

    #[test]
    fn the_cv_as_fields_to_copy() {
        let p = project();
        let groups = groups(&p, DateFormat::Slash, true);
        let titles: Vec<&str> = groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Kontaktinformasjon", "Koordinator – Fjellvegg AS", "Bygg AS", "Bachelor – NTNU", "Truckførerbevis", "Kjernekompetanse"]);
        // Empty fields are left out; the person's own contact field keeps its own label.
        let contact = &groups[0];
        assert_eq!(value(contact, "Fullt navn"), Some("Kari Nordmann"));
        assert_eq!(value(contact, "Telefon"), None);
        assert!(contact.fields.iter().any(|f| f.own_label.as_deref() == Some("GitHub") && f.value == "github.com/kari"));
        // A current job ends «now» in the CV's language; dates follow the chosen format.
        let job = &groups[1];
        assert_eq!(value(job, "Fra"), Some("09/2021"));
        assert_eq!(value(job, "Til"), Some(cv_core::text::labels(&p).present));
        assert_eq!(value(job, "Beskrivelse"), Some("• Planla drift"));
        assert_eq!(value(&groups[2], "Til"), Some("08/2021"));
        assert_eq!(value(&groups[3], "Fra"), Some("2015"));
        assert_eq!(value(&groups[4], "Utsteder"), None);
        assert_eq!(value(&groups[5], "Kjernekompetanse"), Some("Excel, SAP"));
        assert_eq!(value(&groups[5], "Språk"), Some("Engelsk – flytende"));
        let iso = super::groups(&p, DateFormat::Iso, false);
        assert_eq!(value(&iso[1], "Fra"), Some("2021-09"));
        assert_eq!(value(&iso[1], "Beskrivelse"), Some("Planla drift"));
    }

    #[test]
    fn an_empty_cv_has_nothing_to_copy() {
        assert!(groups(&Project::default(), DateFormat::Slash, false).is_empty());
    }

    #[test]
    fn a_job_with_no_title_or_employer_is_still_named() {
        let mut p = Project::default();
        p.experience.push(Job { start: "2020-01".into(), ..Default::default() });
        let g = groups(&p, DateFormat::Slash, false);
        assert_eq!((g[0].title.as_str(), g[0].translate), ("Arbeidserfaring", true));
    }

    #[test]
    fn the_ad_is_checked_for_knockout_questions() {
        let ad = "Vi ser etter deg som har førerkort klasse B og kan bli sikkerhetsklarert. Oppgi lønnsforventning. Referansenr. 2024/17.";
        assert_eq!(asked_in(ad), ["licence", "salary"]);
        assert_eq!(
            asked_in("Must have the right to work in the UK, a valid driving licence and a notice period of max 3 months. Security clearance required."),
            ["permit", "licence", "notice", "clearance"]
        );
        assert_eq!(asked_in("Sikkerhetsklarering kreves. Tiltredelse etter avtale."), ["start", "clearance"]);
        // A reference number and Swedish «visa» (show) are not questions.
        assert!(asked_in("Reference number 17. Visa att du kan.").is_empty());
        assert!(asked_in("").is_empty());
    }

    #[test]
    fn every_label_has_english() {
        let mut keys: Vec<String> = QUESTIONS.iter().map(|(_, key, _)| key.to_string()).collect();
        for g in groups(&project(), DateFormat::Slash, false) {
            if g.translate {
                keys.push(g.title);
            }
            keys.extend(g.fields.iter().filter(|f| f.own_label.is_none()).map(|f| f.key.to_string()));
        }
        keys.extend(["Arbeidserfaring", "Utdanning", "Kurs eller sertifisering", "Telefon", "By og land", "Tittel", "Profil", "Sted", "Kontaktinformasjon"].map(String::from));
        for key in keys {
            assert!(crate::i18n::has_english(&key), "«{key}» has no English");
        }
    }

    #[test]
    fn ready_answers_round_trip_and_tolerate_old_files() {
        let mut r = ReadyAnswers::default();
        assert!(r.is_empty());
        r.answers.insert("licence".into(), " B ".into());
        r.own.push(OwnAnswer { question: "Skift?".into(), answer: "Ja".into() });
        let text = serde_json::to_string(&r).expect("json");
        let back: ReadyAnswers = serde_json::from_str(&text).expect("read");
        assert_eq!(back, r);
        assert_eq!(back.answer("licence"), "B");
        assert_eq!(back.answer("salary"), "");
        // Unknown fields from a newer version are ignored, missing ones get their default.
        let newer: ReadyAnswers = serde_json::from_str(r#"{"answers": {"notice": "3 mnd"}, "later": 1}"#).expect("newer");
        assert_eq!(newer.answer("notice"), "3 mnd");
        assert!(newer.own.is_empty());
        // Blank answers do not count as answers.
        let blank = ReadyAnswers { answers: [("salary".to_string(), "  ".to_string())].into(), own: Vec::new() };
        assert!(blank.is_empty());
    }
}
