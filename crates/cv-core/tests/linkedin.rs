//! Importing LinkedIn's data export (.zip). The fixtures in tests/linkedin/ are made up: an
//! English export (with messages and connections that must never be read) and a Norwegian one.
use std::io::Write;
use std::path::{Path, PathBuf};

use cv_core::import::{self, CvFile};
use cv_core::model::Language;

/// The fixture folder packed as a LinkedIn export, every file under `prefix` in the archive.
fn export(fixture: &str, prefix: &str, name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/linkedin").join(fixture);
    let out = std::env::temp_dir().join(format!("cvbuilder-linkedin-test-{}", std::process::id()));
    std::fs::create_dir_all(&out).expect("dir");
    let path = out.join(name);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("create"));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files: Vec<_> = std::fs::read_dir(&dir).expect("fixture").map(|e| e.expect("entry").path()).collect();
    files.sort();
    for f in files {
        zip.start_file(format!("{prefix}{}", f.file_name().expect("name").to_string_lossy()), options).expect("start");
        zip.write_all(&std::fs::read(&f).expect("read")).expect("write");
    }
    zip.finish().expect("finish");
    path
}

fn read(path: &Path) -> cv_core::import::CvRead {
    match import::cv_from_file(path).expect("import") {
        CvFile::LinkedIn(read) => read,
        CvFile::Text(t) => panic!("read as text, not as a LinkedIn export: {t:.200}"),
        _ => panic!("read as another format, not as a LinkedIn export"),
    }
}

#[test]
fn an_english_export_fills_the_cv() {
    let r = read(&export("en", "", "Basic_LinkedInDataExport_11-19-2026.zip"));
    let p = &r.project;
    let c = &p.candidate;
    assert_eq!(c.name, "Alex Example");
    assert_eq!(c.headline, "Project Manager | Construction and Infrastructure");
    assert!(c.summary.starts_with("Project manager with eight years") && c.summary.contains("\nI lead teams"), "{:?}", c.summary);
    assert_eq!(c.city, "Bristol");
    assert_eq!(c.email, "alex.example@example.com", "the primary address, not the first");
    assert_eq!(c.phone, "+44 7700 900123");
    assert_eq!(p.language, Language::En, "English text gives an English CV");

    assert_eq!(p.experience.len(), 3);
    let j = &p.experience[0];
    assert_eq!((j.company.as_str(), j.title.as_str(), j.location.as_str()), ("Example Build Ltd", "Senior Project Manager", "Bristol, England, United Kingdom"));
    assert_eq!((j.start.as_str(), j.end.clone()), ("2021-03", None), "no end date: a current job");
    assert_eq!(j.bullets.len(), 3, "{:?}", j.bullets);
    assert_eq!(j.bullets[0], "Led the rebuild of a 12,000 m² school with a budget of £30 million", "the bullet mark is taken off, the words kept");
    assert!(j.bullets[2].contains("\"user group\""), "quotes in a quoted field: {:?}", j.bullets[2]);
    let j = &p.experience[1];
    assert_eq!((j.start.as_str(), j.end.as_deref()), ("2016-08", Some("2021-02")));
    assert_eq!(j.bullets, ["Coordinated the planning and procurement for road and bridge projects worth £45 million in total."], "prose stays as written");
    let j = &p.experience[2];
    assert_eq!((j.start.as_str(), j.end.as_deref()), ("2015", Some("2016")), "bare years");
    assert!(j.bullets.is_empty());

    assert_eq!(p.education.len(), 2);
    assert_eq!((p.education[0].qualification.as_str(), p.education[0].institution.as_str()), ("BEng, Civil Engineering", "Example University"));
    assert_eq!((p.education[0].start.as_str(), p.education[0].end.as_str()), ("2011", "2015"));
    assert_eq!(p.education[1].institution, "Sample College");

    let skills: Vec<&str> = p.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(skills, ["Project Management", "Procurement", "AutoCAD", "Risk Management", "Stakeholder Management"], "a skill listed twice is kept once");
    let langs: Vec<(&str, &str)> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
    assert_eq!(langs, [("English", "native"), ("Spanish", "limited working"), ("French", "")]);
    assert_eq!(p.certifications.len(), 1);
    assert_eq!((p.certifications[0].name.as_str(), p.certifications[0].issuer.as_str(), p.certifications[0].year.as_str()), ("PRINCE2 Practitioner", "Example Certification Body", "2019"));

    assert_eq!(p.custom_sections.len(), 1);
    let s = &p.custom_sections[0];
    assert_eq!(s.title, "Projects");
    assert_eq!((s.items[0].title.as_str(), s.items[0].date.as_str()), ("Riverside school rebuild", "03.2021 – 06.2023"));
    assert_eq!(s.items[0].bullets, ["New school for 600 pupils.", "Delivered two weeks early."]);
    assert_eq!(s.items[1].date, "01.2024 – Present");

    assert!(r.found.contains(&"jobber (3)".to_string()) && r.found.contains(&"kompetanse (5)".to_string()), "{:?}", r.found);
    assert!(r.rest.is_empty());
}

