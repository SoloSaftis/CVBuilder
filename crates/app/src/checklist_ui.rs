//! The person's own checklist in the «Sjekkliste» tab: choose it instead of the built-in
//! recruiter check, tick off points, edit it, load one from a file or save it as a file.
//! The list lives in the data folder; the ticks belong to each CV.
use cv_core::checklist::{self, Checklist, Item, Kind, State};
use egui::{CornerRadius, Margin, RichText};

use crate::i18n;
use crate::theme::{self, Icon, Tokens};
use crate::{CvApp, storage};

#[derive(Default)]
pub struct ChecklistState {
    /// The person's own list, when they have one.
    pub own: Option<Checklist>,
    /// A copy being edited in the editor window.
    pub editing: Option<Checklist>,
    pub message: String,
}

impl ChecklistState {
    /// The state at start-up: the person's own list from the data folder. A file that is there
    /// but cannot be used is said out loud, so «Rediger» does not quietly replace it.
    pub fn load() -> ChecklistState {
        match load_own() {
            Ok(own) => ChecklistState { own, ..Default::default() },
            Err(e) => ChecklistState { message: i18n::tf("Sjekklisten din kunne ikke leses: {error}", &[("error", &translate(&e))]), ..Default::default() },
        }
    }
}

fn file() -> std::path::PathBuf {
    storage::data_dir().join("sjekkliste.cvchecklist.json")
}

/// The person's own checklist from the data folder: `Ok(None)` when they have none.
pub fn load_own() -> Result<Option<Checklist>, String> {
    match std::fs::read(file()) {
        Ok(bytes) => checklist::parse_bytes(&bytes).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Writes then renames, so a crash never leaves half a list behind.
fn save_own(list: &Checklist) -> Result<(), String> {
    let path = file();
    std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, checklist::to_file(list)).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// cv-core's messages, one per line, in the menu language.
fn translate(text: &str) -> String {
    text.lines().map(i18n::tdyn).collect::<Vec<_>>().join("\n")
}

/// Reads a checklist file the person picked. Too big a file is refused before it is read.
fn read_file(path: &std::path::Path) -> Result<Checklist, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > checklist::MAX_FILE_BYTES as u64 {
        return Err("Filen er for stor til å være en CVBuilder-sjekkliste.".into());
    }
    checklist::parse_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
}

/// The kind of a point changed: the old value meant something else (a word is not a page
/// count), so start the new kind afresh.
fn set_kind(item: &mut Item, kind: Kind) {
    if item.kind != kind {
        item.kind = kind;
        item.value = kind.starting_value();
    }
}

/// The value of a page-count point, kept to a valid number.
fn pages_value(item: &Item) -> usize {
    item.value.trim().parse::<usize>().ok().filter(|n| (1..=10).contains(n)).unwrap_or(2)
}

/// A file name the «Last inn» dialog will show again: it lists `.json` files only.
fn with_extension(path: &std::path::Path) -> std::path::PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) { path.to_path_buf() } else { std::path::PathBuf::from(format!("{}.cvchecklist.json", path.display())) }
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Manual => i18n::t("Jeg krysser av selv"),
        Kind::Contains => i18n::t("CV-en nevner …"),
        Kind::NotContains => i18n::t("CV-en nevner ikke …"),
        Kind::MaxPages => i18n::t("Høyst antall sider"),
        Kind::Filled => i18n::t("Feltet er fylt ut"),
    }
}

