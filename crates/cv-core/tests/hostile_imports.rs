//! 170: Europass and JSON Resume files made to hurt (deep nesting, a Flate bomb, huge files and
//! lists, odd pictures) are refused or cut down, never a crash or a hang; and a photo switched off
//! stays out of the JSON Resume export.
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine as _;
use cv_core::Project;
use cv_core::import::{self, CvFile, CvRead};
use cv_core::jsonresume;

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(path)).expect("fixture")
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn europass(bytes: &[u8], ext: &str) -> CvRead {
    match import::cv_from_bytes(bytes, ext).expect("import") {
        CvFile::Europass(r) => r,
        _ => panic!("not read as Europass"),
    }
}

/// CVBuilder's own PDF with one more stream: an attached file with this dictionary and content.
fn pdf_with_stream(dict: lopdf::Dictionary, content: Vec<u8>) -> Vec<u8> {
    let pdf = cv_core::document::to_pdf(&cv_core::document::build(&Project::example_en())).expect("pdf");
    let mut doc = lopdf::Document::load_mem(&pdf).expect("load");
    doc.add_object(lopdf::Stream::new(dict, content).with_compression(false));
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

fn embedded(filter: Option<&str>) -> lopdf::Dictionary {
    use lopdf::Object;
    let mut d = lopdf::Dictionary::from_iter([("Type", Object::Name(b"EmbeddedFile".to_vec()))]);
    if let Some(f) = filter {
        d.set("Filter", Object::Name(f.as_bytes().to_vec()));
    }
    d
}

fn zlib(data: impl std::io::Read) -> Vec<u8> {
    let mut enc = flate2::read::ZlibEncoder::new(data, flate2::Compression::best());
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut enc, &mut out).expect("pack");
    out
}

const CANDIDATE: &str = "<?xml version=\"1.0\"?><Candidate xmlns=\"http://www.europass.eu/1.0\">";

// ---------- Europass ----------

#[test]
fn deeply_nested_xml_is_refused_not_a_crash() {
    // 100 000 nested tags used to overflow the reader's stack and abort the app.
    let depth = 100_000;
    let xml = format!("{CANDIDATE}{}{}</Candidate>", "<a>".repeat(depth), "</a>".repeat(depth));
    let e = import::cv_from_bytes(xml.as_bytes(), "xml").err().expect("refused");
    assert!(e.contains("Europass"), "{e}");
    // The same XML attached to a PDF: not read as Europass, and no crash.
    let pdf = pdf_with_stream(embedded(Some("FlateDecode")), zlib(xml.as_bytes()));
    assert!(!matches!(import::cv_from_bytes(&pdf, "pdf"), Ok(CvFile::Europass(_))));
}

#[test]
fn nesting_a_europass_cv_needs_is_read() {
    // 90 levels of wrapping still reads; the limit is far above what a CV uses.
    let xml = format!(
        "{CANDIDATE}<CandidatePerson><PersonName><GivenName>Kari</GivenName><FamilyName>Eksempel</FamilyName></PersonName></CandidatePerson><CandidateProfile languageCode=\"en\"><ExecutiveSummary>Planner.</ExecutiveSummary>{}{}</CandidateProfile></Candidate>",
        "<w>".repeat(90),
        "</w>".repeat(90)
    );
    assert_eq!(europass(xml.as_bytes(), "xml").project.candidate.name, "Kari Eksempel");
}

#[test]
fn too_many_elements_are_refused() {
    let xml = format!("{CANDIDATE}{}</Candidate>", "<x/>".repeat(250_000));
    let e = import::cv_from_bytes(xml.as_bytes(), "xml").err().expect("refused");
    assert!(e.contains("Europass"), "{e}");
}

#[test]
fn long_lists_are_cut_and_read_quickly() {
    // 10 000 skills and links, every one different: read in time, and no more than 500 kept.
    let skills: String = (0..10_000).map(|i| format!("<PersonCompetency><CompetencyName>Skill {i}</CompetencyName></PersonCompetency>")).collect();
    let web: String = (0..10_000).map(|i| format!("<Communication><ChannelCode>Web</ChannelCode><URI>https://example.org/{i}</URI></Communication>")).collect();
    let xml = format!(
        "{CANDIDATE}<CandidatePerson><PersonName><GivenName>Kari</GivenName></PersonName>{web}</CandidatePerson><CandidateProfile languageCode=\"en\"><ExecutiveSummary>Planner.</ExecutiveSummary><Skills>{skills}</Skills></CandidateProfile></Candidate>"
    );
    let started = Instant::now();
    let r = europass(xml.as_bytes(), "xml");
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    assert_eq!(r.project.skills.len(), 500);
    assert_eq!(r.project.skills[0].name, "Skill 0");
    assert!(r.project.candidate.extra.len() <= 500);
}

