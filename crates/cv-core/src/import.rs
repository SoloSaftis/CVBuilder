//! Bringing text in: an existing CV or a job ad from a file (PDF, Word, text, web page), and
//! simple readers that need no AI. The readers only take what they can recognise with
//! confidence (contact details, headed sections, «Stilling:» lines); the rest is left for the
//! person or the AI, and nothing is invented.
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::checks;
use crate::model::{Project, Skill};

/// The file types that can be read, for dialogs and messages.
pub const FILE_TYPES: [&str; 16] = ["pdf", "docx", "doc", "odt", "rtf", "pages", "txt", "md", "html", "htm", "mht", "mhtml", "fodt", "dotx", "docm", "json"];

/// Files larger than this are not ads or CVs; reading them would only make the app wait.
const MAX_FILE: u64 = 50_000_000;
/// How long a PDF or Word reader may work on one file before CVBuilder gives up on it.
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What a file really is, from its first bytes; the extension can be wrong.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Pdf,
    Zip,
    OldWord,
    Rtf,
    Html,
    Text,
    Binary,
}

fn sniff(bytes: &[u8]) -> Kind {
    let head = &bytes[..bytes.len().min(1024)];
    if head.windows(5).any(|w| w == b"%PDF-") {
        return Kind::Pdf;
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Kind::Zip;
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Kind::OldWord;
    }
    if bytes.starts_with(b"{\\rtf") {
        return Kind::Rtf;
    }
    let text = decode_text(head);
    if text.contains('\u{0}') || text.chars().filter(|c| c.is_control() && !c.is_whitespace()).count() > head.len() / 20 + 2 {
        return Kind::Binary;
    }
    let lower = text.trim_start().to_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") || lower.contains("<body") || (lower.starts_with('<') && lower.contains("<head")) {
        return Kind::Html;
    }
    Kind::Text
}

/// Text in whatever encoding a text file or web page came in: UTF-8 (with or without BOM),
/// UTF-16 (Notepad's «Unicode»), else Windows-1252, which older Norwegian files use.
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |b: &[u8], le: bool| {
        let units: Vec<u16> = b.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    // UTF-16 without a BOM: every other byte is zero in Latin text.
    let sample = &bytes[..bytes.len().min(400)];
    if sample.len() >= 8 && sample.iter().skip(1).step_by(2).filter(|b| **b == 0).count() * 10 >= sample.len() / 2 * 9 && sample.iter().step_by(2).all(|b| *b != 0) {
        return utf16(bytes, true);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // A cut at the end of a sample is still UTF-8.
        Err(e) if e.error_len().is_none() => String::from_utf8_lossy(bytes).into_owned(),
        Err(_) => bytes.iter().map(|&b| windows_1252(b)).collect(),
    }
}

fn windows_1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    if (0x80..0xA0).contains(&b) { HIGH[usize::from(b - 0x80)] } else { char::from(b) }
}

/// Runs a file reader on its own thread: a reader that panics or never finishes on an odd file
/// gives an error instead of taking the app down or leaving it waiting.
fn isolated<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(job());
    });
    rx.recv_timeout(READ_TIMEOUT).ok()
}

/// The text of a file. PDFs must contain real text (a scanned page is an image and gives none).
pub fn text_from_file(path: &std::path::Path) -> Result<String, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let bytes = read_file(path)?;
    text_from_bytes(&bytes, &ext)
}

fn read_file(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("Filen kunne ikke leses: {e}"))?.len();
    if size > MAX_FILE {
        return Err("Filen er for stor (over 50 MB) til å være en annonse eller en CV.".into());
    }
    std::fs::read(path).map_err(|e| format!("Filen kunne ikke leses: {e}"))
}

/// The text of a file's bytes; `ext` is the file's extension in lower case, used when the
/// bytes alone do not say what the file is.
pub fn text_from_bytes(bytes: &[u8], ext: &str) -> Result<String, String> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err("Filen er tom; det er ingen tekst å hente.".into());
    }
    let kind = sniff(bytes);
    let text = match (kind, ext) {
        (Kind::Pdf, _) => pdf_text(bytes)?,
        (Kind::Zip, "docx" | "docm" | "dotx" | "") => docx_text(bytes)?,
        (Kind::Zip, "odt" | "ott") => formats_text("odt", bytes)?,
        (Kind::Zip, "pages") => pages_text(bytes)?,
        // A package whose name says nothing: Word first, then OpenDocument, then a Pages preview.
        (Kind::Zip, _) => docx_text(bytes).or_else(|_| formats_text("odt", bytes)).or_else(|_| pages_text(bytes)).map_err(|_| "Filen er en ZIP-pakke, men ikke en Word-, OpenDocument- eller Pages-fil, og kan ikke leses.".to_string())?,
        (Kind::OldWord, _) => doc_text(bytes)?,
        (Kind::Rtf, _) => formats_text("rtf", bytes)?,
        (_, "mht" | "mhtml") => formats_text("mht", bytes)?,
        (Kind::Text, "fodt") => formats_text("fodt", bytes)?,
        (Kind::Binary, _) | (_, "doc") => return Err(format!("«.{ext}» kan ikke leses. Bruk PDF, Word (.docx eller .doc), OpenDocument (.odt), RTF, tekst (.txt/.md) eller en nettside (.html).")),
        // A web page saved under another name is still a web page.
        (Kind::Html, _) | (_, "html" | "htm" | "xhtml") => strip_html(&decode_text(bytes)),
        (_, "pdf") => return Err("Filen heter .pdf, men er ikke en PDF. Den kan være skadet eller bare delvis lastet ned.".into()),
        (_, "docx") => return Err("Filen heter .docx, men er ikke en Word-fil. Den kan være skadet eller bare delvis lastet ned.".into()),
        (Kind::Text, "txt" | "md" | "text" | "markdown" | "") => decode_text(bytes),
        (Kind::Text, other) => return Err(format!("«.{other}» kan ikke leses. Bruk PDF, Word (.docx eller .doc), OpenDocument (.odt), RTF, tekst (.txt/.md) eller en nettside (.html).")),
    };
    let text = tidy(&text);
    if text.trim().chars().filter(|c| c.is_alphanumeric()).count() < 20 {
        return Err("Fant nesten ingen tekst i filen. Er det en skannet PDF (et bilde)? Da må teksten skrives inn eller kopieres.".into());
    }
    Ok(text)
}

fn pdf_text(bytes: &[u8]) -> Result<String, String> {
    let owned = bytes.to_vec();
    let encrypted = bytes.windows(8).any(|w| w == b"/Encrypt");
    match isolated(move || pdf_extract::extract_text_from_mem(&owned)) {
        Some(Ok(t)) if t.chars().any(char::is_alphanumeric) || !encrypted => Ok(t),
        _ if encrypted => Err("PDF-en er passordbeskyttet eller låst. Åpne den, kopier teksten og lim den inn i stedet.".into()),
        Some(_) => Err("PDF-en kunne ikke leses. Den kan være skadet. Åpne den, kopier teksten og lim den inn i stedet.".into()),
        None => Err("PDF-en har et format CVBuilder ikke kan lese tekst fra. Prøv å lagre den som Word eller tekst.".into()),
    }
}

fn docx_text(bytes: &[u8]) -> Result<String, String> {
    let owned = bytes.to_vec();
    match isolated(move || wordcraft_docx::read_with(&owned, &Default::default()).map(|doc| doc.plain_text(wordcraft_doc::StoryRef::Body)).map_err(|e| e.to_string())) {
        Some(Ok(t)) => Ok(t),
        Some(Err(e)) => Err(format!("Word-filen kunne ikke leses: {e}")),
        None => Err("Word-filen kunne ikke leses. Den kan være skadet. Åpne den i Word og lagre den på nytt.".into()),
    }
}

