//! Offline spell check in the CV's language (to-do 046): the words in the CV's own text that the
//! dictionary for its language does not know, listed as *possible* mistakes – never errors, since
//! no dictionary knows every name, tool and compound word.
//!
//! Dictionaries are Hunspell files (`.aff` + `.dic`) read by `spellbook` (pure Rust, MPL-2.0).
//! English, French and Spanish come with the app ([`bundled`]); the others are downloaded once
//! from a pinned commit of wooorm/dictionaries ([`Source::url`]), checked against their SHA-256,
//! and then used offline. A person can also pick a Hunspell file by hand ([`decode`]).
//!
//! Names, e-mail and web addresses, numbers, acronyms, the employers, schools, skills and the job
//! ad's own words are never listed ([`ignored`]); the person's own words neither.
use std::collections::{HashMap, HashSet};
use std::io::Read;

use crate::model::Project;

pub use spellbook::Dictionary;

/// The commit of <https://github.com/wooorm/dictionaries> every download comes from. Each
/// dictionary there is in UTF-8 with its licence next to it.
pub const COMMIT: &str = "8cfea406b505e4d7df52d5a19bce525df98c54ab";

/// A Hunspell dictionary CVBuilder knows where to get.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// Its folder in wooorm/dictionaries («nb», «en-GB») and the file name it is kept under.
    pub id: &'static str,
    /// The CV language codes it checks («nb», «no»).
    pub codes: &'static [&'static str],
    /// Its licence, as SPDX.
    pub licence: &'static str,
    /// The size of `.aff` and `.dic` together, in bytes (for «Last ned (1,4 MB)»).
    pub bytes: u64,
    pub aff_sha256: &'static str,
    pub dic_sha256: &'static str,
    /// It comes with the app; nothing to download.
    pub bundled: bool,
}

impl Source {
    /// Where the file (`index.aff`, `index.dic` or `license`) is downloaded from.
    pub fn url(&self, file: &str) -> String {
        format!("https://raw.githubusercontent.com/wooorm/dictionaries/{COMMIT}/dictionaries/{}/{file}", self.id)
    }

    /// The size for people: «6 MB».
    pub fn size(&self) -> String {
        size_text(self.bytes)
    }
}

/// A download size for people, in whole megabytes rounded up: «1 MB», «6 MB».
pub fn size_text(bytes: u64) -> String {
    format!("{} MB", bytes.div_ceil(1_000_000).max(1))
}

include!("spelling_sources.rs");

/// The dictionaries for a CV language code («nb», «en», «pt-BR»), best first. Empty when there is
/// none: Japanese, Chinese and Thai do not use word lists for this, and some languages have no
/// free dictionary yet.
pub fn sources_for(code: &str) -> Vec<&'static Source> {
    let code = code.to_lowercase();
    let base = code.split('-').next().unwrap_or("");
    let exact: Vec<&Source> = SOURCES.iter().filter(|s| s.codes.iter().any(|c| c.eq_ignore_ascii_case(&code))).collect();
    if !exact.is_empty() && code != base {
        return exact;
    }
    SOURCES.iter().filter(|s| s.codes.iter().any(|c| c.eq_ignore_ascii_case(base))).collect()
}

/// The dictionaries that come with the app: `(aff, dic)`, gzipped.
pub fn bundled(id: &str) -> Option<(&'static [u8], &'static [u8])> {
    macro_rules! dict {
        ($id:literal) => {
            (include_bytes!(concat!("../../../assets/dictionaries/", $id, ".aff.gz")).as_slice(), include_bytes!(concat!("../../../assets/dictionaries/", $id, ".dic.gz")).as_slice())
        };
    }
    Some(match id {
        "en" => dict!("en"),
        "en-GB" => dict!("en-GB"),
        "fr" => dict!("fr"),
        "es" => dict!("es"),
        _ => return None,
    })
}

/// A bundled dictionary, ready to use.
pub fn load_bundled(id: &str) -> Result<Dictionary, String> {
    let (aff, dic) = bundled(id).ok_or_else(|| format!("no bundled dictionary {id}"))?;
    let (aff, dic) = (gunzip(aff)?, gunzip(dic)?);
    load(&aff, &dic)
}

/// Reads a dictionary from its two files' bytes (any encoding [`decode`] knows).
pub fn load_bytes(aff: &[u8], dic: &[u8]) -> Result<Dictionary, String> {
    let encoding = encoding(aff);
    load(&decode(aff, Some(&encoding))?, &decode(dic, Some(&encoding))?)
}

