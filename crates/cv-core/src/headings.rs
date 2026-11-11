//! The CV's headings in any language. Norwegian and English are built in (see `text`); a CV in
//! another language carries its own headings, from the table below or typed by the person or
//! written by the AI. The text of the CV itself is translated by the AI or the person.
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use crate::text::Labels;

/// A CV's headings in a language other than Norwegian and English.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Headings {
    /// The language's own name, e.g. «Deutsch».
    pub language: String,
    pub profile: String,
    pub skills: String,
    pub experience: String,
    pub education: String,
    pub certifications: String,
    pub languages: String,
    pub interests: String,
    pub contact: String,
    /// The word for a current job's end, e.g. «heute».
    pub present: String,
}

/// Headings shipped for common languages: (the language's own name, headings in order profile,
/// skills, experience, education, certifications, languages, interests, contact, present).
const TABLE: [(&str, [&str; 9]); 25] = [
    ("Dansk", ["Profil", "Kernekompetencer", "Erhvervserfaring", "Uddannelse", "Kurser og certificeringer", "Sprog", "Interesser", "Kontakt", "nu"]),
    ("Svenska", ["Profil", "Kärnkompetens", "Arbetslivserfarenhet", "Utbildning", "Kurser och certifieringar", "Språk", "Intressen", "Kontakt", "nu"]),
    ("Nynorsk", ["Profil", "Kjernekompetanse", "Arbeidserfaring", "Utdanning", "Kurs og sertifiseringar", "Språk", "Interesser", "Kontakt", "no"]),
    ("Íslenska", ["Prófíll", "Kjarnahæfni", "Starfsreynsla", "Menntun", "Námskeið og vottanir", "Tungumál", "Áhugamál", "Tengiliður", "núna"]),
    ("Suomi", ["Profiili", "Ydinosaaminen", "Työkokemus", "Koulutus", "Kurssit ja sertifikaatit", "Kielitaito", "Kiinnostuksen kohteet", "Yhteystiedot", "nyt"]),
    ("Deutsch", ["Profil", "Kernkompetenzen", "Berufserfahrung", "Ausbildung", "Weiterbildungen und Zertifikate", "Sprachen", "Interessen", "Kontakt", "heute"]),
    ("Nederlands", ["Profiel", "Kerncompetenties", "Werkervaring", "Opleiding", "Cursussen en certificaten", "Talen", "Interesses", "Contact", "heden"]),
    ("Français", ["Profil", "Compétences clés", "Expérience professionnelle", "Formation", "Formations et certifications", "Langues", "Centres d'intérêt", "Contact", "aujourd'hui"]),
    ("Español", ["Perfil", "Competencias clave", "Experiencia profesional", "Formación", "Cursos y certificaciones", "Idiomas", "Intereses", "Contacto", "actualidad"]),
    ("Português", ["Perfil", "Competências principais", "Experiência profissional", "Formação", "Cursos e certificações", "Idiomas", "Interesses", "Contacto", "presente"]),
    ("Italiano", ["Profilo", "Competenze chiave", "Esperienza professionale", "Istruzione", "Corsi e certificazioni", "Lingue", "Interessi", "Contatti", "oggi"]),
    ("Polski", ["Profil", "Kluczowe kompetencje", "Doświadczenie zawodowe", "Wykształcenie", "Kursy i certyfikaty", "Języki", "Zainteresowania", "Kontakt", "obecnie"]),
    ("Čeština", ["Profil", "Klíčové kompetence", "Pracovní zkušenosti", "Vzdělání", "Kurzy a certifikáty", "Jazyky", "Zájmy", "Kontakt", "současnost"]),
    ("Lietuvių", ["Profilis", "Pagrindinės kompetencijos", "Darbo patirtis", "Išsilavinimas", "Kursai ir sertifikatai", "Kalbos", "Pomėgiai", "Kontaktai", "dabar"]),
    ("Українська", ["Профіль", "Ключові компетенції", "Досвід роботи", "Освіта", "Курси та сертифікати", "Мови", "Інтереси", "Контакти", "дотепер"]),
    ("Română", ["Profil", "Competențe cheie", "Experiență profesională", "Educație", "Cursuri și certificări", "Limbi străine", "Interese", "Contact", "prezent"]),
    ("Magyar", ["Profil", "Kulcskompetenciák", "Szakmai tapasztalat", "Tanulmányok", "Tanfolyamok és tanúsítványok", "Nyelvtudás", "Érdeklődési kör", "Elérhetőség", "jelenleg"]),
    ("Türkçe", ["Profil", "Temel yetkinlikler", "İş deneyimi", "Eğitim", "Kurslar ve sertifikalar", "Diller", "İlgi alanları", "İletişim", "halen"]),
    ("Ελληνικά", ["Προφίλ", "Βασικές δεξιότητες", "Επαγγελματική εμπειρία", "Εκπαίδευση", "Σεμινάρια και πιστοποιήσεις", "Γλώσσες", "Ενδιαφέροντα", "Επικοινωνία", "σήμερα"]),
    ("Русский", ["Профиль", "Ключевые навыки", "Опыт работы", "Образование", "Курсы и сертификаты", "Языки", "Интересы", "Контакты", "по настоящее время"]),
    ("Bahasa Indonesia", ["Profil", "Keahlian utama", "Pengalaman kerja", "Pendidikan", "Kursus dan sertifikasi", "Bahasa", "Minat", "Kontak", "sekarang"]),
    ("Tiếng Việt", ["Giới thiệu", "Kỹ năng chính", "Kinh nghiệm làm việc", "Học vấn", "Khóa học và chứng chỉ", "Ngôn ngữ", "Sở thích", "Liên hệ", "hiện tại"]),
    ("日本語", ["プロフィール", "スキル", "職歴", "学歴", "研修・資格", "語学", "趣味・関心", "連絡先", "現在"]),
    ("简体中文", ["个人简介", "核心技能", "工作经历", "教育背景", "培训与证书", "语言能力", "兴趣爱好", "联系方式", "至今"]),
    ("한국어", ["프로필", "핵심 역량", "경력", "학력", "교육 및 자격증", "어학", "관심사", "연락처", "현재"]),
];

