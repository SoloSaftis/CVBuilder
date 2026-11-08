//! Validation and the recruiter/ATS checklist. A port of the HTML builder's `contract.js`
//! (`ready`, dates) and `recruiter-checks.js` (`audit`, keywords, plain text). The parity test
//! in `tests/parity.rs` runs both on the same project and compares the results.
use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::model::Project;
use crate::text;

// ---------- dates ----------

const MONTHS: [(&str, u32); 37] = [
    ("jan", 1), ("januar", 1), ("january", 1), ("feb", 2), ("februar", 2), ("february", 2), ("mar", 3), ("mars", 3), ("march", 3),
    ("apr", 4), ("april", 4), ("mai", 5), ("may", 5), ("jun", 6), ("juni", 6), ("june", 6), ("jul", 7), ("juli", 7), ("july", 7),
    ("aug", 8), ("august", 8), ("sep", 9), ("sept", 9), ("september", 9), ("okt", 10), ("oktober", 10), ("oct", 10), ("october", 10),
    ("nov", 11), ("november", 11), ("des", 12), ("desember", 12), ("dec", 12), ("december", 12), ("feb.", 2), ("okt.", 10), ("des.", 12),
];

fn pad(year: &str, month: u32) -> Option<String> {
    let year: u32 = year.parse().ok()?;
    ((1..=12).contains(&month) && (1900..=2100).contains(&year)).then(|| format!("{year}-{month:02}"))
}

/// Reads 09.2021, 9/2021, 2021-09, «sep. 2021», «mars 2019». `Some("")` for empty input,
/// `None` when the text cannot be read. The result is `YYYY-MM`.
pub fn parse_month(input: &str) -> Option<String> {
    static YEAR_FIRST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4})\s*[-./]\s*(\d{1,2})$").expect("regex"));
    static MONTH_FIRST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})\s*[-./ ]\s*(\d{4})$").expect("regex"));
    static NAMED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([a-zæøå]+)\.?\s*(\d{4})$").expect("regex"));
    let text = input.trim().to_lowercase();
    if text.is_empty() {
        return Some(String::new());
    }
    // A bare year is allowed when the month is not remembered.
    if text.len() == 4 && (text.starts_with("19") || text.starts_with("20")) && text.chars().all(|c| c.is_ascii_digit()) {
        return Some(text);
    }
    if let Some(c) = YEAR_FIRST.captures(&text) {
        return pad(&c[1], c[2].parse().ok()?);
    }
    if let Some(c) = MONTH_FIRST.captures(&text) {
        return pad(&c[2], c[1].parse().ok()?);
    }
    let c = NAMED.captures(&text)?;
    let month = MONTHS.iter().find(|(name, _)| *name == &c[1])?.1;
    pad(&c[2], month)
}

/// Reads a year from «2016», «aug. 2016» or «2016-08».
pub fn parse_year(input: &str) -> Option<String> {
    static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\D)((?:19|20)\d{2})(?:\D|$)").expect("regex"));
    let text = input.trim();
    if text.is_empty() {
        return Some(String::new());
    }
    YEAR.captures(text).map(|c| c[1].to_string())
}

/// `2021-09` → `09.2021`.
pub fn show_month(value: &str) -> String {
    value.split_once('-').map(|(y, m)| format!("{m}.{y}")).unwrap_or_else(|| value.to_string())
}

/// «nå», «now», «d.d.», «present» typed as an end date mean a current job.
pub fn is_now(input: &str) -> bool {
    let t = input.trim().to_lowercase().replace(' ', "");
    matches!(t.as_str(), "nå" | "naa" | "now" | "present" | "dd" | "d.d" | "d.d." | "dagsdato" | "idag" | "today" | "pågående" | "ongoing")
}

// ---------- release check ----------

