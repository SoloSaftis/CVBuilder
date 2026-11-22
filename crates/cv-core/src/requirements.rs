//! Must-haves and nice-to-haves in a job ad, requirement by requirement (to-do 045).
//!
//! A keyword list treats «Fagbrev som elektriker (krav)» and «Power BI er en fordel» the same.
//! This reads the ad's own structure instead, offline: headings such as «Kvalifikasjoner»,
//! «Requirements» or «Ønskelig» open a block, and cue phrases in a line («er en fordel»,
//! «is a plus», «må ha», «required») decide its kind on their own. Each requirement keeps the
//! ad's own words and a few key terms, and [`backing`] says where the CV shows each term.
//!
//! Cue words cover Norwegian (bokmål and nynorsk) and English in depth, and Swedish, Danish,
//! German, Dutch, French, Spanish and Finnish with the common headings and phrases. An ad with
//! no structure gives no requirements, and the keyword pills stay the whole answer.
use std::sync::LazyLock;

use regex::Regex;

use crate::Project;
use crate::checks::{self, contains, normal};
use crate::import::{ad_keywords, clean_paste};

/// How much the ad needs a requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Need {
    /// «Krav», «Requirements», «må ha», «required».
    Must,
    /// «Ønskelig», «Nice to have», «er en fordel», «is a plus».
    Nice,
}

/// One requirement from the ad, in the ad's own words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub text: String,
    pub need: Need,
    /// Key terms in the line that a CV can show («Python», «fagbrev», «norsk/norwegian»).
    pub terms: Vec<String>,
}

/// What a heading opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    Must,
    Nice,
    /// A subheading such as «Utdanning» or «Experience»: keeps the block it is in, or opens a
    /// must block when it comes first.
    Topic,
    /// Tasks, what the employer offers, about us, contact …: no requirements here.
    Other,
}

/// Headings that open a block of must-haves. Matched whole (after «:»), or as the start of a
/// heading that ends in «:».
const MUST_HEADINGS: &[&str] = &[
    // Norwegian (bokmål and nynorsk)
    "krav", "krav til deg", "krav til stillingen", "krav til kompetanse", "krav til kvalifikasjoner", "kvalifikasjonskrav", "kvalifikasjoner", "kvalifikasjonar",
    "dine kvalifikasjoner", "kvalifikasjoner og egenskaper", "kvalifikasjoner og erfaring", "nødvendige kvalifikasjoner", "vi ser etter", "vi ser etter deg som",
    "vi ser etter deg som har", "vi søker deg som", "vi søker deg som har", "dette ser vi etter", "hvem er du", "hvem du er", "hvem vi ser etter", "om deg", "du har",
    "du må ha", "vi forventer", "vi forventer at du", "vi forventer at du har", "vi trenger deg som", "kompetanse", "din kompetanse", "din bakgrunn", "kven er du",
    "vi ser etter deg som har", "du må", "må ha", "minimumskrav",
    // English
    "requirements", "requirement", "key requirements", "job requirements", "minimum requirements", "qualifications", "required qualifications", "minimum qualifications",
    "basic qualifications", "essential qualifications", "your qualifications", "must have", "must haves", "must-have", "must-haves", "what you bring", "what you'll bring",
    "what you will bring", "what we're looking for", "what we are looking for", "who we're looking for", "who we are looking for", "who you are", "about you", "your profile",
    "you have", "you are", "we are looking for", "we're looking for", "we expect", "skills and experience", "your skills", "requirements and qualifications",
    "what you'll need", "what you need", "what you need to succeed", "essential", "essential criteria", "essential skills", "you will have", "you should have", "profile",
    // Swedish, Danish
    "krav", "kvalifikationer", "kvalifikasjoner", "vi söker dig som", "vi söker dig som har", "vem är du", "om dig", "din profil", "du har", "vi ser gerne at du",
    "vi forventer at du", "din baggrund", "kompetenser", "kompetencer", "vi letar efter", "vi leder efter", "erfarenhet och kompetens",
    // German, Dutch
    "anforderungen", "ihr profil", "dein profil", "das bringen sie mit", "das bringst du mit", "was sie mitbringen", "was du mitbringst", "voraussetzungen", "qualifikationen",
    "functie-eisen", "functie eisen", "vereisten", "wat vragen wij", "wat we vragen", "wat breng je mee", "jouw profiel", "wie ben jij", "wie zoeken wij",
    // French, Spanish, Finnish
    "profil recherché", "exigences", "qualifications requises", "votre profil", "compétences requises", "requisitos", "requisitos mínimos", "perfil", "tu perfil",
    "lo que buscamos", "qué buscamos", "vaatimukset", "edellytämme", "odotamme sinulta", "toivomme sinulta", "sinulla on",
];

