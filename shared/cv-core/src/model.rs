//! The CV project file. Field for field the same JSON as the HTML builder (schema version 2),
//! so a project can move between the two apps in either direction.
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
    pub title: String,
    /// Organisation, client or place.
    pub subtitle: String,
    /// Free text such as «2023» or «03.2022 – 06.2022».
    pub date: String,
    pub bullets: Vec<String>,
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
    pub evidence: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Education {
    pub qualification: String,
    pub institution: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    #[serde(default)]
    pub group: String,
    pub name: String,
    pub evidence: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SpokenLanguage {
    pub name: String,
    pub level: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Certification {
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
        Options { layout: default_layout(), photo: false, include_interests: true, interest_ids: Vec::new(), page_limit: 2, section_order: Vec::new(), palette: Palette::default(), section_colors: BTreeMap::new(), page: PageSetup::default(), checklist_ticks: Vec::new(), headings: None }
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
            version: 2,
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
    #[error("unsupported project version {0}")]
    Version(u8),
}

impl Project {
    /// Reads a version 1 or 2 project file. Fields added in version 2 get their defaults,
    /// exactly like the HTML builder's `upgrade`. Approval is never carried over from a file.
    pub fn from_json(text: &str) -> Result<Project, ProjectError> {
        let mut project: Project = serde_json::from_str(text)?;
        if !matches!(project.version, 1 | 2) {
            return Err(ProjectError::Version(project.version));
        }
        project.version = 2;
        project.approvals.facts_confirmed = false;
        Ok(project)
    }

    /// The project as pretty JSON, with approval cleared so a shared file is always re-checked.
    pub fn to_json(&self) -> Result<String, ProjectError> {
        let mut copy = self.clone();
        copy.approvals.facts_confirmed = false;
        Ok(serde_json::to_string_pretty(&copy)?)
    }

    pub fn is_english(&self) -> bool {
        self.language == Language::En
    }
}

impl Project {
    /// The fictional example the app opens with (the same one as the HTML builder).
    pub fn example() -> Project {
        Project::from_json(include_str!("example.json")).unwrap_or_default()
    }
}
