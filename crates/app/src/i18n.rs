//! The language of the app itself: Norwegian bokmål (the text in the source), English, or a
//! language pack the person added (made by AI or translated by hand). This is only the
//! interface; the CV's own language (its headings and dates) is a separate setting.
//!
//! The Norwegian text is the key: [`t`] returns it as it is, its English entry in [`EN`], or the
//! loaded pack's text (falling back to English, then to the key). [`tf`] does the same and fills
//! in `{name}` placeholders, so a translation may put the words in another order. [`tdyn`]
//! translates text made by cv-core (and cv-ai / cv-mcp) at run time: the checklist, colour and
//! ATS warnings, design descriptions, import summaries and errors. With a pack, its exact
//! entries are used; messages with numbers and names in them (the [`RULES`]) stay English.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{LazyLock, RwLock};

use regex::{Captures, Regex};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    No,
    En,
    /// The language pack last given to [`install`].
    Pack,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    let value = match lang {
        Lang::No => 0,
        Lang::En => 1,
        Lang::Pack => 2,
    };
    CURRENT.store(value, Ordering::Relaxed);
}

pub fn get() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::Pack,
        _ => Lang::No,
    }
}

/// The computer's language: Norwegian (bokmål, nynorsk or plain «no») gives Norwegian, anything
/// else English.
pub fn system() -> Lang {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    let code = locale.split(['-', '_', '.', '@']).next().unwrap_or("");
    if matches!(code, "nb" | "nn" | "no") { Lang::No } else { Lang::En }
}

/// The saved setting without packs: "no", "en", or the computer's language for anything else.
/// [`apply`] also knows the installed packs.
pub fn from_setting(value: &str) -> Lang {
    match value {
        "no" => Lang::No,
        "en" => Lang::En,
        _ => system(),
    }
}

static TABLE: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| EN.iter().copied().collect());
static DYN_TABLE: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| DYN.iter().copied().collect());

// ---------- language packs ----------

/// The `format` of a language pack file.
pub const PACK_FORMAT: &str = "cvbuilder-language/1";

/// A language pack: Norwegian key → text in the pack's language.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pack {
    pub code: String,
    pub name: String,
    /// «AI (model)» or a person.
    pub made_by: String,
    pub strings: BTreeMap<String, String>,
}

/// A pack read from a file, with what had to be left out.
#[derive(Debug)]
pub struct Checked {
    pub pack: Pack,
    /// Translations that were empty, not text, or did not keep the key's `{placeholders}`.
    pub dropped: usize,
    /// Keys this version of CVBuilder does not have.
    pub unknown: usize,
}

/// Why a pack file could not be used at all.
#[derive(Debug, PartialEq)]
pub enum PackError {
    NotJson(String),
    Format,
    NoName,
    BadCode(String),
    Reserved(String),
}

#[derive(Deserialize)]
struct PackFile {
    #[serde(default)]
    format: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    made_by: String,
    #[serde(default)]
    strings: HashMap<String, serde_json::Value>,
}

/// Every text a pack can translate: the interface ([`EN`]) and cv-core's fixed text ([`DYN`]),
/// as (Norwegian key, English).
pub fn all_keys() -> impl Iterator<Item = (&'static str, &'static str)> {
    EN.iter().chain(DYN.iter()).copied()
}

static KEYS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| all_keys().collect());

/// The `{name}` placeholders in a text.
pub fn placeholders(s: &str) -> BTreeSet<&str> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([a-z_]+)\}").expect("i18n pattern"));
    RE.captures_iter(s).filter_map(|c| c.get(1).map(|m| m.as_str())).collect()
}

/// A translation can be used for a key: it is not empty and has exactly the key's placeholders.
pub fn valid_translation(key: &str, text: &str) -> bool {
    !text.trim().is_empty() && placeholders(key) == placeholders(text)
}

/// A language code as stored: «de», «pt-BR», «vi». Lower-case letters (2–8), optionally a
/// two-letter region. «no», «en» and «auto» are taken by the built-in choices.
pub fn normalize_code(code: &str) -> Result<String, PackError> {
    let code = code.trim().replace('_', "-");
    let (base, region) = match code.split_once('-') {
        Some((b, r)) => (b.to_lowercase(), Some(r.to_uppercase())),
        None => (code.to_lowercase(), None),
    };
    let base_ok = (2..=8).contains(&base.len()) && base.chars().all(|c| c.is_ascii_lowercase());
    let region_ok = region.as_ref().is_none_or(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_uppercase()));
    if !base_ok || !region_ok {
        return Err(PackError::BadCode(code));
    }
    let code = match region {
        Some(r) => format!("{base}-{r}"),
        None => base,
    };
    if matches!(code.as_str(), "no" | "en" | "auto") {
        return Err(PackError::Reserved(code));
    }
    Ok(code)
}

/// Reads and checks a pack file. Bad entries are left out (they show in English) and counted.
pub fn parse_pack(text: &str) -> Result<Checked, PackError> {
    let file: PackFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| PackError::NotJson(e.to_string()))?;
    if file.format != PACK_FORMAT {
        return Err(PackError::Format);
    }
    let name = file.name.trim().to_string();
    if name.is_empty() {
        return Err(PackError::NoName);
    }
    let code = normalize_code(&file.code)?;
    let (mut dropped, mut unknown) = (0, 0);
    let mut strings = BTreeMap::new();
    for (key, value) in file.strings {
        if !KEYS.contains_key(key.as_str()) {
            unknown += 1;
            continue;
        }
        match value.as_str() {
            Some(v) if valid_translation(&key, v) => {
                strings.insert(key, v.to_string());
            }
            _ => dropped += 1,
        }
    }
    Ok(Checked { pack: Pack { code, name, made_by: file.made_by.trim().to_string(), strings }, dropped, unknown })
}

fn pack_json(code: &str, name: &str, made_by: &str, about: Option<&str>, strings: &mut dyn Iterator<Item = (&str, &str)>) -> String {
    let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"format\": {},\n", q(PACK_FORMAT)));
    if let Some(about) = about {
        out.push_str(&format!("  \"about\": {},\n", q(about)));
    }
    out.push_str(&format!("  \"code\": {},\n  \"name\": {},\n  \"made_by\": {},\n  \"strings\": {{", q(code), q(name), q(made_by)));
    let mut first = true;
    for (k, v) in strings {
        out.push_str(if first { "\n" } else { ",\n" });
        first = false;
        out.push_str(&format!("    {}: {}", q(k), q(v)));
    }
    out.push_str(if first { "}\n}\n" } else { "\n  }\n}\n" });
    out
}

/// A pack as a file, its strings in the order of the app's own table.
pub fn pack_to_json(pack: &Pack) -> String {
    let mut strings = all_keys().filter_map(|(k, _)| pack.strings.get(k).map(|v| (k, v.as_str())));
    pack_json(&pack.code, &pack.name, &pack.made_by, None, &mut strings)
}

/// A file for translating by hand: every key with its English as the value to replace.
pub fn template_json() -> String {
    let about = "Translate each value in \"strings\" (the English text) into your language. Keep {placeholders}, «», keyboard names and product names as they are. Fill in \"code\" (for example \"de\" or \"pt-BR\") and \"name\" (the language's own name, for example \"Deutsch\"), then import the file in CVBuilder under Settings.";
    pack_json("", "", "", Some(about), &mut all_keys())
}

/// What the language setting resolves to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    No,
    En,
    /// An index into the installed packs.
    Pack(usize),
}

/// The saved setting ("auto", "no", "en" or a pack's code) with the installed packs and the
/// computer's locale (like «de-DE» or «nb_NO.UTF-8»). A code no longer installed means «auto».
pub fn choose(setting: &str, locale: &str, packs: &[Pack]) -> Choice {
    match setting {
        "no" => Choice::No,
        "en" => Choice::En,
        code => match packs.iter().position(|p| p.code == code) {
            Some(i) => Choice::Pack(i),
            None => choose_auto(locale, packs),
        },
    }
}

/// «Follow the computer»: a pack for its language when one is installed, else Norwegian for
/// Norwegian, else English.
fn choose_auto(locale: &str, packs: &[Pack]) -> Choice {
    let locale = locale.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_lowercase();
    let base = locale.split('-').next().unwrap_or("").to_string();
    let lower = |p: &Pack| p.code.to_lowercase();
    let pick = packs
        .iter()
        .position(|p| !locale.is_empty() && lower(p) == locale)
        .or_else(|| packs.iter().position(|p| !base.is_empty() && lower(p) == base))
        .or_else(|| packs.iter().position(|p| !base.is_empty() && lower(p).split('-').next() == Some(base.as_str())));
    match pick {
        Some(i) => Choice::Pack(i),
        None if matches!(base.as_str(), "nb" | "nn" | "no") => Choice::No,
        None => Choice::En,
    }
}

/// The computer's locale, like «de-DE».
pub fn locale() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

/// Sets the interface language from the saved setting.
pub fn apply(setting: &str, packs: &[Pack]) {
    match choose(setting, &locale(), packs) {
        Choice::No => set(Lang::No),
        Choice::En => set(Lang::En),
        Choice::Pack(i) => install(&packs[i]),
    }
}

/// A pack's strings in memory for [`t`], which hands out `&'static str`.
struct Installed {
    code: String,
    name: String,
    fingerprint: u64,
    table: HashMap<&'static str, &'static str>,
}

static PACK: RwLock<Option<&'static Installed>> = RwLock::new(None);

fn fingerprint(pack: &Pack) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&pack.code, &pack.name, &pack.strings).hash(&mut h);
    h.finish()
}

/// The pack's strings as `&'static str`, keyed by the app's own static keys. The text is leaked
/// on purpose: this happens only when a pack is (re)loaded, and is bounded by the pack's size.
fn leak_table(pack: &Pack) -> HashMap<&'static str, &'static str> {
    pack.strings
        .iter()
        .filter_map(|(k, v)| {
            let (key, _) = KEYS.get_key_value(k.as_str())?;
            let text: &'static str = Box::leak(v.clone().into_boxed_str());
            Some((*key, text))
        })
        .collect()
}

/// Makes `pack` the interface language. Loading the same pack again leaks nothing new.
pub fn install(pack: &Pack) {
    let print = fingerprint(pack);
    let same = installed().is_some_and(|i| i.code == pack.code && i.fingerprint == print);
    if !same {
        let table = leak_table(pack);
        let entry: &'static Installed = Box::leak(Box::new(Installed { code: pack.code.clone(), name: pack.name.clone(), fingerprint: print, table }));
        if let Ok(mut slot) = PACK.write() {
            *slot = Some(entry);
        }
    }
    set(Lang::Pack);
}

fn installed() -> Option<&'static Installed> {
    PACK.read().ok().and_then(|slot| *slot)
}

/// The pack in use, as (code, name), when the interface is in a pack's language.
pub fn active_pack() -> Option<(String, String)> {
    if get() != Lang::Pack {
        return None;
    }
    installed().map(|i| (i.code.clone(), i.name.clone()))
}

fn pack_table() -> Option<&'static HashMap<&'static str, &'static str>> {
    installed().map(|i| &i.table)
}

/// A key's text with a pack: the pack's, else English, else the key.
fn ui_text(pack: Option<&HashMap<&'static str, &'static str>>, no: &'static str) -> &'static str {
    pack.and_then(|p| p.get(no)).or_else(|| TABLE.get(no)).copied().unwrap_or(no)
}

/// cv-core text with a pack: the pack's exact entry, else English (patterns too), else as it is.
fn dyn_text(pack: Option<&HashMap<&'static str, &'static str>>, text: &str) -> String {
    match pack.and_then(|p| p.get(text)) {
        Some(t) => t.to_string(),
        None => english(text).unwrap_or_else(|| text.to_string()),
    }
}

/// The interface text for a Norwegian string: itself, its English entry, or the pack's text
/// (English when the pack has none; the key when there is no English either).
pub fn t(no: &'static str) -> &'static str {
    match get() {
        Lang::No => no,
        Lang::En => TABLE.get(no).copied().unwrap_or(no),
        Lang::Pack => ui_text(pack_table(), no),
    }
}

/// [`t`], then `{name}` placeholders filled from `args`.
pub fn tf(no: &'static str, args: &[(&str, &str)]) -> String {
    fill(t(no), args)
}

fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| args.iter().find(|(k, _)| *k == &after[..close]).map(|(_, v)| (close, *v)));
        match value {
            Some((close, v)) => {
                out.push_str(v);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Text produced by cv-core (and cv-ai / cv-mcp) while the app runs, in the interface language.
/// Unknown text is shown as it is.
pub fn tdyn(text: &str) -> String {
    match get() {
        Lang::No => text.to_string(),
        Lang::En => english(text).unwrap_or_else(|| text.to_string()),
        Lang::Pack => dyn_text(pack_table(), text),
    }
}

/// The English for a piece of dynamic text, when it is known.
fn english(text: &str) -> Option<String> {
    if text.is_empty() {
        return Some(String::new());
    }
    if let Some(en) = lookup(text) {
        return Some(en.to_string());
    }
    RULES.iter().find_map(|(re, f)| re.captures(text).map(|c| f(&c)))
}

fn lookup(text: &str) -> Option<&'static str> {
    DYN_TABLE.get(text).or_else(|| TABLE.get(text)).copied()
}

/// A word or phrase inside a pattern: its English when known, else as it is.
fn word(text: &str) -> String {
    english(text).unwrap_or_else(|| text.to_string())
}

/// «Jobb 2 (Firma)» / «Utdanning 1» from the release check.
fn item_name(text: &str) -> String {
    if let Some(rest) = text.strip_prefix("Jobb ") {
        return format!("Job {rest}");
    }
    if let Some(rest) = text.strip_prefix("Utdanning ") {
        return format!("Education {rest}");
    }
    text.to_string()
}

/// «tittel hos firma» / «en jobb» from cv-mcp.
fn job_label(text: &str) -> String {
    if text == "en jobb" {
        return "a job".into();
    }
    match text.rsplit_once(" hos ") {
        Some((title, company)) => format!("{title} at {company}"),
        None => text.to_string(),
    }
}

/// «navnet, e-posten og overskriften» from cv-mcp.
fn field_list(text: &str) -> String {
    let (head, last) = match text.rsplit_once(" og ") {
        Some((h, l)) => (h, Some(l)),
        None => (text, None),
    };
    let head = head.split(", ").map(word).collect::<Vec<_>>().join(", ");
    match last {
        Some(l) => format!("{head} and {}", word(l)),
        None => head,
    }
}

fn plural(n: &str, one: &str, many: &str) -> String {
    if n == "1" { format!("1 {one}") } else { format!("{n} {many}") }
}

type Rule = (Regex, fn(&Captures) -> String);

fn rule(pattern: &str, f: fn(&Captures) -> String) -> Rule {
    (Regex::new(&format!("^(?s:{pattern})$")).expect("i18n pattern"), f)
}

/// Messages with numbers, names or lists in them. The first that matches is used.
static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        // ---- cv-core: own checklist files ----
        rule(r"Punkt (\d+) mangler tekst\.", |c| format!("Point {} has no text.", &c[1])),
        rule(r"Punkt (\d+) har ingen egen id\.", |c| format!("Point {} has no id of its own.", &c[1])),
        rule(r"Punkt (\d+): skriv ordet CV-en skal \(eller ikke skal\) nevne\.", |c| format!("Point {}: write the word the CV should (or should not) mention.", &c[1])),
        rule(r"Punkt (\d+): antall sider må være et tall fra 1 til 10\.", |c| format!("Point {}: the number of pages must be from 1 to 10.", &c[1])),
        rule(r"Punkt (\d+): feltet må være ett av (.+)\.", |c| format!("Point {}: the field must be one of {}.", &c[1], &c[2])),
        rule(r"Filen er ikke en CVBuilder-sjekkliste: (.*)", |c| format!("The file is not a CVBuilder checklist: {}", &c[1])),
        // ---- cv-core: release check ----
        rule(r"(.+) må fylles ut\.", |c| format!("{} is required.", word(&c[1]))),
        rule(r"(Jobb \d+(?: \(.*\))?|Utdanning \d+): (.+)", |c| format!("{}: {}", item_name(&c[1]), word(&c[2]))),
        rule(r"Kompetanse (\d+) mangler navn\.", |c| format!("Skill {} has no name.", &c[1])),
        // ---- cv-core: recruiter checklist ----
        rule(r"Profilen har (\d+) ord", |c| format!("The profile has {}", plural(&c[1], "word", "words"))),
        rule(r"Bare (\d+) av (\d+) fagord fra annonsen finnes i CV-en", |c| format!("Only {} of {} keywords from the ad are in the CV", &c[1], &c[2])),
        rule(r"(\d+) av (\d+) fagord fra annonsen finnes i CV-en", |c| format!("{} of {} keywords from the ad are in the CV", &c[1], &c[2])),
        rule(r"Mangler: (.*)\. Bruk annonsens ordlyd der du faktisk har erfaringen\. Legg aldri inn ord du ikke kan stå for\.", |c| {
            format!("Missing: {}. Use the ad's wording where you really have the experience. Never add words you can't stand behind.", &c[1])
        }),
        rule(r"(\d+) kompetanser i kompetanseseksjonen", |c| format!("{} in the skills section", plural(&c[1], "skill", "skills"))),
        rule(r"Bare (\d+) kompetanser", |c| format!("Only {}", plural(&c[1], "skill", "skills"))),
        rule(r"(\d+) punkt er for lange", |c| if &c[1] == "1" { "1 bullet point is too long".into() } else { format!("{} bullet points are too long", &c[1]) }),
        rule(r"Hold hvert punkt til én–to linjer\. Gjelder: (.*)", |c| format!("Keep each bullet point to one or two lines. Applies to: {}", &c[1])),
        rule(r"(\d+) punkt starter med «Jeg» eller «Ansvarlig for»", |c| {
            if &c[1] == "1" { "1 bullet point starts with “I” or “Responsible for”".into() } else { format!("{} bullet points start with “I” or “Responsible for”", &c[1]) }
        }),
        rule(r"Start med et verb: «Ledet …», «Innførte …», «Behandlet …»\. Gjelder: (.*)", |c| format!("Start with a verb: “Led …”, “Introduced …”, “Handled …”. Applies to: {}", &c[1])),
        rule(r"Varier verbene\. Gjentatt: (.*)\.", |c| format!("Vary the verbs. Repeated: {}.", &c[1])),
        rule(r"(\d+) av (\d+) jobber viser et målbart resultat", |c| format!("{} of {} jobs show a measurable result", &c[1], &c[2])),
        rule(r"Vis heller med eksempler enn å skrive: (.*)\.", |c| format!("Show it with examples instead of writing: {}.", &c[1])),
        rule(r"(.*)\. Forklar kort, for eksempel studier, permisjon eller omstilling\.", |c| {
            static GAP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+) måneder mellom (.*) og (.*)$").expect("i18n pattern"));
            let gaps = c[1].split("; ").map(|g| GAP.captures(g).map(|m| format!("{} months between {} and {}", &m[1], &m[2], &m[3])).unwrap_or_else(|| g.to_string())).collect::<Vec<_>>().join("; ");
            format!("{gaps}. Explain briefly, for example studies, leave or a career change.")
        }),
        rule(r"(\d+) A4-sider; maks to", |c| format!("{} A4 pages; two at most", &c[1])),
        rule(r"(\d+) A4-sider?", |c| plural(&c[1], "A4 page", "A4 pages")),
        // ---- cv-core: designs ----
        rule(r"Basert på malen «(.+)» i Reactive Resume \(MIT-lisens\), © Amruth Pillai", |c| format!("Based on the “{}” template in Reactive Resume (MIT license), © Amruth Pillai", &c[1])),
        rule(r"Basert på temaet «(.+)» i RenderCV \(MIT-lisens\), © Sina Atalay og bidragsyterne", |c| {
            format!("Based on the “{}” theme in RenderCV (MIT license), © Sina Atalay and contributors", &c[1])
        }),
        rule(r"Filen er ikke en CVBuilder-design: (.*)", |c| format!("The file is not a CVBuilder design: {}", &c[1])),
        rule(r"«format» må være «(.+)»\.", |c| format!("“format” must be “{}”.", &c[1])),
        rule(r"«id» «(.+)» brukes allerede av et innebygd design\. Velg en annen id\.", |c| format!("“id” “{}” is already used by a built-in design. Choose another id.", &c[1])),
        rule(r"«(\w+)»: «(.+)» er ikke en av de tillatte skriftene \((.+)\)\.", |c| format!("“{}”: “{}” is not one of the allowed fonts ({}).", &c[1], &c[2], &c[3])),
        rule(r"«(\w+)» må være mellom (\S+) og (\S+)\.", |c| format!("“{}” must be between {} and {}.", &c[1], &c[2], &c[3])),
        rule(r"«section_colors»: «(.+)» er ikke en seksjon \((.+)\)\.", |c| format!("“section_colors”: “{}” is not a section ({}).", &c[1], &c[2])),
        rule(r"«section_colors» «(.+)» er for svak for overskrifter \(kontrast under 3:1\)\.", |c| format!("“section_colors” “{}” is too weak for headings (contrast below 3:1).", &c[1])),
        rule(r"«category» må være en av: (.+)\.", |c| format!("“category” must be one of: {}.", &c[1])),
        rule(r"«columns\.sections»: «(.+)» er ikke en seksjon\.", |c| format!("“columns.sections”: “{}” is not a section.", &c[1])),
        rule(r"«(\w+)» må være en farge som «#2F4A66»\.", |c| format!("“{}” must be a color like “#2F4A66”.", &c[1])),
        // ---- cv-core: import ----
        rule(r"Filen kunne ikke leses: (.*)", |c| format!("The file could not be read: {}", &c[1])),
        rule(r"Word-filen kunne ikke leses: (.*)", |c| format!("The Word file could not be read: {}", &c[1])),
        rule(r"PDF-en kunne ikke leses: (.*)", |c| format!("The PDF could not be read: {}", &c[1])),
        rule(r"«\.(.*)» kan ikke leses\. Bruk PDF, Word \(\.docx\), tekst \(\.txt/\.md\) eller en nettside \(\.html\)\.", |c| {
            format!("“.{}” cannot be read. Use PDF, Word (.docx), text (.txt/.md) or a web page (.html).", &c[1])
        }),
        rule(r"(Annonsen er lagt inn\.|Fra annonsen: .+?\.)(?: (\d+) fagord står allerede i CV-en din og er lagt til i kompetanse: (.+)\.)?", |c| {
            static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+) fagord$").expect("i18n pattern"));
            let head = match c[1].strip_prefix("Fra annonsen: ").and_then(|s| s.strip_suffix('.')) {
                Some(list) => {
                    let list = list.replace("stilling «", "job title «").replace("arbeidsgiver «", "employer «");
                    format!("From the ad: {}.", COUNT.replace(&list, "$1 keywords"))
                }
                None => "The ad has been added.".to_string(),
            };
            match (c.get(2), c.get(3)) {
                (Some(n), Some(list)) => format!("{head} {} already in your CV and added to your skills: {}.", plural(n.as_str(), "keyword is", "keywords are"), list.as_str()),
                _ => head,
            }
        }),
        // ---- cv-ai: fetching pages and talking to AI services ----
        rule(r"Siden svarte med feil (\d+)\. Noen nettsteder krever innlogging; lim da inn annonseteksten selv\.", |c| {
            format!("The page answered with error {}. Some sites require sign-in; then paste the ad text yourself.", &c[1])
        }),
        rule(r"Siden svarte ikke innen (\d+) sekunder\. Prøv igjen, eller kopier annonseteksten og lim den inn\.", |c| {
            format!("The page did not answer within {} seconds. Try again, or copy the ad text and paste it in.", &c[1])
        }),
        rule(r"Fikk ikke kontakt med den lokale AI-serveren på (.+)\. Kjører den fortsatt\?", |c| format!("Could not reach the local AI server at {}. Is it still running?", &c[1])),
        rule(r"Fikk ikke kontakt med (.+)\. Sjekk internettforbindelsen\.", |c| format!("Could not reach {}. Check your internet connection.", &c[1])),
        rule(r"(.+) er opptatt eller har nådd en grense\. Vent litt og prøv igjen\.", |c| format!("{} is busy or has reached a limit. Wait a moment and try again.", &c[1])),
        rule(r"(.+) avslo forespørselen\. Prøv en annen formulering eller en annen tjeneste\.", |c| format!("{} refused the request. Try different wording or another service.", &c[1])),
        rule(r"(.+) svarte, men ikke i formatet CVBuilder trenger\. Prøv igjen\.", |c| format!("{} answered, but not in the format CVBuilder needs. Try again.", &c[1])),
        rule(r"([^:]+): fikk ikke kontakt med (.+)\. Kjører Ollama, LM Studio eller serveren din\?", |c| {
            format!("{}: could not reach {}. Is Ollama, LM Studio or your server running?", &c[1], &c[2])
        }),
        rule(r"([^:]+): fant ingen modeller for denne nøkkelen", |c| format!("{}: no models found for this key", &c[1])),
        rule(r"([^:]+): oversettelsen manglet deler av CV-en; prøv igjen", |c| format!("{}: the translation left out parts of the CV; try again", &c[1])),
        // ---- cv-mcp: what a tool call changed, listed in the chat ----
        rule(r"La til jobben (.+)", |c| format!("Added the job {}", job_label(&c[1]))),
        rule(r"Fjernet jobben (.+)", |c| format!("Removed the job {}", job_label(&c[1]))),
        rule(r"Oppdaterte kjernekompetansen \((\d+) punkter\)", |c| format!("Updated the core skills ({} items)", &c[1])),
        rule(r"Oppdaterte utdanningen \((\d+)\)", |c| format!("Updated the education ({})", &c[1])),
        rule(r"Oppdaterte kurs og sertifiseringer \((\d+)\)", |c| format!("Updated courses and certifications ({})", &c[1])),
        rule(r"Byttet design til (.+)", |c| format!("Switched the design to {}", &c[1])),
        rule(r"Endret seksjonen «(.+)»", |c| format!("Changed the section “{}”", &c[1])),
        rule(r"La til seksjonen «(.+)»", |c| format!("Added the section “{}”", &c[1])),
        rule(r"Fjernet seksjonen «(.+)»", |c| format!("Removed the section “{}”", &c[1])),
        rule(r"Endret (.+) for (.+)", |c| format!("Changed {} for {}", field_list(&c[1]), job_label(&c[2]))),
        rule(r"Endret (.+)", |c| format!("Changed {}", field_list(&c[1]))),
    ]
});