fn placeholder(value: &str) -> bool {
    static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\[(?:fyll|insert|navn|name|tall|antall|number)|<[^>]+>|\b(?:lorem ipsum|todo|tbd)\b").expect("regex"));
    PLACEHOLDER.is_match(value)
}

/// What must be fixed before export, in plain Norwegian. Approval is reported last as
/// `approvals: …` so callers can ask for it at download time instead.
pub fn ready(p: &Project) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    let c = &p.candidate;
    for (value, name) in [(&c.name, "Fullt navn"), (&c.email, "E-post"), (&c.phone, "Telefon"), (&c.city, "By og land"), (&c.headline, "Faglig overskrift")] {
        if value.trim().is_empty() {
            errors.push(format!("{name} må fylles ut."));
        }
    }
    static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").expect("regex"));
    static PHONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\+?[0-9 ()-]{6,30}$").expect("regex"));
    if !c.email.is_empty() && !EMAIL.is_match(&c.email) {
        errors.push("E-postadressen ser ikke riktig ut.".into());
    }
    if !c.phone.is_empty() && (!PHONE.is_match(&c.phone) || c.phone.chars().filter(char::is_ascii_digit).count() < 6) {
        errors.push("Telefonnummeret ser ikke riktig ut. Skriv for eksempel +47 412 34 567.".into());
    }
    if p.target.role.trim().is_empty() {
        errors.push("Skriv hvilken stilling du søker (under «Stillingen»).".into());
    }
    let https = |v: &str| v.is_empty() || (v.starts_with("https://") && !v.contains(char::is_whitespace));
    if !https(&p.target.url) {
        errors.push("Annonselenken må starte med https://.".into());
    }
    if !https(&c.linkedin) {
        errors.push("LinkedIn må være full adresse, for eksempel https://www.linkedin.com/in/navn.".into());
    }
    if p.certifications.iter().any(|x| x.name.trim().is_empty()) {
        errors.push("Et kurs mangler navn.".into());
    }
    if c.summary.split_whitespace().count() > 90 {
        errors.push("Profilen er for lang. Kort den ned til høyst 90 ord, helst 30–70.".into());
    }
    if p.experience.is_empty() && p.education.is_empty() {
        errors.push("Legg til minst én jobb eller utdanning.".into());
    }
    for (i, job) in p.experience.iter().enumerate() {
        let name = if job.company.trim().is_empty() { format!("Jobb {}", i + 1) } else { format!("Jobb {} ({})", i + 1, job.company.trim()) };
        if job.company.trim().is_empty() || job.title.trim().is_empty() {
            errors.push(format!("{name}: fyll inn arbeidsgiver og stillingstittel."));
        }
        if job.start.is_empty() {
            errors.push(format!("{name}: fyll inn når du startet (for eksempel 09.2021)."));
        }
        match &job.end {
            Some(end) if end.is_empty() => errors.push(format!("{name}: fyll inn når du sluttet, eller kryss av for «Nåværende jobb».")),
            Some(end) if !job.start.is_empty() && end < &job.start => errors.push(format!("{name}: sluttdatoen er før startdatoen.")),
            _ => {}
        }
        if job.bullets.is_empty() {
            errors.push(format!("{name}: skriv minst ett punkt om hva du gjorde."));
        }
    }
    for (i, item) in p.education.iter().enumerate() {
        let name = format!("Utdanning {}", i + 1);
        if item.qualification.trim().is_empty() || item.institution.trim().is_empty() {
            errors.push(format!("{name}: fyll inn utdanning og lærested."));
        }
        if item.end.is_empty() {
            errors.push(format!("{name}: fyll inn sluttår (forventet år hvis du fortsatt studerer)."));
        }
        if !item.start.is_empty() && !item.end.is_empty() && item.end < item.start {
            errors.push(format!("{name}: sluttåret er før startåret."));
        }
    }
    for (i, item) in p.skills.iter().enumerate() {
        if item.name.trim().is_empty() {
            errors.push(format!("Kompetanse {} mangler navn.", i + 1));
        }
    }
    if p.languages.iter().any(|x| x.name.trim().is_empty() || x.level.trim().is_empty()) {
        errors.push("Et språk mangler navn eller nivå.".into());
    }
    if p.interests.iter().any(|x| x.label.trim().is_empty()) {
        errors.push("En interesse mangler navn.".into());
    }
    let exported = std::iter::once(&c.name)
        .chain([&c.headline, &c.summary])
        .chain(p.experience.iter().flat_map(|j| [&j.company, &j.title].into_iter().chain(j.bullets.iter())))
        .chain(p.skills.iter().flat_map(|s| [&s.group, &s.name]))
        .chain(p.certifications.iter().flat_map(|x| [&x.name, &x.issuer]))
        .chain(p.education.iter().flat_map(|e| [&e.qualification, &e.institution]));
    if exported.into_iter().any(|v| placeholder(v)) {
        errors.push("Teksten inneholder plassholdere som [tall] eller [fyll inn], eller kode som <…>. Fyll inn ekte tall og tekst, eller fjern dem.".into());
    }
    if p.options.photo && p.candidate.photo.is_empty() {
        errors.push("Bildevalget er på, men det er ikke lagt inn noe bilde.".into());
    }
    if !p.approvals.facts_confirmed {
        errors.push("approvals: bekreft at opplysningene er riktige".into());
    }
    let mut seen = HashSet::new();
    errors.retain(|e| seen.insert(e.clone()));
    errors
}

