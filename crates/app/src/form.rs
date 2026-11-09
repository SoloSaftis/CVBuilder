//! The form: the contact card first, then the CV's sections in the CV's own order. Each section
//! can be moved up or down (the CV follows), folded away, and the person's own sections (projects,
//! volunteering …) can be added and deleted. The example shows the recommended order.
use std::collections::{HashMap, HashSet};

use cv_core::checks;
use cv_core::model::{Certification, ContactField, CustomItem, Education, Interest, InterestStatus, Job, Skill, SpokenLanguage};
use cv_core::text::{self as cvtext, CONTACT_KEYS};
use egui::{CornerRadius, Margin, RichText, Stroke};

use crate::CvApp;
use crate::i18n;
use crate::theme::{self, Icon, Tokens};

/// What the person asked to do with a section's header buttons.
enum SectionAction {
    Up,
    Down,
    Delete,
}

fn form_title(key: &str) -> &'static str {
    match key {
        "profile" => i18n::t("Profil"),
        "skills" => i18n::t("Kjernekompetanse"),
        "experience" => i18n::t("Arbeidserfaring"),
        "education" => i18n::t("Utdanning"),
        "certifications" => i18n::t("Kurs og sertifiseringer"),
        "languages" => i18n::t("Språk"),
        "interests" => i18n::t("Interesser"),
        _ => "",
    }
}

fn contact_label(key: &str) -> &'static str {
    match key {
        "email" => i18n::t("E-post"),
        "phone" => i18n::t("Telefon"),
        "city" => i18n::t("By og land"),
        "linkedin" => "LinkedIn",
        _ => "",
    }
}

impl CvApp {
    pub(crate) fn form(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        self.target_card(ui, &mut changed);
        self.contact_card(ui, &mut changed);
        let order = cvtext::section_order(&self.project);
        let mut action: Option<(usize, SectionAction)> = None;
        for (i, key) in order.iter().enumerate() {
            let custom = !cvtext::SECTION_KEYS.contains(&key.as_str());
            let title = if custom { self.project.custom_sections.iter().find(|s| &s.id == key).map(|s| s.title.clone()).unwrap_or_default() } else { form_title(key).to_string() };
            let title = if title.trim().is_empty() { i18n::t("Egen seksjon").to_string() } else { title };
            if let Some(a) = self.section(ui, key, &title, i, order.len(), custom, |app, ui| app.section_body(ui, key, &mut changed)) {
                action = Some((i, a));
            }
        }
        if let Some((i, a)) = action {
            let mut order = order;
            match a {
                SectionAction::Up if i > 0 => order.swap(i, i - 1),
                SectionAction::Down if i + 1 < order.len() => order.swap(i, i + 1),
                SectionAction::Delete => {
                    let id = order.remove(i);
                    if let Some(at) = self.project.custom_sections.iter().position(|s| s.id == id) {
                        let before = (self.project.clone(), self.is_example);
                        let gone = self.project.custom_sections.remove(at);
                        self.undo = Some(before);
                        self.status = i18n::tf("Seksjonen «{title}» er slettet. Angre hvis det var feil.", &[("title", &gone.title)]);
                    }
                }
                _ => {}
            }
            self.project.options.section_order = order;
            changed = true;
        }
        ui.horizontal(|ui| {
            if ui.button(i18n::t("+ Legg til egen seksjon")).on_hover_text(i18n::t("For eksempel Prosjekter, Frivillig arbeid, Publikasjoner eller Verv")).clicked() {
                self.ask_section_name("");
            }
            if !self.project.options.section_order.is_empty()
                && theme::ghost(ui, i18n::t("Anbefalt rekkefølge"), i18n::t("Profil, kompetanse, erfaring, utdanning … – slik rekrutterere leser")).clicked()
            {
                self.project.options.section_order.clear();
                changed = true;
            }
        });
        ui.add_space(12.0);
        if changed {
            self.touched();
        }
    }

