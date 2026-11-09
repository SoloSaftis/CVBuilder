//! Reading an existing CV without AI: the text of a PDF, Word, OpenDocument, RTF or text file
//! becomes a filled-in project. It takes what it can recognise with confidence – contact
//! details, the profile, jobs and education with their dates, skills, languages, courses,
//! interests and sections such as projects – and leaves the rest as text for the person (or the
//! AI) to place. Nothing is invented: every value is a piece of the CV's own text.
use std::sync::LazyLock;

use regex::Regex;

use crate::checks;
use crate::import::tidy;
use crate::model::{Certification, ContactField, CustomItem, CustomSection, Education, Interest, Job, Project, Skill, SpokenLanguage};

/// What could be read from an existing CV without AI, and the text that was not used.
#[derive(Debug, Clone, Default)]
pub struct CvRead {
    pub project: Project,
    /// The parts that could not be placed, for the person (or the AI) to sort.
    pub rest: String,
    /// What was found, as short Norwegian words («navn», «jobber» …) for the status line.
    pub found: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Top,
    Profile,
    Skills,
    Languages,
    Experience,
    Education,
    Certifications,
    Interests,
    /// A section of its own, such as projects or voluntary work: (Norwegian title, English title).
    Custom(&'static str, &'static str),
    /// Read but not used: references, personal details.
    Skip,
}

/// The headings CVs use, in Norwegian and English.
const HEADINGS: &[(&[&str], Part)] = &[
    (&["profil", "profile", "sammendrag", "summary", "om meg", "about me", "about", "personlig profil", "professional summary", "career summary", "objective", "career objective"], Part::Profile),
    (
        &[
            "kjernekompetanse",
            "kompetanse",
            "ferdigheter",
            "skills",
            "core skills",
            "key skills",
            "technical skills",
            "nøkkelkompetanse",
            "competencies",
            "core competencies",
            "expertise",
            "verktøy",
            "tools",
            "it-kompetanse",
        ],
        Part::Skills,
    ),
    (&["språk", "languages", "language skills", "språkkunnskaper", "language"], Part::Languages),
    (
        &[
            "arbeidserfaring",
            "erfaring",
            "experience",
            "work experience",
            "professional experience",
            "yrkeserfaring",
            "employment",
            "employment history",
            "work history",
            "relevant erfaring",
            "career history",
        ],
        Part::Experience,
    ),
    (&["utdanning", "education", "utdannelse", "academic background", "qualifications", "skolegang"], Part::Education),
    (
        &[
            "kurs",
            "sertifiseringer",
            "sertifikater",
            "kurs og sertifiseringer",
            "kurs og sertifikater",
            "kurs og kompetansebevis",
            "sertifiseringer og kurs",
            "certifications",
            "courses",
            "certificates",
            "certifications and courses",
            "courses and certifications",
            "courses & certifications",
            "certifications & courses",
            "licenses",
            "licences",
            "licenses and certifications",
            "training",
        ],
        Part::Certifications,
    ),
    (&["interesser", "interests", "hobbyer", "hobbies", "fritid", "fritidsinteresser"], Part::Interests),
    (&["prosjekter", "projects", "utvalgte prosjekter", "selected projects"], Part::Custom("Prosjekter", "Projects")),
    (&["frivillig arbeid", "frivillig", "volunteering", "volunteer", "volunteer experience", "community"], Part::Custom("Frivillig arbeid", "Volunteering")),
    (&["verv", "tillitsverv", "verv og styreverv", "positions of trust", "board positions"], Part::Custom("Verv og styreverv", "Positions of trust")),
    (&["publikasjoner", "publications"], Part::Custom("Publikasjoner", "Publications")),
    (&["priser", "utmerkelser", "priser og utmerkelser", "awards", "honours", "honors", "achievements"], Part::Custom("Priser og utmerkelser", "Awards")),
    (&["referanser", "references", "personalia", "kontakt", "contact", "kontaktinformasjon", "contact information", "personal details"], Part::Skip),
];