/// Everything in [`ready`] except the approval itself.
pub fn blocking(p: &Project) -> Vec<String> {
    ready(p).into_iter().filter(|e| !e.starts_with("approvals:")).collect()
}

// ---------- recruiter checklist ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Pass,
    Warn,
    Info,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Check {
    pub level: Level,
    pub text: String,
    pub tip: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct KeywordMatch {
    pub found: Vec<String>,
    pub missing: Vec<String>,
    pub coverage: Option<u32>,
}

/// Lower case, letters/digits/`+`/`#` only, single spaces: the JS builder's `normal`.
pub fn normal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut space = false;
    for ch in value.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '+' || ch == '#' {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(ch);
        } else {
            space = true;
        }
    }
    out
}

fn cv_text(p: &Project) -> String {
    let mut parts: Vec<&str> = vec![&p.candidate.headline, &p.candidate.summary];
    for s in &p.skills {
        parts.extend([s.group.as_str(), s.name.as_str()]);
    }
    for j in &p.experience {
        parts.extend([j.title.as_str(), j.company.as_str()]);
        parts.extend(j.bullets.iter().map(String::as_str));
    }
    for e in &p.education {
        parts.extend([e.qualification.as_str(), e.institution.as_str()]);
    }
    for c in &p.certifications {
        parts.extend([c.name.as_str(), c.issuer.as_str()]);
    }
    parts.join("\n")
}

fn starts_word(haystack: &str, needle: &str, whole: bool) -> bool {
    let padded = format!(" {haystack} ");
    if whole { padded.contains(&format!(" {needle} ")) } else { padded.contains(&format!(" {needle}")) }
}

/// Whether the normalised CV text has the keyword (or one of its `/` alternatives).
/// Short keywords (≤ 3 characters) must match a whole word; longer ones may be inflected.
pub fn contains(haystack: &str, keyword: &str) -> bool {
    keyword.split(['/', '|']).map(normal).filter(|o| !o.is_empty()).any(|o| starts_word(haystack, &o, o.chars().count() <= 3))
}

pub fn keyword_match(p: &Project) -> KeywordMatch {
    let haystack = normal(&cv_text(p));
    let mut seen = HashSet::new();
    let keywords: Vec<String> = p.target.keywords.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty() && seen.insert(k.clone())).collect();
    let (found, missing): (Vec<String>, Vec<String>) = keywords.iter().cloned().partition(|k| contains(&haystack, k));
    let coverage = (!keywords.is_empty()).then(|| ((100.0 * found.len() as f64 / keywords.len() as f64).round()) as u32);
    KeywordMatch { found, missing, coverage }
}

const CLICHES: [&str; 22] = [
    "løsningsorientert", "resultatorientert", "teamplayer", "team player", "lagspiller", "hardworking", "hardtarbeidende", "arbeidsom",
    "strukturert og selvstendig", "selvgående", "motivert", "engasjert", "dynamisk", "dynamic", "detail-oriented", "detaljorientert",
    "go-getter", "synergi", "synergy", "think outside the box", "passionate", "brennende interessert",
];

static WEAK_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(jeg|i|my|min|mitt|mine|ansvarlig for|responsible for|duties included|arbeidsoppgaver|oppgaver)\b").expect("regex"));
static SENSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(født|fødselsdato|fødselsnummer|personnummer|sivilstand|sivilstatus|gift|ugift|samboer|religion|date of birth|born on|marital status|social security|nationality|nasjonalitet)\b").expect("regex")
});
/// A number counts as a result only when it measures something, not a version or model number.
static RESULT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^\p{L}\p{N}.\-])\d+(?:[.,]\d+)?\s?(?:%|prosent|percent|kr\b|nok\b|mnok|mill|million|timer|hours?|dager|days?|uker|weeks?|måneder|months?|år\b|years?|ganger|times|x\b|kunder|customers|anlegg|plants|systemer|systems|prosjekter|projects|personer|people|brukere|users|ansatte|employees|signaler|signals|tags|i/o|meter|km|tonn|tonnes|mw|kw)").expect("regex")
});

