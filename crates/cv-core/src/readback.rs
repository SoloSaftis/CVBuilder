//! «Hva et ATS leser»: the exported file read back the way a simple applicant tracking system
//! would, with the app's own importer, and compared field by field with the CV. Nothing is
//! scored: each field is the same, read differently, or not found, so the person can see what
//! a recruiter searching the ATS would get.
//!
//! Our reader is tuned to our own output and may be kinder than a commercial parser, so the
//! comparison is strict: only the same text (ignoring case and spacing) counts as the same.
use crate::checks;
use crate::import;
use crate::model::Project;

/// A field of the CV that an ATS fills in. The numbers are 1-based, as the person counts jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Email,
    Phone,
    City,
    LinkedIn,
    JobTitle(usize),
    Employer(usize),
    JobDates(usize),
    Qualification(usize),
    School(usize),
    StudyDates(usize),
}

impl Field {
    /// The kind of advice the field needs when it went wrong.
    pub fn is_date(self) -> bool {
        matches!(self, Field::JobDates(_) | Field::StudyDates(_))
    }

    pub fn is_contact(self) -> bool {
        matches!(self, Field::Name | Field::Email | Field::Phone | Field::City | Field::LinkedIn)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Read back as it is in the CV.
    Same,
    /// Found, but read as something else.
    Different,
    /// Not found in the file at all.
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCheck {
    pub field: Field,
    /// As it is in the CV (dates as «03.2021 – 02.2023», a current job as «03.2021 –»).
    pub expected: String,
    /// As the reader got it from the file; empty when not found.
    pub read: String,
    pub state: State,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadBack {
    /// The file's text in the order it was extracted, which is the order an ATS reads.
    pub text: String,
    /// The CV's filled-in fields, in CV order.
    pub fields: Vec<FieldCheck>,
}

impl ReadBack {
    pub fn count(&self, state: State) -> usize {
        self.fields.iter().filter(|f| f.state == state).count()
    }
}

/// Reads an exported file's bytes back (`ext` is `pdf` or `docx`) and compares it with the CV.
pub fn read_back(project: &Project, bytes: &[u8], ext: &str) -> Result<ReadBack, String> {
    let text = import::text_from_bytes(bytes, ext)?;
    Ok(compare(project, &text))
}

/// Compares the CV with what the importer reads from `text`.
pub fn compare(project: &Project, text: &str) -> ReadBack {
    let project = &*project.visible();
    let read = import::read_cv(text).project;
    let mut fields = Vec::new();
    let (c, r) = (&project.candidate, &read.candidate);
    for (field, expected, got, same) in [
        (Field::Name, &c.name, &r.name, plain as fn(&str) -> String),
        (Field::Email, &c.email, &r.email, plain),
        (Field::Phone, &c.phone, &r.phone, digits),
        (Field::City, &c.city, &r.city, plain),
        (Field::LinkedIn, &c.linkedin, &r.linkedin, link),
    ] {
        push(&mut fields, field, expected, got, same(expected) == same(got));
    }

    // Each job is paired with the read job that has its employer or title, else the one in its place.
    let mut used = vec![false; read.experience.len()];
    for (i, job) in project.experience.iter().enumerate().filter(|(_, j)| !(j.title.trim().is_empty() && j.company.trim().is_empty())) {
        let at = pair(&mut used, i, |k| {
            let o = &read.experience[k];
            (!job.company.trim().is_empty() && plain(&o.company) == plain(&job.company)) || (!job.title.trim().is_empty() && plain(&o.title) == plain(&job.title))
        });
        let other = at.map(|k| read.experience[k].clone()).unwrap_or_default();
        let n = i + 1;
        push(&mut fields, Field::JobTitle(n), &job.title, &other.title, plain(&job.title) == plain(&other.title));
        push(&mut fields, Field::Employer(n), &job.company, &other.company, plain(&job.company) == plain(&other.company));
        let expected = dates(&job.start, job.end.as_deref().unwrap_or(""), job.end.is_none());
        let got = dates(&other.start, other.end.as_deref().unwrap_or(""), other.end.is_none() && !other.start.is_empty());
        let same = month(&job.start) == month(&other.start) && (job.end.is_none() && other.end.is_none() || month(job.end.as_deref().unwrap_or("")) == month(other.end.as_deref().unwrap_or("")));
        push(&mut fields, Field::JobDates(n), &expected, &got, same);
    }

    let mut used = vec![false; read.education.len()];
    for (i, edu) in project.education.iter().enumerate().filter(|(_, e)| !(e.qualification.trim().is_empty() && e.institution.trim().is_empty())) {
        let at = pair(&mut used, i, |k| {
            let o = &read.education[k];
            (!edu.institution.trim().is_empty() && plain(&o.institution) == plain(&edu.institution)) || (!edu.qualification.trim().is_empty() && plain(&o.qualification) == plain(&edu.qualification))
        });
        let other = at.map(|k| read.education[k].clone()).unwrap_or_default();
        let n = i + 1;
        push(&mut fields, Field::Qualification(n), &edu.qualification, &other.qualification, plain(&edu.qualification) == plain(&other.qualification));
        push(&mut fields, Field::School(n), &edu.institution, &other.institution, plain(&edu.institution) == plain(&other.institution));
        let year = |v: &str| checks::parse_year(v).unwrap_or_default();
        let expected = dates(&edu.start, &edu.end, false);
        let got = dates(&other.start, &other.end, false);
        push(&mut fields, Field::StudyDates(n), &expected, &got, year(&edu.start) == year(&other.start) && year(&edu.end) == year(&other.end));
    }
    ReadBack { text: text.trim().to_string(), fields }
}

/// Adds the field when the CV has it filled in.
fn push(fields: &mut Vec<FieldCheck>, field: Field, expected: &str, read: &str, same: bool) {
    if expected.trim().is_empty() {
        return;
    }
    let state = if read.trim().is_empty() {
        State::Missing
    } else if same {
        State::Same
    } else {
        State::Different
    };
    fields.push(FieldCheck { field, expected: expected.trim().to_string(), read: read.trim().to_string(), state });
}

/// The read entry for entry `i`: the first unused one `matches` accepts, else the one in place.
fn pair(used: &mut [bool], i: usize, matches: impl Fn(usize) -> bool) -> Option<usize> {
    let at = (0..used.len()).find(|&k| !used[k] && matches(k)).or_else(|| (i < used.len() && !used[i]).then_some(i))?;
    used[at] = true;
    Some(at)
}

/// «03.2021 – 02.2023»; a current job as «03.2021 –»; empty when there is no date.
fn dates(start: &str, end: &str, now: bool) -> String {
    let (start, end) = (checks::show_month(start.trim()), checks::show_month(end.trim()));
    match (start.is_empty(), end.is_empty()) {
        (true, true) => String::new(),
        (false, true) if now => format!("{start} –"),
        (false, true) => start,
        (true, false) => end,
        (false, false) => format!("{start} – {end}"),
    }
}

/// A date as `YYYY-MM` (or `YYYY`), whatever way it was written.
fn month(value: &str) -> String {
    checks::parse_month(value).unwrap_or_else(|| value.trim().to_string())
}

/// Lower case with single spaces and no punctuation at the ends.
fn plain(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| c.is_ascii_punctuation() && c != '+').to_lowercase()
}

fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

/// A LinkedIn address without `https://`, `www.` or a closing slash.
fn link(value: &str) -> String {
    let v = plain(value);
    let v = v.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
    v.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_text_reads_back_the_same() {
        let p = Project::example();
        let back = compare(&p, &checks::plain_text(&p));
        let wrong: Vec<_> = back.fields.iter().filter(|f| f.state != State::Same).collect();
        assert!(wrong.is_empty(), "{wrong:#?}");
        assert!(back.fields.iter().any(|f| f.field == Field::JobDates(1)));
    }

    #[test]
    fn empty_fields_are_left_out_and_lost_ones_are_missing() {
        let mut p = Project::example();
        p.candidate.phone.clear();
        let back = compare(&p, "Nothing like a CV here at all, just some words.");
        assert!(back.fields.iter().all(|f| f.field != Field::Phone));
        assert_eq!(back.count(State::Same), 0);
        assert!(back.fields.iter().all(|f| f.state == State::Missing), "{:#?}", back.fields);
    }

    #[test]
    fn helpers() {
        assert_eq!(dates("2021-03", "", true), "03.2021 –");
        assert_eq!(dates("2018-01", "2021-02", false), "01.2018 – 02.2021");
        assert_eq!(link("https://www.linkedin.com/in/x/"), link("linkedin.com/in/X"));
        assert_eq!(digits("+47 400 00 000"), "4740000000");
    }
}