    /// One section as a card. The header folds it, moves it and (for the person's own sections) deletes it.
    #[allow(clippy::too_many_arguments)]
    fn section(&mut self, ui: &mut egui::Ui, key: &str, title: &str, index: usize, count: usize, deletable: bool, body: impl FnOnce(&mut CvApp, &mut egui::Ui)) -> Option<SectionAction> {
        let t = Tokens::get(ui.ctx());
        let mut action = None;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let open = !self.folded.contains(key);
            // The buttons first, from the right; the fold mark and the title get what is left (a
            // long title is shortened). Only the mark and the title fold the section.
            let header = ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if deletable && theme::icon_button(ui, Icon::Trash, i18n::t("Slett seksjonen"), true).clicked() {
                    action = Some(SectionAction::Delete);
                }
                if theme::icon_button(ui, Icon::Down, i18n::t("Flytt ned i CV-en"), index + 1 < count).clicked() {
                    action = Some(SectionAction::Down);
                }
                if theme::icon_button(ui, Icon::Up, i18n::t("Flytt opp i CV-en"), index > 0).clicked() {
                    action = Some(SectionAction::Up);
                }
                ui.add_space(10.0);
                self.section_color(ui, key);
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(if open { "–" } else { "+" }).size(16.0).color(t.faint));
                        theme::section_title(ui, title);
                    })
                    .response
                    .rect
                })
                .inner
            });
            let title_rect = header.inner.expand2(egui::vec2(4.0, 4.0));
            let click = ui.interact(title_rect, ui.scope_id().with((key, "fold")), egui::Sense::click()).on_hover_text(if open {
                i18n::t("Klikk for å skjule seksjonen i skjemaet. Den står fortsatt i CV-en.")
            } else {
                i18n::t("Klikk for å vise seksjonen")
            });
            if click.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                if open {
                    self.folded.insert(key.to_string());
                } else {
                    self.folded.remove(key);
                }
            }
            if open {
                ui.add_space(4.0);
                body(self, ui);
            }
        });
        ui.add_space(10.0);
        action
    }

    fn section_body(&mut self, ui: &mut egui::Ui, key: &str, changed: &mut bool) {
        match key {
            "profile" => self.profile_body(ui, changed),
            "skills" => self.skills_body(ui, changed),
            "experience" => self.experience_body(ui, changed),
            "education" => self.education_body(ui, changed),
            "certifications" => self.certifications_body(ui, changed),
            "languages" => self.languages_body(ui, changed),
            "interests" => self.interests_body(ui, changed),
            id => self.custom_body(ui, id, changed),
        }
    }

    // ---------- contact card ----------

    fn contact_card(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let t = Tokens::get(ui.ctx());
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t("Kontaktinformasjon"));
            ui.add_space(4.0);
            ui.horizontal_top(|ui| {
                self.portrait(ui, changed);
                ui.vertical(|ui| {
                    let c = &mut self.project.candidate;
                    *changed |= field(ui, i18n::t("Fullt navn"), i18n::t("Navnet ditt slik du vil bli tiltalt. Det står øverst i CV-en."), &mut c.name);
                    *changed |=
                        field(ui, i18n::t("Faglig overskrift · bruk stillingens ord"), i18n::t("Bruk gjerne stillingstittelen fra annonsen – rekrutterere og ATS ser etter den."), &mut c.headline);
                });
            });
            if let Some(original) = self.reference.as_ref().map(|r| r.candidate.headline.clone()) {
                reference_hint(ui, &original);
            }
            ui.add_space(6.0);
            theme::label(ui, i18n::t("Kontaktlinjen · i denne rekkefølgen i CV-en"));
            let order = cvtext::contact_order(&self.project);
            let mut moved: Option<(usize, bool)> = None;
            let mut delete: Option<String> = None;
            for (i, key) in order.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if theme::icon_button(ui, Icon::Up, i18n::t("Flytt opp i kontaktlinjen"), i > 0).clicked() {
                        moved = Some((i, true));
                    }
                    if theme::icon_button(ui, Icon::Down, i18n::t("Flytt ned i kontaktlinjen"), i + 1 < order.len()).clicked() {
                        moved = Some((i, false));
                    }
                    ui.add_space(6.0);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let c = &mut self.project.candidate;
                    let fixed = CONTACT_KEYS.contains(&key.as_str());
                    let room = ui.available_width() - if fixed { 0.0 } else { 34.0 };
                    if fixed {
                        let (value, hint, tip) = match key.as_str() {
                            "email" => (&mut c.email, i18n::t("navn@epost.no"), i18n::t("E-posten rekrutterere svarer til. Bruk en nøktern adresse, gjerne med navnet ditt.")),
                            "phone" => (&mut c.phone, "+47 900 00 000", i18n::t("Mobilnummeret ditt, gjerne med landkode")),
                            "city" => (&mut c.city, i18n::t("Bergen, Norge"), i18n::t("Hvor du bor. By og land holder – full adresse trengs ikke.")),
                            _ => (&mut c.linkedin, "https://www.linkedin.com/in/…", i18n::t("Lenken til LinkedIn-profilen din. Mange rekrutterere sjekker den.")),
                        };
                        ui.add_sized([88.0, 22.0], egui::Label::new(RichText::new(contact_label(key)).size(12.5).color(t.dim)));
                        *changed |= inline(ui, value, hint, room - 94.0, tip);
                    } else if let Some(x) = c.extra.iter_mut().find(|x| &x.id == key) {
                        *changed |= inline(ui, &mut x.label, i18n::t("Navn, f.eks. GitHub"), 88.0, i18n::t("Hva feltet heter, f.eks. GitHub, Portefølje eller Førerkort"));
                        *changed |= inline(ui, &mut x.value, i18n::t("Verdi"), room - 100.0, i18n::t("Lenken eller verdien, f.eks. github.com/navn eller Klasse B"));
                        if theme::remove_button(ui, i18n::t("Fjern kontaktfeltet")).clicked() {
                            delete = Some(key.clone());
                        }
                    }
                });
            }
            if let Some((i, up)) = moved {
                let mut order = order.clone();
                let j = if up { i - 1 } else { i + 1 };
                order.swap(i, j);
                self.project.candidate.contact_order = order;
                *changed = true;
            }
            if let Some(id) = delete {
                self.project.candidate.extra.retain(|x| x.id != id);
                self.project.candidate.contact_order.retain(|k| k != &id);
                *changed = true;
            }
            ui.horizontal(|ui| {
                if ui.button(i18n::t("+ Legg til kontaktfelt")).on_hover_text(i18n::t("For eksempel GitHub, portefølje, nettside eller førerkort")).clicked() {
                    let id = self.next_id("contact");
                    self.project.candidate.extra.push(ContactField { id, label: String::new(), value: String::new() });
                    *changed = true;
                }
                if !self.project.candidate.contact_order.is_empty() && theme::ghost(ui, i18n::t("Anbefalt rekkefølge"), i18n::t("Tilbake til standard rekkefølge i kontaktlinjen")).clicked() {
                    self.project.candidate.contact_order.clear();
                    *changed = true;
                }
            });
        });
        ui.add_space(10.0);
    }

    /// The portrait: a picture (or an empty frame) with clear buttons beside it.
    fn portrait(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let t = Tokens::get(ui.ctx());
        ui.vertical(|ui| {
            ui.set_width(86.0);
            let size = egui::vec2(78.0, 100.0);
            match self.portrait_texture(ui.ctx()) {
                Some(tex) => {
                    let r = ui.add(egui::Image::new((tex.id(), size)).corner_radius(CornerRadius::same(theme::R)).sense(egui::Sense::click()));
                    if r.on_hover_text(i18n::t("Bytt bilde")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.pick("photo", None);
                    }
                }
                None => {
                    let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
                    let hot = r.hovered();
                    ui.painter().rect(rect, CornerRadius::same(theme::R), if hot { t.accent_soft } else { t.hover }, Stroke::new(1.0, if hot { t.accent } else { t.border }), egui::StrokeKind::Inside);
                    ui.painter().text(rect.center() - egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER, "+", egui::FontId::new(22.0, theme::medium()), if hot { t.accent } else { t.faint });
                    ui.painter().text(
                        rect.center() + egui::vec2(0.0, 14.0),
                        egui::Align2::CENTER_CENTER,
                        i18n::t("Legg til bilde"),
                        egui::FontId::new(11.0, egui::FontFamily::Proportional),
                        if hot { t.accent } else { t.dim },
                    );
                    if r.on_hover_text(i18n::t("Valgfritt portrett. Vanlig i Norge, men ikke påkrevd.")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.pick("photo", None);
                    }
                }
            }
            if !self.project.candidate.photo.is_empty() {
                *changed |= ui
                    .checkbox(&mut self.project.options.photo, RichText::new(i18n::t("Vis i CV")).size(12.0))
                    .on_hover_text(i18n::t("Ta med bildet i CV-en, eller behold det her uten å vise det"))
                    .changed();
                if theme::ghost(ui, i18n::t("Fjern bilde"), i18n::t("Slett portrettet fra utkastet")).clicked() {
                    self.project.candidate.photo.clear();
                    self.project.options.photo = false;
                    *changed = true;
                }
            }
        });
    }

    /// The job ad, first in the form: paste it, fetch it from its link or read it from a file,
    /// and CVBuilder fills in the role, the employer and the keywords, and adds the keywords
    /// the CV already shows to the skills.
    fn target_card(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let key = "target";
        let open = !self.folded.contains(key);
        let t = Tokens::get(ui.ctx());
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.5, t.accent)).corner_radius(CornerRadius::same(theme::R_LG)).inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let header = ui.horizontal(|ui| {
                ui.label(RichText::new(if open { "–" } else { "+" }).size(16.0).color(t.faint));
                theme::section_title(ui, i18n::t("Stillingsannonsen du søker på"));
            });
            if ui.interact(header.response.rect, ui.scope_id().with("fold-target"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(if open { i18n::t("Klikk for å skjule annonsefeltene") } else { i18n::t("Klikk for å vise annonsefeltene") }).clicked() {
                if open {
                    self.folded.insert(key.into());
                } else {
                    self.folded.remove(key);
                }
            }
            if !open {
                return;
            }
            theme::hint(ui, i18n::t("Start her. Lim inn annonsen, hent den fra lenken eller fra en fil. CVBuilder fyller ut stilling, arbeidsgiver og fagord, og legger fagord du allerede har i CV-en inn i kompetanse. Annonsen står ikke i CV-en."));
            ui.add_space(4.0);
            theme::label(ui, i18n::t("Lenke til annonsen"));
            // The button first, from the right, and the field takes what is left: a fixed share
            // for the button made the row too wide in languages with a longer label, and the
            // form panel grew a little every frame (found by ui_qa.rs in French).
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 31.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(self.import.busy.is_none(), egui::Button::new(i18n::t("Hent annonsen"))).on_hover_text(i18n::t("Henter teksten fra lenken og fyller ut")).on_disabled_hover_text(i18n::t("Vent til annonsen er hentet")).clicked() {
                    self.fetch_ad();
                }
                let r = ui.add(egui::TextEdit::singleline(&mut self.project.target.url).hint_text("https://…").desired_width(ui.available_width()).margin(Margin::symmetric(8, 6)));
                *changed |= r.changed();
                full_text_hint(r, i18n::t("Lim inn lenken til annonsen, så kan CVBuilder hente teksten"), &self.project.target.url);
            });
            ui.add_space(2.0);
            theme::label(ui, i18n::t("Annonseteksten · lim inn med Ctrl+V"));
            // The text before this frame's edit, so a pasted new ad can replace what the old one filled in.
            let before = self.project.target.description.clone();
            let r = ui.add(egui::TextEdit::multiline(&mut self.project.target.description).id_salt("ad-text").hint_text(i18n::t("Lim inn hele stillingsannonsen her …")).desired_rows(6).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6))).on_hover_text(i18n::t("Hele annonsen. CVBuilder finner stilling, arbeidsgiver og fagord. Annonsen kommer ikke med i CV-en."));
            if self.import.focus_ad {
                r.request_focus();
                r.scroll_to_me(Some(egui::Align::TOP));
                self.import.focus_ad = false;
            }
            if r.changed() {
                *changed = true;
                // A whole ad pasted at once: fill in what can be read right away.
                if self.project.target.description.chars().count() > before.chars().count() + 200 {
                    self.read_pasted_ad(Some(before));
                }
            }
            let idle = self.import.busy.is_none();
            ui.horizontal_wrapped(|ui| {
                if ui.button(i18n::t("Fyll ut fra annonsen")).on_hover_text(i18n::t("Stilling, arbeidsgiver og fagord – uten AI")).clicked() {
                    self.read_pasted_ad(None);
                }
                if ui.add_enabled(idle, egui::Button::new(i18n::t("Fra fil"))).on_hover_text(i18n::t("PDF, Word, tekst eller en lagret nettside")).on_disabled_hover_text(i18n::t("Vent til annonsen er hentet")).clicked() {
                    self.pick("ad_file", None);
                }
                if ai_button(ui, i18n::t("Tilpass CV-en med AI"), i18n::t("AI-en går gjennom CV-en mot annonsen, uten å finne på noe")).clicked() {
                    self.ask_ai(i18n::t("Tilpass CV-en til stillingsannonsen: bruk annonsens ord der de stemmer med det CV-en allerede sier, løft frem det mest relevante i profil og punkter, og lagre fagordene. Ikke legg til noe jeg ikke har. Si til slutt hva jeg kan legge til selv."));
                }
            });
            // Fetching and reading run on a worker; show that something is happening.
            if let Some(busy) = &self.import.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(busy.as_str()).size(12.5).color(t.faint));
                });
            }
            if !self.import.ad_note.is_empty() {
                egui::Frame::new().fill(t.accent_soft).corner_radius(CornerRadius::same(theme::R_SM)).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(&self.import.ad_note).size(12.5)).wrap());
                });
            }
            ui.add_space(4.0);
            let tg = &mut self.project.target;
            pair(ui, |ui, i| {
                if i == 0 {
                    *changed |= field(ui, i18n::t("Stilling"), i18n::t("Stillingen du søker på, slik den står i annonsen"), &mut tg.role);
                } else {
                    *changed |= field(ui, i18n::t("Arbeidsgiver"), i18n::t("Virksomheten som har lyst ut stillingen"), &mut tg.company);
                }
            });
            *changed |= lines(ui, &mut self.buffers, "keywords", i18n::t("Viktige fagord fra annonsen · ett per linje"), i18n::t("Ordene ATS og rekrutterere ser etter. Bruk dem i CV-en der de stemmer."), &mut self.project.target.keywords);
            if ai_button(ui, i18n::t("Finn fagord med AI"), i18n::t("AI-en leser annonsen og lagrer de viktigste fagordene, høyst 25")).clicked() {
                self.ask_ai(i18n::t("Les stillingsannonsen og lagre de viktigste fagordene fra den (høyst 25)."));
            }
        });
        ui.add_space(10.0);
    }

    // ---------- the sections ----------

    fn profile_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        *changed |= area(ui, i18n::t("Kort profil · 30–70 ord"), i18n::t("Hvem du er, hva du kan og hva du søker. Det første rekrutterere leser."), &mut self.project.candidate.summary, 4);
        if let Some(original) = self.reference.as_ref().map(|r| r.candidate.summary.clone()) {
            reference_hint(ui, &original);
        }
        if ai_button(ui, i18n::t("Skriv profil med AI"), i18n::t("AI-en skriver en profil ut fra det som allerede står i CV-en")).clicked() {
            self.ask_ai(i18n::t("Skriv en kort profiltekst (30–70 ord) ut fra det som står i CV-en, rettet mot stillingen jeg søker."));
        }
    }

    fn skills_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        theme::hint(ui, i18n::t("Bruk annonsens ordlyd der den stemmer. Bare det du kan stå for."));
        let mut remove = None;
        for (i, s) in self.project.skills.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                *changed |= inline(ui, &mut s.group, i18n::t("Gruppe"), 110.0, i18n::t("Gruppen den står under i CV-en, f.eks. Verktøy. Samme gruppe samles."));
                let w = ui.available_width() - 44.0;
                *changed |= inline(ui, &mut s.name, i18n::t("Kompetanse eller verktøy"), w, i18n::t("Én kompetanse eller ett verktøy. Bruk annonsens ord – ATS ser etter dem."));
                if theme::remove_button(ui, i18n::t("Fjern kompetansen")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.project.skills.remove(i);
            *changed = true;
        }
        if ui.button(i18n::t("+ Legg til kompetanse")).on_hover_text(i18n::t("Ny kompetanse eller nytt verktøy. Bruk annonsens ord – ATS ser etter dem.")).clicked() {
            let group = self.project.skills.last().map(|s| s.group.clone()).unwrap_or_default();
            self.project.skills.push(Skill { group, name: String::new(), evidence: String::new(), confirmed: true });
            *changed = true;
        }
    }

    fn experience_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        theme::hint(ui, i18n::t("2–4 korte punkter per jobb. Start med et verb og vis resultatet: «Oppnådde X, målt ved Y, ved å gjøre Z»."));
        let tk = Tokens::get(ui.ctx());
        let mut remove = None;
        let mut ai_job: Option<usize> = None;
        for i in 0..self.project.experience.len() {
            let title = {
                let j = &self.project.experience[i];
                if j.company.is_empty() && j.title.is_empty() { i18n::tf("Jobb {n}", &[("n", &(i + 1).to_string())]) } else { format!("{} · {}", j.title, j.company) }
            };
            let original = self.reference.as_ref().and_then(|r| r.experience.iter().find(|x| x.id == self.project.experience[i].id)).map(|j| j.bullets.join("\n"));
            sub_card(ui, &tk, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).family(theme::medium()).color(tk.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, Icon::Trash, i18n::t("Slett jobben"), true).clicked() {
                            remove = Some(i);
                        }
                    });
                });
                let key = self.project.experience[i].id.clone();
                let j = &mut self.project.experience[i];
                pair(ui, |ui, i| {
                    if i == 0 {
                        *changed |= field(ui, i18n::t("Stillingstittel"), i18n::t("Tittelen du hadde. Bruk den vanlige betegnelsen, så ATS kjenner den igjen."), &mut j.title);
                    } else {
                        *changed |= field(ui, i18n::t("Arbeidsgiver"), i18n::t("Firmaet eller virksomheten du jobbet for"), &mut j.company);
                    }
                });
                *changed |= field(ui, i18n::t("Sted · by"), i18n::t("Byen du jobbet i, eller «Hjemmekontor». Valgfritt."), &mut j.location);
                ui.horizontal_top(|ui| {
                    *changed |= date(
                        ui,
                        &mut self.buffers,
                        &mut self.bad_dates,
                        &format!("{key}.start"),
                        i18n::t("Fra (09.2021 eller 2010)"),
                        i18n::t("Måned og år du begynte. Bare året går også."),
                        &mut j.start,
                        false,
                    );
                    let end_key = format!("{key}.end");
                    if self.buffers.get(&end_key).is_some_and(|b| checks::is_now(b)) {
                        // «nå» typed as an end date: this is the current job.
                        j.end = None;
                        self.buffers.remove(&end_key);
                        self.bad_dates.remove(&end_key);
                        *changed = true;
                    }
                    if let Some(end) = j.end.as_mut() {
                        *changed |= date(
                            ui,
                            &mut self.buffers,
                            &mut self.bad_dates,
                            &end_key,
                            i18n::t("Til (eller «nå»)"),
                            i18n::t("Måned og år du sluttet, eller «nå» hvis du jobber der fortsatt"),
                            end,
                            false,
                        );
                    }
                });
                let mut current = j.end.is_none();
                if ui.checkbox(&mut current, i18n::t("Nåværende jobb")).on_hover_text(i18n::t("Jobben pågår: CV-en viser «nå» i stedet for en sluttdato")).changed() {
                    j.end = if current { None } else { Some(String::new()) };
                    *changed = true;
                }
                *changed |= lines(
                    ui,
                    &mut self.buffers,
                    &format!("{key}.bullets"),
                    i18n::t("Hva du gjorde og oppnådde · ett punkt per linje"),
                    i18n::t("Ett resultat per linje. Start med et verb og ta med tall der du kan."),
                    &mut j.bullets,
                );
                if let Some(o) = &original {
                    reference_hint(ui, o);
                }
                if ai_button(ui, i18n::t("Forbedre punktene med AI"), i18n::t("AI-en skriver om punktene så de viser handling og resultat, uten å finne på noe")).clicked() {
                    ai_job = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            let before = (self.project.clone(), self.is_example);
            let gone = self.project.experience.remove(i);
            self.undo = Some(before);
            self.status = i18n::tf("Jobben «{title}» er slettet. Angre hvis det var feil.", &[("title", &gone.title)]);
            *changed = true;
        }
        if let Some(i) = ai_job {
            let j = &self.project.experience[i];
            let ask = i18n::tf(
                "Forbedre punktene for jobben «{title}» hos {company} (id {id}), slik at de viser handling og resultat og bruker annonsens ord der det stemmer.",
                &[("title", &j.title), ("company", &j.company), ("id", &j.id)],
            );
            self.ask_ai(&ask);
        }
        if ui.button(i18n::t("+ Legg til jobb")).on_hover_text(i18n::t("Ny jobb i arbeidserfaringen")).clicked() {
            let id = self.next_id("job");
            self.project.experience.push(Job { id, end: Some(String::new()), confirmed: true, ..Default::default() });
            *changed = true;
        }
    }

    fn education_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let tk = Tokens::get(ui.ctx());
        let mut remove = None;
        for (i, e) in self.project.education.iter_mut().enumerate() {
            sub_card(ui, &tk, |ui| {
                ui.horizontal(|ui| {
                    theme::label(ui, i18n::t("Grad, fagbrev eller studium"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, Icon::Trash, i18n::t("Slett utdanningen"), true).clicked() {
                            remove = Some(i);
                        }
                    });
                });
                *changed |= inline(ui, &mut e.qualification, i18n::t("F.eks. Bachelor i informatikk"), f32::INFINITY, i18n::t("Det offisielle navnet på graden, fagbrevet eller studiet"));
                *changed |= field(ui, i18n::t("Lærested"), i18n::t("Skolen, høyskolen eller universitetet"), &mut e.institution);
                ui.horizontal_top(|ui| {
                    *changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &format!("edu{i}.start"), i18n::t("Fra · år"), i18n::t("Året du begynte, f.eks. 2018"), &mut e.start, true);
                    *changed |= date(ui, &mut self.buffers, &mut self.bad_dates, &format!("edu{i}.end"), i18n::t("Til · år"), i18n::t("Året du ble ferdig, eller ventes ferdig"), &mut e.end, true);
                });
            });
        }
        if let Some(i) = remove {
            self.project.education.remove(i);
            self.buffers.retain(|k, _| !k.starts_with("edu"));
            *changed = true;
        }
        if ui.button(i18n::t("+ Legg til utdanning")).on_hover_text(i18n::t("Ny utdanning: grad, fagbrev eller studium")).clicked() {
            self.project.education.push(Education::default());
            *changed = true;
        }
    }

    fn certifications_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let mut remove = None;
        for (i, c) in self.project.certifications.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let w = ui.available_width() - 44.0 - 60.0 - 16.0;
                *changed |= inline(ui, &mut c.name, i18n::t("Kurs eller sertifisering"), w * 0.6, i18n::t("Navnet slik utstederen skriver det – ATS ser ofte etter nøyaktig det"));
                *changed |= inline(ui, &mut c.issuer, i18n::t("Utsteder"), w * 0.4, i18n::t("Hvem som holdt kurset eller utstedte sertifiseringen"));
                *changed |= year_inline(ui, &mut self.buffers, &mut self.bad_dates, &format!("cert{i}.year"), i18n::t("Året du fullførte. Valgfritt."), &mut c.year);
                if theme::remove_button(ui, i18n::t("Fjern kurset")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.project.certifications.remove(i);
            self.buffers.retain(|k, _| !k.starts_with("cert"));
            *changed = true;
        }
        if ui.button(i18n::t("+ Legg til kurs")).on_hover_text(i18n::t("Nytt kurs eller ny sertifisering. Sertifiseringer i annonsen er ofte noe ATS ser etter.")).clicked() {
            self.project.certifications.push(Certification::default());
            *changed = true;
        }
    }

    fn languages_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        let mut remove = None;
        for (i, l) in self.project.languages.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                *changed |= inline(ui, &mut l.name, i18n::t("Språk"), 110.0, i18n::t("Språket, f.eks. Engelsk"));
                let w = ui.available_width() - 44.0;
                *changed |= inline(ui, &mut l.level, i18n::t("Nivå: morsmål, flytende, B2 …"), w, i18n::t("Hvor godt du kan det. Nivåer som B2 eller C1 er tydelige for rekrutterere."));
                if theme::remove_button(ui, i18n::t("Fjern språket")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.project.languages.remove(i);
            *changed = true;
        }
        if ui.button(i18n::t("+ Legg til språk")).on_hover_text(i18n::t("Nytt språk med nivå, f.eks. engelsk – flytende")).clicked() {
            self.project.languages.push(SpokenLanguage::default());
            *changed = true;
        }
    }

    fn interests_body(&mut self, ui: &mut egui::Ui, changed: &mut bool) {
        theme::hint(ui, i18n::t("Valgfritt. Ta med høyst tre korte, ekte interesser."));
        *changed |= ui
            .checkbox(&mut self.project.options.include_interests, i18n::t("Ta med interesser i CV-en"))
            .on_hover_text(i18n::t("Interesser er valgfrie. Slå av for å holde dem utenfor CV-en."))
            .changed();
        let selected = self.project.options.interest_ids.clone();
        let mut remove = None;
        for (i, item) in self.project.interests.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut on = selected.contains(&item.id);
                if ui
                    .add_enabled(on || selected.len() < 3, egui::Checkbox::without_text(&mut on))
                    .on_hover_text(i18n::t("Ta med i CV-en"))
                    .on_disabled_hover_text(i18n::t("Du har valgt tre. Fjern et kryss for å velge denne."))
                    .changed()
                {
                    if on {
                        self.project.options.interest_ids.push(item.id.clone());
                    } else {
                        self.project.options.interest_ids.retain(|x| x != &item.id);
                    }
                    *changed = true;
                }
                let w = (ui.available_width() - 160.0).max(80.0);
                *changed |= inline(ui, &mut item.label, i18n::t("Interesse"), w, i18n::t("Kort og konkret, f.eks. Maratonløping eller Korps"));
                egui::ComboBox::from_id_salt(("status", i))
                    .width(96.0)
                    .selected_text(match item.status {
                        InterestStatus::Current => i18n::t("Nå"),
                        InterestStatus::Former => i18n::t("Tidligere"),
                        InterestStatus::Interest => i18n::t("Interesse for"),
                    })
                    .show_ui(ui, |ui| {
                        *changed |= ui.selectable_value(&mut item.status, InterestStatus::Current, i18n::t("Nå")).on_hover_text(i18n::t("Noe du driver med nå")).changed();
                        *changed |= ui.selectable_value(&mut item.status, InterestStatus::Former, i18n::t("Tidligere")).on_hover_text(i18n::t("Noe du har drevet med før")).changed();
                        *changed |=
                            ui.selectable_value(&mut item.status, InterestStatus::Interest, i18n::t("Interesse for")).on_hover_text(i18n::t("Noe du er interessert i, men ikke driver med")).changed();
                    })
                    .response
                    .on_hover_text(i18n::t("Om du driver med det nå, gjorde det før, eller er interessert i det"));
                if theme::remove_button(ui, i18n::t("Fjern interessen")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            let id = self.project.interests.remove(i).id;
            self.project.options.interest_ids.retain(|x| x != &id);
            *changed = true;
        }
        if ui.button(i18n::t("+ Legg til interesse")).on_hover_text(i18n::t("Ny interesse. Høyst tre kommer med i CV-en.")).clicked() {
            let id = self.next_id("interest");
            self.project.interests.push(Interest { id, confirmed: true, category: "other".into(), specificity: 1, ..Default::default() });
            *changed = true;
        }
    }

    /// The person's own section: a title and any number of entries.
    fn custom_body(&mut self, ui: &mut egui::Ui, id: &str, changed: &mut bool) {
        let tk = Tokens::get(ui.ctx());
        let Some(si) = self.project.custom_sections.iter().position(|s| s.id == id) else {
            return;
        };
        *changed |= field(ui, i18n::t("Overskrift i CV-en"), i18n::t("Overskriften seksjonen får i CV-en, f.eks. Prosjekter eller Frivillig arbeid"), &mut self.project.custom_sections[si].title);
        let mut remove = None;
        let count = self.project.custom_sections[si].items.len();
        for i in 0..count {
            let key = format!("{id}.{i}.bullets");
            sub_card(ui, &tk, |ui| {
                let item = &mut self.project.custom_sections[si].items[i];
                ui.horizontal(|ui| {
                    theme::label(ui, i18n::t("Tittel"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, Icon::Trash, i18n::t("Slett oppføringen"), true).clicked() {
                            remove = Some(i);
                        }
                    });
                });
                *changed |= inline(ui, &mut item.title, i18n::t("F.eks. Ny barnehage, Trondheim"), f32::INFINITY, i18n::t("Navnet på prosjektet, vervet eller det du vil vise"));
                pair(ui, |ui, i| {
                    if i == 0 {
                        *changed |= field(ui, i18n::t("Organisasjon eller sted"), i18n::t("Hvor eller for hvem, f.eks. firma, forening eller by"), &mut item.subtitle);
                    } else {
                        *changed |= field(ui, i18n::t("Tid (fritt, f.eks. 2023)"), i18n::t("Når, skrevet som du vil, f.eks. 2023 eller 2021–2023"), &mut item.date);
                    }
                });
                *changed |= lines(ui, &mut self.buffers, &key, i18n::t("Punkter · ett per linje"), i18n::t("Hva du gjorde og hva det ga, ett punkt per linje"), &mut item.bullets);
            });
        }
        if let Some(i) = remove {
            self.project.custom_sections[si].items.remove(i);
            self.buffers.retain(|k, _| !k.starts_with(&format!("{id}.")));
            *changed = true;
        }
        if ui.button(i18n::t("+ Legg til oppføring")).on_hover_text(i18n::t("Ny oppføring i denne seksjonen")).clicked() {
            self.project.custom_sections[si].items.push(CustomItem::default());
            *changed = true;
        }
    }
}

