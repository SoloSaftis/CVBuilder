//! Reading a Europass CV without AI: the `.xml` the Europass editor saves, or the PDF it saves
//! with that XML attached (`attachment.xml`). Two formats are read:
//!
//! - the Europass platform's format since 2020: root `Candidate` (namespace
//!   `http://www.europass.eu/1.0`, HR-XML with `oa:`, `hr:` and `eures:` parts);
//! - the older Europass CV (2013–2020): root `SkillsPassport` with `LearnerInfo`.
//!
//! Elements are found by their local name, whatever prefix the file gives them, so a namespace
//! written another way does no harm. Languages get their CEFR levels («B2», or «B1–C1» when the
//! self-assessment differs between skills), the driving licence becomes a contact line, and what
//! the CV has no field for (date of birth, nationality, prose skills, achievements) is returned as
//! text for «Dette må plasseres».
use std::collections::HashSet;

use crate::cvread::{CvRead, MAX_LIST, looks_english, month_or_year};
use crate::import::strip_html;
use crate::linkedin::{count, lines};
use crate::model::{Certification, ContactField, CustomItem, CustomSection, Education, Job, Language, Project, Skill, SpokenLanguage};

/// An element: its local name, attributes (local names) and text, with its children.
#[derive(Debug, Default, Clone)]
pub(crate) struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    /// The first child with this local name (case does not matter).
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// The children with this local name, at most [`MAX_LIST`] of them.
    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name.eq_ignore_ascii_case(name)).take(MAX_LIST)
    }

    /// The node at a path of local names, each the first of its name.
    fn at(&self, path: &[&str]) -> Option<&Node> {
        path.iter().try_fold(self, |n, p| n.child(p))
    }

    /// The trimmed text at a path; `""` when it is not there.
    fn get(&self, path: &[&str]) -> String {
        self.at(path).map(|n| n.text.trim().to_string()).unwrap_or_default()
    }

    fn attr(&self, name: &str) -> &str {
        self.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim()).unwrap_or("")
    }

    /// Every node below this one with this local name, in document order. Walked with a list of
    /// its own rather than by recursion, so no tree is too deep for it. At most [`MAX_LIST`].
    fn descendants(&self, name: &str) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut todo: Vec<&Node> = self.children.iter().rev().collect();
        while let Some(n) = todo.pop() {
            if n.name.eq_ignore_ascii_case(name) {
                out.push(n);
                if out.len() == MAX_LIST {
                    break;
                }
            }
            todo.extend(n.children.iter().rev());
        }
        out
    }
}

/// How deep elements may nest. A Europass CV nests about a dozen levels; a file nested far
/// deeper is not one, and refusing it keeps the reader's stack safe.
const MAX_DEPTH: usize = 100;
/// How many elements a file may have. A long Europass CV has a few thousand.
const MAX_NODES: usize = 100_000;
/// How much text one element keeps (bytes); the rest is left out.
const MAX_TEXT: usize = 1_000_000;
/// How many attributes an element keeps, how long a value may be, and how long a name (bytes).
const MAX_ATTRS: usize = 16;
const MAX_ATTR_VALUE: usize = 10_000;
const MAX_NAME: usize = 200;

/// The first `max` bytes of a text, cut at a character boundary.
pub(crate) fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Appends to an element's text, up to [`MAX_TEXT`].
fn push_text(node: &mut Node, piece: &str) {
    let room = MAX_TEXT.saturating_sub(node.text.len());
    node.text.push_str(cut(piece, room));
}

