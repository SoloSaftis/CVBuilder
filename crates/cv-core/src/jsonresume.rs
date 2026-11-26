//! JSON Resume (<https://jsonresume.org/schema>, v1.0.0): reading a `resume.json` into a CV and
//! writing a CV out as one, offline. The open format other tools (Reactive Resume, RenderCV, the
//! jsonresume.org themes) read and write, so a CV can leave CVBuilder in a documented form.
//!
//! Reading maps field for field; what the CV has no place for (scores, courses, references, URLs
//! of employers …) is returned as text for «Dette må plasseres». Writing keeps everything the
//! schema has a place for. Sections of the person's own go to `projects`, `volunteer`, `awards` or
//! `publications` by their title, each item with the section's title in `x-section` and, when the
//! schema's dates cannot say it exactly, its date as written in `x-date`, so reading the file back
//! gives the same sections.
use serde_json::{Map, Value, json};

use crate::checks;
use std::collections::HashSet;

use crate::cvread::{CvRead, MAX_LIST, looks_english, month_or_year};
use crate::linkedin::{count, lines};
use crate::model::{Certification, ContactField, CustomItem, CustomSection, Education, Interest, Job, Language, Project, Skill, SpokenLanguage};

/// The schema the written file says it follows.
pub const SCHEMA: &str = "https://raw.githubusercontent.com/jsonresume/resume-schema/v1.0.0/schema.json";

/// The top-level keys of the JSON Resume schema.
const KEYS: [&str; 13] = ["basics", "work", "volunteer", "education", "awards", "certificates", "publications", "skills", "languages", "interests", "references", "projects", "meta"];

/// Whether a JSON text is a JSON Resume rather than a CVBuilder project: it has `basics` or
/// `work`, and none of the project's own keys.
pub fn is_json_resume(text: &str) -> bool {
    serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).is_ok_and(|v| is_resume(&v))
}

fn is_resume(v: &Value) -> bool {
    let Value::Object(o) = v else { return false };
    (o.contains_key("basics") || o.contains_key("work")) && !o.contains_key("candidate") && !o.contains_key("experience")
}

/// A JSON text read as a JSON Resume, when it is one (parsed once); `None` for other JSON (a
/// CVBuilder draft) and for text that is not JSON.
pub fn read_if_resume(text: &str) -> Option<Result<CvRead, String>> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    is_resume(&v).then(|| from_value(&v))
}

/// Text of a field; numbers are written out, everything else is empty.
fn s(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(t)) => t.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// A list of the file, at most its first [`MAX_LIST`] entries.
fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    let list = v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    &list[..list.len().min(MAX_LIST)]
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    arr(v, key).iter().filter_map(Value::as_str).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// ISO 8601 as the schema writes it («2021-03-15», «2021-03», «2021») → `YYYY-MM` or a year.
fn date(text: &str) -> String {
    let t = text.trim();
    // «2021-03-15» and «2021-03-15T00:00:00Z»: the day is not kept.
    let t = if t.len() >= 10 && t.as_bytes()[4] == b'-' && t.as_bytes()[7] == b'-' { &t[..7] } else { t };
    month_or_year(t).unwrap_or_default()
}

fn year(text: &str) -> String {
    checks::parse_year(text).unwrap_or_default()
}

/// «03.2021 – 06.2022», «2021 – nå» or «2021», as other imported sections write dates.
fn span(start: &str, end: &str, english: bool) -> String {
    let (start, end) = (date(start), date(end));
    match (start.is_empty(), end.is_empty()) {
        (true, true) => String::new(),
        (true, false) => checks::show_month(&end),
        (false, true) => format!("{} – {}", checks::show_month(&start), if english { "Present" } else { "nå" }),
        (false, false) if start == end => checks::show_month(&start),
        (false, false) => format!("{} – {}", checks::show_month(&start), checks::show_month(&end)),
    }
}

/// The section titles CVBuilder gives the schema's own lists, in Norwegian and English.
const VOLUNTEER: (&str, &str) = ("Frivillig arbeid", "Volunteering");
const AWARDS: (&str, &str) = ("Utmerkelser", "Awards");
const PUBLICATIONS: (&str, &str) = ("Publikasjoner", "Publications");
const PROJECTS: (&str, &str) = ("Prosjekter", "Projects");

