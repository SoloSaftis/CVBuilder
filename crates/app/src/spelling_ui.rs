//! «Stavekontroll» in the checklist panel (to-do 046): possible spelling mistakes in the CV's
//! own text, checked offline against a dictionary for the CV's language. Each word has its
//! suggestions (one click puts it in everywhere, and «Angre» takes it back), «Ignorer» (until
//! CVBuilder closes) and «Legg til i mine ord» (kept in the data folder, for every CV).
//!
//! English, French and Spanish dictionaries come with the app. Others are downloaded once, when
//! the person clicks, from a pinned commit on GitHub (cv_ai::dictionaries); a Hunspell file can
//! also be picked by hand for a computer without internet, or for a language CVBuilder has none for.
use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use cv_core::spelling::{self, Dictionary, Finding, Place, Source};
use egui::Margin;

use crate::checklist_ui::{Status, status_line};
use crate::theme::{self, Tokens};
use crate::undo::step;
use crate::{CvApp, i18n, storage};

/// How many words get suggestions (each takes a moment).
const SUGGESTED: usize = 25;

type Checked = (u64, Vec<(Finding, Vec<String>)>);

#[derive(Default)]
pub struct SpellState {
    /// The CV language code the dictionaries are for; another code starts over.
    code: String,
    dicts: Option<Arc<Vec<Dictionary>>>,
    loading: Option<Receiver<Result<Vec<Dictionary>, String>>>,
    downloading: Option<Receiver<Result<(), String>>>,
    checking: Option<Receiver<Checked>>,
    /// What the results are for (the text, the language and the words left out).
    key: u64,
    results: Vec<(Finding, Vec<String>)>,
    /// Words hidden until CVBuilder closes, in lower case.
    ignored: HashSet<String>,
    /// The person's own words, in lower case (`my-words.txt`).
    pub own: BTreeSet<String>,
    /// Reading the dictionary failed; not tried again until another is downloaded or picked.
    failed: bool,
    /// The last thing that went wrong, for the person.
    pub note: String,
}

impl SpellState {
    pub fn load() -> SpellState {
        SpellState { own: load_own_words(), ..Default::default() }
    }

    /// The results at once, without a dictionary (the screen check uses it).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn show(&mut self, code: &str, results: Vec<(Finding, Vec<String>)>) {
        self.code = code.to_string();
        self.dicts = Some(Arc::new(Vec::new()));
        self.results = results;
        self.key = u64::MAX;
    }
}

/// Where downloaded and picked dictionaries are kept.
pub fn folder() -> PathBuf {
    storage::data_dir().join("dictionaries")
}

fn own_words_file() -> PathBuf {
    storage::data_dir().join("my-words.txt")
}