/// An XML text as a tree. Entities and character references are resolved; CDATA is kept as text.
/// A file nested deeper than [`MAX_DEPTH`] or with more than [`MAX_NODES`] elements is refused;
/// text and attributes are kept up to their limits.
pub(crate) fn parse(text: &str) -> Result<Node, String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut nodes = 0usize;
    let open = |e: &quick_xml::events::BytesStart| Node {
        name: cut(&String::from_utf8_lossy(e.local_name().as_ref()), MAX_NAME).to_string(),
        attrs: e
            .attributes()
            .flatten()
            .take(MAX_ATTRS)
            .map(|a| (cut(&String::from_utf8_lossy(a.key.local_name().as_ref()), MAX_NAME).to_string(), a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|v| cut(&v, MAX_ATTR_VALUE).to_string()).unwrap_or_default()))
            .collect(),
        ..Default::default()
    };
    let bad = |e: &dyn std::fmt::Display| crate::plain::with_details("Europass-filen kunne ikke leses. Den kan være skadet.", &e.to_string());
    loop {
        let event = reader.read_event().map_err(|e| bad(&e))?;
        if matches!(event, Event::Start(_) | Event::Empty(_)) {
            nodes += 1;
            if nodes > MAX_NODES {
                return Err(bad(&format!("more than {MAX_NODES} elements")));
            }
        }
        match event {
            Event::Start(e) => {
                // The stack holds the document node too, so its length is the depth of the new element.
                if stack.len() > MAX_DEPTH {
                    return Err(bad(&format!("elements nested deeper than {MAX_DEPTH} levels")));
                }
                stack.push(open(&e));
            }
            Event::Empty(e) => {
                let node = open(&e);
                if let Some(top) = stack.last_mut() {
                    top.children.push(node);
                }
            }
            Event::End(_) => {
                let node = stack.pop().unwrap_or_default();
                match stack.last_mut() {
                    Some(top) => top.children.push(node),
                    None => return Err(bad(&"end tag without start")),
                }
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    push_text(top, &t.decode().map_err(|e| bad(&e))?);
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    push_text(top, &String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Event::GeneralRef(r) => {
                let piece = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => match r.decode().map_err(|e| bad(&e))?.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        other => crate::import::entities(&format!("&{other};")),
                    },
                };
                if let Some(top) = stack.last_mut() {
                    push_text(top, &piece);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut root = stack.into_iter().next().unwrap_or_default();
    root.children.retain(|c| !c.name.is_empty());
    root.children.into_iter().next().ok_or_else(|| "Filen er ikke en Europass-CV.".to_string())
}

/// Whether a text is a Europass CV in XML.
pub fn is_europass(text: &str) -> bool {
    let head: String = text.chars().take(4000).collect();
    head.trim_start_matches('\u{feff}').trim_start().starts_with('<') && (head.contains("SkillsPassport") || head.contains("europass.eu/1.0") || head.contains("europass.cedefop.europa.eu")) && (head.contains("<SkillsPassport") || head.contains("Candidate"))
}

/// The largest attachment looked at, packed or unpacked (bytes). A Europass XML is well under a
/// megabyte; a bigger attachment is something else, or a file made to fill the memory.
pub(crate) const MAX_ATTACHMENT: usize = 5_000_000;
/// How many attached files are looked at, and how much is unpacked of one before its start shows
/// whether it is a Europass CV at all.
const MAX_ATTACHMENTS: usize = 20;
const HEAD: u64 = 64 * 1024;

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// The Europass XML attached to a PDF from the Europass editor, if there is one. Every embedded
/// file is looked at, so the attachment's name and where the PDF lists it do not matter.
///
/// The PDF is not loaded as a whole: loading unpacks every object stream without a limit, so a
/// small PDF could fill the memory. Instead the attached files' streams (`/Type /EmbeddedFile`,
/// which a PDF never puts inside an object stream) are found in the bytes and unpacked with a
/// limit: a little first to see whether it is a Europass CV, the rest only then.
pub fn xml_in_pdf(bytes: &[u8]) -> Option<String> {
    let mut at = 0;
    let mut looked_at = 0;
    while looked_at < MAX_ATTACHMENTS {
        let i = at + find(&bytes[at..], b"/EmbeddedFile")?;
        at = i + b"/EmbeddedFile".len();
        // «/EmbeddedFiles», the name tree of the catalogue, is another name.
        if bytes.get(at).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'#') {
            continue;
        }
        looked_at += 1;
        // The stream's dictionary runs from its «N 0 obj» to the «stream» keyword after the name.
        let start = rfind(&bytes[..i], b"obj").map_or(0, |s| s + 3);
        let Some(keyword) = find(&bytes[i..bytes.len().min(i + 4096)], b"stream").map(|s| i + s) else { continue };
        let dict = &bytes[start..keyword];
        let mut data = keyword + b"stream".len();
        if bytes.get(data) == Some(&b'\r') {
            data += 1;
        }
        if bytes.get(data) == Some(&b'\n') {
            data += 1;
        }
        let window = &bytes[data.min(bytes.len())..bytes.len().min(data + MAX_ATTACHMENT + 64)];
        let Some(end) = find(window, b"endstream") else { continue };
        let content = &window[..end];
        if let Some(text) = attachment_text(dict, content) {
            return Some(text);
        }
    }
    None
}

/// An attached file's text when it is a Europass CV: stored as it is, or packed with Flate (the
/// only packing the Europass editor uses). Unpacking stops at [`MAX_ATTACHMENT`]; an attachment
/// that is larger, uses another filter, or does not unpack is passed over.
fn attachment_text(dict: &[u8], content: &[u8]) -> Option<String> {
    use std::io::Read;
    let unpack = |limit: u64| {
        let mut out = Vec::new();
        // A cut stream still gives what came before the cut.
        let _ = flate2::read::ZlibDecoder::new(content).take(limit).read_to_end(&mut out);
        out
    };
    let has = |name: &[u8]| find(dict, name).is_some();
    let data = if !has(b"/Filter") {
        content.to_vec()
    } else if has(b"/FlateDecode") || has(b"/Fl ") || has(b"/Fl/") || has(b"/Fl]") {
        // One filter only: any other (ASCII85, LZW, a crypt filter …) is not unpacked here.
        if [&b"/ASCII"[..], b"/LZW", b"/RunLength", b"/Crypt", b"/A85", b"/AHx", b"/RL"].iter().any(|f| has(f)) {
            return None;
        }
        if !is_europass(&crate::import::decode_text(&unpack(HEAD))) {
            return None;
        }
        let out = unpack(MAX_ATTACHMENT as u64 + 1);
        if out.len() > MAX_ATTACHMENT {
            return None;
        }
        out
    } else {
        return None;
    };
    let text = crate::import::decode_text(&data);
    is_europass(&text).then_some(text)
}

/// HTML from a Europass description («<ul><li>…</li></ul>», often escaped) as lines.
fn html_lines(html: &str) -> Vec<String> {
    lines(&strip_html(html))
}

fn html_text(html: &str) -> String {
    strip_html(html)
}

/// «B1–C1», or one level when every skill has it.
fn cefr(levels: &[String]) -> String {
    let mut l: Vec<String> = levels.iter().map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty()).collect();
    l.sort();
    l.dedup();
    match l.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [low, .., high] => format!("{low}–{high}"),
    }
}