fn load(aff: &str, dic: &str) -> Result<Dictionary, String> {
    match Dictionary::new(aff, dic) {
        Ok(d) => Ok(d),
        // Hunspell skips a broken line; spellbook refuses the whole file. Leave those lines out.
        Err(e) => Dictionary::new(aff, &without_bad_lines(aff, dic)).map_err(|_| e.to_string()),
    }
}

/// A `.dic` text without the lines whose flags cannot be read: with `FLAG num`, flags that are
/// not numbers separated by commas («"A/S"» in the Danish dictionary).
fn without_bad_lines(aff: &str, dic: &str) -> String {
    let numeric = aff.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == ["FLAG", "num"]);
    if !numeric {
        return dic.to_string();
    }
    let good = |line: &str| {
        let word = line.split(['\t', ' ']).next().unwrap_or("");
        // The flags follow the last «/» that is not escaped.
        let bytes = word.as_bytes();
        let slash = (0..bytes.len()).rev().find(|&i| bytes[i] == b'/' && (i == 0 || bytes[i - 1] != b'\\'));
        match slash {
            None => true,
            Some(i) => {
                let flags = &word[i + 1..];
                !flags.is_empty() && flags.split(',').all(|f| f.parse::<u16>().is_ok_and(|n| n > 0))
            }
        }
    };
    dic.lines().enumerate().filter(|(i, l)| *i == 0 || good(l)).map(|(_, l)| l).collect::<Vec<_>>().join("\n")
}

fn gunzip(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(unpack(bytes)?).map_err(|e| e.to_string())
}

/// The bytes of a gzipped file (the bundled dictionaries are kept that way).
pub fn unpack(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The `SET` line of an `.aff` file (its encoding), upper-cased; «UTF-8» when it has none.
fn set_line(aff: &str) -> String {
    aff.lines().find_map(|l| l.trim().strip_prefix("SET ").map(|e| e.trim().to_uppercase())).unwrap_or_else(|| "UTF-8".into())
}

/// The encoding an `.aff` file names on its `SET` line (ASCII in every encoding), «UTF-8» when none.
pub fn encoding(aff: &[u8]) -> String {
    let head: String = aff.iter().take(4096).map(|&b| b as char).collect();
    set_line(&head)
}

/// The text of a Hunspell file. UTF-8, ISO 8859-1 and ISO 8859-15 (what most older dictionaries
/// use) are read; another encoding is refused with its name. `encoding` is the `.aff` file's for
/// a `.dic` file; for the `.aff` file itself it is read from its `SET` line.
pub fn decode(bytes: &[u8], named: Option<&str>) -> Result<String, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let encoding = match named {
        Some(e) => e.to_uppercase(),
        None => self::encoding(bytes),
    };
    let latin = |fifteen: bool| -> String {
        bytes
            .iter()
            .map(|&b| match (fifteen, b) {
                (true, 0xA4) => '€',
                (true, 0xA6) => 'Š',
                (true, 0xA8) => 'š',
                (true, 0xB4) => 'Ž',
                (true, 0xB8) => 'ž',
                (true, 0xBC) => 'Œ',
                (true, 0xBD) => 'œ',
                (true, 0xBE) => 'Ÿ',
                _ => b as char,
            })
            .collect()
    };
    let text = match encoding.replace('_', "-").as_str() {
        "UTF-8" | "UTF8" => String::from_utf8(bytes.to_vec()).map_err(|_| "the dictionary says UTF-8 but is not".to_string())?,
        "ISO8859-1" | "ISO-8859-1" | "LATIN1" => latin(false),
        "ISO8859-15" | "ISO-8859-15" => latin(true),
        other => return Err(format!("unsupported encoding {other}")),
    };
    // spellbook reads text: say so in the file itself too.
    Ok(text.lines().map(|l| if l.trim_start().starts_with("SET ") { "SET UTF-8" } else { l }).collect::<Vec<_>>().join("\n"))
}

/// Where a word stands in the CV, for «Står i: …».
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Place {
    Headline,
    Profile,
    /// A job, by its title or employer.
    Job(String),
    Education(String),
    Certifications,
    Interests,
    /// A section of the person's own, by its title.
    Section(String),
}