/// An old Word file (Word 97–2003, .doc), read by WordCraft.
fn doc_text(bytes: &[u8]) -> Result<String, String> {
    if wordcraft_docbin::is_encrypted(bytes) {
        return Err("Word-filen er passordbeskyttet. Åpne den i Word, fjern passordet og lagre den på nytt.".into());
    }
    let owned = bytes.to_vec();
    match isolated(move || wordcraft_docbin::read(&owned).map(|doc| doc.plain_text(wordcraft_doc::StoryRef::Body)).map_err(|e| e.to_string())) {
        Some(Ok(t)) => Ok(t),
        Some(Err(e)) => Err(format!("Den eldre Word-filen (.doc) kunne ikke leses: {e}. Åpne den i Word og lagre den som .docx.")),
        None => Err("Den eldre Word-filen (.doc) kunne ikke leses. Åpne den i Word og lagre den som .docx.".into()),
    }
}

/// RTF, OpenDocument (.odt, .fodt) and saved web archives (.mht), read by WordCraft.
fn formats_text(ext: &'static str, bytes: &[u8]) -> Result<String, String> {
    let owned = bytes.to_vec();
    let name = match ext {
        "rtf" => "RTF-filen",
        "mht" => "Nettarkivet (.mht)",
        _ => "OpenDocument-filen",
    };
    match isolated(move || wordcraft_formats::import(ext, &owned).unwrap_or_else(|| Err("ukjent format".into())).map(|doc| doc.plain_text(wordcraft_doc::StoryRef::Body))) {
        Some(Ok(t)) => Ok(t),
        Some(Err(e)) => Err(format!("{name} kunne ikke leses: {e}. Lagre den som Word (.docx) eller PDF og prøv igjen.")),
        None => Err(format!("{name} kunne ikke leses. Lagre den som Word (.docx) eller PDF og prøv igjen.")),
    }
}

/// An Apple Pages file: a package with a PDF preview of the document inside.
fn pages_text(bytes: &[u8]) -> Result<String, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "Pages-filen kunne ikke åpnes.".to_string())?;
    let name = zip.file_names().find(|n| n.to_lowercase().ends_with("preview.pdf")).map(str::to_string);
    let Some(name) = name else {
        return Err("Pages-filen har ingen forhåndsvisning å lese. I Pages: Fil → Eksporter til → Word eller PDF.".into());
    };
    let mut pdf = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_name(&name).map_err(|e| e.to_string())?, &mut pdf).map_err(|e| e.to_string())?;
    pdf_text(&pdf)
}

/// A job ad from a file: a saved web page is read with its job data, other files as text.
pub fn ad_from_file(path: &std::path::Path) -> Result<AdInfo, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let bytes = read_file(path)?;
    if sniff(&bytes) == Kind::Html || (matches!(ext.as_str(), "html" | "htm" | "xhtml") && sniff(&bytes) == Kind::Text) {
        return ad_from_page(&decode_text(&bytes));
    }
    Ok(read_ad(&text_from_bytes(&bytes, &ext)?))
}

/// Trims each line and keeps at most one empty line between paragraphs.
pub fn tidy(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in text.lines().map(|l| l.trim_end_matches('\r').trim()) {
        if line.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}

/// A web page as readable text: no scripts, styles or tags, block elements on their own lines.
pub fn strip_html(html: &str) -> String {
    static DROP: LazyLock<Regex> = LazyLock::new(|| {
        let tags = ["script", "style", "noscript", "svg", "head", "nav", "footer", "template", "iframe", "button", "select"];
        Regex::new(&format!(r"(?is)<!--.*?-->|{}", tags.iter().map(|t| format!(r"<{t}\b.*?</{t}\s*>")).collect::<Vec<_>>().join("|"))).expect("regex")
    });
    // Block elements end a line; a label (<dt>) and its value (<dd>) get a line each.
    // A run of them («</p><p>») is one line break.
    static BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:\s*</?(?:br|p|li|ul|ol|dl|dt|dd|h[1-6]|div|tr|table|section|article|header|main|aside|blockquote|pre|hr|form|fieldset|figure|figcaption)\b[^>]*>\s*)+").expect("regex"));
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("regex"));
    let s = DROP.replace_all(html, " ");
    let s = BREAK.replace_all(&s, "\n");
    let s = TAG.replace_all(&s, " ");
    let s = entities(&s);
    tidy(&s.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n"))
}

/// HTML entities as characters, in one pass, so «&amp;oslash;» stays «&oslash;» as written.
pub fn entities(s: &str) -> String {
    static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(#[xX][0-9a-fA-F]{1,6}|#[0-9]{1,7}|[a-zA-Z][a-zA-Z0-9]{1,7});").expect("regex"));
    ENTITY
        .replace_all(s, |c: &regex::Captures| {
            let name = &c[1];
            let number = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                name.strip_prefix('#').and_then(|d| d.parse().ok())
            };
            if let Some(n) = number {
                return char::from_u32(n).filter(|ch| *ch != '\u{0}').map(String::from).unwrap_or_default();
            }
            let ch = match name {
                "nbsp" => " ",
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                "apos" => "'",
                "aelig" => "æ",
                "oslash" => "ø",
                "aring" => "å",
                "AElig" => "Æ",
                "Oslash" => "Ø",
                "Aring" => "Å",
                "auml" => "ä",
                "ouml" => "ö",
                "uuml" => "ü",
                "Auml" => "Ä",
                "Ouml" => "Ö",
                "Uuml" => "Ü",
                "szlig" => "ß",
                "eacute" => "é",
                "Eacute" => "É",
                "egrave" => "è",
                "aacute" => "á",
                "agrave" => "à",
                "oacute" => "ó",
                "ccedil" => "ç",
                "ntilde" => "ñ",
                "ndash" => "–",
                "mdash" => "—",
                "hellip" => "…",
                "lsquo" => "‘",
                "rsquo" => "’",
                "ldquo" => "“",
                "rdquo" => "”",
                "laquo" => "«",
                "raquo" => "»",
                "bull" => "•",
                "middot" => "·",
                "copy" => "©",
                "reg" => "®",
                "trade" => "™",
                "euro" => "€",
                "times" => "×",
                "shy" | "zwnj" | "zwj" | "lrm" | "rlm" => "",
                _ => return c[0].to_string(),
            };
            ch.to_string()
        })
        .into_owned()
}

/// What a job ad says, as far as it can be read without AI.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdInfo {
    pub role: String,
    pub company: String,
    pub description: String,
    pub keywords: Vec<String>,
}

/// The spaces, invisible marks and line endings people paste (tabs, non-breaking spaces,
/// Windows line endings, soft hyphens …) as plain spaces and line breaks, each line trimmed.
pub fn clean_paste(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' | '\u{2028}' | '\u{2029}' | '\u{85}' | '\u{b}' | '\u{c}' => out.push('\n'),
            '\t' | '\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => out.push(' '),
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' | '\u{ad}' => {}
            c => out.push(c),
        }
    }
    tidy(&out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n"))
}

/// A line without its list marker («• », «- », «* », «1. », «a) », an emoji …).
fn unbullet(line: &str) -> &str {
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:\d{1,2}[.)]|[a-h]\))\s+").expect("regex"));
    let l = line.trim_start_matches(|c: char| !(c.is_alphanumeric() || ".#+(«\"'“".contains(c))).trim_start();
    match NUMBER.find(l) {
        Some(m) => &l[m.end()..],
        None => l,
    }
}