/// A bare year counts from January when a job starts and to December when it ends.
fn month_index(value: &str, end: bool) -> Option<i32> {
    match value.split_once('-') {
        Some((y, m)) => Some(y.parse::<i32>().ok()? * 12 + m.parse::<i32>().ok()? - 1),
        None => Some(value.parse::<i32>().ok()? * 12 + if end { 11 } else { 0 }),
    }
}

/// Gaps of more than six months between jobs: (after, before, months).
pub fn gaps(p: &Project, today_index: i32) -> Vec<(String, String, i32)> {
    let mut jobs: Vec<(i32, i32, &str)> = p
        .experience
        .iter()
        .filter_map(|j| {
            let start = month_index(&j.start, false)?;
            let end = match &j.end {
                None => today_index,
                Some(e) => month_index(e, true)?,
            };
            Some((start, end, j.company.as_str()))
        })
        .collect();
    jobs.sort_by_key(|j| j.0);
    let mut out = Vec::new();
    let mut reach: Option<(i32, &str)> = None;
    for (start, end, company) in jobs {
        if let Some((r, last)) = reach {
            if start - r > 6 {
                out.push((last.to_string(), company.to_string(), start - r));
            }
            if end > r {
                reach = Some((end, company));
            }
        } else {
            reach = Some((end, company));
        }
    }
    out
}