/// The CV's own prose: what the person wrote, not names of things.
pub fn texts(p: &Project) -> Vec<(Place, &str)> {
    let mut out: Vec<(Place, &str)> = vec![(Place::Headline, p.candidate.headline.as_str()), (Place::Profile, p.candidate.summary.as_str())];
    for j in &p.experience {
        let name = if j.title.trim().is_empty() { j.company.clone() } else { j.title.clone() };
        out.push((Place::Job(name.clone()), j.title.as_str()));
        for b in &j.bullets {
            out.push((Place::Job(name.clone()), b.as_str()));
        }
    }
    for e in &p.education {
        out.push((Place::Education(e.qualification.clone()), e.qualification.as_str()));
    }
    for c in &p.certifications {
        out.push((Place::Certifications, c.name.as_str()));
    }
    for i in &p.interests {
        out.push((Place::Interests, i.label.as_str()));
        out.push((Place::Interests, i.description.as_str()));
    }
    for s in &p.custom_sections {
        let place = Place::Section(s.title.clone());
        out.push((place.clone(), s.title.as_str()));
        for item in &s.items {
            out.push((place.clone(), item.title.as_str()));
            for b in &item.bullets {
                out.push((place.clone(), b.as_str()));
            }
        }
    }
    out.retain(|(_, t)| !t.trim().is_empty());
    out
}

/// Words never listed for this CV, in lower case: the person's name and contact lines, the
/// employers, places, schools, course issuers, skills and tools, languages, and every word of the
/// job ad (the employer's own spelling of its name and products).
pub fn ignored(p: &Project) -> HashSet<String> {
    let mut from: Vec<&str> = vec![&p.candidate.name, &p.candidate.email, &p.candidate.city, &p.candidate.linkedin, &p.target.role, &p.target.company, &p.target.description];
    from.extend(p.candidate.extra.iter().map(|c| c.value.as_str()));
    from.extend(p.target.keywords.iter().map(String::as_str));
    for j in &p.experience {
        from.extend([j.company.as_str(), j.location.as_str()]);
    }
    for e in &p.education {
        from.push(&e.institution);
    }
    for c in &p.certifications {
        from.push(&c.issuer);
    }
    from.extend(p.skills.iter().flat_map(|s| [s.name.as_str(), s.group.as_str()]));
    from.extend(p.languages.iter().map(|l| l.name.as_str()));
    for s in &p.custom_sections {
        from.extend(s.items.iter().map(|i| i.subtitle.as_str()));
    }
    let mut out = HashSet::new();
    for text in from {
        for w in words(text) {
            out.insert(w.to_lowercase());
            for part in w.split('-') {
                out.insert(part.to_lowercase());
            }
        }
    }
    out
}

const APOSTROPHES: [char; 2] = ['\'', '’'];

/// The words worth checking in a text, in order. A chunk with a digit, an e-mail or web address,
/// a path or code (`@ / \ _ # + = < > & . : ://` inside it) is skipped whole, and so are acronyms
/// («ATS», «NAV») and words with a capital inside («JavaScript», «iPhone»): those are names.
pub fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for chunk in text.split_whitespace() {
        let core = chunk.trim_matches(|c: char| !c.is_alphanumeric());
        if core.is_empty()
            || core.chars().any(|c| c.is_numeric() || matches!(c, '@' | '/' | '\\' | '_' | '#' | '+' | '=' | '<' | '>' | '&' | '.' | ':' | '|' | '~' | '$' | '%' | '*'))
            || chunk.to_lowercase().starts_with("www")
        {
            continue;
        }
        // «(Python,» «“word”» → word; «A/S» was skipped above.
        for piece in core.split(|c: char| !(c.is_alphabetic() || is_mark(c) || c == '-' || APOSTROPHES.contains(&c))) {
            let piece = piece.trim_matches(|c: char| c == '-' || APOSTROPHES.contains(&c));
            if piece.chars().count() < 2 || is_name_like(piece) {
                continue;
            }
            out.push(piece);
        }
    }
    out
}

fn is_mark(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c) || ('\u{900}'..='\u{dff}').contains(&c) && !c.is_alphabetic()
}

/// An acronym, or a word with a capital after its first letter.
fn is_name_like(word: &str) -> bool {
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    let upper = letters.iter().filter(|c| c.is_uppercase()).count();
    if upper == letters.len() && upper > 0 {
        return true;
    }
    // A capital that does not start the word or a part of it («iPhone», «LinkedIn»; «Oslo-Bergen» is fine).
    let mut prev: Option<char> = None;
    for c in word.chars() {
        if c.is_uppercase() && prev.is_some_and(|p| p.is_alphabetic()) {
            return true;
        }
        prev = Some(c);
    }
    false
}