const ROLE_LABELS: &str = r"stillingstittel|stillingsnavn|stillingsbetegnelse|jobbtittel|stilling|tittel|job\s*title|position|role|title";
const COMPANY_LABELS: &str = r"arbeidsgiver|arbeidsgivar|firmanavn|firma|bedrift|selskap|virksomhet|verksemd|organisasjon|employer|company\s*name|company|hiring\s+organi[sz]ation";
/// Other «label: value» lines in ads; their labels are not key words.
const OTHER_LABELS: &str = r"søknadsfrist|frist|arbeidssted|sted|lokasjon|location|sektor|ansettelsesform|type ansettelse|stillingsprosent|omfang|antall stillinger|arbeidsspråk|kontaktperson|kontakt|telefon|mobil|e-post|epost|referanse|ref\.?|finn-kode|finnkode|oppstart|tiltredelse|lønn|deadline|employment type|job type|start date|salary|seniority level|industry|bransje";

fn labelled(labels: &str) -> Regex {
    Regex::new(&format!(r"(?i)^(?:{labels})\s*[:：\-–—|]\s*(.{{1,150}})$")).expect("regex")
}

fn alone(labels: &str) -> Regex {
    Regex::new(&format!(r"(?i)^(?:{labels})\s*:?$")).expect("regex")
}

// «Vi søker: Elektriker» names the role, but «Vi søker» alone is a heading over a list.
static ROLE_LABELLED: LazyLock<Regex> = LazyLock::new(|| labelled(&format!("{ROLE_LABELS}|vi søker|vi ser etter|we are hiring|we're hiring|now hiring")));
// NAV's «Stillingstittel<tab>Backend utvikler», copied from a table: a label without a colon.
static ROLE_SPACED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?i:stillingstittel|stillingsnavn|jobbtittel|job title)\s+(\p{Lu}.{1,100})$").expect("regex"));
static COMPANY_SPACED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?i:arbeidsgiver|arbeidsgivar|employer|firmanavn)\s+(\p{Lu}.{1,100})$").expect("regex"));
static ROLE_ALONE: LazyLock<Regex> = LazyLock::new(|| alone(ROLE_LABELS));
static COMPANY_LABELLED: LazyLock<Regex> = LazyLock::new(|| labelled(COMPANY_LABELS));
static COMPANY_ALONE: LazyLock<Regex> = LazyLock::new(|| alone(COMPANY_LABELS));
static ANY_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)^(?:{ROLE_LABELS}|{COMPANY_LABELS}|{OTHER_LABELS})\s*(?:[:：\-–—|]\s*|:?$)")).expect("regex"));

/// Values that follow a «Stilling:» label in some ads but are not a job title.
fn employment_type(value: &str) -> bool {
    static TYPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:\d|(?:fast|vikariat|vikar|heltid|deltid|engasjement|prosjektstilling|full[- ]?time|part[- ]?time|permanent|temporary|contract|kontrakt|internship)\b)").expect("regex")
    });
    TYPE.is_match(value)
}

/// A «label: value» (or a label alone with the value on the next line), searched from the top.
fn label_value(lines: &[&str], with_value: &Regex, spaced: &Regex, label_only: &Regex, ok: impl Fn(&str) -> bool) -> Option<String> {
    for (i, raw) in lines.iter().enumerate() {
        let line = unbullet(raw);
        let spaced_value = || spaced.captures(line).filter(|c| !c[1].ends_with('.'));
        if let Some(c) = with_value.captures(line).or_else(spaced_value) {
            // «Firma: Eksempel AS | Sted: Oslo»: only the first part.
            let value = field(c[1].split(['|', '·', '∙', '•']).next().unwrap_or_default());
            if ok(&value) {
                return Some(value);
            }
        } else if label_only.is_match(line) {
            let next = lines[i + 1..].iter().map(|l| unbullet(l)).find(|l| !l.is_empty()).unwrap_or_default();
            let value = field(next);
            if value.chars().count() <= 100 && !value.ends_with('.') && !ANY_LABEL.is_match(&value) && ok(&value) {
                return Some(value);
            }
        }
    }
    None
}

/// A role or employer as it should be stored: one line, no list marks or trailing punctuation.
fn field(value: &str) -> String {
    unbullet(value).split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches([',', ';', ':', '–', '-', '|', '·']).trim().to_string()
}

fn role_ok(value: &str) -> bool {
    let n = value.chars().count();
    (2..=150).contains(&n) && value.chars().filter(|c| c.is_alphabetic()).count() >= 2 && !employment_type(value) && !value.contains("://")
}

fn company_ok(value: &str) -> bool {
    let n = value.chars().count();
    (2..=100).contains(&n) && value.chars().any(char::is_alphabetic) && !value.contains("://") && !value.contains('@')
}

/// Lines at the top of ads that are page furniture, greetings or headings, never the title.
fn boilerplate(line: &str) -> bool {
    const EXACT: &[&str] = &[
        "ledig stilling", "ledige stillinger", "ledig jobb", "stillingsannonse", "jobbannonse", "annonse", "stilling", "jobb", "jobs", "job", "about the job", "about the role",
        "about us", "about you", "job description", "jobbeskrivelse", "stillingsbeskrivelse", "om stillingen", "om jobben", "om rollen", "om oss", "om deg", "om bedriften",
        "om arbeidsgiver", "om arbeidsgiveren", "om selskapet", "om virksomheten", "hei", "hei!", "hello", "hi", "dear", "lagre", "del", "share", "søk", "søk nå", "søk her",
        "apply", "apply now", "easy apply", "meny", "menu", "tilbake", "back", "vis mer", "vis mindre", "show more", "see more", "les mer", "read more", "promoted", "fremhevet",
        "ny", "new", "nyhet", "logg inn", "sign in", "log in", "registrer deg", "mvh", "hilsen", "vennlig hilsen", "med vennlig hilsen", "regards", "best regards",
        "kind regards", "takk", "thanks", "arbeidsoppgaver", "oppgaver", "ansvarsområder", "kvalifikasjoner", "krav", "vi ser etter", "vi søker", "vi tilbyr", "hva vi tilbyr",
        "personlige egenskaper", "egenskaper", "ønskede kvalifikasjoner", "requirements", "qualifications", "responsibilities", "nice to have", "what we offer", "we offer",
        "benefits", "who you are", "what you will do", "your tasks", "your profile", "kontakt", "kontaktinformasjon", "contact", "nøkkelinformasjon", "key information",
        "fast", "heltid", "deltid", "full-time", "part-time", "remote", "hybrid", "on-site", "norge", "norway", "oslo", "bergen", "trondheim", "stavanger", "kristiansand",
        "tromsø", "drammen", "fredrikstad", "sandnes", "bodø", "ålesund", "lillestrøm", "sandvika", "asker", "bærum", "hamar", "gjøvik", "molde", "haugesund", "arendal",
        "tønsberg", "skien", "moss", "sarpsborg", "porsgrunn", "stockholm", "göteborg", "københavn", "copenhagen", "london",
    ];
    const STARTS: &[&str] = &[
        "hopp til", "gå til", "skip to", "logg inn", "sign in", "log in", "lagre ", "del ", "share ", "publisert", "posted", "reposted", "søknadsfrist", "frist", "deadline",
        "sist endret", "finn-kode", "finnkode", "annonse ", "ref.", "referanse", "vis ", "show ", "see who", "se hvem", "dear ", "hei ", "hello ", "hi ", "kjære ",
    ];
    let l = line.trim().trim_end_matches([':', '!', '.']).to_lowercase();
    EXACT.contains(&l.as_str()) || STARTS.iter().any(|s| l.starts_with(s)) || crate::cvread::is_heading(&l)
}

/// A line ending in a company form («AS», «ASA», «AB», «Ltd» …) or a public employer's kind.
fn company_line(line: &str) -> bool {
    static FORM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:\s(?:AS|ASA|SA|ANS|DA|BA|AB|ApS|A/S|Oy|GmbH|AG|Ltd\.?|Limited|Inc\.?|LLC|BV|NV|Plc|KF|IKS|HF|SF)|(?i:\s(?:kommune|fylkeskommune|universitet|universitetssykehus|sykehus|helseforetak|direktoratet|etaten)))$").expect("regex")
    });
    FORM.is_match(line.trim())
}

