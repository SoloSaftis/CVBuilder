//! Must-haves and nice-to-haves read from real-looking ads, and where a CV shows them.
use cv_core::Project;
use cv_core::requirements::{Need, Place, ad_requirements, backing, summary};

fn ad(name: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ads").join(name)).expect("fixture")
}

fn find<'a>(list: &'a [cv_core::requirements::Requirement], start: &str) -> &'a cv_core::requirements::Requirement {
    list.iter().find(|r| r.text.starts_with(start)).unwrap_or_else(|| panic!("no «{start}» in {:#?}", list.iter().map(|r| (&r.text, r.need)).collect::<Vec<_>>()))
}

#[test]
fn a_norwegian_ad_splits_into_krav_and_fordeler() {
    let list = ad_requirements(&ad("elektriker-no.txt"));
    let texts: Vec<(&str, Need)> = list.iter().map(|r| (r.text.as_str(), r.need)).collect();
    assert_eq!(
        texts,
        vec![
            ("Fagbrev som elektriker (krav)", Need::Must),
            ("Minst 3 års erfaring fra service eller installasjon", Need::Must),
            ("Førerkort klasse B", Need::Must),
            ("Gode norskkunnskaper, muntlig og skriftlig", Need::Must),
            ("Erfaring med KNX og smarthus", Need::Nice),
            ("Kjennskap til NEK 400", Need::Nice),
            ("Det er en fordel om du har jobbet med solcelleanlegg", Need::Nice),
        ],
        "tasks, personal qualities, what they offer and the contact lines are no requirements"
    );
    assert!(find(&list, "Fagbrev").terms.iter().any(|t| t.to_lowercase() == "fagbrev"), "{:?}", find(&list, "Fagbrev").terms);
    assert!(find(&list, "Gode norsk").terms.iter().any(|t| t.starts_with("norsk/")), "{:?}", find(&list, "Gode norsk").terms);
    assert!(find(&list, "Kjennskap til NEK").terms.iter().any(|t| t == "NEK 400"), "{:?}", find(&list, "Kjennskap til NEK").terms);
    assert!(find(&list, "Erfaring med KNX").terms.iter().any(|t| t == "KNX"), "{:?}", find(&list, "Erfaring med KNX").terms);
}

#[test]
fn an_english_ad_splits_into_requirements_and_nice_to_haves() {
    let list = ad_requirements(&ad("data-engineer-en.txt"));
    let must: Vec<&str> = list.iter().filter(|r| r.need == Need::Must).map(|r| r.text.as_str()).collect();
    let nice: Vec<&str> = list.iter().filter(|r| r.need == Need::Nice).map(|r| r.text.as_str()).collect();
    assert_eq!(must, ["5+ years of experience as a data engineer", "Strong Python and SQL", "Experience with Apache Spark or Databricks", "Hands-on experience with AWS or Azure", "Fluent English, written and spoken"]);
    assert_eq!(nice, ["Terraform and Kubernetes", "Experience with Power BI", "A degree in computer science or similar", "Experience with Kafka is a plus"]);
    let sql = find(&list, "Strong Python");
    assert!(sql.terms.contains(&"Python".to_string()) && sql.terms.contains(&"SQL".to_string()), "{:?}", sql.terms);
    assert!(find(&list, "Fluent English").terms.iter().any(|t| t.contains("english")));
    assert_eq!(find(&list, "Experience with Apache").terms, ["Apache Spark/Databricks"], "either one backs it");
    assert_eq!(find(&list, "Hands-on").terms, ["AWS/Azure"]);
    assert_eq!(cv_core::requirements::label("Fluent English, written and spoken", &find(&list, "Fluent English").terms[0]), "English");
    assert_eq!(cv_core::requirements::label("x", "AWS/Azure"), "AWS / Azure");
}

#[test]
fn prose_lines_and_inline_headings_are_split_by_their_own_cues() {
    let list = ad_requirements(&ad("radgiver-no-prosa.txt"));
    let texts: Vec<(&str, Need)> = list.iter().map(|r| (r.text.as_str(), r.need)).collect();
    assert_eq!(
        texts,
        vec![
            ("Utdanning innen økonomi på bachelornivå", Need::Must),
            ("Erfaring med SAP er et krav", Need::Must),
            ("erfaring fra offentlig sektor er en fordel", Need::Nice),
            ("God kjennskap til Excel og budsjettarbeid", Need::Must),
            ("Erfaring med Power BI", Need::Nice),
        ]
    );
}

#[test]
fn an_ad_without_structure_gives_no_requirements() {
    let text = "Vi søker en blid og engasjert medarbeider til butikken vår på Lillestrøm. Du får jobbe i et hyggelig team med fokus på kundeservice. Send søknad i dag!";
    assert!(ad_requirements(text).is_empty());
    assert!(ad_requirements("").is_empty());
}

#[test]
fn other_languages_use_their_own_headings() {
    let sv = "Kvalifikationer\n- Erfarenhet av Java och Spring Boot\n- Goda kunskaper i svenska\n\nMeriterande\n- Erfarenhet av React\n\nVi erbjuder\n- Friskvårdsbidrag";
    let list = ad_requirements(sv);
    assert_eq!(list.iter().map(|r| r.need).collect::<Vec<_>>(), [Need::Must, Need::Must, Need::Nice], "{list:#?}");
    let de = "Ihr Profil\n- Abgeschlossene Ausbildung als Elektroniker\n- Erfahrung mit SPS (Siemens S7) ist von Vorteil\n\nWir bieten\n- 30 Tage Urlaub";
    let list = ad_requirements(de);
    assert_eq!(list.iter().map(|r| r.need).collect::<Vec<_>>(), [Need::Must, Need::Nice], "{list:#?}");
}

#[test]
fn the_cv_shows_where_each_requirement_is_backed() {
    let mut p = Project::example_en();
    p.experience[0].bullets.push("Built pipelines in Python and SQL on AWS".into());
    p.skills.push(cv_core::model::Skill { name: "Terraform".into(), ..Default::default() });
    p.skills.push(cv_core::model::Skill { name: "Kubernetes".into(), ..Default::default() });
    let list = ad_requirements(&ad("data-engineer-en.txt"));
    let sql = backing(&p, find(&list, "Strong Python"));
    assert!(sql.shown(), "{sql:?}");
    assert!(matches!(sql.places.first(), Some(Place::Job(_))), "a job is named first: {sql:?}");
    let cloud = backing(&p, find(&list, "Hands-on"));
    assert!(cloud.shown(), "AWS alone is enough for «AWS or Azure»: {cloud:?}");
    let tf = backing(&p, find(&list, "Terraform"));
    assert!(tf.shown() && tf.only_listed(), "only the skills list backs it: {tf:?}");
    let kafka = backing(&p, find(&list, "Experience with Kafka"));
    assert!(!kafka.shown() && kafka.missing.contains(&"Kafka".to_string()), "{kafka:?}");
    let (must_shown, must, nice_shown, nice) = summary(&p, &list);
    assert!(must >= 4 && nice >= 3 && must_shown <= must && nice_shown <= nice && nice_shown >= 1, "{must_shown}/{must} {nice_shown}/{nice}");
}

#[test]
fn norwegian_language_requirements_find_the_languages_section() {
    let mut p = Project::example();
    p.languages = vec![cv_core::model::SpokenLanguage { name: "Norsk".into(), level: "Morsmål".into() }];
    let list = ad_requirements(&ad("elektriker-no.txt"));
    let b = backing(&p, find(&list, "Gode norsk"));
    assert!(b.shown() && b.places.contains(&Place::Languages), "{b:?}");
}

