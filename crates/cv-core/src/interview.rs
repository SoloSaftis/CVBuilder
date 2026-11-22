//! «Forbered intervjuet»: a short sheet of what an interviewer is likely to ask about, built
//! only from the CV and the job ad, with fixed rules and no AI:
//!
//! - the claims in the CV that carry a number or a result («cut waiting times by 20 %»);
//! - what the ad asks for, and where in the CV it is shown (a job, an education …), only listed
//!   (skills or profile, with no job behind it), or not shown at all;
//! - gaps: what the ad asks for that the CV does not show, and more than six months without a job;
//! - the questions almost every interview has (why this job, why leave the current one).
//!
//! The wording is Norwegian keys with `{name}` placeholders; the app translates them into the
//! menu language (the sheet is the person's own preparation, not part of the CV). The person's
//! notes are kept by the app, never in the CV file.
use std::collections::HashSet;
use std::sync::Arc;

use wordcraft_doc::props::{Border, BorderStyle, Borders, LineSpacing};
use wordcraft_doc::{Block, CharProps, Document, Paragraph, Rgb, TextColor};

use crate::checks::{RESULT, contains, gaps, normal};
use crate::layouts::{effective, rgb};
use crate::model::Project;
use crate::text;

/// Translates a Norwegian key and fills in its `{name}` placeholders.
pub type Tr<'a> = &'a dyn Fn(&'static str, &[(&str, &str)]) -> String;

/// At most this many claims and requirements, so the sheet stays short enough to read.
pub const MAX_CLAIMS: usize = 8;
pub const MAX_NEEDS: usize = 12;

/// The parts of the sheet, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Lines in the CV with a number or a result.
    Claims,
    /// What the ad asks for that the CV shows (or only lists).
    Ad,
    /// What the ad asks for that the CV does not show, and time without a job.
    Gaps,
    /// The questions almost every interview has.
    Always,
}

impl Part {
    pub const ALL: [Part; 4] = [Part::Claims, Part::Ad, Part::Gaps, Part::Always];

    /// The heading, a Norwegian key.
    pub fn title(self) -> &'static str {
        match self {
            Part::Claims => "Påstander i CV-en du kan få spørsmål om",
            Part::Ad => "Det annonsen ber om, og hvor CV-en viser det",
            Part::Gaps => "Hull å forberede seg på",
            Part::Always => "Spørsmål som nesten alltid kommer",
        }
    }
}

/// How much the ad leans on a requirement, from the words around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Must,
    Nice,
    Unknown,
}

impl Weight {
    /// A short tag for the sheet, a Norwegian key.
    pub fn tag(self) -> Option<&'static str> {
        match self {
            Weight::Must => Some("Krav"),
            Weight::Nice => Some("Ønskelig"),
            Weight::Unknown => None,
        }
    }
}

/// A place in the CV: text from the CV itself (a job's title and employer), or a section,
/// named by its Norwegian key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Cv(String),
    Section(&'static str),
}

/// A Norwegian key and the values for its placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
}

impl Line {
    fn new(key: &'static str, args: &[(&'static str, &str)]) -> Line {
        Line { key, args: args.iter().map(|(k, v)| (*k, v.to_string())).collect() }
    }

    pub fn render(&self, tr: Tr<'_>) -> String {
        let args: Vec<(&str, &str)> = self.args.iter().map(|(k, v)| (*k, v.as_str())).collect();
        tr(self.key, &args)
    }
}

/// One thing to prepare for.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Stable while the CV and the ad stay the same: the key the person's notes are kept under.
    pub id: String,
    pub part: Part,
    /// Text from the CV or the ad, shown as it is (a bullet, a requirement); may be empty.
    pub subject: String,
    pub weight: Weight,
    /// The question to be ready for.
    pub question: Line,
    /// Where the CV shows it (claims: the job the line is in).
    pub places: Vec<Place>,
}

/// The whole sheet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sheet {
    pub items: Vec<Item>,
    /// The requirements came from the ad's text, not from the person's own keyword list.
    pub suggested: bool,
}

impl Sheet {
    pub fn part(&self, part: Part) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |i| i.part == part)
    }
}

