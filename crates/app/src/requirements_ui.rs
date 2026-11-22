//! «Krav i annonsen»: the ad's must-haves and nice-to-haves, one by one, with where the CV
//! shows each (to-do 045). Read offline from the ad's headings and cue phrases; the AI sorts
//! them again only when the person asks. Shown in the recruiter check, above the key words.
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, channel};

use cv_core::checks::normal;
use cv_core::requirements::{self, Need, Place, Requirement};
use egui::Margin;

use crate::CvApp;
use crate::checklist_ui::{Status, status_line};
use crate::i18n;
use crate::theme::{self, Tokens};

#[derive(Default)]
pub struct RequirementsState {
    /// The ad text the list was read from; another ad reads it again.
    ad: Option<String>,
    list: Vec<Requirement>,
    /// The list came from the AI, not from CVBuilder's own reading.
    by_ai: bool,
    /// Requirements the person moved to the other group, by their normalised text.
    moved: HashSet<String>,
    inbox: Option<Receiver<Result<Vec<(String, Need)>, String>>>,
    pub note: String,
}

impl RequirementsState {
    pub fn busy(&self) -> bool {
        self.inbox.is_some()
    }

    /// The list for this ad, read again when the ad changed.
    fn refresh(&mut self, ad: &str) {
        if self.ad.as_deref() != Some(ad) {
            *self = RequirementsState { ad: Some(ad.to_string()), list: requirements::ad_requirements(ad), ..Default::default() };
        }
    }

    /// A requirement's need after the person's moves.
    fn need(&self, r: &Requirement) -> Need {
        match (r.need, self.moved.contains(&normal(&r.text))) {
            (n, false) => n,
            (Need::Must, true) => Need::Nice,
            (Need::Nice, true) => Need::Must,
        }
    }
}

/// A place in the CV in the interface language.
fn place_name(p: &Place) -> String {
    match p {
        Place::Job(name) | Place::Education(name) | Place::Certification(name) | Place::Section(name) if !name.is_empty() => name.clone(),
        Place::Job(_) => i18n::t("Arbeidserfaring").to_string(),
        Place::Education(_) => i18n::t("Utdanning").to_string(),
        Place::Certification(_) => i18n::t("Kurs og sertifiseringer").to_string(),
        Place::Section(_) => i18n::t("Egen seksjon").to_string(),
        Place::Skills => i18n::t("Kjernekompetanse").to_string(),
        Place::Profile => i18n::t("Profil").to_string(),
        Place::Languages => i18n::t("Språk").to_string(),
    }
}

impl CvApp {
    /// The requirement table in the recruiter check. Nothing when the ad has no requirements
    /// CVBuilder can read; the key word pills below are then the whole answer.
    pub(crate) fn ad_requirements(&mut self, ui: &mut egui::Ui) {
        let ad = self.project.target.description.clone();
        if ad.trim().is_empty() {
            return;
        }
        self.requirements.refresh(&ad);
        let t = Tokens::get(ui.ctx());
        let state = &self.requirements;
        if state.list.is_empty() {
            // An ad without headings or cue phrases: the AI can still find them, if asked.
            ui.add_space(6.0);
            theme::label(ui, i18n::t("Krav i annonsen"));
            theme::hint(ui, i18n::t("Fant ingen overskrifter som «Krav» eller «Ønskelig» i annonsen. Fagordene under gjelder fortsatt."));
            self.sort_button(ui);
            return;
        }
        let mut move_to: Option<String> = None;
        ui.add_space(10.0);
        theme::label(ui, i18n::t("Krav i annonsen"));
        let judged: Vec<(Need, bool)> = state.list.iter().filter(|r| !r.terms.is_empty()).map(|r| (state.need(r), requirements::backing(&self.project, r).shown())).collect();
        let count = |need: Need| (judged.iter().filter(|(n, s)| *n == need && *s).count(), judged.iter().filter(|(n, _)| *n == need).count());
        let ((ms, m), (ns, n)) = (count(Need::Must), count(Need::Nice));
        theme::hint(
            ui,
            &i18n::tf("Må ha: {a} av {b} vises i CV-en. Fint å ha: {c} av {d}.", &[("a", &ms.to_string()), ("b", &m.to_string()), ("c", &ns.to_string()), ("d", &n.to_string())]),
        );
        for need in [Need::Must, Need::Nice] {
            let group: Vec<&Requirement> = state.list.iter().filter(|r| state.need(r) == need).collect();
            if group.is_empty() {
                continue;
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new(if need == Need::Must { i18n::t("Må ha") } else { i18n::t("Fint å ha") }).size(12.5).family(theme::medium()).color(t.dim));
            for r in group {
                let b = requirements::backing(&self.project, r);
                let status = if r.terms.is_empty() {
                    Status::Open
                } else if b.shown() {
                    Status::Done
                } else if b.found.is_empty() && need == Need::Must {
                    Status::Problem
                } else {
                    Status::ToDo
                };
                status_line(ui, status, &r.text);
                let label = |terms: &[String]| terms.iter().map(|x| requirements::label(&r.text, x)).collect::<Vec<_>>().join(", ");
                let mut lines: Vec<String> = Vec::new();
                if r.terms.is_empty() {
                    lines.push(i18n::t("CVBuilder kan ikke sjekke dette selv. Se etter det i CV-en.").to_string());
                }
                if !b.places.is_empty() {
                    let places: Vec<String> = b.places.iter().take(3).map(place_name).collect();
                    lines.push(i18n::tf("Vises i: {places}", &[("places", &places.join("; "))]));
                }
                if b.only_listed() {
                    lines.push(i18n::t("Står bare i listen, ingen jobb viser det. Vis det i en jobb hvis det stemmer.").to_string());
                }
                if !b.missing.is_empty() {
                    lines.push(i18n::tf("Ikke funnet: {terms}", &[("terms", &label(&b.missing))]));
                }
                egui::Frame::new().inner_margin(Margin { left: 20, right: 0, top: 0, bottom: 2 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in &lines {
                        theme::hint(ui, line);
                    }
                    let other = if need == Need::Must { i18n::t("Flytt til «Fint å ha»") } else { i18n::t("Flytt til «Må ha»") };
                    let link = egui::Button::new(egui::RichText::new(other).size(12.0).color(t.accent)).frame(false).small();
                    if ui.add(link).on_hover_text(i18n::t("Hvis CVBuilder sorterte dette feil")).clicked() {
                        move_to = Some(normal(&r.text));
                    }
                });
            }
        }
        if let Some(key) = move_to {
            if !self.requirements.moved.remove(&key) {
                self.requirements.moved.insert(key);
            }
        }
        self.sort_button(ui);
    }

