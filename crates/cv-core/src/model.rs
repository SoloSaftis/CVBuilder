//! The CV project file. Field for field the same JSON as the HTML builder (schema version 3),
//! so a project can move between the two apps in either direction. Version 3 gave every item a
//! stable id and added `options.hidden` (see `ids`); versions 1 and 2 are read and upgraded.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub version: u8,
    pub language: Language,
    pub candidate: Candidate,
    pub target: Target,
    pub experience: Vec<Job>,
    pub education: Vec<Education>,
    pub skills: Vec<Skill>,
    pub languages: Vec<SpokenLanguage>,
    #[serde(default)]
    pub certifications: Vec<Certification>,
    pub interests: Vec<Interest>,
    /// Sections the person adds themselves (projects, volunteering, publications …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_sections: Vec<CustomSection>,
    pub public_context: Vec<PublicContext>,
    pub options: Options,
    pub approvals: Approvals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    No,
    En,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub city: String,
    pub headline: String,
    #[serde(default)]
    pub linkedin: String,
    pub summary: String,
    /// `data:image/jpeg;base64,…`, already cropped to the 25:32 portrait box, or empty.
    pub photo: String,
    /// More contact lines the person adds (GitHub, portfolio, driving licence …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<ContactField>,
    /// The order of the contact line: `email`, `phone`, `city`, `linkedin` and the ids of `extra`.
    /// Empty means the recommended order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contact_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ContactField {
    pub id: String,
    /// Shown before the value when not empty, e.g. «Førerkort».
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CustomSection {
    pub id: String,
    pub title: String,
    pub items: Vec<CustomItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CustomItem {
    #[serde(default)]
    pub id: String,
    pub title: String,
    /// Organisation, client or place.
    pub subtitle: String,
    /// Free text such as «2023» or «03.2022 – 06.2022».
    pub date: String,
    pub bullets: Vec<String>,
    /// The id of each bullet, at the same position (see `ids`).
    #[serde(default)]
    pub bullet_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub role: String,
    pub company: String,
    pub url: String,
    pub description: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: String,
    pub company: String,
    pub title: String,
    #[serde(default)]
    pub location: String,
    /// `YYYY-MM`.
    pub start: String,
    /// `YYYY-MM`, `""` while unfinished, or `None` (JSON `null`) for a current job.
    pub end: Option<String>,
    pub bullets: Vec<String>,
    /// The id of each bullet, at the same position (see `ids`).
    #[serde(default)]
    pub bullet_ids: Vec<String>,
    pub evidence: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Education {
    #[serde(default)]
    pub id: String,
    pub qualification: String,
    pub institution: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub group: String,
    pub name: String,
    pub evidence: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SpokenLanguage {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub level: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Certification {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub issuer: String,
    pub year: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InterestStatus {
    #[default]
    Current,
    Former,
    Interest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Interest {
    pub id: String,
    pub label: String,
    pub status: InterestStatus,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub confirmed: bool,
    pub specificity: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PublicContext {
    pub topic: String,
    pub tags: Vec<String>,
    pub source_url: String,
    pub checked_on: String,
    pub scope: String,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    /// The design's id (see `layouts`): `classic`, `modern`, `rr-onyx`, an imported one …
    #[serde(default = "default_layout")]
    pub layout: String,
    pub photo: bool,
    pub include_interests: bool,
    pub interest_ids: Vec<String>,
    pub page_limit: u8,
    /// The order of the CV's sections: `profile`, `skills`, `experience`, `education`,
    /// `certifications`, `languages`, `interests` and custom section ids. Empty means recommended.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section_order: Vec<String>,
    /// Colours chosen for the whole CV, over the design's own. Independent of the design.
    #[serde(default, skip_serializing_if = "Palette::is_empty")]
    pub palette: Palette,
    /// A colour per section (section key → `#RRGGBB`) for its heading, line and organisation names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub section_colors: BTreeMap<String, String>,
    /// Page size, margins, columns, text size, spacing and page breaks.
    #[serde(default, skip_serializing_if = "PageSetup::is_default")]
    pub page: PageSetup,
    /// Points of the person's own checklist they have ticked off for this CV (item ids).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checklist_ticks: Vec<String>,
    /// Headings for a CV in a language other than Norwegian and English.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headings: Option<crate::headings::Headings>,
    /// Ids of the items left out of this CV (a tailored copy): they stay in the form and the
    /// file, but not in the preview, the downloads or the checks. See `ids`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// On a tailored copy: the file name of the full CV it was made from (in the same folder).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub copy_of: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PageSize {
    #[default]
    A4,
    Letter,
    Legal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Margins {
    /// 17 mm all round (the recommended CV margin).
    #[default]
    Normal,
    /// 12.7 mm.
    Narrow,
    /// 19 mm top and bottom, 25 mm at the sides.
    Moderate,
    /// 25.4 mm.
    Wide,
    /// `custom_mm`.
    Custom,
}

/// One column, or a sidebar on the left or right, whatever the design has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnsChoice {
    One,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PageSetup {
    pub size: PageSize,
    pub margins: Margins,
    /// Top, right, bottom, left in millimetres, for `Custom`.
    pub custom_mm: [f32; 4],
    /// `None` keeps the design's own columns.
    pub columns: Option<ColumnsChoice>,
    /// Points added to every text size (-1.5 to +2).
    pub text_delta: f32,
    /// Spacing multiplier (0.8 to 1.3).
    pub spacing: f32,
    /// Section keys that start on a new page.
    pub page_breaks: Vec<String>,
    /// Section keys after which the next section starts on a new page.
    pub page_breaks_after: Vec<String>,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup { size: PageSize::A4, margins: Margins::Normal, custom_mm: [17.0, 17.0, 15.0, 17.0], columns: None, text_delta: 0.0, spacing: 1.0, page_breaks: Vec::new(), page_breaks_after: Vec::new() }
    }
}

impl PageSetup {
    pub fn is_default(&self) -> bool {
        *self == PageSetup::default()
    }
}

/// The CV's colours. Each one left out keeps the design's own colour. Colours are `#RRGGBB`;
/// `header_fill` and `heading_fill` may be `none` to remove the design's band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_fill: Option<String>,
}

impl Palette {
    pub fn is_empty(&self) -> bool {
        *self == Palette::default()
    }
}

fn default_layout() -> String {
    "classic".into()
}

impl Default for Options {
    fn default() -> Self {
        Options { layout: default_layout(), photo: false, include_interests: true, interest_ids: Vec::new(), page_limit: 2, section_order: Vec::new(), palette: Palette::default(), section_colors: BTreeMap::new(), page: PageSetup::default(), checklist_ticks: Vec::new(), headings: None, hidden: Vec::new(), copy_of: String::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Approvals {
    pub facts_confirmed: bool,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            version: VERSION,
            language: Language::No,
            candidate: Candidate::default(),
            target: Target::default(),
            experience: Vec::new(),
            education: Vec::new(),
            skills: Vec::new(),
            languages: Vec::new(),
            certifications: Vec::new(),
            interests: Vec::new(),
            custom_sections: Vec::new(),
            public_context: Vec::new(),
            options: Options::default(),
            approvals: Approvals::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("the project file could not be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported project version {0}: the file was saved by a newer CVBuilder")]
    Version(u8),
}

/// The project schema this version writes.
pub const VERSION: u8 = 3;

impl Project {
    /// Reads a version 1, 2 or 3 project file. Fields added later get their defaults, and items
    /// without an id get one (the same ids every time the file is read), exactly like the HTML
    /// builder's `upgrade`. Approval is never carried over from a file.
    pub fn from_json(text: &str) -> Result<Project, ProjectError> {
        // The version first, so a file from a newer CVBuilder says so instead of naming a field.
        if let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(text)
            && let Some(v) = o.get("version").and_then(serde_json::Value::as_u64)
            && !(1..=u64::from(VERSION)).contains(&v)
        {
            return Err(ProjectError::Version(u8::try_from(v).unwrap_or(u8::MAX)));
        }
        let mut project: Project = serde_json::from_str(text)?;
        if !(1..=VERSION).contains(&project.version) {
            return Err(ProjectError::Version(project.version));
        }
        project.version = VERSION;
        project.assign_ids(true);
        project.approvals.facts_confirmed = false;
        Ok(project)
    }

    /// The project as pretty JSON, with approval cleared so a shared file is always re-checked.
    pub fn to_json(&self) -> Result<String, ProjectError> {
        let mut copy = self.clone();
        copy.version = VERSION;
        copy.normalize_ids();
        copy.approvals.facts_confirmed = false;
        Ok(serde_json::to_string_pretty(&copy)?)
    }

    /// The project as a version 2 file, for a CVBuilder or HTML builder from before schema
    /// version 3: the same CV without the ids, and in full (they cannot hide items).
    pub fn to_v2_json(&self) -> Result<String, ProjectError> {
        let mut copy = self.clone();
        copy.version = 2;
        copy.approvals.facts_confirmed = false;
        copy.options.hidden.clear();
        copy.options.copy_of.clear();
        let mut v = serde_json::to_value(&copy)?;
        fn strip(v: &mut serde_json::Value, list: &str, keys: &[&str]) {
            if let Some(items) = v.get_mut(list).and_then(serde_json::Value::as_array_mut) {
                for o in items.iter_mut().filter_map(serde_json::Value::as_object_mut) {
                    for k in keys {
                        o.remove(*k);
                    }
                }
            }
        }
        strip(&mut v, "experience", &["bullet_ids"]);
        for list in ["education", "skills", "languages", "certifications"] {
            strip(&mut v, list, &["id"]);
        }
        if let Some(sections) = v.get_mut("custom_sections").and_then(serde_json::Value::as_array_mut) {
            for s in sections {
                strip(s, "items", &["id", "bullet_ids"]);
            }
        }
        Ok(serde_json::to_string_pretty(&v)?)
    }

    pub fn is_english(&self) -> bool {
        self.language == Language::En
    }

    /// The CV's language as a BCP 47 code: «nb», «en», or the code of its own headings
    /// («العربية» → «ar»). A language typed by hand with no known code counts as English.
    pub fn language_code(&self) -> &'static str {
        if let Some(h) = &self.options.headings
            && let Some(code) = crate::headings::code_of(&h.language)
        {
            return code;
        }
        if self.is_english() { "en" } else { "nb" }
    }

    /// The CV is written right to left (Arabic, Hebrew, Persian, Urdu, Uyghur …).
    pub fn is_right_to_left(&self) -> bool {
        crate::headings::is_right_to_left(self.language_code())
    }
}

impl Project {
    /// Anything the person has written: a CV with none of it (a new, untouched one) has nothing to lose.
    pub fn has_content(&self) -> bool {
        let c = &self.candidate;
        let t = &self.target;
        [&c.name, &c.email, &c.phone, &c.city, &c.headline, &c.linkedin, &c.summary, &c.photo, &t.role, &t.company, &t.url, &t.description].iter().any(|v| !v.trim().is_empty())
            || !c.extra.is_empty()
            || !t.keywords.is_empty()
            || !self.experience.is_empty()
            || !self.education.is_empty()
            || !self.skills.is_empty()
            || !self.languages.is_empty()
            || !self.certifications.is_empty()
            || !self.interests.is_empty()
            || !self.custom_sections.is_empty()
    }

    /// The fictional example the app opens with (the same one as the HTML builder).
    pub fn example() -> Project {
        Project::from_json(include_str!("example.json")).unwrap_or_default()
    }

    /// The same example in English: a made-up person in the UK, for every CV language except
    /// Norwegian (its headings and dates are then set to the chosen language).
    pub fn example_en() -> Project {
        Project::from_json(include_str!("example_en.json")).unwrap_or_default()
    }
}