/// Whether a line at the top of an ad reads like a job title.
fn title_like(line: &str) -> bool {
    static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{1,2}\.\s?\d{1,2}\.\s?\d{2,4}|\d{4}-\d{2}-\d{2}").expect("regex"));
    static SLOGAN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:vi|du|we|you|hos|er du|are you|har du|do you|ønsker du|vil du|brenner du|bli med|join|velkommen|welcome|takk|thank)\b").expect("regex")
    });
    let n = line.chars().count();
    (3..=80).contains(&n)
        && line.split_whitespace().count() <= 8
        && line.chars().filter(|c| c.is_alphabetic()).count() >= 3
        && !line.contains("http")
        && !line.contains("www.")
        && !line.contains('@')
        && !line.ends_with(['.', ',', ':', ';', '!', '?'])
        && !line.contains(':')
        && !line.contains(['·', '∙', '|', '•'])
        && !DATE.is_match(line)
        && !SLOGAN.is_match(line)
        && !boilerplate(line)
}

fn closing(line: &str) -> bool {
    let l = line.trim().trim_end_matches([',', '!', '.']).to_lowercase();
    ["mvh", "hilsen", "vennlig hilsen", "med vennlig hilsen", "beste hilsen", "regards", "best regards", "kind regards", "cheers", "best"].contains(&l.as_str())
}

/// The top of an ad, where the title and employer are: up to the first paragraph.
fn header_lines<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    for raw in lines {
        let line = unbullet(raw);
        if line.is_empty() {
            continue;
        }
        // The first paragraph or a sign-off («Mvh») ends the top.
        let sentence = line.chars().count() > 60 && line.ends_with(['.', '!', '?']);
        if line.chars().count() > 120 || line.split_whitespace().count() > 14 || sentence || out.len() >= 15 || closing(line) {
            break;
        }
        out.push(line);
    }
    out
}

/// The employer from sentences in the ad: «Vi i X søker», «Om X», «X AS», «Hos X får du» …
fn company_in_body(text: &str, role: &str) -> Option<String> {
    const NAME: &str = r"[A-ZÆØÅ0-9][\wÆØÅæøå&.'’\-]*(?:\s+(?:[A-ZÆØÅ0-9&][\wÆØÅæøå&.'’\-]*|og|&|for|i|of|the))*?";
    static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [
            format!(r"\b[Vv]i i ({NAME})\s+(?:søker|ser|er|har|trenger|tilbyr|jobber|skal|leverer|utvikler|bygger|vil)\b"),
            format!(r"(?m)^(?:Om|About)\s+({NAME})\s*$"),
            format!(r"\b({NAME}\s(?:AS|ASA|SA|ANS|AB|ApS|A/S|Oy|GmbH|AG|Ltd|Limited|Inc|LLC|BV|NV|Plc|KF|IKS|HF))\b"),
            r"\b([A-ZÆØÅ][\wæøå\-]+(?:\s[A-ZÆØÅ][\wæøå\-]+)?\s(?:kommune|fylkeskommune))\b".to_string(),
            format!(r"\b[Hh]os ({NAME})\s+(?:får|har|er|jobber|blir|vil|kan|tror|bygger|utvikler|skaper|satser)\b"),
            format!(r"\bAt ({NAME}),\s+we\b"),
            format!(r"\bWe are ({NAME}),"),
            format!(r"\b({NAME})\s+(?:søker|is looking for|is hiring|is seeking|are looking for|are hiring)\b"),
        ]
        .iter()
        .map(|p| Regex::new(p).expect("regex"))
        .collect()
    });
    const NOT_NAMES: &[&str] = &[
        "oss", "us", "deg", "you", "stillingen", "jobben", "rollen", "the job", "the role", "the company", "the team", "the position", "teamet", "avdelingen", "arbeidsgiver",
        "arbeidsgiveren", "bedriften", "selskapet", "firmaet", "virksomheten", "company", "vi", "du", "we", "they", "de", "hun", "han", "vi nå", "nå", "now", "i dag", "today",
    ];
    const LEADING: &[&str] = &["Hos", "Vi", "Om", "I", "At", "About", "Join", "Bli", "Med", "For", "Til", "Velkommen", "Welcome", "Kontakt", "Contact", "Som", "As", "Og", "And", "The", "Nå"];
    for re in PATTERNS.iter() {
        for c in re.captures_iter(text) {
            let mut name = field(&c[1]);
            while let Some((first, rest)) = name.split_once(' ') {
                if LEADING.contains(&first) {
                    name = rest.to_string();
                } else {
                    break;
                }
            }
            let lower = name.to_lowercase();
            if company_ok(&name) && name.chars().count() <= 60 && !NOT_NAMES.contains(&lower.as_str()) && !boilerplate(&name) && !lower.eq(&role.to_lowercase()) && name.chars().next().is_some_and(char::is_uppercase) {
                return Some(name);
            }
        }
    }
    None
}

/// The role from a sentence when the ad has no title line: «Vi søker en prosjektleder til …».
fn role_in_body(text: &str) -> Option<String> {
    static ASK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(?:(?:søker|søkjer|ser etter|trenger|looking for|seeking|hiring)\s+(?:nå\s+|now\s+)?(?:en|et|ei|ein|a|an|our|vår|ny|nye|new)|(?:stilling|jobb|vikariat|position|role|job)\s+(?:som|as(?:\s+an?)?))\s+([^.,;:!?()\n]{2,80})").expect("regex")
    });
    const STOP: &[&str] = &["til", "i", "som", "med", "for", "på", "hos", "ved", "innen", "to", "at", "in", "who", "with", "that", "og", "and", "or", "eller", "fra", "from", "der", "where", "which"];
    const SKIP: &[&str] = &["erfaren", "dyktig", "engasjert", "motivert", "strukturert", "selvgående", "ny", "nye", "kreativ", "erfarne", "dyktige", "experienced", "skilled", "talented", "motivated", "passionate", "driven", "new", "great"];
    const NOT_ROLES: &[&str] = &["person", "kandidat", "kollega", "candidate", "colleague", "deg", "you", "someone", "somebody", "medarbeider", "medarbeidere", "team member"];
    for c in ASK.captures_iter(text) {
        let words: Vec<&str> = c[1].split_whitespace().skip_while(|w| SKIP.contains(&w.to_lowercase().as_str())).take_while(|w| !STOP.contains(&w.to_lowercase().as_str())).take(4).collect();
        let role = words.join(" ");
        if role.chars().count() >= 3 && !NOT_ROLES.contains(&role.to_lowercase().as_str()) {
            let mut chars = role.chars();
            return chars.next().map(|first| first.to_uppercase().chain(chars).collect());
        }
    }
    None
}