/// ISO 639 codes (the two- and three-letter forms Europass writes) as language names.
const LANGUAGES: &[(&[&str], &str, &str)] = &[
    (&["no", "nor", "nb", "nob", "nn", "nno"], "Norsk", "Norwegian"),
    (&["en", "eng"], "Engelsk", "English"),
    (&["de", "deu", "ger"], "Tysk", "German"),
    (&["fr", "fra", "fre"], "Fransk", "French"),
    (&["es", "spa"], "Spansk", "Spanish"),
    (&["it", "ita"], "Italiensk", "Italian"),
    (&["pt", "por"], "Portugisisk", "Portuguese"),
    (&["nl", "nld", "dut"], "Nederlandsk", "Dutch"),
    (&["sv", "swe"], "Svensk", "Swedish"),
    (&["da", "dan"], "Dansk", "Danish"),
    (&["fi", "fin"], "Finsk", "Finnish"),
    (&["is", "isl", "ice"], "Islandsk", "Icelandic"),
    (&["pl", "pol"], "Polsk", "Polish"),
    (&["cs", "ces", "cze"], "Tsjekkisk", "Czech"),
    (&["sk", "slk", "slo"], "Slovakisk", "Slovak"),
    (&["sl", "slv"], "Slovensk", "Slovenian"),
    (&["hr", "hrv"], "Kroatisk", "Croatian"),
    (&["sr", "srp"], "Serbisk", "Serbian"),
    (&["bs", "bos"], "Bosnisk", "Bosnian"),
    (&["hu", "hun"], "Ungarsk", "Hungarian"),
    (&["ro", "ron", "rum"], "Rumensk", "Romanian"),
    (&["bg", "bul"], "Bulgarsk", "Bulgarian"),
    (&["el", "ell", "gre"], "Gresk", "Greek"),
    (&["et", "est"], "Estisk", "Estonian"),
    (&["lv", "lav"], "Latvisk", "Latvian"),
    (&["lt", "lit"], "Litauisk", "Lithuanian"),
    (&["ga", "gle"], "Irsk", "Irish"),
    (&["mt", "mlt"], "Maltesisk", "Maltese"),
    (&["ru", "rus"], "Russisk", "Russian"),
    (&["uk", "ukr"], "Ukrainsk", "Ukrainian"),
    (&["tr", "tur"], "Tyrkisk", "Turkish"),
    (&["ar", "ara"], "Arabisk", "Arabic"),
    (&["fa", "fas", "per"], "Persisk", "Persian"),
    (&["ur", "urd"], "Urdu", "Urdu"),
    (&["hi", "hin"], "Hindi", "Hindi"),
    (&["zh", "zho", "chi"], "Kinesisk", "Chinese"),
    (&["ja", "jpn"], "Japansk", "Japanese"),
    (&["ko", "kor"], "Koreansk", "Korean"),
    (&["so", "som"], "Somali", "Somali"),
    (&["ti", "tir"], "Tigrinja", "Tigrinya"),
    (&["se", "sme"], "Nordsamisk", "Northern Sami"),
];

