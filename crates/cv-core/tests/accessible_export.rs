//! Accessible, clean exports (053): every PDF and Word file carries the CV's language, a title
//! and author from the CV, nothing about this computer or the job ad the CV doesn't show, and the
//! PDF is tagged (headings, lists, the portrait's alt text) and passes krilla's PDF/UA-1 checks.
use std::io::Read;

use cv_core::{Project, document, headings};

/// The example CV with the headings (and so the language) of a built-in CV language.
fn in_language(name: &str) -> Project {
    let mut p = Project::example();
    if name == "Bokmål" {
        return p;
    }
    if name == "English" {
        return Project::example_en();
    }
    p.language = cv_core::model::Language::En;
    p.options.headings = headings::built_in(name);
    assert!(p.options.headings.is_some(), "{name} is no built-in language");
    p
}

/// The PDF's bytes as text without whitespace, for looking for dictionary entries.
fn squashed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).split_whitespace().collect()
}

fn docx_part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    let mut out = String::new();
    zip.by_name(name).unwrap_or_else(|_| panic!("{name} missing")).read_to_string(&mut out).expect("read");
    out
}

/// The PDF in Norwegian, English, Arabic and Hindi: its language, title and author, tags for the
/// name, the section headings and the bullet lists. Latin CVs pass krilla's PDF/UA-1 checks, so
/// the file says PDF/UA-1 and shows its title in the viewer's title bar. Arabic and Hindi pass
/// with the Windows fonts; with another system's fallback fonts some glyphs may map to no text
/// (`NoCodepointMapping`), and then the file must make no PDF/UA claim.
#[test]
fn pdf_carries_language_title_author_and_tags() {
    for (name, code, latin) in [("Bokmål", "nb", true), ("English", "en", true), ("العربية", "ar", false), ("हिन्दी", "hi", false)] {
        let p = in_language(name);
        assert_eq!(p.language_code(), code);
        let doc = document::build(&p);
        let problems = document::pdf_ua_problems(&doc).expect("check");
        let raw = squashed(&document::to_pdf(&doc).expect("pdf"));
        assert!(raw.contains(&format!("/Lang({code}")), "{name}: document language");
        if latin || cfg!(windows) || problems.is_empty() {
            assert!(problems.is_empty(), "{name}: {problems:?}");
            assert!(raw.contains("pdfuaid:part"), "{name}: PDF/UA-1 identifier");
            assert!(raw.contains("/DisplayDocTitletrue"), "{name}: title in the title bar");
        } else {
            assert!(problems.iter().all(|p| p.starts_with("NoCodepointMapping")), "{name}: {problems:?}");
            assert!(!raw.contains("pdfuaid"), "{name}: a file that fails the checks claims PDF/UA-1");
        }
        assert!(raw.contains("/S/H1") && raw.contains("/S/H2"), "{name}: name and section headings");
        assert!(raw.contains("/S/L") && raw.contains("/S/LI") && raw.contains("/S/LBody"), "{name}: bullet lists");
    }
}

/// Every design gives a PDF that passes krilla's PDF/UA-1 checks, the sidebar ones too, with
/// and without the portrait.
#[test]
fn every_design_passes_the_pdf_ua_checks() {
    let mut failed = Vec::new();
    for design in cv_core::layouts::all() {
        for photo in [false, true] {
            let mut p = Project::example();
            p.options.layout = design.id.to_string();
            p.options.photo = photo;
            p.candidate.photo = format!("data:image/jpeg;base64,{TINY_JPEG}");
            let problems = document::pdf_ua_problems(&document::build(&p)).expect("check");
            if !problems.is_empty() {
                failed.push(format!("{} photo={photo}: {problems:?}", design.id));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The Word file's language follows the CV, and its properties hold the CV's title and author
/// and nothing else about the person, the computer or the job ad.
#[test]
fn word_carries_language_and_clean_properties() {
    for (name, code) in [("Bokmål", "nb"), ("العربية", "ar"), ("हिन्दी", "hi")] {
        let p = in_language(name);
        let bytes = document::to_docx(&document::build(&p)).expect("docx");
        let styles = docx_part(&bytes, "word/styles.xml");
        assert!(styles.contains(&format!("w:val=\"{code}")), "{name}: w:lang in styles: {}", &styles[..styles.len().min(1500)]);
        let core = docx_part(&bytes, "docProps/core.xml");
        assert!(core.contains(&format!("<dc:title>{} – CV</dc:title>", p.candidate.name)), "{core}");
        assert!(core.contains(&format!("<dc:creator>{}</dc:creator>", p.candidate.name)), "{core}");
        for absent in ["lastModifiedBy", "dcterms:modified", "cp:revision"] {
            assert!(!core.contains(absent), "{name}: {absent} in {core}");
        }
        let app = docx_part(&bytes, "docProps/app.xml");
        assert!(!app.contains("<Company>") && !app.contains("<Manager>"), "{app}");
        if let Ok(user) = std::env::var("USERNAME").or_else(|_| std::env::var("USER")) {
            for part in [&core, &app] {
                assert!(user.len() < 3 || !part.contains(&user), "the computer's user name {user} in {part}");
            }
        }
    }
}

/// The job ad's company and the keywords the CV doesn't use stay out of the file properties;
/// a keyword the CV does use is kept.
#[test]
fn properties_hold_only_what_the_cv_shows() {
    let mut p = Project::example();
    p.target.company = "Hemmelig Arbeidsgiver AS".into();
    p.target.role = "Rolle som ikke står i CV-en".into();
    let used = p.skills.first().map(|s| s.name.clone()).expect("a skill");
    p.target.keywords = vec![used.clone(), "ordsomikkefinnesicv".into()];
    let (title, author, subject, keywords) = document::properties(&p);
    assert_eq!(title, format!("{} – CV", p.candidate.name));
    assert_eq!(author, p.candidate.name);
    assert_eq!(subject, "", "a role the CV doesn't show");
    assert_eq!(keywords, used);
    let doc = document::build(&p);
    let core = docx_part(&document::to_docx(&doc).expect("docx"), "docProps/core.xml");
    assert!(!core.contains("Hemmelig") && !core.contains("ordsomikkefinnesicv"), "{core}");
    let pdf = String::from_utf8_lossy(&document::to_pdf(&doc).expect("pdf")).into_owned();
    assert!(!pdf.contains("Hemmelig") && !pdf.contains("ordsomikkefinnesicv"), "the PDF has the ad's words");
}

/// A 4×4 grey JPEG.
const TINY_JPEG: &str = "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAMCAgMCAgMDAwMEAwMEBQgFBQQEBQoHBwYIDAoMDAsKCwsNDhIQDQ4RDgsLEBYQERMUFRUVDA8XGBYUGBIUFRT/2wBDAQMEBAUEBQkFBQkUDQsNFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBT/wAARCAAEAAQDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwAooooA/9k=";

/// The portrait's alt text is the person's name, in no particular language.
#[test]
fn the_portrait_is_described_by_the_name() {
    let mut p = Project::example();
    p.options.photo = true;
    p.candidate.photo = format!("data:image/jpeg;base64,{TINY_JPEG}");
    let raw = squashed(&document::to_pdf(&document::build(&p)).expect("pdf"));
    let alt: String = format!("/Alt({})", p.candidate.name).split_whitespace().collect();
    assert!(raw.contains(&alt) || raw.contains("/Alt<"), "portrait alt text: {:?}", raw.match_indices("/Alt").map(|(i, _)| &raw[i..(i + 60).min(raw.len())]).collect::<Vec<_>>());
}