/// A word the dictionary may not know: how it was written the first time, how often, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub word: String,
    pub count: usize,
    pub places: Vec<Place>,
}

/// Whether any of the dictionaries knows the word, also with a straight apostrophe for a curly
/// one, and a hyphenated word part by part («IT-konsulent», «well-known»).
pub fn known(dicts: &[Dictionary], word: &str) -> bool {
    let one = |w: &str| dicts.iter().any(|d| d.check(w));
    if one(word) {
        return true;
    }
    if word.contains('’') && one(&word.replace('’', "'")) {
        return true;
    }
    if word.contains('-') {
        return word.split('-').filter(|p| !p.is_empty()).all(|p| p.chars().count() < 2 || is_name_like(p) || one(p));
    }
    false
}

/// The possible mistakes in a CV, in the order they first appear. `ok` says whether a word is
/// spelt right (see [`known`]); `own` are the person's own words and the ones ignored, in lower case.
pub fn find(p: &Project, ok: impl Fn(&str) -> bool, own: &HashSet<String>) -> Vec<Finding> {
    let skip = ignored(p);
    let mut verdict: HashMap<String, bool> = HashMap::new();
    let mut found: Vec<Finding> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (place, text) in texts(p) {
        for w in words(text) {
            let key = w.to_lowercase();
            if skip.contains(&key) || own.contains(&key) {
                continue;
            }
            let good = *verdict.entry(w.to_string()).or_insert_with(|| ok(w));
            if good {
                continue;
            }
            match index.get(&key) {
                Some(&i) => {
                    found[i].count += 1;
                    if !found[i].places.contains(&place) {
                        found[i].places.push(place.clone());
                    }
                }
                None => {
                    index.insert(key, found.len());
                    found.push(Finding { word: w.to_string(), count: 1, places: vec![place.clone()] });
                }
            }
        }
    }
    found
}

/// Up to `n` suggestions for a word, from all dictionaries, without repeats.
pub fn suggest(dicts: &[Dictionary], word: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in dicts {
        let mut list = Vec::new();
        d.suggest(word, &mut list);
        for s in list {
            if !out.contains(&s) && s != word {
                out.push(s);
            }
        }
    }
    out.truncate(n);
    out
}

/// Puts `to` in place of the word `from` everywhere in the CV's own text ([`texts`]), as a whole
/// word and in any case: «Teh» becomes «The» when `to` is «the». How many places changed.
pub fn replace_word(p: &mut Project, from: &str, to: &str) -> usize {
    let mut n = 0;
    let mut fix = |s: &mut String| {
        let (text, k) = replace_in(s, from, to);
        if k > 0 {
            *s = text;
            n += k;
        }
    };
    fix(&mut p.candidate.headline);
    fix(&mut p.candidate.summary);
    for j in &mut p.experience {
        fix(&mut j.title);
        j.bullets.iter_mut().for_each(&mut fix);
    }
    for e in &mut p.education {
        fix(&mut e.qualification);
    }
    for c in &mut p.certifications {
        fix(&mut c.name);
    }
    for i in &mut p.interests {
        fix(&mut i.label);
        fix(&mut i.description);
    }
    for s in &mut p.custom_sections {
        fix(&mut s.title);
        for item in &mut s.items {
            fix(&mut item.title);
            item.bullets.iter_mut().for_each(&mut fix);
        }
    }
    n
}

