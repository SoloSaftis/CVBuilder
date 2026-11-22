//! Reading LinkedIn's data export («Get a copy of your data»), the `.zip` with Profile.csv,
//! Positions.csv, Education.csv and so on, without AI and without unpacking it. Only the files
//! named in [`READ`] are opened, in memory; messages, connections and everything else in the
//! archive are never read. Columns are found by their header name, so a column LinkedIn adds or
//! moves does no harm, and a missing file is simply skipped.
use std::io::{Read, Seek};

use crate::checks;
use crate::cvread::{CvRead, looks_english, month_or_year};
use crate::model::{Certification, CustomItem, CustomSection, Education, Job, Language, Project, Skill, SpokenLanguage};

/// The files that are read. Nothing else in the archive is opened.
pub const READ: [&str; 10] = ["Profile.csv", "Positions.csv", "Education.csv", "Skills.csv", "Languages.csv", "Certifications.csv", "Projects.csv", "Email Addresses.csv", "PhoneNumbers.csv", "Phone Numbers.csv"];

/// A CSV file in the archive larger than this is not part of a profile.
const MAX_CSV: u64 = 20_000_000;

/// Whether a ZIP archive is a LinkedIn data export: it holds Profile.csv or Positions.csv.
pub fn is_export<R: Read + Seek>(reader: R) -> bool {
    zip::ZipArchive::new(reader).is_ok_and(|zip| zip.file_names().any(|n| matches!(base(n).to_lowercase().as_str(), "profile.csv" | "positions.csv")))
}

/// The file name without its folders («Basic_LinkedInDataExport/Positions.csv» → «Positions.csv»).
fn base(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

/// One CSV file: its rows as (header → value) lookups.
struct Table {
    head: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// The rows, each with a getter that takes any of a column's names.
    fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows.iter().map(|r| Row { head: &self.head, cells: r })
    }
}

struct Row<'a> {
    head: &'a [String],
    cells: &'a [String],
}

impl Row<'_> {
    /// The value under the first of `names` the file has, trimmed; `""` when there is none.
    fn get(&self, names: &[&str]) -> String {
        names
            .iter()
            .find_map(|n| {
                let n = norm(n);
                self.head.iter().position(|h| *h == n)
            })
            .and_then(|i| self.cells.get(i))
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    }
}

/// A header as it is compared: lower case, letters and digits only («Started On» → «startedon»).
fn norm(header: &str) -> String {
    header.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// RFC 4180 CSV: quoted fields may hold commas, quotes («""») and line breaks.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    rows
}

/// Some files start with a few lines of notes before the header («Notes: …»); the header is the
/// first row that has one of the columns this reader knows.
fn table(text: &str, known: &[&str]) -> Option<Table> {
    let rows = parse_csv(text);
    let known: Vec<String> = known.iter().map(|k| norm(k)).collect();
    let at = rows.iter().position(|r| r.iter().any(|h| known.contains(&norm(h))))?;
    let head = rows[at].iter().map(|h| norm(h)).collect();
    Some(Table { head, rows: rows[at + 1..].to_vec() })
}

/// The CSV files this reader knows, read from the archive by name.
struct Export {
    files: Vec<(String, String)>,
}

impl Export {
    fn open<R: Read + Seek>(reader: R) -> Result<Export, String> {
        let mut zip = zip::ZipArchive::new(reader).map_err(|_| "ZIP-filen kunne ikke åpnes. Den kan være skadet eller bare delvis lastet ned.".to_string())?;
        let wanted: Vec<String> = READ.iter().map(|n| n.to_lowercase()).collect();
        let names: Vec<String> = zip.file_names().filter(|n| wanted.contains(&base(n).to_lowercase())).map(str::to_string).collect();
        let mut files = Vec::new();
        for name in names {
            let Ok(entry) = zip.by_name(&name) else { continue };
            if entry.size() > MAX_CSV {
                continue;
            }
            let mut bytes = Vec::new();
            if entry.take(MAX_CSV).read_to_end(&mut bytes).is_ok() {
                files.push((base(&name).to_lowercase(), crate::import::decode_text(&bytes)));
            }
        }
        Ok(Export { files })
    }

    fn table(&self, file: &str, known: &[&str]) -> Option<Table> {
        let file = file.to_lowercase();
        self.files.iter().find(|(n, _)| *n == file).and_then(|(_, text)| table(text, known))
    }
}