pub fn audit(p: &Project, pages: usize, today_index: i32) -> (Vec<Check>, KeywordMatch) {
    let mut checks = Vec::new();
    let mut check = |ok: bool, pass: String, fail: String, tip: String, level: Level| {
        checks.push(if ok { Check { level: Level::Pass, text: pass, tip: String::new() } } else { Check { level, text: fail, tip } });
    };
    let c = &p.candidate;
    let bullets: Vec<&str> = p.experience.iter().flat_map(|j| j.bullets.iter().map(String::as_str)).collect();
    let role = normal(&p.target.role);
    let headline = normal(&c.headline);
    let summary = c.summary.trim();
    check(
        !(c.name.is_empty() || c.email.is_empty() || c.phone.is_empty() || c.city.is_empty()),
        "Kontaktinformasjon står som vanlig tekst øverst".into(),
        "Kontaktinformasjon mangler".into(),
        "Fyll inn navn, e-post, telefon og by. Det står som vanlig tekst øverst i CV-en, ikke i topptekst eller bilde.".into(),
        Level::Warn,
    );
    if !c.phone.is_empty() {
        let has_code = c.phone.trim_start().starts_with('+') || c.phone.trim_start().starts_with("00");
        check(has_code, "Telefonnummeret har landskode".into(), "Telefonnummeret mangler landskode".into(), "Skriv for eksempel +47 412 34 567. Utenlandske arbeidsgivere og søknadssystemer trenger landskoden.".into(), Level::Info);
    }
    check(!c.linkedin.is_empty(), "LinkedIn-profil er oppgitt".into(), "LinkedIn-profil mangler".into(), "Rekrutterere sjekker nesten alltid LinkedIn. Legg inn full adresse, og hold profilen lik CV-en.".into(), Level::Info);
    if headline.is_empty() {
        check(false, String::new(), "Faglig overskrift mangler".into(), "Skriv en kort linje under navnet, gjerne med stillingens egne ord: for eksempel «Sykepleier | Akuttmottak | Legemiddelhåndtering».".into(), Level::Warn);
    } else {
        let role_words: Vec<&str> = role.split(' ').filter(|w| w.chars().count() > 3).collect();
        let stem_hit = role_words.iter().any(|w| {
            let n = w.chars().count();
            let keep = (n.saturating_sub(3)).max(4);
            headline.contains(&w.chars().take(keep).collect::<String>())
        });
        let ok = role.is_empty() || stem_hit || p.target.keywords.iter().any(|k| contains(&headline, k));
        check(ok, "Overskriften speiler stillingen".into(), "Overskriften bruker ikke stillingens ord".into(), "La overskriften bruke stillingens tittel eller to–tre fagord fra annonsen, uten å late som du har tittelen fra før.".into(), Level::Warn);
    }
    let count = summary.split_whitespace().count();
    check(
        (30..=70).contains(&count),
        format!("Profilen har {count} ord"),
        if count > 0 { format!("Profilen har {count} ord") } else { "Profiltekst mangler".into() },
        "Sikt mot 30–70 ord: hvem du er faglig, hva du kan best, og hva du vil bidra med i denne jobben.".into(),
        Level::Warn,
    );
    let m = keyword_match(p);
    match m.coverage {
        None => check(false, String::new(), "Ingen fagord fra annonsen er lagt inn".into(), "Lim inn annonsen under «Stillingen» og trykk «Foreslå fagord». Søknadssystemer og rekrutterere leter etter annonsens egne ord.".into(), Level::Info),
        Some(coverage) => {
            let total = m.found.len() + m.missing.len();
            check(
                coverage >= 70,
                format!("{} av {total} fagord fra annonsen finnes i CV-en", m.found.len()),
                format!("Bare {} av {total} fagord fra annonsen finnes i CV-en", m.found.len()),
                format!("Mangler: {}. Bruk annonsens ordlyd der du faktisk har erfaringen. Legg aldri inn ord du ikke kan stå for.", m.missing.join(", ")),
                Level::Warn,
            );
        }
    }
    let skills = p.skills.len();
    check(
        skills >= 6,
        format!("{skills} kompetanser i kompetanseseksjonen"),
        if skills > 0 { format!("Bare {skills} kompetanser") } else { "Ingen kompetanser lagt inn".into() },
        "En kort, gruppert kompetanseliste tidlig i CV-en gjør fagordene lette å finne. Sikt mot 6–20.".into(),
        Level::Warn,
    );
    if skills > 1 {
        check(p.skills.iter().all(|s| !s.group.trim().is_empty()), "Kompetansene er gruppert".into(), "Noen kompetanser mangler gruppe".into(), "Grupper dem, for eksempel «Fag», «Verktøy», «Språk og sertifikater». Grupperte linjer leses raskere.".into(), Level::Info);
    }
    let excerpt = |t: &str, n: usize| format!("«{}…»", t.chars().take(n).collect::<String>());
    if !bullets.is_empty() {
        let long: Vec<&str> = bullets.iter().copied().filter(|b| b.chars().count() > 220).collect();
        check(long.is_empty(), "Punktene er korte nok til å skannes".into(), format!("{} punkt er for lange", long.len()), format!("Hold hvert punkt til én–to linjer. Gjelder: {}", long.iter().take(2).map(|b| excerpt(b, 45)).collect::<Vec<_>>().join(", ")), Level::Warn);
        let weak: Vec<&str> = bullets.iter().copied().filter(|b| WEAK_START.is_match(b.trim())).collect();
        check(weak.is_empty(), "Punktene starter med handling".into(), format!("{} punkt starter med «Jeg» eller «Ansvarlig for»", weak.len()), format!("Start med et verb: «Ledet …», «Innførte …», «Behandlet …». Gjelder: {}", weak.iter().take(2).map(|b| excerpt(b, 40)).collect::<Vec<_>>().join(", ")), Level::Warn);
        let mut starts: BTreeMap<String, usize> = BTreeMap::new();
        for b in &bullets {
            if let Some(first) = normal(b).split(' ').next().filter(|w| !w.is_empty()) {
                *starts.entry(first.to_string()).or_default() += 1;
            }
        }
        let repeated: Vec<String> = starts.into_iter().filter(|(_, n)| *n > 3).map(|(k, _)| k).collect();
        check(repeated.is_empty(), "Variert språk i punktene".into(), "Mange punkter starter med samme ord".into(), format!("Varier verbene. Gjentatt: {}.", repeated.join(", ")), Level::Info);
        let jobs = p.experience.len();
        let quantified = p.experience.iter().filter(|j| j.bullets.iter().any(|b| RESULT.is_match(b))).count();
        let text = format!("{quantified} av {jobs} jobber viser et målbart resultat");
        check(quantified >= jobs.min(2), text.clone(), text, "Rekrutterere legger merke til resultater: «Oppnådde X, målt ved Y, ved å gjøre Z». Bruk bare tall du kan dokumentere.".into(), Level::Info);
    }
    let all = normal(&std::iter::once(summary).chain(bullets.iter().copied()).collect::<Vec<_>>().join(" "));
    let cliches: Vec<&str> = CLICHES.iter().copied().filter(|x| all.contains(&normal(x))).collect();
    check(cliches.is_empty(), "Ingen tomme moteord".into(), "Moteord som ikke beviser noe".into(), format!("Vis heller med eksempler enn å skrive: {}.", cliches.join(", ")), Level::Info);
    let personal = std::iter::once(summary).chain([c.headline.as_str()]).chain(bullets.iter().copied()).collect::<Vec<_>>().join(" ");
    check(!SENSITIVE.is_match(&personal), "Ingen unødvendige personopplysninger".into(), "Personopplysninger som ikke hører hjemme i en CV".into(), "Fjern fødselsdato, personnummer, sivilstand, religion og nasjonalitet.".into(), Level::Warn);
    let holes = gaps(p, today_index);
    check(
        holes.is_empty(),
        "Ingen uforklarte opphold over seks måneder".into(),
        "Opphold over seks måneder i tidslinjen".into(),
        format!("{}. Forklar kort, for eksempel studier, permisjon eller omstilling.", holes.iter().map(|(a, b, n)| format!("{n} måneder mellom {a} og {b}")).collect::<Vec<_>>().join("; ")),
        Level::Info,
    );
    if !p.experience.is_empty() {
        check(p.experience.iter().all(|j| !j.location.trim().is_empty()), "Arbeidssted er oppgitt for jobbene".into(), "Arbeidssted mangler for noen jobber".into(), "Oppgi by for hver jobb. Mange søknadssystemer sorterer på sted.".into(), Level::Info);
    }
    check(pages <= 2, format!("{pages} A4-side{}", if pages == 1 { "" } else { "r" }), format!("{pages} A4-sider; maks to"), "Kort ned eller velg utformingen Kompakt; legg prosjektliste i eget vedlegg.".into(), Level::Warn);
    checks.push(Check { level: Level::Pass, text: "Én tekstspalte, vanlige overskrifter og ekte tekst".into(), tip: String::new() });
    (checks, m)
}