/// The language codes (BCP 47) of [`TABLE`], in the same order.
const TABLE_CODES: [&str; 25] = ["da", "sv", "nn", "is", "fi", "de", "nl", "fr", "es", "pt", "it", "pl", "cs", "lt", "uk", "ro", "hu", "tr", "el", "ru", "id", "vi", "ja", "zh", "ko"];

/// A CV language from assets/cv-languages.json: its headings, and the words the CV reader needs
/// to read an old CV written in it.
#[derive(Debug, Clone, Deserialize)]
pub struct CvLanguage {
    /// BCP 47, e.g. «sr-Latn».
    pub code: String,
    /// Its own name, e.g. «Srpski».
    pub name: String,
    #[serde(default)]
    pub english_name: String,
    /// Profile, skills, experience, education, certifications, languages, interests, contact, present.
    pub headings: [String; 9],
    /// Lower-case section headings by part: profile, skills, experience, education,
    /// certifications, languages, interests, projects, volunteering, skip.
    #[serde(default)]
    pub import: HashMap<String, Vec<String>>,
    /// Twelve lists of lower-case month names and abbreviations, January first.
    #[serde(default)]
    pub months: Vec<Vec<String>>,
    /// Lower-case words for «present» in a date range.
    #[serde(default)]
    pub now: Vec<String>,
}

/// The languages in assets/cv-languages.json (the ones not in [`TABLE`]).
pub static MORE: LazyLock<Vec<CvLanguage>> = LazyLock::new(|| {
    let list: Vec<CvLanguage> = serde_json::from_str(include_str!("../../../assets/cv-languages.json")).unwrap_or_default();
    // A name already in the table keeps the table's headings.
    list.into_iter().filter(|l| !TABLE.iter().any(|(n, _)| n.eq_ignore_ascii_case(&l.name))).collect()
});

/// The languages with built-in headings, by their own names.
pub fn built_in_languages() -> Vec<&'static str> {
    TABLE.iter().map(|(name, _)| *name).chain(MORE.iter().map(|l| l.name.as_str())).collect()
}

/// The code of a built-in CV language («Deutsch» → «de»), including «Bokmål» (nb) and «English» (en).
pub fn code_of(language: &str) -> Option<&'static str> {
    let wanted = language.trim().to_lowercase();
    match wanted.as_str() {
        "bokmål" | "norsk" | "norsk (bokmål)" => return Some("nb"),
        "english" => return Some("en"),
        _ => {}
    }
    TABLE.iter().position(|(n, _)| n.to_lowercase() == wanted).map(|i| TABLE_CODES[i]).or_else(|| MORE.iter().find(|l| l.name.to_lowercase() == wanted).map(|l| l.code.as_str()))
}

