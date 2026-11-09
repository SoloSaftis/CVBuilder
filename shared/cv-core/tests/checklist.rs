//! The person's own checklist: automatic points, ticked points, files and their checks.
//! All people, firms and data here are made up.
use cv_core::checklist::{self, Checklist, Item, Kind, State};
use cv_core::model::{ContactField, CustomItem, CustomSection, Interest, Job, Skill, SpokenLanguage};
use cv_core::{Language, Project, checks};

fn item(id: &str, kind: Kind, value: &str) -> Item {
    Item { id: id.into(), text: format!("Punkt {id}"), kind, value: value.into() }
}

fn checklist_of(items: Vec<Item>) -> Checklist {
    Checklist { items, ..Default::default() }
}

/// The state of one point of `items` for this CV.
fn state(p: &Project, pages: usize, kind: Kind, value: &str) -> State {
    checklist::evaluate(&checklist_of(vec![item("x", kind, value)]), p, pages, &[])[0].1
}

#[test]
fn automatic_and_manual_points_are_evaluated() {
    let p = Project::example();
    let list = Checklist::starter();
    assert!(list.problems().is_empty());
    let states = checklist::evaluate(&list, &p, 1, &[]);
    let by = |id: &str| states.iter().find(|(i, _)| i.id == id).map(|(_, s)| *s).expect(id);
    assert_eq!(by("c1"), State::Done, "the example has LinkedIn");
    assert_eq!(by("c2"), State::Done, "one page is within two");
    assert_eq!(by("c3"), State::Done, "no buzzword");
    assert_eq!(by("c4"), State::Open, "manual points start open");
    let states = checklist::evaluate(&list, &p, 3, &["c4".to_string()]);
    assert_eq!(states.iter().find(|(i, _)| i.id == "c4").map(|(_, s)| *s), Some(State::Done));
    assert_eq!(states.iter().find(|(i, _)| i.id == "c2").map(|(_, s)| *s), Some(State::NotDone));

    let mine = Checklist { items: vec![Item { id: "x".into(), text: "Nevner Power BI".into(), kind: Kind::Contains, value: "Power BI".into() }, Item { id: "y".into(), text: "Nevner SAP".into(), kind: Kind::Contains, value: "SAP/S4".into() }], ..Default::default() };
    let s = checklist::evaluate(&mine, &p, 1, &[]);
    assert_eq!((s[0].1, s[1].1), (State::Done, State::NotDone));
}

#[test]
fn contains_ignores_case_and_handles_norwegian_letters_and_phrases() {
    let p = Project::example();
    for word in ["POWER bi", "power-BI", "INNKJØP", "økonomi og administrasjon", "Økonomi", "trondheim", "Prosjektkoordinator", "HMS", "SAP/Excel", "  excel  "] {
        assert_eq!(state(&p, 1, Kind::Contains, word), State::Done, "{word}");
        assert_eq!(state(&p, 1, Kind::NotContains, word), State::NotDone, "{word}");
    }
    for word in ["SAP", "økonomi og ledelse", "Kubernetes", "æøå"] {
        assert_eq!(state(&p, 1, Kind::Contains, word), State::NotDone, "{word}");
        assert_eq!(state(&p, 1, Kind::NotContains, word), State::Done, "{word}");
    }
}