#[test]
fn a_flate_bomb_in_an_attachment_is_passed_over() {
    // 200 MB of spaces packs to about a megabyte; it must not be unpacked whole.
    let bomb = zlib(std::io::Read::take(std::io::repeat(b' '), 200_000_000));
    assert!(bomb.len() < 5_000_000, "{}", bomb.len());
    let pdf = pdf_with_stream(embedded(Some("FlateDecode")), bomb);
    let started = Instant::now();
    assert_eq!(cv_core::europass::xml_in_pdf(&pdf), None);
    // The PDF is still read as the ordinary PDF it is.
    assert!(matches!(import::cv_from_bytes(&pdf, "pdf"), Ok(CvFile::Text(_))));
    assert!(started.elapsed() < Duration::from_secs(20), "{:?}", started.elapsed());
}

#[test]
fn a_bomb_in_an_object_stream_is_never_unpacked() {
    // Loading the whole PDF unpacked every object stream without a limit; the attachment is now
    // found in the bytes, so a 200 MB object stream next to it is never touched.
    let bomb = zlib(std::io::Read::take(std::io::repeat(b'0'), 200_000_000));
    let xml = fixture("europass/candidate-en.xml");
    let packed = zlib(&xml[..]);
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.extend(format!("1 0 obj\n<</Type/ObjStm/N 1/First 2/Filter/FlateDecode/Length {}>>\nstream\n", bomb.len()).into_bytes());
    pdf.extend(&bomb);
    pdf.extend(b"\nendstream\nendobj\n");
    pdf.extend(format!("2 0 obj\n<</Type /EmbeddedFile /Subtype /text#2Fxml /Filter /FlateDecode /Length {}>>\nstream\r\n", packed.len()).into_bytes());
    pdf.extend(&packed);
    pdf.extend(b"\r\nendstream\nendobj\n%%EOF\n");
    let started = Instant::now();
    assert!(cv_core::europass::xml_in_pdf(&pdf).is_some_and(|t| t.contains("Esempio")));
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

#[test]
fn many_small_bombs_are_bounded() {
    // Fifty attachments, each unpacking to 200 MB of text that is not Europass: only a little of
    // each (and of at most twenty) is unpacked.
    let bomb = zlib(std::io::Read::take(std::io::repeat(b'x'), 200_000_000));
    let mut pdf = b"%PDF-1.7\n".to_vec();
    for n in 1..=50 {
        pdf.extend(format!("{n} 0 obj\n<</Type/EmbeddedFile/Filter/FlateDecode>>\nstream\n").into_bytes());
        pdf.extend(&bomb);
        pdf.extend(b"\nendstream\nendobj\n");
    }
    let started = Instant::now();
    assert_eq!(cv_core::europass::xml_in_pdf(&pdf), None);
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

#[test]
fn attachments_of_a_normal_size_are_still_read() {
    let xml = fixture("europass/candidate-en.xml");
    for (filter, content) in [(Some("FlateDecode"), zlib(&xml[..])), (None, xml.clone())] {
        let pdf = pdf_with_stream(embedded(filter), content);
        assert!(cv_core::europass::xml_in_pdf(&pdf).is_some_and(|t| t.contains("Esempio")), "{filter:?}");
    }
    // A filter CVBuilder does not unpack is passed over.
    let pdf = pdf_with_stream(embedded(Some("LZWDecode")), xml);
    assert_eq!(cv_core::europass::xml_in_pdf(&pdf), None);
}

// ---------- JSON Resume ----------

/// A real 1×1 JPEG (as made by CVBuilder's photo code).
fn tiny_jpeg() -> Vec<u8> {
    let mut img = Vec::new();
    // Baseline JPEG, 1×1, one grey component.
    img.extend_from_slice(&[0xFF, 0xD8]);
    img.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
    img.extend_from_slice(&[1u8; 64]);
    img.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00]);
    img.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x14, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00]);
    img.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00]);
    img.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x00, 0xFF, 0xD9]);
    img
}

fn resume_with_image(image: &str) -> String {
    serde_json::json!({ "basics": { "name": "Sam Example", "label": "Developer", "summary": "Builds web apps.", "image": image } }).to_string()
}