/// Headings that open a block of nice-to-haves.
const NICE_HEADINGS: &[&str] = &[
    "ønskelig", "ønskelige kvalifikasjoner", "ønskede kvalifikasjoner", "ønskjeleg", "ønskjelege kvalifikasjonar", "det er en fordel", "det er en fordel om du har",
    "det er ein fordel", "fordel", "fordeler", "fordelaktig", "det er et pluss", "et pluss", "pluss", "det er også fint om du har", "gjerne også", "i tillegg er det fint om du har",
    "nice to have", "nice-to-have", "nice to haves", "nice-to-haves", "bonus", "bonus points", "bonus if you have", "preferred qualifications", "preferred", "desirable",
    "desirable criteria", "desirable skills", "it's a plus", "it is a plus", "a plus", "plus", "extra credit", "good to have", "advantageous", "would be great",
    "it would be great if you have", "even better if you have", "meriterande", "meriterande kvalifikationer", "det är meriterande", "meriterande om du har", "önskvärt",
    "ønskeligt", "det er en fordel hvis du har", "det er et plus", "wünschenswert", "von vorteil", "nice to have", "pluspunten", "een pré", "atouts", "serait un plus",
    "se valorará", "valorable", "deseable", "katsotaan eduksi", "eduksi katsotaan", "arvostamme",
];

/// Headings that close the requirement blocks.
const OTHER_HEADINGS: &[&str] = &[
    "arbeidsoppgaver", "arbeidsoppgåver", "oppgaver", "oppgåver", "dine oppgaver", "dine arbeidsoppgaver", "ansvarsområder", "ansvar og arbeidsoppgaver", "om stillingen",
    "om jobben", "om rollen", "stillingen", "rollen", "vi tilbyr", "hva vi tilbyr", "vi kan tilby", "vi kan by på", "dette tilbyr vi", "vi tilbyr deg", "vi tilbyd",
    "personlige egenskaper", "egenskaper", "personlege eigenskapar", "om oss", "om bedriften", "om arbeidsgiver", "om arbeidsgiveren", "om selskapet", "om virksomheten",
    "søknad", "søknaden", "slik søker du", "søk", "kontakt", "kontaktinformasjon", "kontaktpersoner", "spørsmål om stillingen", "praktisk informasjon", "annen informasjon",
    "andre opplysninger", "nøkkelinformasjon", "lønn", "lønn og betingelser", "responsibilities", "key responsibilities", "your responsibilities", "your tasks", "tasks",
    "what you will do", "what you'll do", "what you will be doing", "what you'll be doing", "your role", "the role", "about the role", "about the job", "about the position",
    "job description", "about us", "about the company", "who we are", "we offer", "what we offer", "what we can offer", "what's in it for you", "benefits", "perks",
    "how to apply", "application", "contact", "personal qualities", "personal attributes", "personal skills", "key information", "salary", "arbetsuppgifter",
    "dina arbetsuppgifter", "vi erbjuder", "om oss", "om tjänsten", "arbejdsopgaver", "dine opgaver", "vi tilbyder", "personliga egenskaper", "personlige kompetencer",
    "ihre aufgaben", "deine aufgaben", "aufgaben", "wir bieten", "das bieten wir", "über uns", "taken", "jouw taken", "wat bieden wij", "wij bieden", "over ons",
    "missions", "vos missions", "nous offrons", "funciones", "ofrecemos", "tehtävät", "tarjoamme",
];