/// Reads a job ad from plain text as people paste it (finn.no, NAV, LinkedIn, an e-mail …):
/// «Stilling:» / «Arbeidsgiver:» lines (Norwegian and English) when there are any, else the
/// title and employer from the top of the ad, else from its sentences. Key words from the text.
pub fn read_ad(text: &str) -> AdInfo {
    let clean = clean_paste(text);
    let lines: Vec<&str> = clean.lines().collect();
    let mut role = label_value(&lines, &ROLE_LABELLED, &ROLE_SPACED, &ROLE_ALONE, role_ok).unwrap_or_default();
    let mut company = label_value(&lines, &COMPANY_LABELLED, &COMPANY_SPACED, &COMPANY_ALONE, company_ok).unwrap_or_default();
    let header = header_lines(&lines);
    // finn.no: «Employer», «Title», «Fast stilling ∙ Heltid ∙ Oslo»: the line before the facts
    // line is the title, and the one before that the employer.
    let facts = |l: &str| l.contains(['·', '∙', '|']) && employment_type(&field(l.split(['·', '∙', '|']).next().unwrap_or_default()));
    if let Some(m) = header.iter().position(|l| facts(l)) {
        if role.is_empty() && m >= 1 && title_like(header[m - 1]) {
            role = field(header[m - 1]);
            let before = if m >= 2 { header[m - 2] } else { "" };
            if company.is_empty() && company_ok(before) && before.chars().count() <= 60 && !boilerplate(before) && !before.ends_with(['.', ':', '?', '!']) && !before.contains(':') {
                company = field(before);
            }
        }
    }
    let mut title_at = None;
    if role.is_empty() {
        if let Some(i) = header.iter().position(|l| title_like(l) && !company_line(l) && *l != company) {
            title_at = Some(i);
            role = field(header[i]);
            // LinkedIn and finn put «Employer · Place» on the line after the title.
            if company.is_empty() {
                if let Some(next) = header.get(i + 1) {
                    let first = field(next.split(['·', '∙', '|', '•']).next().unwrap_or_default());
                    if (next.contains(['·', '∙', '|', '•']) || company_line(next)) && company_ok(&first) && !employment_type(&first) && !boilerplate(&first) {
                        company = first;
                    }
                }
            }
        }
    }
    if company.is_empty() {
        if let Some(line) = header.iter().find(|l| company_line(l) && **l != role && !l.contains(':')) {
            company = field(line);
        }
    }
    // «Title» then «Employer» as the first two lines (finn and LinkedIn copies): a short name
    // with a capital, not a place, a heading, an employment type or «Om …».
    if company.is_empty() {
        if let Some(next) = title_at.and_then(|i| header.get(i + 1)) {
            static ABOUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:om|about)\s").expect("regex"));
            let words = next.split_whitespace().count();
            if title_like(next) && words <= 5 && next.chars().next().is_some_and(|c| c.is_uppercase()) && !employment_type(next) && !ABOUT.is_match(next) && role_in_body(next).is_none() {
                company = field(next);
            }
        }
    }
    if role.is_empty() {
        role = role_in_body(&clean).unwrap_or_default();
    }
    if company.is_empty() {
        company = company_in_body(&clean, &role).unwrap_or_default();
    }
    let keywords = ad_keywords(&clean, &company, 25);
    AdInfo { role, company, keywords, description: tidy(text) }
}

/// Words in ads that are about the ad or the job market, not skills a CV should show.
const AD_WORDS: &[&str] = &[
    "stillingstittel", "stillingsnavn", "stilling", "stillingen", "stillinger", "stillingsprosent", "arbeidsgiver", "arbeidsgiveren", "arbeidsgivar", "arbeidssted", "arbeidsspråk",
    "ansettelsesform", "søknadsfrist", "søknad", "søknaden", "søkere", "søker", "kandidat", "kandidater", "kandidaten", "kontaktperson", "kontaktpersoner", "firma", "bedrift",
    "bedriften", "selskap", "selskapet", "virksomhet", "virksomheten", "sektor", "heltid", "deltid", "fast", "vikariat", "kvalifikasjoner", "arbeidsoppgaver", "egenskaper",
    "tilbyr", "erfaring", "kjennskap", "kompetanse", "fortrinnsvis", "utdanning", "utdannelse", "løsning", "løsninger", "løsningene", "fungerende", "gjerne", "fordel",
    "ønskelig", "mulighet", "muligheter", "hverdag", "hverdagen", "arbeidstid", "fleksitid", "hjemmekontor", "lønn", "årslønn", "lønnstrinn", "pensjon", "pensjonsordning",
    "pensjonskasse", "forsikring", "betingelser", "kollegaer", "kolleger", "kollegaene", "avdeling", "avdelingen", "ansvar", "ansvaret", "oppgaver", "oppgavene", "rollen",
    "jobben", "teamet", "teknologi", "teknologien", "applikasjoner", "utvikling", "utviklingen", "kunnskap", "kunnskaper", "interesse", "motivasjon", "personlig",
    "personlige", "mennesker", "samfunnet", "samfunnsoppdrag", "fremtidens", "spennende", "tverrfaglig", "tverrfaglige", "selvstendig", "selvstendige", "relevant",
    "relevante", "tilsvarende", "nødvendig", "nødvendige", "inkluderende", "mangfold", "arbeidsmiljø", "arbeidsmiljøet", "medarbeidere", "medarbeider", "organisasjon",
    "organisasjonen", "kommunikasjonsevner", "samarbeidsevner", "norskkunnskaper", "lederstillinger", "ledige", "omfattende", "annonse", "annonsen", "stillingsannonse",
    "about", "job", "jobs", "company", "employer", "title", "position", "role", "location", "deadline", "apply", "requirements", "qualifications", "responsibilities",
    "benefits", "nice", "offer", "salary", "remote", "hybrid", "onsite", "years", "year", "experience", "knowledge", "skills", "ability", "strong", "good", "excellent",
    "team", "teams", "work", "working", "jobbmatch", "finn", "nav", "linkedin", "arbeidsplassen", "logg", "lagre", "januar", "februar", "mars", "april", "mai", "juni",
    "juli", "august", "september", "oktober", "november", "desember", "january", "february", "march", "june", "july", "october", "december", "mandag", "tirsdag",
    "onsdag", "torsdag", "fredag", "lørdag", "søndag", "jobbnorge", "webcruiter", "arbeidstiden", "arbeidstidsordning", "arbeidshverdag", "arbeidshverdagen", "bachelornivå", "masternivå", "utviklere", "arbeidsoppgavene", "eventuelt", "komplekse", "kvalifisere", "kvalifiserte", "kvalifisert", "bakgrunner", "bakgrunn", "rekrutteringssystemet", "ferdigheter", "kreativitet", "offentlig", "offentlige", "beslutningsdyktig", "boliglånsordninger", "bærekraftige", "aktivitetsbasert", "varierte", "varierende", "ansatte", "ansettelse", "arbeidsplass", "lønnsplassering", "stillingskode", "søknadsprosessen", "attester", "vitnemål", "sikkerhetsklarering", "sikkerhetsklareres", "politiattest", "spennende", "utfordrende", "utfordringer", "selvstendighet", "gjennomføringsevne", "kommunikasjon", "samarbeid", "fleksibel", "fleksible", "konkurransedyktig", "konkurransedyktige", "opportunities", "including", "environment", "candidates", "application", "om", "oss", "vi", "du", "deg", "ser", "etter", "har", "med", "og", "eller", "fra", "til", "for", "en", "et",
];