#[test]
fn contains_reaches_every_part_of_the_cv() {
    let mut p = Project::default();
    p.candidate.name = "Kari Fjordheim".into();
    p.candidate.headline = "Lagerleder | Truckfører".into();
    p.candidate.email = "kari@fjordpost.example".into();
    p.candidate.phone = "+47 400 00 001".into();
    p.candidate.city = "Ålesund, Norge".into();
    p.candidate.linkedin = "https://www.linkedin.com/in/kari-fjordheim-eksempel".into();
    p.candidate.extra.push(ContactField { id: "web".into(), label: "Nettside".into(), value: "karisblogg.example".into() });
    p.candidate.summary = "Erfaren med varemottak.".into();
    p.skills.push(Skill { group: "Verktøy".into(), name: "Truckførerbevis T4".into(), evidence: String::new(), confirmed: true });
    p.experience.push(Job { id: "j1".into(), company: "Havlager AS (fiktiv)".into(), title: "Lagermedarbeider".into(), location: "Molde".into(), start: "2020-01".into(), end: None, bullets: vec!["Plukket ordre med håndterminal.".into()], ..Default::default() });
    p.languages.push(SpokenLanguage { name: "Tysk".into(), level: "grunnleggende".into() });
    p.interests.push(Interest { id: "i1".into(), label: "Seiling".into(), confirmed: true, ..Default::default() });
    p.options.include_interests = true;
    p.options.interest_ids = vec!["i1".into()];
    p.custom_sections.push(CustomSection { id: "frivillig".into(), title: "Frivillig arbeid".into(), items: vec![CustomItem { title: "Kasserer".into(), subtitle: "Idrettslaget Eksempel".into(), date: "2019".into(), bullets: vec!["Førte regnskapet for dugnadskassen.".into()] }] });
    for word in ["Fjordheim", "truckfører", "fjordpost", "ålesund", "linkedin", "karisblogg", "varemottak", "T4", "havlager", "lagermedarbeider", "Molde", "håndterminal", "tysk", "grunnleggende", "seiling", "frivillig arbeid", "kasserer", "idrettslaget", "dugnadskassen"] {
        assert_eq!(state(&p, 1, Kind::Contains, word), State::Done, "{word} is in the CV");
    }
    // Headings and the «nå» of a current job are in every CV, so they say nothing about this one.
    for word in ["arbeidserfaring", "kompetanse", "nå", "profil"] {
        assert_eq!(state(&p, 1, Kind::Contains, word), State::NotDone, "{word} is only a heading or date");
    }
    // Interests the person has not chosen to show are not in the CV.
    p.options.include_interests = false;
    assert_eq!(state(&p, 1, Kind::Contains, "seiling"), State::NotDone);
}

#[test]
fn contains_finds_text_with_combining_letters() {
    // «å» typed as «a» plus a ring, as text pasted from some PDFs comes.
    let mut p = Project::default();
    p.candidate.summary = "Ra\u{30a}dgiver innen o\u{308}konomi og cafe\u{301}drift.".into();
    for word in ["rådgiver", "økonomi", "ökonomi", "kafé", "café"] {
        let expected = if word == "økonomi" || word == "kafé" { State::NotDone } else { State::Done };
        assert_eq!(state(&p, 1, Kind::Contains, word), expected, "{word}");
    }
    assert_eq!(checks::normal("Ra\u{30a}dgiver"), checks::normal("Rådgiver"));
    // A mark with no letter before it is dropped; it neither joins nor splits words.
    assert_eq!(checks::normal("\u{30a}x - \u{301}y z\u{20dd}"), "x y z");
}

#[test]
fn max_pages_uses_the_real_page_count_and_waits_for_it() {
    let p = Project::example();
    assert_eq!(state(&p, 2, Kind::MaxPages, "2"), State::Done);
    assert_eq!(state(&p, 3, Kind::MaxPages, "2"), State::NotDone);
    assert_eq!(state(&p, 1, Kind::MaxPages, " 1 "), State::Done);
    assert_eq!(state(&p, 0, Kind::MaxPages, "1"), State::Open, "no page count yet");
}