/// Every key the sheet may use, so the app can check each one has a translation.
pub const KEYS: &[&str] = &[
    "Påstander i CV-en du kan få spørsmål om",
    "Det annonsen ber om, og hvor CV-en viser det",
    "Hull å forberede seg på",
    "Spørsmål som nesten alltid kommer",
    "Krav",
    "Ønskelig",
    "Profil",
    "Kjernekompetanse",
    "Språk",
    "Hva gjorde du selv, hvordan ble det målt, og hva var vanskelig?",
    "Gi et konkret eksempel fra {place}: hva du gjorde og hva det førte til.",
    "Det står i CV-en, men ingen jobb eller utdanning viser det. Ha et konkret eksempel klart.",
    "Annonsen ber om dette, men CV-en viser det ikke. Hva svarer du hvis du får spørsmål om det?",
    "Mellom {after} og {before} er det {months} måneder uten jobb i CV-en. Hva gjorde du da?",
    "Hvorfor vil du jobbe som {role} hos {company}, og hvorfor nå?",
    "Hvorfor vil du jobbe som {role}, og hvorfor nå?",
    "Hvorfor vil du jobbe hos {company}, og hvorfor nå?",
    "Hvorfor søker du denne jobben, og hvorfor nå?",
    "Hvorfor vil du slutte hos {company}?",
    "Profilen din sier «{headline}». Hva er et eksempel som viser det?",
    "Fortell kort om deg selv: de tre tingene du vil at de skal huske.",
    "Hva vet du om {company}, og hva vil du spørre dem om?",
    "Hva vil du spørre dem om?",
    "CV-en viser det: {where}",
    "Fra: {where}",
    "Notater",
    "Forbered intervjuet",
    "Forbered intervjuet: {job}",
    "Bygd fra CV-en og annonsen. Kan du ikke snakke om en linje, bør den kanskje ut av CV-en.",
    "Ingenting å vise ennå. Skriv jobbene dine og lim inn stillingsannonsen, så fylles arket.",
];

/// Words that make a line of the ad a requirement, or only a wish (lower case; Norwegian,
/// English, Swedish, Danish, German).
const MUST: &[&str] = &[
    "krav", "kreves", "påkrevd", "må ha", "må kunne", "må være", "nødvendig", "kvalifikasjoner", "required", "requirement", "must", "essential", "you have", "you will have",
    "qualifications", "krav:", "ska ha", "måste", "skal have", "kræver", "voraussetzung", "erforderlich", "anforderungen", "sie haben", "du har", "du er", "vi ser etter",
];
const NICE: &[&str] = &[
    "ønskelig", "en fordel", "fordelaktig", "et pluss", "positivt", "nice to have", "a plus", "an advantage", "bonus", "preferred", "desirable", "meriterande", "meriterende",
    "önskvärt", "en fördel", "wünschenswert", "von vorteil", "gerne", "ideally", "ideelt",
];

/// The block of the ad a line belongs to, from the last heading above it.
fn weights(ad: &str) -> Vec<(String, Weight)> {
    let mut block = Weight::Unknown;
    let mut out = Vec::new();
    for raw in ad.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
        let own = if has(NICE) {
            Weight::Nice
        } else if has(MUST) {
            Weight::Must
        } else {
            Weight::Unknown
        };
        // A short line, or one ending in «:», is a heading for the lines under it.
        let heading = line.ends_with(':') || (line.split_whitespace().count() <= 4 && !line.starts_with(['-', '•', '*', '–']));
        if heading {
            block = own;
            out.push((normal(line), own));
            continue;
        }
        out.push((normal(line), if own == Weight::Unknown { block } else { own }));
    }
    out
}

/// What the ad asks for: the person's own keyword list, else the terms found in the ad.
fn needs(p: &Project) -> (Vec<String>, bool) {
    let mut seen = HashSet::new();
    let own: Vec<String> = p.target.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !normal(k).is_empty() && seen.insert(normal(k))).collect();
    if !own.is_empty() {
        return (own, false);
    }
    (crate::import::ad_keywords(&p.target.description, &p.target.company, MAX_NEEDS), true)
}

fn joined(parts: &[&str]) -> String {
    normal(&parts.join("\n"))
}