/// Subheadings inside a requirement block.
const TOPIC_HEADINGS: &[&str] = &[
    "utdanning", "utdannelse", "utdanning og erfaring", "erfaring", "erfaring og kompetanse", "språk", "sertifiseringer", "sertifikater", "førerkort", "education", "experience",
    "skills", "technical skills", "languages", "certifications", "certificates", "utbildning", "erfarenhet", "uddannelse", "ausbildung", "erfahrung", "opleiding", "ervaring",
];

/// Phrases in a line that make it a nice-to-have, whatever block it is in.
const NICE_CUES: &[&str] = &[
    "er en fordel", "er ein fordel", "en fordel", "ein fordel", "fordelaktig", "er et pluss", "et pluss", "ønskelig", "ønskjeleg", "ønskes", "fortrinnsvis", "det er fint om",
    "er positivt", "vil bli vektlagt", "vektlegges", "is a plus", "a plus", "is an advantage", "an advantage", "advantageous", "nice to have", "nice-to-have", "preferred",
    "preferably", "ideally", "desirable", "is a bonus", "bonus if", "would be a bonus", "would be great", "good to have", "is beneficial", "beneficial", "meriterande",
    "meriterar", "är ett plus", "önskvärt", "ønskeligt", "er et plus", "von vorteil", "wünschenswert", "idealerweise", "wäre schön", "ein plus", "is een pré", "een pré",
    "pluspunt", "est un plus", "serait un plus", "un atout", "souhaité", "souhaitée", "se valorará", "valorable", "deseable", "katsotaan eduksi", "eduksi", "plussaa",
];

/// Words or phrases in a line that make it a must-have, whatever block it is in.
const MUST_CUES: &[&str] = &[
    "krav", "et krav", "er et krav", "er påkrevd", "påkrevd", "påkravd", "kreves", "krevst", "må ha", "må kunne", "må beherske", "må være", "nødvendig", "er nødvendig",
    "required", "is required", "are required", "requirement", "must have", "must", "mandatory", "essential", "is essential", "minimum", "at least", "krävs", "ett krav",
    "måste", "kræves", "påkrævet", "skal have", "skal kunne", "erforderlich", "zwingend", "vorausgesetzt", "voraussetzung", "notwendig", "vereist", "verplicht", "requis",
    "requise", "obligatoire", "indispensable", "imprescindible", "obligatorio", "requerido", "vaaditaan", "edellytetään", "edellytys",
];

/// Spoken languages ads ask for, with the CV names that show them («Norsk», «Norwegian»).
const LANGUAGES: &[(&str, &str)] = &[
    ("norsk", "norsk/norwegian/norska/norsch"),
    ("norwegian", "norsk/norwegian"),
    ("skandinavisk", "norsk/svensk/dansk/norwegian/swedish/danish/skandinavisk/scandinavian"),
    ("scandinavian", "norsk/svensk/dansk/norwegian/swedish/danish/skandinavisk/scandinavian"),
    ("engelsk", "engelsk/english/englisch/engelska"),
    ("english", "engelsk/english/englisch/engelska"),
    ("svensk", "svensk/swedish/svenska"),
    ("swedish", "svensk/swedish/svenska"),
    ("dansk", "dansk/danish"),
    ("danish", "dansk/danish"),
    ("tysk", "tysk/german/deutsch/tyska"),
    ("german", "tysk/german/deutsch"),
    ("fransk", "fransk/french/français/franska"),
    ("french", "fransk/french/français"),
    ("spansk", "spansk/spanish/español/spanska"),
    ("spanish", "spansk/spanish/español"),
    ("finsk", "finsk/finnish/suomi/finska"),
    ("finnish", "finsk/finnish/suomi"),
    ("polsk", "polsk/polish/polski"),
    ("polish", "polsk/polish/polski"),
];