/// «S U M M A RY», «E X P E R I E N C E»: headings spaced out letter by letter.
fn unspace(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let single = words.iter().filter(|w| w.chars().count() == 1 && w.chars().all(char::is_alphabetic)).count();
    if words.len() >= 4 && single * 10 >= words.len() * 6 {
        // Words are separated by two or more spaces; letters by one.
        line.split("  ").map(|w| w.split_whitespace().collect::<String>()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
    } else {
        line.to_string()
    }
}

/// A CV section heading such as «Arbeidserfaring» or «Skills» (for the job-ad reader, which skips them).
pub(crate) fn is_heading(text: &str) -> bool {
    heading_of(text).is_some()
}

fn heading_of(text: &str) -> Option<Part> {
    let l = text.trim().trim_end_matches(':').trim().to_lowercase();
    if l.is_empty() || l.chars().count() > 40 {
        return None;
    }
    HEADINGS.iter().find(|(words, _)| words.contains(&l.as_str())).map(|(_, part)| *part)
}

/// A heading alone on its line, or at the start of a line with the section's first words after
/// it («Experience Cedar Cloud Austin, TX»), as two-column PDFs often give. Returns the part and
/// what follows the heading.
fn heading_line(line: &str) -> Option<(Part, String)> {
    if let Some(part) = heading_of(line) {
        return Some((part, String::new()));
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    let starts_upper = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    for n in (1..=3).rev() {
        if words.len() <= n || !starts_upper(words[0]) || !starts_upper(words[n]) {
            continue;
        }
        let head = words[..n].join(" ").to_lowercase();
        if !PREFIX_HEADINGS.contains(&head.trim_end_matches(':')) {
            continue;
        }
        if let Some(part) = heading_of(&head) {
            // «Profile Senior Software Engineer with …» is a heading; «Experience with Excel» is not.
            return Some((part, words[n..].join(" ")));
        }
    }
    None
}

/// Headings that may share their line with the section's first words. Words such as «Community»
/// or «Training» also start names («Community Skills Network»), so they must stand alone.
const PREFIX_HEADINGS: &[&str] = &[
    "profil",
    "profile",
    "summary",
    "sammendrag",
    "experience",
    "work experience",
    "erfaring",
    "arbeidserfaring",
    "education",
    "utdanning",
    "skills",
    "kompetanse",
    "ferdigheter",
    "projects",
    "prosjekter",
    "languages",
    "språk",
    "certifications",
    "interests",
    "interesser",
];

// ---------- dates ----------

const DATE: &str = r"(?:\d{1,2}\s*[./-]\s*(?:19|20)\d{2}|(?:19|20)\d{2}\s*[./-]\s*\d{1,2}|\b(?:jan(?:uar|uary)?|feb(?:ruar|ruary)?|mar(?:s|ch)?|apr(?:il)?|mai|may|jun(?:i|e)?|jul(?:i|y)?|aug(?:ust)?|sep(?:t|tember)?|okt(?:ober)?|oct(?:ober)?|nov(?:ember)?|des(?:ember)?|dec(?:ember)?)\.?\s+(?:19|20)\d{2}|(?:19|20)\d{2})";
const NOW: &str = r"(?:present|current|now|today|ongoing|nå|naa|d\.\s?d\.?|dd|pågående|i dag|dags dato)";

/// A date range at the end of a line: «2019 – 2022», «03.2021 – nå», «Jan 2020 - Present».
static RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\(?\s*({DATE})\s*(?:–|—|-|−|to|til)\s*({DATE}|{NOW})\s*\)?\s*$")).expect("regex"));
/// One date or year at the end of a line: «… 2024».
static SINGLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)(?:^|[\s,·|(])({DATE})\)?\s*$")).expect("regex"));

/// The text before a date range at the end of the line, the start, and the end (`None` = now).
fn split_range(line: &str) -> Option<(String, String, Option<String>)> {
    let c = RANGE.captures(line)?;
    let whole = c.get(0)?;
    let before = clean_field(&line[..whole.start()]);
    let start = month_or_year(&c[1])?;
    let end = if checks::is_now(&c[2].replace('.', "")) || Regex::new(&format!("(?i)^{NOW}$")).ok()?.is_match(c[2].trim()) { None } else { Some(month_or_year(&c[2])?) };
    Some((before, start, end))
}

fn month_or_year(text: &str) -> Option<String> {
    let t = text.trim().trim_end_matches('.');
    checks::parse_month(t).filter(|m| !m.is_empty()).or_else(|| checks::parse_year(t).filter(|y| !y.is_empty()))
}

fn clean_field(text: &str) -> String {
    text.trim().trim_end_matches([',', '|', '·', '–', '-', '—', ':', '(']).trim().to_string()
}

// ---------- lines ----------

const BULLETS: [char; 9] = ['•', '·', '▪', '◦', '‣', '–', '-', '*', '►'];

fn is_bullet(line: &str) -> bool {
    line.trim_start().starts_with(BULLETS)
}

fn unbullet(line: &str) -> &str {
    line.trim().trim_start_matches(BULLETS).trim()
}

/// A short line that can be an employer, a school or a title: a few words, not a sentence.
fn is_label(line: &str) -> bool {
    let t = line.trim();
    let words = t.split_whitespace().count();
    !t.is_empty() && !is_bullet(t) && (1..=if t.contains(" | ") { 18 } else { 9 }).contains(&words) && !t.ends_with(['.', '!', '?', ';']) && heading_line(t).is_none()
}

/// A trailing place on an employer line: «Northstar Health London, UK», «Fjellvegg Bygg AS\tTrondheim».
fn split_place(line: &str, home: &str) -> (String, String) {
    static COUNTRYISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([A-ZÆØÅ][\p{L}.'\-]+,\s?[A-ZÆØÅ][\p{L}.\- ]{1,20})$").expect("regex"));
    let t = line.trim();
    // Most jobs are where the person lives: «… Civic Works Digital New York, NY».
    if !home.is_empty() && t.len() > home.len() + 2 && t.ends_with(home) {
        return (clean_field(&t[..t.len() - home.len()]), home.to_string());
    }
    for sep in ['\t', '|', '·'] {
        if let Some((a, b)) = t.rsplit_once(sep) {
            let (a, b) = (a.trim(), b.trim());
            if !a.is_empty() && !b.is_empty() && b.split_whitespace().count() <= 4 && !b.chars().any(|c| c.is_ascii_digit()) {
                return (a.to_string(), b.to_string());
            }
        }
    }
    if let Some((a, b)) = t.split_once("   ") {
        return (a.trim().to_string(), b.trim().to_string());
    }
    if let Some(c) = COUNTRYISH.captures(t) {
        let m = c.get(0).map(|m| m.start()).unwrap_or(t.len());
        return (t[..m].trim().to_string(), c[1].trim().to_string());
    }
    // The person's own town at the end of the line («… AS (fiktiv) Trondheim»).
    let town = home.split(',').next().unwrap_or("").trim();
    if !town.is_empty() && t.len() > town.len() + 2 && t.ends_with(town) {
        return (clean_field(&t[..t.len() - town.len()]), town.to_string());
    }
    (t.to_string(), String::new())
}

/// Bullet points from the lines under an entry. Lines that carry on a sentence are joined;
/// a Word file's list gives no bullet marks, so each finished sentence starts a new point.
fn bullets(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let starts_new = is_bullet(t) || out.last().is_none_or(|last| last.ends_with(['.', '!', '?', ':']) || t.chars().next().is_some_and(char::is_uppercase) && !last.ends_with(','));
        let text = unbullet(t);
        if text.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if !starts_new => {
                if last.ends_with('-') && !last.ends_with(" -") {
                    last.pop();
                    last.push_str(text);
                } else {
                    last.push(' ');
                    last.push_str(text);
                }
            }
            _ => out.push(text.to_string()),
        }
    }
    out
}

/// One dated entry in a section: the title on the date line (or a line by it), an employer or
/// school on the line before or after, and the lines under it.
#[derive(Debug, Default)]
struct Entry {
    on_date_line: String,
    labels: Vec<String>,
    start: String,
    end: Option<String>,
    body: Vec<String>,
}

/// Splits a section into dated entries. Lines before the first date belong to it (an employer
/// line above the title), and a short line right after a date line is its employer or school.
fn entries(lines: &[&str], max_above: usize) -> (Vec<Entry>, Vec<String>) {
    let lines: Vec<&str> = lines.iter().map(|l| l.trim()).collect();
    let dated: Vec<usize> =
        (0..lines.len()).filter(|&i| !is_bullet(lines[i]) && lines[i].split_whitespace().count() <= if lines[i].contains(" | ") { 26 } else { 14 } && split_range(lines[i]).is_some()).collect();
    if dated.is_empty() {
        return (Vec::new(), lines.iter().filter(|l| !l.is_empty()).map(|l| l.to_string()).collect());
    }
    let mut out = Vec::new();
    let mut used = vec![false; lines.len()];
    // Labels around each date line.
    let mut spans = Vec::new();
    for (k, &i) in dated.iter().enumerate() {
        let (before, start, end) = split_range(lines[i]).unwrap_or_default();
        used[i] = true;
        let mut e = Entry { on_date_line: before, start, end, ..Default::default() };
        // Up to two label lines right above, if they are not the previous entry's body.
        let floor = if k == 0 { 0 } else { dated[k - 1] + 1 };
        let mut above = Vec::new();
        let mut j = i;
        while j > floor && above.len() < max_above {
            j -= 1;
            if lines[j].is_empty() {
                if above.is_empty() {
                    continue;
                }
                break;
            }
            if !is_label(lines[j]) || used[j] {
                break;
            }
            // Under the previous entry, a short line just after its date line is that entry's employer.
            if k > 0 && j == dated[k - 1] + 1 {
                break;
            }
            above.push(j);
        }
        above.reverse();
        for &a in &above {
            used[a] = true;
            e.labels.push(lines[a].to_string());
        }
        // One label line right below.
        if let Some(n) = (i + 1..lines.len()).find(|&n| !lines[n].is_empty()) {
            let next_is_date = dated.get(k + 1) == Some(&n);
            // A line whose sentence carries on below («Mentored emerging designers / through …») is body, not a label.
            let carries_on = lines.get(n + 1).is_some_and(|l| l.trim_start().chars().next().is_some_and(char::is_lowercase));
            if !next_is_date && !carries_on && is_label(lines[n]) && !used[n] && (e.labels.is_empty() || e.on_date_line.is_empty() && e.labels.len() < 2) {
                // Not the next entry's label: that one sits right above the next date line.
                let next_date = dated.get(k + 1).copied().unwrap_or(lines.len());
                let only_labels_until_next = (n + 1..next_date).all(|m| lines[m].is_empty());
                if !(only_labels_until_next && next_date < lines.len()) {
                    used[n] = true;
                    e.labels.push(lines[n].to_string());
                }
            }
        }
        spans.push(e);
    }
    // Bodies: the unused lines up to the next entry.
    let starts: Vec<usize> = dated.clone();
    for (k, mut e) in spans.into_iter().enumerate() {
        let from = starts[k] + 1;
        let to = starts.get(k + 1).copied().unwrap_or(lines.len());
        e.body = (from..to).filter(|&m| !used[m] && !lines[m].is_empty()).map(|m| lines[m].to_string()).collect();
        // The next entry's label lines were taken above; anything left between is body.
        for m in from..to {
            used[m] = true;
        }
        out.push(e);
    }
    let before_first: Vec<String> = (0..dated[0]).filter(|&m| !used[m] && !lines[m].is_empty()).map(|m| lines[m].to_string()).collect();
    (out, before_first)
}

/// Splits «Senior Designer, Acme» or «Konsulent hos Acme» into title and employer.
fn title_and_employer(text: &str) -> (String, String) {
    for sep in [" hos ", " at ", " @ ", " – ", " — ", " | ", ", ", " - "] {
        if let Some((a, b)) = text.split_once(sep) {
            if !a.trim().is_empty() && !b.trim().is_empty() {
                return (a.trim().to_string(), b.trim().to_string());
            }
        }
    }
    (text.to_string(), String::new())
}

fn read_jobs(lines: &[&str], home: &str) -> (Vec<Job>, Vec<String>) {
    let (entries, loose) = entries(lines, 2);
    let mut jobs = Vec::new();
    for (n, e) in entries.into_iter().enumerate() {
        let mut title = e.on_date_line.clone();
        let mut employer_line = String::new();
        let mut labels = e.labels.clone();
        if title.is_empty() && !labels.is_empty() {
            title = labels.pop().unwrap_or_default();
        }
        if let Some(l) = labels.first() {
            employer_line = l.clone();
        }
        let (mut company, mut location) = split_place(&employer_line, home);
        if company.is_empty() {
            let (t, c) = title_and_employer(&title);
            if !c.is_empty() {
                title = t;
                let (c, l) = split_place(&c, home);
                company = c;
                location = l;
            }
        }
        let body: Vec<&str> = e.body.iter().map(String::as_str).collect();
        jobs.push(Job { id: format!("job_imported_{}", n + 1), company, title, location, start: e.start, end: e.end, bullets: bullets(&body), evidence: String::new(), confirmed: true });
    }
    (jobs, loose)
}

const SCHOOL_WORDS: &[&str] = &[
    "univers",
    "université",
    "hochschule",
    "college",
    "høgskole",
    "høgskulen",
    "høyskole",
    "høyskolen",
    "school",
    "skole",
    "skolen",
    "institute",
    "institutt",
    "academy",
    "akademi",
    "akademiet",
    "fagskole",
    "fagskolen",
    "gymnas",
    "videregående",
    "polytechnic",
    "business school",
    "ntnu",
    "uio",
    "uib",
    "oslomet",
    "nmbu",
    "uit",
    "bi ",
];
const DEGREES: &[&str] =
    &["BA", "BSc", "BS", "BBA", "BEng", "BEd", "LLB", "MA", "MSc", "MS", "MBA", "MEng", "MEd", "LLM", "MPhil", "PhD", "DPhil", "Bachelor", "Master", "Cand.", "Fagbrev", "Svennebrev"];

/// A line that is a degree, or starts with one: «MA», «BSc Computer Science».
fn is_degree(text: &str) -> bool {
    text.split_whitespace().next().is_some_and(|w| DEGREES.iter().any(|d| d.eq_ignore_ascii_case(w.trim_end_matches([',', '.']))))
}

fn is_school(text: &str) -> bool {
    let l = format!("{} ", text.to_lowercase());
    SCHOOL_WORDS.iter().any(|w| l.contains(w))
}

fn read_education(lines: &[&str]) -> (Vec<Education>, Vec<String>) {
    let (entries, loose) = entries(lines, 5);
    let mut out = Vec::new();
    for e in entries {
        let mut parts: Vec<String> = e.labels.clone();
        if !e.on_date_line.is_empty() {
            parts.extend(e.on_date_line.split(" | ").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        }
        // A short line under the date that is a school («Eksempel høgskole») belongs to the entry.
        let mut body = e.body.clone();
        if let Some(first) = body.first().cloned() {
            if is_label(&first) && (is_school(&first) || parts.len() < 2) {
                parts.push(first);
                body.remove(0);
            }
        }
        let mut institution = String::new();
        let mut qualification = Vec::new();
        // «University of the Arts / London / MA / Brand Communication»: the school, then the degree.
        if let Some(d) = parts.iter().position(|p| is_degree(p)).filter(|&d| d > 0 && parts[..d].iter().any(|p| is_school(p))) {
            let start = checks::parse_year(&e.start).unwrap_or_default();
            let end = e.end.as_deref().and_then(|v| checks::parse_year(v)).unwrap_or_default();
            out.push(Education { qualification: parts[d..].join(" "), institution: parts[..d].join(" "), start, end });
            continue;
        }
        for p in &parts {
            if institution.is_empty() && is_school(p) {
                // «University of Brighton BA»: the degree at the end goes to the qualification.
                let mut words: Vec<&str> = p.split_whitespace().collect();
                if let Some(last) = words.last() {
                    if DEGREES.iter().any(|d| d.eq_ignore_ascii_case(last.trim_end_matches(','))) {
                        qualification.insert(0, last.to_string());
                        words.pop();
                    }
                }
                institution = words.join(" ");
            } else {
                qualification.push(p.clone());
            }
        }
        if institution.is_empty() && qualification.len() >= 2 {
            institution = qualification.pop().unwrap_or_default();
        }
        let start = checks::parse_year(&e.start).unwrap_or_default();
        let end = e.end.as_deref().and_then(|v| checks::parse_year(v)).unwrap_or_default();
        out.push(Education { qualification: qualification.join(" "), institution, start, end });
    }
    (out, loose)
}

fn read_skills(lines: &[&str]) -> Vec<Skill> {
    // A line ending with a comma carries on below.
    let mut joined: Vec<String> = Vec::new();
    for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        match joined.last_mut() {
            Some(last) if last.ends_with(',') || l.chars().next().is_some_and(char::is_lowercase) => {
                last.push(' ');
                last.push_str(l);
            }
            _ => joined.push(l.to_string()),
        }
    }
    let mut out = Vec::new();
    let mut group = String::new();
    for (i, line) in joined.iter().enumerate() {
        let text = unbullet(line);
        // «Prosjekt: fremdriftsplaner, innkjøp …»
        if let Some((g, rest)) = text.split_once(':') {
            if g.split_whitespace().count() <= 4 && !rest.trim().is_empty() {
                group = g.trim().to_string();
                push_skills(&mut out, &group, rest);
                continue;
            }
        }
        // A short line without commas over a list is the list's group name.
        let next_is_list = joined.get(i + 1).is_some_and(|n| n.contains(','));
        if !text.contains(',') && text.split_whitespace().count() <= 4 && next_is_list {
            group = text.to_string();
            continue;
        }
        push_skills(&mut out, &group, text);
    }
    out
}

fn push_skills(out: &mut Vec<Skill>, group: &str, text: &str) {
    for item in text.split([',', '•', '·', ';', '|']).map(|s| s.trim().trim_start_matches(['-', '*', '–']).trim()) {
        if (2..=60).contains(&item.chars().count()) && !out.iter().any(|s: &Skill| s.name.eq_ignore_ascii_case(item)) {
            out.push(Skill { group: group.to_string(), name: item.to_string(), evidence: String::new(), confirmed: true });
        }
    }
}

const LEVELS: &[&str] = &[
    "native",
    "fluent",
    "conversational",
    "basic",
    "beginner",
    "elementary",
    "intermediate",
    "advanced",
    "proficient",
    "professional",
    "bilingual",
    "good",
    "working",
    "limited",
    "full",
    "morsmål",
    "flytende",
    "god",
    "gode",
    "godt",
    "grunnleggende",
    "muntlig",
    "skriftlig",
    "middels",
    "noe",
    "litt",
    "a1",
    "a2",
    "b1",
    "b2",
    "c1",
    "c2",
];

fn is_level(word: &str) -> bool {
    LEVELS.contains(&word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase().as_str())
}

/// Language names in English and Norwegian, so a language list cannot swallow other text.
const LANGUAGE_NAMES: &[&str] = &[
    "norwegian",
    "norsk",
    "bokmål",
    "nynorsk",
    "english",
    "engelsk",
    "swedish",
    "svensk",
    "danish",
    "dansk",
    "finnish",
    "finsk",
    "icelandic",
    "islandsk",
    "german",
    "tysk",
    "french",
    "fransk",
    "spanish",
    "spansk",
    "portuguese",
    "portugisisk",
    "italian",
    "italiensk",
    "dutch",
    "nederlandsk",
    "polish",
    "polsk",
    "czech",
    "tsjekkisk",
    "slovak",
    "slovakisk",
    "hungarian",
    "ungarsk",
    "romanian",
    "rumensk",
    "bulgarian",
    "bulgarsk",
    "greek",
    "gresk",
    "turkish",
    "tyrkisk",
    "russian",
    "russisk",
    "ukrainian",
    "ukrainsk",
    "lithuanian",
    "litauisk",
    "latvian",
    "latvisk",
    "estonian",
    "estisk",
    "serbian",
    "serbisk",
    "croatian",
    "kroatisk",
    "bosnian",
    "bosnisk",
    "slovenian",
    "slovensk",
    "albanian",
    "albansk",
    "arabic",
    "arabisk",
    "hebrew",
    "hebraisk",
    "persian",
    "farsi",
    "persisk",
    "urdu",
    "hindi",
    "punjabi",
    "bengali",
    "tamil",
    "chinese",
    "kinesisk",
    "mandarin",
    "cantonese",
    "kantonesisk",
    "japanese",
    "japansk",
    "korean",
    "koreansk",
    "vietnamese",
    "vietnamesisk",
    "thai",
    "thailandsk",
    "tagalog",
    "filipino",
    "indonesian",
    "indonesisk",
    "malay",
    "malayisk",
    "somali",
    "somalisk",
    "tigrinya",
    "tigrinja",
    "amharic",
    "amharisk",
    "swahili",
    "kurdish",
    "kurdisk",
    "pashto",
    "dari",
    "sami",
    "samisk",
    "nordsamisk",
    "kven",
    "kvensk",
    "sign language",
    "tegnspråk",
    "norsk tegnspråk",
    "catalan",
    "katalansk",
    "irish",
    "irsk",
    "welsh",
    "walisisk",
    "afrikaans",
];

fn is_language_name(text: &str) -> bool {
    let l = text.trim().to_lowercase();
    LANGUAGE_NAMES.contains(&l.as_str())
}

fn read_languages(lines: &[&str]) -> Vec<SpokenLanguage> {
    let text = lines.join("\n");
    let mut out: Vec<SpokenLanguage> = Vec::new();
    let items: Vec<&str> = text.split(['\n', '·', ',', ';', '|', '•']).map(str::trim).filter(|s| !s.is_empty()).collect();
    // Languages that share the next one's level («Norwegian and / English fluent»), over lines.
    let mut shared: Vec<usize> = Vec::new();
    let share = |out: &mut Vec<SpokenLanguage>, shared: &[usize]| {
        for &i in shared.iter().rev() {
            if let Some(level) = out.get(i + 1).map(|l| l.level.clone()) {
                if out[i].level.is_empty() {
                    out[i].level = level;
                }
            }
        }
    };
    for item in items {
        if let Some((name, level)) = item.split_once([':', '(']).or_else(|| item.split_once(" – ")).or_else(|| item.split_once(" - ")) {
            let level = level.trim().trim_end_matches(')').trim();
            // «Norsk og engelsk: flytende» gives two languages with the same level.
            let names: Vec<&str> = name.split(" og ").flat_map(|n| n.split(" and ")).map(str::trim).collect();
            if names.iter().all(|n| is_language_name(n)) {
                for n in names {
                    out.push(SpokenLanguage { name: capitalise(n), level: level.to_string() });
                }
            }
            continue;
        }
        // «English Fluent Dutch Conversational» from a two-column PDF: language, then its level.
        // «Norwegian and English fluent written and spoken»: both languages share the level.
        let words: Vec<&str> = item.split_whitespace().collect();
        let mut k = 0;
        while k < words.len() {
            let word = words[k];
            let bare = word.trim_matches(|c: char| !c.is_alphabetic());
            let is_and = matches!(bare.to_lowercase().as_str(), "and" | "og") || word == "&";
            // The next language may be on the next line.
            let next_is_language = words.get(k + 1).is_none_or(|w| is_language_name(w.trim_matches(|c: char| !c.is_alphabetic())));
            if is_and && next_is_language && out.last().is_some_and(|l| l.level.is_empty()) {
                shared.push(out.len() - 1);
            } else if is_language_name(bare) {
                out.push(SpokenLanguage { name: capitalise(bare), level: String::new() });
            } else if is_level(word) || !word.chars().any(char::is_uppercase) && out.last().is_some_and(|l| !l.level.is_empty() && l.level.split_whitespace().count() < 5) {
                // A level, or the rest of one («written and spoken»).
                if let Some(last) = out.last_mut() {
                    if !last.level.is_empty() {
                        last.level.push(' ');
                    }
                    last.level.push_str(word);
                }
            } else {
                // Not a language: this is not a language list (a two-column layout mixed it up).
                break;
            }
            k += 1;
        }
        if k < words.len() {
            share(&mut out, &shared);
            return out;
        }
    }
    share(&mut out, &shared);
    out
}

fn capitalise(word: &str) -> String {
    let mut c = word.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn read_certifications(lines: &[&str]) -> Vec<Certification> {
    // Entries are split by blank lines, or each line is one when every line has a year.
    let mut groups: Vec<Vec<&str>> = vec![Vec::new()];
    for l in lines {
        if l.trim().is_empty() {
            if groups.last().is_some_and(|g| !g.is_empty()) {
                groups.push(Vec::new());
            }
        } else if let Some(g) = groups.last_mut() {
            g.push(l.trim());
        }
    }
    groups.retain(|g| !g.is_empty());
    if groups.len() == 1 && groups[0].len() > 1 && groups[0].iter().all(|l| SINGLE.is_match(l) || l.contains('·')) {
        groups = groups[0].iter().map(|l| vec![*l]).collect();
    }
    let mut out = Vec::new();
    for g in groups {
        let text = g.join("\n");
        // «Prosjektledelse, grunnkurs · Eksempel kursleverandør · 2022»
        let year_at = g.iter().position(|l| SINGLE.is_match(l));
        let (name, mut issuer, year);
        if let Some(i) = year_at {
            let line = g[i];
            let c = SINGLE.captures(line).expect("matched");
            year = checks::parse_year(&c[1]).unwrap_or_default();
            let before_year = clean_field(&line[..c.get(0).map(|m| m.start()).unwrap_or(0)]);
            let mut head: Vec<String> = g[..i].iter().map(|s| s.to_string()).collect();
            if !before_year.is_empty() {
                head.push(before_year);
            }
            let head = head.join(" ");
            let mut pieces = head.split(['·', '|']).map(str::trim).filter(|s| !s.is_empty());
            name = pieces.next().unwrap_or("").to_string();
            issuer = pieces.collect::<Vec<_>>().join(", ");
            if issuer.is_empty() {
                issuer = g[i + 1..].join(" ");
            }
        } else {
            let mut pieces = text.split(['·', '|', '\n']).map(str::trim).filter(|s| !s.is_empty());
            name = pieces.next().unwrap_or("").to_string();
            issuer = pieces.collect::<Vec<_>>().join(", ");
            year = String::new();
        }
        if (2..=120).contains(&name.chars().count()) {
            out.push(Certification { name, issuer, year });
        }
    }
    out
}

fn read_interests(lines: &[&str]) -> Vec<Interest> {
    let mut out = Vec::new();
    for item in lines.join("\n").split(['\n', '·', ',', ';', '•', '|']).map(|s| unbullet(s).trim()) {
        if (2..=60).contains(&item.chars().count()) {
            out.push(Interest { id: format!("interest_imported_{}", out.len() + 1), label: item.to_string(), category: "other".into(), specificity: 1, confirmed: true, ..Default::default() });
        }
    }
    out
}

fn read_custom(lines: &[&str], id: &str, title: &str) -> Option<CustomSection> {
    let mut items = Vec::new();
    // Dated items first; then single-year lines («Open Metrics Toolkit 2025»).
    let (entries, loose) = entries(lines, 2);
    for e in entries {
        let (title, subtitle) = if e.on_date_line.is_empty() {
            (e.labels.first().cloned().unwrap_or_default(), e.labels.get(1).cloned().unwrap_or_default())
        } else {
            (e.on_date_line.clone(), e.labels.first().cloned().unwrap_or_default())
        };
        let end = e.end.as_deref().map(checks::show_month).unwrap_or_else(|| "nå".to_string());
        let body: Vec<&str> = e.body.iter().map(String::as_str).collect();
        items.push(CustomItem { title, subtitle, date: format!("{} – {end}", checks::show_month(&e.start)), bullets: bullets(&body) });
    }
    let mut current: Option<CustomItem> = None;
    for line in &loose {
        let single = (!is_bullet(line) && line.split_whitespace().count() <= 12).then(|| SINGLE.captures(line)).flatten();
        if let Some(c) = single {
            if let Some(item) = current.take() {
                items.push(item);
            }
            let at = c.get(0).map(|m| m.start()).unwrap_or(0);
            current = Some(CustomItem { title: clean_field(&line[..at]), date: checks::parse_year(&c[1]).unwrap_or_default(), ..Default::default() });
        } else if let Some(item) = current.as_mut() {
            let mut b = item.bullets.clone();
            b.push(line.clone());
            let refs: Vec<&str> = b.iter().map(String::as_str).collect();
            item.bullets = bullets(&refs);
        } else {
            current = Some(CustomItem { title: unbullet(line).to_string(), ..Default::default() });
        }
    }
    if let Some(item) = current {
        items.push(item);
    }
    items.retain(|i| !i.title.is_empty() || !i.bullets.is_empty());
    (!items.is_empty()).then(|| CustomSection { id: id.to_string(), title: title.to_string(), items })
}

// ---------- the top of the CV ----------

static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("regex"));
static PHONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:\+\d{1,3}[\s.\-]?)?(?:\(\d{1,4}\)[\s.\-]?)?(?:\d[\s.\-]?){6,11}\d").expect("regex"));
static LINKEDIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:https?://)?(?:[a-z]{2,3}\.)?linkedin\.com/in/[A-Za-z0-9_\-%]+/?").expect("regex"));
static WEB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:https?://)?(?:www\.)?[a-z0-9\-]+(?:\.[a-z0-9\-]+)*\.(?:com|no|net|org|io|dev|me|se|dk|uk|de|app|page|site|info|co)(?:/[^\s·|,]*)?").expect("regex"));