#[test]
fn filled_checks_every_field_and_spaces_are_empty() {
    let full = Project::example();
    let mut blank = Project::example();
    let c = &mut blank.candidate;
    for f in [&mut c.name, &mut c.headline, &mut c.email, &mut c.phone, &mut c.city, &mut c.linkedin, &mut c.summary] {
        *f = "  \t ".into();
    }
    blank.target.role = " ".into();
    blank.target.keywords = vec![" ".into(), String::new()];
    let mut with_photo = Project::example();
    with_photo.candidate.photo = "data:image/jpeg;base64,AAAA".into();
    for (key, _) in checklist::FIELDS {
        assert_eq!(state(&blank, 1, Kind::Filled, key), State::NotDone, "{key} of spaces is empty");
        let expected = if key == "photo" { State::NotDone } else { State::Done };
        assert_eq!(state(&full, 1, Kind::Filled, key), expected, "{key} in the example");
    }
    assert_eq!(state(&with_photo, 1, Kind::Filled, "photo"), State::NotDone, "a photo the CV does not show");
    with_photo.options.photo = true;
    assert_eq!(state(&with_photo, 1, Kind::Filled, "photo"), State::Done);
    assert_eq!(state(&full, 1, Kind::Filled, " linkedin "), State::Done);
    assert_eq!(state(&full, 1, Kind::Filled, "skostørrelse"), State::NotDone);
}

#[test]
fn ticks_follow_ids_survive_edits_and_go_with_deleted_points() {
    let p = Project::example();
    let mut before = Checklist::starter();
    let mut ticks = vec!["c4".to_string(), "c5".to_string(), "c4".to_string()];
    let mut after = before.clone();
    after.items[3].text = "Lest høyt for en venn".into();
    after.items.swap(3, 4);
    after.items.remove(3); // c5
    checklist::forget_removed(&before, &after, &mut ticks);
    assert_eq!(ticks, vec!["c4".to_string()], "the deleted point's tick goes, repeats go");
    let s = checklist::evaluate(&after, &p, 1, &ticks);
    assert_eq!(s.iter().find(|(i, _)| i.id == "c4").map(|(_, s)| *s), Some(State::Done), "an edited text keeps its tick");
    // Ticks of points of another list (not deleted here) are kept for when it comes back.
    let mut other = vec!["z9".to_string()];
    checklist::forget_removed(&before, &after, &mut other);
    assert_eq!(other, vec!["z9".to_string()]);

    // A new point never takes the id of one just deleted, or the old tick would tick it.
    before.items.pop();
    let id = before.new_id();
    assert!(!["c1", "c2", "c3", "c4", "c5"].contains(&id.as_str()), "{id}");
    for _ in 0..60 {
        let id = before.new_id();
        assert!(!before.items.iter().any(|i| i.id == id), "{id} is taken");
        before.items.push(item(&id, Kind::Manual, ""));
    }
    assert!(before.problems().is_empty(), "{:?}", before.problems());
}

#[test]
fn checklist_files_round_trip_and_explain_mistakes() {
    let list = Checklist::starter();
    let back = checklist::parse(&checklist::to_file(&list)).expect("valid");
    assert_eq!(back, list);
    let mut bad = list.clone();
    bad.items.push(Item { id: "z".into(), text: String::new(), kind: Kind::MaxPages, value: "many".into() });
    bad.items.push(Item { id: "w".into(), text: "Felt".into(), kind: Kind::Filled, value: "shoe size".into() });
    let err = checklist::parse(&checklist::to_file(&bad)).expect_err("bad");
    assert!(err.contains("Punkt 6 mangler tekst") && err.contains("1 til 10") && err.contains("Punkt 7"), "{err}");
    assert!(checklist::parse("{}").is_err());
}

#[test]
fn every_kind_round_trips_through_a_file() {
    let mine = Checklist {
        format: checklist::FORMAT.into(),
        name: "Søknader våren «2027»".into(),
        items: vec![item("a", Kind::Manual, ""), item("b", Kind::Contains, "Power BI/Excel"), item("c", Kind::NotContains, "løsningsorientert"), item("d", Kind::MaxPages, "3"), item("e", Kind::Filled, "email")],
    };
    let text = checklist::to_file(&mine);
    assert!(text.contains("\"not_contains\"") && text.contains("\"max_pages\""), "{text}");
    assert_eq!(checklist::parse(&text).expect("valid"), mine);
    // With a byte-order mark, as Notepad on Windows saves it.
    assert_eq!(checklist::parse(&format!("\u{feff}{text}")).expect("bom"), mine);
    assert_eq!(checklist::parse_bytes(format!("\u{feff}{text}").as_bytes()).expect("bom bytes"), mine);
}