    /// «La AI-en sortere kravene», or the way back to CVBuilder's own reading.
    fn sort_button(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        if !self.requirements.note.is_empty() {
            theme::hint(ui, &self.requirements.note.clone());
        }
        if self.requirements.busy() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(i18n::t("AI-en sorterer kravene …"));
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if self.requirements.by_ai {
                if ui.button(i18n::t("Bruk CVBuilders egen sortering")).on_hover_text(i18n::t("Les kravene fra annonsen på nytt uten AI")).clicked() {
                    self.requirements.ad = None;
                }
            } else if ui.button(i18n::t("La AI-en sortere kravene")).on_hover_text(i18n::t("Bare annonsen sendes, ikke CV-en. AI-en finner ikke på krav, og ingenting i CV-en endres.")).clicked() {
                self.sort_requirements_ai();
            }
        });
    }

    /// Sends the ad (not the CV) to the AI in use and has it sort the requirements again.
    pub(crate) fn sort_requirements_ai(&mut self) {
        let Some(backend) = self.chat_backend() else {
            self.ai.connect_open = true;
            self.ai.choice = Some(cv_ai::Service::Claude);
            return;
        };
        if let Some(destination) = self.must_ask(cv_ai::cost::Work::Chat) {
            self.ai_pending = Some(crate::ai_notice::Pending { work: cv_ai::cost::Work::Chat, text: self.project.target.description.clone(), destination, sort_requirements: true });
            return;
        }
        let found: Vec<(String, Need)> = self.requirements.list.iter().map(|r| (r.text.clone(), self.requirements.need(r))).collect();
        let (system, user) = cv_ai::tasks::requirements_request(&self.project, &found);
        let (tx, rx) = channel();
        let ctx = self.egui_ctx.clone();
        // The ad leaves now: remember the yes for this destination.
        self.ai_sent(cv_ai::cost::Work::Chat);
        std::thread::spawn(move || {
            let reply = crate::langpack_ui::ask(&backend, &system, &user).and_then(|text| cv_ai::tasks::parse_requirements(&text).ok_or_else(|| i18n::t("Svaret fra AI-en kunne ikke leses. Prøv igjen.").to_string()));
            let _ = tx.send(reply);
            ctx.request_repaint();
        });
        self.requirements.note.clear();
        self.requirements.inbox = Some(rx);
    }

    /// The AI's answer, when it comes.
    pub(crate) fn requirements_poll(&mut self) {
        let Some(rx) = &self.requirements.inbox else { return };
        let Ok(reply) = rx.try_recv() else { return };
        self.requirements.inbox = None;
        match reply {
            Ok(list) if !list.is_empty() => {
                self.requirements.list = list.into_iter().map(|(text, need)| Requirement { terms: requirements::terms(&text), text, need }).collect();
                self.requirements.by_ai = true;
                self.requirements.moved.clear();
                self.requirements.note = i18n::t("Sortert av AI-en. Sjekk at det stemmer med annonsen.").to_string();
            }
            Ok(_) => self.requirements.note = i18n::t("AI-en fant ingen krav i annonsen.").to_string(),
            Err(e) => self.requirements.note = e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moved_requirement_changes_group_and_a_new_ad_starts_over() {
        let mut s = RequirementsState::default();
        s.refresh("Krav\n- Fagbrev som elektriker\n\nØnskelig\n- KNX");
        assert_eq!(s.list.len(), 2);
        assert_eq!(s.need(&s.list[0].clone()), Need::Must);
        s.moved.insert(normal(&s.list[0].text));
        assert_eq!(s.need(&s.list[0].clone()), Need::Nice);
        s.refresh("Krav\n- Fagbrev som elektriker\n\nØnskelig\n- KNX");
        assert_eq!(s.moved.len(), 1, "the same ad keeps the person's moves");
        s.refresh("Requirements\n- Python");
        assert!(s.moved.is_empty() && s.list.len() == 1);
    }
}
