//! JSON Resume: reading a made-up `resume.json` (tests/jsonresume/), and writing a CV out and
//! reading it back without losing what the schema has a place for.
use std::path::Path;

use cv_core::Project;
use cv_core::import::{self, CvFile};
use cv_core::jsonresume;
use cv_core::model::{ContactField, CustomItem, CustomSection, Language};

fn fixture() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/jsonresume/resume.json")).expect("fixture")
}

#[test]
fn a_resume_json_fills_the_cv() {
    let bytes = fixture().into_bytes();
    let r = match import::cv_from_bytes(&bytes, "json").expect("import") {
        CvFile::JsonResume(r) => r,
        _ => panic!("not read as a JSON Resume"),
    };
    let p = &r.project;
    let c = &p.candidate;
    assert_eq!(p.language, Language::En);
    assert_eq!((c.name.as_str(), c.headline.as_str(), c.city.as_str()), ("Sam Example", "Frontend Developer", "Manchester"));
    assert_eq!((c.email.as_str(), c.phone.as_str()), ("sam.example@example.com", "+44 7700 900123"));
    assert_eq!(c.linkedin, "https://www.linkedin.com/in/sam-example");
    assert!(c.extra.iter().any(|e| e.id == "web" && e.value == "https://sam.example.com"));
    assert!(c.extra.iter().any(|e| e.label == "GitHub" && e.value == "https://github.com/sam-example"));
    // A photo on the web is not fetched; it is left for the person.
    assert!(c.photo.is_empty() && r.rest.contains("https://example.com/photo.jpg"));

    assert_eq!(p.experience.len(), 2);
    let j = &p.experience[0];
    assert_eq!((j.company.as_str(), j.title.as_str(), j.start.as_str(), j.end.clone()), ("Example Digital Ltd", "Frontend Developer", "2021-03", None));
    assert_eq!(j.bullets.len(), 3, "summary and highlights: {:?}", j.bullets);
    assert_eq!(p.experience[1].end.as_deref(), Some("2021-02"));

    let e = &p.education[0];
    assert_eq!((e.qualification.as_str(), e.institution.as_str(), e.start.as_str(), e.end.as_str()), ("BSc, Computer Science", "Example University", "2016-09", "2019-06"));
    assert!(r.rest.contains("First") && r.rest.contains("Databases"), "score and courses go to the person: {}", r.rest);

    assert_eq!(p.skills.iter().map(|s| (s.group.as_str(), s.name.as_str())).collect::<Vec<_>>(), [("Frontend", "TypeScript"), ("Frontend", "React"), ("Frontend", "CSS"), ("", "Accessibility testing")]);
    assert_eq!(p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect::<Vec<_>>(), [("English", "Native speaker"), ("French", "Intermediate")]);
    assert_eq!((p.certifications[0].issuer.as_str(), p.certifications[0].year.as_str()), ("Example Association", "2022"));
    assert_eq!(p.interests[0].label, "Climbing");
    assert_eq!(p.interests[0].tags, ["bouldering", "outdoor"]);

    let titles: Vec<&str> = p.custom_sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Volunteering", "Awards", "Publications", "Projects"]);
    assert_eq!(p.custom_sections[0].items[0].date, "01.2020 – Present");
    assert_eq!(p.custom_sections[1].items[0].date, "11.2023");
    let project = &p.custom_sections[3].items[0];
    assert_eq!((project.title.as_str(), project.subtitle.as_str(), project.date.as_str()), ("Bus times app", "Author", "01.2022 – 06.2022"));
    assert!(r.rest.contains("Jane Referee"), "references go to the person");
    assert!(r.found.iter().any(|f| f == "jobber (2)"));
}

#[test]
fn a_cvbuilder_project_is_not_a_json_resume() {
    assert!(!jsonresume::is_json_resume(&Project::example().to_json().expect("json")));
    assert!(jsonresume::is_json_resume(&fixture()));
    assert!(!jsonresume::is_json_resume("[1, 2]"));
}

/// What survives the trip: everything except CVBuilder's own notes and ids.
fn comparable(p: &Project) -> Project {
    let mut p = p.clone();
    p.target = Default::default();
    p.options = Default::default();
    p.public_context.clear();
    // Contact ids only tie a line to the contact order, which is CVBuilder's own.
    for (i, e) in p.candidate.extra.iter_mut().enumerate() {
        if e.id != "web" {
            e.id = format!("contact_imported_{}", i + 1);
        }
    }
    for (i, j) in p.experience.iter_mut().enumerate() {
        j.id = format!("job_imported_{}", i + 1);
        j.evidence.clear();
        j.confirmed = true;
        if j.end.as_deref() == Some("") {
            j.end = None;
        }
    }
    for s in &mut p.skills {
        s.evidence.clear();
        s.confirmed = true;
    }
    for (i, s) in p.custom_sections.iter_mut().enumerate() {
        s.id = format!("section_imported_{}", i + 1);
    }
    p.interests = p.interests.iter().enumerate().map(|(i, x)| cv_core::model::Interest { id: format!("interest_imported_{}", i + 1), label: x.label.clone(), tags: x.tags.clone(), category: "other".into(), specificity: 1, confirmed: true, ..Default::default() }).collect();
    p
}

fn with_extras(mut p: Project) -> Project {
    p.candidate.extra = vec![
        ContactField { id: "web".into(), label: String::new(), value: "https://example.com".into() },
        ContactField { id: "c2".into(), label: "GitHub".into(), value: "https://github.com/example".into() },
        ContactField { id: "c3".into(), label: if p.is_english() { "Driving licence" } else { "Førerkort" }.into(), value: "B".into() },
    ];
    let item = |title: &str, sub: &str, date: &str| CustomItem { title: title.into(), subtitle: sub.into(), date: date.into(), bullets: vec![format!("{title} – one line")] };
    let en = p.is_english();
    p.custom_sections = vec![
        CustomSection { id: "s1".into(), title: if en { "Volunteering" } else { "Frivillig arbeid" }.into(), items: vec![item("Coach", "Example club", "2019 – nå"), item("Board", "Example union", "spring 2020")] },
        CustomSection { id: "s2".into(), title: if en { "Awards" } else { "Utmerkelser" }.into(), items: vec![item("Prize", "Example jury", "2022")] },
        CustomSection { id: "s3".into(), title: if en { "Publications" } else { "Publikasjoner" }.into(), items: vec![item("Article", "Example press", "")] },
        CustomSection { id: "s4".into(), title: "Prosjekter".into(), items: vec![item("App", "Self", "03.2021 – 06.2022")] },
        CustomSection { id: "s5".into(), title: "Talks".into(), items: vec![item("Keynote", "Example conf", "2023")] },
    ];
    // Dates are written as other imports write them.
    if en {
        p.custom_sections[0].items[0].date = "2019 – Present".into();
    }
    p
}

#[test]
fn a_cv_written_as_json_resume_reads_back_the_same() {
    for p in [with_extras(Project::example()), with_extras(Project::example_en())] {
        let json = jsonresume::write(&p);
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(v["$schema"], jsonresume::SCHEMA);
        assert!(v.get("target").is_none() && v.get("options").is_none(), "the ad and the design are not part of a resume");
        let back = jsonresume::read(&json).expect("read back");
        assert_eq!(comparable(&back.project), comparable(&p), "{json}");
    }
}

#[test]
fn a_resume_json_written_back_is_the_same() {
    let first = jsonresume::read(&fixture()).expect("read");
    let again = jsonresume::read(&jsonresume::write(&first.project)).expect("read again");
    assert_eq!(comparable(&again.project), comparable(&first.project));
}