/// Reads a JSON Resume. `found` lists what was read («jobber (3)» …), `rest` what has no place.
pub fn read(text: &str) -> Result<CvRead, String> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| crate::plain::with_details("JSON Resume-filen kunne ikke leses. Den kan være skadet.", &e.to_string()))?;
    from_value(&v)
}

/// The largest photo taken from `basics.image` (the `data:` text, bytes), and its largest side in
/// pixels. CVBuilder's own photos are a few hundred kilobytes.
const MAX_PHOTO: usize = 2_000_000;
const MAX_PHOTO_SIDE: u32 = 4000;

/// `basics.image` as CVBuilder's photo: a JPEG `data:` URL that decodes, of a sensible size.
fn photo(image: &str) -> Option<String> {
    use base64::Engine as _;
    let encoded = image.strip_prefix("data:image/jpeg;base64,")?;
    if image.len() > MAX_PHOTO {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.trim()).ok()?;
    let (w, h) = crate::document::jpeg_size(&bytes)?;
    (w <= MAX_PHOTO_SIDE && h <= MAX_PHOTO_SIDE).then(|| format!("data:image/jpeg;base64,{}", encoded.trim()))
}

/// The most text kept from the keys the schema does not have (bytes, all of them together).
const MAX_UNKNOWN: usize = 20_000;