fn load_own_words() -> BTreeSet<String> {
    std::fs::read_to_string(own_words_file()).map(|t| t.lines().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

fn save_own_words(words: &BTreeSet<String>) -> Result<(), String> {
    let text: String = words.iter().map(|w| format!("{w}\n")).collect();
    std::fs::create_dir_all(storage::data_dir()).and_then(|_| std::fs::write(own_words_file(), text)).map_err(|e| e.to_string())
}

/// The file name a picked dictionary is kept under for a CV language («own-nb»).
fn own_id(code: &str) -> String {
    format!("own-{}", code.split('-').next().unwrap_or(code).to_lowercase())
}

/// Whether a source can be used without downloading.
fn present(s: &Source, dir: &Path) -> bool {
    s.bundled || dir.join(format!("{}.dic", s.id)).is_file()
}

/// What can check this CV language now: the bundled and downloaded dictionaries, and a picked one.
fn usable(code: &str, dir: &Path) -> (Vec<&'static Source>, bool) {
    let sources = spelling::sources_for(code).into_iter().filter(|s| present(s, dir)).collect();
    (sources, dir.join(format!("{}.dic", own_id(code))).is_file())
}

fn load_all(code: &str, dir: &Path) -> Result<Vec<Dictionary>, String> {
    let (sources, own) = usable(code, dir);
    let mut out = Vec::new();
    let read = |id: &str| -> Result<Dictionary, String> {
        let aff = std::fs::read(dir.join(format!("{id}.aff"))).map_err(|e| e.to_string())?;
        let dic = std::fs::read(dir.join(format!("{id}.dic"))).map_err(|e| e.to_string())?;
        spelling::load_bytes(&aff, &dic)
    };
    for s in sources {
        out.push(if s.bundled { spelling::load_bundled(s.id)? } else { read(s.id)? });
    }
    if own {
        out.push(read(&own_id(code))?);
    }
    Ok(out)
}

/// Reads a Hunspell `.dic` with its `.aff` next to it, tries it, and keeps it (in UTF-8) for
/// the CV language `code`.
pub fn import(dic_path: &Path, code: &str, dir: &Path) -> Result<(), String> {
    let aff_path = dic_path.with_extension("aff");
    let not_read = |e: std::io::Error| cv_core::plain::with_details(i18n::t("Fant ikke .aff-filen som hører til. Den må ligge ved siden av .dic-filen med samme navn."), &e.to_string());
    let aff = std::fs::read(&aff_path).map_err(not_read)?;
    let dic = std::fs::read(dic_path).map_err(|e| e.to_string())?;
    let bad = |e: String| cv_core::plain::with_details(i18n::t("Filen er ikke en ordbok CVBuilder kan lese. Velg en Hunspell-ordbok (.dic med .aff ved siden av) i UTF-8 eller ISO 8859-1."), &e);
    let aff_text = spelling::decode(&aff, None).map_err(bad)?;
    let encoding = spelling::encoding(&aff);
    let dic_text = spelling::decode(&dic, Some(&encoding)).map_err(bad)?;
    Dictionary::new(&aff_text, &dic_text).map_err(|e| bad(e.to_string()))?;
    cv_ai::dictionaries::save(&own_id(code), aff_text.as_bytes(), dic_text.as_bytes(), None, dir)
}

/// A place in the CV in the interface language.
fn place_name(p: &Place) -> String {
    match p {
        Place::Job(n) | Place::Education(n) | Place::Section(n) if !n.trim().is_empty() => n.clone(),
        Place::Headline => i18n::tdyn("Faglig overskrift"),
        Place::Profile => i18n::t("Profil").into(),
        Place::Job(_) => i18n::t("Arbeidserfaring").into(),
        Place::Education(_) => i18n::t("Utdanning").into(),
        Place::Certifications => i18n::t("Kurs og sertifiseringer").into(),
        Place::Interests => i18n::t("Interesser").into(),
        Place::Section(_) => i18n::t("Egen seksjon").into(),
    }
}

/// The language's name in the interface language («norsk (bokmål)», «German»).
fn language_name(code: &str) -> String {
    i18n::language_name(code).unwrap_or_else(|| code.to_string())
}

fn fingerprint(p: &cv_core::Project, code: &str, left_out: &HashSet<String>, own: &BTreeSet<String>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut h);
    for (_, text) in spelling::texts(p) {
        text.hash(&mut h);
    }
    spelling::ignored(p).len().hash(&mut h);
    let mut left: Vec<&String> = left_out.iter().collect();
    left.sort();
    left.hash(&mut h);
    own.hash(&mut h);
    h.finish()
}

impl CvApp {
    /// Answers from the threads: a dictionary read, downloaded, or a check done.
    fn spelling_poll(&mut self) {
        let s = &mut self.spelling;
        if let Some(Ok(result)) = s.loading.as_ref().map(|rx| rx.try_recv()) {
            s.loading = None;
            match result {
                Ok(d) => s.dicts = Some(Arc::new(d)),
                Err(e) => {
                    s.failed = true;
                    s.note = i18n::details(i18n::t("Ordboken kunne ikke leses. Last den ned på nytt, eller velg en annen fil."), &e);
                }
            }
        }
        if let Some(Ok(result)) = s.downloading.as_ref().map(|rx| rx.try_recv()) {
            s.downloading = None;
            match result {
                Ok(()) => {
                    s.dicts = None;
                    s.failed = false;
                    s.note.clear();
                }
                Err(e) => s.note = e,
            }
        }
        if let Some(Ok((key, results))) = s.checking.as_ref().map(|rx| rx.try_recv()) {
            s.checking = None;
            s.key = key;
            s.results = results;
        }
    }

    fn start_download(&mut self, sources: Vec<&'static Source>) {
        let (tx, rx) = channel();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let dir = folder();
            let result = sources.iter().try_for_each(|s| cv_ai::dictionaries::download(s, &dir));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.spelling.note.clear();
        self.spelling.downloading = Some(rx);
    }

    /// A Hunspell file the person picked, for the CV's language.
    pub(crate) fn import_dictionary(&mut self, path: &Path) {
        match import(path, self.project.language_code(), &folder()) {
            Ok(()) => {
                self.spelling.dicts = None;
                self.spelling.failed = false;
                self.spelling.note.clear();
                self.status = i18n::t("Ordboken er lagt til. Stavekontrollen bruker den nå.").into();
            }
            Err(e) => self.spelling.note = e,
        }
    }

    /// Puts a suggestion in place of a word everywhere; «Angre» takes it back.
    fn use_suggestion(&mut self, from: &str, to: &str) {
        let mut changed = self.project.clone();
        let n = spelling::replace_word(&mut changed, from, to);
        if n == 0 {
            return;
        }
        self.remember(step::SPELLING);
        self.project = changed;
        self.buffers.clear();
        self.touched();
        self.status = i18n::tf("«{from}» er byttet med «{to}» ({n} steder). Angre tar det tilbake.", &[("from", from), ("to", to), ("n", &n.to_string())]);
    }

    fn add_own_word(&mut self, word: &str) {
        self.spelling.own.insert(word.to_lowercase());
        if let Err(e) = save_own_words(&self.spelling.own) {
            self.spelling.note = i18n::failed(i18n::t("Ordet ble ikke lagret."), &e, cv_core::plain::TRY_AGAIN);
        }
    }

    /// The «Stavekontroll» card in the checklist panel.
    pub(crate) fn spelling_card(&mut self, ui: &mut egui::Ui) {
        self.spelling_poll();
        let code = self.project.language_code().to_string();
        if self.spelling.code != code {
            let own = std::mem::take(&mut self.spelling.own);
            let ignored = std::mem::take(&mut self.spelling.ignored);
            self.spelling = SpellState { code: code.clone(), own, ignored, ..Default::default() };
        }
        let dir = folder();
        let (sources, own_dict) = usable(&code, &dir);
        let has_any = !sources.is_empty() || own_dict || self.spelling.dicts.is_some();
        // Read the dictionary in the background the first time it is needed.
        if has_any && !self.spelling.failed && self.spelling.dicts.is_none() && self.spelling.loading.is_none() && self.spelling.downloading.is_none() {
            let (tx, rx) = channel();
            let (ctx, c, d) = (self.egui_ctx.clone(), code.clone(), dir.clone());
            std::thread::spawn(move || {
                let _ = tx.send(load_all(&c, &d));
                ctx.request_repaint();
            });
            self.spelling.loading = Some(rx);
        }
        // Check again when the text changed.
        if let Some(dicts) = self.spelling.dicts.clone() {
            let key = fingerprint(&self.project, &code, &self.spelling.ignored, &self.spelling.own);
            if key != self.spelling.key && self.spelling.key != u64::MAX && self.spelling.checking.is_none() {
                let (tx, rx) = channel();
                let project = self.project.clone();
                let left_out: HashSet<String> = self.spelling.ignored.iter().chain(self.spelling.own.iter()).cloned().collect();
                let ctx = self.egui_ctx.clone();
                std::thread::spawn(move || {
                    let found = spelling::find(&project, |w| spelling::known(&dicts, w), &left_out);
                    let results = found.into_iter().enumerate().map(|(i, f)| {
                        let s = if i < SUGGESTED { spelling::suggest(&dicts, &f.word, 4) } else { Vec::new() };
                        (f, s)
                    });
                    let _ = tx.send((key, results.collect()));
                    ctx.request_repaint();
                });
                self.spelling.checking = Some(rx);
            }
        }
        let t = Tokens::get(ui.ctx());
        let mut action: Option<Action> = None;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t("Stavekontroll"));
            let state = &self.spelling;
            let language = language_name(&code);
            if !has_any {
                let missing = spelling::sources_for(&code);
                if missing.is_empty() {
                    theme::hint(ui, &i18n::tf("CVBuilder har ingen ordbok for {language} ennå. Har du en Hunspell-ordbok (.dic og .aff), kan du velge den.", &[("language", &language)]));
                } else {
                    let size = spelling::size_text(missing.iter().map(|s| s.bytes).sum());
                    theme::hint(
                        ui,
                        &i18n::tf(
                            "Ordboken for {language} er ikke på maskinen. Den lastes ned fra GitHub én gang ({size}), og etter det sjekkes CV-en uten nett. Ingenting fra CV-en sendes.",
                            &[("language", &language), ("size", &size)],
                        ),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if !missing.is_empty() {
                        if state.downloading.is_some() {
                            ui.spinner();
                            ui.label(i18n::t("Laster ned ordboken …"));
                        } else if ui.button(i18n::t("Last ned ordboken")).on_hover_text(i18n::t("Henter ordboken fra GitHub. Bare ordboken lastes ned; CV-en blir på maskinen.")).clicked() {
                            action = Some(Action::Download(missing.clone()));
                        }
                    }
                    if ui
                        .button(i18n::t("Velg ordbokfil …"))
                        .on_hover_text(i18n::t("En Hunspell-ordbok (.dic med .aff ved siden av), for maskiner uten nett eller et språk CVBuilder ikke har"))
                        .clicked()
                    {
                        action = Some(Action::Pick);
                    }
                });
            } else if state.failed {
                // The note below says what went wrong.
            } else if state.dicts.is_none() || state.key == 0 && state.checking.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(i18n::t("Sjekker stavingen …"));
                });
            } else if state.results.is_empty() {
                status_line(ui, Status::Done, &i18n::tf("Fant ingen mulige skrivefeil ({language}).", &[("language", &language)]));
            } else {
                theme::hint(
                    ui,
                    &i18n::tf(
                        "{n} ord som ordboken for {language} ikke kjenner. Ordboken kjenner ikke alle navn, fagord og sammensatte ord, så se selv om de er feil.",
                        &[("n", &state.results.len().to_string()), ("language", &language)],
                    ),
                );
                for (f, suggestions) in &state.results {
                    let places: Vec<String> = f.places.iter().take(3).map(place_name).collect();
                    let text = if f.count > 1 {
                        i18n::tf("«{word}» ({n} ganger) i {places}", &[("word", &f.word), ("n", &f.count.to_string()), ("places", &places.join("; "))])
                    } else {
                        i18n::tf("«{word}» i {places}", &[("word", &f.word), ("places", &places.join("; "))])
                    };
                    status_line(ui, Status::ToDo, &text);
                    egui::Frame::new().inner_margin(Margin { left: 20, right: 0, top: 0, bottom: 2 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let link = |ui: &mut egui::Ui, text: &str, tip: &str| {
                                ui.add(egui::Button::new(egui::RichText::new(text).size(12.0).color(t.accent)).frame(false).small()).on_hover_text(tip).clicked()
                            };
                            if suggestions.is_empty() {
                                ui.label(egui::RichText::new(i18n::t("Ingen forslag")).size(12.0).color(t.dim));
                            } else {
                                ui.label(egui::RichText::new(i18n::t("Bytt til:")).size(12.0).color(t.dim));
                                for s in suggestions {
                                    if link(ui, s, &i18n::tf("Bytter «{from}» med «{to}» overalt i CV-en", &[("from", &f.word), ("to", s)])) {
                                        action = Some(Action::Use(f.word.clone(), s.clone()));
                                    }
                                }
                            }
                            if link(ui, i18n::t("Ignorer"), i18n::t("Skjuler ordet til du lukker CVBuilder")) {
                                action = Some(Action::Ignore(f.word.clone()));
                            }
                            if link(ui, i18n::t("Legg til i mine ord"), i18n::t("Ordet regnes som riktig i alle CV-ene dine på denne maskinen")) {
                                action = Some(Action::Own(f.word.clone()));
                            }
                        });
                    });
                }
            }
            if !state.note.is_empty() {
                status_line(ui, Status::Problem, &i18n::tdyn_lines(&state.note));
            }
            if has_any {
                let mut from: Vec<String> = sources.iter().map(|s| format!("{} ({})", s.id, s.licence)).collect();
                if own_dict {
                    from.push(i18n::t("din egen ordbokfil").into());
                }
                // (No names only when the screen check put results in without a dictionary.)
                if !from.is_empty() {
                    theme::hint(ui, &i18n::tf("Ordbok: {list}. Navn, e-post, firmaer, ferdigheter og ordene i annonsen hoppes over.", &[("list", &from.join(", "))]));
                }
                if !state.own.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        theme::hint(ui, &i18n::tf("Mine ord: {n}", &[("n", &state.own.len().to_string())]));
                        if ui
                            .add(egui::Button::new(egui::RichText::new(i18n::t("Glem mine ord")).size(12.0).color(t.accent)).frame(false).small())
                            .on_hover_text(i18n::t("Tømmer listen over ord du har lagt til"))
                            .clicked()
                        {
                            action = Some(Action::ForgetOwn);
                        }
                    });
                }
            }
        });
        match action {
            Some(Action::Download(list)) => self.start_download(list),
            Some(Action::Pick) => self.pick("dictionary", None),
            Some(Action::Use(from, to)) => self.use_suggestion(&from, &to),
            Some(Action::Ignore(w)) => {
                self.spelling.ignored.insert(w.to_lowercase());
                self.spelling.results.retain(|(f, _)| f.word.to_lowercase() != w.to_lowercase());
            }
            Some(Action::Own(w)) => {
                self.add_own_word(&w);
                self.spelling.results.retain(|(f, _)| f.word.to_lowercase() != w.to_lowercase());
            }
            Some(Action::ForgetOwn) => {
                self.spelling.own.clear();
                if let Err(e) = save_own_words(&self.spelling.own) {
                    self.spelling.note = i18n::failed(i18n::t("Ordet ble ikke lagret."), &e, cv_core::plain::TRY_AGAIN);
                }
            }
            None => {}
        }
    }
}

