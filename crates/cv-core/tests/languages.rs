//! Every built-in CV language gives a PDF and a Word file with that language's headings, and
//! reading them back gives the headings again.
use cv_core::{Project, document, headings, import};

/// Scripts whose letters are reordered or joined when drawn (right to left, Arabic, Hebrew, the
/// Indian scripts, Thai, Khmer). They draw correctly, but WordCraft's PDF text layer maps the
/// drawn shapes back to the wrong letters, so copying from the PDF (or an ATS reading it) gets
/// scrambled text. The Word file has the real text. Known limit of the PDF writer, to be fixed
/// there (ActualText for shaped runs).
fn shaped_script(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32,
            0x0590..=0x08FF        // Hebrew, Arabic, Syriac, Thaana …
            | 0x0900..=0x0DFF      // Devanagari … Sinhala
            | 0x0E00..=0x0E7F      // Thai
            | 0x1780..=0x17FF      // Khmer
            | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
    })
}

#[test]
fn every_built_in_language_exports_its_headings() {
    // PDF text extraction may split CJK headings into letters; compare without spaces.
    let squash = |s: &str| s.split_whitespace().collect::<String>().to_lowercase();
    let mut failed = Vec::new();
    for name in headings::built_in_languages() {
        let mut p = Project::example();
        p.language = cv_core::model::Language::En;
        p.options.headings = headings::built_in(name);
        let h = p.options.headings.clone().expect("headings");
        let doc = document::build(&p);
        let pdf = document::to_pdf(&doc).unwrap_or_else(|e| panic!("{name}: pdf {e}"));
        let docx = document::to_docx(&doc).unwrap_or_else(|e| panic!("{name}: docx {e}"));
        let word = import::text_from_bytes(&docx, "docx").unwrap_or_else(|e| panic!("{name}: {e}"));
        if !squash(&word).contains(&squash(&h.experience)) {
            failed.push(format!("{name}: «{}» not in the Word text", h.experience));
        }
        let text = import::text_from_bytes(&pdf, "pdf").unwrap_or_else(|e| panic!("{name}: {e}"));
        if !shaped_script(&h.experience) && !squash(&text).contains(&squash(&h.experience)) {
            failed.push(format!("{name}: «{}» not in the PDF text: {:.120}", h.experience, text.replace('\n', " ")));
        }
    }
    assert!(failed.is_empty(), "{} languages:\n{}", failed.len(), failed.join("\n"));
}

/// A CV's PDF in one language, for looking at by eye:
/// `CVBUILDER_LANG=العربية CVBUILDER_PDF=out.pdf cargo test --test languages one_pdf -- --ignored`
#[test]
#[ignore = "writes a file; run on purpose"]
fn one_pdf() {
    let (Ok(name), Ok(out)) = (std::env::var("CVBUILDER_LANG"), std::env::var("CVBUILDER_PDF")) else { return };
    let mut p = Project::example();
    p.language = cv_core::model::Language::En;
    p.options.headings = headings::built_in(&name);
    std::fs::write(out, document::to_pdf(&document::build(&p)).expect("pdf")).expect("write");
}

/// The English example is as complete as the Norwegian one, has no Norwegian left, and blocks
/// the download for the same reason only (the person has not confirmed the facts).
#[test]
fn the_english_example_is_complete_and_english() {
    let no = Project::example();
    let en = Project::example_en();
    assert_eq!(en.language, cv_core::model::Language::En);
    assert_eq!((en.experience.len(), en.education.len(), en.skills.len(), en.languages.len()), (no.experience.len(), no.education.len(), no.skills.len(), no.languages.len()));
    let json = en.to_json().expect("json");
    for word in ["æ", "ø", "å", "Æ", "Ø", "Å", "fiktiv", "Eksempel", "Norge"] {
        assert!(!json.contains(word), "Norwegian left in the English example: {word}");
    }
    let without_approval = |p: &Project| cv_core::checks::blocking(p).into_iter().filter(|b| !b.starts_with("approvals:")).collect::<Vec<_>>();
    assert_eq!(without_approval(&en), without_approval(&no), "the examples block the download differently");
    // It reads back from its own Word file.
    let docx = document::to_docx(&document::build(&en)).expect("docx");
    let text = import::text_from_bytes(&docx, "docx").expect("text");
    assert!(text.contains("Project Coordinator") && text.contains("Alex Morgan"));
}