/// Words in front of a term that say how well, not what: «Strong Python», «gode Excel».
const FILLERS: &[&str] = &[
    "strong", "solid", "good", "excellent", "fluent", "proven", "deep", "hands-on", "basic", "advanced", "practical", "relevant", "very", "god", "gode", "godt", "solid",
    "inngående", "grunnleggende", "dokumentert", "praktisk", "relevant", "relevante", "svært", "meget", "goda", "gedegen", "fundierte", "sehr", "gute",
];

/// The phrase found as whole words in the normalised line.
fn has_phrase(norm: &str, phrase: &str) -> bool {
    let p = normal(phrase);
    !p.is_empty() && format!(" {norm} ").contains(&format!(" {p} "))
}

/// The kind of a heading line, if it is one. `norm` is the line normalised, `ends_colon` says
/// whether it ended in «:».
fn heading(line: &str) -> Option<Block> {
    let raw = line.trim().trim_matches(|c: char| "#*_=".contains(c)).trim();
    let colon = raw.ends_with([':', '：']);
    // «Kvalifikasjoner (krav)», «Ønskelig (ikke krav)»: the brackets say more than the word.
    let bracket = raw.find('(').filter(|_| raw.trim_end_matches([':', '：', ' ']).ends_with(')')).map(|i| raw[i..].to_string());
    let bare = if bracket.is_some() { raw.split('(').next().unwrap_or(raw) } else { raw };
    let bare = bare.trim_end_matches([':', '：', ' ']);
    let norm = normal(bare);
    if norm.is_empty() || norm.split(' ').count() > 9 || raw.chars().count() > 80 {
        return None;
    }
    let lists: [(&[&str], Block); 4] = [(MUST_HEADINGS, Block::Must), (NICE_HEADINGS, Block::Nice), (OTHER_HEADINGS, Block::Other), (TOPIC_HEADINGS, Block::Topic)];
    let mut best: Option<(usize, Block)> = None;
    for (list, block) in lists {
        for h in list {
            let h = normal(h);
            let whole = norm == h;
            // «Kvalifikasjoner og personlige egenskaper:» starts a block too, but only with
            // the colon: «Krav om førerkort klasse B» is a requirement, not a heading.
            let start = colon && norm.starts_with(&format!("{h} "));
            if (whole || start) && best.is_none_or(|(len, _)| h.len() > len) {
                best = Some((h.len(), block));
            }
        }
    }
    let mut block = best.map(|(_, b)| b)?;
    if let Some(b) = bracket {
        let b = normal(&b);
        let not_required = ["ikke krav", "ikkje krav", "not required", "optional", "valgfritt"].iter().any(|c| has_phrase(&b, c));
        if not_required || NICE_CUES.iter().chain(NICE_HEADINGS).any(|c| has_phrase(&b, c)) {
            block = Block::Nice;
        } else if has_phrase(&b, "krav") || has_phrase(&b, "required") || has_phrase(&b, "must") {
            block = Block::Must;
        }
    }
    Some(block)
}

/// The need a line says it has itself, if any. Nice cues win: «Erfaring med X er en fordel,
/// men ikke et krav».
fn cue(norm: &str) -> Option<Need> {
    if NICE_CUES.iter().any(|c| has_phrase(norm, c)) {
        return Some(Need::Nice);
    }
    if has_phrase(norm, "ikke et krav") || has_phrase(norm, "not required") || has_phrase(norm, "not a requirement") {
        return Some(Need::Nice);
    }
    MUST_CUES.iter().any(|c| has_phrase(norm, c)).then_some(Need::Must)
}