// ---------- small helpers ----------

fn sub_card(ui: &mut egui::Ui, t: &Tokens, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(t.panel).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui);
    });
    ui.add_space(6.0);
}

/// The text as it was before the language was switched, to translate from.
fn reference_hint(ui: &mut egui::Ui, original: &str) {
    if original.trim().is_empty() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R_SM)).inner_margin(Margin::symmetric(8, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(i18n::t("Original før språkbytte:")).size(11.5).family(theme::medium()).color(t.dim));
        ui.add(egui::Label::new(RichText::new(original).size(12.0).color(t.dim)).wrap().selectable(true));
    });
}

/// A button that hands a request to the AI chat, saying on hover what the AI will do.
pub(crate) fn ai_button(ui: &mut egui::Ui, text: &str, tip: &str) -> egui::Response {
    debug_assert!(!tip.trim().is_empty(), "the AI button «{text}» needs a tip");
    let t = Tokens::get(ui.ctx());
    ui.add(egui::Button::new(RichText::new(text).color(t.accent).size(13.0)).fill(t.accent_soft).stroke(Stroke::NONE).corner_radius(CornerRadius::same(theme::R))).on_hover_text(tip)
}

/// A labelled one-line field. `tip` says what to write; it shows on hover.
fn field(ui: &mut egui::Ui, label: &str, tip: &str, value: &mut String) -> bool {
    theme::label(ui, label);
    let r = ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6)));
    let changed = r.changed();
    full_text_hint(r, tip, value);
    ui.add_space(2.0);
    changed
}

