//! The CV project file. Field for field the same JSON as the HTML builder (schema version 2),
//! so a project can move between the two apps in either direction.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutId {
    #[default]
    Classic,
    Modern,
    Nordic,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    pub layout: LayoutId,
    pub photo: bool,
    pub include_interests: bool,
    pub interest_ids: Vec<String>,
    pub page_limit: u8,
}

impl Default for Options {
    fn default() -> Self {
        Options { layout: LayoutId::Classic, photo: false, include_interests: true, interest_ids: Vec::new(), page_limit: 2 }
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
