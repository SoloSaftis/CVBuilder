//! Downloading always works, also while the checklist still says something is missing.
use cv_core::{Project, document};

#[test]
fn an_incomplete_cv_still_exports() {
    let mut p = Project::default();
    p.candidate.name = "test".into();
    p.candidate.phone = "5445456".into();
    p.target.role = "test".into();
    assert!(!cv_core::checks::blocking(&p).is_empty());
    let doc = document::build(&p);
    let pdf = document::to_pdf(&doc).expect("pdf");
    assert!(pdf.starts_with(b"%PDF"));
    let docx = document::to_docx(&doc).expect("docx");
    assert!(docx.starts_with(b"PK"));
    let empty = document::build(&Project::default());
    assert!(document::to_pdf(&empty).is_ok());
    assert!(document::to_docx(&empty).is_ok());
}