/// A one-line field inside a row, without its own label: `hint` shows while it is empty.
fn inline(ui: &mut egui::Ui, value: &mut String, hint: &str, width: f32, tip: &str) -> bool {
    let r = ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(width).margin(Margin::symmetric(8, 5)));
    let changed = r.changed();
    full_text_hint(r, tip, value);
    changed
}

/// The field's tip on hover, and below it the whole value when the field is too narrow for it.
fn full_text_hint(r: egui::Response, tip: &str, value: &str) {
    debug_assert!(!tip.trim().is_empty(), "a field needs a tip");
    let fits = (value.chars().count() as f32) * 7.6 < r.rect.width() - 18.0;
    if !value.is_empty() && !fits {
        r.on_hover_text(format!("{tip}\n\n{value}"));
    } else {
        r.on_hover_text(tip);
    }
}

fn area(ui: &mut egui::Ui, label: &str, tip: &str, value: &mut String, rows: usize) -> bool {
    debug_assert!(!tip.trim().is_empty(), "the field «{label}» needs a tip");
    theme::label(ui, label);
    ui.add(egui::TextEdit::multiline(value).desired_rows(rows).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6))).on_hover_text(tip).changed()
}

/// A list typed one item per line. The raw text is kept while typing, so new lines work.
fn lines(ui: &mut egui::Ui, buffers: &mut HashMap<String, String>, key: &str, label: &str, tip: &str, list: &mut Vec<String>) -> bool {
    debug_assert!(!tip.trim().is_empty(), "the field «{label}» needs a tip");
    theme::label(ui, label);
    let buffer = buffers.entry(key.to_string()).or_insert_with(|| list.join("\n"));
    if ui.add(egui::TextEdit::multiline(buffer).desired_rows(3).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6))).on_hover_text(tip).changed() {
        *list = buffer.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        return true;
    }
    false
}