fn replace_in(text: &str, from: &str, to: &str) -> (String, usize) {
    let wanted = from.to_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut n = 0;
    let mut rest = text;
    let is_word = |c: char| c.is_alphanumeric() || is_mark(c);
    // Hyphens and apostrophes inside a word belong to it, as in [`words`]: «self-motivatd» and
    // «recieve's» are found whole, so they are replaced whole too.
    let joins = |c: char| c == '-' || APOSTROPHES.contains(&c);
    while !rest.is_empty() {
        // The next word: word characters, with hyphens and apostrophes only between them.
        let start = rest.char_indices().find(|(_, c)| is_word(*c)).map(|(i, _)| i).unwrap_or(rest.len());
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let mut end = 0;
        let mut chars = rest.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if is_word(c) {
                end = i + c.len_utf8();
            } else if !(joins(c) && chars.peek().is_some_and(|&(_, n)| is_word(n) || joins(n))) {
                break;
            }
        }
        let word = &rest[..end];
        if !word.is_empty() && word.to_lowercase() == wanted {
            let capital = word.chars().next().is_some_and(char::is_uppercase) && to.chars().next().is_some_and(char::is_lowercase);
            if capital {
                let mut c = to.chars();
                out.extend(c.next().into_iter().flat_map(char::to_uppercase));
                out.push_str(c.as_str());
            } else {
                out.push_str(to);
            }
            n += 1;
        } else {
            out.push_str(word);
        }
        rest = &rest[end..];
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Education, Job, Skill};

    fn english() -> Vec<Dictionary> {
        vec![load_bundled("en").expect("en"), load_bundled("en-GB").expect("en-GB")]
    }

    #[test]
    fn names_addresses_numbers_and_acronyms_are_not_words() {
        assert_eq!(words("Led 12 people at ACME (Oslo), see https://x.no or me@x.no – B2B, C++ and Node.js."), vec!["Led", "people", "at", "Oslo", "see", "or", "and"]);
        assert_eq!(words("JavaScript iPhone LinkedIn NAV well-known «quoted» it's"), vec!["well-known", "quoted", "it's"]);
    }

    #[test]
    fn every_bundled_dictionary_loads_and_knows_common_words() {
        for (id, good, bad) in [("en", "management", "managment"), ("en-GB", "colour", "colur"), ("fr", "responsable", "responsabel"), ("es", "experiencia", "experiensia")] {
            let d = load_bundled(id).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(d.check(good), "{id} should know {good}");
            assert!(!d.check(bad), "{id} should not know {bad}");
        }
    }

    #[test]
    fn a_typo_is_found_with_its_place_and_a_suggestion() {
        let dicts = english();
        let mut p = Project::default();
        p.candidate.name = "Ola Nordmann".into();
        p.candidate.summary = "Experienced manager with strong leadership skils. Ola leads teams.".into();
        p.experience.push(Job {
            company: "Fjordkraft".into(),
            title: "Project manager".into(),
            bullets: vec!["Recieved praise from Fjordkraft".into(), "Improved skils".into()],
            ..Default::default()
        });
        p.skills.push(Skill { name: "Kubernetes".into(), ..Default::default() });
        p.education.push(Education { qualification: "Master of Science".into(), institution: "NTNU Gløshaugen".into(), ..Default::default() });
        let found = find(&p, |w| known(&dicts, w), &HashSet::new());
        let words: Vec<&str> = found.iter().map(|f| f.word.as_str()).collect();
        assert_eq!(words, vec!["skils", "Recieved"], "the name, employer and school are ignored");
        assert_eq!(found[0].count, 2);
        assert_eq!(found[0].places, vec![Place::Profile, Place::Job("Project manager".into())]);
        assert!(suggest(&dicts, "Recieved", 5).contains(&"Received".to_string()));
        // The person's own words are not listed.
        let own: HashSet<String> = ["skils".to_string()].into();
        assert_eq!(find(&p, |w| known(&dicts, w), &own).len(), 1);
    }

    #[test]
    fn both_english_spellings_and_hyphenated_words_pass() {
        let dicts = english();
        for w in ["colour", "color", "organisation", "well-known", "IT-manager", "don't", "don’t"] {
            assert!(known(&dicts, w), "{w}");
        }
        assert!(!known(&dicts, "well-knwon"));
    }

    #[test]
    fn a_word_is_replaced_whole_and_keeps_its_capital() {
        let mut p = Project::default();
        p.candidate.summary = "Recieved awards. I recieved, not unrecieved.".into();
        p.experience.push(Job { bullets: vec!["recieved".into()], ..Default::default() });
        assert_eq!(replace_word(&mut p, "recieved", "received"), 3);
        assert_eq!(p.candidate.summary, "Received awards. I received, not unrecieved.");
        assert_eq!(p.experience[0].bullets[0], "received");
    }

    #[test]
    fn a_word_with_a_hyphen_or_apostrophe_is_replaced_whole() {
        let mut p = Project::default();
        p.candidate.summary = "Self-motivatd and self-motivatd. I recieve's it, and recieve’s too.".into();
        p.experience.push(Job { bullets: vec!["A self-motivatd- team".into(), "'recieve's'".into()], ..Default::default() });
        assert_eq!(replace_word(&mut p, "self-motivatd", "self-motivated"), 3);
        assert_eq!(replace_word(&mut p, "recieve's", "receives"), 2);
        assert_eq!(replace_word(&mut p, "recieve’s", "receives"), 1);
        assert_eq!(p.candidate.summary, "Self-motivated and self-motivated. I receives it, and receives too.");
        assert_eq!(p.experience[0].bullets, ["A self-motivated- team", "'receives'"]);
        // Only the whole word: one half of a hyphenated word stays as it is.
        p.candidate.headline = "well-knwon, knwon".into();
        assert_eq!(replace_word(&mut p, "knwon", "known"), 1);
        assert_eq!(p.candidate.headline, "well-knwon, known");
        // Every word that `words` finds can be replaced.
        for w in words("Self-motivatd, recieve's «co-op» rock'n'roll") {
            let mut q = Project::default();
            q.candidate.summary = format!("x {w}.");
            assert_eq!(replace_word(&mut q, w, "ok"), 1, "{w}");
        }
    }

    #[test]
    fn each_cv_language_finds_its_dictionaries() {
        let ids = |c: &str| sources_for(c).iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!(ids("en"), vec!["en", "en-GB"]);
        assert_eq!(ids("en-GB"), vec!["en-GB"]);
        assert_eq!(ids("nb"), vec!["nb"]);
        assert!(ids("ja").is_empty() && ids("zh").is_empty() && ids("th").is_empty());
        for s in SOURCES {
            assert_eq!(s.bundled, bundled(s.id).is_some(), "{}", s.id);
            assert_eq!(s.aff_sha256.len() + s.dic_sha256.len(), 128, "{}", s.id);
        }
    }

    #[test]
    fn older_dictionaries_in_latin_1_are_read() {
        let aff = b"SET ISO8859-1\nTRY esianrtolcdugmphbyfvkwz\n";
        let dic = b"2\nbl\xe5b\xe6r\nk\xf8\n";
        let d = load_bytes(aff, dic).expect("latin-1");
        assert!(d.check("blåbær") && d.check("kø"));
        assert!(decode(b"SET KOI8-R\n", None).is_err());
    }
}