#[test]
fn broken_files_are_explained_without_panicking() {
    let wrong_format = r#"{"format":"cvbuilder-design/1","name":"X","items":[]}"#;
    assert!(checklist::parse(wrong_format).expect_err("format").contains("«format» må være «cvbuilder-checklist/1»"));
    let no_format = r#"{"name":"X","items":[]}"#;
    assert!(checklist::parse(no_format).expect_err("no format").contains("«format»"));
    let unknown_kind = r#"{"format":"cvbuilder-checklist/1","name":"X","items":[{"id":"a","text":"T","kind":"sometimes"}]}"#;
    assert!(checklist::parse(unknown_kind).expect_err("kind").starts_with("Filen er ikke en CVBuilder-sjekkliste"));
    let unknown_field = r#"{"format":"cvbuilder-checklist/1","name":"X","items":[],"colour":"red"}"#;
    assert!(checklist::parse(unknown_field).is_err());
    let slash_only = r#"{"format":"cvbuilder-checklist/1","name":"X","items":[{"id":"a","text":"T","kind":"contains","value":" / "}]}"#;
    assert!(checklist::parse(slash_only).expect_err("slash").contains("Punkt 1: skriv ordet"));
    let too_many = Checklist { items: (0..101).map(|n| item(&format!("i{n}"), Kind::Manual, "")).collect(), ..Default::default() };
    assert!(checklist::parse(&checklist::to_file(&too_many)).expect_err("many").contains("høyst 100"));
    let huge = format!(r#"{{"format":"cvbuilder-checklist/1","name":"{}","items":[]}}"#, "x".repeat(checklist::MAX_FILE_BYTES));
    assert!(checklist::parse(&huge).expect_err("huge").contains("for stor"));
    assert!(checklist::parse_bytes(&[0x7b, 0xff, 0xfe, 0x7d]).expect_err("binary").contains("UTF-8"));
    // Every cut of a valid file is either read or explained, never a panic.
    let text = checklist::to_file(&Checklist::starter());
    for end in (0..text.len()).filter(|i| text.is_char_boundary(*i)) {
        let _ = checklist::parse(&text[..end]);
    }
    for junk in ["", "null", "[]", "42", "\"x\"", "{\"items\":null}", "{\"items\":[null]}", "{\"items\":[{}]}"] {
        assert!(checklist::parse(junk).is_err(), "{junk}");
    }
}

#[test]
fn hand_written_files_get_ids_and_a_name() {
    // A file without ids, with one id twice, and without a name: older or hand-made.
    let text = r#"{"format":"cvbuilder-checklist/1","items":[{"text":"Ett"},{"id":"a","text":"To"},{"id":"a","text":"Tre","kind":"max_pages","value":"2"},{"id":" ","text":"Fire"}]}"#;
    let list = checklist::parse(text).expect("repaired");
    let ids: Vec<&str> = list.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids[1], "a");
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), 4, "{ids:?}");
    assert!(ids.iter().all(|i| !i.trim().is_empty()), "{ids:?}");
    assert_eq!(list.name, "Min sjekkliste");
    assert_eq!(list.items[0].kind, Kind::Manual);
    // Ticking one of the two former «a» points does not tick the other.
    let s = checklist::evaluate(&list, &Project::example(), 1, &["a".to_string()]);
    assert_eq!(s.iter().filter(|(_, st)| *st == State::Done).count(), 2, "«a» and the page point");
    // In the editor two points with one id are a problem to fix.
    let twice = checklist_of(vec![item("a", Kind::Manual, ""), item("a", Kind::Manual, "")]);
    assert!(twice.problems().iter().any(|p| p.contains("Punkt 2 har ingen egen id")), "{:?}", twice.problems());
    let unnamed = Checklist { name: "  ".into(), ..Default::default() };
    assert!(unnamed.problems().iter().any(|p| p.contains("navn")));
}