/// Reads a typed date when the field is left. Only a readable date is ever stored.
fn commit_date(response: &egui::Response, buffer: &mut String, bad: &mut HashSet<String>, key: &str, value: &mut String, year: bool) -> bool {
    let shown = |v: &str| {
        if year { v.to_string() } else { checks::show_month(v) }
    };
    let mut changed = false;
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
    changed
}

/// A month or year typed the way people write it. `tip` says which date; it shows on hover.
#[allow(clippy::too_many_arguments)]
fn date(ui: &mut egui::Ui, buffers: &mut HashMap<String, String>, bad: &mut HashSet<String>, key: &str, label: &str, tip: &str, value: &mut String, year: bool) -> bool {
    debug_assert!(!tip.trim().is_empty(), "the date «{label}» needs a tip");
    let shown = |v: &str| {
        if year { v.to_string() } else { checks::show_month(v) }
    };
    let mut changed = false;
    ui.vertical(|ui| {
        ui.set_width(if year { 80.0 } else { 130.0 });
        theme::label(ui, label);
        let buffer = buffers.entry(key.to_string()).or_insert_with(|| shown(value));
        let response = ui.add(egui::TextEdit::singleline(buffer).hint_text(if year { "2021" } else { "09.2021" }).desired_width(if year { 70.0 } else { 110.0 }).margin(Margin::symmetric(8, 6)));
        changed = commit_date(&response, buffer, bad, key, value, year);
        response.on_hover_text(tip);
        if bad.contains(key) {
            let t = Tokens::get(ui.ctx());
            ui.add(egui::Label::new(RichText::new(if year { i18n::t("Skriv årstall, f.eks. 2021.") } else { i18n::t("Skriv måned og år, f.eks. 09.2021.") }).size(12.0).color(t.danger)).wrap());
        }
    });
    changed
}

