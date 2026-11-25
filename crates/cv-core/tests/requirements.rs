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
    p.languages = vec![cv_core::model::SpokenLanguage { id: String::new(), name: "Norsk".into(), level: "Morsmål".into() }];
    let list = ad_requirements(&ad("elektriker-no.txt"));
    let b = backing(&p, find(&list, "Gode norsk"));
    assert!(b.shown() && b.places.contains(&Place::Languages), "{b:?}");
}


/// A one-term requirement, for asking what backs it.
fn req(term: &str) -> cv_core::requirements::Requirement {
    cv_core::requirements::Requirement { text: term.into(), need: Need::Must, terms: vec![term.into()] }
}

fn needs(ad: &str) -> Vec<(String, Need)> {
    ad_requirements(ad).into_iter().map(|r| (r.text, r.need)).collect()
}

#[test]
fn backing_matches_whole_words_only() {
    let mut p = Project::example_en();
    p.experience.truncate(1);
    p.education.clear();
    p.certifications.clear();
    p.custom_sections.clear();
    p.skills.clear();
    p.languages.clear();
    p.candidate.summary = "Polished, excellent communicator.".into();
    p.candidate.headline = "Frontend developer".into();
    p.experience[0].title = "Developer".into();
    p.experience[0].company = "Norsk Hydro".into();
    p.experience[0].bullets = vec!["Built apps in JavaScript and C++ on ASP.NET with Node.js".into(), "Ran data pipelines on Apache Spark".into()];
    for term in ["Java", "Excel", "Polish", "norsk/norwegian/norska/norsch", "C", "Spark SQL", "Go", "Apps Script"] {
        assert!(!backing(&p, &req(term)).shown(), "«{term}» must not be found inside another word or name");
    }
    for term in ["JavaScript", "C++", ".NET", "Node.js", "Apache Spark", "pipeline", "Spark"] {
        assert!(backing(&p, &req(term)).shown(), "«{term}» is in the CV as a whole word");
    }
    // A language is shown by the languages section, not by an employer's name.
    p.languages = vec![cv_core::model::SpokenLanguage { id: String::new(), name: "Norwegian".into(), level: "Native".into() }];
    let b = backing(&p, &req("norsk/norwegian/norska/norsch"));
    assert!(b.shown() && b.places == [Place::Languages], "{b:?}");
}

#[test]
fn hedges_are_nice_to_haves() {
    let no = "Kvalifikasjoner\n• Fagbrev som tømrer\n• Du bør ha erfaring med prosjektledelse\n• Gjerne erfaring fra rehabilitering\n• Du kan gjerne ha kjennskap til BIM\n• Førerkort klasse BE (fordel)\n• Erfaring med Revit er kjekt";
    assert_eq!(
        needs(no),
        [
            ("Fagbrev som tømrer".to_string(), Need::Must),
            ("Du bør ha erfaring med prosjektledelse".into(), Need::Nice),
            ("Gjerne erfaring fra rehabilitering".into(), Need::Nice),
            ("Du kan gjerne ha kjennskap til BIM".into(), Need::Nice),
            ("Førerkort klasse BE (fordel)".into(), Need::Nice),
            ("Erfaring med Revit er kjekt".into(), Need::Nice),
        ]
    );
    let en = "Requirements\n- 3+ years with Go\n- Docker (nice to have)\n- Rust is a plus\n- GraphQL would be a bonus\n- AWS certification is an asset";
    let got = needs(en);
    assert_eq!(got.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [Need::Must, Need::Nice, Need::Nice, Need::Nice, Need::Nice], "{got:#?}");
}

#[test]
fn a_must_word_never_lifts_a_nice_block() {
    let ad = "Nice to have\n- Minimum 2 years with Kotlin\n- At least basic Spanish\n- Must be comfortable with on-call\n\nØnskelig\n• Minst 2 års erfaring med SAP er nødvendig";
    assert!(needs(ad).iter().all(|(_, n)| *n == Need::Nice), "{:#?}", needs(ad));
    assert_eq!(needs(ad).len(), 4);
}

#[test]
fn a_block_ends_at_prose_and_key_value_lines() {
    let ad = "Kvalifikasjoner\n• Fagbrev som elektriker\n• Førerkort klasse B\nVi holder til i nye lokaler sentralt i byen med kort vei til kollektivtransport.\nKonkurransedyktige betingelser\n\nRequirements\nPython\nStart date: ASAP\nFlexible hours\n\nKrav\nVi er et selskap i sterk vekst med over to hundre ansatte i fem land, og vi har store ambisjoner for de neste årene i Norden.\nGod lønn\nErfaring med SAP er et krav.";
    assert_eq!(
        needs(ad),
        [
            ("Fagbrev som elektriker".to_string(), Need::Must),
            ("Førerkort klasse B".into(), Need::Must),
            ("Python".into(), Need::Must),
            ("Erfaring med SAP er et krav".into(), Need::Must),
        ],
        "prose, «Key: value» lines and what follows them are no requirements; a cue still counts"
    );
}

#[test]
fn perks_and_benefits_are_no_requirements() {
    let no = "Krav\n• Bachelorgrad i økonomi\n\nFordeler:\n• Gode pensjonsordninger\n• Fleksibel arbeidstid\n• Treningstilskudd\n\nFordeler hos oss\n• Hytte på fjellet";
    assert_eq!(needs(no), [("Bachelorgrad i økonomi".to_string(), Need::Must)]);
    let en = "Requirements\n- Strong SQL\n\nBenefits:\n- Health insurance\n- Stock options are a plus for many\n\nPerks and benefits\n- Free lunch\n\nWhat you'll get\n- A MacBook";
    assert_eq!(needs(en), [("Strong SQL".to_string(), Need::Must)]);
    // «Fordel:» alone still means an advantage.
    assert_eq!(needs("Fordel:\n• Erfaring med Azure"), [("Erfaring med Azure".to_string(), Need::Nice)]);
}

#[test]
fn a_long_ad_is_read_quickly() {
    let mut text = String::new();
    // The same points again and again (each is listed once), so the whole ad is read, not just
    // up to the 40th requirement.
    for i in 0..500 {
        text.push_str(&format!("Om oss\nVi er et selskap nummer {i} med lange avsnitt om kultur, kunder, verdier og alt det andre som gjør oss til en god arbeidsplass for deg og familien din.\n\n"));
        text.push_str("Kvalifikasjoner\n• Erfaring med Python og SQL\n• Gode norskkunnskaper\n• Du bør ha førerkort\nVi gleder oss til å høre fra deg, og vi lover å svare alle som søker innen fristen.\n\nNice to have\n- Kubernetes\n- Minimum 2 years with Go\n\nBenefits\n- Pension\nSøknadsfrist: 01.01.2027\n\n");
    }
    text.push_str("Krav\n• Fagbrev som elektriker\n");
    assert!(text.lines().count() > 8000);
    let p = Project::example_en();
    let start = std::time::Instant::now();
    let list = ad_requirements(&text);
    for r in &list {
        let _ = backing(&p, r);
    }
    let _ = summary(&p, &list);
    let took = start.elapsed();
    assert_eq!(list.len(), 6, "{list:#?}");
    assert_eq!(list.last().map(|r| r.text.as_str()), Some("Fagbrev som elektriker"), "the last line of the ad was read");
    assert!(took < std::time::Duration::from_secs(2), "a {}-line ad took {took:?}", text.lines().count());
}