fn language_name(code: &str, english: bool) -> String {
    let c = code.trim().to_lowercase();
    LANGUAGES.iter().find(|(codes, _, _)| codes.contains(&c.as_str())).map(|(_, no, en)| if english { en } else { no }.to_string()).unwrap_or_else(|| code.trim().to_string())
}

/// What is read, gathered before it becomes a project: the language of the CV is decided from
/// the whole text first.
#[derive(Default)]
struct Read {
    project: Project,
    found: Vec<String>,
    rest: Vec<String>,
}

/// Reads a Europass CV in XML (either format).
pub fn read_xml(text: &str) -> Result<CvRead, String> {
    let root = parse(text.trim_start_matches('\u{feff}'))?;
    let mut r = Read::default();
    let english = if root.name.eq_ignore_ascii_case("SkillsPassport") {
        let english = root.attr("locale").to_lowercase().starts_with("en") || looks_english(&prose_of(&root));
        read_v3(&root, english, &mut r);
        english
    } else if root.name.eq_ignore_ascii_case("Candidate") {
        let lang = root.at(&["CandidateProfile"]).map(|p| p.attr("languageCode").to_lowercase()).unwrap_or_default();
        let english = if lang.is_empty() { looks_english(&prose_of(&root)) } else { !matches!(lang.as_str(), "no" | "nor" | "nb" | "nob" | "nn" | "nno") };
        read_candidate(&root, english, &mut r);
        english
    } else {
        return Err("Filen er ikke en Europass-CV.".into());
    };
    let mut p = r.project;
    if english {
        p.language = Language::En;
    }
    let c = &p.candidate;
    let mut found = Vec::new();
    for (value, word) in [(&c.name, "navn"), (&c.headline, "tittel"), (&c.summary, "profil"), (&c.city, "sted"), (&c.email, "e-post"), (&c.phone, "telefon"), (&c.linkedin, "LinkedIn")] {
        if !value.is_empty() {
            found.push(word.to_string());
        }
    }
    count(&mut found, "jobber", p.experience.len());
    count(&mut found, "utdanning", p.education.len());
    count(&mut found, "kompetanse", p.skills.len());
    count(&mut found, "kurs", p.certifications.len());
    count(&mut found, "språk", p.languages.len());
    count(&mut found, "andre seksjoner", p.custom_sections.len().min(1));
    found.extend(r.found);
    if !p.has_content() {
        return Err("Europass-filen har ingen profil, jobber eller utdanning å hente inn.".into());
    }
    Ok(CvRead { project: p, rest: r.rest.join("\n\n"), found })
}

/// The words of the descriptions, to tell the CV's language.
fn prose_of(root: &Node) -> String {
    let mut out = String::new();
    for name in ["Description", "Activities", "ExecutiveSummary", "Label", "PositionTitle"] {
        for n in root.descendants(name) {
            out.push_str(&strip_html(&n.text));
            out.push('\n');
        }
    }
    out
}

fn pick(english: bool, no: &str, en: &str) -> String {
    if english { en.to_string() } else { no.to_string() }
}

/// A contact value that is a LinkedIn profile goes in its own field, others become contact lines.
fn web(p: &mut Project, label: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() || p.candidate.extra.len() >= MAX_LIST {
        return;
    }
    let c = &mut p.candidate;
    if c.linkedin.is_empty() && value.to_lowercase().contains("linkedin.com") {
        c.linkedin = value.to_string();
    } else if !c.extra.iter().any(|e| e.value == value) {
        let id = if label.is_empty() && !c.extra.iter().any(|e| e.id == "web") { "web".to_string() } else { format!("contact_imported_{}", c.extra.len() + 1) };
        c.extra.push(ContactField { id, label: label.to_string(), value: value.to_string() });
    }
}

fn licence(p: &mut Project, english: bool, classes: &[String]) {
    if !classes.is_empty() {
        let id = format!("contact_imported_{}", p.candidate.extra.len() + 1);
        p.candidate.extra.push(ContactField { id, label: pick(english, "Førerkort", "Driving licence"), value: classes.join(", ") });
    }
}

// ---------- the older format (SkillsPassport) ----------