/// Words that make a short line a job title rather than a name.
const TITLE_WORDS: &[&str] = &[
    "senior",
    "junior",
    "lead",
    "head",
    "chief",
    "manager",
    "designer",
    "engineer",
    "developer",
    "consultant",
    "coordinator",
    "specialist",
    "analyst",
    "director",
    "officer",
    "assistant",
    "advisor",
    "adviser",
    "architect",
    "scientist",
    "nurse",
    "teacher",
    "accountant",
    "administrator",
    "technician",
    "rådgiver",
    "leder",
    "konsulent",
    "utvikler",
    "ingeniør",
    "koordinator",
    "medarbeider",
    "sykepleier",
    "lærer",
    "daglig",
    "prosjektleder",
    "selger",
    "tekniker",
    "operatør",
];

fn looks_like_name(line: &str) -> bool {
    let t = line.trim();
    let words = t.split_whitespace().count();
    (2..=5).contains(&words) && !t.contains('@') && !t.chars().any(|c| c.is_ascii_digit()) && heading_line(t).is_none() && t.chars().all(|c| c.is_alphabetic() || " -.'".contains(c))
}

fn has_title_word(line: &str) -> bool {
    line.to_lowercase().split_whitespace().any(|w| TITLE_WORDS.contains(&w))
}