/// The places in the CV that show `term`, and whether one of them is real evidence (a job, an
/// education, a course, an item of the person's own sections, a language).
fn places(p: &Project, term: &str) -> (Vec<Place>, bool) {
    let mut out = Vec::new();
    let mut evidence = false;
    for job in text::sort_jobs(&p.experience) {
        let mut parts = vec![job.title.as_str(), job.company.as_str()];
        parts.extend(job.bullets.iter().map(String::as_str));
        if contains(&joined(&parts), term) {
            out.push(Place::Cv(heading(&[&job.title, &job.company])));
            evidence = true;
        }
    }
    for e in text::sort_education(&p.education) {
        if contains(&joined(&[&e.qualification, &e.institution]), term) {
            out.push(Place::Cv(heading(&[&e.qualification, &e.institution])));
            evidence = true;
        }
    }
    for c in &p.certifications {
        if contains(&joined(&[&c.name, &c.issuer]), term) {
            out.push(Place::Cv(heading(&[&c.name])));
            evidence = true;
        }
    }
    for s in &p.custom_sections {
        for i in &s.items {
            let mut parts = vec![i.title.as_str(), i.subtitle.as_str()];
            parts.extend(i.bullets.iter().map(String::as_str));
            if contains(&joined(&parts), term) {
                out.push(Place::Cv(heading(&[&i.title, &i.subtitle])));
                evidence = true;
            }
        }
    }
    if p.languages.iter().any(|l| contains(&normal(&l.name), term)) {
        out.push(Place::Section("Språk"));
        evidence = true;
    }
    let skills: Vec<&str> = p.skills.iter().flat_map(|s| [s.group.as_str(), s.name.as_str()]).collect();
    if contains(&joined(&skills), term) {
        out.push(Place::Section("Kjernekompetanse"));
    }
    if contains(&joined(&[&p.candidate.headline, &p.candidate.summary]), term) {
        out.push(Place::Section("Profil"));
    }
    (out, evidence)
}

fn heading(parts: &[&str]) -> String {
    parts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" – ")
}

fn id(kind: &str, text: &str) -> String {
    let mut n = normal(text);
    if n.len() > 80 {
        let mut end = 80;
        while !n.is_char_boundary(end) {
            end -= 1;
        }
        n.truncate(end);
    }
    format!("{kind}:{n}")
}

/// The sheet for this CV and its job ad. `today_index` is `year * 12 + month - 1`.
pub fn sheet(p: &Project, today_index: i32) -> Sheet {
    let mut items = Vec::new();
    // Claims: every line with a number or a result, newest job first.
    let mut seen = HashSet::new();
    let mut claims: Vec<(String, String)> = Vec::new();
    for job in text::sort_jobs(&p.experience) {
        for b in &job.bullets {
            claims.push((b.trim().to_string(), heading(&[&job.title, &job.company])));
        }
    }
    for s in &p.custom_sections {
        for i in &s.items {
            for b in &i.bullets {
                claims.push((b.trim().to_string(), heading(&[&i.title, &i.subtitle])));
            }
        }
    }
    for (bullet, place) in claims.into_iter().filter(|(b, _)| RESULT.is_match(b)).take(MAX_CLAIMS) {
        if !seen.insert(normal(&bullet)) {
            continue;
        }
        items.push(Item {
            id: id("claim", &bullet),
            part: Part::Claims,
            subject: bullet,
            weight: Weight::Unknown,
            question: Line::new("Hva gjorde du selv, hvordan ble det målt, og hva var vanskelig?", &[]),
            places: if place.is_empty() { Vec::new() } else { vec![Place::Cv(place)] },
        });
    }
    // What the ad asks for: musts first, then the rest, wishes last.
    let (terms, suggested) = needs(p);
    let lines = weights(&p.target.description);
    let mut asked: Vec<(String, Weight)> = terms
        .into_iter()
        .take(MAX_NEEDS)
        .map(|term| {
            let weight = lines.iter().find(|(line, _)| contains(line, &term)).map_or(Weight::Unknown, |(_, w)| *w);
            (term, weight)
        })
        .collect();
    let rank = |w: Weight| match w {
        Weight::Must => 0,
        Weight::Unknown => 1,
        Weight::Nice => 2,
    };
    asked.sort_by_key(|(_, w)| rank(*w));
    let mut missing = Vec::new();
    for (term, weight) in asked {
        let (found, evidence) = places(p, &term);
        let item = |part, question| Item { id: id("need", &term), part, subject: term.clone(), weight, question, places: found.clone() };
        if found.is_empty() {
            missing.push(item(Part::Gaps, Line::new("Annonsen ber om dette, men CV-en viser det ikke. Hva svarer du hvis du får spørsmål om det?", &[])));
        } else if evidence {
            let first = found.iter().find_map(|pl| if let Place::Cv(s) = pl { Some(s.clone()) } else { None }).unwrap_or_default();
            items.push(item(Part::Ad, Line::new("Gi et konkret eksempel fra {place}: hva du gjorde og hva det førte til.", &[("place", &first)])));
        } else {
            items.push(item(Part::Ad, Line::new("Det står i CV-en, men ingen jobb eller utdanning viser det. Ha et konkret eksempel klart.", &[])));
        }
    }
    // Gaps: what the ad asks for that the CV does not show, then time without a job.
    items.extend(missing);
    for (after, before, months) in gaps(p, today_index) {
        items.push(Item {
            id: id("gap", &format!("{after} {before}")),
            part: Part::Gaps,
            subject: String::new(),
            weight: Weight::Unknown,
            question: Line::new("Mellom {after} og {before} er det {months} måneder uten jobb i CV-en. Hva gjorde du da?", &[("after", &after), ("before", &before), ("months", &months.to_string())]),
            places: Vec::new(),
        });
    }
    // The questions almost every interview has.
    let always = |id: &str, question: Line| Item { id: id.to_string(), part: Part::Always, subject: String::new(), weight: Weight::Unknown, question, places: Vec::new() };
    items.push(always("about", Line::new("Fortell kort om deg selv: de tre tingene du vil at de skal huske.", &[])));
    let (role, company) = (p.target.role.trim(), p.target.company.trim());
    items.push(always(
        "why",
        match (role.is_empty(), company.is_empty()) {
            (false, false) => Line::new("Hvorfor vil du jobbe som {role} hos {company}, og hvorfor nå?", &[("role", role), ("company", company)]),
            (false, true) => Line::new("Hvorfor vil du jobbe som {role}, og hvorfor nå?", &[("role", role)]),
            (true, false) => Line::new("Hvorfor vil du jobbe hos {company}, og hvorfor nå?", &[("company", company)]),
            (true, true) => Line::new("Hvorfor søker du denne jobben, og hvorfor nå?", &[]),
        },
    ));
    let headline = p.candidate.headline.trim();
    if !headline.is_empty() {
        items.push(always("headline", Line::new("Profilen din sier «{headline}». Hva er et eksempel som viser det?", &[("headline", headline)])));
    }
    for job in p.experience.iter().filter(|j| j.end.is_none() && !j.company.trim().is_empty()) {
        items.push(always(&id("leave", &job.company), Line::new("Hvorfor vil du slutte hos {company}?", &[("company", job.company.trim())])));
    }
    items.push(always(
        "ask",
        if company.is_empty() { Line::new("Hva vil du spørre dem om?", &[]) } else { Line::new("Hva vet du om {company}, og hva vil du spørre dem om?", &[("company", company)]) },
    ));
    Sheet { items, suggested }
}