/// «<From year="2019" month="--03"/>» → «2019-03».
fn v3_date(n: Option<&Node>) -> String {
    let Some(n) = n else { return String::new() };
    let year = n.attr("year");
    let month = n.attr("month").trim_start_matches('-');
    match (year.is_empty(), month.parse::<u8>()) {
        (true, _) => String::new(),
        (false, Ok(m)) if (1..=12).contains(&m) => format!("{year}-{m:02}"),
        _ => year.to_string(),
    }
}

fn read_v3(root: &Node, english: bool, r: &mut Read) {
    let Some(info) = root.child("LearnerInfo") else { return };
    let p = &mut r.project;
    let id = info.child("Identification");
    if let Some(id) = id {
        p.candidate.name = [id.get(&["PersonName", "FirstName"]), id.get(&["PersonName", "Surname"])].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        let contact = id.child("ContactInfo");
        if let Some(ci) = contact {
            p.candidate.city = ci.get(&["Address", "Contact", "Municipality"]);
            p.candidate.email = ci.get(&["Email", "Contact"]);
            p.candidate.phone = ci.descendants("Telephone").iter().map(|t| t.get(&["Contact"])).find(|t| !t.is_empty()).unwrap_or_default();
            for w in ci.descendants("Website") {
                web(p, "", &w.get(&["Contact"]));
            }
            for m in ci.descendants("InstantMessaging") {
                let label = m.get(&["Use", "Label"]);
                let label = if label.is_empty() { m.get(&["Use", "Code"]) } else { label };
                web(p, &label, &m.get(&["Contact"]));
            }
            let address = ["AddressLine", "PostalCode"].iter().map(|k| ci.get(&["Address", "Contact", k])).chain([ci.get(&["Address", "Contact", "Country", "Label"])]).filter(|s| !s.is_empty()).collect::<Vec<_>>();
            if !address.is_empty() {
                r.rest.push(format!("{}: {}", pick(english, "Adresse", "Address"), address.join(", ")));
            }
        }
        if let Some(d) = id.child("Demographics") {
            let birth = d.child("Birthdate").map(|b| [b.attr("year"), b.attr("month").trim_start_matches('-'), b.attr("day").trim_start_matches('-')].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-")).unwrap_or_default();
            if !birth.is_empty() {
                r.rest.push(format!("{}: {birth}", pick(english, "Fødselsdato", "Date of birth")));
            }
            let nationality: Vec<String> = d.descendants("Nationality").iter().map(|n| if n.get(&["Label"]).is_empty() { n.get(&["Code"]) } else { n.get(&["Label"]) }).filter(|s| !s.is_empty()).collect();
            if !nationality.is_empty() {
                r.rest.push(format!("{}: {}", pick(english, "Statsborgerskap", "Nationality"), nationality.join(", ")));
            }
        }
    }
    p.candidate.headline = info.get(&["Headline", "Description", "Label"]);

    for w in info.at(&["WorkExperienceList"]).map(|l| l.all("WorkExperience").collect::<Vec<_>>()).unwrap_or_default() {
        let (company, title) = (w.get(&["Employer", "Name"]), w.get(&["Position", "Label"]));
        if company.is_empty() && title.is_empty() {
            continue;
        }
        let period = w.child("Period");
        let current = period.is_some_and(|p| p.get(&["Current"]).eq_ignore_ascii_case("true"));
        let end = v3_date(period.and_then(|p| p.child("To")));
        let end = if current || end.is_empty() { None } else { Some(end) };
        let location = w.get(&["Employer", "ContactInfo", "Address", "Contact", "Municipality"]);
        let n = p.experience.len() + 1;
        p.experience.push(Job { bullet_ids: Vec::new(), id: format!("job_imported_{n}"), company, title, location, start: v3_date(period.and_then(|p| p.child("From"))), end, bullets: html_lines(&w.get(&["Activities"])), evidence: String::new(), confirmed: true });
    }

    for e in info.at(&["EducationList"]).map(|l| l.all("Education").collect::<Vec<_>>()).unwrap_or_default() {
        let (qualification, institution) = (e.get(&["Title"]), e.get(&["Organisation", "Name"]));
        if qualification.is_empty() && institution.is_empty() {
            continue;
        }
        let period = e.child("Period");
        let (start, end) = (v3_date(period.and_then(|p| p.child("From"))), v3_date(period.and_then(|p| p.child("To"))));
        let more = [e.get(&["Level", "Label"]), html_text(&e.get(&["Activities"]))].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>();
        if !more.is_empty() {
            r.rest.push(format!("{qualification} ({institution}):\n{}", more.join("\n")));
        }
        p.education.push(Education { id: String::new(), qualification, institution, start, end });
    }

    if let Some(skills) = info.child("Skills") {
        if let Some(lang) = skills.child("Linguistic") {
            for m in lang.descendants("MotherTongue") {
                let code = m.get(&["Description", "Code"]);
                let name = m.get(&["Description", "Label"]);
                let name = if name.is_empty() { language_name(&code, english) } else { name };
                if !name.is_empty() {
                    p.languages.push(SpokenLanguage { id: String::new(), name, level: pick(english, "morsmål", "native") });
                }
            }
            for f in lang.descendants("ForeignLanguage") {
                let code = f.get(&["Description", "Code"]);
                let name = f.get(&["Description", "Label"]);
                let name = if name.is_empty() { language_name(&code, english) } else { name };
                if name.is_empty() {
                    continue;
                }
                let levels: Vec<String> = f.child("ProficiencyLevel").map(|l| l.children.iter().map(|c| c.text.clone()).collect()).unwrap_or_default();
                p.languages.push(SpokenLanguage { id: String::new(), name: name.clone(), level: cefr(&levels) });
                for cert in f.descendants("Certificate") {
                    let title = cert.get(&["Title"]);
                    if !title.is_empty() {
                        p.certifications.push(Certification { id: String::new(), name: title, issuer: cert.get(&["AwardingBody"]), year: cert.child("Date").map(|d| d.attr("year").to_string()).unwrap_or_default() });
                    }
                }
            }
        }
        // Digital skills written as a list become skills; prose goes to the person.
        let digital = pick(english, "Digitale ferdigheter", "Digital skills");
        if let Some(computer) = skills.child("Computer") {
            let html = computer.get(&["Description"]);
            if html.to_lowercase().contains("<li") && html_lines(&html).iter().all(|l| l.chars().count() <= 60) {
                let mut seen: HashSet<String> = p.skills.iter().map(|s| s.name.to_ascii_lowercase()).collect();
                for name in html_lines(&html).into_iter().take(MAX_LIST) {
                    if seen.insert(name.to_ascii_lowercase()) {
                        p.skills.push(Skill { id: String::new(), group: digital.clone(), name, evidence: String::new(), confirmed: true });
                    }
                }
            } else if !html.is_empty() {
                r.rest.push(format!("{digital}:\n{}", html_text(&html)));
            }
        }
        for (key, no, en) in [("Communication", "Kommunikasjon", "Communication skills"), ("Organisational", "Organisering og ledelse", "Organisational skills"), ("JobRelated", "Jobbrelaterte ferdigheter", "Job-related skills"), ("Other", "Andre ferdigheter", "Other skills")] {
            let text = html_text(&skills.get(&[key, "Description"]));
            if !text.is_empty() {
                r.rest.push(format!("{}:\n{text}", pick(english, no, en)));
            }
        }
        let classes: Vec<String> = skills.descendants("Licence").iter().map(|l| l.text.trim().to_string()).filter(|s| !s.is_empty()).collect();
        licence(p, english, &classes);
    }

    for a in info.descendants("Achievement") {
        let title = if a.get(&["Title", "Label"]).is_empty() { a.get(&["Title", "Code"]) } else { a.get(&["Title", "Label"]) };
        let text = html_text(&a.get(&["Description"]));
        if !text.is_empty() {
            r.rest.push(format!("{title}:\n{text}"));
        }
    }
}

// ---------- the platform's format since 2020 (Candidate) ----------

/// «2019-09-01», «2019-09» or «2019» → `YYYY-MM` or a year.
fn date(text: &str) -> String {
    let t = text.trim();
    let t = if t.len() >= 10 && t.as_bytes().get(7) == Some(&b'-') { &t[..7] } else { t };
    month_or_year(t).unwrap_or_default()
}

/// The date under a start or end element: `hr:FormattedDateTime`, or the element's own text.
fn period_date(n: Option<&Node>) -> String {
    let Some(n) = n else { return String::new() };
    let t = n.get(&["FormattedDateTime"]);
    date(if t.is_empty() { &n.text } else { &t })
}

fn city(n: &Node) -> String {
    n.descendants("CityName").first().map(|c| c.text.trim().to_string()).unwrap_or_default()
}

fn read_candidate(root: &Node, english: bool, r: &mut Read) {
    let p = &mut r.project;
    if let Some(person) = root.child("CandidatePerson") {
        p.candidate.name = [person.get(&["PersonName", "GivenName"]), person.get(&["PersonName", "FamilyName"])].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        for c in person.all("Communication") {
            let channel = c.get(&["ChannelCode"]).to_lowercase();
            let uri = c.get(&["URI"]);
            match channel.as_str() {
                "email" if p.candidate.email.is_empty() => p.candidate.email = uri,
                "telephone" | "mobiletelephone" | "phone" if p.candidate.phone.is_empty() => {
                    let dialing = c.get(&["CountryDialing"]);
                    let number = c.get(&["DialNumber"]);
                    let number = if number.is_empty() { uri } else { number };
                    p.candidate.phone = if dialing.is_empty() || number.starts_with('+') { number } else { format!("+{} {number}", dialing.trim_start_matches('+')) };
                }
                "web" | "social media" | "socialmedia" | "instantmessage" => {
                    let label = if channel == "web" { String::new() } else { c.get(&["UseCode"]) };
                    web(p, &label, &uri);
                }
                _ => {}
            }
            if let Some(address) = c.child("Address") {
                if p.candidate.city.is_empty() {
                    p.candidate.city = address.get(&["CityName"]);
                }
                let more: Vec<String> = ["AddressLine", "PostalCode", "CountryCode"].iter().map(|k| address.get(&[k])).filter(|s| !s.is_empty()).collect();
                if !more.is_empty() {
                    r.rest.push(format!("{}: {}", pick(english, "Adresse", "Address"), more.join(", ")));
                }
            }
        }
        let birth = person.get(&["BirthDate"]);
        if !birth.is_empty() {
            r.rest.push(format!("{}: {birth}", pick(english, "Fødselsdato", "Date of birth")));
        }
        let nationality: Vec<String> = person.all("NationalityCode").map(|n| n.text.trim().to_uppercase()).filter(|s| !s.is_empty()).collect();
        if !nationality.is_empty() {
            r.rest.push(format!("{}: {}", pick(english, "Statsborgerskap", "Nationality"), nationality.join(", ")));
        }
        for m in person.all("PrimaryLanguageCode") {
            let name = language_name(&m.text, english);
            if !name.is_empty() {
                p.languages.push(SpokenLanguage { id: String::new(), name, level: pick(english, "morsmål", "native") });
            }
        }
    }
    let Some(profile) = root.child("CandidateProfile") else { return };
    p.candidate.summary = html_text(&profile.get(&["ExecutiveSummary"]));

    for employer in profile.descendants("EmployerHistory") {
        let company = employer.get(&["OrganizationName"]);
        let employer_city = employer.child("OrganizationContact").map(city).unwrap_or_default();
        for position in employer.all("PositionHistory") {
            if p.experience.len() >= MAX_LIST {
                break;
            }
            let title = position.get(&["PositionTitle"]);
            let period = position.child("EmploymentPeriod");
            let current = period.is_some_and(|p| p.get(&["CurrentIndicator"]).eq_ignore_ascii_case("true"));
            let end = period_date(period.and_then(|p| p.child("EndDate")));
            let end = if current || end.is_empty() { None } else { Some(end) };
            let location = if position.get(&["City"]).is_empty() { employer_city.clone() } else { position.get(&["City"]) };
            let n = p.experience.len() + 1;
            p.experience.push(Job { bullet_ids: Vec::new(), id: format!("job_imported_{n}"), company: company.clone(), title, location, start: period_date(period.and_then(|p| p.child("StartDate"))), end, bullets: html_lines(&position.get(&["Description"])), evidence: String::new(), confirmed: true });
        }
    }

    for e in profile.descendants("EducationOrganizationAttendance") {
        let institution = e.get(&["OrganizationName"]);
        let qualification = e.get(&["EducationDegree", "DegreeName"]);
        if qualification.is_empty() && institution.is_empty() {
            continue;
        }
        let period = e.child("AttendancePeriod");
        let more = [e.get(&["EducationDegree", "FinalGrade", "ScoreText"]), html_text(&e.get(&["Description"])), html_text(&e.get(&["EducationDegree", "OccupationalSkillsCovered"]))].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>();
        if !more.is_empty() {
            r.rest.push(format!("{qualification} ({institution}):\n{}", more.join("\n")));
        }
        p.education.push(Education { id: String::new(), qualification, institution, start: period_date(period.and_then(|p| p.child("StartDate"))), end: period_date(period.and_then(|p| p.child("EndDate"))) });
    }

    // Languages: a competency in the «language» taxonomy, with CEFR scores per skill.
    for c in profile.descendants("PersonCompetency") {
        let taxonomy = c.get(&["TaxonomyID"]).to_lowercase();
        if taxonomy == "language" {
            let name = language_name(&c.get(&["CompetencyID"]), english);
            if name.is_empty() || p.languages.iter().any(|l| l.name.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let levels: Vec<String> = c.descendants("ScoreText").iter().map(|s| s.text.clone()).collect();
            p.languages.push(SpokenLanguage { id: String::new(), name, level: cefr(&levels) });
        }
    }
    // Digital skills, alone or in the person's own groups.
    let digital = pick(english, "Digitale ferdigheter", "Digital skills");
    if let Some(skills) = profile.child("Skills") {
        let mut seen: HashSet<String> = p.skills.iter().map(|s| s.name.to_ascii_lowercase()).collect();
        let mut add = |group: &str, c: &Node| {
            let name = c.get(&["CompetencyName"]);
            if !name.is_empty() && p.skills.len() < MAX_LIST && seen.insert(name.to_ascii_lowercase()) {
                p.skills.push(Skill { id: String::new(), group: group.to_string(), name, evidence: String::new(), confirmed: true });
            }
        };
        for c in skills.all("PersonCompetency") {
            add(&digital, c);
        }
        for g in skills.all("SkillsGroup") {
            let title = g.get(&["Title"]);
            let title = if title.is_empty() { digital.clone() } else { title };
            for c in g.all("PersonCompetency") {
                add(&title, c);
            }
        }
    }
    let classes: Vec<String> = profile.descendants("LicenseTypeCode").iter().map(|l| l.text.trim().to_string()).filter(|s| !s.is_empty()).collect();
    licence(p, english, &classes);

    for c in profile.descendants("Certification") {
        let name = [c.get(&["CertificationName"]), c.get(&["Title"]), c.get(&["Name"])].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
        if !name.is_empty() {
            let issuer = [c.get(&["IssuingAuthority", "Name"]), c.get(&["IssuingAuthority"]), c.get(&["OrganizationName"])].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
            let year = crate::checks::parse_year(&[c.get(&["FirstIssuedDate", "FormattedDateTime"]), c.get(&["FirstIssuedDate"]), c.get(&["IssueDate"]), c.get(&["Date"])].join(" ")).unwrap_or_default();
            p.certifications.push(Certification { id: String::new(), name, issuer, year });
        }
    }

    // The person's own sections: a title with items.
    for others in profile.all("Others") {
        let title = others.get(&["Title"]);
        let items: Vec<CustomItem> = others.all("Other").filter(|o| !o.get(&["Title"]).is_empty() || !o.get(&["Description"]).is_empty()).map(|o| CustomItem { id: String::new(), bullet_ids: Vec::new(), title: o.get(&["Title"]), subtitle: String::new(), date: o.get(&["Date"]), bullets: html_lines(&o.get(&["Description"])) }).collect();
        if title.is_empty() || items.is_empty() {
            continue;
        }
        let id = format!("section_imported_{}", p.custom_sections.len() + 1);
        p.custom_sections.push(CustomSection { id, title, items });
    }
    for publication in profile.descendants("Publication") {
        let text: Vec<String> = publication.children.iter().map(|c| html_text(&c.text)).filter(|s| !s.is_empty()).collect();
        if !text.is_empty() {
            r.rest.push(format!("{}:\n{}", pick(english, "Publikasjon", "Publication"), text.join("\n")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_one_or_a_range() {
        assert_eq!(cefr(&["B2".into(), "b2".into()]), "B2");
        assert_eq!(cefr(&["C1".into(), "B1".into(), "B2".into()]), "B1–C1");
        assert_eq!(cefr(&[]), "");
    }

    #[test]
    fn prefixes_and_entities() {
        let n = parse("<?xml version=\"1.0\"?><a:Root xmlns:a=\"x\"><a:Name>Kari &amp; Ola &#248;</a:Name><b x:k=\"v\"/></a:Root>").expect("xml");
        assert_eq!(n.name, "Root");
        assert_eq!(n.get(&["name"]), "Kari & Ola ø");
        assert_eq!(n.child("b").map(|b| b.attr("k")), Some("v"));
    }

    #[test]
    fn old_dates() {
        let n = parse("<P><From year=\"2019\" month=\"--03\"/><To year=\"2020\"/></P>").expect("xml");
        assert_eq!(v3_date(n.child("From")), "2019-03");
        assert_eq!(v3_date(n.child("To")), "2020");
    }
}