fn from_value(v: &Value) -> Result<CvRead, String> {
    if !v.is_object() {
        return Err("Filen er ikke en JSON Resume.".into());
    }
    let mut p = Project::default();
    let mut found: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();

    // The CV's language follows the words the person wrote.
    let mut prose = String::new();
    let b = v.get("basics").cloned().unwrap_or(Value::Null);
    prose.push_str(&format!("{}\n{}\n", s(&b, "label"), s(&b, "summary")));
    for w in arr(&v, "work").iter().chain(arr(&v, "volunteer")).chain(arr(&v, "projects")) {
        prose.push_str(&format!("{} {} {}\n", s(w, "summary"), s(w, "description"), strings(w, "highlights").join("\n")));
    }
    let english = match v.get("meta").map(|m| s(m, "language")).as_deref() {
        Some("nb" | "no" | "nn") => false,
        Some(code) if !code.is_empty() => true,
        _ => looks_english(&prose),
    };
    if english {
        p.language = Language::En;
    }
    let pick = |(no, en): (&str, &str)| if english { en.to_string() } else { no.to_string() };

    // ---- basics ----
    let c = &mut p.candidate;
    c.name = s(&b, "name");
    c.headline = s(&b, "label");
    c.email = s(&b, "email");
    c.phone = s(&b, "phone");
    c.summary = s(&b, "summary").replace("\r\n", "\n");
    if let Some(loc) = b.get("location") {
        c.city = s(loc, "city");
        if c.city.is_empty() {
            c.city = s(loc, "region");
        }
        let other: Vec<String> = ["address", "postalCode", "region", "countryCode"].iter().map(|k| s(loc, k)).filter(|t| !t.is_empty() && *t != c.city).collect();
        if !other.is_empty() {
            rest.push(format!("{}: {}", pick(("Adresse", "Address")), other.join(", ")));
        }
    }
    let image = s(&b, "image");
    if let Some(photo) = photo(&image) {
        c.photo = photo;
        p.options.photo = true;
    } else if !image.is_empty() && !image.starts_with("data:") {
        // A link is kept for the person; a picture CVBuilder cannot use (not a JPEG, too large,
        // or broken) is left out, since its text is no help to them.
        rest.push(format!("{}: {image}", pick(("Bilde", "Photo"))));
    }
    let url = s(&b, "url");
    if !url.is_empty() {
        c.extra.push(ContactField { id: "web".into(), label: String::new(), value: url });
    }
    for (i, profile) in arr(&b, "profiles").iter().enumerate() {
        let (network, username, link) = (s(profile, "network"), s(profile, "username"), s(profile, "url"));
        let value = if link.is_empty() { username } else { link };
        if value.is_empty() {
            continue;
        }
        if c.linkedin.is_empty() && (network.eq_ignore_ascii_case("linkedin") || value.to_lowercase().contains("linkedin.com")) {
            c.linkedin = value;
        } else {
            c.extra.push(ContactField { id: format!("contact_imported_{}", i + 1), label: network, value });
        }
    }
    for (value, word) in [(&c.name, "navn"), (&c.headline, "tittel"), (&c.summary, "profil"), (&c.city, "sted"), (&c.email, "e-post"), (&c.phone, "telefon"), (&c.linkedin, "LinkedIn")] {
        if !value.is_empty() {
            found.push(word.into());
        }
    }

    // ---- work ----
    for w in arr(&v, "work") {
        let (company, title) = (if s(w, "name").is_empty() { s(w, "company") } else { s(w, "name") }, s(w, "position"));
        if company.is_empty() && title.is_empty() {
            continue;
        }
        let mut bullets = lines(&s(w, "summary"));
        bullets.extend(lines(&s(w, "description")));
        bullets.extend(strings(w, "highlights").iter().flat_map(|h| lines(h)));
        let end = s(w, "endDate");
        let end = if end.is_empty() || checks::is_now(&end) { None } else { Some(date(&end)) };
        if !s(w, "url").is_empty() {
            rest.push(format!("{company}: {}", s(w, "url")));
        }
        let n = p.experience.len() + 1;
        p.experience.push(Job { bullet_ids: Vec::new(), id: format!("job_imported_{n}"), company, title, location: s(w, "location"), start: date(&s(w, "startDate")), end, bullets, evidence: String::new(), confirmed: true });
    }
    count(&mut found, "jobber", p.experience.len());

    // ---- education ----
    for e in arr(&v, "education") {
        let institution = s(e, "institution");
        let (study, area) = (s(e, "studyType"), s(e, "area"));
        let qualification = match (study.is_empty(), area.is_empty()) {
            (false, false) => format!("{study}, {area}"),
            (false, true) => study,
            _ => area,
        };
        if institution.is_empty() && qualification.is_empty() {
            continue;
        }
        let score = s(e, "score");
        let courses = strings(e, "courses");
        if !score.is_empty() || !courses.is_empty() {
            let mut line = format!("{} ({institution})", if qualification.is_empty() { institution.clone() } else { qualification.clone() });
            if !score.is_empty() {
                line.push_str(&format!(" – {}: {score}", pick(("Karakter", "Score"))));
            }
            if !courses.is_empty() {
                line.push_str(&format!(" – {}: {}", pick(("Emner", "Courses")), courses.join(", ")));
            }
            rest.push(line);
        }
        p.education.push(Education { id: String::new(), qualification, institution, start: date(&s(e, "startDate")), end: date(&s(e, "endDate")) });
    }
    count(&mut found, "utdanning", p.education.len());

    // ---- skills: a skill with keywords is a group of skills ----
    let mut seen: HashSet<String> = HashSet::new();
    for k in arr(v, "skills") {
        let (name, keywords) = (s(k, "name"), strings(k, "keywords"));
        let mut add = |group: &str, name: &str| {
            if !name.is_empty() && p.skills.len() < MAX_LIST && seen.insert(name.to_ascii_lowercase()) {
                p.skills.push(Skill { id: String::new(), group: group.to_string(), name: name.to_string(), evidence: String::new(), confirmed: true });
            }
        };
        if keywords.is_empty() {
            add("", &name);
        } else {
            for kw in &keywords {
                add(&name, kw);
            }
        }
        let level = s(k, "level");
        if !level.is_empty() && !name.is_empty() {
            rest.push(format!("{name}: {level}"));
        }
    }
    count(&mut found, "kompetanse", p.skills.len());

    // ---- certificates ----
    for k in arr(&v, "certificates") {
        let name = s(k, "name");
        if !name.is_empty() {
            p.certifications.push(Certification { id: String::new(), name, issuer: s(k, "issuer"), year: year(&s(k, "date")) });
        }
    }
    count(&mut found, "kurs", p.certifications.len());

    // ---- languages ----
    for l in arr(&v, "languages") {
        let name = s(l, "language");
        if !name.is_empty() {
            p.languages.push(SpokenLanguage { id: String::new(), name, level: s(l, "fluency") });
        }
    }
    count(&mut found, "språk", p.languages.len());

    // ---- interests ----
    for i in arr(&v, "interests") {
        let label = s(i, "name");
        if !label.is_empty() {
            let tags = strings(i, "keywords");
            let id = format!("interest_imported_{}", p.interests.len() + 1);
            p.interests.push(Interest { id, label, category: "other".into(), tags, specificity: 1, confirmed: true, ..Default::default() });
        }
    }
    count(&mut found, "interesser", p.interests.len());

    // ---- sections of their own ----
    let section = |title: String, items: Vec<CustomItem>, p: &mut Project| {
        if items.is_empty() {
            return;
        }
        if let Some(s) = p.custom_sections.iter_mut().find(|s| s.title == title) {
            s.items.extend(items);
        } else {
            let id = format!("section_imported_{}", p.custom_sections.len() + 1);
            p.custom_sections.push(CustomSection { id, title, items });
        }
    };
    // The title of the section an item came from in CVBuilder, and its date as it was written.
    let title_of = |x: &Value, default: (&str, &str)| if s(x, "x-section").is_empty() { pick(default) } else { s(x, "x-section") };
    let date_of = |x: &Value, start: &str, end: &str| if s(x, "x-date").is_empty() { span(&s(x, start), &s(x, end), english) } else { s(x, "x-date") };
    for x in arr(&v, "volunteer").iter().filter(|x| !s(x, "position").is_empty() || !s(x, "organization").is_empty()) {
        let mut bullets = lines(&s(x, "summary"));
        bullets.extend(strings(x, "highlights"));
        section(title_of(x, VOLUNTEER), vec![CustomItem { id: String::new(), bullet_ids: Vec::new(), title: s(x, "position"), subtitle: s(x, "organization"), date: date_of(x, "startDate", "endDate"), bullets }], &mut p);
    }
    for x in arr(&v, "awards").iter().filter(|x| !s(x, "title").is_empty()) {
        section(title_of(x, AWARDS), vec![CustomItem { id: String::new(), bullet_ids: Vec::new(), title: s(x, "title"), subtitle: s(x, "awarder"), date: date_of(x, "date", "date"), bullets: lines(&s(x, "summary")) }], &mut p);
    }
    for x in arr(&v, "publications").iter().filter(|x| !s(x, "name").is_empty()) {
        let mut bullets = lines(&s(x, "summary"));
        if !s(x, "url").is_empty() {
            bullets.push(s(x, "url"));
        }
        section(title_of(x, PUBLICATIONS), vec![CustomItem { id: String::new(), bullet_ids: Vec::new(), title: s(x, "name"), subtitle: s(x, "publisher"), date: date_of(x, "releaseDate", "releaseDate"), bullets }], &mut p);
    }
    for x in arr(&v, "projects").iter().filter(|x| !s(x, "name").is_empty()) {
        let mut bullets = lines(&s(x, "description"));
        bullets.extend(strings(x, "highlights"));
        if !s(x, "url").is_empty() {
            bullets.push(s(x, "url"));
        }
        let subtitle = if s(x, "entity").is_empty() { strings(x, "roles").join(", ") } else { s(x, "entity") };
        section(title_of(x, PROJECTS), vec![CustomItem { id: String::new(), bullet_ids: Vec::new(), title: s(x, "name"), subtitle, date: date_of(x, "startDate", "endDate"), bullets }], &mut p);
    }
    count(&mut found, "andre seksjoner", p.custom_sections.len().min(1));

    // ---- what has no place ----
    for r in arr(&v, "references") {
        let (name, reference) = (s(r, "name"), s(r, "reference"));
        if !name.is_empty() || !reference.is_empty() {
            rest.push(format!("{}: {name} – {reference}", pick(("Referanse", "Reference"))));
        }
    }
    if let Value::Object(o) = v {
        // The keys the schema does not have, written out up to [`MAX_UNKNOWN`] in all.
        let mut room = MAX_UNKNOWN;
        for (k, value) in o.iter().filter(|(k, _)| !KEYS.contains(&k.as_str()) && !k.starts_with('$')) {
            let line = format!("{k}: {value}");
            if line.len() > room {
                rest.push(format!("{} …", crate::europass::cut(&line, room)));
                break;
            }
            room -= line.len();
            rest.push(line);
        }
    }

    if !p.has_content() {
        return Err("JSON Resume-filen har ingen profil, jobber eller utdanning å hente inn.".into());
    }
    Ok(CvRead { project: p, rest: rest.join("\n"), found })
}