// The columns, by every name they are known to have had.
const FIRST: &[&str] = &["First Name", "Fornavn"];
const LAST: &[&str] = &["Last Name", "Etternavn"];
const HEADLINE: &[&str] = &["Headline", "Overskrift"];
const SUMMARY: &[&str] = &["Summary", "Sammendrag", "About"];
const GEO: &[&str] = &["Geo Location", "Location", "Sted"];
const COMPANY: &[&str] = &["Company Name", "Company", "Firmanavn", "Selskap"];
const TITLE: &[&str] = &["Title", "Tittel", "Stilling"];
const DESCRIPTION: &[&str] = &["Description", "Beskrivelse"];
const LOCATION: &[&str] = &["Location", "Sted"];
const STARTED: &[&str] = &["Started On", "Start Date", "Startet", "Startdato"];
const FINISHED: &[&str] = &["Finished On", "End Date", "Avsluttet", "Sluttdato"];
const SCHOOL: &[&str] = &["School Name", "School", "Skole"];
const DEGREE: &[&str] = &["Degree Name", "Degree", "Grad"];
const NAME: &[&str] = &["Name", "Navn"];
const PROFICIENCY: &[&str] = &["Proficiency", "Ferdighetsnivå", "Nivå"];
const AUTHORITY: &[&str] = &["Authority", "Utsteder"];
const EMAIL: &[&str] = &["Email Address", "E-postadresse"];
const PRIMARY: &[&str] = &["Primary", "Primær"];
const NUMBER: &[&str] = &["Number", "Nummer"];

/// Reads a LinkedIn data export. `found` lists what was read with counts, as Norwegian words and
/// numbers («jobber (6)»), for the status line; `rest` is always empty, since every value has a
/// place.
pub fn read_export<R: Read + Seek>(reader: R) -> Result<CvRead, String> {
    let export = Export::open(reader)?;
    let mut p = Project::default();
    let mut found: Vec<String> = Vec::new();
    let mut prose = String::new();

    if let Some(t) = export.table("Profile.csv", &[FIRST, LAST, HEADLINE].concat()) {
        if let Some(r) = t.rows().next() {
            let c = &mut p.candidate;
            c.name = [r.get(FIRST), r.get(LAST)].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
            c.headline = r.get(HEADLINE);
            c.summary = r.get(SUMMARY).replace("\r\n", "\n");
            // «Oslo, Oslo, Norway» → «Oslo».
            c.city = r.get(GEO).split(',').next().unwrap_or("").trim().to_string();
            prose.push_str(&format!("{}\n{}\n", c.headline, c.summary));
            for (value, word) in [(&c.name, "navn"), (&c.headline, "tittel"), (&c.summary, "profil"), (&c.city, "sted")] {
                if !value.is_empty() {
                    found.push(word.into());
                }
            }
        }
    }
    if let Some(t) = export.table("Email Addresses.csv", EMAIL) {
        let rows: Vec<Row> = t.rows().filter(|r| r.get(EMAIL).contains('@')).collect();
        if let Some(r) = rows.iter().find(|r| yes(&r.get(PRIMARY))).or(rows.first()) {
            p.candidate.email = r.get(EMAIL);
            found.push("e-post".into());
        }
    }
    if let Some(t) = export.table("PhoneNumbers.csv", NUMBER).or_else(|| export.table("Phone Numbers.csv", NUMBER)) {
        if let Some(n) = t.rows().map(|r| r.get(NUMBER)).find(|n| n.chars().filter(char::is_ascii_digit).count() >= 5) {
            p.candidate.phone = n;
            found.push("telefon".into());
        }
    }

    if let Some(t) = export.table("Positions.csv", &[COMPANY, TITLE, STARTED].concat()) {
        for r in t.rows() {
            let (company, title) = (r.get(COMPANY), r.get(TITLE));
            if company.is_empty() && title.is_empty() {
                continue;
            }
            let description = r.get(DESCRIPTION);
            prose.push_str(&description);
            prose.push('\n');
            let finished = r.get(FINISHED);
            let end = if finished.is_empty() || checks::is_now(&finished) { None } else { Some(date(&finished)) };
            let n = p.experience.len() + 1;
            p.experience.push(Job { bullet_ids: Vec::new(), id: format!("job_imported_{n}"), company, title, location: r.get(LOCATION), start: date(&r.get(STARTED)), end, bullets: lines(&description), evidence: String::new(), confirmed: true });
        }
        count(&mut found, "jobber", p.experience.len());
    }
    if let Some(t) = export.table("Education.csv", &[SCHOOL, DEGREE].concat()) {
        for r in t.rows() {
            let (institution, qualification) = (r.get(SCHOOL), r.get(DEGREE));
            if institution.is_empty() && qualification.is_empty() {
                continue;
            }
            p.education.push(Education { id: String::new(), qualification, institution, start: year(&r.get(STARTED)), end: year(&r.get(FINISHED)) });
        }
        count(&mut found, "utdanning", p.education.len());
    }
    if let Some(t) = export.table("Skills.csv", NAME) {
        for name in t.rows().map(|r| r.get(NAME)).filter(|n| !n.is_empty()) {
            if !p.skills.iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
                p.skills.push(Skill { id: String::new(), group: String::new(), name, evidence: String::new(), confirmed: true });
            }
        }
        count(&mut found, "kompetanse", p.skills.len());
    }
    if let Some(t) = export.table("Certifications.csv", &[NAME, AUTHORITY].concat()) {
        for r in t.rows() {
            let name = r.get(NAME);
            if !name.is_empty() {
                p.certifications.push(Certification { id: String::new(), name, issuer: r.get(AUTHORITY), year: year(&r.get(STARTED)) });
            }
        }
        count(&mut found, "kurs", p.certifications.len());
    }

    // The CV's language follows the text the person wrote on LinkedIn.
    let english = looks_english(&prose);
    if english {
        p.language = Language::En;
    }
    if let Some(t) = export.table("Languages.csv", &[NAME, PROFICIENCY].concat()) {
        for r in t.rows() {
            let name = r.get(NAME);
            if !name.is_empty() {
                p.languages.push(SpokenLanguage { id: String::new(), name, level: level(&r.get(PROFICIENCY), english) });
            }
        }
        count(&mut found, "språk", p.languages.len());
    }
    if let Some(t) = export.table("Projects.csv", &[TITLE, DESCRIPTION].concat()) {
        let present = if english { "Present" } else { "nå" };
        let items: Vec<CustomItem> = t
            .rows()
            .filter(|r| !r.get(TITLE).is_empty())
            .map(|r| {
                let (start, end) = (date(&r.get(STARTED)), r.get(FINISHED));
                let date = match (start.is_empty(), end.is_empty()) {
                    (true, _) => String::new(),
                    (false, true) => format!("{} – {present}", checks::show_month(&start)),
                    (false, false) => format!("{} – {}", checks::show_month(&start), checks::show_month(&self::date(&end))),
                };
                CustomItem { id: String::new(), bullet_ids: Vec::new(), title: r.get(TITLE), subtitle: String::new(), date, bullets: lines(&r.get(DESCRIPTION)) }
            })
            .collect();
        if !items.is_empty() {
            count(&mut found, "andre seksjoner", 1);
            p.custom_sections.push(CustomSection { id: "section_imported_1".into(), title: if english { "Projects" } else { "Prosjekter" }.into(), items });
        }
    }

    if p.candidate.name.is_empty() && p.experience.is_empty() && p.education.is_empty() && p.skills.is_empty() {
        return Err("LinkedIn-eksporten har ingen profil, jobber eller utdanning å hente inn.".into());
    }
    Ok(CvRead { project: p, rest: String::new(), found })
}