/// Key words from an ad: technologies and tools (also «C#», «.NET», «Node.js», «Power BI»,
/// «SAP S/4HANA», «TwinCAT 3»), acronyms and domain words, strongest first, each once, spelled
/// as the ad spells them. Labels, headings, the employer's own name and ad phrases are left out.
pub fn ad_keywords(text: &str, company: &str, limit: usize) -> Vec<String> {
    static WEB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)https?://\S+|www\.\S+|[\w.+\-]+@[\w\-]+\.[\w.\-]+").expect("regex"));
    static SHOUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-ZÆØÅ]{2,}(?:[ \t]+[A-ZÆØÅ]{2,})+\b").expect("regex"));
    let clean = clean_paste(text);
    // The text without labels, web addresses and SHOUTED headings, which are not key words.
    let mut body = String::with_capacity(clean.len());
    for raw in clean.lines() {
        let line = unbullet(raw);
        let line = ANY_LABEL.find(line).map_or(line, |m| &line[m.end()..]);
        let line = WEB.replace_all(line, " ");
        let letters: Vec<char> = line.chars().filter(|c| c.is_alphabetic()).collect();
        let shouted = !letters.is_empty() && letters.iter().all(|c| c.is_uppercase()) && line.split_whitespace().all(|w| w.trim_end_matches([':', '&']).chars().all(char::is_alphabetic));
        let line = if shouted && (line.split_whitespace().any(|w| w.chars().count() >= 7) || boilerplate(&line)) {
            line.to_lowercase()
        } else {
            SHOUT.replace_all(&line, |c: &regex::Captures| if c[0].split_whitespace().any(|w| w.chars().count() >= 7) { c[0].to_lowercase() } else { c[0].to_string() }).into_owned()
        };
        body.push_str(&line);
        body.push('\n');
    }
    let norm = checks::normal(&body);
    let mut out: Vec<String> = Vec::new();
    let push = |term: String, out: &mut Vec<String>| {
        let key = checks::normal(&term);
        if !key.is_empty() && !out.iter().any(|k| checks::normal(k) == key) {
            out.push(term);
        }
    };
    let tech = tech_terms(&body);
    let suggested: Vec<String> = checks::suggest_keywords(&body, limit * 3).into_iter().filter(|t| !t.contains('\n') && !junk_term(t)).map(|t| spelled_as_in(&body, &t)).collect();
    let known: Vec<String> = tech.iter().chain(&suggested).cloned().collect();
    for term in tech.into_iter().chain(list_names(&body, &known)).chain(suggested) {
        push(term, &mut out);
    }
    let company_norm = checks::normal(company);
    let mut kept: Vec<String> = Vec::new();
    for term in &out {
        let key = checks::normal(term);
        if AD_WORDS.contains(&key.as_str()) || key == company_norm {
            continue;
        }
        // «sikkerhets-» in «sikkerhets- og beredskapsarbeid» is half a word.
        if body.contains(&format!("{term}- ")) && !body.contains(&format!("{term} ")) {
            continue;
        }
        // «PSTs» is the possessive of «PST».
        if term.len() > 2 && term.ends_with('s') && term[..term.len() - 1].chars().all(|c| c.is_uppercase()) && out.iter().any(|o| *o == term[..term.len() - 1]) {
            continue;
        }
        // «BI» that only ever stands in «Power BI», «analytics» only in «Acme Analytics».
        let inside = |longer: &str| {
            let long = checks::normal(longer);
            long != key && format!(" {long} ").contains(&format!(" {key} ")) && count_words(&norm, &key) <= count_words(&norm, &long)
        };
        if out.iter().any(|o| inside(o)) || (!company_norm.is_empty() && inside(company)) {
            continue;
        }
        // «løsninger» and «løsningene»: one word, inflected.
        if !key.contains(' ') && kept.iter().any(|k| same_stem(&checks::normal(k), &key)) {
            continue;
        }
        kept.push(term.clone());
    }
    kept.truncate(limit);
    kept
}

/// Technology names the general keyword finder splits or misses: dotted and slashed names,
/// known multi-word products and «Vendor Product» pairs, in the order the ad has them.
fn tech_terms(body: &str) -> Vec<String> {
    static DOTTED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:^|[^\w.])(ASP\.NET(?: Core)?|\.NET(?: (?:Core|Framework|MAUI)\b)?|[A-Za-z][A-Za-z0-9]*\.js|(?:SAP )?S/4 ?HANA|CI/CD|TCP/IP|UI/UX|UX/UI|OT/IT|IT/OT|MIG/MAG|PL/SQL|T-SQL|(?:NS-EN(?: ISO)?|NS-ISO|NS|EN|ISO|IEC|NEK) ?\d{3,5}(?:-\d{1,2})*)(?:$|[^\w])").expect("regex")
    });
    static KNOWN: LazyLock<Regex> = LazyLock::new(|| {
        let names = [
            "Power BI", "Power Apps", "Power Automate", "Power Platform", "Azure DevOps", "Azure Functions", "Google Cloud(?: Platform)?", "Google Analytics", "Apache Spark",
            "Apache Kafka", "Apache Airflow", "Spring Boot", "Visual Studio(?: Code)?", "SQL Server", "Microsoft 365", "Office 365", "Dynamics 365", "Business Central",
            "GitHub Actions", "GitLab CI", "Red Hat", "React Native", "Ruby on Rails", "Unreal Engine", "TIA Portal", "Creative Cloud", "machine learning", "deep learning",
            "Material Design", "Data Factory", "Solid ?Works", "Revit MEP", "Tekla Structures",
        ];
        Regex::new(&format!(r"(?i)\b(?:{})\b", names.join("|"))).expect("regex")
    });
    static VENDOR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b(?:SAP|Microsoft|Azure|AWS|Google|Apache|Adobe|Autodesk|Siemens|Oracle|IBM|Atlassian|Cisco|Salesforce|Beckhoff|Schneider|Rockwell|Visma|Unit4) (?:[A-Z][A-Za-z0-9]+|[A-Z0-9]{2,})(?: (?:[A-Z][a-z]+[A-Za-z0-9]*|\d{2,4}))?\b").expect("regex")
    });
    const NOT_PRODUCTS: &[&str] = &["Er", "Og", "Is", "And", "Or", "The", "I", "Med", "Eller", "Som", "For", "Til", "Kan", "Vi", "Du", "We", "You", "Norge", "Norway", "Norden"];
    let mut found: Vec<(usize, String)> = Vec::new();
    for c in DOTTED.captures_iter(body) {
        if let Some(m) = c.get(1) {
            found.push((m.start(), m.as_str().to_string()));
        }
    }
    for m in KNOWN.find_iter(body) {
        found.push((m.start(), m.as_str().split_whitespace().collect::<Vec<_>>().join(" ")));
    }
    for m in VENDOR.find_iter(body) {
        let words: Vec<&str> = m.as_str().split(' ').collect();
        let product: Vec<&str> = words.iter().skip(1).take_while(|w| !NOT_PRODUCTS.contains(w)).copied().collect();
        if !product.is_empty() {
            found.push((m.start(), format!("{} {}", words[0], product.join(" "))));
        }
    }
    found.sort_by_key(|(at, _)| *at);
    // «SAP S/4HANA» over «S/4HANA», «Google Cloud Platform» over «Google Cloud».
    let all: Vec<String> = found.into_iter().map(|(_, t)| t).collect();
    all.iter().filter(|t| !all.iter().any(|o| o.len() > t.len() && o.to_lowercase().contains(&t.to_lowercase()))).cloned().collect()
}

/// Names in a list next to a known technology: in «Kubernetes, Terraform og AWS» the first two
/// count once AWS is known. Places and people in lists have no such neighbour.
fn list_names(body: &str, known: &[String]) -> Vec<String> {
    static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(?:[,;]|\s/\s|\s(?:og|and|or|eller|samt|&)\s)\s*").expect("regex"));
    const NOT_NAMES: &[&str] = &["vi", "du", "we", "you", "i", "it", "the", "a", "en", "et", "de", "der", "det", "dette", "this", "that", "our", "vår", "våre"];
    static TITLE_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:leder|rådgiver|ingeniør|direktør|sjef|manager|konsulent|koordinator|director|officer|advisor)\b").expect("regex"));
    let known: Vec<String> = known.iter().map(|k| checks::normal(k)).collect();
    let name_word = |w: &str| {
        let w = w.trim_matches(|c: char| "()\"«»“”".contains(c));
        !w.ends_with(['.', ':', '!', '?']) && w.chars().next().is_some_and(|c| c.is_uppercase() || c == '.') && w.chars().any(char::is_alphabetic)
    };
    let techy = |c: &str| {
        let caps = c.chars().filter(|ch| ch.is_uppercase()).count();
        caps >= 2 || c.chars().any(|ch| ch.is_ascii_digit() || "#+.".contains(ch)) || c.as_bytes().windows(2).any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase())
    };
    let mut out = Vec::new();
    for line in body.lines() {
        let items: Vec<&str> = SPLIT.split(line).map(str::trim).filter(|s| !s.is_empty()).collect();
        if items.len() < 2 {
            continue;
        }
        let last = items.len() - 1;
        let mut chunks: Vec<String> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let words: Vec<&str> = item.split_whitespace().collect();
            let chunk: Vec<&str> = if i == 0 && last > 0 {
                let mut tail: Vec<&str> = words.iter().rev().take_while(|w| name_word(w)).copied().collect();
                tail.reverse();
                tail
            } else if i == last {
                words.iter().take_while(|w| name_word(w)).copied().collect()
            } else if words.iter().all(|w| name_word(w)) {
                words
            } else {
                Vec::new()
            };
            let chunk = chunk.iter().map(|w| w.trim_matches(|c: char| "()\"«»“”".contains(c))).collect::<Vec<_>>().join(" ");
            // «FPGA-» in «FPGA- og kretsdesign» is half a word.
            if chunk.ends_with('-') {
                continue;
            }
            let key = checks::normal(&chunk);
            let count = chunk.split_whitespace().count();
            // Two plain capitalised words are more often a person («Kari Nordmann») than a tool.
            let plain_pair = count > 1 && !techy(&chunk) && !known.contains(&key);
            let job_title = TITLE_WORD.is_match(&key);
            if (1..=3).contains(&count) && (2..=30).contains(&chunk.chars().count()) && !plain_pair && !job_title && !NOT_NAMES.contains(&key.as_str()) && !AD_WORDS.contains(&key.as_str()) && !boilerplate(&chunk) {
                chunks.push(chunk);
            }
        }
        if chunks.len() >= 2 && chunks.iter().any(|c| known.contains(&checks::normal(c))) {
            out.extend(chunks);
        }
    }
    out
}