/// A line without its list marker.
fn unbullet(line: &str) -> (bool, &str) {
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:\d{1,2}[.)]|[a-h]\))\s+").expect("regex"));
    let trimmed = line.trim();
    let l = trimmed.trim_start_matches(|c: char| !(c.is_alphanumeric() || "#.(«\"'“".contains(c))).trim_start();
    let l = NUMBER.find(l).map_or(l, |m| &l[m.end()..]);
    (l.len() != trimmed.len(), l)
}

/// The sentences of a line: a requirement line may hold two («Fagbrev er et krav. Erfaring fra
/// industri er en fordel.»). Dots in «C#.NET», «Node.js» and «f.eks.» do not split.
fn sentences(line: &str) -> Vec<String> {
    static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.;!?]\s+(?:\p{Lu}|«|\()|;\s*").expect("regex"));
    let mut out = Vec::new();
    let mut from = 0;
    for m in SPLIT.find_iter(line) {
        let piece = &line[from..m.start() + 1];
        // «f.eks. Python» and «bl.a. SQL» are not sentence ends.
        let last = piece.rsplit(' ').next().unwrap_or_default().to_lowercase();
        if ["f.eks.", "bl.a.", "e.g.", "i.e.", "etc.", "evt.", "inkl.", "ca.", "z.b.", "t.ex.", "mv."].contains(&last.as_str()) {
            continue;
        }
        out.push(piece.trim_end_matches([';', ' ']).to_string());
        from = m.start() + 1;
    }
    out.push(line[from..].to_string());
    out.into_iter().map(|s| s.trim().trim_end_matches([';', ',', '.', '!']).trim().to_string()).filter(|s| s.chars().filter(|c| c.is_alphabetic()).count() >= 3).collect()
}

/// The CV names for a language word in an ad: «norskkunnskaper» → «norsk/norwegian/…».
fn language_of(word: &str) -> Option<&'static str> {
    const ENDS: &[&str] = &["", "kunnskaper", "kunnskap", "kunnskapar", "ferdigheter", "språklig", "talende", "kunskaper", "kundskaber", "kenntnisse"];
    LANGUAGES.iter().find(|(name, _)| word.strip_prefix(name).is_some_and(|end| ENDS.contains(&end))).map(|(_, shows)| *shows)
}

/// The key terms of one requirement: the ad's key words in it, and the languages it names
/// («gode norskkunnskaper» → «norsk/norwegian»).
pub fn terms(text: &str) -> Vec<String> {
    let norm = normal(text);
    let mut out: Vec<String> = Vec::new();
    for word in norm.split(' ') {
        if let Some(shows) = language_of(word) {
            if !out.iter().any(|o| o == shows) {
                out.push(shows.to_string());
            }
        }
    }
    for k in ad_keywords(text, "", 6) {
        // «Strong Python», «Solid SQL»: the CV shows the tool, not the adjective.
        let k = match k.split_once(' ') {
            Some((first, rest)) if FILLERS.contains(&first.to_lowercase().as_str()) => rest.to_string(),
            _ => k,
        };
        let key = normal(&k);
        // The language words themselves came in above.
        if LANGUAGES.iter().any(|(n, _)| key == *n || (key.starts_with(n) && key.len() > n.len() + 4)) {
            continue;
        }
        // A cue word is not something the CV can show.
        if MUST_CUES.iter().chain(NICE_CUES).any(|c| normal(c) == key) {
            continue;
        }
        if !out.iter().any(|o| normal(o) == key) {
            out.push(k);
        }
    }
    // What the key-word finder leaves (it is tuned for whole ads): runs of content words,
    // «data engineer», «Terraform», «offentlig sektor».
    for run in content_runs(text) {
        let key = normal(&run);
        let covered = out.iter().any(|o| o.split('/').any(|alt| {
            let a = normal(alt);
            format!(" {a} ").contains(&format!(" {key} ")) || format!(" {key} ").contains(&format!(" {a} "))
        }));
        if !covered {
            out.push(run);
        }
    }
    // «Apache Spark or Databricks»: either one shows it, so they are one term.
    let norm = normal(text);
    let mut i = 0;
    while i + 1 < out.len() {
        let (a, b) = (normal(&out[i]), normal(&out[i + 1]));
        let either = !out[i].contains('/') && !out[i + 1].contains('/') && ["or", "eller", "elle", "eller", "oder", "of", "ou", "o", "tai", "alternativt"].iter().any(|w| norm.contains(&format!("{a} {w} {b}")));
        if either {
            let joined = format!("{}/{}", out[i], out[i + 1]);
            out[i] = joined;
            out.remove(i + 1);
        } else {
            i += 1;
        }
    }
    out.truncate(6);
    out
}