// ---------- plain text for portals ----------

pub fn plain_text(p: &Project) -> String {
    let labels = text::labels(p);
    let c = &p.candidate;
    let mut lines: Vec<String> = vec![c.name.clone(), c.headline.clone(), [&c.email, &c.phone, &c.city, &c.linkedin].iter().filter(|s| !s.is_empty()).map(|s| s.as_str()).collect::<Vec<_>>().join(" | ")];
    let section = |lines: &mut Vec<String>, title: &str| {
        lines.push(String::new());
        lines.push(title.to_uppercase());
    };
    if !c.summary.is_empty() {
        section(&mut lines, labels.profile);
        lines.push(c.summary.clone());
    }
    if !p.skills.is_empty() {
        section(&mut lines, labels.skills);
        for (group, names) in text::group_skills(&p.skills) {
            lines.push(if group.is_empty() { names.join(", ") } else { format!("{group}: {}", names.join(", ")) });
        }
    }
    if !p.experience.is_empty() {
        section(&mut lines, labels.experience);
        for job in text::sort_jobs(&p.experience) {
            lines.push(String::new());
            lines.push(if job.location.is_empty() { format!("{} | {}", job.title, job.company) } else { format!("{} | {} | {}", job.title, job.company, job.location) });
            lines.push(text::period(job, labels));
            lines.extend(job.bullets.iter().map(|b| format!("- {b}")));
        }
    }
    if !p.education.is_empty() {
        section(&mut lines, labels.education);
        for e in text::sort_education(&p.education) {
            lines.push(format!("{} | {} | {}", e.qualification, e.institution, text::year_range(&e.start, &e.end)));
        }
    }
    if !p.certifications.is_empty() {
        section(&mut lines, labels.certifications);
        for x in text::sort_certifications(&p.certifications) {
            lines.push([&x.name, &x.issuer, &x.year].iter().filter(|s| !s.is_empty()).map(|s| s.as_str()).collect::<Vec<_>>().join(" | "));
        }
    }
    if !p.languages.is_empty() {
        section(&mut lines, labels.languages);
        lines.push(p.languages.iter().map(|l| format!("{}: {}", l.name, l.level)).collect::<Vec<_>>().join(" | "));
    }
    let mut out = lines.join("\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", out.trim())
}