/// A non-empty text field.
fn put(o: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.trim().is_empty() {
        o.insert(key.into(), Value::String(value.trim().to_string()));
    }
}

/// `YYYY-MM` or a year as the schema's ISO 8601; anything else is left out.
fn iso(value: &str) -> Option<String> {
    let v = value.trim();
    let ok = (v.len() == 4 && v.chars().all(|c| c.is_ascii_digit())) || (v.len() == 7 && v.as_bytes()[4] == b'-' && v.chars().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit()));
    ok.then(|| v.to_string())
}

/// A free-text date of a section item («03.2021 – 06.2022», «2021 – nå», «2023») as a start and
/// an end date, when it reads as one.
fn split_span(text: &str) -> Option<(String, Option<String>)> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let (a, b) = match t.split_once(['–', '—']).or_else(|| t.split_once(" - ")) {
        Some((a, b)) => (a.trim(), Some(b.trim())),
        None => (t, None),
    };
    let start = month_or_year(a)?;
    match b {
        None => Some((start, None)),
        Some(b) if checks::is_now(b) => Some((start, Some(String::new()))),
        Some(b) => Some((start, Some(month_or_year(b)?))),
    }
}

/// Which of the schema's lists a section of the person's own belongs in, by its title.
fn kind_of(title: &str) -> &'static str {
    let t = title.trim().to_lowercase();
    let is = |words: &[&str]| words.iter().any(|w| t == *w);
    if is(&["prosjekter", "projects", "project", "prosjekt", "utvalgte prosjekter", "selected projects"]) {
        "projects"
    } else if is(&["frivillig arbeid", "frivillig", "verv", "verv og frivillig arbeid", "volunteering", "volunteer", "volunteer work", "volunteer experience"]) {
        "volunteer"
    } else if is(&["utmerkelser", "priser", "priser og utmerkelser", "awards", "honours", "honors", "awards and honours"]) {
        "awards"
    } else if is(&["publikasjoner", "publications", "publikasjoner og foredrag"]) {
        "publications"
    } else {
        "other"
    }
}