/// Where the CV shows an item, as one line («CV-en viser det: …»; for a claim «Fra: …»).
pub fn where_line(item: &Item, tr: Tr<'_>) -> Option<String> {
    if item.places.is_empty() || item.part == Part::Gaps {
        return None;
    }
    let names: Vec<String> = item.places.iter().map(|pl| match pl {
        Place::Cv(s) => s.clone(),
        Place::Section(key) => tr(key, &[]),
    }).collect();
    let key = if item.part == Part::Claims { "Fra: {where}" } else { "CV-en viser det: {where}" };
    Some(tr(key, &[("where", &names.join("; "))]))
}

/// The heading line of an item: its subject with the must/nice tag, or the question itself
/// when there is no subject.
pub fn headline(item: &Item, tr: Tr<'_>) -> String {
    let subject = if item.subject.is_empty() { item.question.render(tr) } else if item.part == Part::Claims { format!("«{}»", item.subject) } else { item.subject.clone() };
    match item.weight.tag() {
        Some(tag) => format!("{subject} ({})", tr(tag, &[])),
        None => subject,
    }
}

/// The title of the sheet: the job it is for, when the ad names it.
pub fn title(p: &Project, tr: Tr<'_>) -> String {
    let job = heading(&[&p.target.role, &p.target.company]);
    if job.is_empty() { tr("Forbered intervjuet", &[]) } else { tr("Forbered intervjuet: {job}", &[("job", &job)]) }
}