enum Action {
    Download(Vec<&'static Source>),
    Pick,
    Use(String, String),
    Ignore(String),
    Own(String),
    ForgetOwn,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_dictionary_in_latin_1_is_kept_in_utf_8_for_its_language() {
        let dir = std::env::temp_dir().join(format!("cvbuilder-spell-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("x.aff"), b"SET ISO8859-1\n").expect("aff");
        std::fs::write(dir.join("x.dic"), b"1\nbl\xe5b\xe6r\n").expect("dic");
        let kept = dir.join("kept");
        import(&dir.join("x.dic"), "fi", &kept).expect("import");
        assert_eq!(usable("fi", &kept), (Vec::new(), true));
        let dicts = load_all("fi", &kept).expect("load");
        assert!(spelling::known(&dicts, "blåbær"));
        // Without its .aff, the plain message says what is missing.
        std::fs::remove_file(dir.join("x.aff")).expect("rm");
        assert!(import(&dir.join("x.dic"), "fi", &kept).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_languages_need_no_download() {
        let nowhere = std::env::temp_dir().join("cvbuilder-no-such-folder");
        for code in ["en", "en-GB", "fr", "fr-CA", "es", "es-MX"] {
            assert!(!usable(code, &nowhere).0.is_empty(), "{code}");
        }
        assert!(usable("nb", &nowhere).0.is_empty() && !spelling::sources_for("nb").is_empty());
        assert!(usable("ja", &nowhere).0.is_empty() && spelling::sources_for("ja").is_empty());
    }
}