// ---------- keyword suggestions from a job ad ----------

/// Technologies, acronyms and recurring long domain words from a job ad, strongest first.
/// Same rules as the HTML builder's `suggestKeywords`.
pub fn suggest_keywords(description: &str, limit: usize) -> Vec<String> {
    use crate::wordlists::{COMMON, DICTIONARY, STOP};
    static VERSIONED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-ZÆØÅ][A-Za-zÆØÅæøå]*)\s+(\d[\d.\-]*\d|\d)\b").expect("regex"));
    static TECH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[A-ZÆØÅ][A-Za-z0-9ÆØÅæøå]*[A-ZÆØÅ][A-Za-z0-9ÆØÅæøå]*|[A-Za-z]+(?:\+\+|#))$").expect("regex"));
    static CAPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-ZÆØÅ]{2,}|[a-zæøå][A-ZÆØÅ]").expect("regex"));
    let mut found: Vec<(String, String, f32)> = Vec::new(); // (key, term, weight)
    let mut add = |term: &str, weight: f32| {
        let clean = term.trim_matches(|c: char| !(c.is_alphanumeric() || c == '+' || c == '#')).trim_start_matches(|c: char| !c.is_alphanumeric()).trim().to_string();
        let key = normal(&clean);
        if clean.chars().count() < 2 || STOP.contains(&key.as_str()) || key.chars().all(|c| c.is_ascii_digit()) {
            return;
        }
        match found.iter_mut().find(|(k, _, _)| *k == key) {
            Some(entry) => entry.2 += weight,
            None => found.push((key, clean, weight)),
        }
    };
    for c in VERSIONED.captures_iter(description) {
        if CAPS.is_match(&c[1]) {
            add(&c[0], 3.0);
        }
    }
    for token in description.split(|c: char| !(c.is_alphanumeric() || c == '+' || c == '#')).filter(|t| !t.is_empty()) {
        if TECH.is_match(token) {
            add(token, 2.0);
        }
    }
    let lower = normal(description);
    let mut counts: Vec<(String, usize)> = Vec::new();
    for word in lower.split(' ') {
        if word.chars().count() >= 9 && word.chars().all(char::is_alphabetic) && !COMMON.contains(&word) {
            match counts.iter_mut().find(|(w, _)| w == word) {
                Some(entry) => entry.1 += 1,
                None => counts.push((word.to_string(), 1)),
            }
        }
    }
    // Long words count only when repeated or very specific (12+ letters).
    for (word, times) in counts {
        if times >= 2 || word.chars().count() >= 12 {
            add(&word, 0.5 + times as f32 * 0.5);
        }
    }
    for term in DICTIONARY {
        if starts_word(&lower, &normal(term), true) {
            add(term, 1.0);
        }
    }
    let keys: Vec<String> = found.iter().map(|(k, _, _)| k.clone()).collect();
    let redundant = |key: &str| {
        let parts: Vec<&str> = key.split(' ').collect();
        if parts.len() == 2 && parts[1].len() == 1 && parts[1].chars().all(|c| c.is_ascii_digit()) && keys.iter().any(|k| k == parts[0]) {
            return true;
        }
        key.chars().count() <= 3 && keys.iter().any(|k| k != key && k.starts_with(&format!("{key} ")))
    };
    let mut terms: Vec<(String, String, f32)> = found.iter().filter(|(k, _, _)| !redundant(k)).cloned().collect();
    terms.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    terms.into_iter().take(limit).map(|(_, term, _)| term).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn suggests_technologies_and_domain_words() {
        let terms = super::suggest_keywords("Erfaring med TwinCAT 3 eller CODESYS, IEC 61131-3, C/C++, JSON og feilsøking. Vi søker sykepleier med legemiddelhåndtering. Legemiddelhåndtering er viktig.", 25);
        for t in ["TwinCAT", "CODESYS", "IEC 61131-3", "C++", "JSON", "feilsøking", "legemiddelhåndtering"] {
            assert!(terms.iter().any(|x| x == t), "{t} missing from {terms:?}");
        }
        assert!(!terms.iter().any(|x| x == "IEC"));
    }
}