/// The interface: every Norwegian string the app passes to [`t`] or [`tf`], with its English.
static EN: &[(&str, &str)] = &[
    // ---- lib.rs: window, status bar, checklist, dialogs ----
    ("Dette er et eksempel. Velg «Ny CV» for å starte på din egen, eller skriv rett inn i eksemplet.", "This is an example. Choose “New CV” to start your own, or type straight into the example."),
    ("Velkommen tilbake. Utkastet ditt er hentet fra denne maskinen.", "Welcome back. Your draft was restored from this computer."),
    (
        "Det lagrede utkastet kunne ikke åpnes, men er tatt vare på som sikkerhetskopi. Du ser nå eksemplet.",
        "The saved draft could not be opened, but it has been kept as a backup. You are now looking at the example.",
    ),
    ("Nå er dette din CV. Den huskes på denne maskinen mens du skriver.", "This is now your CV. It is remembered on this computer as you type."),
    ("CVBuilder-utkast", "CVBuilder draft"),
    ("CV (PDF, Word, tekst, nettside, CVBuilder)", "CV (PDF, Word, text, web page, CVBuilder)"),
    ("Annonse (PDF, Word, tekst, nettside)", "Job ad (PDF, Word, text, web page)"),
    ("CVBuilder-design", "CVBuilder design"),
    ("Bilde", "Photo"),
    ("Utkastet er åpnet. Bekreft opplysningene når du laster ned.", "The draft is open. Confirm the details when you download."),
    ("«{file}» kunne ikke åpnes. Her åpner du bare .json-utkast lagret med «Lagre som fil».", "“{file}” could not be opened. Only .json drafts saved with “Save as” can be opened here."),
    ("Utkastet er lagret i {path}. Filen inneholder dine personopplysninger.", "The draft is saved in {path}. The file contains your personal data."),
    ("Kunne ikke lagre: {e}", "Could not save: {e}"),
    ("Word-filen er lagret: {path}. Åpne den og se over sidebrytingen før du sender den.", "The Word file is saved: {path}. Open it and check the page breaks before you send it."),
    ("PDF-en er lagret med ekte, søkbar tekst: {path}", "The PDF is saved with real, searchable text: {path}"),
    ("Kunne ikke lagre filen: {e}", "Could not save the file: {e}"),
    ("CV-teksten er kopiert. Lim den inn der søknadsportalen ber om CV som tekst.", "The CV text is copied. Paste it where the application portal asks for your CV as text."),
    ("Skjul (vis igjen fra «Vis:» øverst)", "Hide (show it again from “View” at the top)"),
    ("Angre", "Undo"),
    ("Eksemplet er tilbake.", "The example is back."),
    ("CV-en din er tilbake.", "Your CV is back."),
    ("Husk utkastet på denne maskinen", "Remember the draft on this computer"),
    ("Utkastet huskes ikke lenger. Lagre som fil før du lukker.", "The draft is no longer remembered. Save it as a file before you close."),
    ("Bærbar modus", "Portable mode"),
    ("MCP på", "MCP on"),
    ("AI-apper på denne maskinen kan koble til CVBuilder. Se «MCP» i AI-panelet.", "AI apps on this computer can connect to CVBuilder. See “MCP” in the AI panel."),
    ("AI-assistent", "AI assistant"),
    ("Sjekkliste", "Checklist"),
    ("Sjekkliste · {n}", "Checklist · {n}"),
    ("Før nedlasting", "Before download"),
    ("Alt nødvendig er fylt ut. Du bekrefter opplysningene når du laster ned.", "Everything required is filled in. You confirm the details when you download."),
    ("Rekrutterersjekk", "Recruiter check"),
    ("Råd fra rekrutterere og søknadssystemer (ATS). Veiledning, ikke en poengsum.", "Advice from recruiters and applicant tracking systems (ATS). Guidance, not a score."),
    ("Fagord fra annonsen", "Keywords from the ad"),
    ("Står i annonsen, men ikke i CV-en. Ta det med bare hvis det stemmer.", "In the ad, but not in the CV. Add it only if it is true."),
    ("Spør AI-en om de manglende fagordene", "Ask the AI about the missing keywords"),
    (
        "Hvilke av fagordene fra annonsen som mangler i CV-en kan jeg ta med på en ærlig måte? Spør meg om det du trenger å vite.",
        "Which of the keywords from the ad that are missing in my CV can I honestly include? Ask me whatever you need to know.",
    ),
    ("FORHÅNDSVISNING", "PREVIEW"),
    ("Du ser et eksempel. Personen og firmaene er oppdiktet.", "You are looking at an example. The person and the companies are made up."),
    (
        "CV-en blir {n} sider. Rekrutterere anbefaler høyst to – kort ned der du kan, eller velg «Kompakt».",
        "The CV is {n} pages long. Recruiters recommend two at most – shorten it where you can, or choose “Kompakt”.",
    ),
    ("Lager forhåndsvisning …", "Creating preview …"),
    ("engelsk", "English"),
    ("bokmål", "Norwegian (bokmål)"),
    ("Bytte CV-en til {to}?", "Switch the CV to {to}?"),
    (
        "Overskrifter, datoer og «nå»/«Present» byttes automatisk til {to}. Teksten du har skrevet selv – profil, punkter, titler – må oversettes.",
        "Headings, dates and «nå»/“Present” switch to {to} automatically. The text you wrote yourself – profile, bullet points, titles – must be translated.",
    ),
    ("AI-en kan oversette alt for deg. Du ser endringene mens den jobber og kan angre dem.", "The AI can translate everything for you. You see the changes as it works and can undo them."),
    ("AI er ikke koblet til", "AI is not connected"),
    (
        "Da må du oversette teksten til {to} selv. CVBuilder viser originalteksten under hvert felt mens du skriver, og eksporterer ikke en blanding uten at du ser over den.",
        "Then you need to translate the text into {to} yourself. CVBuilder shows the original text under each field while you type, and does not export a mix without you checking it.",
    ),
    ("Oversett med AI", "Translate with AI"),
    ("Koble til AI …", "Connect AI …"),
    ("Bare bytt overskrifter", "Only switch headings"),
    ("Bytt, jeg oversetter selv", "Switch, I'll translate myself"),
    ("Avbryt", "Cancel"),
    (
        "Oversett hele CV-en til {to}: overskrift, profil, alle jobbpunkter, titler, kompetanse, utdanning, kurs, språk, interesser og egne seksjoner. Bytt CV-språket til {code}. Behold alle fakta og tall.",
        "Translate the whole CV into {to}: headline, profile, all job bullet points, titles, skills, education, courses, languages, interests and custom sections. Switch the CV language to {code}. Keep all facts and numbers.",
    ),
    ("CV-en er nå på {to}. Originalteksten vises under feltene mens du oversetter.", "The CV is now in {to}. The original text is shown under the fields while you translate."),
    ("SKJEMA", "FORM"),
    ("AI OG SJEKKLISTE", "AI AND CHECKLIST"),
    ("Stemmer alt i CV-en?", "Is everything in the CV correct?"),
    (
        "Les gjennom en siste gang: navn, kontaktinfo, datoer, og at alt som står der er sant – også det AI-en har skrevet.",
        "Read it through one last time: name, contact details, dates, and that everything in it is true – including what the AI wrote.",
    ),
    ("Ja, last ned", "Yes, download"),
    ("Noe viktig mangler i CV-en", "Something important is missing from the CV"),
    ("Du kan laste ned likevel, men dette bør fylles ut først:", "You can download anyway, but this should be filled in first:"),
    ("… og {n} til", "… and {n} more"),
    ("Last ned likevel", "Download anyway"),
    ("Lagre filen selv om noe mangler. Du bekrefter også at det som står i CV-en er sant.", "Save the file even though something is missing. You also confirm that what the CV says is true."),
    ("Vis sjekkliste", "Show checklist"),
    ("Lukk og se hva som mangler i sjekklisten", "Close and see what is missing in the checklist"),
    ("Ingenting er lastet ned.", "Nothing was downloaded."),
    ("Tilpass portrettet", "Adjust the portrait"),
    (
        "Beskjær og gjør små justeringer. Originalfilen endres ikke, og posisjonsdata fra kameraet blir ikke med.",
        "Crop and make small adjustments. The original file is not changed, and location data from the camera is left out.",
    ),
    ("Venstre–høyre", "Left–right"),
    ("Opp–ned", "Up–down"),
    ("Rett opp (grader)", "Straighten (degrees)"),
    ("Lysstyrke", "Brightness"),
    ("Kontrast", "Contrast"),
    ("Tilbakestill", "Reset"),
    ("Bruk bildet", "Use photo"),
    ("Portrettet er lagt inn.", "The portrait has been added."),
    ("Bildet kunne ikke lagres: {e}", "The photo could not be saved: {e}"),
    ("Alle visningene er skjult", "All views are hidden"),
    ("Velg hva du vil se under «Vis:» øverst.", "Choose what to see under “View” at the top."),
    ("Vis alt", "Show all"),
    // ---- menu_ui.rs: the ribbon, the File menu and About ----
    ("Fil", "File"),
    ("Ny CV", "New CV"),
    ("Åpne eksempel", "Open example"),
    ("Åpne", "Open"),
    ("Lagre som", "Save as"),
    ("Importer CV fra fil", "Import CV from file"),
    ("Last ned PDF", "Download PDF"),
    ("Last ned Word", "Download Word"),
    ("Kopier CV-en som tekst", "Copy the CV as text"),
    ("Om CVBuilder", "About CVBuilder"),
    ("Hjem", "Home"),
    ("Sett inn", "Insert"),
    ("Sideoppsett", "Layout"),
    ("Design", "Design"),
    ("Vis", "View"),
    ("Hjelp", "Help"),
    ("Vis verktøylinjen", "Show the toolbar"),
    ("Skjul verktøylinjen", "Hide the toolbar"),
    ("Se «Sjekkliste» for hva som mangler før nedlasting.", "See “Checklist” for what is missing before download."),
    ("{pages} side · mål maks {limit}", "{pages} page · target max {limit}"),
    ("{pages} sider · mål maks {limit}", "{pages} pages · target max {limit}"),
    ("Målet velger du under Sideoppsett. Rekrutterere anbefaler høyst to sider for de fleste.", "You set the target under Layout. Recruiters recommend two pages at most for most people."),
    ("Versjon, bygg og lisenser", "Version, build and licenses"),
    ("Format for egne design", "Format for your own designs"),
    ("Hvordan en .cvdesign.json-fil ser ut", "What a .cvdesign.json file looks like"),
    ("Start på en tom CV. Den du har nå kan hentes tilbake med Angre.", "Start a blank CV. The one you have now can be brought back with Undo."),
    ("Eksempel", "Example"),
    ("Et fiktivt eksempel med anbefalt oppsett og rekkefølge", "A fictional example with the recommended layout and order"),
    ("Åpne et lagret CVBuilder-utkast (.json)", "Open a saved CVBuilder draft (.json)"),
    ("Lagre utkastet som fil", "Save the draft as a file"),
    ("Importer CV", "Import CV"),
    ("Hent inn en CV du har fra før: PDF, Word, tekst eller nettside – med eller uten AI", "Bring in a CV you already have: PDF, Word, text or web page – with or without AI"),
    ("Stillingsannonse", "Job ad"),
    ("Lim inn annonse", "Paste ad"),
    ("Gå til feltet for annonseteksten øverst i skjemaet", "Go to the ad text field at the top of the form"),
    ("Hent fra lenke", "Fetch from link"),
    ("Hent annonsen fra lenken i skjemaet", "Fetch the ad from the link in the form"),
    ("Fra fil", "From file"),
    ("Les annonsen fra PDF, Word, tekst eller en lagret nettside", "Read the ad from PDF, Word, text or a saved web page"),
    ("Eksporter", "Export"),
    ("Kopier tekst", "Copy text"),
    ("Til portaler som ber deg lime inn CV-en som tekst", "For portals that ask you to paste your CV as text"),
    ("Hent tilbake CV-en fra før siste store endring", "Bring back the CV from before the last big change"),
    ("Språk", "Language"),
    ("Bokmål", "Norwegian"),
    ("Overskrifter og datoer på bokmål", "Headings and dates in Norwegian (bokmål)"),
    ("Overskrifter og datoer på engelsk", "Headings and dates in English"),
    ("Oversett", "Translate"),
    ("Oversett hele CV-en, med AI eller selv", "Translate the whole CV, with AI or yourself"),
    ("Erfaring", "Experience"),
    ("+ Jobb", "+ Job"),
    ("Ny jobb i arbeidserfaringen", "New job in the work experience"),
    ("+ Utdanning", "+ Education"),
    ("+ Kurs", "+ Course"),
    ("Kurs eller sertifisering", "Course or certification"),
    ("+ Seksjon", "+ Section"),
    ("Legg til en seksjon. Du kan endre navnet og flytte den i skjemaet.", "Add a section. You can rename it and move it in the form."),
    ("Egen seksjon med eget navn", "Custom section with its own name"),
    ("Kompetanse", "Skills"),
    ("+ Kompetanse", "+ Skill"),
    ("+ Språk", "+ Language"),
    ("+ Interesse", "+ Interest"),
    ("Kontakt", "Contact"),
    ("+ Kontaktfelt", "+ Contact field"),
    ("GitHub, portefølje, nettside, førerkort …", "GitHub, portfolio, website, driver's license …"),
    ("Bytt bilde", "Change photo"),
    ("Valgfritt portrett", "Optional portrait"),
    ("Farger", "Colors"),
    ("Egne design", "Your own designs"),
    ("Importer design", "Import design"),
    ("Fra en .cvdesign.json-fil", "From a .cvdesign.json file"),
    ("Lagre som mal-fil", "Save as template file"),
    ("Dette designet som fil du kan endre", "This design as a file you can edit"),
    ("Visninger", "Views"),
    ("Skjema", "Form"),
    ("Forhåndsvisning", "Preview"),
    ("AI og sjekkliste", "AI and checklist"),
    ("Tilpass", "Fit"),
    ("Tema", "Theme"),
    ("Følg systemet", "Follow system"),
    ("Lyst", "Light"),
    ("Mørkt", "Dark"),
    ("Språk i menyene", "Menu language"),
    ("Språket i menyene og knappene. CV-ens eget språk velger du under Hjem.", "The language of the menus and buttons. The CV's own language is set under Home."),
    ("Tilkobling", "Connection"),
    ("Koble til AI", "Connect AI"),
    ("Claude, ChatGPT, Gemini, Grok eller en lokal modell", "Claude, ChatGPT, Gemini, Grok or a local model"),
    ("MCP for andre apper", "MCP for other apps"),
    ("Bruk CVBuilder fra Claude Code, Claude Desktop m.fl.", "Use CVBuilder from Claude Code, Claude Desktop and others"),
    ("Assistent", "Assistant"),
    ("Åpne chatten", "Open the chat"),
    ("Ny samtale", "New conversation"),
    ("Rekrutterersjekk og hva som mangler", "Recruiter check and what is missing"),
    ("Tom CV klar. Start med kontaktinformasjonen, eller be AI-en om hjelp.", "Blank CV ready. Start with the contact details, or ask the AI for help."),
    ("Tom CV klar. Den forrige kan hentes tilbake med Angre.", "Blank CV ready. The previous one can be brought back with Undo."),
    ("Du ser eksemplet. CV-en din kan hentes tilbake med Angre.", "You are looking at the example. Your CV can be brought back with Undo."),
    ("Angret.", "Undone."),
    ("Lagt til. Fyll ut feltene i skjemaet.", "Added. Fill in the fields in the form."),
    ("CV-er som rekrutterere og søknadssystemer leser riktig", "CVs that recruiters and applicant tracking systems read correctly"),
    ("Versjon", "Version"),
    ("Bygg (git)", "Build (git)"),
    ("Bærbar modus (ved siden av programmet)", "Portable mode (next to the program)"),
    ("Brukerens egen datamappe", "The user's own data folder"),
    (
        "Lisens: MIT eller Apache-2.0. Bygger på WordCraft og Craft AI-integrasjonene (ArtCraft/storytold). Design fra Reactive Resume (MIT, © Amruth Pillai) og RenderCV (MIT, © Sina Atalay og bidragsyterne) er laget om til én spalte. Skrift: DM Sans (SIL OFL). Full liste i THIRD-PARTY.txt.",
        "License: MIT or Apache-2.0. Built on WordCraft and the Craft AI integrations (ArtCraft/storytold). Designs from Reactive Resume (MIT, © Amruth Pillai) and RenderCV (MIT, © Sina Atalay and contributors) have been remade as one column. Font: DM Sans (SIL OFL). Full list in THIRD-PARTY.txt.",
    ),
    ("Kopier versjonsinfo", "Copy version info"),
    ("Lukk", "Close"),
    // ---- page_ui.rs: Layout ----
    ("Normal · 17 mm", "Normal · 17 mm"),
    ("Smal · 12,7 mm", "Narrow · 12.7 mm"),
    ("Moderat · 19 / 25 mm", "Moderate · 19 / 25 mm"),
    ("Bred · 25,4 mm", "Wide · 25.4 mm"),
    ("Egendefinert", "Custom"),
    ("US Letter · 8,5 × 11 in", "US Letter · 8.5 × 11 in"),
    ("US Legal · 8,5 × 14 in", "US Legal · 8.5 × 14 in"),
    ("Marger: {m}", "Margins: {m}"),
    ("Egendefinerte marger", "Custom margins"),
    ("Topp", "Top"),
    ("Høyre", "Right"),
    ("Bunn", "Bottom"),
    ("Venstre", "Left"),
    ("Størrelse: {s}", "Size: {s}"),
    ("A4 brukes i Norge og Europa, Letter i USA og Canada.", "A4 is used in Norway and Europe, Letter in the US and Canada."),
    ("Spalter: designets", "Columns: as designed"),
    ("Spalter: én", "Columns: one"),
    ("Spalter: sidefelt venstre", "Columns: sidebar left"),
    ("Spalter: sidefelt høyre", "Columns: sidebar right"),
    ("Som designet", "As designed"),
    ("Én spalte (best for ATS)", "One column (best for ATS)"),
    ("To spalter, sidefelt til venstre", "Two columns, sidebar on the left"),
    ("To spalter, sidefelt til høyre", "Two columns, sidebar on the right"),
    (
        "Sidefeltet får kontakt, kompetanse, språk, kurs og interesser.\nTo spalter leses dårligere av eldre søknadssystemer.",
        "The sidebar gets contact details, skills, languages, courses and interests.\nOlder applicant tracking systems read two columns less reliably.",
    ),
    ("Sider", "Pages"),
    ("Ny side før …", "New page before …"),
    ("Ny side før … ({n})", "New page before … ({n})"),
    ("Kryss av for seksjoner som skal starte på en ny side.", "Tick the sections that should start on a new page."),
    ("Fjern alle sideskift", "Remove all page breaks"),
    ("Mål: maks {limit} sider", "Target: max {limit} pages"),
    ("Hvor mange sider du sikter mot. CVBuilder varsler når CV-en blir lengre.", "How many pages you are aiming for. CVBuilder warns you when the CV gets longer."),
    ("1 side (nyutdannet, kort erfaring)", "1 page (recent graduate, short experience)"),
    ("2 sider (anbefalt for de fleste)", "2 pages (recommended for most people)"),
    ("3 sider", "3 pages"),
    ("4 sider (akademisk, lang karriere)", "4 pages (academic, long career)"),
    ("Tekst og avstand", "Text and spacing"),
    ("Tekst", "Text"),
    ("Mindre tekst", "Smaller text"),
    ("normal", "normal"),
    ("Større tekst", "Larger text"),
    ("Avstand", "Spacing"),
    ("Tett", "Tight"),
    ("Normal", "Normal"),
    ("Luftig", "Airy"),
    ("Tilbake til standard sideoppsett", "Back to the standard layout"),
    // ---- form.rs: the form ----
    ("Profil", "Profile"),
    ("Kjernekompetanse", "Core skills"),
    ("Arbeidserfaring", "Work experience"),
    ("Utdanning", "Education"),
    ("Kurs og sertifiseringer", "Courses and certifications"),
    ("Interesser", "Interests"),
    ("E-post", "Email"),
    ("Telefon", "Phone"),
    ("By og land", "City and country"),
    ("Egen seksjon", "Custom section"),
    ("Seksjonen «{title}» er slettet. Angre hvis det var feil.", "The section “{title}” was deleted. Undo if that was a mistake."),
    ("+ Legg til egen seksjon", "+ Add custom section"),
    ("For eksempel Prosjekter, Frivillig arbeid, Publikasjoner eller Verv", "For example Projects, Volunteering, Publications or Positions of trust"),
    ("Anbefalt rekkefølge", "Recommended order"),
    ("Profil, kompetanse, erfaring, utdanning … – slik rekrutterere leser", "Profile, skills, experience, education … – the way recruiters read"),
    ("Slett seksjonen", "Delete the section"),
    ("Flytt ned i CV-en", "Move down in the CV"),
    ("Flytt opp i CV-en", "Move up in the CV"),
    ("Kontaktinformasjon", "Contact details"),
    ("Fullt navn", "Full name"),
    ("Faglig overskrift · bruk stillingens ord", "Professional headline · use the job's own words"),
    ("Kontaktlinjen · i denne rekkefølgen i CV-en", "Contact line · in this order in the CV"),
    ("Flytt opp", "Move up"),
    ("Flytt ned", "Move down"),
    ("navn@epost.no", "name@example.com"),
    ("Bergen, Norge", "Bergen, Norway"),
    ("Navn, f.eks. GitHub", "Name, e.g. GitHub"),
    ("Verdi", "Value"),
    ("+ Legg til kontaktfelt", "+ Add contact field"),
    ("For eksempel GitHub, portefølje, nettside eller førerkort", "For example GitHub, portfolio, website or driver's license"),
    ("Legg til bilde", "Add photo"),
    ("Valgfritt portrett. Vanlig i Norge, men ikke påkrevd.", "Optional portrait. Common in Norway, but not required."),
    ("Vis i CV", "Show in CV"),
    ("Fjern bilde", "Remove photo"),
    ("Stillingsannonsen du søker på", "The job ad you are applying to"),
    (
        "Start her. Lim inn annonsen, hent den fra lenken eller fra en fil. CVBuilder fyller ut stilling, arbeidsgiver og fagord, og legger fagord du allerede har i CV-en inn i kompetanse. Annonsen står ikke i CV-en.",
        "Start here. Paste the ad, or fetch it from its link or a file. CVBuilder fills in the job title, employer and keywords, and adds the keywords already in your CV to your skills. The ad itself is not part of the CV.",
    ),
    ("Lenke til annonsen", "Link to the ad"),
    ("Hent annonsen", "Fetch the ad"),
    ("Henter teksten fra lenken og fyller ut", "Fetches the text from the link and fills in the fields"),
    ("Vent til annonsen er hentet", "Wait until the ad has been fetched"),
    ("Lim inn annonseteksten i feltet først, eller hent den fra lenken eller en fil.", "Paste the ad text in the field first, or fetch it from the link or a file."),
    ("Noe gikk galt under lesingen. Prøv igjen, eller kopier teksten og lim den inn.", "Something went wrong while reading. Try again, or copy the text and paste it in."),
    ("Annonseteksten · lim inn med Ctrl+V", "Ad text · paste with Ctrl+V"),
    ("Lim inn hele stillingsannonsen her …", "Paste the whole job ad here …"),
    ("Fyll ut fra annonsen", "Fill in from the ad"),
    ("Stilling, arbeidsgiver og fagord – uten AI", "Job title, employer and keywords – without AI"),
    ("PDF, Word, tekst eller en lagret nettside", "PDF, Word, text or a saved web page"),
    ("Tilpass CV-en med AI", "Tailor the CV with AI"),
    ("AI-en går gjennom CV-en mot annonsen, uten å finne på noe", "The AI goes through the CV against the ad, without making anything up"),
    (
        "Tilpass CV-en til stillingsannonsen: bruk annonsens ord der de stemmer med det CV-en allerede sier, løft frem det mest relevante i profil og punkter, og lagre fagordene. Ikke legg til noe jeg ikke har. Si til slutt hva jeg kan legge til selv.",
        "Tailor the CV to the job ad: use the ad's words where they match what the CV already says, bring the most relevant things forward in the profile and bullet points, and save the keywords. Don't add anything I don't have. Finally, tell me what I could add myself.",
    ),
    ("Stilling", "Job title"),
    ("Arbeidsgiver", "Employer"),
    ("Viktige fagord fra annonsen · ett per linje", "Important keywords from the ad · one per line"),
    ("Finn fagord med AI", "Find keywords with AI"),
    ("Les stillingsannonsen og lagre de viktigste fagordene fra den (høyst 25).", "Read the job ad and save its most important keywords (25 at most)."),
    ("Kort profil · 30–70 ord", "Short profile · 30–70 words"),
    ("Skriv profil med AI", "Write profile with AI"),
    ("AI-en skriver en profil ut fra det som allerede står i CV-en", "The AI writes a profile from what is already in the CV"),
    (
        "Skriv en kort profiltekst (30–70 ord) ut fra det som står i CV-en, rettet mot stillingen jeg søker.",
        "Write a short profile (30–70 words) from what is in the CV, aimed at the job I am applying for.",
    ),
    ("Bruk annonsens ordlyd der den stemmer. Bare det du kan stå for.", "Use the ad's wording where it fits. Only what you can stand behind."),
    ("Gruppe", "Group"),
    ("Kompetanse eller verktøy", "Skill or tool"),
    ("+ Legg til kompetanse", "+ Add skill"),
    (
        "2–4 korte punkter per jobb. Start med et verb og vis resultatet: «Oppnådde X, målt ved Y, ved å gjøre Z».",
        "2–4 short bullet points per job. Start with a verb and show the result: “Achieved X, measured by Y, by doing Z”.",
    ),
    ("Jobb {n}", "Job {n}"),
    ("Slett jobben", "Delete the job"),
    ("Stillingstittel", "Job title"),
    ("Sted · by", "Location · city"),
    ("Fra (09.2021 eller 2010)", "From (09.2021 or 2010)"),
    ("Til (eller «nå»)", "To (or “now”)"),
    ("Nåværende jobb", "Current job"),
    ("Hva du gjorde og oppnådde · ett punkt per linje", "What you did and achieved · one point per line"),
    ("Forbedre punktene med AI", "Improve the bullet points with AI"),
    ("Jobben «{title}» er slettet. Angre hvis det var feil.", "The job “{title}” was deleted. Undo if that was a mistake."),
    (
        "Forbedre punktene for jobben «{title}» hos {company} (id {id}), slik at de viser handling og resultat og bruker annonsens ord der det stemmer.",
        "Improve the bullet points for the job “{title}” at {company} (id {id}), so they show action and result and use the ad's words where they fit.",
    ),
    ("+ Legg til jobb", "+ Add job"),
    ("Grad, fagbrev eller studium", "Degree, trade certificate or program"),
    ("Slett utdanningen", "Delete the education"),
    ("Lærested", "School or university"),
    ("Fra · år", "From · year"),
    ("Til · år", "To · year"),
    ("+ Legg til utdanning", "+ Add education"),
    ("Utsteder", "Issuer"),
    ("+ Legg til kurs", "+ Add course"),
    ("Nivå: morsmål, flytende, B2 …", "Level: native, fluent, B2 …"),
    ("+ Legg til språk", "+ Add language"),
    ("Valgfritt. Ta med høyst tre korte, ekte interesser.", "Optional. Include at most three short, genuine interests."),
    ("Ta med interesser i CV-en", "Include interests in the CV"),
    ("Ta med i CV-en", "Include in the CV"),
    ("Nå", "Now"),
    ("Tidligere", "Former"),
    ("Interesse for", "Interested in"),
    ("+ Legg til interesse", "+ Add interest"),
    ("Overskrift i CV-en", "Heading in the CV"),
    ("Tittel", "Title"),
    ("Slett oppføringen", "Delete the entry"),
    ("F.eks. Ny barnehage, Trondheim", "E.g. New kindergarten, Trondheim"),
    ("Organisasjon eller sted", "Organization or place"),
    ("Tid (fritt, f.eks. 2023)", "Time (free text, e.g. 2023)"),
    ("Punkter · ett per linje", "Bullet points · one per line"),
    ("+ Legg til oppføring", "+ Add entry"),
    ("Original før språkbytte:", "Original before the language switch:"),
    ("Skriv årstall, f.eks. 2021.", "Enter a year, e.g. 2021."),
    ("Skriv måned og år, f.eks. 09.2021.", "Enter month and year, e.g. 09.2021."),
    ("År", "Year"),
    // ---- chat_ui.rs: the AI chat and the MCP window ----
    ("Leser CV-en", "Reading the CV"),
    ("Leste CV-en", "Read the CV"),
    ("Sjekker CV-en", "Checking the CV"),
    ("Sjekket CV-en", "Checked the CV"),
    ("Endrer profilen", "Changing the profile"),
    ("Endret profilen", "Changed the profile"),
    ("Endrer stillingen", "Changing the target job"),
    ("Endret stillingen", "Changed the target job"),
    ("Skriver arbeidserfaring", "Writing work experience"),
    ("Skrev arbeidserfaring", "Wrote work experience"),
    ("Fjerner en jobb", "Removing a job"),
    ("Fjernet en jobb", "Removed a job"),
    ("Oppdaterer kompetanse", "Updating skills"),
    ("Oppdaterte kompetanse", "Updated skills"),
    ("Oppdaterer utdanning", "Updating education"),
    ("Oppdaterte utdanning", "Updated education"),
    ("Oppdaterer kurs", "Updating courses"),
    ("Oppdaterte kurs", "Updated courses"),
    ("Oppdaterer språk", "Updating languages"),
    ("Oppdaterte språk", "Updated languages"),
    ("Oppdaterer interesser", "Updating interests"),
    ("Oppdaterte interesser", "Updated interests"),
    ("Ser på designene", "Looking at the designs"),
    ("Så på designene", "Looked at the designs"),
    ("Bytter design", "Switching design"),
    ("Byttet design", "Switched design"),
    ("Endrer kontaktlinjen", "Changing the contact line"),
    ("Endret kontaktlinjen", "Changed the contact line"),
    ("Endrer rekkefølgen", "Changing the order"),
    ("Endret rekkefølgen", "Changed the order"),
    ("Endrer fargene", "Changing the colors"),
    ("Endrer sideoppsettet", "Changing the page layout"),
    (
        "Språket i menyene og knappene. Mangler språket ditt, kan du legge det til med AI eller fra en språkfil.",
        "The language of the menus and buttons. If yours is missing, add it with AI or from a language file.",
    ),
    ("Tilbakestill språk", "Reset language"),
    ("Fjern endringene dine, så språket blir slik det fulgte med CVBuilder", "Remove your changes, so the language is as it came with CVBuilder"),
    ("Språk som følger med CVBuilder kan ikke slettes; bare endringer du har gjort", "Languages that come with CVBuilder can't be deleted; only changes you have made"),
    ("Overskriftene byttes med en gang; teksten oversetter du med «Oversett»", "The headings change right away; translate the text with “Translate”"),
    ("Annet språk …", "Other language …"),
    ("Et hvilket som helst språk: skriv det inn, så lager AI-en eller du overskriftene", "Any language: type it, and the AI or you write the headings"),
    ("Språket i CV-en: overskrifter og datoer", "The language of the CV: headings and dates"),
    ("CV-en har nå overskrifter på {language}. Trykk «Oversett» for å oversette teksten også.", "The CV now has headings in {language}. Press “Translate” to translate the text too."),
    (
        "AI-en oversetter all teksten og overskriftene. Du ser endringene mens den jobber og kan angre dem.",
        "The AI translates all the text and the headings. You see the changes as it works and can undo them.",
    ),
    ("AI-en oversetter hele CV-en og overskriftene", "The AI translates the whole CV and the headings"),
    ("Anbefalt: {advice} sider", "Recommended: {advice} pages"),
    (
        "CV-en er {pages} sider. Vi anbefaler sterkt {advice}: rekrutterere leser raskt, og det viktigste må komme først. Kort ned, eller velg «Kompakt».",
        "The CV is {pages} pages. We strongly recommend {advice}: recruiters read fast, and what matters most must come first. Shorten it, or choose “Kompakt”.",
    ),
    (
        "CV-en har nå overskrifter på {language}. Originalteksten vises under feltene mens du oversetter.",
        "The CV now has headings in {language}. The original text is shown under the fields while you translate.",
    ),
    (
        "Da oversetter du teksten selv. Overskriftene byttes nå, og originalteksten vises under hvert felt mens du skriver.",
        "Then you translate the text yourself. The headings change now, and the original text is shown under each field while you write.",
    ),
    (
        "Det er ingen grense, men rekrutterere bruker sekunder på hver CV: én side for kort erfaring, to sider for de fleste.",
        "There is no limit, but recruiters spend seconds on each CV: one page for a short career, two pages for most people.",
    ),
    ("Det som kommer etter «{title}», starter på en ny side", "What comes after “{title}” starts on a new page"),
    ("Et hvilket som helst språk, skrevet på det språket, f.eks. Kiswahili eller 日本語", "Any language, written in that language, e.g. Kiswahili or 日本語"),
    ("Etter", "After"),
    ("Før", "Before"),
    ("Koble til en AI-tjeneste eller en lokal modell for å oversette automatisk", "Connect an AI service or a local model to translate automatically"),
    ("Kort erfaring: én side holder.", "Short career: one page is enough."),
    ("Lukk uten å endre noe", "Close without changing anything"),
    ("Ny side … ({n})", "New page … ({n})"),
    ("Ny side …", "New page …"),
    ("Oversett CV-en", "Translate the CV"),
    (
        "Oversett hele CV-en til {language}: overskrift, profil, alle jobbpunkter og titler, kompetanse, utdanning, kurs, språk, interesser og egne seksjoner. Sett CV-ens overskrifter med cv_set_headings (language «{language}»).{headings} Behold alle fakta, navn på firma og tall.",
        "Translate the whole CV into {language}: headline, profile, every job bullet and title, skills, education, courses, languages, interests and custom sections. Set the CV's headings with cv_set_headings (language “{language}”).{headings} Keep all facts, company names and numbers.",
    ),
    ("Overskriften slik den skal stå i CV-en på dette språket", "The heading as it should appear in the CV in this language"),
    ("Overskriftene byttes; teksten oversetter du selv med originalen som hjelp", "The headings change; you translate the text yourself with the original as a guide"),
    ("Overskriftene finnes ferdig på dette språket", "The headings are ready-made in this language"),
    ("Overskriftene finnes ferdig på dette språket.", "The headings are ready-made in this language."),
    ("Overskriftene må skrives: AI-en gjør det, eller du fyller dem inn under.", "The headings must be written: the AI does it, or you fill them in below."),
    ("Overskrifter i CV-en", "Headings in the CV"),
    ("Se og endre overskriftene CV-en får på dette språket", "See and change the headings the CV gets in this language"),
    ("Seksjon", "Section"),
    ("Start en seksjon på en ny side, før eller etter den, så ingen deles stygt", "Start a section on a new page, before or after it, so none is split awkwardly"),
    ("To sider for de fleste. Ingen grense.", "Two pages for most people. No limit."),
    ("Velg eller skriv et språk først", "Choose or type a language first"),
    ("Velg et språk fra listen, eller skriv et hvilket som helst språk i feltet ved siden av", "Choose a language from the list, or type any language in the field next to it"),
    ("Velg seksjoner som skal starte en ny side (før), eller som skal avsluttes med et sideskift (etter).", "Choose sections that start a new page (before), or that end with a page break (after)."),
    ("{pages} side · anbefalt {advice}", "{pages} page · recommended {advice}"),
    ("{pages} sider · anbefalt {advice}", "{pages} pages · recommended {advice}"),
    ("«nå» (nåværende jobb)", "“present” (current job)"),
    ("… eller skriv et språk", "… or type a language"),
    ("Plassering i CV-en", "Position in the CV"),
    ("Nederst", "At the bottom"),
    ("Øverst", "At the top"),
    ("Etter «{name}»", "After “{name}”"),
    ("Seksjonen kommer sist i CV-en. Du kan flytte den senere.", "The section comes last in the CV. You can move it later."),
    ("Seksjonen kommer først, før profilen", "The section comes first, before the profile"),
    ("Seksjonen kommer rett etter denne", "The section comes right after this one"),
    ("Hvor i CV-en den nye seksjonen skal stå. Pilene på seksjonen flytter den senere.", "Where in the CV the new section goes. The arrows on the section move it later."),
    (" sider", " pages"),
    ("+ Legg til punkt", "+ Add point"),
    ("CV-en nevner ikke …", "The CV does not mention …"),
    ("CV-en nevner …", "The CV mentions …"),
    ("CVBuilder ser etter dette i hele CV-teksten, uten å bry seg om store og små bokstaver", "CVBuilder looks for this in the whole CV text, ignoring upper and lower case"),
    ("Det høyeste antallet sider CV-en kan ha for at punktet er i orden", "The most pages the CV can have for this point to be met"),
    ("Din egen sjekkliste i stedet for standardsjekken", "Your own checklist instead of the standard check"),
    (
        "Du har ingen egen sjekkliste ennå. Trykk «Rediger» for å lage en, eller «Last inn» for å hente en fra fil.",
        "You have no checklist of your own yet. Press “Edit” to make one, or “Load” to open one from a file.",
    ),
    ("Erstatt punktene med et eksempel du kan bygge videre på", "Replace the points with an example you can build on"),
    ("Et nytt punkt nederst i listen", "A new point at the bottom of the list"),
    ("Feltet er fylt ut", "The field is filled in"),
    ("Feltet som må være fylt ut", "The field that must be filled in"),
    ("Hent en sjekkliste fra en .cvchecklist.json-fil", "Open a checklist from a .cvchecklist.json file"),
    ("Hva punktet sier, f.eks. «Har bedt om referanse»", "What the point says, e.g. “Asked for a reference”"),
    ("Hva slags punkt dette er", "What kind of point this is"),
    ("Hva slags punkt dette er: noe du krysser av selv, eller noe CVBuilder sjekker", "What kind of point this is: something you tick yourself, or something CVBuilder checks"),
    ("Hvilket felt i CV-en som må være fylt ut", "Which field in the CV must be filled in"),
    ("Høyst antall sider", "At most this many pages"),
    ("Jeg krysser av selv", "I tick it myself"),
    ("Kryss av når du har gjort dette. Avkrysningen lagres med denne CV-en.", "Tick this when you have done it. The tick is saved with this CV."),
    ("Kunne ikke lagre sjekklisten: {error}", "Could not save the checklist: {error}"),
    ("Sjekklisten din kunne ikke leses: {error}", "Your checklist could not be read: {error}"),
    ("Listen har så mange punkter som den kan ha", "The list has as many points as it can have"),
    ("Lag en egen sjekkliste først", "Make a checklist of your own first"),
    ("Lagre sjekklisten din som fil, for å dele den eller ta vare på den", "Save your checklist as a file, to share it or keep it"),
    ("Lagre sjekklisten og bruk den i sjekkliste-fanen", "Save the checklist and use it in the Checklist tab"),
    ("Lagre som fil", "Save as file"),
    ("Lagre", "Save"),
    ("Last inn", "Load"),
    ("Legg til, endre og fjern punkter i din egen sjekkliste", "Add, change and remove points in your own checklist"),
    ("Lukk uten å lagre endringene", "Close without saving the changes"),
    ("Min egen", "My own"),
    ("Min sjekkliste", "My checklist"),
    ("Navn på sjekklisten", "Name of the checklist"),
    ("Ord eller uttrykk; flere kan skilles med /", "Word or phrase; separate several with /"),
    (
        "Punkter CVBuilder sjekker selv (ord i CV-en, antall sider, utfylte felt), og punkter du krysser av selv.",
        "Points CVBuilder checks by itself (words in the CV, page count, filled-in fields), and points you tick yourself.",
    ),
    ("Rediger", "Edit"),
    ("Rekrutterersjekken CVBuilder har innebygd", "The recruiter check built into CVBuilder"),
    ("Rett feilene over først", "Fix the errors above first"),
    ("Sjekklisten er lagret.", "The checklist has been saved."),
    ("Sjekklisten er lagret: {path}", "The checklist has been saved: {path}"),
    ("Slett punktet", "Delete the point"),
    ("Standard", "Standard"),
    ("Start fra eksempel", "Start from an example"),
    ("Teksten som vises i sjekklisten", "The text shown in the checklist"),
    ("Velg felt", "Choose a field"),
    ("Vises som overskrift i sjekkliste-fanen", "Shown as the heading in the Checklist tab"),
    ("{done} av {all} punkter er i orden.", "{done} of {all} points are met."),
    ("«{name}» er lastet inn og brukes nå.", "“{name}” has been loaded and is now in use."),
    ("Innstillinger", "Settings"),
    ("Språk i menyene, tema og mer", "Menu language, theme and more"),
    ("Utkast", "Draft"),
    ("Data", "Data"),
    ("Bærbar modus: alt lagres i mappen ved siden av programmet.", "Portable mode: everything is saved in the folder next to the program."),
    ("Lagres i brukerens egen datamappe.", "Saved in your user data folder."),
    ("API-nøkler lagres i maskinens nøkkelring, aldri i denne mappen.", "API keys are kept in the computer's keychain, never in this folder."),
    ("Ny seksjon", "New section"),
    ("Navn på seksjonen, slik det står i CV-en", "Name of the section, as it appears in the CV"),
    ("Legg til", "Add"),
    ("Tilkoblet: {name}", "Connected: {name}"),
    ("Seksjonen «{name}» er lagt til. Fyll den ut i skjemaet.", "The section “{name}” has been added. Fill it in in the form."),
    ("Står feltet tomt, heter den «{name}». Du kan endre navnet senere.", "If you leave this empty, it will be called “{name}”. You can rename it later."),
    ("Endret sideoppsett", "Changed the page layout"),
    ("Endret fargene", "Changed the colors"),
    ("Skriver egen seksjon", "Writing a custom section"),
    ("Skrev egen seksjon", "Wrote a custom section"),
    ("Sletter en seksjon", "Deleting a section"),
    ("Slettet en seksjon", "Deleted a section"),
    ("Bytter språk", "Switching language"),
    ("Byttet språk", "Switched language"),
    ("{doing}: gikk ikke", "{doing}: failed"),
    ("Skreddersy CV-en til stillingsannonsen", "Tailor the CV to the job ad"),
    ("Gjør punktene i arbeidserfaringen mer resultatorienterte", "Make the work experience bullet points more results-focused"),
    ("Skriv en kort profiltekst", "Write a short profile"),
    ("Hvilke fagord fra annonsen mangler i CV-en?", "Which keywords from the ad are missing in the CV?"),
    ("Få CV-en ned på to sider", "Get the CV down to two pages"),
    ("Oversett CV-en til engelsk", "Translate the CV into English"),
    ("MCP-forbindelsen kunne ikke startes på denne maskinen, så Claude Code kan ikke endre CV-en.", "The MCP connection could not be started on this computer, so Claude Code cannot change the CV."),
    ("Samtalen stoppet uventet.", "The conversation stopped unexpectedly."),
    ("Stoppet.", "Stopped."),
    ("Du snakker med", "You are talking to"),
    ("Bruk CVBuilder fra Claude Code, Claude Desktop eller en annen AI-app", "Use CVBuilder from Claude Code, Claude Desktop or another AI app"),
    ("Tenker …", "Thinking …"),
    ("AI-endringene er angret.", "The AI changes were undone."),
    ("Endringene er angret.", "The changes were undone."),
    ("Claude Code · din egen innlogging", "Claude Code · your own sign-in"),
    ("Ingen AI tilkoblet", "No AI connected"),
    ("Claude Code · din egen innlogging, via MCP", "Claude Code · your own sign-in, via MCP"),
    ("Koble til AI-tjeneste med API-nøkkel …", "Connect an AI service with an API key …"),
    ("Kjører på denne maskinen ({url}). CV-en forlater ikke maskinen.", "Runs on this computer ({url}). The CV never leaves the computer."),
    ("Sendes til din AI-server på {url}.", "Sent to your AI server at {url}."),
    ("Meldinger og CV-tekst sendes til {company} med din nøkkel.", "Messages and CV text are sent to {company} with your key."),
    ("Kjører i Claude Code på denne maskinen, med kontoen du er logget inn med.", "Runs in Claude Code on this computer, with the account you are signed in with."),
    ("Hva vil du gjøre med CV-en?", "What would you like to do with the CV?"),
    (
        "Skriv med egne ord. AI-en endrer CV-en direkte, du ser endringene med en gang, og kan angre dem. Den finner aldri på fakta: der et tall mangler, skriver den [tall] som du fyller inn.",
        "Write in your own words. The AI changes the CV directly, you see the changes right away, and you can undo them. It never makes up facts: where a number is missing, it writes [number] for you to fill in.",
    ),
    ("Koble til en AI først", "Connect an AI first"),
    (
        "Bruk Claude, ChatGPT, Gemini eller Grok med din egen API-nøkkel. Har du Claude Code installert, kan du bruke det i stedet.",
        "Use Claude, ChatGPT, Gemini or Grok with your own API key. If you have Claude Code installed, you can use that instead.",
    ),
    ("Be om en endring, f.eks. «gjør profilen kortere»", "Ask for a change, e.g. “make the profile shorter”"),
    ("Enter sender · Shift+Enter gir ny linje", "Enter sends · Shift+Enter adds a new line"),
    ("Stopp", "Stop"),
    ("Bruk CVBuilder fra en annen AI-app", "Use CVBuilder from another AI app"),
    (
        "CVBuilder er en MCP-server. AI-apper som støtter MCP, som Claude Code og Claude Desktop, kan lese og endre CV-en som er åpen her, og du ser endringene med en gang. Hver endring kan angres i chatten.",
        "CVBuilder is an MCP server. AI apps that support MCP, such as Claude Code and Claude Desktop, can read and change the CV open here, and you see the changes right away. Every change can be undone in the chat.",
    ),
    ("Claude Code: kjør denne kommandoen i en terminal", "Claude Code: run this command in a terminal"),
    ("Andre apper (f.eks. Claude Desktop): legg dette inn i MCP-innstillingene", "Other apps (e.g. Claude Desktop): add this to the MCP settings"),
    (
        "CVBuilder må være åpent mens AI-appen jobber. Forbindelsen går bare på denne maskinen (127.0.0.1) og krever en nøkkel som bare CVBuilder kjenner.",
        "CVBuilder must be open while the AI app works. The connection stays on this computer (127.0.0.1) and needs a key that only CVBuilder knows.",
    ),
    ("Kopier", "Copy"),
    ("Endret av en AI-app over MCP", "Changed by an AI app over MCP"),
    ("Endringer i CV-en", "Changes to the CV"),
    ("Angre disse endringene", "Undo these changes"),
    ("Angret", "Undone"),
    // ---- ai_ui.rs: Connect AI ----
    ("Tilkoblet. {service} fant {n} modeller; den anbefalte er valgt.", "Connected. {service} found {n} models; the recommended one is selected."),
    ("Små modeller kan streve med å bruke verktøy; modeller som qwen3 eller llama3.1 fungerer best.", "Small models can struggle to use tools; models such as qwen3 or llama3.1 work best."),
    ("Denne maskinen har ingen nøkkelring, så nøkkelen huskes bare til du lukker programmet.", "This computer has no keychain, so the key is only remembered until you close the program."),
    (
        "Fant ingen lokal AI-server på de vanlige adressene. Start Ollama eller LM Studio, eller skriv inn adressen selv.",
        "No local AI server was found at the usual addresses. Start Ollama or LM Studio, or enter the address yourself.",
    ),
    (
        "Valgfritt. Med AI kan du be om endringer i chatten, og se CV-en endre seg mens AI-en jobber. Alt kan angres.",
        "Optional. With AI you can ask for changes in the chat and watch the CV change while the AI works. Everything can be undone.",
    ),
    ("Velg tjenesten du har en API-nøkkel til.", "Choose the service you have an API key for."),
    ("1. Lag en API-nøkkel hos {company}:", "1. Create an API key at {company}:"),
    ("2. Lim inn nøkkelen her:", "2. Paste the key here:"),
    ("API-nøkkel", "API key"),
    (
        "Når du bruker AI-chatten, sendes meldingene dine, CV-teksten og stillingsannonsen til {company}. {company} tar betalt for bruken på din nøkkel. Nøkkelen lagres i maskinens egen nøkkelring, aldri i CV-filen.",
        "When you use the AI chat, your messages, the CV text and the job ad are sent to {company}. {company} charges the usage to your key. The key is stored in the computer's own keychain, never in the CV file.",
    ),
    ("Jeg forstår og vil koble til", "I understand and want to connect"),
    ("3. Test og koble til", "3. Test and connect"),
    ("Tester nøkkelen hos {service} …", "Testing the key with {service} …"),
    ("Koble fra og slett nøkkelen", "Disconnect and delete the key"),
    ("Koblet fra {service}. Nøkkelen er slettet fra denne maskinen.", "Disconnected from {service}. The key has been deleted from this computer."),
    ("Modell", "Model"),
    (
        "Bruk en AI-modell som kjører på din egen maskin, for eksempel med Ollama eller LM Studio. Ingen nøkkel og ingen kostnad, og CV-en forlater ikke maskinen. Modellen endrer CV-en med de samme verktøyene (MCP-verktøyene til CVBuilder) som de andre tjenestene.",
        "Use an AI model that runs on your own computer, for example with Ollama or LM Studio. No key and no cost, and the CV never leaves the computer. The model changes the CV with the same tools (CVBuilder's MCP tools) as the other services.",
    ),
    ("Har du ikke en lokal server?", "Don't have a local server?"),
    ("Last ned Ollama", "Download Ollama"),
    ("og kjør for eksempel «ollama pull qwen3».", "and run, for example, “ollama pull qwen3”."),
    ("1. Adressen til serveren (OpenAI-kompatibel, slutter vanligvis på /v1):", "1. The server address (OpenAI-compatible, usually ends in /v1):"),
    ("Finn lokale servere", "Find local servers"),
    ("Ser etter lokale AI-servere …", "Looking for local AI servers …"),
    ("Fant {name} på {url} ({count} modeller)", "Found {name} at {url} ({count} models)"),
    ("2. Nøkkel (bare hvis serveren din krever det):", "2. Key (only if your server requires one):"),
    ("Valgfri", "Optional"),
    ("Adressen er på denne maskinen: meldingene og CV-teksten blir her.", "The address is on this computer: the messages and the CV text stay here."),
    ("Merk: adressen er ikke på denne maskinen. Meldingene og CV-teksten sendes dit.", "Note: the address is not on this computer. The messages and the CV text are sent there."),
    ("Kobler til den lokale serveren …", "Connecting to the local server …"),
    ("Koble fra", "Disconnect"),
    ("Koblet fra den lokale modellen.", "Disconnected from the local model."),
    // ---- designs_ui.rs: designs and the import window ----
    ("Dine designs", "Your designs"),
    ("Design: {name}", "Design: {name}"),
    ("To spalter", "Two columns"),
    ("Importer eget design …", "Import your own design …"),
    ("Lagre dette designet som mal-fil …", "Save this design as a template file …"),
    ("Filen kunne ikke leses: {e}", "The file could not be read: {e}"),
    ("«{name}» er importert og valgt. Den ligger under «Dine designs».", "“{name}” is imported and selected. You'll find it under “Your designs”."),
    ("Designet er gyldig, men kunne ikke lagres: {e}", "The design is valid, but could not be saved: {e}"),
    ("Kunne ikke importere «{file}»:\n{e}", "Could not import “{file}”:\n{e}"),
    ("Mal-filen er lagret: {path}. Endre den og importer den igjen.", "The template file is saved: {path}. Edit it and import it again."),
    ("Kunne ikke lagre mal-filen: {e}", "Could not save the template file: {e}"),
    ("Importer eget design", "Import your own design"),
    (
        "Et design bestemmer skrift, størrelser, farger, toppfelt og overskrifter. Teksten og den ene spalten er lik i alle design, så CV-en leses like godt av søknadssystemer (ATS).",
        "A design sets the fonts, sizes, colors, header and headings. The text and the single column are the same in every design, so the CV reads just as well in applicant tracking systems (ATS).",
    ),
    ("Filformat: .cvdesign.json", "File format: .cvdesign.json"),
    (
        "En JSON-fil (UTF-8) med feltene under. «format» må være «cvbuilder-design/1». Lag en ved å lagre et design du liker som mal-fil og endre den.",
        "A JSON file (UTF-8) with the fields below. “format” must be “cvbuilder-design/1”. Make one by saving a design you like as a template file and editing it.",
    ),
    (
        "Ikke mulig, med vilje: to spalter, sidefelt, tabeller, tekstbokser, ikoner, ferdighetsstolper og tekst i topp- eller bunntekst. Det gjør CV-er vanskeligere å lese for ATS.",
        "Not possible, on purpose: two columns, sidebars, tables, text boxes, icons, skill bars and text in headers or footers. They make CVs harder for ATS to read.",
    ),
    ("Velg fil …", "Choose file …"),
    ("Lagre mal-fil å starte fra …", "Save a template file to start from …"),
    ("2–40 tegn: a–z, 0–9 og bindestrek. Ikke det samme som et innebygd design.", "2–40 characters: a–z, 0–9 and hyphen. Not the same as a built-in design."),
    ("Navn og én setning, vises i listen.", "Name and one sentence, shown in the list."),
    ("Hvem designet bygger på (valgfritt).", "Who the design is based on (optional)."),
    (
        "Arial, Calibri, Cambria, Georgia, Garamond, Times New Roman, Verdana, Trebuchet MS, Tahoma, Palatino Linotype, Book Antiqua, Century Gothic, Segoe UI eller Aptos.",
        "Arial, Calibri, Cambria, Georgia, Garamond, Times New Roman, Verdana, Trebuchet MS, Tahoma, Palatino Linotype, Book Antiqua, Century Gothic, Segoe UI or Aptos.",
    ),
    ("Brødtekst 9,5–12 pt.", "Body text 9.5–12 pt."),
    ("Navnet 16–34 pt. name_bold, name_upper: true/false. name_spacing: 0–3.", "The name 16–34 pt. name_bold, name_upper: true/false. name_spacing: 0–3."),
    (
        "Farger som «#2F4A66»: overskrifter, brødtekst, datoer, linjer. Brødteksten må ha kontrast minst 4,5:1.",
        "Colors like “#2F4A66”: headings, body text, dates, lines. The body text needs a contrast of at least 4.5:1.",
    ),
    ("left eller center.", "left or center."),
    ("Fargebånd bak toppfeltet og tekstfargen på det, eller null.", "A color band behind the header and the text color on it, or null."),
    ("rule, rule_above, double, bar, band eller plain. band trenger heading_fill.", "rule, rule_above, double, bar, band or plain. band needs heading_fill."),
    ("right (samme linje) eller below (egen linje).", "right (same line) or below (own line)."),
    ("Luft mellom delene, 0,75–1,3.", "Space between the parts, 0.75–1.3."),
    // ---- colors_ui.rs: colors ----
    ("Bruk designets egen farge", "Use the design's own color"),
    ("designets", "from design"),
    ("Som i designet", "As in the design"),
    ("Farger · egne", "Colors · custom"),
    ("Fargene er uavhengige av designet. Bytter du design, beholdes fargene du har valgt.", "Colors are independent of the design. If you switch design, the colors you chose are kept."),
    ("Ferdige paletter", "Ready-made palettes"),
    ("Dine farger", "Your colors"),
    ("Hovedfarge", "Main color"),
    ("Brødtekst", "Body text"),
    ("Datoer og detaljer", "Dates and details"),
    ("Linjer", "Lines"),
    ("Fargefelt øverst", "Color band at the top"),
    ("Tekst i fargefeltet", "Text in the color band"),
    ("Bånd bak overskrifter", "Band behind headings"),
    ("Hver seksjon kan også få sin egen farge: bruk fargeknappen på seksjonen i skjemaet.", "Each section can also have its own color: use the color button on the section in the form."),
    ("Tilbake til designets farger", "Back to the design's colors"),
    ("Seksjonens egen farge. Høyreklikk for å bruke hovedfargen igjen.", "The section's own color. Right-click to use the main color again."),
    ("Gi seksjonen sin egen farge", "Give the section its own color"),
    // ---- import_ui.rs: importing a CV or a job ad ----
    ("CV-en er hentet inn. Den forrige kan hentes tilbake med Angre.", "The CV has been imported. The previous one can be brought back with Undo."),
    ("JSON-filen er ikke et CVBuilder-utkast.", "The JSON file is not a CVBuilder draft."),
    ("Leser CV-en …", "Reading the CV …"),
    ("Leser annonsen …", "Reading the ad …"),
    ("Lim inn lenken til annonsen i feltet over først.", "Paste the link to the ad in the field above first."),
    ("Henter annonsen …", "Fetching the ad …"),
    ("{note} Angre hvis noe ble feil.", "{note} Undo if something went wrong."),
    ("Ny CV fra filen. AI-en fyller den ut nå; den forrige kan hentes tilbake med Angre.", "New CV from the file. The AI is filling it in now; the previous one can be brought back with Undo."),
    (
        "Her er teksten fra en CV jeg har fra før. Fyll ut CV-en i CVBuilder med disse opplysningene: kontaktinfo, overskrift, profil, alle jobber med datoer og punkter, utdanning, kompetanse, kurs, språk og interesser. Bruk bare det som står i teksten, og skriv punktene kort med verb først. Behold alle tall. Si til slutt hva jeg bør sjekke.\n\n---\n{cut}",
        "Here is the text of a CV I already have. Fill in the CV in CVBuilder with this information: contact details, headline, profile, all jobs with dates and bullet points, education, skills, courses, languages and interests. Use only what is in the text, and keep the bullet points short, verb first. Keep all numbers. Finally, tell me what I should check.\n\n---\n{cut}",
    ),
    ("lite", "very little"),
    ("Hentet inn {found}. Den forrige CV-en kan hentes tilbake med Angre.", "Imported {found}. The previous CV can be brought back with Undo."),
    ("Hent inn en CV du har fra før", "Import a CV you already have"),
    ("Filer som kan leses", "Files that can be read"),
    (
        "PDF med ekte tekst (ikke skannet) · Word (.docx) · tekst (.txt, .md) · nettside (.html) · CVBuilder-utkast (.json)",
        "PDF with real text (not scanned) · Word (.docx) · text (.txt, .md) · web page (.html) · CVBuilder draft (.json)",
    ),
    ("Med AI – fyller ut alt, også jobber og utdanning", "With AI – fills in everything, including jobs and education"),
    ("Koble til AI under AI-fanen for å bruke dette.", "Connect AI under the AI tab to use this."),
    (
        "Uten AI – kontaktinfo, profil, kompetanse og språk. Resten vises, så du kan lime det inn.",
        "Without AI – contact details, profile, skills and languages. The rest is shown so you can paste it in.",
    ),
    (
        "Du får en ny CV med designet og fargene du har valgt. Den du har nå kan hentes tilbake med Angre. Med AI sendes teksten til tjenesten du har koblet til.",
        "You get a new CV with the design and colors you have chosen. The one you have now can be brought back with Undo. With AI, the text is sent to the service you have connected.",
    ),
    ("Velg fil", "Choose file"),
    ("Dette må plasseres", "This still needs a place"),
    (
        "Kontaktinfo, profil, kompetanse og språk er fylt inn. Jobber, utdanning og annet må du legge inn selv i skjemaet – kopier fra teksten under – eller la AI-en gjøre det.",
        "Contact details, profile, skills and languages are filled in. Add jobs, education and the rest yourself in the form – copy from the text below – or let the AI do it.",
    ),
    ("La AI-en legge det inn", "Let the AI add it"),
    ("Kopier teksten", "Copy the text"),
    (
        "Legg inn disse delene fra CV-en min (jobber med datoer og punkter, utdanning, kurs og annet). Bruk bare det som står her og behold alle tall:\n\n---\n{cut}",
        "Add these parts of my CV (jobs with dates and bullet points, education, courses and the rest). Use only what is here and keep all numbers:\n\n---\n{cut}",
    ),
    // ---- photo.rs, theme.rs ----
    ("Bildet kunne ikke åpnes: {e}", "The photo could not be opened: {e}"),
    ("Bildet er for stort. Velg et mindre bilde.", "The photo is too large. Choose a smaller one."),
    // ---- tooltips (every control explains itself on hover; see tooltips.rs) ----
    // menu_ui.rs
    ("Ny CV, åpne og lagre utkast, importer, last ned og innstillinger", "New CV, open and save drafts, import, download and settings"),
    ("Fil, stillingsannonse, nedlasting, angre og CV-ens språk", "File, job ad, download, undo and the CV’s language"),
    ("Legg til jobber, utdanning, kurs, kompetanse, språk og egne seksjoner", "Add jobs, education, courses, skills, languages and your own sections"),
    ("Marger, papirstørrelse, spalter, sideskift og tekststørrelse", "Margins, paper size, columns, page breaks and text size"),
    ("Velg utseende og farger, eller importer et eget design", "Choose the look and colors, or import your own design"),
    ("Velg hvilke visninger du ser, og zoom i forhåndsvisningen", "Choose which views you see, and zoom the preview"),
    ("Koble til AI, åpne chatten og se sjekklisten", "Connect AI, open the chat and see the checklist"),
    ("Om CVBuilder, format for egne design og innstillinger", "About CVBuilder, the format for your own designs, and settings"),
    (
        "Lagre CV-en som PDF med ekte, søkbar tekst – den formen søknadssystemer (ATS) leser sikrest.",
        "Save the CV as a PDF with real, searchable text – the form applicant tracking systems (ATS) read most reliably.",
    ),
    ("Lagre CV-en som Word-fil (.docx), for portaler og rekrutterere som ber om Word.", "Save the CV as a Word file (.docx), for portals and recruiters that ask for Word."),
    ("Ny utdanning: grad, fagbrev eller studium", "New education: a degree, a trade certificate or a course of study"),
    ("Nytt kurs eller ny sertifisering. Sertifiseringer i annonsen er ofte noe ATS ser etter.", "A new course or certification. Certifications named in the ad are often what an ATS looks for."),
    ("Ny seksjon «{name}». Du kan endre navnet før den legges til.", "New section “{name}”. You can change the name before it is added."),
    ("En seksjon med et navn du velger selv", "A section with a name you choose yourself"),
    ("Prosjekter, frivillig arbeid, verv, publikasjoner og andre seksjoner", "Projects, volunteering, positions of trust, publications and other sections"),
    ("Ny kompetanse eller nytt verktøy. Bruk annonsens ord – ATS ser etter dem.", "A new skill or tool. Use the ad’s words – an ATS looks for them."),
    ("Nytt språk med nivå, f.eks. engelsk – flytende", "A new language with its level, e.g. English – fluent"),
    ("Ny interesse. Høyst tre kommer med i CV-en.", "A new interest. At most three go into the CV."),
    ("Vis eller skjul skjemaet der du skriver CV-en", "Show or hide the form where you write the CV"),
    ("Vis eller skjul sidene slik de blir i PDF-en", "Show or hide the pages as they will look in the PDF"),
    ("Vis eller skjul panelet med AI-assistenten og sjekklisten", "Show or hide the panel with the AI assistant and the checklist"),
    ("Vis skjema, forhåndsvisning og AI-panelet igjen", "Show the form, the preview and the AI panel again"),
    ("Sidene fyller bredden av forhåndsvisningen", "The pages fill the width of the preview"),
    ("Siden i {zoom} av virkelig størrelse", "The page at {zoom} of its real size"),
    ("Be AI-assistenten om endringer i CV-en med egne ord", "Ask the AI assistant for changes to the CV in your own words"),
    ("Vent til AI-en er ferdig, eller stopp den i chatten.", "Wait until the AI is done, or stop it in the chat."),
    ("Start en ny samtale. CV-en endres ikke.", "Start a new conversation. The CV is not changed."),
    ("Kopier versjon, bygg og system, f.eks. til en feilmelding", "Copy the version, build and system, e.g. for a bug report"),
    ("Lukk vinduet (Esc)", "Close the window (Esc)"),
    // page_ui.rs
    ("Passer de fleste CV-er", "Suits most CVs"),
    ("Mer tekst per side, men siden blir tettere", "More text per page, but the page gets denser"),
    ("Litt mer luft oppe og nede", "A little more space at the top and bottom"),
    ("Mest luft – fint for en kort CV", "The most space – good for a short CV"),
    ("Velg hver marg selv, i millimeter", "Set each margin yourself, in millimeters"),
    ("Dra eller skriv inn margen, 5–40 mm", "Drag or type the margin, 5–40 mm"),
    ("Luften rundt teksten på siden. Normal passer de fleste.", "The space around the text on the page. Normal suits most."),
    ("Vanlig i Norge og Europa", "Standard in Norway and Europe"),
    ("Vanlig i USA og Canada", "Standard in the US and Canada"),
    ("Lengre amerikansk format, sjelden brukt til CV", "A longer American format, rarely used for CVs"),
    ("Papirstørrelsen: A4 eller amerikansk Letter og Legal", "The paper size: A4, or American Letter and Legal"),
    ("Et sidefelt med kontakt og kompetanse. Eldre ATS kan lese to spalter i feil rekkefølge.", "A sidebar with contact details and skills. Older ATS can read two columns in the wrong order."),
    ("Bruk spaltene designet har", "Use the columns the design has"),
    ("Leses riktig av alle søknadssystemer – anbefalt", "Read correctly by every applicant tracking system – recommended"),
    ("Én spalte leses best av ATS. To spalter gir et sidefelt.", "One column is read best by an ATS. Two columns give a sidebar."),
    ("«{title}» starter øverst på en ny side", "“{title}” starts at the top of a new page"),
    ("La seksjonene flyte fritt over sidene igjen", "Let the sections flow freely across the pages again"),
    ("Velg seksjoner som skal starte på en ny side, så ingen deles stygt", "Choose sections that start on a new page, so none is split awkwardly"),
    ("CVBuilder varsler når CV-en blir lengre enn dette", "CVBuilder warns you when the CV gets longer than this"),
    ("Hvor mange sider du sikter mot. Rekrutterere anbefaler høyst to for de fleste.", "How many pages you aim for. Recruiters recommend at most two for most people."),
    ("Teksten er så liten den kan bli", "The text is as small as it can be"),
    ("Teksten er så stor den kan bli", "The text is as large as it can be"),
    ("Mindre luft mellom linjer og deler – får plass til mer", "Less space between lines and parts – fits more"),
    ("Vanlig avstand mellom linjer og deler", "Normal space between lines and parts"),
    ("Mer luft – lettere å lese når CV-en er kort", "More space – easier to read when the CV is short"),
    // colors_ui.rs, designs_ui.rs
    ("Bruk paletten på hele CV-en. Den beholdes når du bytter design.", "Use the palette for the whole CV. It is kept when you change the design."),
    ("Fargen på overskrifter og detaljer som skiller seg ut", "The color of headings and details that stand out"),
    ("Fargen på vanlig tekst. Mørk tekst på lys bakgrunn leses best.", "The color of normal text. Dark text on a light background reads best."),
    ("Fargen på datoer, steder og annen hjelpetekst", "The color of dates, places and other supporting text"),
    ("Fargen på skillelinjene", "The color of the dividing lines"),
    ("Et farget felt bak navnet og kontaktinformasjonen øverst", "A colored band behind the name and contact details at the top"),
    ("Tekstfargen i fargefeltet. Sørg for god kontrast mot feltet.", "The text color in the colored band. Make sure it contrasts well with the band."),
    ("Et farget bånd bak hver seksjonsoverskrift", "A colored band behind each section heading"),
    ("Fjern alle egne farger, også seksjonenes", "Remove all your own colors, the sections’ too"),
    ("Farger for hele CV-en, uavhengig av designet", "Colors for the whole CV, independent of the design"),
    ("Bytter bare utseendet. Teksten i CV-en er den samme.", "Only changes the look. The text of the CV stays the same."),
    ("Utseendet på CV-en: skrift, farger og overskrifter. Teksten er lik i alle design.", "The look of the CV: font, colors and headings. The text is the same in every design."),
    ("Velg en .cvdesign.json-fil å importere", "Choose a .cvdesign.json file to import"),
    // form.rs
    ("Klikk for å skjule seksjonen i skjemaet. Den står fortsatt i CV-en.", "Click to hide the section in the form. It stays in the CV."),
    ("Klikk for å vise seksjonen", "Click to show the section"),
    ("Klikk for å skjule annonsefeltene", "Click to hide the job ad fields"),
    ("Klikk for å vise annonsefeltene", "Click to show the job ad fields"),
    ("Navnet ditt slik du vil bli tiltalt. Det står øverst i CV-en.", "Your name as you want to be addressed. It is at the top of the CV."),
    ("Bruk gjerne stillingstittelen fra annonsen – rekrutterere og ATS ser etter den.", "Use the job title from the ad if it fits – recruiters and ATS look for it."),
    ("Flytt opp i kontaktlinjen", "Move up in the contact line"),
    ("Flytt ned i kontaktlinjen", "Move down in the contact line"),
    ("E-posten rekrutterere svarer til. Bruk en nøktern adresse, gjerne med navnet ditt.", "The email recruiters reply to. Use a plain address, ideally with your name."),
    ("Mobilnummeret ditt, gjerne med landkode", "Your mobile number, ideally with the country code"),
    ("Hvor du bor. By og land holder – full adresse trengs ikke.", "Where you live. City and country are enough – no full address needed."),
    ("Lenken til LinkedIn-profilen din. Mange rekrutterere sjekker den.", "The link to your LinkedIn profile. Many recruiters check it."),
    ("Hva feltet heter, f.eks. GitHub, Portefølje eller Førerkort", "What the field is called, e.g. GitHub, Portfolio or Driving licence"),
    ("Lenken eller verdien, f.eks. github.com/navn eller Klasse B", "The link or the value, e.g. github.com/name or Class B"),
    ("Fjern kontaktfeltet", "Remove the contact field"),
    ("Tilbake til standard rekkefølge i kontaktlinjen", "Back to the standard order of the contact line"),
    ("Ta med bildet i CV-en, eller behold det her uten å vise det", "Put the photo in the CV, or keep it here without showing it"),
    ("Slett portrettet fra utkastet", "Delete the portrait from the draft"),
    ("Lim inn lenken til annonsen, så kan CVBuilder hente teksten", "Paste the link to the ad, and CVBuilder can fetch the text"),
    (
        "Hele annonsen. CVBuilder finner stilling, arbeidsgiver og fagord. Annonsen kommer ikke med i CV-en.",
        "The whole ad. CVBuilder finds the job title, the employer and the keywords. The ad does not go into the CV.",
    ),
    ("Stillingen du søker på, slik den står i annonsen", "The job you are applying for, as the ad puts it"),
    ("Virksomheten som har lyst ut stillingen", "The organization that advertised the job"),
    ("Ordene ATS og rekrutterere ser etter. Bruk dem i CV-en der de stemmer.", "The words ATS and recruiters look for. Use them in the CV where they are true."),
    ("AI-en leser annonsen og lagrer de viktigste fagordene, høyst 25", "The AI reads the ad and saves the most important keywords, at most 25"),
    ("Hvem du er, hva du kan og hva du søker. Det første rekrutterere leser.", "Who you are, what you can do and what you are looking for. The first thing recruiters read."),
    ("Gruppen den står under i CV-en, f.eks. Verktøy. Samme gruppe samles.", "The group it is listed under in the CV, e.g. Tools. Skills in the same group are kept together."),
    ("Én kompetanse eller ett verktøy. Bruk annonsens ord – ATS ser etter dem.", "One skill or tool. Use the ad’s words – an ATS looks for them."),
    ("Fjern kompetansen", "Remove the skill"),
    ("Tittelen du hadde. Bruk den vanlige betegnelsen, så ATS kjenner den igjen.", "The title you had. Use the common name, so an ATS recognizes it."),
    ("Firmaet eller virksomheten du jobbet for", "The company or organization you worked for"),
    ("Byen du jobbet i, eller «Hjemmekontor». Valgfritt.", "The city you worked in, or “Remote”. Optional."),
    ("Måned og år du begynte. Bare året går også.", "The month and year you started. Just the year works too."),
    ("Måned og år du sluttet, eller «nå» hvis du jobber der fortsatt", "The month and year you left, or “now” if you still work there"),
    ("Jobben pågår: CV-en viser «nå» i stedet for en sluttdato", "The job is ongoing: the CV shows “now” instead of an end date"),
    ("Ett resultat per linje. Start med et verb og ta med tall der du kan.", "One result per line. Start with a verb and include numbers where you can."),
    ("AI-en skriver om punktene så de viser handling og resultat, uten å finne på noe", "The AI rewrites the bullet points to show action and result, without making anything up"),
    ("F.eks. Bachelor i informatikk", "E.g. Bachelor of Computer Science"),
    ("Det offisielle navnet på graden, fagbrevet eller studiet", "The official name of the degree, trade certificate or course of study"),
    ("Skolen, høyskolen eller universitetet", "The school, college or university"),
    ("Året du begynte, f.eks. 2018", "The year you started, e.g. 2018"),
    ("Året du ble ferdig, eller ventes ferdig", "The year you finished, or expect to finish"),
    ("Navnet slik utstederen skriver det – ATS ser ofte etter nøyaktig det", "The name as the issuer writes it – an ATS often looks for exactly that"),
    ("Hvem som holdt kurset eller utstedte sertifiseringen", "Who ran the course or issued the certification"),
    ("Året du fullførte. Valgfritt.", "The year you completed it. Optional."),
    ("Fjern kurset", "Remove the course"),
    ("Språket, f.eks. Engelsk", "The language, e.g. English"),
    ("Hvor godt du kan det. Nivåer som B2 eller C1 er tydelige for rekrutterere.", "How well you know it. Levels like B2 or C1 are clear to recruiters."),
    ("Fjern språket", "Remove the language"),
    ("Interesser er valgfrie. Slå av for å holde dem utenfor CV-en.", "Interests are optional. Turn this off to keep them out of the CV."),
    ("Du har valgt tre. Fjern et kryss for å velge denne.", "You have chosen three. Clear one to choose this one."),
    ("Interesse", "Interest"),
    ("Kort og konkret, f.eks. Maratonløping eller Korps", "Short and concrete, e.g. Marathon running or Brass band"),
    ("Noe du driver med nå", "Something you do now"),
    ("Noe du har drevet med før", "Something you used to do"),
    ("Noe du er interessert i, men ikke driver med", "Something you are interested in, but do not do"),
    ("Om du driver med det nå, gjorde det før, eller er interessert i det", "Whether you do it now, did it before, or are interested in it"),
    ("Fjern interessen", "Remove the interest"),
    ("Overskriften seksjonen får i CV-en, f.eks. Prosjekter eller Frivillig arbeid", "The heading the section gets in the CV, e.g. Projects or Volunteering"),
    ("Navnet på prosjektet, vervet eller det du vil vise", "The name of the project, the position or what you want to show"),
    ("Hvor eller for hvem, f.eks. firma, forening eller by", "Where or for whom, e.g. a company, a club or a city"),
    ("Når, skrevet som du vil, f.eks. 2023 eller 2021–2023", "When, written as you like, e.g. 2023 or 2021–2023"),
    ("Hva du gjorde og hva det ga, ett punkt per linje", "What you did and what it achieved, one point per line"),
    ("Ny oppføring i denne seksjonen", "A new entry in this section"),
    // chat_ui.rs, ai_ui.rs
    ("Bruk AI-tjenesten du har koblet til med API-nøkkel", "Use the AI service you connected with an API key"),
    ("Bruk Claude Code på denne maskinen, med kontoen du er logget inn med", "Use Claude Code on this computer, with the account you are signed in with"),
    ("Velg en AI-tjeneste og legg inn API-nøkkelen din", "Choose an AI service and enter your API key"),
    ("Hvilken AI du snakker med. Du kan bytte når som helst.", "Which AI you are talking to. You can switch at any time."),
    ("Send dette til AI-en. Du ser endringene og kan angre dem.", "Send this to the AI. You see the changes and can undo them."),
    ("Skriv med egne ord hva du vil endre. Enter sender.", "Write in your own words what you want to change. Enter sends."),
    ("Stopp AI-en. Endringer den alt har gjort, blir værende og kan angres.", "Stop the AI. Changes it has already made stay and can be undone."),
    ("Send meldingen (Enter)", "Send the message (Enter)"),
    ("Skriv en melding først", "Write a message first"),
    ("Kopier til utklippstavlen", "Copy to the clipboard"),
    ("Sett CV-en tilbake til slik den var før disse endringene", "Put the CV back the way it was before these changes"),
    ("En modell på din egen maskin, f.eks. med Ollama. Ingen nøkkel og ingen kostnad.", "A model on your own computer, e.g. with Ollama. No key and no cost."),
    ("Bruk {service} med din egen API-nøkkel fra {company}", "Use {service} with your own API key from {company}"),
    ("Åpner siden i nettleseren, der du lager nøkkelen", "Opens the page in your browser, where you create the key"),
    ("Lim inn nøkkelen. Den lagres i maskinens nøkkelring, aldri i CV-filen.", "Paste the key. It is stored in the computer’s keyring, never in the CV file."),
    ("Bekreft at du har lest hva som sendes, og hvem som tar betalt", "Confirm that you have read what is sent, and who charges for it"),
    ("Prøver nøkkelen og henter modellene du kan velge", "Tries the key and fetches the models you can choose"),
    ("Lim inn nøkkelen og kryss av først", "Paste the key and tick the box first"),
    ("Slå av AI og slett nøkkelen fra denne maskinen", "Turn off AI and delete the key from this computer"),
    ("Bruk denne modellen i chatten", "Use this model in the chat"),
    ("Modellen chatten bruker. Større modeller svarer ofte bedre, men tregere.", "The model the chat uses. Larger models often answer better, but more slowly."),
    ("Åpner nedlastingssiden til Ollama i nettleseren", "Opens Ollama’s download page in your browser"),
    ("Adressen til den lokale serveren. Ollama bruker http://localhost:11434/v1.", "The address of the local server. Ollama uses http://localhost:11434/v1."),
    ("Bruk standardadressen til {name}: {url}", "Use the standard address of {name}: {url}"),
    ("Ser etter Ollama, LM Studio og andre på de vanlige adressene", "Looks for Ollama, LM Studio and others at the usual addresses"),
    ("Bare hvis serveren krever nøkkel. Ollama og LM Studio gjør det ikke.", "Only if the server requires a key. Ollama and LM Studio do not."),
    ("Kobler til serveren og henter modellene den har", "Connects to the server and fetches the models it has"),
    ("Skriv en adresse som begynner med http", "Enter an address that starts with http"),
    ("Slå av den lokale modellen i CVBuilder", "Turn off the local model in CVBuilder"),
    // import_ui.rs, settings_ui.rs, lib.rs
    ("AI-en leser hele CV-en og fyller ut alle delene. Teksten sendes til AI-tjenesten.", "The AI reads the whole CV and fills in every part. The text is sent to the AI service."),
    ("Leses på denne maskinen. Jobber og utdanning legger du inn selv.", "Read on this computer. You add jobs and education yourself."),
    ("Velg CV-filen du vil hente inn", "Choose the CV file you want to bring in"),
    ("Lukk uten å endre noe (Esc)", "Close without changing anything (Esc)"),
    ("AI-en legger jobbene, utdanningen og resten inn i skjemaet", "The AI puts the jobs, the education and the rest into the form"),
    ("Kopier teksten, så du kan lime den inn i skjemaet selv", "Copy the text, so you can paste it into the form yourself"),
    ("Samme språk som datamaskinen: norsk hvis den er på norsk, ellers engelsk", "The same language as the computer: Norwegian if it is set to Norwegian, otherwise English"),
    ("Menyer og knapper på norsk bokmål", "Menus and buttons in Norwegian Bokmål"),
    ("Menyer og knapper på engelsk", "Menus and buttons in English"),
    ("Lyst eller mørkt, slik datamaskinen er innstilt", "Light or dark, as the computer is set"),
    ("Lys bakgrunn", "Light background"),
    ("Mørk bakgrunn, skånsomt for øynene om kvelden", "Dark background, easier on the eyes in the evening"),
    ("Utkastet lagres her mens du skriver. Slå av på en delt maskin.", "The draft is saved here as you type. Turn this off on a shared computer."),
    ("Lukk vinduet. Innstillingene er alt lagret.", "Close the window. The settings are already saved."),
    ("Legg seksjonen til i CV-en og skjemaet (Enter)", "Add the section to the CV and the form (Enter)"),
    ("Be AI-en om endringer i CV-en med egne ord", "Ask the AI for changes to the CV in your own words"),
    ("Hva som mangler før nedlasting, og råd fra rekrutterere og ATS", "What is missing before download, and advice from recruiters and ATS"),
    ("AI-en spør hvilke av fagordene du faktisk kan stå for, før noe legges inn", "The AI asks which of the keywords you can honestly stand behind before anything is added"),
    ("AI-en oversetter all teksten. Du ser endringene og kan angre dem.", "The AI translates all the text. You see the changes and can undo them."),
    (
        "Overskrifter og datoer byttes nå. Teksten din oversetter du selv, med originalen under hvert felt.",
        "Headings and dates switch now. You translate your own text, with the original under each field.",
    ),
    ("Du bekrefter at alt i CV-en stemmer, og blir ikke spurt igjen for denne CV-en", "You confirm that everything in the CV is correct, and are not asked again for this CV"),
    ("Gå tilbake uten å laste ned (Esc)", "Go back without downloading (Esc)"),
    ("Forstørr for å beskjære tettere rundt ansiktet", "Zoom in to crop closer around the face"),
    ("Flytt utsnittet mot venstre eller høyre", "Move the crop left or right"),
    ("Flytt utsnittet opp eller ned", "Move the crop up or down"),
    ("Drei bildet litt hvis det heller", "Rotate the photo a little if it is tilted"),
    ("Gjør bildet lysere eller mørkere", "Make the photo lighter or darker"),
    ("Mer eller mindre forskjell mellom lyst og mørkt", "More or less difference between light and dark"),
    ("Tilbake til utsnittet og justeringene fra start", "Back to the original crop and adjustments"),
    ("Legg portrettet inn i CV-en. Originalfilen endres ikke.", "Put the portrait into the CV. The original file is not changed."),
    ("Lukk uten å endre portrettet (Esc)", "Close without changing the portrait (Esc)"),
    // ---- langpack_ui.rs: added interface languages ----
    (
        "Samme språk som datamaskinen: norsk hvis den er på norsk, et tilleggsspråk hvis du har lagt til datamaskinens språk, ellers engelsk",
        "The same language as the computer: Norwegian if it is in Norwegian, an added language if you have added the computer's language, otherwise English",
    ),
    ("Menyer og knapper på {name} ({code}). Laget av {who}.", "Menus and buttons in {name} ({code}). Made by {who}."),
    (
        "Meldinger med tall og navn i, for eksempel fra sjekklisten, vises på engelsk i tilleggsspråk.",
        "Messages with numbers and names in them, for example from the checklist, are shown in English in added languages.",
    ),
    ("Oversetter {done} av {total} …", "Translating {done} of {total} …"),
    ("Språk: {name}", "Language: {name}"),
    ("Stopper etter denne delen …", "Stopping after this part …"),
    ("Stopp oversettelsen. Det som er oversatt, beholdes.", "Stop the translation. What has been translated is kept."),
    ("Legg til språk med AI …", "Add a language with AI …"),
    ("Velg eller skriv et språk, så oversetter AI-en alle tekstene i CVBuilder til det", "Choose or type a language, and the AI translates all of CVBuilder's text into it"),
    ("Vent til oversettelsen er ferdig", "Wait until the translation is finished"),
    ("Lag på nytt", "Make again"),
    ("La AI-en oversette det valgte språket på nytt. Det gamle beholdes til hver del er ferdig.", "Let the AI translate the selected language again. The old text is kept until each part is done."),
    ("Velg et tilleggsspråk først, og vent til en oversettelse er ferdig", "Choose an added language first, and wait until any translation is finished"),
    ("Rediger …", "Edit …"),
    ("Rett tekstene i det valgte språket selv", "Correct the text of the selected language yourself"),
    ("Eksporter språket …", "Export the language …"),
    ("Lagre det valgte språket som fil, for å dele det eller rette det i et annet program", "Save the selected language as a file, to share it or edit it in another program"),
    ("Velg et tilleggsspråk først", "Choose an added language first"),
    ("Eksporter mal for oversettelse …", "Export a template for translation …"),
    ("Lagre en fil med alle tekstene på engelsk, som du kan oversette selv og importere", "Save a file with all the text in English, which you can translate yourself and import"),
    ("Importer språkfil …", "Import language file …"),
    ("Legg til et språk fra en .cvlang.json-fil", "Add a language from a .cvlang.json file"),
    ("Slett språk", "Delete language"),
    ("Fjern det valgte språket fra denne maskinen", "Remove the selected language from this computer"),
    ("Slette «{name}» fra denne maskinen? Det kan ikke angres, men du kan eksportere det først.", "Delete “{name}” from this computer? This cannot be undone, but you can export it first."),
    ("Slett", "Delete"),
    ("Slett språkfilen. Menyene går tilbake til «Følg systemet».", "Delete the language file. The menus go back to “Follow the system”."),
    ("Koble til en AI først, eller eksporter malen og oversett den selv.", "Connect an AI first, or export the template and translate it yourself."),
    ("Oversettelsen stoppet uventet.", "The translation stopped unexpectedly."),
    ("AI-en svarte ikke med JSON.", "The AI did not answer with JSON."),
    ("Stoppet. {n} tekster ble oversatt til {name} og er lagret.", "Stopped. {n} texts were translated into {name} and have been saved."),
    ("{name} er klart: {n} tekster ble oversatt.", "{name} is ready: {n} texts were translated."),
    ("{n} tekster ble ikke oversatt og vises som før, eller på engelsk.", "{n} texts were not translated and are shown as before, or in English."),
    ("Kunne ikke lagre språkfilen: {e}", "Could not save the language file: {e}"),
    ("Oversettelsen til {name} feilet: {e}", "The translation into {name} failed: {e}"),
    ("Legg til språk", "Add language"),
    (
        "AI-en oversetter alle de {n} tekstene i CVBuilder til språket du velger. Det tar noen minutter, og du kan bruke programmet imens.",
        "The AI translates all {n} texts in CVBuilder into the language you choose. It takes a few minutes, and you can use the program in the meantime.",
    ),
    ("Velg et språk …", "Choose a language …"),
    ("Bruk dette språket", "Use this language"),
    ("Velg et språk fra listen, eller skriv navnet og koden under", "Choose a language from the list, or type its name and code below"),
    ("Navn på språket, slik det skrives på språket selv", "The language's name, as it is written in the language itself"),
    ("Navnet vises i listen over språk, for eksempel Deutsch eller Tiếng Việt", "The name is shown in the list of languages, for example Deutsch or Tiếng Việt"),
    ("Språkkode", "Language code"),
    (
        "En kort kode for språket (ISO 639), for eksempel de, pl, es, vi eller pt-BR. Den brukes også til å velge språket automatisk.",
        "A short code for the language (ISO 639), for example de, pl, es, vi or pt-BR. It is also used to choose the language automatically.",
    ),
    (
        "CVBuilder har skrift for de fleste språk, også japansk, kinesisk og koreansk. Språk som skrives fra høyre mot venstre, som arabisk og hebraisk, vises ennå ikke riktig i menyene.",
        "CVBuilder has fonts for most languages, including Japanese, Chinese and Korean. Languages written right to left, such as Arabic and Hebrew, are not yet shown correctly in the menus.",
    ),
    ("Oversettes med {name}", "Translated with {name}"),
    ("Uten AI: eksporter malen under Innstillinger, oversett den selv og importer den.", "Without AI: export the template under Settings, translate it yourself and import it."),
    ("Språket finnes allerede. Det oversettes på nytt og erstattes del for del.", "The language already exists. It is translated again and replaced part by part."),
    ("Start oversettelsen. Du ser framdriften under Innstillinger og nederst i vinduet.", "Start the translation. You see the progress under Settings and at the bottom of the window."),
    ("Rediger {name}", "Edit {name}"),
    (
        "Engelsk til venstre, oversettelsen til høyre. Et tomt felt vises på engelsk. Behold plassholdere i krøllparenteser som de står.",
        "English on the left, the translation on the right. An empty field is shown in English. Keep placeholders in curly braces as they are.",
    ),
    ("Søk …", "Search …"),
    ("Vis bare tekster som inneholder dette, på engelsk, norsk eller i oversettelsen", "Show only texts that contain this, in English, Norwegian or the translation"),
    ("Oversettelsen av teksten til venstre. Rød tekst: plassholderne stemmer ikke.", "The translation of the text on the left. Red text: the placeholders do not match."),
    ("Lagre språket og bruk endringene med en gang", "Save the language and use the changes straight away"),
    ("rettet for hånd", "edited by hand"),
    ("{name} er lagret.", "{name} has been saved."),
    ("{n} tekster hadde feil plassholdere og vises på engelsk.", "{n} texts had wrong placeholders and are shown in English."),
    ("{name} er slettet.", "{name} has been deleted."),
    ("Kunne ikke slette språkfilen: {e}", "Could not delete the language file: {e}"),
    ("CVBuilder-språkfil", "CVBuilder language file"),
    (
        "Malen er lagret: {path}. Oversett verdiene, fyll inn «code» og «name», og importer filen.",
        "The template is saved: {path}. Translate the values, fill in “code” and “name”, and import the file.",
    ),
    ("Språket er lagret: {path}", "The language is saved: {path}"),
    ("Kunne ikke lese filen: {e}", "Could not read the file: {e}"),
    ("{name} er importert med {n} tekster.", "{name} has been imported with {n} texts."),
    ("{n} tekster var tomme eller hadde feil plassholdere og vises på engelsk.", "{n} texts were empty or had wrong placeholders and are shown in English."),
    ("{n} tekster finnes ikke i denne versjonen av CVBuilder og ble hoppet over.", "{n} texts do not exist in this version of CVBuilder and were skipped."),
    ("Filen er ikke gyldig JSON: {e}", "The file is not valid JSON: {e}"),
    ("Filen er ikke en CVBuilder-språkfil (feltet «format» må være «{format}»).", "The file is not a CVBuilder language file (the “format” field must be “{format}”)."),
    ("Fyll inn språkets navn i feltet «name».", "Fill in the language's name in the “name” field."),
    ("Fyll inn språkkoden i feltet «code», for eksempel de eller pt-BR.", "Fill in the language code in the “code” field, for example de or pt-BR."),
    (
        "«{code}» er ikke en gyldig språkkode. Bruk 2–8 små bokstaver, gjerne med region, for eksempel de eller pt-BR.",
        "“{code}” is not a valid language code. Use 2–8 lowercase letters, optionally with a region, for example de or pt-BR.",
    ),
    ("Språkkoden «{code}» brukes av de innebygde valgene. Velg en annen.", "The language code “{code}” is used by the built-in choices. Choose another one."),
];