#[test]
fn only_a_sensible_jpeg_becomes_the_photo() {
    let jpeg = format!("data:image/jpeg;base64,{}", b64(&tiny_jpeg()));
    let r = jsonresume::read(&resume_with_image(&jpeg)).expect("read");
    assert_eq!(r.project.candidate.photo, jpeg);
    assert!(r.project.options.photo);

    // Not a JPEG, not a picture, broken, claiming a giant size, or too large: left out, and its
    // text is not put in front of the person either.
    let mut giant = tiny_jpeg();
    let sof = giant.windows(2).position(|w| w == [0xFF, 0xC0]).expect("SOF");
    giant[sof + 5..sof + 9].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    for bad in [
        format!("data:image/png;base64,{}", b64(b"\x89PNG\r\n\x1a\nnot really")),
        format!("data:image/svg+xml;base64,{}", b64(b"<svg/>")),
        "data:image/jpeg;base64,!!!not base64!!!".to_string(),
        format!("data:image/jpeg;base64,{}", b64(b"no jpeg here")),
        format!("data:image/jpeg;base64,{}", b64(&giant)),
        format!("data:image/jpeg;base64,{}", "A".repeat(3_000_000)),
    ] {
        let r = jsonresume::read(&resume_with_image(&bad)).expect("read");
        assert!(r.project.candidate.photo.is_empty(), "{bad:.60}");
        assert!(!r.project.options.photo, "{bad:.60}");
        assert!(!r.rest.contains("data:"), "{bad:.60}");
    }
}

#[test]
fn a_photo_switched_off_is_not_exported() {
    let mut p = Project::example_en();
    p.candidate.photo = format!("data:image/jpeg;base64,{}", b64(&tiny_jpeg()));
    p.options.photo = false;
    let v: serde_json::Value = serde_json::from_str(&jsonresume::write(&p)).expect("json");
    assert!(v["basics"].get("image").is_none(), "{}", v["basics"]);
    p.options.photo = true;
    let v: serde_json::Value = serde_json::from_str(&jsonresume::write(&p)).expect("json");
    assert_eq!(v["basics"]["image"], p.candidate.photo.as_str());
}

#[test]
fn huge_lists_and_unknown_keys_are_cut() {
    let skills: Vec<serde_json::Value> = (0..5_000).map(|i| serde_json::json!({ "name": format!("Group {i}"), "keywords": (0..50).map(|k| format!("Skill {i}-{k}")).collect::<Vec<_>>() })).collect();
    let work: Vec<serde_json::Value> = (0..5_000).map(|i| serde_json::json!({ "name": format!("Company {i}"), "position": "Developer" })).collect();
    let mut v = serde_json::json!({ "basics": { "name": "Sam Example" }, "work": work, "skills": skills });
    for i in 0..2_000 {
        v[format!("extra{i}")] = serde_json::Value::String("x".repeat(1_000));
    }
    let text = v.to_string();
    let started = Instant::now();
    let r = match import::cv_from_bytes(text.as_bytes(), "json").expect("import") {
        CvFile::JsonResume(r) => r,
        _ => panic!("not read as a JSON Resume"),
    };
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    assert_eq!(r.project.experience.len(), 500);
    assert_eq!(r.project.skills.len(), 500);
    assert!(r.rest.len() < 25_000, "{}", r.rest.len());
}

#[test]
fn a_huge_json_file_is_refused_by_its_size() {
    // Over the 50 MB limit: refused without being parsed.
    let dir = std::env::temp_dir().join(format!("cvbuilder-170-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("huge.json");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(b"{\"basics\":{\"name\":\"").expect("write");
        let chunk = vec![b'a'; 1_000_000];
        for _ in 0..51 {
            f.write_all(&chunk).expect("write");
        }
        f.write_all(b"\"}}").expect("write");
    }
    let e = import::json_resume_from_file(&path).err().expect("refused");
    assert!(e.contains("50 MB"), "{e}");
    // A CVBuilder draft is not a resume: it is left for the app to open as it is.
    let draft = dir.join("draft.json");
    std::fs::write(&draft, Project::example().to_json().expect("json")).expect("write");
    assert!(matches!(import::json_resume_from_file(&draft), Ok(None)));
    let resume = dir.join("resume.json");
    std::fs::write(&resume, fixture("jsonresume/resume.json")).expect("write");
    assert!(matches!(import::json_resume_from_file(&resume), Ok(Some(_))));
    let _ = std::fs::remove_dir_all(&dir);
}