/// «jobber (6)»; nothing when there are none.
pub(crate) fn count(found: &mut Vec<String>, word: &str, n: usize) {
    match n {
        0 => {}
        1 if word == "andre seksjoner" => found.push(word.into()),
        _ => found.push(format!("{word} ({n})")),
    }
}

fn yes(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "yes" | "ja" | "true" | "1")
}

/// LinkedIn writes «Mar 2021», «2019» or (in some exports) «2021-03» or «03/2021»; the model
/// keeps `YYYY-MM` or a bare year. A date that cannot be read is left empty.
fn date(text: &str) -> String {
    month_or_year(text).unwrap_or_default()
}

fn year(text: &str) -> String {
    checks::parse_year(text).unwrap_or_default()
}

/// A LinkedIn description as bullet lines: one per line, bullet marks taken off, the words kept
/// exactly as written (long prose stays prose; the checklist gives advice on bullets).
pub(crate) fn lines(text: &str) -> Vec<String> {
    const MARKS: [char; 8] = ['•', '·', '▪', '◦', '‣', '–', '-', '*'];
    text.lines().map(|l| l.trim().trim_start_matches(MARKS).trim()).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// LinkedIn's five levels, in the words CVBuilder's own examples use; another value is kept.
fn level(proficiency: &str, english: bool) -> String {
    let p = proficiency.trim().to_lowercase();
    let pick = |no: &str, en: &str| if english { en.to_string() } else { no.to_string() };
    if p.starts_with("native") {
        pick("morsmål", "native")
    } else if p.starts_with("full professional") {
        pick("flytende", "fluent")
    } else if p.starts_with("professional working") {
        pick("svært godt", "professional working")
    } else if p.starts_with("limited working") {
        pick("godt", "limited working")
    } else if p.starts_with("elementary") {
        pick("grunnleggende", "elementary")
    } else {
        proficiency.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_with_quotes_commas_and_line_breaks() {
        let rows = parse_csv("A,B,C\r\n\"x, y\",\"say \"\"hi\"\"\",\"one\ntwo\"\r\n,,\n");
        assert_eq!(rows, vec![vec!["A", "B", "C"], vec!["x, y", "say \"hi\"", "one\ntwo"]]);
    }

    #[test]
    fn notes_before_the_header_are_skipped() {
        let t = table("Notes:\n\"Some note, with a comma\"\n\nFirst Name,Last Name\nKari,Nordmann\n", &[FIRST, LAST].concat()).expect("table");
        let r = t.rows().next().expect("row");
        assert_eq!((r.get(FIRST), r.get(LAST)), ("Kari".to_string(), "Nordmann".to_string()));
    }

    #[test]
    fn dates_in_the_forms_linkedin_uses() {
        for (input, want) in [("Mar 2021", "2021-03"), ("2019", "2019"), ("mars 2021", "2021-03"), ("okt. 2020", "2020-10"), ("2021-03", "2021-03"), ("03/2021", "2021-03"), ("", ""), ("someday", "")] {
            assert_eq!(date(input), want, "{input}");
        }
    }
}