/// A term as the ad writes it: «english» for «engelsk/english/…» in an English ad, both names
/// for «Apache Spark/Databricks».
pub fn label(requirement: &str, term: &str) -> String {
    if !LANGUAGES.iter().any(|(_, shows)| *shows == term) {
        return term.replace('/', " / ");
    }
    let norm = normal(requirement);
    let alt = term.split('/').find(|alt| norm.split(' ').any(|w| w.starts_with(alt))).unwrap_or_else(|| term.split('/').next().unwrap_or(term));
    let mut chars = alt.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// Words that carry no requirement of their own: little words, «erfaring», «years», levels.
const PLAIN: &[&str] = &[
    // Norwegian
    "og", "eller", "samt", "med", "fra", "til", "for", "som", "på", "i", "av", "om", "innen", "innenfor", "ved", "hos", "en", "et", "ei", "ein", "eit", "den", "det",
    "de", "du", "deg", "dine", "din", "ditt", "vi", "oss", "vår", "har", "ha", "hatt", "er", "være", "vært", "kan", "kunne", "skal", "vil", "må", "bør", "gjerne",
    "også", "minst", "minimum", "år", "års", "årig", "erfaring", "erfaringer", "erfaren", "kjennskap", "kunnskap", "kunnskaper", "kompetanse", "forståelse", "evne",
    "evner", "god", "gode", "godt", "meget", "svært", "solid", "inngående", "grunnleggende", "dokumentert", "relevant", "relevante", "tilsvarende", "lignende",
    "liknende", "annen", "andre", "aktuell", "aktuelle", "fordel", "pluss", "krav", "ønskelig", "nødvendig", "muntlig", "skriftlig", "flytende", "jobbet", "jobbe",
    "arbeid", "arbeidet", "arbeide", "jobb", "utdanning", "utdannelse", "fagbrev", "bakgrunn", "klasse", "type", "nivå", "f", "eks", "bl", "a", "ca", "etc", "mv",
    "sitt", "sin", "sine", "seg", "deres", "slik", "hvor", "når", "der", "mye", "mer", "flere", "fleire", "ulike", "både", "bruk", "bruke", "være", "veldig",
    "personlig", "egnethet", "vektlagt", "vektlegges", "kreves", "påkrevd", "ikke", "ikkje", "om du har",
    // English
    "and", "or", "with", "from", "to", "for", "as", "a", "an", "the", "in", "of", "on", "at", "by", "you", "your", "we", "our", "have", "has", "having", "had", "is",
    "are", "be", "been", "will", "can", "should", "must", "plus", "years", "year", "yrs", "experience", "experienced", "knowledge", "understanding", "skills", "skill",
    "ability", "strong", "solid", "good", "great", "excellent", "fluent", "proven", "deep", "hands", "on", "basic", "advanced", "practical", "relevant", "similar",
    "related", "equivalent", "other", "degree", "written", "spoken", "verbal", "working", "work", "worked", "using", "use", "used", "within", "least", "minimum",
    "required", "preferred", "preferably", "ideally", "nice", "bonus", "advantage", "desirable", "familiarity", "familiar", "exposure", "background", "level", "e",
    "g", "i", "etc", "such", "like", "including", "both", "any", "some", "more", "least", "is a plus", "plus", "it", "this", "that", "these", "those", "who", "which",
    // Swedish, Danish, German
    "och", "av", "inom", "erfarenhet", "kunskaper", "goda", "god", "meriterande", "og", "erfaring", "kendskab", "und", "oder", "mit", "von", "in", "im", "der", "die",
    "das", "ein", "eine", "als", "erfahrung", "kenntnisse", "gute", "sehr", "vorteil", "abgeschlossene", "ist",
];

/// Runs of content words in a requirement, each run at most three words; longer runs are
/// split into single words.
fn content_runs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut run: Vec<String> = Vec::new();
    let flush = |run: &mut Vec<String>, out: &mut Vec<String>| {
        if run.len() <= 3 {
            if !run.is_empty() {
                out.push(run.join(" "));
            }
        } else {
            out.append(run);
        }
        run.clear();
    };
    for raw in text.split(|c: char| c.is_whitespace() || ",;:()«»\"“”/".contains(c)) {
        let word = raw.trim_matches(|c: char| !(c.is_alphanumeric() || c == '#' || c == '+'));
        // «bachelornivå» is shown by «Bachelor i økonomi».
        let word = word.strip_suffix("nivå").filter(|w| w.chars().count() >= 4).unwrap_or(word);
        let key = normal(word);
        let plain = key.is_empty()
            || key.split(' ').all(|w| PLAIN.contains(&w))
            || language_of(&key).is_some()
            || key.chars().all(|c| c.is_ascii_digit() || c == '+')
            || (key.chars().count() < 2)
            || MUST_CUES.iter().chain(NICE_CUES).any(|c| normal(c) == key);
        if plain {
            flush(&mut run, &mut out);
        } else {
            run.push(word.to_string());
        }
    }
    flush(&mut run, &mut out);
    out.retain(|r| r.chars().count() >= 3 || r.chars().any(|c| c.is_uppercase()));
    out
}