/// A page number or running header: «1 / 2», «Side 1 av 2», «Ingrid Haugland 1 / 1», «VebjørnJohannessen1/2».
fn is_page_mark(line: &str) -> bool {
    static COUNTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^([\p{L} .'\-]{0,60}?)\s*(?:side|page|s\.)?\s*\d{1,2}\s*(?:/|av|of)\s*\d{1,2}\s*$").expect("regex"));
    COUNTER.captures(line.trim()).is_some_and(|c| c[1].split_whitespace().count() <= 5)
}

/// «LUCAS MARTINS» → «Lucas Martins»; names already in mixed case are kept.
fn name_case(name: &str) -> String {
    if name.chars().any(char::is_lowercase) {
        return name.to_string();
    }
    name.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_contact_line(line: &str) -> bool {
    EMAIL.is_match(line)
        || PHONE.find_iter(line).any(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8)
        || LINKEDIN.is_match(line)
        || WEB.is_match(line) && line.split_whitespace().count() <= 6
}

/// The town on a contact line once the e-mail, phone and links are taken out.
fn town_from(line: &str) -> Option<String> {
    let mut rest = line.to_string();
    for re in [&*EMAIL, &*LINKEDIN, &*WEB, &*PHONE] {
        rest = re.replace_all(&rest, " | ").to_string();
    }
    rest.split(['|', '·', '•', '\t'])
        .map(str::trim)
        .find(|s| {
            let n = s.chars().count();
            (2..=40).contains(&n) && s.chars().all(|c| c.is_alphabetic() || " ,.-'".contains(c)) && s.chars().next().is_some_and(char::is_uppercase) && s.split_whitespace().count() <= 4
        })
        .map(|s| s.trim_end_matches(',').to_string())
}

/// Reads an existing CV's text without AI.
pub fn read_cv(text: &str) -> CvRead {
    let text = tidy(text);
    let mut lines: Vec<String> = text.lines().map(unspace).filter(|l| !is_page_mark(l)).collect();
    let mut p = Project::default();
    let mut found: Vec<&str> = Vec::new();

    // Contact details anywhere near the top (two-column PDFs put them in odd places).
    let c = &mut p.candidate;
    if let Some(m) = EMAIL.find(&text) {
        c.email = m.as_str().to_string();
        found.push("e-post");
    }
    if let Some(m) = PHONE.find_iter(&text).find(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8 && !m.as_str().contains('.') && !RANGE.is_match(m.as_str())) {
        c.phone = m.as_str().trim().to_string();
        found.push("telefon");
    }
    if let Some(m) = LINKEDIN.find(&text) {
        let url = m.as_str().trim_end_matches('/');
        c.linkedin = if url.starts_with("http") { url.to_string() } else { format!("https://{url}") };
        found.push("LinkedIn");
    }
    // Lines with the e-mail, phone or LinkedIn, wherever the layout put them, and links right by them.
    let personal = |l: &str| EMAIL.is_match(l) || LINKEDIN.is_match(l) || PHONE.find_iter(l).any(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8 && !RANGE.is_match(l));
    let anchors: Vec<usize> = (0..lines.len()).filter(|&i| personal(&lines[i]) && heading_line(&lines[i]).is_none_or(|(_, rest)| rest.is_empty())).collect();
    let contact_lines: Vec<usize> =
        (0..lines.len()).filter(|&i| anchors.contains(&i) || is_contact_line(&lines[i]) && lines[i].split_whitespace().count() <= 6 && anchors.iter().any(|&a| a.abs_diff(i) <= 2)).collect();
    let near_contact: Vec<usize> = match (contact_lines.first(), contact_lines.last()) {
        (Some(&a), Some(&b)) => (a.saturating_sub(2)..(b + 3).min(lines.len())).collect(),
        _ => Vec::new(),
    };
    let town_line = |i: &usize| {
        let l = lines[*i].trim();
        let alone = l.contains(',') && l.split_whitespace().count() <= 4 && l.chars().all(|c| c.is_alphabetic() || " ,.-'".contains(c));
        if contact_lines.contains(i) || alone { town_from(l) } else { None }
    };
    let mut taken: Vec<usize> = contact_lines.clone();
    if let Some((i, town)) = near_contact.iter().find_map(|i| town_line(i).map(|t| (*i, t))) {
        taken.push(i);
        c.city = town;
        found.push("sted");
    }
    if let Some(web) = contact_lines.iter().find_map(|&i| {
        WEB.find_iter(&lines[i]).map(|m| m.as_str().to_string()).find(|w| !w.to_lowercase().contains("linkedin") && !c.email.ends_with(w.trim_start_matches("www.")) && !c.email.contains(w.as_str()))
    }) {
        c.extra.push(ContactField { id: "web".into(), label: String::new(), value: web });
        found.push("nettside");
    }

    // The name: a short line near the top; a job title above it («SENIOR CONTENT DESIGNER») is skipped.
    let first_contact = contact_lines.first().copied().unwrap_or(lines.len().min(10));
    let near: Vec<usize> = (0..lines.len().min(12)).filter(|&i| looks_like_name(&lines[i])).collect();
    // In a two-column PDF the name can come after a sidebar: a name line among the few lines just
    // above the contact lines wins.
    let above_contact: Vec<usize> = (0..first_contact).rev().filter(|&i| !lines[i].trim().is_empty()).take(3).collect();
    let by_contact = above_contact.iter().copied().filter(|&i| looks_like_name(&lines[i]) && !has_title_word(&lines[i])).max();
    let name_at = by_contact.or(match near.as_slice() {
        [a, b, ..] if has_title_word(&lines[*a]) && !has_title_word(&lines[*b]) => Some(*b),
        [a, ..] => Some(*a),
        [] => (first_contact.saturating_sub(3)..first_contact).rev().find(|&i| looks_like_name(&lines[i]) && !has_title_word(&lines[i])),
    });
    if let Some(i) = name_at {
        c.name = name_case(lines[i].trim());
        found.push("navn");
        taken.push(i);
        // The headline: the short line under the name, or a title line just above it.
        let below = (i + 1..lines.len().min(i + 3)).find(|&j| !lines[j].trim().is_empty());
        let above = i.checked_sub(1).filter(|&j| has_title_word(&lines[j]) && looks_like_name(&lines[j]));
        if let Some(j) = below.filter(|&j| is_label(&lines[j]) && !is_contact_line(&lines[j]) && !lines[j].chars().any(|c| c.is_ascii_digit())).or(above) {
            c.headline = name_case(lines[j].trim());
            found.push("tittel");
            taken.push(j);
        }
    }
    // A page header repeats the name and title on every page; those lines are not part of a section.
    let (name, headline) = (p.candidate.name.to_lowercase(), p.candidate.headline.to_lowercase());
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim().to_lowercase();
        if !t.is_empty() && (t == name || !headline.is_empty() && t == headline) {
            taken.push(i);
        }
    }
    // The name, title and contact lines are read; they are not part of any section below.
    for i in taken {
        if let Some(l) = lines.get_mut(i) {
            l.clear();
        }
    }

    // Sections.
    let mut parts: Vec<(Part, Vec<String>)> = vec![(Part::Top, Vec::new())];
    for line in &lines {
        if let Some((part, rest)) = heading_line(line) {
            let in_skills = parts.last().is_some_and(|(p, b)| *p == Part::Skills && !b.iter().all(|l| l.trim().is_empty()));
            if part == Part::Skills && in_skills && rest.is_empty() {
                if let Some(last) = parts.last_mut() {
                    last.1.push(line.trim().to_string());
                }
                continue;
            }
            parts.push((part, if rest.is_empty() { Vec::new() } else { vec![rest] }));
        } else if let Some(last) = parts.last_mut() {
            last.1.push(line.clone());
        }
    }
    let home = p.candidate.city.clone();
    let mut rest = Vec::new();
    let mut leftover = |title: &str, lines: Vec<String>| {
        let body = tidy(&lines.join("\n"));
        if !body.trim().is_empty() {
            rest.push(format!("{}\n{body}", title.to_uppercase()));
        }
    };
    for (part, body) in parts {
        let refs: Vec<&str> = body.iter().map(String::as_str).collect();
        if refs.iter().all(|l| l.trim().is_empty()) {
            continue;
        }
        match part {
            Part::Profile if p.candidate.summary.is_empty() => {
                p.candidate.summary = join_lines(&refs);
                found.push("profil");
            }
            Part::Top => {
                // A paragraph under the contact lines with no heading is the profile.
                let para: Vec<&str> = refs.iter().skip(first_contact + 1).map(|l| l.trim()).filter(|l| !l.is_empty() && !is_contact_line(l)).collect();
                let words: usize = para.iter().map(|l| l.split_whitespace().count()).sum();
                if p.candidate.summary.is_empty() && words >= 25 && para.iter().all(|l| l.split_whitespace().count() >= 4) {
                    p.candidate.summary = join_lines(&para);
                    found.push("profil");
                }
            }
            Part::Skills => {
                let skills = read_skills(&refs);
                if !skills.is_empty() {
                    p.skills.extend(skills);
                    found.push("kompetanse");
                }
            }
            Part::Languages => {
                let langs = read_languages(&refs);
                if !langs.is_empty() {
                    p.languages.extend(langs);
                    found.push("språk");
                }
            }
            Part::Experience => {
                let (jobs, loose) = read_jobs(&refs, &home);
                if !jobs.is_empty() {
                    p.experience.extend(jobs);
                    found.push("jobber");
                }
                if !loose.is_empty() {
                    leftover("Erfaring", loose);
                }
            }
            Part::Education => {
                let (edu, loose) = read_education(&refs);
                if !edu.is_empty() {
                    p.education.extend(edu);
                    found.push("utdanning");
                }
                if !loose.is_empty() {
                    leftover("Utdanning", loose);
                }
            }
            Part::Certifications => {
                let certs = read_certifications(&refs);
                if !certs.is_empty() {
                    p.certifications.extend(certs);
                    found.push("kurs");
                }
            }
            Part::Interests => {
                let interests = read_interests(&refs);
                if !interests.is_empty() {
                    p.interests.extend(interests);
                    found.push("interesser");
                }
            }
            Part::Custom(no, en) => {
                let id = format!("section_imported_{}", p.custom_sections.len() + 1);
                let title = if p.is_english() { en } else { no };
                match read_custom(&refs, &id, title) {
                    Some(section) => {
                        p.custom_sections.push(section);
                        if !found.contains(&"andre seksjoner") {
                            found.push("andre seksjoner");
                        }
                    }
                    None => leftover(title, body),
                }
            }
            Part::Profile => leftover("Profil", body),
            Part::Skip => {}
        }
    }
    // An English CV keeps English headings.
    if looks_english(&text) {
        p.language = crate::model::Language::En;
        for s in &mut p.custom_sections {
            for item in &mut s.items {
                if let Some(d) = item.date.strip_suffix(" – nå") {
                    item.date = format!("{d} – Present");
                }
            }
            if let Some((_, en)) = HEADINGS.iter().find_map(|(_, part)| match part {
                Part::Custom(no, en) if *no == s.title => Some((*no, *en)),
                _ => None,
            }) {
                s.title = en.to_string();
            }
        }
    }
    let mut seen = Vec::new();
    found.retain(|f| {
        let new = !seen.contains(f);
        seen.push(*f);
        new
    });
    CvRead { project: p, rest: rest.join("\n\n"), found: found.into_iter().map(String::from).collect() }
}

/// Lines of a paragraph as one text; a word broken with a hyphen at the line end is put together.
fn join_lines(lines: &[&str]) -> String {
    let mut out = String::new();
    for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        if out.ends_with('-') && !out.ends_with(" -") && l.chars().next().is_some_and(char::is_lowercase) {
            out.push_str(l);
        } else {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(l);
        }
    }
    out
}

/// More English than Norwegian words: the CV's headings and dates should be English.
fn looks_english(text: &str) -> bool {
    const EN: &[&str] = &["the", "and", "with", "of", "for", "experience", "education", "skills", "present", "university"];
    const NO: &[&str] = &["og", "med", "for", "av", "erfaring", "utdanning", "kompetanse", "nå", "jeg", "har", "som", "til"];
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).collect();
    let en = words.iter().filter(|w| EN.contains(w)).count();
    let no = words.iter().filter(|w| NO.contains(w)).count();
    en > no * 2 && en >= 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_marks() {
        for l in ["1 / 2", "Side 1 av 2", "Page 2 of 3", "Ingrid Haugland 1 / 1", "Vebjørn Johannessen 1 / 2", "VebjørnJohannessen1/2"] {
            assert!(is_page_mark(&unspace(l)), "{l}");
        }
        for l in ["Prosjektkoordinator 03.2021 – nå", "Senior Designer 2019 – 2022", "Jobbet 3 av 4 dager"] {
            assert!(!is_page_mark(l), "{l}");
        }
    }
}