/// Terms the general keyword finder gives that are never key words: salary codes and phone
/// numbers.
fn junk_term(term: &str) -> bool {
    static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:sko|styrk|lr|ltr|kode|stillingskode|ref|id|nr|tlf|tel|mob)\b|\d{6,}").expect("regex"));
    CODE.is_match(term)
}

/// How many times a normalised term stands as whole words in normalised text.
fn count_words(norm: &str, key: &str) -> usize {
    format!(" {norm} ").matches(&format!(" {key} ")).count()
}

/// Two Norwegian or English word forms of one word («løsninger», «løsningene»).
fn same_stem(a: &str, b: &str) -> bool {
    fn stem(w: &str) -> &str {
        for end in ["ene", "ane", "ers", "er", "en", "et", "ar", "a", "e", "s"] {
            if let Some(s) = w.strip_suffix(end) {
                if s.chars().count() >= 5 {
                    return s;
                }
            }
        }
        w
    }
    !a.contains(' ') && a.chars().count() >= 6 && b.chars().count() >= 6 && (stem(a) == stem(b) || stem(a) == b || stem(b) == a)
}

/// The term as the ad spells it («python» → «Python», «c#» → «C#»): taken from a place in the
/// middle of a sentence, so a capital that only starts a sentence or a list item is not used.
fn spelled_as_in(body: &str, term: &str) -> String {
    if term.chars().any(char::is_uppercase) {
        return term.to_string();
    }
    let pattern = term.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+");
    let Ok(re) = Regex::new(&format!(r"(?i)(?:^|[^\w])({pattern})(?:$|[^\w])")) else { return term.to_string() };
    let short = term.chars().count() <= 4 || term.chars().any(|c| !c.is_alphabetic() && c != ' ');
    let mut first: Option<String> = None;
    for line in body.lines() {
        for c in re.captures_iter(line) {
            let Some(m) = c.get(1) else { continue };
            let before = line[..m.start()].trim_end();
            let mid = !before.is_empty() && !before.ends_with(['.', '!', '?', ':']);
            let spelled = m.as_str().split_whitespace().collect::<Vec<_>>().join(" ");
            if mid && spelled.chars().any(char::is_uppercase) {
                return spelled;
            }
            if mid {
                return term.to_string();
            }
            first.get_or_insert(spelled);
        }
    }
    match first {
        Some(s) if short => s,
        _ => term.to_string(),
    }
}

/// The page's own job data (schema.org JobPosting), wherever it sits in the page's JSON-LD:
/// alone, in a list, in «@graph» or wrapped (finn.no wraps it in another object).
fn job_posting(html: &str) -> Option<Value> {
    static LD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<script[^>]*application/ld\+json[^>]*>(.*?)</script\s*>"#).expect("regex"));
    fn is_job(v: &Value) -> bool {
        let named = |t: &Value| t.as_str().is_some_and(|s| s.eq_ignore_ascii_case("JobPosting") || s.ends_with("/JobPosting"));
        match v.get("@type") {
            Some(Value::Array(a)) => a.iter().any(named),
            Some(t) => named(t),
            None => false,
        }
    }
    fn find(v: &Value, depth: u8) -> Option<&Value> {
        if depth > 8 {
            return None;
        }
        match v {
            Value::Object(o) if is_job(v) => (!o.is_empty()).then_some(v),
            Value::Object(o) => o.values().find_map(|x| find(x, depth + 1)),
            Value::Array(a) => a.iter().find_map(|x| find(x, depth + 1)),
            _ => None,
        }
    }
    for c in LD.captures_iter(html) {
        let raw = c[1].trim().trim_start_matches("<!--").trim_end_matches("-->").replace("/*<![CDATA[*/", "").replace("/*]]>*/", "").replace("<![CDATA[", "").replace("]]>", "");
        let raw = raw.trim().trim_end_matches(';');
        // Some sites put raw line breaks inside the JSON strings, which JSON does not allow.
        let parsed = serde_json::from_str::<Value>(raw).or_else(|_| serde_json::from_str::<Value>(&raw.replace(['\n', '\r', '\t'], " ")));
        if let Ok(v) = parsed {
            if let Some(job) = find(&v, 0) {
                return Some(job.clone());
            }
        }
    }
    None
}

/// A JSON-LD value as one line of text: a string, the first of a list, or an object's name.
fn ld_text(v: &Value) -> String {
    match v {
        Value::String(s) => field(&strip_html(&entities(s)).replace('\n', " ")),
        Value::Array(a) => a.iter().map(ld_text).find(|s| !s.is_empty()).unwrap_or_default(),
        Value::Object(o) => ["name", "legalName", "@value"].iter().filter_map(|k| o.get(*k)).map(ld_text).find(|s| !s.is_empty()).unwrap_or_default(),
        _ => String::new(),
    }
}

/// The ad's title from a page without job data: its <h1>, else og:title, else <title>, without
/// the site's name («Backend utvikler - arbeidsplassen.no»).
fn page_title(html: &str) -> Option<String> {
    static H1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<h1\b[^>]*>(.*?)</h1\s*>").expect("regex"));
    static OG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)<meta[^>]+(?:property|name)=["']og:title["'][^>]*content=["']([^"']*)|<meta[^>]+content=["']([^"']*)["'][^>]*(?:property|name)=["']og:title["']"#).expect("regex")
    });
    static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<title\b[^>]*>(.*?)</title\s*>").expect("regex"));
    static SITE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.(?:no|com|se|dk|io|org|net)\b|finn|nav\b|linkedin|arbeidsplassen|jobbnorge|webcruiter|indeed|glassdoor|karrierestart|(?:jobb|jobs?|karriere|careers?)\b|stillinger").expect("regex"));
    let site_free = |t: String| -> String {
        for sep in [" | ", " - ", " – ", " — ", " :: "] {
            if let Some((head, tail)) = t.rsplit_once(sep) {
                if SITE.is_match(tail) {
                    return field(head);
                }
            }
        }
        t
    };
    // An <h1> inside a script or a comment is not on the page.
    static HIDDEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<!--.*?-->|<script\b.*?</script\s*>|<style\b.*?</style\s*>|<template\b.*?</template\s*>|<noscript\b.*?</noscript\s*>").expect("regex"));
    let visible = HIDDEN.replace_all(html, " ");
    let html = visible.as_ref();
    let h1 = H1.captures_iter(html).map(|c| field(&strip_html(&c[1]).replace('\n', " "))).find(|t| !t.is_empty());
    let og = OG.captures(html).and_then(|c| c.get(1).or(c.get(2))).map(|m| field(&entities(m.as_str())));
    let title = TITLE.captures(html).map(|c| field(&entities(&c[1])));
    [h1, og, title].into_iter().flatten().map(site_free).find(|t| title_like(t))
}