/// The requirements in an ad, in the ad's order, each once. Empty when the ad has no headings
/// or cue phrases the reader knows.
pub fn ad_requirements(ad: &str) -> Vec<Requirement> {
    let clean = clean_paste(ad);
    let mut block: Option<Block> = None;
    let mut out: Vec<Requirement> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in clean.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (bulleted, body) = unbullet(line);
        // «Krav: fagbrev som elektriker» – a heading and its first point on one line.
        let mut rest = body;
        if !bulleted {
            if let Some(b) = heading(body) {
                block = Some(match (b, block) {
                    (Block::Topic, Some(Block::Must | Block::Nice)) => block.unwrap_or(Block::Must),
                    (Block::Topic, _) => Block::Must,
                    (b, _) => b,
                });
                continue;
            }
            if let Some((head, tail)) = body.split_once([':', '：']) {
                if !tail.trim().is_empty() && head.chars().count() <= 40 {
                    if let Some(b) = heading(&format!("{head}:")) {
                        if b != Block::Topic {
                            block = Some(b);
                        }
                        rest = tail.trim();
                    }
                }
            }
        }
        // A lead-in such as «Vi ser etter deg som har:» opens nothing new and is no requirement.
        if rest.ends_with([':', '：']) {
            continue;
        }
        for sentence in sentences(rest) {
            let norm = normal(&sentence);
            let words = norm.split(' ').count();
            let need = match (cue(&norm), block) {
                (_, Some(Block::Other)) => None,
                (Some(n), _) => Some(n),
                (None, Some(Block::Must)) => Some(Need::Must),
                (None, Some(Block::Nice)) => Some(Need::Nice),
                _ => None,
            };
            // Outside a block, only a sentence that says «krav» or «fordel» counts; a long
            // paragraph in a block is prose, not a requirement.
            let Some(need) = need else { continue };
            if words > 45 || (block.is_none() && words > 30) {
                continue;
            }
            // «Søknadsfrist» and «Kontakt» lines have their own labels; «Vi tilbyr» prose
            // without a heading still has no cue, so it never gets here.
            if !seen.insert(norm.clone()) {
                continue;
            }
            let terms = terms(&sentence);
            out.push(Requirement { text: sentence, need, terms });
            if out.len() >= 40 {
                return out;
            }
        }
    }
    out
}