/// The sheet as plain text to copy or print, with the person's notes.
pub fn to_text(p: &Project, sheet: &Sheet, notes: &dyn Fn(&str) -> String, tr: Tr<'_>) -> String {
    let mut out = vec![title(p, tr), tr("Bygd fra CV-en og annonsen. Kan du ikke snakke om en linje, bør den kanskje ut av CV-en.", &[])];
    for part in Part::ALL {
        let items: Vec<&Item> = sheet.part(part).collect();
        if items.is_empty() {
            continue;
        }
        out.push(String::new());
        out.push(tr(part.title(), &[]).to_uppercase());
        for item in items {
            out.push(String::new());
            out.push(format!("- {}", headline(item, tr)));
            if !item.subject.is_empty() {
                out.push(format!("  {}", item.question.render(tr)));
            }
            if let Some(w) = where_line(item, tr) {
                out.push(format!("  {w}"));
            }
            let note = notes(&item.id);
            if !note.trim().is_empty() {
                out.push(format!("  {}: {}", tr("Notater", &[]), note.trim().replace('\n', "\n    ")));
            }
        }
    }
    format!("{}\n", out.join("\n"))
}

/// The sheet as a document for PDF: one column, the CV's own font and colours, a line to
/// write on under each question that has no note yet. `lang` is the language of the words
/// (the menu language), for the PDF's language tag and right-to-left text.
pub fn to_document(p: &Project, sheet: &Sheet, notes: &dyn Fn(&str) -> String, tr: Tr<'_>, lang: &str) -> Document {
    let l = effective(p);
    let mut doc = Document::new();
    doc.core.title = title(p, tr);
    doc.core.creator = p.candidate.name.clone();
    let g = crate::document::Geometry::of(p);
    let sec = &mut doc.last_section;
    sec.page_w = g.page_w;
    sec.page_h = g.page_h;
    sec.margin_top = g.top;
    sec.margin_left = g.left;
    sec.margin_right = g.right;
    sec.margin_bottom = g.bottom;
    let ink = rgb(&l.ink);
    let muted = rgb(&l.muted);
    let accent = rgb(&l.accent);
    let chars = |size: f32, bold: bool, color: Rgb| CharProps { font: Some(l.body_font.clone()), size: Some(size), bold: Some(bold), color: Some(TextColor::Rgb(color)), ..Default::default() };
    let para = |text: &str, props: CharProps, before: f32, after: f32| {
        let mut para = Paragraph::new();
        let _ = para.insert_text(0, text, &props);
        para.props.space_before = Some(before);
        para.props.space_after = Some(after);
        para.props.line_spacing = Some(LineSpacing::Multiple(1.15));
        para
    };
    let size = 10.5;
    let mut body = vec![para(&title(p, tr), chars(17.0, true, ink), 0.0, 4.0)];
    body.push(para(&tr("Bygd fra CV-en og annonsen. Kan du ikke snakke om en linje, bør den kanskje ut av CV-en.", &[]), chars(size - 1.0, false, muted), 0.0, 6.0));
    for part in Part::ALL {
        let items: Vec<&Item> = sheet.part(part).collect();
        if items.is_empty() {
            continue;
        }
        let mut h = para(&tr(part.title(), &[]), chars(12.5, true, accent), 14.0, 4.0).styled("Heading1");
        h.props.keep_next = Some(true);
        h.props.outline_level = Some(0);
        h.props.borders = Some(Borders { bottom: Some(Border { style: BorderStyle::Single, width: 0.6, color: Some(rgb(&l.rule)), space: 2.0 }), ..Default::default() });
        body.push(h);
        for item in items {
            let mut head = para(&headline(item, tr), chars(size, true, ink), 7.0, 1.0);
            head.props.keep_next = Some(true);
            body.push(head);
            if !item.subject.is_empty() {
                let mut q = para(&item.question.render(tr), chars(size, false, ink), 0.0, 1.0);
                q.props.keep_next = Some(true);
                body.push(q);
            }
            if let Some(w) = where_line(item, tr) {
                let mut q = para(&w, chars(size - 1.0, false, muted), 0.0, 1.0);
                q.props.keep_next = Some(true);
                body.push(q);
            }
            let note = notes(&item.id);
            if note.trim().is_empty() {
                // An empty line with a rule under it, to write on.
                let mut line = para(" ", chars(size, false, ink), 10.0, 2.0);
                line.props.borders = Some(Borders { bottom: Some(Border { style: BorderStyle::Single, width: 0.4, color: Some(rgb(&l.rule)), space: 1.0 }), ..Default::default() });
                body.push(line);
            } else {
                body.push(para(&format!("{}: {}", tr("Notater", &[]), note.trim()), chars(size, false, ink), 2.0, 2.0));
            }
        }
    }
    doc.body = body.into_iter().map(|p| Arc::new(Block::Para(p))).collect();
    let styles = Arc::make_mut(&mut doc.styles);
    styles.default_chr.lang = Some(lang.to_string());
    if crate::headings::is_right_to_left(lang) {
        styles.default_chr.lang_bidi = Some(lang.to_string());
        styles.default_para.bidi = Some(true);
        doc.last_section.rtl = true;
    }
    doc.ensure_nonempty();
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CustomItem, CustomSection, Education, Job, Skill, SpokenLanguage};

    /// Norwegian, placeholders filled: what the app shows with the Norwegian menus.
    fn no(key: &'static str, args: &[(&str, &str)]) -> String {
        let mut s = key.to_string();
        for (k, v) in args {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    fn project() -> Project {
        let mut p = Project::default();
        p.candidate.headline = "Prosjektkoordinator med blikk for drift".into();
        p.target.role = "Driftskoordinator".into();
        p.target.company = "Nordvik Energi".into();
        p.target.description = "Om stillingen\nDu får ansvar for drift.\nKvalifikasjoner:\n- Erfaring med SAP\n- Fagbrev som elektriker\n- Gode kunnskaper i Excel\nDet er en fordel med Power BI.\nVi tilbyr gode betingelser.".into();
        p.target.keywords = vec!["SAP".into(), "fagbrev".into(), "Excel".into(), "Power BI".into(), "sap ".into()];
        p.experience.push(Job {
            company: "Fjellvegg AS".into(),
            title: "Koordinator".into(),
            start: "2021-09".into(),
            end: None,
            bullets: vec!["Kortet ned ventetiden med 20 % ved å planlegge i SAP".into(), "Hadde ansvar for innkjøp".into()],
            ..Default::default()
        });
        p.experience.push(Job {
            company: "Bygg AS".into(),
            title: "Lagermedarbeider".into(),
            start: "2017-01".into(),
            end: Some("2020-06".into()),
            bullets: vec!["Plukket ordrer til 40 kunder hver dag".into()],
            ..Default::default()
        });
        p.education.push(Education { qualification: "Fagbrev logistikk".into(), institution: "Vg3".into(), start: "2014".into(), end: "2016".into() });
        p.skills.push(Skill { name: "Excel".into(), ..Default::default() });
        p.languages.push(SpokenLanguage { name: "Engelsk".into(), level: "flytende".into() });
        p
    }

    fn today() -> i32 {
        2026 * 12 + 10
    }

    #[test]
    fn claims_are_the_lines_with_a_number_or_a_result() {
        let s = sheet(&project(), today());
        let claims: Vec<&str> = s.part(Part::Claims).map(|i| i.subject.as_str()).collect();
        assert_eq!(claims, ["Kortet ned ventetiden med 20 % ved å planlegge i SAP", "Plukket ordrer til 40 kunder hver dag"]);
        let first = s.part(Part::Claims).next().unwrap();
        assert_eq!(first.places, [Place::Cv("Koordinator – Fjellvegg AS".into())]);
        assert_eq!(where_line(first, &no).as_deref(), Some("Fra: Koordinator – Fjellvegg AS"));
        // A version number is not a result.
        let mut p = Project::default();
        p.experience.push(Job { company: "X".into(), bullets: vec!["Programmerte i TwinCAT 3".into()], start: "2020-01".into(), ..Default::default() });
        assert_eq!(sheet(&p, today()).part(Part::Claims).count(), 0);
    }

    #[test]
    fn requirements_say_where_the_cv_shows_them() {
        let s = sheet(&project(), today());
        assert!(!s.suggested);
        let ad: Vec<(&str, Weight)> = s.part(Part::Ad).map(|i| (i.subject.as_str(), i.weight)).collect();
        // Musts first; «sap » is the same keyword as «SAP».
        assert_eq!(ad, [("SAP", Weight::Must), ("fagbrev", Weight::Must), ("Excel", Weight::Must)]);
        let sap = s.part(Part::Ad).find(|i| i.subject == "SAP").unwrap();
        assert_eq!(sap.question.render(&no), "Gi et konkret eksempel fra Koordinator – Fjellvegg AS: hva du gjorde og hva det førte til.");
        // Shown by an education.
        let fagbrev = s.part(Part::Ad).find(|i| i.subject == "fagbrev").unwrap();
        assert_eq!(fagbrev.places, [Place::Cv("Fagbrev logistikk – Vg3".into())]);
        // Only in the skills list: have an example ready.
        let excel = s.part(Part::Ad).find(|i| i.subject == "Excel").unwrap();
        assert_eq!(excel.places, [Place::Section("Kjernekompetanse")]);
        assert!(excel.question.key.starts_with("Det står i CV-en, men ingen jobb"));
        assert_eq!(where_line(excel, &no).as_deref(), Some("CV-en viser det: Kjernekompetanse"));
        assert_eq!(headline(excel, &no), "Excel (Krav)");
    }

    #[test]
    fn gaps_are_missing_requirements_and_time_without_a_job() {
        let s = sheet(&project(), today());
        let gaps: Vec<&Item> = s.part(Part::Gaps).collect();
        assert_eq!(gaps[0].subject, "Power BI");
        assert_eq!(gaps[0].weight, Weight::Nice);
        assert_eq!(headline(gaps[0], &no), "Power BI (Ønskelig)");
        assert_eq!(where_line(gaps[0], &no), None);
        // 06.2020 → 09.2021: fourteen months.
        assert_eq!(gaps[1].question.render(&no), "Mellom Bygg AS og Fjellvegg AS er det 14 måneder uten jobb i CV-en. Hva gjorde du da?");
        assert_eq!(gaps.len(), 2);
    }

    #[test]
    fn the_usual_questions_name_the_job_and_the_current_employer() {
        let s = sheet(&project(), today());
        let always: Vec<String> = s.part(Part::Always).map(|i| i.question.render(&no)).collect();
        assert_eq!(
            always,
            [
                "Fortell kort om deg selv: de tre tingene du vil at de skal huske.",
                "Hvorfor vil du jobbe som Driftskoordinator hos Nordvik Energi, og hvorfor nå?",
                "Profilen din sier «Prosjektkoordinator med blikk for drift». Hva er et eksempel som viser det?",
                "Hvorfor vil du slutte hos Fjellvegg AS?",
                "Hva vet du om Nordvik Energi, og hva vil du spørre dem om?",
            ]
        );
    }

    #[test]
    fn an_empty_cv_still_has_the_usual_questions() {
        let s = sheet(&Project::default(), today());
        assert!(s.part(Part::Claims).next().is_none() && s.part(Part::Ad).next().is_none() && s.part(Part::Gaps).next().is_none());
        let always: Vec<&str> = s.part(Part::Always).map(|i| i.question.key).collect();
        assert_eq!(always, ["Fortell kort om deg selv: de tre tingene du vil at de skal huske.", "Hvorfor søker du denne jobben, og hvorfor nå?", "Hva vil du spørre dem om?"]);
    }

    #[test]
    fn without_a_keyword_list_the_terms_come_from_the_ad() {
        let mut p = project();
        p.target.keywords.clear();
        p.target.description = "Krav: erfaring med TwinCAT og CODESYS.".into();
        let s = sheet(&p, today());
        assert!(s.suggested);
        let missing: Vec<(&str, Weight)> = s.part(Part::Gaps).filter(|i| !i.subject.is_empty()).map(|i| (i.subject.as_str(), i.weight)).collect();
        assert!(missing.contains(&("TwinCAT", Weight::Must)), "{missing:?}");
        assert!(missing.contains(&("CODESYS", Weight::Must)), "{missing:?}");
    }

    #[test]
    fn own_sections_count_as_evidence_and_claims() {
        let mut p = Project::default();
        p.target.keywords = vec!["Python".into()];
        p.custom_sections.push(CustomSection {
            id: "projects".into(),
            title: "Prosjekter".into(),
            items: vec![CustomItem { title: "Værstasjon".into(), subtitle: "Hobby".into(), date: "2023".into(), bullets: vec!["Skrev den i Python for 40 brukere".into()] }],
        });
        let s = sheet(&p, today());
        let python = s.part(Part::Ad).next().expect("shown");
        assert_eq!(python.places, [Place::Cv("Værstasjon – Hobby".into())]);
        assert_eq!(s.part(Part::Claims).next().map(|i| i.subject.as_str()), Some("Skrev den i Python for 40 brukere"));
    }

    #[test]
    fn ids_stay_the_same_and_differ_between_items() {
        let a = sheet(&project(), today());
        let b = sheet(&project(), today());
        assert_eq!(a.items.iter().map(|i| &i.id).collect::<Vec<_>>(), b.items.iter().map(|i| &i.id).collect::<Vec<_>>());
        let ids: HashSet<&String> = a.items.iter().map(|i| &i.id).collect();
        assert_eq!(ids.len(), a.items.len());
        // A very long line still gets a short id, cut on a character boundary.
        let mut p = Project::default();
        p.experience.push(Job { company: "Æ".into(), start: "2020-01".into(), bullets: vec![format!("Økte salget med 30 % {}", "ø".repeat(200))], ..Default::default() });
        let s = sheet(&p, today());
        assert!(s.items[0].id.len() <= 90);
    }

    #[test]
    fn the_text_has_every_part_and_the_notes() {
        let p = project();
        let s = sheet(&p, today());
        let notes = |id: &str| if id.starts_with("need:sap") { "Viste innkjøpsflyten\nfor ledelsen".to_string() } else { String::new() };
        let text = to_text(&p, &s, &notes, &no);
        assert!(text.starts_with("Forbered intervjuet: Driftskoordinator – Nordvik Energi\n"));
        for part in Part::ALL {
            assert!(text.contains(&part.title().to_uppercase()), "{part:?} missing");
        }
        assert!(text.contains("- «Kortet ned ventetiden med 20 % ved å planlegge i SAP»\n  Hva gjorde du selv"));
        assert!(text.contains("- SAP (Krav)\n"));
        assert!(text.contains("  Notater: Viste innkjøpsflyten\n    for ledelsen\n"));
        // An empty CV has no empty parts.
        let empty = to_text(&Project::default(), &sheet(&Project::default(), today()), &|_| String::new(), &no);
        assert!(!empty.contains(&Part::Claims.title().to_uppercase()));
        assert!(empty.contains(&Part::Always.title().to_uppercase()));
    }

    #[test]
    fn the_pdf_has_real_text() {
        let p = project();
        let s = sheet(&p, today());
        let doc = to_document(&p, &s, &|id| if id == "why" { "Jeg vil bygge noe".into() } else { String::new() }, &no, "nb");
        let bytes = crate::document::to_pdf(&doc).expect("pdf");
        assert!(bytes.starts_with(b"%PDF"));
        let text = crate::import::text_from_bytes(&bytes, "pdf").expect("read back");
        for want in ["Forbered intervjuet", "SAP (Krav)", "Notater: Jeg vil bygge noe", "Fagbrev logistikk"] {
            assert!(text.contains(want), "«{want}» not in the PDF text:\n{text}");
        }
        // Right to left when the words are.
        let ar = to_document(&p, &s, &|_| String::new(), &no, "ar");
        assert!(ar.last_section.rtl);
    }

    /// Writes the sheet as PDF in other scripts, with the bundled packs' words, to look at:
    /// `CVBUILDER_SHEET_DIR=<dir> cargo test -p cv-core --release sheet_pdfs -- --ignored`
    #[test]
    #[ignore = "writes files to look at"]
    fn sheet_pdfs() {
        let dir = std::path::PathBuf::from(std::env::var("CVBUILDER_SHEET_DIR").expect("CVBUILDER_SHEET_DIR"));
        let p = project();
        let s = sheet(&p, today());
        for code in ["no", "ja", "hi", "ar"] {
            let strings: std::collections::HashMap<String, String> = if code == "no" {
                Default::default()
            } else {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/languages/{code}.cvlang.json"));
                let pack: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("pack")).expect("json");
                serde_json::from_value(pack["strings"].clone()).expect("strings")
            };
            let tr = |key: &'static str, args: &[(&str, &str)]| {
                let template = strings.get(key).map(String::as_str).unwrap_or(key);
                no(Box::leak(template.to_string().into_boxed_str()), args)
            };
            let doc = to_document(&p, &s, &|id| if id == "why" { "Note 1".into() } else { String::new() }, &tr, code);
            std::fs::write(dir.join(format!("sheet-{code}.pdf")), crate::document::to_pdf(&doc).expect("pdf")).expect("write");
            std::fs::write(dir.join(format!("sheet-{code}.txt")), to_text(&p, &s, &|_| String::new(), &tr)).expect("write");
        }
    }

    #[test]
    fn every_key_is_listed() {
        let mut p = project();
        p.target.role.clear();
        let mut used: Vec<&str> = Part::ALL.iter().map(|p| p.title()).collect();
        for s in [sheet(&project(), today()), sheet(&p, today()), sheet(&Project::default(), today())] {
            used.extend(s.items.iter().map(|i| i.question.key));
        }
        for key in used {
            assert!(KEYS.contains(&key), "«{key}» is not in KEYS");
        }
    }
}