/// Fixed text that cv-core, cv-ai and cv-mcp produce, with its English.
static DYN: &[(&str, &str)] = &[
    ("Navn", "Name"),
    ("Portrett", "Portrait"),
    ("Stillingen du søker", "The job you are applying for"),
    ("En sjekkliste kan ha høyst 100 punkter.", "A checklist can have at most 100 points."),
    ("Gi sjekklisten et navn.", "Give the checklist a name."),
    ("Filen er for stor til å være en CVBuilder-sjekkliste.", "The file is too big to be a CVBuilder checklist."),
    ("Filen er ikke en CVBuilder-sjekkliste: den er ikke tekst i UTF-8.", "The file is not a CVBuilder checklist: it is not UTF-8 text."),
    // ---- checks::ready (what blocks the download) ----
    ("Faglig overskrift", "Professional headline"),
    ("E-postadressen ser ikke riktig ut.", "The email address does not look right."),
    ("Telefonnummeret ser ikke riktig ut. Skriv for eksempel +47 412 34 567.", "The phone number does not look right. Write it like +47 412 34 567."),
    ("Skriv hvilken stilling du søker (under «Stillingen»).", "Enter the job you are applying for (under “Job title”)."),
    ("Annonselenken må starte med https://.", "The ad link must start with https://."),
    ("LinkedIn må være full adresse, for eksempel https://www.linkedin.com/in/navn.", "LinkedIn must be a full address, for example https://www.linkedin.com/in/name."),
    ("Et kurs mangler navn.", "A course has no name."),
    ("Profilen er for lang. Kort den ned til høyst 90 ord, helst 30–70.", "The profile is too long. Cut it to 90 words at most, ideally 30–70."),
    ("Legg til minst én jobb eller utdanning.", "Add at least one job or education."),
    ("fyll inn arbeidsgiver og stillingstittel.", "fill in the employer and job title."),
    ("fyll inn når du startet (for eksempel 09.2021).", "fill in when you started (for example 09.2021)."),
    ("fyll inn når du sluttet, eller kryss av for «Nåværende jobb».", "fill in when you left, or tick “Current job”."),
    ("sluttdatoen er før startdatoen.", "the end date is before the start date."),
    ("skriv minst ett punkt om hva du gjorde.", "write at least one bullet point about what you did."),
    ("fyll inn utdanning og lærested.", "fill in the education and the school."),
    ("fyll inn sluttår (forventet år hvis du fortsatt studerer).", "fill in the end year (the expected year if you are still studying)."),
    ("sluttåret er før startåret.", "the end year is before the start year."),
    ("Et språk mangler navn eller nivå.", "A language is missing its name or level."),
    ("En interesse mangler navn.", "An interest has no name."),
    (
        "Teksten inneholder plassholdere som [tall] eller [fyll inn], eller kode som <…>. Fyll inn ekte tall og tekst, eller fjern dem.",
        "The text contains placeholders such as [number] or [insert], or code such as <…>. Fill in real numbers and text, or remove them.",
    ),
    ("Bildevalget er på, men det er ikke lagt inn noe bilde.", "Show photo is on, but no photo has been added."),
    // ---- checks::audit (the recruiter checklist) ----
    ("Kontaktinformasjon står som vanlig tekst øverst", "Contact details are plain text at the top"),
    ("Kontaktinformasjon mangler", "Contact details are missing"),
    (
        "Fyll inn navn, e-post, telefon og by. Det står som vanlig tekst øverst i CV-en, ikke i topptekst eller bilde.",
        "Fill in name, email, phone and city. They appear as plain text at the top of the CV, not in a page header or an image.",
    ),
    ("Telefonnummeret har landskode", "The phone number has a country code"),
    ("Telefonnummeret mangler landskode", "The phone number has no country code"),
    (
        "Skriv for eksempel +47 412 34 567. Utenlandske arbeidsgivere og søknadssystemer trenger landskoden.",
        "Write it like +47 412 34 567. Employers and applicant tracking systems abroad need the country code.",
    ),
    ("LinkedIn-profil er oppgitt", "LinkedIn profile is included"),
    ("LinkedIn-profil mangler", "LinkedIn profile is missing"),
    (
        "Rekrutterere sjekker nesten alltid LinkedIn. Legg inn full adresse, og hold profilen lik CV-en.",
        "Recruiters almost always check LinkedIn. Add the full address, and keep the profile in line with the CV.",
    ),
    ("Faglig overskrift mangler", "Professional headline is missing"),
    (
        "Skriv en kort linje under navnet, gjerne med stillingens egne ord: for eksempel «Sykepleier | Akuttmottak | Legemiddelhåndtering».",
        "Write a short line under your name, ideally in the job's own words: for example “Nurse | Emergency department | Medication management”.",
    ),
    ("Overskriften speiler stillingen", "The headline mirrors the job"),
    ("Overskriften bruker ikke stillingens ord", "The headline does not use the job's words"),
    (
        "La overskriften bruke stillingens tittel eller to–tre fagord fra annonsen, uten å late som du har tittelen fra før.",
        "Let the headline use the job title or two or three keywords from the ad, without pretending you already hold that title.",
    ),
    ("Profiltekst mangler", "Profile text is missing"),
    (
        "Sikt mot 30–70 ord: hvem du er faglig, hva du kan best, og hva du vil bidra med i denne jobben.",
        "Aim for 30–70 words: who you are professionally, what you do best, and what you want to contribute in this job.",
    ),
    ("Ingen fagord fra annonsen er lagt inn", "No keywords from the ad have been added"),
    (
        "Lim inn annonsen øverst i skjemaet og trykk «Fyll ut fra annonsen». Søknadssystemer og rekrutterere leter etter annonsens egne ord.",
        "Paste the ad at the top of the form and press “Fill in from the ad”. Application systems and recruiters look for the ad's own words.",
    ),
    ("Ingen kompetanser lagt inn", "No skills added"),
    ("En kort, gruppert kompetanseliste tidlig i CV-en gjør fagordene lette å finne. Sikt mot 6–20.", "A short, grouped list of skills early in the CV makes the keywords easy to find. Aim for 6–20."),
    ("Kompetansene er gruppert", "The skills are grouped"),
    ("Noen kompetanser mangler gruppe", "Some skills have no group"),
    (
        "Grupper dem, for eksempel «Fag», «Verktøy», «Språk og sertifikater». Grupperte linjer leses raskere.",
        "Group them, for example “Professional”, “Tools”, “Languages and certificates”. Grouped lines are quicker to read.",
    ),
    ("Punktene er korte nok til å skannes", "The bullet points are short enough to scan"),
    ("Punktene starter med handling", "The bullet points start with an action"),
    ("Variert språk i punktene", "Varied language in the bullet points"),
    ("Mange punkter starter med samme ord", "Many bullet points start with the same word"),
    (
        "Rekrutterere legger merke til resultater: «Oppnådde X, målt ved Y, ved å gjøre Z». Bruk bare tall du kan dokumentere.",
        "Recruiters notice results: “Achieved X, measured by Y, by doing Z”. Only use numbers you can document.",
    ),
    ("Ingen tomme moteord", "No empty buzzwords"),
    ("Moteord som ikke beviser noe", "Buzzwords that prove nothing"),
    ("Ingen unødvendige personopplysninger", "No unnecessary personal data"),
    ("Personopplysninger som ikke hører hjemme i en CV", "Personal data that does not belong in a CV"),
    ("Fjern fødselsdato, personnummer, sivilstand, religion og nasjonalitet.", "Remove date of birth, national ID number, marital status, religion and nationality."),
    ("Ingen uforklarte opphold over seks måneder", "No unexplained gaps of more than six months"),
    ("Opphold over seks måneder i tidslinjen", "Gaps of more than six months in the timeline"),
    ("Arbeidssted er oppgitt for jobbene", "A location is given for the jobs"),
    ("Arbeidssted mangler for noen jobber", "Some jobs have no location"),
    ("Oppgi by for hver jobb. Mange søknadssystemer sorterer på sted.", "Give a city for each job. Many applicant tracking systems sort by location."),
    ("Kort ned eller velg utformingen Kompakt; legg prosjektliste i eget vedlegg.", "Shorten it or choose the Kompakt design; put a project list in a separate attachment."),
    ("Én tekstspalte, vanlige overskrifter og ekte tekst", "One text column, standard headings and real text"),
    // ---- layouts: categories, warnings, palettes ----
    ("Formell · jus, finans, offentlig sektor, ledelse", "Formal · law, finance, public sector, management"),
    ("Teknisk · IT, ingeniør, industri", "Technical · IT, engineering, industry"),
    ("Næringsliv · salg, konsulent, prosjekt", "Business · sales, consulting, projects"),
    ("Kreativ · design, media, markedsføring", "Creative · design, media, marketing"),
    ("Akademisk · forskning, undervisning, helse", "Academic · research, teaching, health"),
    ("Kompakt · lang erfaring, mye innhold", "Compact · long experience, lots of content"),
    ("Første jobb · studenter og nyutdannede", "First job · students and recent graduates"),
    ("Andre", "Other"),
    (
        "Designet har to spalter. Det ser bra ut, men eldre søknadssystemer (ATS) kan lese spaltene i feil rekkefølge. Bruk et design med én spalte når du søker gjennom store portaler.",
        "This design has two columns. It looks good, but older applicant tracking systems (ATS) may read the columns in the wrong order. Use a one-column design when you apply through large portals.",
    ),
    ("Brødteksten har for svak farge til å leses godt. Velg en mørkere tekstfarge.", "The body text color is too light to read well. Choose a darker text color."),
    ("Datoer og kontaktlinjen har for svak farge. Velg en mørkere farge for detaljer.", "The dates and the contact line are too light. Choose a darker color for details."),
    (
        "En eller flere overskriftsfarger er for lyse. Velg mørkere farger, så overskriftene leses godt.",
        "One or more heading colors are too light. Choose darker colors so the headings are easy to read.",
    ),
    ("Teksten i fargefeltet øverst leses dårlig mot bakgrunnen.", "The text in the color band at the top is hard to read against its background."),
    ("Designets egne", "The design's own"),
    ("Marine", "Navy"),
    ("Petrol", "Petrol"),
    ("Skog", "Forest"),
    ("Vin", "Wine"),
    ("Plomme", "Plum"),
    ("Terrakotta", "Terracotta"),
    ("Kobolt", "Cobalt"),
    ("Grafitt", "Graphite"),
    ("Sort og hvitt", "Black and white"),
    // ---- layouts: design validation ----
    ("«id» må ha 2–40 tegn: små bokstaver a–z, tall og bindestrek.", "“id” must have 2–40 characters: lowercase letters a–z, digits and hyphens."),
    ("«name» mangler.", "“name” is missing."),
    ("«columns.text» er for svak mot «columns.fill» (kontrast under 4,5:1).", "“columns.text” is too weak against “columns.fill” (contrast below 4.5:1)."),
    ("«columns.heading» er for svak mot «columns.fill» (kontrast under 3:1).", "“columns.heading” is too weak against “columns.fill” (contrast below 3:1)."),
    ("«header_fill» og «header_text» må settes sammen (eller begge være null).", "“header_fill” and “header_text” must be set together (or both be null)."),
    ("«heading_style» «band» trenger «heading_fill».", "“heading_style” “band” needs “heading_fill”."),
    ("«ink» er for lys til brødtekst (kontrast under 4,5:1 mot hvitt).", "“ink” is too light for body text (contrast below 4.5:1 against white)."),
    ("«muted» er for lys (kontrast under 3:1 mot hvitt).", "“muted” is too light (contrast below 3:1 against white)."),
    ("«header_text» er for svak mot «header_fill» (kontrast under 4,5:1).", "“header_text” is too weak against “header_fill” (contrast below 4.5:1)."),
    ("«accent» er for svak for overskrifter (kontrast under 3:1).", "“accent” is too weak for headings (contrast below 3:1)."),
    // ---- layouts: the four built-in designs ----
    ("Rolig og tidløs. Passer de fleste bransjer.", "Calm and timeless. Suits most industries."),
    ("Moderne skrift og tydelig fargeaksent. Teknologi og konsulent.", "Modern font and a clear color accent. Technology and consulting."),
    ("Serif i navn og overskrifter, dempede farger. Rådgivning, offentlig sektor, ledelse.", "Serif name and headings, muted colors. Advisory work, public sector, management."),
    ("Tettere og nøktern. Når lang erfaring må få plass på to sider.", "Tighter and understated. For when long experience must fit on two pages."),
    // ---- designs.json ----
    (
        "Luftig og vennlig med sentrert topp og petrolgrønne overskrifter uten streker, fin for første jobb og designroller.",
        "Airy and friendly with a centered header and petrol-green headings without rules, good for a first job and design roles.",
    ),
    (
        "Luftig og vennlig med smal venstrespalte for ferdigheter og utdanning, fin for første jobb og designroller.",
        "Airy and friendly with a narrow left column for skills and education, good for a first job and design roles.",
    ),
    (
        "Saklig og strukturert med marineblå strek over hver seksjon, passer utviklere og ingeniører.",
        "Matter-of-fact and structured with a navy rule above each section, suits developers and engineers.",
    ),
    (
        "Plommefarget serif i overskriftene med strek under, varm og stilsikker for merkevare, markedsføring og kommunikasjon.",
        "Plum serif headings with a rule below, warm and stylish for branding, marketing and communications.",
    ),
    (
        "Plommefarget høyrespalte med hvit tekst og serif-overskrifter, stilsikker for merkevare og kommunikasjon.",
        "Plum right column with white text and serif headings, stylish for branding and communications.",
    ),
    (
        "Fiolett toppfelt med hvit tekst og fiolette overskrifter med strek under, moderne for dataanalytikere og utviklere.",
        "Violet header band with white text and violet headings with a rule below, modern for data analysts and developers.",
    ),
    (
        "Lys lilla venstrespalte med navn og kontakt i et fiolett felt øverst, moderne for dataanalytikere og utviklere.",
        "Light purple left column with name and contact details in a violet band at the top, modern for data analysts and developers.",
    ),
    (
        "Teglrødt toppfelt og Baskerville-aktige overskrifter uten streker, passer arkitekter, designere og kreative fag.",
        "Brick-red header band and Baskerville-style headings without rules, suits architects, designers and creative fields.",
    ),
    (
        "Teglrødt toppfelt over to spalter med ferdigheter og utdanning til venstre, passer arkitekter og kreative fag.",
        "Brick-red header band over two columns with skills and education on the left, suits architects and creative fields.",
    ),
    (
        "Bringebærrødt toppfelt og overskrifter med strek under, energisk for markedsføring, salg og vekstroller.",
        "Raspberry-red header band and headings with a rule below, energetic for marketing, sales and growth roles.",
    ),
    (
        "Rosa venstrespalte med navn og kontakt i et bringebærrødt felt øverst, energisk for markedsføring og vekstroller.",
        "Pink left column with name and contact details in a raspberry-red band at the top, energetic for marketing and growth roles.",
    ),
    (
        "Salviegrønn topp og grønne serif-overskrifter med strek under, rolig og troverdig for prosjektledere og miljøroller.",
        "Sage-green header and green serif headings with a rule below, calm and credible for project managers and environmental roles.",
    ),
    (
        "Salviegrønn venstrespalte med navn og kontakt øverst og grønne serif-overskrifter, rolig for prosjektledere.",
        "Sage-green left column with name and contact details at the top and green serif headings, calm for project managers.",
    ),
    (
        "Tett og nøktern med sentrert topp og okerbrune slab-serif-overskrifter med strek under, rommer mye erfaring.",
        "Tight and understated with a centered header and ochre slab-serif headings with a rule below, fits a lot of experience.",
    ),
    (
        "Mørke serif-overskrifter og tynne grå linjer som rammer inn seksjonene, sobert for økonomi og finans.",
        "Dark serif headings and thin grey lines framing the sections, sober for accounting and finance.",
    ),
    (
        "Lyseblått toppfelt og stålblå overskrifter med strek under, ryddig og imøtekommende for kunde- og salgsroller.",
        "Light-blue header band and steel-blue headings with a rule below, tidy and approachable for customer and sales roles.",
    ),
    (
        "Lyseblått toppfelt over to spalter med ferdigheter og utdanning til høyre, ryddig for kunde- og salgsroller.",
        "Light-blue header band over two columns with skills and education on the right, tidy for customer and sales roles.",
    ),
    (
        "Kobberbrune serif-overskrifter i versaler med strek under, tett og seriøst for HR, ledelse og offentlig sektor.",
        "Copper-brown serif headings in capitals with a rule below, tight and serious for HR, management and the public sector.",
    ),
    (
        "Stram og nøktern med mørke serif-overskrifter og én blågrå linje under toppen, passer forskning og helse.",
        "Strict and understated with dark serif headings and one blue-grey line under the header, suits research and health care.",
    ),
    (
        "Navn i sperrede versaler og små overskrifter i versaler med svart strek under, tett og effektivt for lange karrierer.",
        "Name in spaced capitals and small capital headings with a black rule below, tight and efficient for long careers.",
    ),
    (
        "Rosarødt toppfelt med hvit tekst og serif-overskrifter med strek under, vennlig for første jobb, læring og undervisning.",
        "Rose-red header band with white text and serif headings with a rule below, friendly for a first job, learning and teaching.",
    ),
    (
        "Smal venstrespalte og et rosarødt navnefelt over hovedspalten, vennlig for første jobb og læringsroller.",
        "Narrow left column and a rose-red name band above the main column, friendly for a first job and learning roles.",
    ),
    (
        "Overskrifter i sperrede versaler på brunrøde felt og rammede seksjoner, tydelig og elegant for service og reiseliv.",
        "Headings in spaced capitals on russet bands and framed sections, clear and elegant for service and tourism.",
    ),
    (
        "Stålblå overskrifter med strek under og rent sans-serif-oppsett, solid for ingeniører og tekniske fag.",
        "Steel-blue headings with a rule below and a clean sans-serif layout, solid for engineers and technical fields.",
    ),
    (
        "Minimalistisk gråtone med overskrifter i versaler under en lys grå linje, passer IT-sikkerhet og teknologi.",
        "Minimalist greyscale with capital headings under a light grey line, suits IT security and technology.",
    ),
    (
        "Stort serif-navn med grønn tittel og luftige, strekfrie overskrifter, redaksjonelt for innholds- og tekstroller.",
        "Large serif name with a green title and airy headings without rules, editorial for content and writing roles.",
    ),
    (
        "Ren og saklig med blått navn og blå streker ved overskriftene, en trygg standard for de fleste.",
        "Clean and matter-of-fact with a blue name and blue rules by the headings, a safe default for most people.",
    ),
    (
        "Varm teglrød med serif-navn og overskrifter i kapitéler, passer humanistiske og kreative fag.",
        "Warm brick red with a serif name and small-caps headings, suits the humanities and creative fields.",
    ),
    (
        "Venstrestilt og lett med geometrisk skrift og blå heltrukne linjer, passer ingeniører og teknologer.",
        "Left-aligned and light with a geometric font and solid blue lines, suits engineers and technologists.",
    ),
    (
        "Helt svart og tett serif-oppsett med heltrukne linjer, laget for ingeniør- og teknologisøknader.",
        "All-black, tight serif layout with solid lines, made for engineering and technology applications.",
    ),
    ("Tradisjonelt svart-hvitt serif-oppsett med smale marger, passer jus, akademia og finans.", "Traditional black-and-white serif layout with narrow margins, suits law, academia and finance."),
    ("Litterær Garamond i dyp blekkfiolett med kapitéler i overskriftene, passer akademia og forlag.", "Literary Garamond in deep ink violet with small-caps headings, suits academia and publishing."),
    (
        "Klassisk moderncv-preg med blå strek til venstre for overskriftene, passer forskere og ingeniører.",
        "Classic moderncv look with a blue rule to the left of the headings, suits researchers and engineers.",
    ),
    (
        "Lyst og luftig med sentrert topp og havgrønne overskrifter i kapitéler, passer helse og service.",
        "Light and airy with a centered header and sea-green small-caps headings, suits health care and service.",
    ),
    (
        "LaTeX-aktig svart serif med heltrukne linjer og kursiv arbeidsgiver, populær blant utviklere og studenter.",
        "LaTeX-style black serif with solid lines and the employer in italics, popular with developers and students.",
    ),
    // ---- import: reading files and ads ----
    ("PDF-en har et format CVBuilder ikke kan lese tekst fra. Prøv å lagre den som Word eller tekst.", "The PDF is in a format CVBuilder cannot read text from. Try saving it as Word or text."),
    (
        "Fant nesten ingen tekst i filen. Er det en skannet PDF (et bilde)? Da må teksten skrives inn eller kopieres.",
        "Hardly any text was found in the file. Is it a scanned PDF (an image)? Then the text must be typed or copied in.",
    ),
    ("Filen er for stor (over 50 MB) til å være en annonse eller en CV.", "The file is too large (over 50 MB) to be an ad or a CV."),
    ("Filen er tom; det er ingen tekst å hente.", "The file is empty; there is no text to read."),
    ("OpenDocument-filer (.odt) kan ikke leses. Lagre den som Word (.docx) eller PDF først.", "OpenDocument files (.odt) cannot be read. Save it as Word (.docx) or PDF first."),
    ("Pages-filer kan ikke leses. Eksporter den som Word (.docx) eller PDF først.", "Pages files cannot be read. Export it as Word (.docx) or PDF first."),
    ("Filen er en ZIP-pakke, ikke en Word-fil (.docx), og kan ikke leses.", "The file is a ZIP archive, not a Word file (.docx), and cannot be read."),
    ("Dette er en eldre Word-fil (.doc). Åpne den i Word og lagre den som .docx, så kan den leses.", "This is an older Word file (.doc). Open it in Word and save it as .docx, then it can be read."),
    ("RTF-filer kan ikke leses. Lagre den som Word (.docx) eller PDF først.", "RTF files cannot be read. Save it as Word (.docx) or PDF first."),
    ("Filen heter .pdf, men er ikke en PDF. Den kan være skadet eller bare delvis lastet ned.", "The file is named .pdf but is not a PDF. It may be damaged or only partly downloaded."),
    ("Filen heter .docx, men er ikke en Word-fil. Den kan være skadet eller bare delvis lastet ned.", "The file is named .docx but is not a Word file. It may be damaged or only partly downloaded."),
    ("PDF-en er passordbeskyttet eller låst. Åpne den, kopier teksten og lim den inn i stedet.", "The PDF is password-protected or locked. Open it, copy the text and paste it in instead."),
    (
        "PDF-en kunne ikke leses. Den kan være skadet. Åpne den, kopier teksten og lim den inn i stedet.",
        "The PDF could not be read. It may be damaged. Open it, copy the text and paste it in instead.",
    ),
    ("Word-filen kunne ikke leses. Den kan være skadet. Åpne den i Word og lagre den på nytt.", "The Word file could not be read. It may be damaged. Open it in Word and save it again."),
    (
        "Siden krever innlogging, så annonsen kunne ikke hentes. Åpne den i nettleseren, kopier teksten og lim den inn.",
        "The page requires sign-in, so the ad could not be fetched. Open it in your browser, copy the text and paste it in.",
    ),
    (
        "Siden viste nesten ingen tekst. Noen nettsteder bygger siden med JavaScript; åpne den i nettleseren, kopier teksten og lim den inn.",
        "The page showed almost no text. Some sites build the page with JavaScript; open it in your browser, copy the text and paste it in.",
    ),
    ("Fant ikke noe å fylle ut. Lim inn hele annonseteksten først.", "Found nothing to fill in. Paste the whole ad text first."),
    ("e-post", "email"),
    ("LinkedIn", "LinkedIn"),
    ("telefon", "phone"),
    ("navn", "name"),
    ("profil", "profile"),
    ("kompetanse", "skills"),
    ("språk", "languages"),
    // ---- cv-ai: fetching pages and talking to AI services ----
    ("Lenken må starte med https://", "The link must start with https://"),
    ("Fikk ikke hentet siden. Sjekk lenken og internettforbindelsen.", "Could not fetch the page. Check the link and your internet connection."),
    ("Siden var for stor eller ikke tekst.", "The page was too large or not text."),
    ("Fant ikke nettstedet. Sjekk at lenken er riktig.", "Could not find the website. Check that the link is right."),
    ("Siden sendte videre for mange ganger. Åpne den i nettleseren, kopier teksten og lim den inn.", "The page redirected too many times. Open it in your browser, copy the text and paste it in."),
    ("Lenken er ikke gyldig. Kopier hele lenken fra nettleseren.", "The link is not valid. Copy the whole link from your browser."),
    ("Fant ikke annonsen på lenken (feil 404). Den kan være tatt ned, eller lenken er feil.", "Could not find the ad at the link (error 404). It may have been taken down, or the link is wrong."),
    (
        "Nettstedet vil ikke svare akkurat nå (for mange forespørsler). Vent litt, eller kopier annonseteksten og lim den inn.",
        "The website will not answer right now (too many requests). Wait a moment, or copy the ad text and paste it in.",
    ),
    ("Lenken går til en PDF. Last den ned og hent den inn med «Fra fil».", "The link goes to a PDF. Download it and bring it in with “From file”."),
    ("Lenken går ikke til en nettside med tekst. Kopier annonseteksten og lim den inn.", "The link does not go to a web page with text. Copy the ad text and paste it in."),
    ("Siden er for stor (over 5 MB). Kopier annonseteksten og lim den inn.", "The page is too large (over 5 MB). Copy the ad text and paste it in."),
    ("Fikk ikke hentet hele siden. Prøv igjen.", "Could not fetch the whole page. Try again."),
    ("Nøkkelen ble ikke godtatt. Sjekk at du har limt inn hele API-nøkkelen.", "The key was not accepted. Check that you pasted the whole API key."),
    ("AI-en brukte for mange steg på én melding. Del opp oppgaven og prøv igjen.", "The AI used too many steps on one message. Split up the task and try again."),
    ("Claude Code er ikke logget inn. Åpne en terminal, kjør «claude», logg inn, og prøv igjen.", "Claude Code is not signed in. Open a terminal, run “claude”, sign in, and try again."),
    ("Claude Code svarte ikke på ti minutter og ble stoppet.", "Claude Code did not answer within ten minutes and was stopped."),
    ("Claude Code svarte ikke på fem minutter og ble stoppet.", "Claude Code did not answer within five minutes and was stopped."),
    // ---- cv-mcp: what a tool call changed ----
    ("Endret sideoppsettet", "Changed the layout"),
    ("Endret rekkefølgen på seksjonene", "Changed the order of the sections"),
    ("Oppdaterte språkene", "Updated the languages"),
    ("Oppdaterte interessene", "Updated the interests"),
    ("Byttet CV-språket til engelsk", "Switched the CV language to English"),
    ("Byttet CV-språket til bokmål", "Switched the CV language to Norwegian (bokmål)"),
    ("navnet", "the name"),
    ("overskriften", "the headline"),
    ("e-posten", "the email"),
    ("telefonnummeret", "the phone number"),
    ("bostedet", "the place of residence"),
    ("LinkedIn-adressen", "the LinkedIn address"),
    ("profilteksten", "the profile text"),
    ("stillingen", "the job title"),
    ("arbeidsgiveren", "the employer"),
    ("annonselenken", "the ad link"),
    ("annonseteksten", "the ad text"),
    ("fagordene fra annonsen", "the keywords from the ad"),
    ("tittelen", "the title"),
    ("stedet", "the location"),
    ("startdatoen", "the start date"),
    ("sluttdatoen", "the end date"),
    ("punktene", "the bullet points"),
    ("ekstra kontaktlinjer", "extra contact lines"),
    ("rekkefølgen i kontaktlinjen", "the order of the contact line"),
];

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use cv_core::Project;
    use cv_core::model::{Certification, Education, Job, Skill, SpokenLanguage};

    use super::*;

    /// Every string literal passed to `t(` or `tf(` in the app's source, unescaped.
    fn keys_in_source() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("src") {
            let path = entry.expect("entry").path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !name.ends_with(".rs") || name == "i18n.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            let bytes = text.as_bytes();
            for call in ["t(", "tf("] {
                let mut from = 0;
                while let Some(at) = text[from..].find(call).map(|i| i + from) {
                    from = at + call.len();
                    let before = text[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let mut i = from;
                    while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                        i += 1;
                    }
                    if bytes.get(i) != Some(&b'"') {
                        continue;
                    }
                    out.push((name.clone(), literal(&text[i + 1..])));
                }
            }
        }
        out
    }

    /// A Rust string literal's value, from just after its opening quote.
    fn literal(rest: &str) -> String {
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return out,
                '\\' => match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('0') => out.push('\0'),
                    Some(other) => out.push(other),
                    None => break,
                },
                c => out.push(c),
            }
        }
        panic!("unterminated string literal");
    }

    fn placeholders(s: &str) -> BTreeSet<String> {
        let re = Regex::new(r"\{([a-z_]+)\}").expect("regex");
        re.captures_iter(s).map(|c| c[1].to_string()).collect()
    }

    #[test]
    fn every_key_in_the_source_has_english() {
        let keys = keys_in_source();
        assert!(keys.len() > 300, "the source scan found only {} keys", keys.len());
        let missing: Vec<String> = keys.iter().filter(|(_, k)| !TABLE.contains_key(k.as_str())).map(|(f, k)| format!("{f}: {k:?}")).collect();
        assert!(missing.is_empty(), "keys without English:\n{}", missing.join("\n"));
    }

    #[test]
    fn english_entries_are_complete() {
        let mut seen = HashSet::new();
        for (no, en) in EN.iter().chain(DYN.iter()) {
            assert!(seen.insert(*no), "duplicate key {no:?}");
            assert!(!en.trim().is_empty(), "empty English for {no:?}");
            assert_eq!(placeholders(no), placeholders(en), "placeholders differ for {no:?}");
        }
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(fill("{a} av {b} – {a}", &[("a", "1"), ("b", "{b}")]), "1 av {b} – 1");
        assert_eq!(fill("ingen {x} her {", &[]), "ingen {x} her {");
    }

    fn translated(text: &str) -> bool {
        english(text).is_some()
    }

    /// Projects that make cv-core say most of what it can say.
    fn projects() -> Vec<Project> {
        let mut broken = Project::default();
        broken.candidate.email = "ikke-epost".into();
        broken.candidate.phone = "12".into();
        broken.candidate.linkedin = "linkedin.com/in/x".into();
        broken.target.url = "http://x".into();
        broken.candidate.summary = "ord ".repeat(95);
        broken.experience.push(Job { id: "j1".into(), end: Some(String::new()), ..Default::default() });
        broken.experience.push(Job {
            id: "j2".into(),
            company: "Firma".into(),
            title: "Jeg".into(),
            start: "2020-05".into(),
            end: Some("2019-01".into()),
            bullets: vec!["Jeg var ansvarlig for alt, og ".repeat(10)],
            ..Default::default()
        });
        broken.education.push(Education::default());
        broken.skills.push(Skill { group: String::new(), name: String::new(), evidence: String::new(), confirmed: true });
        broken.skills.push(Skill { group: String::new(), name: "Rust".into(), evidence: String::new(), confirmed: true });
        broken.certifications.push(Certification::default());
        broken.languages.push(SpokenLanguage::default());
        broken.options.photo = true;
        broken.target.keywords = vec!["Rust".into(), "Kotlin".into()];
        let mut example = Project::example();
        example.target.keywords = vec!["Finnes ikke".into()];
        vec![Project::default(), Project::example(), broken, example]
    }

    #[test]
    fn cv_core_messages_have_english() {
        set(Lang::En);
        let mut missing = Vec::new();
        let mut check = |text: &str| {
            if !translated(text) {
                missing.push(text.to_string());
            }
        };
        for p in projects() {
            for e in cv_core::checks::blocking(&p) {
                check(&e);
            }
            for pages in [1, 3] {
                let (list, _) = cv_core::checks::audit(&p, pages, 2026 * 12);
                for c in list {
                    check(&c.text);
                    check(&c.tip);
                }
            }
            let mut ad = cv_core::import::AdInfo { role: "Utvikler".into(), company: "Firma".into(), description: String::new(), keywords: vec!["Rust".into(), "Ny".into()] };
            let mut q = p.clone();
            check(&cv_core::import::apply_ad(&mut q, &ad));
            ad = cv_core::import::AdInfo::default();
            check(&cv_core::import::apply_ad(&mut q, &ad));
        }
        for d in cv_core::layouts::all() {
            check(&d.description);
            check(&d.credit);
            check(cv_core::layouts::category_label(&d.category));
            if let Some(w) = d.ats_warning() {
                check(w);
            }
            let mut pale = d.clone();
            pale.ink = "#EEEEEE".into();
            pale.muted = "#EEEEEE".into();
            pale.accent = "#EEEEEE".into();
            pale.header_fill = Some("#FFFFFF".into());
            pale.header_text = Some("#FEFEFE".into());
            for w in pale.color_warnings() {
                check(&w);
            }
        }
        for (_, label) in cv_core::layouts::CATEGORIES {
            check(label);
        }
        check(cv_core::layouts::category_label("ukjent"));
        for (name, _) in cv_core::layouts::palettes() {
            check(name);
        }
        for e in cv_core::layouts::parse("{}").err().unwrap_or_default().lines() {
            check(e);
        }
        {
            use cv_core::checklist::{self, Checklist, Item, Kind};
            let item = |id: &str, kind: Kind, value: &str| Item { id: id.into(), text: String::new(), kind, value: value.into() };
            let bad = Checklist {
                format: "x".into(),
                name: " ".into(),
                items: (0..101).map(|_| item("a", Kind::Contains, "/")).chain([item("b", Kind::MaxPages, "0"), item("c", Kind::Filled, "sko")]).collect(),
            };
            for e in bad.problems() {
                check(&e);
            }
            for e in [checklist::parse(&"x".repeat(checklist::MAX_FILE_BYTES + 1)), checklist::parse_bytes(&[0xff, 0xfe]), checklist::parse("[")] {
                check(&e.err().unwrap_or_default());
            }
            for (_, name) in checklist::FIELDS {
                check(name);
            }
        }
        for f in cv_core::import::read_cv("Ola Nordmann\nola@example.com\n+47 900 00 000\nlinkedin.com/in/ola\n\nProfil\nErfaren utvikler.\n\nKompetanse\nRust, Kotlin\n\nSpråk\nNorsk: morsmål").found
        {
            check(&f);
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "cv-core text without English:\n{}", missing.join("\n"));
    }

    #[test]
    fn patterns_read_numbers_and_names() {
        set(Lang::En);
        assert_eq!(tdyn("Profilen har 41 ord"), "The profile has 41 words");
        assert_eq!(tdyn("8 av 9 fagord fra annonsen finnes i CV-en"), "8 of 9 keywords from the ad are in the CV");
        assert_eq!(tdyn("Jobb 2 (Firma AS): sluttdatoen er før startdatoen."), "Job 2 (Firma AS): the end date is before the start date.");
        assert_eq!(tdyn("Fullt navn må fylles ut."), "Full name is required.");
        assert_eq!(tdyn("Endret navnet, e-posten og overskriften"), "Changed the name, the email and the headline");
        assert_eq!(tdyn("Endret tittelen for Utvikler hos Firma"), "Changed the title for Utvikler at Firma");
        assert_eq!(
            tdyn("Fra annonsen: stilling «Utvikler», 3 fagord. 2 fagord står allerede i CV-en din og er lagt til i kompetanse: Rust, Go."),
            "From the ad: job title «Utvikler», 3 keywords. 2 keywords are already in your CV and added to your skills: Rust, Go."
        );
        assert_eq!(tdyn("Noe helt ukjent"), "Noe helt ukjent");
        assert_eq!(tf("Jobb {n}", &[("n", "3")]), "Job 3");
    }

    fn german() -> Pack {
        let mut strings = BTreeMap::new();
        strings.insert("Angre".to_string(), "Rückgängig".to_string());
        strings.insert("Kunne ikke lagre: {e}".to_string(), "Konnte nicht speichern: {e}".to_string());
        strings.insert("Navn".to_string(), "Name (DE)".to_string());
        Pack { code: "de".into(), name: "Deutsch".into(), made_by: "AI (test)".into(), strings }
    }

    #[test]
    fn pack_round_trip() {
        let pack = german();
        let text = pack_to_json(&pack);
        let back = parse_pack(&text).expect("parse");
        assert_eq!(back.pack, pack);
        assert_eq!((back.dropped, back.unknown), (0, 0));
        // The template has every key, with English as the value, and must be filled in first.
        let template = template_json();
        let v: serde_json::Value = serde_json::from_str(&template).expect("template json");
        assert_eq!(v["strings"].as_object().map(|m| m.len()), Some(all_keys().count()));
        assert_eq!(v["strings"]["Angre"], "Undo");
        assert_eq!(parse_pack(&template).err(), Some(PackError::NoName));
        let filled = template.replacen("\"code\": \"\"", "\"code\": \"pt_br\"", 1).replacen("\"name\": \"\"", "\"name\": \"Português\"", 1);
        let checked = parse_pack(&filled).expect("filled template");
        assert_eq!(checked.pack.code, "pt-BR");
        assert_eq!(checked.pack.strings.len(), all_keys().count());
    }

    #[test]
    fn bad_entries_are_dropped_and_counted() {
        let text = r#"{"format": "cvbuilder-language/1", "code": "pl", "name": "Polski", "strings": {
            "Angre": "Cofnij",
            "Kunne ikke lagre: {e}": "Nie udało się zapisać",
            "«{file}» kunne ikke åpnes. Her åpner du bare .json-utkast lagret med «Lagre som fil».": "{plik} {file}",
            "Bilde": "   ",
            "Lukk": 3,
            "Finnes ikke i appen": "x"
        }}"#;
        let checked = parse_pack(text).expect("parse");
        assert_eq!(checked.pack.strings.len(), 1);
        assert_eq!(checked.pack.strings["Angre"], "Cofnij");
        // A lost {e}, an extra {plik}, blank text and a number are dropped; one key is unknown.
        assert_eq!(checked.dropped, 4);
        assert_eq!(checked.unknown, 1);
    }

    #[test]
    fn pack_files_are_checked() {
        let file = |format: &str, code: &str, name: &str| format!(r#"{{"format": "{format}", "code": "{code}", "name": "{name}", "strings": {{}}}}"#);
        assert_eq!(parse_pack(&file("cvbuilder-language/2", "de", "Deutsch")).err(), Some(PackError::Format));
        assert_eq!(parse_pack(&file(PACK_FORMAT, "de", " ")).err(), Some(PackError::NoName));
        assert_eq!(parse_pack(&file(PACK_FORMAT, "d", "Deutsch")).err(), Some(PackError::BadCode("d".into())));
        assert_eq!(parse_pack(&file(PACK_FORMAT, "en", "English")).err(), Some(PackError::Reserved("en".into())));
        assert!(matches!(parse_pack("not json"), Err(PackError::NotJson(_))));
        assert_eq!(normalize_code(" DE "), Ok("de".into()));
        assert_eq!(normalize_code("pt-br"), Ok("pt-BR".into()));
        assert_eq!(normalize_code("zh_TW"), Ok("zh-TW".into()));
        assert!(normalize_code("de-DEU").is_err());
        assert!(normalize_code("toolonglang").is_err());
        assert!(normalize_code("d3").is_err());
        assert!(normalize_code("").is_err());
    }

    #[test]
    fn leaked_lookups_use_the_pack_then_english() {
        let table = leak_table(&german());
        let text: &'static str = ui_text(Some(&table), "Angre");
        assert_eq!(text, "Rückgängig");
        assert_eq!(fill(ui_text(Some(&table), "Kunne ikke lagre: {e}"), &[("e", "Disk voll")]), "Konnte nicht speichern: Disk voll");
        // Missing in the pack: English; unknown everywhere: the key itself.
        assert_eq!(ui_text(Some(&table), "Bilde"), "Photo");
        assert_eq!(ui_text(Some(&table), "Ingen slik tekst"), "Ingen slik tekst");
        assert_eq!(ui_text(None, "Angre"), "Undo");
        // Keys not in the app are not kept.
        let mut odd = german();
        odd.strings.insert("Ukjent".into(), "Unbekannt".into());
        assert_eq!(leak_table(&odd).len(), 3);
        // cv-core text: the pack's exact entry, else English, also for the patterns.
        assert_eq!(dyn_text(Some(&table), "Navn"), "Name (DE)");
        assert_eq!(dyn_text(Some(&table), "Portrett"), "Portrait");
        assert_eq!(dyn_text(Some(&table), "Profilen har 41 ord"), "The profile has 41 words");
        assert_eq!(dyn_text(Some(&table), "Noe helt ukjent"), "Noe helt ukjent");
    }

    #[test]
    fn auto_picks_a_matching_pack() {
        let mut brazil = german();
        brazil.code = "pt-BR".into();
        brazil.name = "Português (Brasil)".into();
        let mut nynorsk = german();
        nynorsk.code = "nn".into();
        let packs = vec![german(), brazil, nynorsk];
        assert_eq!(choose("auto", "de-DE", &packs), Choice::Pack(0));
        assert_eq!(choose("auto", "de_AT.UTF-8", &packs), Choice::Pack(0));
        assert_eq!(choose("auto", "pt-BR", &packs), Choice::Pack(1));
        assert_eq!(choose("auto", "pt-PT", &packs), Choice::Pack(1));
        assert_eq!(choose("auto", "nn-NO", &packs), Choice::Pack(2));
        assert_eq!(choose("auto", "nb-NO", &packs), Choice::No);
        assert_eq!(choose("auto", "fr-FR", &packs), Choice::En);
        assert_eq!(choose("auto", "", &packs), Choice::En);
        assert_eq!(choose("no", "de-DE", &packs), Choice::No);
        assert_eq!(choose("en", "de-DE", &packs), Choice::En);
        assert_eq!(choose("pt-BR", "nb-NO", &packs), Choice::Pack(1));
        // A pack that is gone: as «auto».
        assert_eq!(choose("pl", "nb-NO", &packs), Choice::No);
        assert_eq!(choose("pl", "de-DE", &[]), Choice::En);
    }

    #[test]
    fn settings_values() {
        assert_eq!(from_setting("no"), Lang::No);
        assert_eq!(from_setting("en"), Lang::En);
        assert_eq!(from_setting("auto"), system());
    }
}
