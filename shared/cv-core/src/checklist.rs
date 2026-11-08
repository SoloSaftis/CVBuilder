//! A checklist of the person's own, instead of (or as well as) the built-in recruiter check:
//! points CVBuilder checks by itself (the CV mentions a word, stays within a page count, has a
//! field filled in) and points the person ticks off themselves. Kept as `.cvchecklist.json`.
use serde::{Deserialize, Serialize};

use crate::checks;
use crate::model::Project;
use crate::text;

pub const FORMAT: &str = "cvbuilder-checklist/1";

/// The most points a list may have.
pub const MAX_ITEMS: usize = 100;

/// A checklist file is a few kilobytes; anything near this is not one.
pub const MAX_FILE_BYTES: usize = 1_000_000;

/// The fields a `filled` point can ask for: (key, Norwegian name).
pub const FIELDS: [(&str, &str); 10] = [
    ("name", "Navn"),
    ("headline", "Faglig overskrift"),
    ("email", "E-post"),
    ("phone", "Telefon"),
    ("city", "By og land"),
    ("linkedin", "LinkedIn"),
    ("summary", "Profil"),
    ("photo", "Portrett"),
    ("target", "Stillingen du søker"),
    ("keywords", "Fagord fra annonsen"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The person ticks it off.
    #[default]
    Manual,
    /// The CV text mentions `value` (any of several separated by «/»).
    Contains,
    /// The CV text does not mention `value`.
    NotContains,
    /// At most `value` pages.
    MaxPages,
    /// The field named in `value` is filled in (see [`FIELDS`]).
    Filled,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Manual, Kind::Contains, Kind::NotContains, Kind::MaxPages, Kind::Filled];

    /// The value a point of this kind starts with, so a new or changed point is usable at once
    /// where it can be (a page count), and empty where the person must choose.
    pub fn starting_value(self) -> String {
        match self {
            Kind::MaxPages => "2".into(),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Stable id, so a tick survives edits to the text. Files written by hand may leave it
    /// out; [`parse`] gives such points one.
    #[serde(default)]
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checklist {
    /// Missing in a broken file; [`Checklist::problems`] then says what it must be.
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub items: Vec<Item>,
}

impl Default for Checklist {
    fn default() -> Self {
        Checklist { format: FORMAT.into(), name: "Min sjekkliste".into(), items: Vec::new() }
    }
}

/// How one point stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Done,
    NotDone,
    /// A manual point not ticked yet, or an automatic one that cannot be judged yet (the page
    /// count before the preview has been made).
    Open,
}

impl Checklist {
    /// A starting point people can change: a few useful points of each kind.
    pub fn starter() -> Checklist {
        let item = |id: &str, text: &str, kind: Kind, value: &str| Item { id: id.into(), text: text.into(), kind, value: value.into() };
        Checklist {
            format: FORMAT.into(),
            name: "Min sjekkliste".into(),
            items: vec![
                item("c1", "LinkedIn-profilen er med", Kind::Filled, "linkedin"),
                item("c2", "CV-en er på høyst to sider", Kind::MaxPages, "2"),
                item("c3", "Ingen tomme moteord som «løsningsorientert»", Kind::NotContains, "løsningsorientert"),
                item("c4", "Jeg har lest gjennom CV-en høyt", Kind::Manual, ""),
                item("c5", "Referansene vet at de kan bli kontaktet", Kind::Manual, ""),
            ],
        }
    }

    /// Everything wrong with a checklist file, in plain Norwegian. Empty when it can be used.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.format != FORMAT {
            out.push(format!("«format» må være «{FORMAT}»."));
        }
        if self.name.trim().is_empty() {
            out.push("Gi sjekklisten et navn.".into());
        }
        if self.items.len() > MAX_ITEMS {
            out.push(format!("En sjekkliste kan ha høyst {MAX_ITEMS} punkter."));
        }
        let mut seen = std::collections::HashSet::new();
        for (i, it) in self.items.iter().enumerate() {
            let n = i + 1;
            if it.text.trim().is_empty() {
                out.push(format!("Punkt {n} mangler tekst."));
            }
            // Two points with one id would be ticked together.
            if it.id.trim().is_empty() || !seen.insert(it.id.as_str()) {
                out.push(format!("Punkt {n} har ingen egen id."));
            }
            match it.kind {
                // «/» alone has no word in it, and would never (or always) be found.
                Kind::Contains | Kind::NotContains if !it.value.split(['/', '|']).any(|w| !checks::normal(w).is_empty()) => out.push(format!("Punkt {n}: skriv ordet CV-en skal (eller ikke skal) nevne.")),
                Kind::MaxPages if !it.value.trim().parse::<usize>().is_ok_and(|p| (1..=10).contains(&p)) => out.push(format!("Punkt {n}: antall sider må være et tall fra 1 til 10.")),
                Kind::Filled if !FIELDS.iter().any(|(k, _)| *k == it.value.trim()) => {
                    out.push(format!("Punkt {n}: feltet må være ett av {}.", FIELDS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")))
                }
                _ => {}
            }
        }
        out
    }

    /// An id not in use, for a new point. It is new in time too, not just in this list: a CV
    /// keeps the ticks of deleted points, and a reused id would tick the new point there.
    pub fn new_id(&self) -> String {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        // At most `items.len()` of these are taken, so one of them is free.
        (0..=self.items.len() as u128)
            .map(|n| format!("c{}", base36(stamp + n)))
            .find(|id| !self.items.iter().any(|i| &i.id == id))
            .unwrap_or_else(|| format!("c{}", self.items.len() + 1))
    }

    /// Gives points without an id, or with an id another point has, an id of their own.
    /// Hand-written and merged files can have both.
    pub fn repair_ids(&mut self) {
        let mut seen = std::collections::HashSet::new();
        for i in 0..self.items.len() {
            let id = self.items[i].id.trim().to_string();
            if id.is_empty() || seen.contains(&id) {
                let fresh = self.new_id();
                seen.insert(fresh.clone());
                self.items[i].id = fresh;
            } else {
                self.items[i].id = id.clone();
                seen.insert(id);
            }
        }
    }
}

fn base36(mut n: u128) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Reads a `.cvchecklist.json` file; the error lists everything to fix.
pub fn parse(text: &str) -> Result<Checklist, String> {
    if text.len() > MAX_FILE_BYTES {
        return Err("Filen er for stor til å være en CVBuilder-sjekkliste.".into());
    }
    // Notepad on Windows saves UTF-8 with a byte-order mark, which JSON readers refuse.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut list: Checklist = serde_json::from_str(text).map_err(|e| format!("Filen er ikke en CVBuilder-sjekkliste: {e}"))?;
    list.repair_ids();
    if list.name.trim().is_empty() {
        list.name = Checklist::default().name;
    }
    let problems = list.problems();
    if problems.is_empty() { Ok(list) } else { Err(problems.join("\n")) }
}

/// Reads a checklist file as it lies on disk, which may not be text at all.
pub fn parse_bytes(bytes: &[u8]) -> Result<Checklist, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("Filen er for stor til å være en CVBuilder-sjekkliste.".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "Filen er ikke en CVBuilder-sjekkliste: den er ikke tekst i UTF-8.".to_string())?;
    parse(text)
}

pub fn to_file(list: &Checklist) -> String {
    serde_json::to_string_pretty(list).unwrap_or_default()
}

/// After the list has been edited: forgets the ticks of points that were deleted, so the CV
/// does not keep them, and drops repeated ticks.
pub fn forget_removed(before: &Checklist, after: &Checklist, ticks: &mut Vec<String>) {
    let gone = |id: &String| before.items.iter().any(|i| &i.id == id) && !after.items.iter().any(|i| &i.id == id);
    let mut seen = std::collections::HashSet::new();
    ticks.retain(|id| !gone(id) && seen.insert(id.clone()));
}

fn filled(p: &Project, field: &str) -> bool {
    let c = &p.candidate;
    let has = |s: &str| !s.trim().is_empty();
    match field {
        "name" => has(&c.name),
        "headline" => has(&c.headline),
        "email" => has(&c.email),
        "phone" => has(&c.phone),
        "city" => has(&c.city),
        "linkedin" => has(&c.linkedin),
        "summary" => has(&c.summary),
        // The portrait counts only when the CV shows it.
        "photo" => has(&c.photo) && p.options.photo,
        "target" => has(&p.target.role),
        "keywords" => p.target.keywords.iter().any(|k| has(k)),
        _ => false,
    }
}

/// Everything the CV says, normalised for [`checks::contains`]: every section, the contact
/// line and the interests shown. Section headings and dates are left out, so «erfaring» or
/// «nå» are not found just because every CV has them.
pub fn cv_words(p: &Project) -> String {
    let mut parts: Vec<String> = vec![p.candidate.name.clone(), checks::cv_text(p)];
    parts.extend(text::contact_order(p).iter().map(|k| text::contact_value(p, k, true)));
    parts.extend(p.experience.iter().map(|j| j.location.clone()));
    parts.extend(p.languages.iter().map(|l| l.level.clone()));
    parts.extend(text::selected_interests(p));
    for s in &p.custom_sections {
        parts.push(s.title.clone());
        parts.extend(s.items.iter().map(|i| i.date.clone()));
    }
    checks::normal(&parts.join("\n"))
}

/// How each point stands for this CV. `pages` is the real page count, 0 while it is not known
/// yet; `ticked` holds the ids of manual points the person ticked.
pub fn evaluate(list: &Checklist, project: &Project, pages: usize, ticked: &[String]) -> Vec<(Item, State)> {
    let text = cv_words(project);
    let yes = |b: bool| if b { State::Done } else { State::NotDone };
    list.items
        .iter()
        .map(|it| {
            let state = match it.kind {
                Kind::Manual => {
                    if ticked.contains(&it.id) {
                        State::Done
                    } else {
                        State::Open
                    }
                }
                Kind::Contains => yes(checks::contains(&text, &it.value)),
                Kind::NotContains => yes(!checks::contains(&text, &it.value)),
                Kind::MaxPages if pages == 0 => State::Open,
                Kind::MaxPages => yes(it.value.trim().parse::<usize>().is_ok_and(|max| pages <= max)),
                Kind::Filled => yes(filled(project, it.value.trim())),
            };
            (it.clone(), state)
        })
        .collect()
}