impl CvApp {
    /// The row at the top of the «Sjekkliste» tab: which list, and the tools for one's own.
    pub(crate) fn checklist_choice(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let own = self.settings.own_checklist;
            if ui.radio(!own, i18n::t("Standard")).on_hover_text(i18n::t("Rekrutterersjekken CVBuilder har innebygd")).clicked() && own {
                self.settings.own_checklist = false;
                storage::save_settings(&self.settings);
            }
            if ui.radio(own, i18n::t("Min egen")).on_hover_text(i18n::t("Din egen sjekkliste i stedet for standardsjekken")).clicked() && !own {
                if self.checklist.own.is_none() {
                    self.checklist.editing = Some(Checklist::starter());
                }
                self.settings.own_checklist = true;
                storage::save_settings(&self.settings);
            }
            ui.add_space(8.0);
            if ui.button(i18n::t("Rediger")).on_hover_text(i18n::t("Legg til, endre og fjern punkter i din egen sjekkliste")).clicked() {
                self.checklist.editing = Some(self.checklist.own.clone().unwrap_or_else(Checklist::starter));
                self.checklist.message.clear();
            }
            if ui.button(i18n::t("Last inn")).on_hover_text(i18n::t("Hent en sjekkliste fra en .cvchecklist.json-fil")).clicked() {
                self.pick("checklist_load", None);
            }
            if ui
                .add_enabled(self.checklist.own.is_some(), egui::Button::new(i18n::t("Lagre som fil")))
                .on_hover_text(i18n::t("Lagre sjekklisten din som fil, for å dele den eller ta vare på den"))
                .on_disabled_hover_text(i18n::t("Lag en egen sjekkliste først"))
                .clicked()
            {
                self.pick("checklist_save", None);
            }
        });
        if !self.checklist.message.is_empty() {
            theme::hint(ui, &self.checklist.message);
        }
    }

    /// The person's own checklist, evaluated for this CV, with manual points to tick.
    pub(crate) fn own_checklist(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let Some(list) = self.checklist.own.clone() else {
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::hint(ui, i18n::t("Du har ingen egen sjekkliste ennå. Trykk «Rediger» for å lage en, eller «Last inn» for å hente en fra fil."));
            });
            return;
        };
        // The real page count; 0 until the preview is made, which leaves page points open.
        let states = checklist::evaluate(&list, &self.project, self.preview.page_count, &self.project.options.checklist_ticks);
        let done = states.iter().filter(|(_, s)| *s == State::Done).count();
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, &list.name);
            theme::hint(ui, &i18n::tf("{done} av {all} punkter er i orden.", &[("done", &done.to_string()), ("all", &states.len().to_string())]));
            ui.add_space(4.0);
            for (item, state) in states {
                if item.kind == Kind::Manual {
                    let mut on = state == State::Done;
                    let r = ui.add(egui::Checkbox::new(&mut on, RichText::new(&item.text).size(13.5))).on_hover_text(i18n::t("Kryss av når du har gjort dette. Avkrysningen lagres med denne CV-en."));
                    if r.changed() {
                        let ticks = &mut self.project.options.checklist_ticks;
                        ticks.retain(|id| id != &item.id);
                        if on {
                            ticks.push(item.id.clone());
                        }
                        self.save_at = Some(std::time::Instant::now());
                    }
                } else {
                    let color = match state {
                        State::Done => t.success,
                        State::NotDone => t.warning,
                        State::Open => t.faint,
                    };
                    crate::status_line(ui, color, &item.text);
                }
            }
        });
    }

    /// The editor window for one's own checklist.
    pub(crate) fn checklist_editor(&mut self, ctx: &egui::Context) {
        let Some(mut list) = self.checklist.editing.take() else {
            return;
        };
        let t = Tokens::get(ctx);
        let mut done: Option<bool> = None;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.checklist-editor")).show(ctx, |ui| {
            ui.set_width(620.0);
            ui.heading(i18n::t("Min sjekkliste"));
            theme::hint(ui, i18n::t("Punkter CVBuilder sjekker selv (ord i CV-en, antall sider, utfylte felt), og punkter du krysser av selv."));
            ui.add_space(6.0);
            theme::label(ui, i18n::t("Navn på sjekklisten"));
            ui.add(egui::TextEdit::singleline(&mut list.name).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6))).on_hover_text(i18n::t("Vises som overskrift i sjekkliste-fanen"));
            ui.add_space(6.0);
            let mut remove = None;
            let mut swap = None;
            let count = list.items.len();
            // As tall as the window allows, so the buttons below always fit on a small screen.
            let list_height = (ui.ctx().content_rect().height() - 330.0).clamp(160.0, 520.0);
            egui::ScrollArea::vertical().max_height(list_height).show(ui, |ui| {
                for (i, item) in list.items.iter_mut().enumerate() {
                    egui::Frame::new().fill(t.hover).corner_radius(CornerRadius::same(theme::R)).inner_margin(Margin::same(8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(("kind", &item.id))
                                .selected_text(kind_name(item.kind))
                                .width(170.0)
                                .show_ui(ui, |ui| {
                                    for k in Kind::ALL {
                                        if ui.selectable_label(item.kind == k, kind_name(k)).on_hover_text(i18n::t("Hva slags punkt dette er")).clicked() {
                                            set_kind(item, k);
                                        }
                                    }
                                })
                                .response
                                .on_hover_text(i18n::t("Hva slags punkt dette er: noe du krysser av selv, eller noe CVBuilder sjekker"));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                if theme::icon_button(ui, Icon::Trash, i18n::t("Slett punktet"), true).clicked() {
                                    remove = Some(i);
                                }
                                if theme::icon_button(ui, Icon::Down, i18n::t("Flytt ned"), i + 1 < count).clicked() {
                                    swap = Some((i, i + 1));
                                }
                                if theme::icon_button(ui, Icon::Up, i18n::t("Flytt opp"), i > 0).clicked() {
                                    swap = Some((i, i - 1));
                                }
                            });
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut item.text)
                                .id_salt(("text", &item.id))
                                .hint_text(i18n::t("Hva punktet sier, f.eks. «Har bedt om referanse»"))
                                .desired_width(f32::INFINITY)
                                .margin(Margin::symmetric(8, 5)),
                        )
                        .on_hover_text(i18n::t("Teksten som vises i sjekklisten"));
                        match item.kind {
                            Kind::Contains | Kind::NotContains => {
                                ui.add(
                                    egui::TextEdit::singleline(&mut item.value)
                                        .id_salt(("value", &item.id))
                                        .hint_text(i18n::t("Ord eller uttrykk; flere kan skilles med /"))
                                        .desired_width(f32::INFINITY)
                                        .margin(Margin::symmetric(8, 5)),
                                )
                                .on_hover_text(i18n::t("CVBuilder ser etter dette i hele CV-teksten, uten å bry seg om store og små bokstaver"));
                            }
                            Kind::MaxPages => {
                                // The box shows a valid number even for a hand-written «mange»;
                                // keep the value equal to what it shows.
                                let mut n = pages_value(item);
                                ui.add(egui::DragValue::new(&mut n).range(1..=10).suffix(i18n::t(" sider")))
                                    .on_hover_text(i18n::t("Det høyeste antallet sider CV-en kan ha for at punktet er i orden"));
                                if item.value != n.to_string() {
                                    item.value = n.to_string();
                                }
                            }
                            Kind::Filled => {
                                let current = checklist::FIELDS.iter().find(|(k, _)| *k == item.value.trim()).map(|(_, name)| i18n::tdyn(name)).unwrap_or_else(|| i18n::t("Velg felt").to_string());
                                egui::ComboBox::from_id_salt(("field", &item.id))
                                    .selected_text(current)
                                    .show_ui(ui, |ui| {
                                        for (k, name) in checklist::FIELDS {
                                            ui.selectable_value(&mut item.value, k.to_string(), i18n::tdyn(name)).on_hover_text(i18n::t("Feltet som må være fylt ut"));
                                        }
                                    })
                                    .response
                                    .on_hover_text(i18n::t("Hvilket felt i CV-en som må være fylt ut"));
                            }
                            Kind::Manual => {}
                        }
                    });
                    ui.add_space(4.0);
                }
            });
            if let Some(i) = remove {
                list.items.remove(i);
            }
            if let Some((a, b)) = swap {
                list.items.swap(a, b);
            }
            ui.horizontal(|ui| {
                let room = list.items.len() < checklist::MAX_ITEMS;
                if ui
                    .add_enabled(room, egui::Button::new(i18n::t("+ Legg til punkt")))
                    .on_hover_text(i18n::t("Et nytt punkt nederst i listen"))
                    .on_disabled_hover_text(i18n::t("Listen har så mange punkter som den kan ha"))
                    .clicked()
                {
                    let id = list.new_id();
                    list.items.push(Item { id, text: String::new(), kind: Kind::Manual, value: String::new() });
                }
                if ui.button(i18n::t("Start fra eksempel")).on_hover_text(i18n::t("Erstatt punktene med et eksempel du kan bygge videre på")).clicked() {
                    list.items = Checklist::starter().items;
                }
            });
            let problems = list.problems();
            for p in &problems {
                ui.label(RichText::new(i18n::tdyn(p)).size(12.5).color(t.danger));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::primary(
                    ui,
                    i18n::t("Lagre"),
                    if problems.is_empty() { i18n::t("Lagre sjekklisten og bruk den i sjekkliste-fanen") } else { i18n::t("Rett feilene over først") },
                    problems.is_empty(),
                )
                .clicked()
                {
                    done = Some(true);
                }
                if ui.button(i18n::t("Avbryt")).on_hover_text(i18n::t("Lukk uten å lagre endringene")).clicked() {
                    done = Some(false);
                }
            });
        });
        // A click beside the window or Esc closes it only when nothing would be lost.
        let before = self.checklist.own.clone();
        let unchanged = list == before.clone().unwrap_or_else(Checklist::starter);
        match done.or((r.should_close() && unchanged).then_some(false)) {
            Some(true) => match save_own(&list) {
                Ok(()) => {
                    if let Some(before) = &before {
                        let ticks = self.project.options.checklist_ticks.clone();
                        checklist::forget_removed(before, &list, &mut self.project.options.checklist_ticks);
                        if ticks != self.project.options.checklist_ticks {
                            self.save_at = Some(std::time::Instant::now());
                        }
                    }
                    self.checklist.own = Some(list);
                    self.settings.own_checklist = true;
                    storage::save_settings(&self.settings);
                    self.checklist.message = i18n::t("Sjekklisten er lagret.").into();
                }
                Err(e) => {
                    self.checklist.message = i18n::tf("Kunne ikke lagre sjekklisten: {error}", &[("error", &e)]);
                    self.checklist.editing = Some(list);
                }
            },
            Some(false) => {}
            None => self.checklist.editing = Some(list),
        }
    }

    /// A picked checklist file to use.
    pub(crate) fn load_checklist(&mut self, path: &std::path::Path) {
        match read_file(path) {
            Ok(list) => match save_own(&list) {
                Ok(()) => {
                    self.checklist.message = i18n::tf("«{name}» er lastet inn og brukes nå.", &[("name", &list.name)]);
                    self.checklist.own = Some(list);
                    self.settings.own_checklist = true;
                    storage::save_settings(&self.settings);
                }
                Err(e) => self.checklist.message = i18n::tf("Kunne ikke lagre sjekklisten: {error}", &[("error", &e)]),
            },
            // Several problems come one per line; each is translated on its own.
            Err(e) => self.checklist.message = translate(&e),
        }
    }

    pub(crate) fn save_checklist_file(&mut self, path: &std::path::Path) {
        let Some(list) = &self.checklist.own else {
            return;
        };
        let path = with_extension(path);
        let path = path.as_path();
        self.checklist.message = match std::fs::write(path, checklist::to_file(list)) {
            Ok(()) => i18n::tf("Sjekklisten er lagret: {path}", &[("path", &path.display().to_string())]),
            Err(e) => i18n::tf("Kunne ikke lagre sjekklisten: {error}", &[("error", &e.to_string())]),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_kind_starts_with_a_value_that_fits_it() {
        let mut item = Item { id: "a".into(), text: "T".into(), kind: Kind::Contains, value: "løsningsorientert".into() };
        set_kind(&mut item, Kind::MaxPages);
        assert_eq!((item.kind, item.value.as_str()), (Kind::MaxPages, "2"));
        set_kind(&mut item, Kind::MaxPages);
        assert_eq!(item.value, "2", "choosing the same kind keeps the value");
        item.value = "4".into();
        set_kind(&mut item, Kind::Contains);
        assert_eq!(item.value, "", "a page count is not a word to look for");
        let list = Checklist { items: vec![item], ..Default::default() };
        assert!(!list.problems().is_empty(), "an empty word must be filled in before saving");
    }

    #[test]
    fn page_points_show_and_keep_a_valid_number() {
        let item = |v: &str| Item { id: "a".into(), text: "T".into(), kind: Kind::MaxPages, value: v.into() };
        for (value, n) in [("3", 3), (" 1 ", 1), ("mange", 2), ("0", 2), ("11", 2), ("", 2)] {
            assert_eq!(pages_value(&item(value)), n, "{value:?}");
        }
    }

    #[test]
    fn saved_checklist_files_can_be_found_again() {
        let p = |s: &str| std::path::PathBuf::from(s);
        assert_eq!(with_extension(&p("liste.cvchecklist.json")), p("liste.cvchecklist.json"));
        assert_eq!(with_extension(&p("liste.JSON")), p("liste.JSON"));
        assert_eq!(with_extension(&p("liste")), p("liste.cvchecklist.json"));
    }

    #[test]
    fn checklist_files_round_trip_through_the_disk() {
        let dir = std::env::temp_dir().join(format!("cvbuilder-checklist-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("min.cvchecklist.json");
        let list = Checklist::starter();
        std::fs::write(&path, checklist::to_file(&list)).expect("write");
        assert_eq!(read_file(&path).expect("read"), list);
        std::fs::write(&path, [0xff, 0xfe, 0x00]).expect("write");
        assert!(read_file(&path).expect_err("binary").contains("UTF-8"));
        assert!(read_file(&dir.join("finnes-ikke.json")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn several_problems_are_translated_one_by_one() {
        i18n::set(i18n::Lang::En);
        let text = translate("Punkt 1 mangler tekst.\nPunkt 2 har ingen egen id.");
        assert_eq!(text, "Point 1 has no text.\nPoint 2 has no id of its own.");
    }
}