#[test]
fn messages_and_connections_are_never_read() {
    let r = read(&export("en", "", "export.zip"));
    let json = r.project.to_json().expect("json");
    for private in ["private message", "Someone Else", "private@example.com", "Other Co"] {
        assert!(!json.contains(private), "«{private}» came from a file that must not be read");
    }
    assert!(!cv_core::linkedin::READ.iter().any(|f| f.eq_ignore_ascii_case("messages.csv") || f.eq_ignore_ascii_case("Connections.csv")));
}

#[test]
fn a_norwegian_export_in_a_folder_gives_a_norwegian_cv() {
    let r = read(&export("no", "Basic_LinkedInDataExport_11-19-2026/", "linkedin.zip"));
    let p = &r.project;
    assert_eq!(p.candidate.name, "Ingrid Eksempelsen");
    assert_eq!(p.candidate.city, "Bergen");
    assert_eq!(p.candidate.email, "ingrid.eksempelsen@example.no");
    assert_eq!(p.language, Language::No);
    assert_eq!(p.experience.len(), 2);
    assert_eq!((p.experience[0].start.as_str(), p.experience[0].end.clone()), ("2021-03", None), "Norwegian month names");
    assert_eq!(p.experience[0].bullets, ["Koordinerte innkjøp for 14 prosjekter", "Innførte ukentlige fremdriftsmøter med alle underleverandører"]);
    assert_eq!((p.experience[1].start.as_str(), p.experience[1].end.as_deref()), ("2018-08", Some("2021-02")));
    let langs: Vec<(&str, &str)> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
    assert_eq!(langs, [("Norsk", "morsmål"), ("Engelsk", "flytende")]);
    assert!(p.custom_sections.is_empty() && p.certifications.is_empty(), "missing files are skipped");
}

#[test]
fn other_zip_files_are_not_taken_for_an_export() {
    // A Word file is a ZIP too; it is still read as text.
    let doc = cv_core::document::build(&cv_core::Project::example());
    let path = std::env::temp_dir().join(format!("cvbuilder-linkedin-test-{}-cv.docx", std::process::id()));
    std::fs::write(&path, cv_core::document::to_docx(&doc).expect("docx")).expect("write");
    assert!(matches!(import::cv_from_file(&path), Ok(CvFile::Text(_))));

    // A ZIP with only files the reader does not use is not an export.
    let dir = std::env::temp_dir().join(format!("cvbuilder-linkedin-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("other.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("create"));
    zip.start_file("notes.txt", zip::write::SimpleFileOptions::default()).expect("start");
    zip.write_all(b"just some notes").expect("write");
    zip.finish().expect("finish");
    assert!(import::cv_from_file(&path).is_err());

    // An export with a profile file but nothing in it says so.
    let path = dir.join("empty-export.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("create"));
    zip.start_file("Profile.csv", zip::write::SimpleFileOptions::default()).expect("start");
    zip.write_all(b"First Name,Last Name,Headline\r\n").expect("write");
    zip.finish().expect("finish");
    let err = import::cv_from_file(&path).err().expect("an error");
    assert!(err.contains("LinkedIn"), "{err}");
}
