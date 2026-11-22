//! Europass: the two XML formats (tests/europass/, made-up people), and a PDF with the XML
//! attached as the Europass editor saves it.
use std::path::Path;

use cv_core::import::{self, CvFile, CvRead};
use cv_core::model::Language;

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/europass").join(name)).expect("fixture")
}

fn europass(bytes: &[u8], ext: &str) -> CvRead {
    match import::cv_from_bytes(bytes, ext).expect("import") {
        CvFile::Europass(r) => r,
        CvFile::Text(t) => panic!("read as text: {t:.200}"),
        _ => panic!("read as another format"),
    }
}

#[test]
fn the_platform_format_fills_the_cv() {
    let r = europass(&bytes("candidate-en.xml"), "xml");
    let p = &r.project;
    let c = &p.candidate;
    assert_eq!(p.language, Language::En);
    assert_eq!((c.name.as_str(), c.email.as_str(), c.phone.as_str(), c.city.as_str()), ("Maria Esempio", "maria.esempio@example.org", "+39 320 000 0000", "Bologna"));
    assert_eq!(c.linkedin, "https://www.linkedin.com/in/maria-esempio-example");
    assert!(c.extra.iter().any(|e| e.id == "web" && e.value == "https://portfolio.example.org"));
    assert!(c.extra.iter().any(|e| e.label == "Driving licence" && e.value == "B, C1"));
    assert!(c.summary.starts_with("Logistics planner with eight years") && !c.summary.contains('<'));

    assert_eq!(p.experience.len(), 2);
    let j = &p.experience[0];
    assert_eq!((j.company.as_str(), j.title.as_str(), j.location.as_str(), j.start.as_str(), j.end.clone()), ("Example Freight S.r.l.", "Logistics Planner", "Bologna", "2020-02", None));
    assert_eq!(j.bullets, ["Plan daily routes for 40 trucks across northern Italy.", "Cut empty kilometres by 12% with shared loads."]);
    let j = &p.experience[1];
    assert_eq!((j.location.as_str(), j.start.as_str(), j.end.as_deref()), ("Modena", "2016-09", Some("2020-01")));

    let e = &p.education[0];
    assert_eq!((e.qualification.as_str(), e.institution.as_str(), e.start.as_str(), e.end.as_str()), ("BSc Economics and Management", "Example University of Bologna", "2011-09", "2014-07"));

    let langs: Vec<(&str, &str)> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
    assert_eq!(langs, [("Italian", "native"), ("English", "B2–C1"), ("German", "B1")]);
    let skills: Vec<(&str, &str)> = p.skills.iter().map(|s| (s.group.as_str(), s.name.as_str())).collect();
    assert_eq!(skills, [("Digital skills", "SAP EWM"), ("Office tools", "Excel (pivot tables)"), ("Office tools", "Power BI")]);
    assert_eq!(p.custom_sections[0].title, "Volunteering");
    assert_eq!(p.custom_sections[0].items[0].date, "2019 – Present");

    // What a CV here has no field for is left for the person, never dropped silently.
    for bit in ["1992-04-17", "IT", "108/110", "Via Esempio 1"] {
        assert!(r.rest.contains(bit), "{bit} not in the rest: {}", r.rest);
    }
}

#[test]
fn the_older_format_fills_the_cv() {
    let r = europass(&bytes("skillspassport-no.xml"), "xml");
    let p = &r.project;
    let c = &p.candidate;
    assert_eq!(p.language, Language::No);
    assert_eq!((c.name.as_str(), c.headline.as_str(), c.city.as_str(), c.phone.as_str()), ("Kari Eksempel", "Sykepleier", "Trondheim", "+47 400 00 000"));
    assert_eq!(c.email, "kari.eksempel@example.no");
    assert!(c.linkedin.contains("kari-eksempel"));
    assert!(c.extra.iter().any(|e| e.label == "Skype" && e.value == "kari.eksempel"));
    assert!(c.extra.iter().any(|e| e.label == "Førerkort" && e.value == "B"));

    let j = &p.experience[0];
    assert_eq!((j.company.as_str(), j.start.as_str(), j.end.clone(), j.location.as_str()), ("Eksempel sykehus (fiktivt)", "2016-08", None, "Trondheim"));
    assert_eq!(j.bullets.len(), 2);
    assert_eq!((p.experience[1].start.as_str(), p.experience[1].end.as_deref()), ("2013-06", Some("2016-07")));
    assert_eq!((p.education[0].start.as_str(), p.education[0].end.as_str()), ("2013", "2016"));

    let langs: Vec<(&str, &str)> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
    assert_eq!(langs, [("norsk", "morsmål"), ("engelsk", "C1"), ("Spansk", "A2–B1")]);
    assert_eq!((p.certifications[0].name.as_str(), p.certifications[0].year.as_str()), ("DELE A2", "2018"));
    assert_eq!(p.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["DIPS Arena", "Microsoft Office"]);
    for bit in ["1990-06-12", "Norsk Sykepleierforbund", "forklare behandling", "Eksempelveien 1"] {
        assert!(r.rest.contains(bit), "{bit} not in the rest: {}", r.rest);
    }
}

/// A real PDF (CVBuilder's own) with the Europass XML attached the way PDF attachments are.
fn pdf_with_attachment(xml: &[u8]) -> Vec<u8> {
    use lopdf::{Dictionary, Object, Stream};
    let pdf = cv_core::document::to_pdf(&cv_core::document::build(&cv_core::Project::example_en())).expect("pdf");
    let mut doc = lopdf::Document::load_mem(&pdf).expect("load");
    let mut file = Stream::new(Dictionary::from_iter([("Type", Object::Name(b"EmbeddedFile".to_vec())), ("Subtype", Object::Name(b"text/xml".to_vec()))]), xml.to_vec());
    file.compress().expect("compress");
    let file = doc.add_object(file);
    let spec = doc.add_object(Dictionary::from_iter([
        ("Type", Object::Name(b"Filespec".to_vec())),
        ("F", Object::string_literal("attachment.xml")),
        ("UF", Object::string_literal("attachment.xml")),
        ("EF", Object::Dictionary(Dictionary::from_iter([("F", Object::Reference(file))]))),
    ]));
    let names = doc.add_object(Dictionary::from_iter([("Names", Object::Array(vec![Object::string_literal("attachment.xml"), Object::Reference(spec)]))]));
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).expect("root");
    let catalog = doc.get_object_mut(root).and_then(Object::as_dict_mut).expect("catalog");
    catalog.set("Names", Object::Dictionary(Dictionary::from_iter([("EmbeddedFiles", Object::Reference(names))])));
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

#[test]
fn a_europass_pdf_is_read_from_its_attachment() {
    let pdf = pdf_with_attachment(&bytes("candidate-en.xml"));
    let r = europass(&pdf, "pdf");
    assert_eq!(r.project.candidate.name, "Maria Esempio");
    assert_eq!(r.project.experience.len(), 2);
    // Without the attachment, the same PDF is read as text as before.
    let plain = cv_core::document::to_pdf(&cv_core::document::build(&cv_core::Project::example_en())).expect("pdf");
    assert!(matches!(import::cv_from_bytes(&plain, "pdf"), Ok(CvFile::Text(_))));
}

#[test]
fn other_xml_is_not_taken_for_europass() {
    let e = import::cv_from_bytes(b"<?xml version=\"1.0\"?><note><to>Someone</to><body>Some text that is long enough to read.</body></note>", "xml").err().expect("refused");
    assert!(e.contains("Europass"), "{e}");
}
