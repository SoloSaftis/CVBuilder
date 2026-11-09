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
const TABLE: [(&str, [&str; 9]); 19] = [
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
    ("日本語", ["プロフィール", "スキル", "職歴", "学歴", "研修・資格", "語学", "趣味・関心", "連絡先", "現在"]),
    ("简体中文", ["个人简介", "核心技能", "工作经历", "教育背景", "培训与证书", "语言能力", "兴趣爱好", "联系方式", "至今"]),
    ("한국어", ["프로필", "핵심 역량", "경력", "학력", "교육 및 자격증", "어학", "관심사", "연락처", "현재"]),
];

/// The languages with built-in headings, by their own names.
pub fn built_in_languages() -> Vec<&'static str> {
    TABLE.iter().map(|(name, _)| *name).collect()
}

/// Built-in headings for a language, matched by its own name (case-insensitive).
pub fn built_in(language: &str) -> Option<Headings> {
    let wanted = language.trim().to_lowercase();
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