/// A year field inside a row, without its own label. On hover: `tip`, and what is wrong with
/// what was typed.
fn year_inline(ui: &mut egui::Ui, buffers: &mut HashMap<String, String>, bad: &mut HashSet<String>, key: &str, tip: &str, value: &mut String) -> bool {
    debug_assert!(!tip.trim().is_empty(), "a year field needs a tip");
    let buffer = buffers.entry(key.to_string()).or_insert_with(|| value.clone());
    let response = ui.add(egui::TextEdit::singleline(buffer).hint_text(i18n::t("År")).desired_width(52.0).margin(Margin::symmetric(8, 5)));
    let changed = commit_date(&response, buffer, bad, key, value, true);
    if bad.contains(key) {
        response.on_hover_text(format!("{tip}\n\n{}", i18n::t("Skriv årstall, f.eks. 2021.")));
    } else {
        response.on_hover_text(tip);
    }
    changed
}

/// Two fields side by side when the panel is wide enough, otherwise one under the other.
fn pair(ui: &mut egui::Ui, mut field: impl FnMut(&mut egui::Ui, usize)) {
    if ui.available_width() >= 320.0 {
        ui.columns(2, |cols| {
            for (i, col) in cols.iter_mut().enumerate() {
                field(col, i);
            }
        });
    } else {
        field(ui, 0);
        field(ui, 1);
    }
}