#[test]
fn ticks_are_saved_with_the_cv_only_when_used() {
    let mut p = Project::example();
    assert!(!p.to_json().expect("json").contains("checklist_ticks"));
    p.options.checklist_ticks = vec!["c4".into()];
    let back = Project::from_json(&p.to_json().expect("json")).expect("read");
    assert_eq!(back.options.checklist_ticks, vec!["c4".to_string()]);
}

// ---------- the built-in «Standard» checks ----------

fn texts(p: &Project, pages: usize) -> Vec<(checks::Level, String)> {
    checks::audit(p, pages, 2026 * 12 + 10).0.into_iter().map(|c| (c.level, c.text)).collect()
}

fn passes(p: &Project, text: &str) -> bool {
    texts(p, 1).iter().any(|(l, t)| *l == checks::Level::Pass && t == text)
}

#[test]
fn the_example_cv_has_no_false_alarms() {
    let p = Project::example();
    let m = checks::keyword_match(&p);
    // The example leaves out one keyword on purpose, to show what a missing one looks like.
    assert_eq!(m.missing, vec!["ISY ProArc".to_string()], "{m:?}");
    assert_eq!(m.coverage, Some(89));
    for text in ["Kontaktinformasjon står som vanlig tekst øverst", "Telefonnummeret har landskode", "LinkedIn-profil er oppgitt", "Overskriften speiler stillingen", "Ingen tomme moteord", "Ingen unødvendige personopplysninger", "Ingen uforklarte opphold over seks måneder", "Punktene starter med handling", "Punktene er korte nok til å skannes"] {
        assert!(passes(&p, text), "{text}: {:?}", texts(&p, 1));
    }
}

#[test]
fn keywords_match_inflections_case_punctuation_and_repeats() {
    let mut p = Project::example();
    p.target.keywords = vec![
        "Fremdriftsplan".into(),      // the CV says «fremdriftsplaner»
        "leverandørene".into(),       // the CV says «leverandører»
        "Overleveringsmøte".into(),   // the CV says «overleveringsmøter»
        "power-bi".into(),            // punctuation
        "EXCEL".into(),
        "excel".into(),               // the same keyword again
        "engelsk".into(),             // in the languages section
        "Kundene".into(),             // the CV says «kunder» and «Kunde»
        "Sykepleierne".into(),        // not in the CV in any form
    ];
    let m = checks::keyword_match(&p);
    assert_eq!(m.found.len() + m.missing.len(), 8, "«excel» counts once: {m:?}");
    assert_eq!(m.missing, vec!["Sykepleierne".to_string()], "{m:?}");
    // Endings are whole endings: a title does not match a different word with the same start.
    let haystack = checks::normal("Jobbet med utvikling av systemer og management");
    for (keyword, found) in [("utvikler", false), ("Developer", false), ("Manager", false), ("systemene", true), ("system", true), ("Systems", true), ("utviklingen", true)] {
        assert_eq!(checks::contains(&haystack, keyword), found, "{keyword}");
    }
}

#[test]
fn gaps_count_the_months_without_a_job() {
    let job = |company: &str, start: &str, end: &str| Job { id: company.into(), company: company.into(), title: "T".into(), start: start.into(), end: Some(end.into()), ..Default::default() };
    // Out after June, in again in January: July to December is six months, not more.
    let mut p = Project { experience: vec![job("A", "2009-08", "2011-06"), job("B", "2012-01", "2014-12")], ..Default::default() };
    assert!(checks::gaps(&p, 2026 * 12).is_empty(), "{:?}", checks::gaps(&p, 2026 * 12));
    p.experience.push(job("C", "2015-08", "2016-01"));
    assert_eq!(checks::gaps(&p, 2026 * 12), vec![("B".to_string(), "C".to_string(), 7)]);
}

