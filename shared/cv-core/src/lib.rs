//! CVBuilder core: the CV data model, recruiter/ATS checks, and Word + PDF output built on
//! WordCraft's document model, so both files come from one layout.
pub mod checks;
mod wordlists;
pub mod document;
pub mod layouts;
pub mod model;
pub mod text;
pub use model::{LayoutId, Language, Project, ProjectError};