#[cfg(test)]
mod survey {
    /// Loads every dictionary from `CVBUILDER_DICTS/<id>/index.{aff,dic}` and times it.
    #[test]
    #[ignore]
    fn every_source_loads() {
        let dir = std::path::PathBuf::from(std::env::var("CVBUILDER_DICTS").expect("CVBUILDER_DICTS"));
        for s in super::SOURCES {
            let t = std::time::Instant::now();
            let (aff, dic) = (std::fs::read(dir.join(s.id).join("index.aff")).unwrap(), std::fs::read(dir.join(s.id).join("index.dic")).unwrap());
            match super::load_bytes(&aff, &dic) {
                Ok(d) => {
                    let load = t.elapsed();
                    let t = std::time::Instant::now();
                    let mut v = Vec::new();
                    d.suggest("xqzwrtyp", &mut v);
                    println!("{} ok load {:?} suggest {:?}", s.id, load, t.elapsed());
                }
                Err(e) => println!("{} FAIL {e}", s.id),
            }
        }
    }
}

#[cfg(test)]
mod survey_words {
    #[test]
    #[ignore]
    fn compounds() {
        let dir = std::path::PathBuf::from(std::env::var("CVBUILDER_DICTS").expect("CVBUILDER_DICTS"));
        for (id, words) in [
            (
                "nb",
                "prosjektleder kundeservicemedarbeider ansvarlig for leveranser og budsjett, ledet et team på åtte. Erfaren systemutvikler med god kompetanse innen skyløsninger. Fagbrev elektriker. Ansvarlihg Kundeorientertt",
            ),
            ("nn", "prosjektleiar med ansvar for leveransar og budsjett. Erfaren systemutviklar"),
            ("de", "Projektleiter Kundendienstmitarbeiter verantwortlich für Lieferungen und Budget, leitete ein Team von acht Personen. Erfahrener Softwareentwickler Verantwortlihc"),
            ("sv", "projektledare kundtjänstmedarbetare ansvarig för leveranser och budget, ledde ett team"),
            ("da", "projektleder kundeservicemedarbejder ansvarlig for leverancer og budget"),
            ("nl", "projectleider klantenservicemedewerker verantwoordelijk voor leveringen en budget"),
        ] {
            let d = super::load_bytes(&std::fs::read(dir.join(id).join("index.aff")).unwrap(), &std::fs::read(dir.join(id).join("index.dic")).unwrap()).unwrap();
            let bad: Vec<&str> = super::words(words).into_iter().filter(|w| !super::known(std::slice::from_ref(&d), w)).collect();
            let sugg: Vec<String> = bad.iter().take(3).map(|w| format!("{w}→{:?}", super::suggest(std::slice::from_ref(&d), w, 3))).collect();
            println!("{id}: unknown {bad:?} {sugg:?}");
        }
    }
}