#[test]
fn ordinary_words_are_not_buzzwords_or_private_details() {
    let mut p = Project::example();
    p.experience[0].bullets = vec![
        "I 2021 innførte jeg digitale sjekklister for arbeidsområder på byggeplassen.".into(),
        "Beregnet termodynamiske laster for 12 anlegg.".into(),
        "Sikret riktig lagring av gift og etsende stoffer.".into(),
    ];
    for text in ["Ingen tomme moteord", "Ingen unødvendige personopplysninger", "Punktene starter med handling"] {
        assert!(passes(&p, text), "{text}: {:?}", texts(&p, 1));
    }
    p.experience[0].bullets.push("Svært motiverte og engasjerte kolleger.".into());
    p.candidate.summary = format!("{} Gift, to barn.", p.candidate.summary);
    assert!(!passes(&p, "Ingen tomme moteord"));
    assert!(!passes(&p, "Ingen unødvendige personopplysninger"));
    // «I» is a weak start in English.
    let mut en = Project::example();
    en.language = Language::En;
    en.experience[0].bullets = vec!["I led the purchasing of 6 housing projects.".into()];
    assert!(!passes(&en, "Punktene starter med handling"), "{:?}", texts(&en, 1));
}

#[test]
fn empty_english_and_very_long_cvs_are_checked_without_trouble() {
    let empty = Project::default();
    let (list, m) = checks::audit(&empty, 1, 2026 * 12);
    assert!(!list.is_empty() && m.coverage.is_none());
    let s = checklist::evaluate(&Checklist::starter(), &empty, 1, &[]);
    assert_eq!(s.iter().map(|(_, s)| *s).collect::<Vec<_>>(), vec![State::NotDone, State::Done, State::Done, State::Open, State::Open]);
    let mut spaces = Project::default();
    spaces.candidate.name = " ".into();
    spaces.candidate.email = " ".into();
    spaces.candidate.phone = " ".into();
    spaces.candidate.city = " ".into();
    spaces.candidate.linkedin = " ".into();
    assert!(!passes(&spaces, "Kontaktinformasjon står som vanlig tekst øverst"));
    assert!(!passes(&spaces, "LinkedIn-profil er oppgitt"));

    let mut en = Project::example();
    en.language = Language::En;
    en.target.keywords = vec!["Purchasing".into(), "stakeholders".into()];
    en.experience[0].bullets = vec!["Led purchasing for 6 housing projects and reported to stakeholder groups.".into()];
    let m = checks::keyword_match(&en);
    assert!(m.missing.is_empty(), "{m:?}");

    let mut long = Project::example();
    for n in 0..40 {
        let mut j = long.experience[1].clone();
        j.id = format!("j{n}");
        j.bullets = (0..30).map(|b| format!("Fulgte opp {b} leverandører og {n} prosjekter med Power BI.")).collect();
        long.experience.push(j);
    }
    long.target.keywords = (0..80).map(|n| format!("fagord{n}")).collect();
    let started = std::time::Instant::now();
    let mut big = Checklist::starter();
    big.items.extend((0..90).map(|n| item(&format!("k{n}"), Kind::Contains, &format!("ord{n}/Power BI"))));
    for _ in 0..5 {
        let _ = checks::audit(&long, 9, 2026 * 12);
        let s = checklist::evaluate(&big, &long, 9, &[]);
        assert_eq!(s.iter().find(|(i, _)| i.id == "c2").map(|(_, s)| *s), Some(State::NotDone));
    }
    // Drawn every frame: must stay well under a frame's worth per call on any machine.
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
}