/// The built-in CV language that fits a computer's locale best («de-AT» → «Deutsch»,
/// «sr-Latn-RS» → «Srpski», «zh-TW» → Traditional Chinese, «nb-NO» → «Bokmål», «en-US» →
/// «English»), or `None` when there is none for it.
pub fn for_locale(locale: &str) -> Option<&'static str> {
    let locale = locale.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_lowercase();
    let locale = match locale.as_str() {
        "zh-tw" | "zh-hk" | "zh-mo" => "zh-hant".to_string(),
        l if l.starts_with("zh-hant") => "zh-hant".to_string(),
        l => l.to_string(),
    };
    let base = locale.split('-').next().unwrap_or("").to_string();
    match base.as_str() {
        "nb" | "no" => return Some("Bokmål"),
        "en" => return Some("English"),
        "" => return None,
        _ => {}
    }
    let all: Vec<(&'static str, &'static str)> =
        TABLE.iter().zip(TABLE_CODES).map(|((n, _), c)| (c, *n)).chain(MORE.iter().map(|l| (l.code.as_str(), l.name.as_str()))).collect();
    let mut tags: Vec<&str> = locale.split('-').collect();
    while !tags.is_empty() {
        let want = tags.join("-");
        if let Some((_, name)) = all.iter().find(|(c, _)| c.to_lowercase() == want) {
            return Some(name);
        }
        tags.pop();
    }
    all.iter().find(|(c, _)| c.to_lowercase().split('-').next() == Some(base.as_str())).map(|(_, n)| *n)
}

/// Built-in headings for a language, matched by its own name (case-insensitive).
pub fn built_in(language: &str) -> Option<Headings> {
    let wanted = language.trim().to_lowercase();
    if let Some(l) = MORE.iter().find(|l| l.name.to_lowercase() == wanted) {
        let h = &l.headings;
        return Some(Headings {
            language: l.name.clone(),
            profile: h[0].clone(),
            skills: h[1].clone(),
            experience: h[2].clone(),
            education: h[3].clone(),
            certifications: h[4].clone(),
            languages: h[5].clone(),
            interests: h[6].clone(),
            contact: h[7].clone(),
            present: h[8].clone(),
        });
    }
    TABLE.iter().find(|(name, _)| name.to_lowercase() == wanted).map(|(name, h)| Headings {
        language: name.to_string(),
        profile: h[0].into(),
        skills: h[1].into(),
        experience: h[2].into(),
        education: h[3].into(),
        certifications: h[4].into(),
        languages: h[5].into(),
        interests: h[6].into(),
        contact: h[7].into(),
        present: h[8].into(),
    })
}

impl Headings {
    /// Every heading is filled in.
    pub fn is_complete(&self) -> bool {
        [&self.profile, &self.skills, &self.experience, &self.education, &self.certifications, &self.languages, &self.interests, &self.contact, &self.present].iter().all(|s| !s.trim().is_empty())
    }

    /// Missing headings filled from English, so a half-typed set still makes a whole CV.
    fn filled(&self) -> Headings {
        let en = &crate::text::EN;
        let or = |s: &str, d: &str| if s.trim().is_empty() { d.to_string() } else { s.trim().to_string() };
        Headings {
            language: self.language.clone(),
            profile: or(&self.profile, en.profile),
            skills: or(&self.skills, en.skills),
            experience: or(&self.experience, en.experience),
            education: or(&self.education, en.education),
            certifications: or(&self.certifications, en.certifications),
            languages: or(&self.languages, en.languages),
            interests: or(&self.interests, en.interests),
            contact: or(&self.contact, en.contact),
            present: or(&self.present, en.present),
        }
    }
}

/// Labels for these headings. The functions that write the CV take `&'static Labels`, so each
/// distinct set is kept for the rest of the run (a handful at most).
pub fn labels(h: &Headings) -> &'static Labels {
    static KEPT: LazyLock<Mutex<HashMap<Headings, &'static Labels>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let h = h.filled();
    let Ok(mut kept) = KEPT.lock() else { return &crate::text::EN };
    if let Some(l) = kept.get(&h) {
        return l;
    }
    let s = |v: &str| -> &'static str { Box::leak(v.to_string().into_boxed_str()) };
    let l: &'static Labels = Box::leak(Box::new(Labels {
        profile: s(&h.profile),
        skills: s(&h.skills),
        experience: s(&h.experience),
        education: s(&h.education),
        certifications: s(&h.certifications),
        languages: s(&h.languages),
        interests: s(&h.interests),
        contact: s(&h.contact),
        present: s(&h.present),
    }));
    kept.insert(h, l);
    l
}
