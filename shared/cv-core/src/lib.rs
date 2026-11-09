//! CVBuilder core: the CV data model, recruiter/ATS checks, and Word + PDF output built on
//! WordCraft's document model, so both files come from one layout.
pub mod checklist;
pub mod checks;
pub mod import;
mod wordlists;
pub mod document;
pub mod headings;
pub mod layouts;
pub mod model;
pub mod text;
pub use model::{Language, Project, ProjectError};