/// Where the CV shows a term.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A job, by title and employer.
    Job(String),
    Education(String),
    Certification(String),
    Skills,
    Profile,
    Languages,
    /// A section the person made, by its title.
    Section(String),
}

/// How well the CV backs one requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backing {
    pub found: Vec<String>,
    pub missing: Vec<String>,
    /// Where the found terms are, each place once, jobs first.
    pub places: Vec<Place>,
}

impl Backing {
    /// Every term is in the CV.
    pub fn shown(&self) -> bool {
        !self.found.is_empty() && self.missing.is_empty()
    }

    /// The terms are only in the skills list or the profile: nothing the person did backs them.
    pub fn only_listed(&self) -> bool {
        !self.places.is_empty() && self.places.iter().all(|p| matches!(p, Place::Skills | Place::Profile))
    }
}

/// Where the CV shows each of the requirement's terms.
pub fn backing(p: &Project, r: &Requirement) -> Backing {
    // Only what is in this copy of the CV backs a requirement (hidden items are left out).
    let p = &*p.visible();
    let haystack = normal(&checks::cv_text(p));
    let mut found = Vec::new();
    let mut missing = Vec::new();
    let mut places: Vec<Place> = Vec::new();
    let add = |place: Place, places: &mut Vec<Place>| {
        if !places.contains(&place) {
            places.push(place);
        }
    };
    for term in &r.terms {
        if !contains(&haystack, term) {
            missing.push(term.clone());
            continue;
        }
        found.push(term.clone());
        let has = |text: &str| contains(&normal(text), term);
        for j in &p.experience {
            if has(&j.title) || j.bullets.iter().any(|b| has(b)) || has(&j.company) {
                let name = [j.title.trim(), j.company.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                add(Place::Job(name), &mut places);
            }
        }
        for e in &p.education {
            if has(&e.qualification) || has(&e.institution) {
                add(Place::Education(e.qualification.trim().to_string()), &mut places);
            }
        }
        for c in &p.certifications {
            if has(&c.name) || has(&c.issuer) {
                add(Place::Certification(c.name.trim().to_string()), &mut places);
            }
        }
        for s in &p.custom_sections {
            if s.items.iter().any(|i| has(&i.title) || has(&i.subtitle) || i.bullets.iter().any(|b| has(b))) {
                add(Place::Section(s.title.trim().to_string()), &mut places);
            }
        }
        if p.languages.iter().any(|l| has(&l.name)) {
            add(Place::Languages, &mut places);
        }
        if p.skills.iter().any(|s| has(&s.name) || has(&s.group)) {
            add(Place::Skills, &mut places);
        }
        if has(&p.candidate.headline) || has(&p.candidate.summary) {
            add(Place::Profile, &mut places);
        }
    }
    // Jobs and what was done first, the lists after: that is the evidence a recruiter trusts.
    places.sort_by_key(|p| match p {
        Place::Job(_) => 0,
        Place::Section(_) | Place::Certification(_) | Place::Education(_) => 1,
        Place::Languages => 2,
        Place::Skills => 3,
        Place::Profile => 4,
    });
    Backing { found, missing, places }
}

/// Counts for the summary line: (must shown, must judged, nice shown, nice judged). A
/// requirement with no terms cannot be judged and is left out.
pub fn summary(p: &Project, list: &[Requirement]) -> (usize, usize, usize, usize) {
    let mut c = (0, 0, 0, 0);
    for r in list.iter().filter(|r| !r.terms.is_empty()) {
        let shown = backing(p, r).shown();
        match r.need {
            Need::Must => {
                c.1 += 1;
                c.0 += usize::from(shown);
            }
            Need::Nice => {
                c.3 += 1;
                c.2 += usize::from(shown);
            }
        }
    }
    c
}