/// An ad from its parts, written as «Stilling: …» / «Arbeidsgiver: …» lines over the text,
/// so the text in the ad box later reads back to the same role and employer.
fn ad_with(role: &str, company: &str, body: &str) -> AdInfo {
    let mut head = String::new();
    if !role.is_empty() {
        head.push_str(&format!("Stilling: {role}\n"));
    }
    if !company.is_empty() {
        head.push_str(&format!("Arbeidsgiver: {company}\n"));
    }
    let mut ad = read_ad(&format!("{head}\n{body}"));
    if !role.is_empty() {
        ad.role = role.to_string();
    }
    if !company.is_empty() {
        ad.company = company.to_string();
    }
    ad
}

/// Reads a job ad from a web page: the page's own job data (schema.org JobPosting) when it has
/// it, which most job boards do, else its title and text. Never fails; see [`ad_from_page`].
pub fn ad_from_html(html: &str) -> AdInfo {
    ad_from_page(html).unwrap_or_else(|_| read_ad(&strip_html(html)))
}

/// Like [`ad_from_html`], but says so when the page is a sign-in wall or shows no ad text
/// (pages built by JavaScript), so the person can paste the text instead.
pub fn ad_from_page(html: &str) -> Result<AdInfo, String> {
    if let Some(job) = job_posting(html) {
        let role = ld_text(job.get("title").or(job.get("name")).unwrap_or(&Value::Null));
        let company = ld_text(job.get("hiringOrganization").unwrap_or(&Value::Null));
        let mut description = match job.get("description") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => ld_text(other),
            None => String::new(),
        };
        // Some sites escape the HTML in the description («&lt;p&gt;»).
        if !description.contains('<') && description.contains("&lt;") {
            description = entities(&description);
        }
        let mut body = strip_html(&description);
        if !body.chars().any(char::is_alphanumeric) {
            body = strip_html(html);
        }
        return Ok(ad_with(&role, &company, &body));
    }
    let text = strip_html(html);
    let letters = text.chars().filter(|c| c.is_alphanumeric()).count();
    let lower = html.to_lowercase();
    let start: String = text.chars().take(3000).collect::<String>().to_lowercase();
    let sign_in = ["logg inn", "log in", "sign in", "innlogging", "logg på", "sign up"].iter().any(|w| start.contains(w));
    if (lower.contains("type=\"password\"") || lower.contains("type='password'") || lower.contains("authwall")) && (sign_in || letters < 3000) || (sign_in && letters < 600) {
        return Err("Siden krever innlogging, så annonsen kunne ikke hentes. Åpne den i nettleseren, kopier teksten og lim den inn.".into());
    }
    if letters < 80 {
        return Err("Siden viste nesten ingen tekst. Noen nettsteder bygger siden med JavaScript; åpne den i nettleseren, kopier teksten og lim den inn.".into());
    }
    Ok(ad_with(&page_title(html).unwrap_or_default(), "", &text))
}

/// Keywords from the ad that the CV already shows (in a job, the profile, education …) but that
/// are not in the skills or languages: they can be added to the skills honestly.
pub fn skills_you_have(project: &Project) -> Vec<String> {
    let mut without = project.clone();
    without.skills.clear();
    without.languages.clear();
    let text = checks::normal(&checks::cv_text(&without));
    let listed = checks::normal(&project.skills.iter().flat_map(|s| [s.name.as_str(), s.group.as_str()]).chain(project.languages.iter().map(|l| l.name.as_str())).collect::<Vec<_>>().join("\n"));
    let not_skills = [checks::normal(&project.target.role), checks::normal(&project.target.company)];
    let mut seen = std::collections::HashSet::new();
    project
        .target
        .keywords
        .iter()
        .map(|k| k.trim())
        .filter(|k| {
            let key = checks::normal(k);
            !key.is_empty() && !not_skills.contains(&key) && seen.insert(key) && shows(&text, k) && !checks::contains(&listed, k)
        })
        .map(String::from)
        .collect()
}

/// Whether normalised CV text shows a key word. Names (with capitals, digits or signs, or
/// short) must stand as a whole word, so «Java» is not found in «JavaScript»; ordinary
/// words may be inflected («feilsøking» in «feilsøkingen»).
fn shows(text: &str, keyword: &str) -> bool {
    let name = keyword.chars().any(|c| c.is_uppercase() || c.is_ascii_digit() || "#+./".contains(c)) || keyword.chars().count() <= 5;
    if name {
        keyword.split(['|']).map(checks::normal).filter(|o| !o.is_empty()).any(|o| format!(" {text} ").contains(&format!(" {o} ")))
    } else {
        checks::contains(text, keyword)
    }
}

/// Fills the target job from an ad and adds the ad's keywords the CV already shows to the
/// skills (group «Fra annonsen»). What the person typed is kept; what CVBuilder filled from the
/// previous ad text is replaced when a different ad comes in. Running it twice changes nothing
/// the second time. Returns a summary.
pub fn apply_ad(project: &mut Project, ad: &AdInfo) -> String {
    if ad.role.trim().is_empty() && ad.company.trim().is_empty() && ad.keywords.is_empty() && ad.description.trim().is_empty() {
        return "Fant ikke noe å fylle ut. Lim inn hele annonseteksten først.".into();
    }
    let t = &mut project.target;
    let new_ad = !ad.description.trim().is_empty() && clean_paste(&t.description) != clean_paste(&ad.description);
    // What the previous ad text gives was filled by CVBuilder, not typed, so it may go.
    let old = if new_ad && !t.description.trim().is_empty() { read_ad(&t.description) } else { AdInfo::default() };
    let free = |current: &str, from_old: &str| current.trim().is_empty() || (!from_old.is_empty() && current.trim() == from_old.trim());
    let mut found = Vec::new();
    if !ad.role.trim().is_empty() && free(&t.role, &old.role) && t.role.trim() != ad.role.trim() {
        t.role = ad.role.trim().to_string();
        found.push(format!("stilling «{}»", t.role));
    }
    if !ad.company.trim().is_empty() && free(&t.company, &old.company) && t.company.trim() != ad.company.trim() {
        t.company = ad.company.trim().to_string();
        found.push(format!("arbeidsgiver «{}»", t.company));
    }
    if new_ad {
        t.description = ad.description.clone();
        let old_words: Vec<String> = old.keywords.iter().map(|k| checks::normal(k)).collect();
        t.keywords.retain(|k| !old_words.contains(&checks::normal(k)));
    }
    let before = t.keywords.len();
    for k in &ad.keywords {
        let key = checks::normal(k);
        if !key.is_empty() && !t.keywords.iter().any(|x| checks::normal(x) == key) && t.keywords.len() < 30 {
            t.keywords.push(k.trim().to_string());
        }
    }
    if t.keywords.len() > before {
        found.push(format!("{} fagord", t.keywords.len() - before));
    }
    let have = skills_you_have(project);
    for k in &have {
        project.skills.push(Skill { group: "Fra annonsen".into(), name: k.clone(), evidence: "Står allerede i CV-en".into(), confirmed: true });
    }
    let mut out = if found.is_empty() { "Annonsen er lagt inn.".to_string() } else { format!("Fra annonsen: {}.", found.join(", ")) };
    if !have.is_empty() {
        out.push_str(&format!(" {} fagord står allerede i CV-en din og er lagt til i kompetanse: {}.", have.len(), have.join(", ")));
    }
    out
}

pub use crate::cvread::{CvRead, read_cv};