/// The CV as a JSON Resume (pretty, two-space indent). The job ad, the design and CVBuilder's own
/// notes (evidence, approvals) are not part of it.
pub fn write(p: &Project) -> String {
    // An export of this copy of the CV: hidden items are left out, as in the PDF.
    let p = &*p.visible();
    let c = &p.candidate;
    let mut basics = Map::new();
    put(&mut basics, "name", &c.name);
    put(&mut basics, "label", &c.headline);
    // The photo only when it is switched on, as in the PDF: a photo turned off stays private.
    if p.options.photo && !c.photo.is_empty() {
        put(&mut basics, "image", &c.photo);
    }
    put(&mut basics, "email", &c.email);
    put(&mut basics, "phone", &c.phone);
    if let Some(web) = c.extra.iter().find(|e| e.id == "web" && e.label.is_empty()) {
        put(&mut basics, "url", &web.value);
    }
    put(&mut basics, "summary", &c.summary);
    if !c.city.trim().is_empty() {
        basics.insert("location".into(), json!({ "city": c.city.trim() }));
    }
    let mut profiles = Vec::new();
    if !c.linkedin.trim().is_empty() {
        let v = c.linkedin.trim();
        let mut o = Map::new();
        o.insert("network".into(), "LinkedIn".into());
        if v.contains('/') || v.contains('.') {
            o.insert("url".into(), v.into());
        } else {
            o.insert("username".into(), v.into());
        }
        profiles.push(Value::Object(o));
    }
    for e in c.extra.iter().filter(|e| !(e.id == "web" && e.label.is_empty()) && !e.value.trim().is_empty()) {
        let v = e.value.trim();
        let mut o = Map::new();
        put(&mut o, "network", &e.label);
        let link = v.starts_with("http://") || v.starts_with("https://") || v.starts_with("www.");
        o.insert(if link { "url" } else { "username" }.into(), v.into());
        profiles.push(Value::Object(o));
    }
    if !profiles.is_empty() {
        basics.insert("profiles".into(), Value::Array(profiles));
    }

    let work: Vec<Value> = p
        .experience
        .iter()
        .map(|j| {
            let mut o = Map::new();
            put(&mut o, "name", &j.company);
            put(&mut o, "position", &j.title);
            put(&mut o, "location", &j.location);
            if let Some(d) = iso(&j.start) {
                o.insert("startDate".into(), d.into());
            }
            if let Some(d) = j.end.as_deref().and_then(iso) {
                o.insert("endDate".into(), d.into());
            }
            if !j.bullets.is_empty() {
                o.insert("highlights".into(), j.bullets.clone().into());
            }
            Value::Object(o)
        })
        .collect();

    let education: Vec<Value> = p
        .education
        .iter()
        .map(|e| {
            let mut o = Map::new();
            put(&mut o, "institution", &e.institution);
            put(&mut o, "studyType", &e.qualification);
            if let Some(d) = iso(&e.start) {
                o.insert("startDate".into(), d.into());
            }
            if let Some(d) = iso(&e.end) {
                o.insert("endDate".into(), d.into());
            }
            Value::Object(o)
        })
        .collect();

    // Skills in groups: a group is one skill with its skills as keywords, in the CV's order.
    let mut skills: Vec<(String, Vec<String>)> = Vec::new();
    for k in &p.skills {
        let group = k.group.trim();
        if group.is_empty() {
            skills.push((k.name.clone(), Vec::new()));
        } else if let Some(g) = skills.iter_mut().find(|(n, kw)| n == group && !kw.is_empty()) {
            g.1.push(k.name.clone());
        } else {
            skills.push((group.to_string(), vec![k.name.clone()]));
        }
    }
    let skills: Vec<Value> = skills.into_iter().map(|(name, kw)| if kw.is_empty() { json!({ "name": name }) } else { json!({ "name": name, "keywords": kw }) }).collect();

    let languages: Vec<Value> = p
        .languages
        .iter()
        .map(|l| {
            let mut o = Map::new();
            put(&mut o, "language", &l.name);
            put(&mut o, "fluency", &l.level);
            Value::Object(o)
        })
        .collect();
    let certificates: Vec<Value> = p
        .certifications
        .iter()
        .map(|k| {
            let mut o = Map::new();
            put(&mut o, "name", &k.name);
            put(&mut o, "issuer", &k.issuer);
            if let Some(d) = iso(&k.year) {
                o.insert("date".into(), d.into());
            }
            Value::Object(o)
        })
        .collect();
    let interests: Vec<Value> = p
        .interests
        .iter()
        .map(|i| {
            let mut o = Map::new();
            put(&mut o, "name", &i.label);
            if !i.tags.is_empty() {
                o.insert("keywords".into(), i.tags.clone().into());
            }
            Value::Object(o)
        })
        .collect();

    let (mut projects, mut volunteer, mut awards, mut publications) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for section in &p.custom_sections {
        let kind = kind_of(&section.title);
        for item in &section.items {
            let mut o = Map::new();
            let (title, by, start, end, list) = match kind {
                "volunteer" => ("position", "organization", "startDate", Some("endDate"), &mut volunteer),
                "awards" => ("title", "awarder", "date", None, &mut awards),
                "publications" => ("name", "publisher", "releaseDate", None, &mut publications),
                _ => ("name", "entity", "startDate", Some("endDate"), &mut projects),
            };
            put(&mut o, title, &item.title);
            put(&mut o, by, &item.subtitle);
            // The schema's dates where the text reads as them; the text itself when they would
            // not give it back exactly.
            let mut shown = String::new();
            match (split_span(&item.date), end) {
                (Some((s, None)), None) => {
                    shown = checks::show_month(&s);
                    o.insert(start.into(), s.into());
                }
                (Some((s, Some(e))), Some(end)) => {
                    shown = span(&s, &e, p.is_english());
                    o.insert(start.into(), s.into());
                    if !e.is_empty() {
                        o.insert(end.into(), e.into());
                    }
                }
                _ => {}
            }
            if shown != item.date.trim() {
                put(&mut o, "x-date", &item.date);
            }
            if !item.bullets.is_empty() {
                if end.is_some() {
                    o.insert("highlights".into(), item.bullets.clone().into());
                } else {
                    put(&mut o, "summary", &item.bullets.join("
"));
                }
            }
            o.insert("x-section".into(), section.title.clone().into());
            list.push(Value::Object(o));
        }
    }

    let mut out = Map::new();
    out.insert("$schema".into(), SCHEMA.into());
    out.insert("basics".into(), Value::Object(basics));
    for (key, list) in [("work", work), ("volunteer", volunteer), ("education", education), ("awards", awards), ("certificates", certificates), ("publications", publications), ("skills", skills), ("languages", languages), ("interests", interests), ("projects", projects)] {
        if !list.is_empty() {
            out.insert(key.into(), Value::Array(list));
        }
    }
    out.insert("meta".into(), json!({ "version": "v1.0.0", "language": p.language_code() }));
    serde_json::to_string_pretty(&Value::Object(out)).unwrap_or_default()
}
