//! CVBuilder core: the CV data model, recruiter/ATS checks, and Word + PDF output built on
//! WordCraft's document model, so both files come from one layout.
pub mod checklist;
pub mod checks;
pub mod cvread;
pub mod import;
pub mod readback;
pub mod requirements;
pub mod spelling;
pub mod linkedin;
pub mod europass;
pub mod jsonresume;
mod wordlists;
pub mod document;
pub mod headings;
pub mod ids;
pub mod layouts;
pub mod model;
pub mod plain;
mod pdfread;
pub mod text;
pub use model::{Language, Project, ProjectError};
