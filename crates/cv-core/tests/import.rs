//! Reading an existing CV or a job ad: files, web pages and the readers that need no AI.
use cv_core::{Project, document, import};

fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cvbuilder-import-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write");
    path
}

#[test]
fn word_and_pdf_files_give_their_text_back() {
    let p = Project::example();
    let doc = document::build(&p);
    for (name, bytes) in [("cv.docx", document::to_docx(&doc).expect("docx")), ("cv.pdf", document::to_pdf(&doc).expect("pdf"))] {
        let text = import::text_from_file(&temp(name, &bytes)).expect(name);
        assert!(text.contains(&p.candidate.name), "{name}: name missing in {text:.200}");
        assert!(text.contains("Fjellvegg"), "{name}: employer missing");
        let read = import::read_cv(&text);
        assert_eq!(read.project.candidate.email, p.candidate.email, "{name}");
        assert_eq!(read.project.candidate.name, p.candidate.name, "{name}");
    }
}

#[test]
fn unreadable_files_are_explained() {
    assert!(import::text_from_file(&temp("x.xyz", b"hello")).expect_err("type").contains("PDF"));
    assert!(import::text_from_file(&temp("empty.txt", b"  ")).expect_err("empty").contains("tekst"));
    assert!(import::text_from_file(&temp("broken.pdf", b"%PDF-1.4 not really")).is_err(), "a broken PDF is an error, not a crash");
}

#[test]
fn a_job_ad_in_plain_text() {
    let ad = import::read_ad("Stilling: Applikasjonsingeniør\nArbeidsgiver: Eksempel Automasjon AS\n\nVi søker deg med erfaring fra PLS, TwinCAT og EtherCAT. Du har god kjennskap til PLS-programmering.");
    assert_eq!(ad.role, "Applikasjonsingeniør");
    assert_eq!(ad.company, "Eksempel Automasjon AS");
    assert!(ad.keywords.iter().any(|k| k == "TwinCAT"), "{:?}", ad.keywords);
    let plain = import::read_ad("Prosjektkoordinator\nVi har en spennende stilling.");
    assert_eq!(plain.role, "Prosjektkoordinator", "the first short line, without a label");
}

#[test]
fn a_job_ad_web_page_with_job_data() {
    let html = r#"<html><head><title>x</title><script type="application/ld+json">{"@context":"https://schema.org","@type":"JobPosting","title":"Prosjektleder bygg","hiringOrganization":{"@type":"Organization","name":"Fjord Bygg AS"},"description":"<p>Du får ansvar for <b>fremdriftsplaner</b> og HMS.</p><ul><li>Erfaring med ISY ProArc</li></ul>"}</script></head><body><nav>Meny</nav><p>Annet innhold</p></body></html>"#;
    let ad = import::ad_from_html(html);
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Prosjektleder bygg", "Fjord Bygg AS"));
    assert!(ad.description.contains("fremdriftsplaner") && !ad.description.contains("<b>"));
    let text = import::strip_html("<style>.a{}</style><p>Linje&nbsp;1</p><p>S&oslash;knadsfrist &amp; mer</p><script>x()</script>");
    assert_eq!(text, "Linje 1\nSøknadsfrist & mer");
}

#[test]
fn ad_fills_the_target_and_adds_only_skills_the_cv_already_shows() {
    let mut p = Project::example();
    p.target = Default::default();
    let ad = import::AdInfo { role: "Prosjektkoordinator".into(), company: "Nytt Firma AS".into(), description: "Ad text".into(), keywords: vec!["leiligheter".into(), "Excel".into(), "SAP".into()] };
    let summary = import::apply_ad(&mut p, &ad);
    assert_eq!(p.target.company, "Nytt Firma AS");
    let names: Vec<&str> = p.skills.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"leiligheter"), "it is in a job bullet, so it may go to skills: {names:?}");
    assert!(!names.contains(&"SAP"), "never something the CV does not show");
    assert_eq!(names.iter().filter(|n| **n == "Excel").count(), 0, "Excel is already a skill (inside «Excel (pivot, oppslag)»)");
    assert!(summary.contains("leiligheter"), "{summary}");
    // What the person typed is kept.
    p.target.role = "Min egen tittel".into();
    import::apply_ad(&mut p, &ad);
    assert_eq!(p.target.role, "Min egen tittel");
}

#[test]
fn an_existing_cv_without_ai() {
    let text = "Kari Nordmann\nkari.nordmann@example.no | +47 912 34 567 | linkedin.com/in/kari-nordmann\n\nProfil\nErfaren elektriker med fagbrev.\nLiker å løse problemer.\n\nKompetanse\nPLS, Elkraft • Feilsøking\n\nSpråk\nNorsk: morsmål\nEngelsk - flytende\n\nArbeidserfaring\nElektriker, Strøm AS, 2018–nå\n- Installerte anlegg\n";
    let read = import::read_cv(text);
    let c = &read.project.candidate;
    assert_eq!((c.name.as_str(), c.email.as_str()), ("Kari Nordmann", "kari.nordmann@example.no"));
    assert_eq!(c.phone, "+47 912 34 567");
    assert_eq!(c.linkedin, "https://linkedin.com/in/kari-nordmann");
    assert_eq!(c.summary, "Erfaren elektriker med fagbrev. Liker å løse problemer.");
    assert_eq!(read.project.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["PLS", "Elkraft", "Feilsøking"]);
    assert_eq!(read.project.languages[1].level, "flytende");
    let job = &read.project.experience[0];
    assert_eq!((job.title.as_str(), job.company.as_str(), job.start.as_str(), job.end.as_deref()), ("Elektriker", "Strøm AS", "2018", None));
    assert_eq!(job.bullets, ["Installerte anlegg"]);
}

/// The fictional example CV saved by Word in every format people send: each gives the same CV.
#[test]
fn cvs_in_word_doc_odt_rtf_pdf_and_unicode_text_fill_in_the_same() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cv-files");
    for name in ["example.docx", "example.doc", "example.odt", "example.rtf", "example.pdf", "example-utf16.txt"] {
        let text = import::text_from_file(&dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let r = import::read_cv(&text);
        let p = &r.project;
        let c = &p.candidate;
        assert_eq!(c.name, "Ingrid Haugland", "{name}");
        assert_eq!(c.email, "ingrid.haugland@example.no", "{name}");
        assert_eq!(c.phone, "+47 400 00 000", "{name}");
        assert_eq!(c.city, "Trondheim, Norge", "{name}");
        assert_eq!(c.headline, "Prosjektkoordinator | Innkjøp | Kundeoppfølging", "{name}");
        assert!(c.summary.starts_with("Prosjektkoordinator med sju års erfaring"), "{name}: {}", c.summary);
        let jobs: Vec<_> = p.experience.iter().map(|j| (j.title.as_str(), j.company.as_str(), j.location.as_str(), j.start.as_str(), j.end.as_deref(), j.bullets.len())).collect();
        assert_eq!(
            jobs,
            [("Prosjektkoordinator", "Fjellvegg Bygg AS (fiktiv)", "Trondheim", "2021-03", None, 3), ("Innkjøpsmedarbeider", "Byggvare Midt AS (fiktiv)", "Trondheim", "2018-01", Some("2021-02"), 2)],
            "{name}"
        );
        let e = &p.education[0];
        assert_eq!((e.qualification.as_str(), e.institution.as_str(), e.start.as_str(), e.end.as_str()), ("Bachelor i økonomi og administrasjon", "Eksempel høgskole", "2015", "2018"), "{name}");
        let cert = &p.certifications[0];
        assert_eq!((cert.name.as_str(), cert.issuer.as_str(), cert.year.as_str()), ("Prosjektledelse, grunnkurs", "Eksempel kursleverandør", "2022"), "{name}");
        let langs: Vec<_> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
        assert_eq!(langs, [("Norsk", "morsmål"), ("Engelsk", "flytende")], "{name}");
        assert_eq!(p.interests.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Langrenn", "Frivillig i idrettslag"], "{name}");
        assert!(p.skills.iter().any(|s| s.group == "Verktøy" && s.name == "Power BI"), "{name}");
        assert!(r.rest.is_empty(), "{name}: nothing left over, got {}", r.rest);
    }
}

/// English layouts from CV builders: a two-column sidebar, headings with the section's first
/// words on the same line, spaced-out headings, the employer above the title, page numbers.
#[test]
fn english_cvs_from_common_layouts() {
    let sidebar = "Skills\nProduct design\n\nInteraction design, Design systems,\nPrototyping\n\nResearch\n\nUsability testing, Interviewing, Journey\nmapping\n\n\
        Education\nUniversity of the Arts\nLondon\nMA\nBrand Communication\n2013 – 2017\n\nCertifications\nUX Design Professional\nCertificate\n2024\nGoogle\n\n\
        Languages\nEnglish\nFluent Dutch\nConversational\n\nInterests\nEditorial illustration\nAlex Example\n\nSenior Product Designer\n\n\
        alex.example@example.com +1 (202) 555-0110\nAmsterdam, Netherlands alexexample.example.com\n\nS U M M A RY\n\
        Senior Product Designer with eight years of experience in digital product design. Combines thoughtful problem solving with hands-\non delivery and clear communication.\n\n\
        E X P E R I E N C E\nNorthstar Health London, UK\nSenior Product Designer 2022 – Present\n• Led a patient booking redesign across web and mobile,\n\
        increasing completed appointments by 28% in six months.\n\n• Built an accessible component library used by six squads.\n\n\
        Studio Assembly London, UK\nProduct Designer Jan 2019 – Dec 2022\n• Designed self-service journeys for retail clients.\n\nAlex Example 1 / 2\n\n\
        Projects Open Metrics Toolkit 2025\nPublished a lightweight dashboard starter used by 14 teams.\n\n\
        Community\nCommunity Skills Network\n2023 – Present\nMentored emerging designers\nthrough monthly portfolio reviews.\n";
    let r = import::read_cv(sidebar);
    let p = &r.project;
    let c = &p.candidate;
    assert_eq!((c.name.as_str(), c.headline.as_str(), c.city.as_str(), c.phone.as_str()), ("Alex Example", "Senior Product Designer", "Amsterdam, Netherlands", "+1 (202) 555-0110"));
    assert_eq!(c.extra[0].value, "alexexample.example.com");
    assert!(c.summary.contains("hands-on delivery"), "{}", c.summary);
    assert_eq!(p.language, cv_core::model::Language::En);
    let jobs: Vec<_> = p.experience.iter().map(|j| (j.title.as_str(), j.company.as_str(), j.location.as_str(), j.start.as_str(), j.end.as_deref(), j.bullets.len())).collect();
    assert_eq!(jobs, [("Senior Product Designer", "Northstar Health", "London, UK", "2022", None, 2), ("Product Designer", "Studio Assembly", "London, UK", "2019-01", Some("2022-12"), 1)]);
    assert_eq!(p.experience[0].bullets[0], "Led a patient booking redesign across web and mobile, increasing completed appointments by 28% in six months.");
    let e = &p.education[0];
    assert_eq!((e.qualification.as_str(), e.institution.as_str()), ("MA Brand Communication", "University of the Arts London"));
    assert_eq!((p.certifications[0].name.as_str(), p.certifications[0].issuer.as_str()), ("UX Design Professional Certificate", "Google"));
    let langs: Vec<_> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
    assert_eq!(langs, [("English", "Fluent"), ("Dutch", "Conversational")]);
    assert!(p.skills.iter().any(|s| s.group == "Research" && s.name == "Journey mapping"));
    assert_eq!(p.interests.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Editorial illustration"]);
    let sections: Vec<_> = p.custom_sections.iter().map(|s| (s.title.as_str(), s.items[0].title.as_str(), s.items[0].date.as_str())).collect();
    assert_eq!(sections, [("Projects", "Open Metrics Toolkit", "2025"), ("Volunteering", "Community Skills Network", "2023 – Present")]);
    assert_eq!(p.custom_sections[1].items[0].bullets, ["Mentored emerging designers through monthly portfolio reviews."]);
}

#[test]
fn plain_text_cvs_with_pipes_and_a_shared_language_level() {
    let text = "Kari Nordmann\nElektriker\nkari@example.no | 912 34 567 | Bergen\n\nARBEIDSERFARING\n\nElektriker | Strøm AS | Bergen\n01.2020 – nå\n- Installerte og feilsøkte anlegg i næringsbygg.\n\n\
        Lærling | Lys og Varme AS | Os\n08.2016 – 12.2019\n- Fikk fagbrev i 2019.\n\nUTDANNING\nVg3 Elektrikerfaget | Bergen tekniske fagskole | 2014 – 2016\n\n\
        SPRÅK\nNorsk og engelsk\nflytende skriftlig og muntlig\n";
    let r = import::read_cv(text);
    let p = &r.project;
    let jobs: Vec<_> = p.experience.iter().map(|j| (j.title.as_str(), j.company.as_str(), j.location.as_str(), j.start.as_str(), j.end.as_deref())).collect();
    assert_eq!(jobs, [("Elektriker", "Strøm AS", "Bergen", "2020-01", None), ("Lærling", "Lys og Varme AS", "Os", "2016-08", Some("2019-12"))]);
    assert_eq!((p.education[0].qualification.as_str(), p.education[0].institution.as_str()), ("Vg3 Elektrikerfaget", "Bergen tekniske fagskole"));
    let langs: Vec<_> = p.languages.iter().map(|l| (l.name.as_str(), l.level.as_str())).collect();
    assert_eq!(langs, [("Norsk", "flytende skriftlig og muntlig"), ("Engelsk", "flytende skriftlig og muntlig")]);
    assert_eq!(p.candidate.city, "Bergen");
}

// ---------- job ads: the shapes people paste ----------

fn has(keywords: &[String], term: &str) -> bool {
    keywords.iter().any(|k| k == term)
}

/// No key word twice (case-insensitively), none with a line break, none that is an ad word.
fn clean_keywords(keywords: &[String]) {
    let mut seen = std::collections::HashSet::new();
    for k in keywords {
        assert!(seen.insert(k.to_lowercase()), "«{k}» twice in {keywords:?}");
        assert!(!k.contains('\n') && k.trim() == k && !k.is_empty(), "«{k:?}» is not one clean term");
        let lower = k.to_lowercase();
        for word in ["stillingstittel", "arbeidsgiver", "kvalifikasjoner", "stillingen", "om", "søknadsfrist", "requirements", "nice", "about", "vi tilbyr", "arbeidsoppgaver", "firma"] {
            assert_ne!(lower, word, "an ad word as a key word: {keywords:?}");
        }
    }
    assert!(keywords.len() <= 25, "{} key words", keywords.len());
}

#[test]
fn a_finn_style_ad_with_headings_bullets_and_tech_names() {
    let text = "Ledig stilling\nSenior utvikler .NET\nNordlys Data AS\nOslo · Fast · Heltid\n\nOM STILLINGEN\nVi ser etter en erfaren utvikler med C#, .NET, Node.js og Azure DevOps.\n\nKVALIFIKASJONER\n• Erfaring med C# og .NET\n• Kjennskap til SAP S/4HANA og Power BI\n• TwinCAT 3 er en fordel\n• Feilsøking og dokumentasjon\n\nVi tilbyr\n- Gode betingelser\n- Fleksibel arbeidstid\n\nSøknadsfrist: 01.12.2026";
    let ad = import::read_ad(text);
    assert_eq!(ad.role, "Senior utvikler .NET", "«Ledig stilling» is page furniture");
    assert_eq!(ad.company, "Nordlys Data AS");
    for term in ["C#", ".NET", "Node.js", "Azure DevOps", "SAP S/4HANA", "Power BI", "TwinCAT", "feilsøking"] {
        assert!(has(&ad.keywords, term), "{term} missing from {:?}", ad.keywords);
    }
    for junk in ["BI", "power bi", "DevOps", "S/4HANA", "OM", "STILLINGEN", "KVALIFIKASJONER", "Nordlys", "Data"] {
        assert!(!has(&ad.keywords, junk), "«{junk}» in {:?}", ad.keywords);
    }
    clean_keywords(&ad.keywords);
}

#[test]
fn a_nav_style_ad_with_labels_crlf_tabs_and_numbered_lists() {
    let text = "Stillingstittel:\tSykepleier\r\nArbeidsgiver:\u{a0}Eksempel\u{a0}kommune\r\nArbeidssted: Bergen\r\n\r\nArbeidsoppgaver\r\n\t* Legemiddelhåndtering\r\n\t* Dokumentasjon i DIPS\r\nKvalifikasjoner\r\n1. Norsk autorisasjon som sykepleier\r\n2) Erfaring fra hjemmesykepleie\r\n";
    let ad = import::read_ad(text);
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Sykepleier", "Eksempel kommune"), "tabs and non-breaking spaces are spaces");
    for term in ["DIPS", "legemiddelhåndtering", "hjemmesykepleie"] {
        assert!(has(&ad.keywords, term), "{term} missing from {:?}", ad.keywords);
    }
    assert!(!ad.keywords.iter().any(|k| k.contains('\r')));
    clean_keywords(&ad.keywords);
    // NAV's page puts the label and the value on separate lines.
    let ad = import::read_ad("Om jobben\nStillingstittel\nBackend-utvikler\nArbeidsgiver\nStatens Eksempelbygg\nType ansettelse\nFast, heltid 100%\n\nVi bruker C#, Azure og GitHub.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Backend-utvikler", "Statens Eksempelbygg"));
    // «Stilling: 100 %» is not a title.
    let ad = import::read_ad("Stilling: 100 % fast\nStillingstittel: Lagermedarbeider\nVi søker en lagermedarbeider med truckførerbevis.");
    assert_eq!(ad.role, "Lagermedarbeider");
}

#[test]
fn an_english_linkedin_style_ad() {
    let text = "Data Engineer\nAcme Analytics · Stockholm (Hybrid)\n\nAbout the job\nWe are Acme Analytics, a fast growing company. 🚀\n\nRequirements\n- 3+ years with Python, SQL and Apache Spark\n- Experience with Kubernetes, Terraform and AWS\n\nNice to have\n* Power BI\n* Docker";
    let ad = import::read_ad(text);
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Data Engineer", "Acme Analytics"));
    for term in ["Python", "SQL", "Apache Spark", "Kubernetes", "Terraform", "AWS", "Power BI"] {
        assert!(has(&ad.keywords, term), "{term} missing from {:?}", ad.keywords);
    }
    assert!(!ad.keywords.iter().any(|k| k.eq_ignore_ascii_case("analytics")), "the employer's own name is not a key word: {:?}", ad.keywords);
    assert!(!has(&ad.keywords, "python"), "spelled as the ad spells it");
    clean_keywords(&ad.keywords);
    // Copied with an emoji in front of the title.
    let ad = import::read_ad("🚀 Frontend Developer\nNordic Example AB · Göteborg\nReact, TypeScript och CSS.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Frontend Developer", "Nordic Example AB"));
}

#[test]
fn ads_without_a_title_line_or_with_the_employer_only_in_the_text() {
    // An e-mail: greeting, a paragraph, a sign-off.
    let ad = import::read_ad("Hei,\n\nVi i Fjordkraft Eksempel søker en prosjektleder til vår avdeling i Trondheim. Du får ansvar for fremdrift og budsjett.\n\nMvh\nKari");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Prosjektleder", "Fjordkraft Eksempel"));
    assert!(!has(&ad.keywords, "Kari") && !has(&ad.keywords, "Mvh"));
    // The employer only as «X AS» in the text.
    let ad = import::read_ad("Elektriker\nVil du jobbe med installasjon? Hos Lysverk Eksempel AS får du varierte oppdrag.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Elektriker", "Lysverk Eksempel AS"));
    // «Om X» as a heading.
    let ad = import::read_ad("Regnskapsmedarbeider\n\nOm Tallknuserne\nVi er et regnskapsbyrå med 20 ansatte.");
    assert_eq!(ad.company, "Tallknuserne");
    // English sentences.
    let ad = import::read_ad("Are you ready for a new challenge? Example Robotics is looking for a Senior Embedded Engineer to join our team in Oslo.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Senior Embedded Engineer", "Example Robotics"));
    // Nothing to go on: nothing is invented.
    let ad = import::read_ad("Vi har det travelt og trenger hjelp. Ring oss på telefon så forteller vi mer om dette.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("", ""));
}

#[test]
fn key_words_skip_codes_phone_numbers_possessives_and_shouting() {
    let text = "Stilling: OSINT-utvikler\nArbeidsgiver: Eksempeltjenesten\n\nEksempeltjenestens OSINT-miljø vokser. PSTs og PST sine utviklere bruker Python og SQL.\nMå kunne sikkerhetsklareres for STRENGT HEMMELIG.\nKontakt: OT 90821144\nSKO 1087 Seniorrådgiver / SKO 1364 Seniorrådgiver";
    let ad = import::read_ad(text);
    for junk in ["PSTs", "SKO 1087", "SKO 1364", "STRENGT", "HEMMELIG", "OT 90821144"] {
        assert!(!has(&ad.keywords, junk), "«{junk}» in {:?}", ad.keywords);
    }
    assert!(has(&ad.keywords, "PST") && has(&ad.keywords, "Python"), "{:?}", ad.keywords);
    clean_keywords(&ad.keywords);
}

#[test]
fn a_very_long_ad_is_read_quickly() {
    let block = "Vi søker en driftstekniker med erfaring fra SCADA, PLS og feilsøking i industrien. Du jobber med Siemens TIA Portal og dokumentasjon. ";
    let text = format!("Driftstekniker\nEksempel Industri AS\n\n{}", block.repeat(1500));
    let start = std::time::Instant::now();
    let ad = import::read_ad(&text);
    assert!(start.elapsed() < std::time::Duration::from_secs(10), "took {:?}", start.elapsed());
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Driftstekniker", "Eksempel Industri AS"));
    assert!(has(&ad.keywords, "Siemens TIA Portal") && has(&ad.keywords, "SCADA"), "{:?}", ad.keywords);
    clean_keywords(&ad.keywords);
}

// ---------- job ads on web pages ----------

#[test]
fn job_data_in_every_shape_job_boards_use() {
    let page = |ld: &str| format!(r#"<html><head><title>Side | FINN.no</title><script type="application/ld+json">{ld}</script></head><body><nav>Meny</nav><h1>Annet</h1><p>Logg inn</p></body></html>"#);
    let job = r#"{"@type":"JobPosting","title":"Lagerleder","hiringOrganization":{"@type":"Organization","name":"Eksempel Lager AS"},"description":"<p>Du leder lageret og bruker SAP og Excel.</p>"}"#;
    let shapes = [
        job.to_string(),
        format!("[{{\"@type\":\"BreadcrumbList\"}},{job}]"),
        format!("{{\"@context\":\"https://schema.org\",\"@graph\":[{{\"@type\":\"WebPage\"}},{job}]}}"),
        // finn.no wraps the posting in another object.
        format!("{{\"script:ld+json\":{job}}}"),
        job.replace(r#""@type":"JobPosting""#, r#""@type":["JobPosting"]"#),
        job.replace(r#"{"@type":"Organization","name":"Eksempel Lager AS"}"#, r#""Eksempel Lager AS""#),
        job.replace(r#"{"@type":"Organization","name":"Eksempel Lager AS"}"#, r#"[{"@type":"Organization","name":"Eksempel Lager AS"}]"#),
        // HTML escaped inside the description, and a raw line break JSON does not allow.
        job.replace("<p>Du leder lageret og bruker SAP og Excel.</p>", "&lt;p&gt;Du leder lageret\nog bruker SAP og Excel.&lt;/p&gt;"),
        format!("<!-- {job} -->"),
    ];
    for (i, ld) in shapes.iter().enumerate() {
        let ad = import::ad_from_page(&page(ld)).unwrap_or_else(|e| panic!("shape {i}: {e}"));
        assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Lagerleder", "Eksempel Lager AS"), "shape {i}");
        assert!(ad.description.contains("lageret") && !ad.description.contains('<') && !ad.description.contains("&lt;"), "shape {i}: {}", ad.description);
        assert!(!ad.description.contains("Meny") && !ad.description.contains("Logg inn"), "shape {i}: the page around the ad is left out");
        assert!(has(&ad.keywords, "SAP") && has(&ad.keywords, "Excel"), "shape {i}: {:?}", ad.keywords);
        // The text in the ad box reads back to the same role and employer.
        let again = import::read_ad(&ad.description);
        assert_eq!((again.role, again.company), (ad.role.clone(), ad.company.clone()), "shape {i}");
    }
    let ad = import::ad_from_html(&page(r#"{"@type":"JobPosting","title":"Utvikler &amp; tester","hiringOrganization":{"name":"Bl&aring;b&aelig;r &#38; Co AS"},"description":"Test av C# og .NET i et lite team med mange oppgaver."}"#));
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Utvikler & tester", "Blåbær & Co AS"));
}

#[test]
fn pages_without_job_data() {
    let body = "<p>Vi søker en dyktig kokk til vår restaurant. Du lager mat fra bunnen av og har fagbrev som kokk. Erfaring fra à la carte er en fordel.</p>";
    let h1 = format!("<html><head><title>Kokk - arbeidsplassen.no</title><script>var x = '<h1>Nei</h1>';</script><style>h1{{color:red}}</style></head><body><a>Hopp til innhold</a><h1 class=\"x\">Kokk</h1>{body}</body></html>");
    let ad = import::ad_from_page(&h1).expect("page");
    assert_eq!(ad.role, "Kokk");
    assert!(!ad.description.contains("var x") && !ad.description.contains("color:red"), "scripts and styles are left out");
    // Without <h1>: og:title, then <title>, without the site's name.
    let og = format!("<html><head><meta property=\"og:title\" content=\"Servitør | Eksempel Jobb\"><title>x</title></head><body>{body}</body></html>");
    assert_eq!(import::ad_from_page(&og).expect("og").role, "Servitør");
    let title = format!("<html><head><title>Kokk – FINN jobb</title></head><body>{body}</body></html>");
    assert_eq!(import::ad_from_page(&title).expect("title").role, "Kokk");
    // A headline that is a question is not the title.
    let question = format!("<html><body><h1>Er du kokken vi ser etter?</h1>{body}</body></html>");
    assert_ne!(import::ad_from_page(&question).expect("question").role, "Er du kokken vi ser etter?");
}

#[test]
fn sign_in_walls_and_empty_pages_are_explained() {
    let wall = "<html><body><h1>Logg inn</h1><form><input type=\"password\" name=\"p\"><button>Logg inn</button></form><p>Logg inn for å se annonsen.</p></body></html>";
    assert!(import::ad_from_page(wall).expect_err("wall").contains("innlogging"));
    let script_only = "<html><head><title>Jobb</title></head><body><div id=\"root\"></div><script src=\"app.js\"></script></body></html>";
    assert!(import::ad_from_page(script_only).expect_err("empty").contains("JavaScript"));
    // ad_from_html never fails.
    let _ = import::ad_from_html(wall);
}

#[test]
fn html_text_entities_and_huge_pages() {
    assert_eq!(import::strip_html("<p>S&oslash;knad &#248; &#xF8; &AElig; &amp;oslash; &ndash; &hellip;</p>"), "Søknad ø ø Æ &oslash; – …", "one pass: «&amp;oslash;» stays as written");
    assert_eq!(import::strip_html("<dl><dt>Stillingstittel</dt><dd>Kokk</dd></dl>"), "Stillingstittel\nKokk");
    assert_eq!(import::strip_html("Ukjent &foo; og &#0; og &"), "Ukjent &foo; og og &");
    let huge = format!("<html><body><h1>Kokk</h1>{}<script>{}</script><p>Vi søker en kokk med fagbrev.</p></body></html>", "<div><span>Innhold</span></div>".repeat(60_000), "x".repeat(500_000));
    let start = std::time::Instant::now();
    let ad = import::ad_from_html(&huge);
    assert!(start.elapsed() < std::time::Duration::from_secs(20), "took {:?}", start.elapsed());
    assert_eq!(ad.role, "Kokk");
}

// ---------- job ads in files ----------

/// An ad as a Word and a PDF file, made with CVBuilder's own writer.
fn ad_document() -> wordcraft_doc::Document {
    let mut p = Project::default();
    p.candidate.name = "Lagermedarbeider".into();
    p.candidate.headline = "Arbeidsgiver: Eksempel Logistikk AS".into();
    p.candidate.summary = "Vi søker en lagermedarbeider med truckførerbevis og erfaring fra SAP og WMS. Du plukker og pakker varer.".into();
    document::build(&p)
}

#[test]
fn ads_in_word_pdf_text_and_html_files() {
    let doc = ad_document();
    for (name, bytes) in [("ad.docx", document::to_docx(&doc).expect("docx")), ("ad.pdf", document::to_pdf(&doc).expect("pdf"))] {
        let ad = import::ad_from_file(&temp(name, &bytes)).expect(name);
        assert_eq!(ad.company, "Eksempel Logistikk AS", "{name}");
        assert!(has(&ad.keywords, "SAP"), "{name}: {:?}", ad.keywords);
    }
    let text = "Stilling: Butikkmedarbeider\nArbeidsgiver: Eksempel Mat AS\nDu jobber i kassen og med varebestilling.";
    // UTF-8 with a BOM, UTF-16 (Notepad's «Unicode») and Windows-1252.
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend(text.as_bytes());
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let latin: Vec<u8> = text.chars().map(|c| u8::try_from(u32::from(c)).expect("latin-1")).collect();
    for (name, bytes) in [("bom.txt", bom), ("utf16.txt", utf16), ("ansi.txt", latin)] {
        let ad = import::ad_from_file(&temp(name, &bytes)).expect(name);
        assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Butikkmedarbeider", "Eksempel Mat AS"), "{name}");
        assert!(ad.description.contains("varebestilling") && !ad.description.starts_with('\u{feff}'), "{name}");
    }
    // A saved web page with job data, also when it was saved as .txt or .pdf.
    let html = r#"<!DOCTYPE html><html><head><script type="application/ld+json">{"@type":"JobPosting","title":"Kokk","hiringOrganization":"Eksempel Kro AS","description":"Lag mat fra bunnen av, med fagbrev og godt humør."}</script></head><body><p>Meny</p></body></html>"#;
    for name in ["ad.html", "ad.txt", "ad.pdf"] {
        let ad = import::ad_from_file(&temp(name, html.as_bytes())).expect(name);
        assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Kokk", "Eksempel Kro AS"), "{name}");
    }
}

#[test]
fn broken_empty_locked_and_wrong_files_give_a_clear_error() {
    let real_pdf = document::to_pdf(&ad_document()).expect("pdf");
    let docx = document::to_docx(&ad_document()).expect("docx");
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("empty.txt", Vec::new(), "tom"),
        ("blank.pdf", b"   \n".to_vec(), "tom"),
        ("half.pdf", real_pdf[..real_pdf.len() / 3].to_vec(), "PDF"),
        ("locked.pdf", b"%PDF-1.7\n1 0 obj <</Type/Catalog>> endobj\ntrailer <</Root 1 0 R /Encrypt 2 0 R>>\n%%EOF".to_vec(), "passordbeskyttet"),
        ("text.pdf", b"Dette er bare tekst med et feil navn, ikke en PDF i det hele tatt.".to_vec(), ".pdf"),
        ("half.docx", docx[..docx.len() / 2].to_vec(), "Word"),
        ("old.doc", vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0, 0, 0], ".doc"),
        ("letter.rtf", b"{\\rtf1\\ansi Hei}".to_vec(), "tekst"),
        ("bin.txt", (0..2000u32).map(|i| (i * 7 % 256) as u8).collect(), "kan ikke leses"),
        ("x.xyz", b"hello there, this is some text".to_vec(), "PDF"),
    ];
    for (name, bytes, says) in cases {
        let start = std::time::Instant::now();
        let err = import::ad_from_file(&temp(name, &bytes)).expect_err(name);
        assert!(err.contains(says), "{name}: {err}");
        assert!(start.elapsed() < std::time::Duration::from_secs(35), "{name} took {:?}", start.elapsed());
    }
    // A Word file named .pdf is read as what it is.
    assert!(import::ad_from_file(&temp("word.pdf", &docx)).is_ok());
    assert!(import::text_from_file(std::path::Path::new("does-not-exist.txt")).expect_err("missing").contains("kunne ikke leses"));
}

// ---------- filling in the target ----------

fn ad(role: &str, company: &str, description: &str, keywords: &[&str]) -> import::AdInfo {
    import::AdInfo { role: role.into(), company: company.into(), description: description.into(), keywords: keywords.iter().map(|k| k.to_string()).collect() }
}

#[test]
fn applying_an_ad_twice_changes_nothing_the_second_time() {
    let mut p = Project::example();
    p.target = Default::default();
    let first = import::read_ad("Stilling: Prosjektkoordinator\nArbeidsgiver: Nytt Firma AS\n\nErfaring med Excel, ISY ProArc og leiligheter. Feilsøking.");
    import::apply_ad(&mut p, &first);
    let once = p.clone();
    let summary = import::apply_ad(&mut p, &first);
    assert_eq!(p, once, "the second run adds nothing");
    assert_eq!(summary, "Annonsen er lagt inn.");
    let skills: Vec<&str> = p.skills.iter().map(|s| s.name.as_str()).collect();
    let mut unique = skills.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(skills.len(), unique.len(), "no skill twice: {skills:?}");
}

#[test]
fn applying_keeps_what_the_person_typed_and_replaces_what_the_old_ad_gave() {
    let mut p = Project::example();
    p.target = Default::default();
    let old_text = "Stilling: Prosjektkoordinator\nArbeidsgiver: Gammelt Firma AS\n\nVi bruker Revit og AutoCAD, Navisworks og Solibri.";
    import::apply_ad(&mut p, &import::read_ad(old_text));
    assert_eq!(p.target.company, "Gammelt Firma AS");
    p.target.keywords.push("Mitt eget ord".into());
    // A different ad comes in (fetched, from a file or pasted over the old one).
    let new = import::read_ad("Stilling: Innkjøper\nArbeidsgiver: Nytt Firma AS\n\nDu bruker SAP og Excel.");
    import::apply_ad(&mut p, &new);
    assert_eq!((p.target.role.as_str(), p.target.company.as_str()), ("Innkjøper", "Nytt Firma AS"), "what the old ad filled in makes way");
    assert!(p.target.keywords.iter().any(|k| k == "Mitt eget ord"), "what the person added stays");
    assert!(!p.target.keywords.iter().any(|k| k == "Revit"), "the old ad's key words go: {:?}", p.target.keywords);
    assert!(p.target.keywords.iter().any(|k| k == "SAP"));
    // What the person typed is kept when yet another ad comes in.
    p.target.role = "Innkjøper (vikariat)".into();
    import::apply_ad(&mut p, &import::read_ad("Stilling: Lagersjef\nArbeidsgiver: Tredje AS\n\nLagerstyring i SAP."));
    assert_eq!(p.target.role, "Innkjøper (vikariat)");
    assert_eq!(p.target.company, "Tredje AS");
    // An empty ad says so and changes nothing.
    let before = p.clone();
    assert!(import::apply_ad(&mut p, &import::AdInfo::default()).contains("Fant ikke"));
    assert_eq!(p, before);
}

#[test]
fn key_words_are_merged_without_case_duplicates() {
    let mut p = Project::default();
    p.target.keywords = vec!["FEILSØKING".into(), "sql".into()];
    import::apply_ad(&mut p, &ad("", "", "", &["feilsøking", "SQL", "Ølbrygging", "ølbrygging"]));
    assert_eq!(p.target.keywords, ["FEILSØKING", "sql", "Ølbrygging"]);
}

#[test]
fn skills_are_only_added_when_the_cv_really_shows_them() {
    let mut p = Project::default();
    p.experience.push(cv_core::model::Job { title: "Utvikler".into(), company: "Eksempel AS".into(), bullets: vec!["Laget nettsider i JavaScript og feilsøkingen av betalingsløsningen.".into()], ..Default::default() });
    p.languages.push(cv_core::model::SpokenLanguage { name: "Engelsk".into(), level: "flytende".into() });
    p.target.keywords = vec!["Java".into(), "JavaScript".into(), "javascript".into(), "feilsøking".into(), "engelsk".into(), "Utvikler".into()];
    p.target.role = "Utvikler".into();
    let have = import::skills_you_have(&p);
    assert_eq!(have, ["JavaScript", "feilsøking"], "not Java (in JavaScript), not twice, not a language, not the role");
}

#[test]
fn finn_copy_tab_labels_and_label_lines_with_more_on_them() {
    // finn.no copied from the browser: employer, title, then the facts line.
    let ad = import::read_ad("Politiets eksempeltjeneste\nOSINT-utvikler\nFast stilling ∙ Heltid ∙ Oslo\n\nEr du en kreativ utvikler? Vi bruker Python og Elasticsearch.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("OSINT-utvikler", "Politiets eksempeltjeneste"));
    // NAV's table copied with tabs between label and value.
    let ad = import::read_ad("Stillingstittel\tBackend utvikler\nArbeidsgiver\tStatens Eksempelbygg\nType ansettelse\tFast, heltid 100%");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Backend utvikler", "Statens Eksempelbygg"));
    // More than the value on the line.
    let ad = import::read_ad("Firma: Eksempel Bygg AS | Sted: Oslo\nStilling: Tømrer · Fast\nVi søker tømrer.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Tømrer", "Eksempel Bygg AS"));
    // «Vi søker: …» names the role; «Vi søker» alone is a heading.
    assert_eq!(import::read_ad("Vi søker: Elektriker\nMed fagbrev.").role, "Elektriker");
    let ad = import::read_ad("Lagerarbeider\nVi søker\n- Erfaring fra lager\n- Truckførerbevis");
    assert_eq!(ad.role, "Lagerarbeider");
}

#[test]
fn more_trades_standards_and_the_employer_on_the_second_line() {
    let ad = import::read_ad("Vi har en ledig stilling som sveiser hos Eksempel Verft AS. Du må ha sertifikat etter NS-EN ISO 9606-1 og erfaring med TIG og MIG/MAG.");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Sveiser", "Eksempel Verft AS"));
    for term in ["NS-EN ISO 9606-1", "MIG/MAG", "TIG"] {
        assert!(has(&ad.keywords, term), "{term} missing from {:?}", ad.keywords);
    }
    assert!(!has(&ad.keywords, "NS") && !has(&ad.keywords, "MIG"), "{:?}", ad.keywords);
    let ad = import::read_ad("Butikkmedarbeider deltid\nEksempel Sport\n\n✅ Dette tilbyr vi\n▪ Gode personalrabatter\n\n‣ Kvalifikasjoner\n◦ Erfaring fra salg og kundeservice\n◦ Førerkort klasse B er en fordel");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Butikkmedarbeider deltid", "Eksempel Sport"));
    assert!(has(&ad.keywords, "kundeservice") && has(&ad.keywords, "førerkort"), "{:?}", ad.keywords);
    // Alternatives stay apart: «AWS/GCP» is two key words.
    let ad = import::read_ad("Skyutvikler\n\nDu kjenner AWS/GCP og Terraform godt, og har jobbet med CI/CD.");
    assert!(has(&ad.keywords, "AWS") && has(&ad.keywords, "GCP") && has(&ad.keywords, "CI/CD"), "{:?}", ad.keywords);
    // The line after the title is not an employer when it is a place, a type or a heading.
    for second in ["Oslo", "Heltid", "Om stillingen", "Publisert 3. mai"] {
        assert_eq!(import::read_ad(&format!("Kokk\n{second}\n\nVi lager mat fra bunnen av.")).company, "", "{second}");
    }
}

/// Reads every CV in CVBUILDER_CV_DIR and prints what was found, for checking the reader by eye:
/// `CVBUILDER_CV_DIR=<folder> cargo test -p cv-core --test import read_cv_folder -- --ignored --nocapture`
#[test]
#[ignore = "reads a folder of real CVs; run on purpose"]
fn read_cv_folder() {
    let Ok(dir) = std::env::var("CVBUILDER_CV_DIR") else { return };
    let mut files: Vec<_> = std::fs::read_dir(dir).expect("folder").flatten().map(|e| e.path()).collect();
    files.sort();
    for path in files {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        match cv_core::import::text_from_file(&path) {
            Err(e) => println!("== {name}: ERROR {e}"),
            Ok(text) => {
                let r = cv_core::import::read_cv(&text);
                let p = &r.project;
                println!(
                    "== {name}: name={:?} email={} phone={} city={:?} headline={:?} summary={}w skills={} langs={} jobs={} edu={} certs={} interests={} rest={}c",
                    p.candidate.name, !p.candidate.email.is_empty(), !p.candidate.phone.is_empty(), p.candidate.city, p.candidate.headline,
                    p.candidate.summary.split_whitespace().count(), p.skills.len(), p.languages.len(), p.experience.len(), p.education.len(),
                    p.certifications.len(), p.interests.len(), r.rest.chars().count()
                );
                for j in &p.experience {
                    println!("   job: {:?} @ {:?} {}–{:?} ({} bullets)", j.title, j.company, j.start, j.end, j.bullets.len());
                }
                for e in &p.education {
                    println!("   edu: {:?} @ {:?} {}–{}", e.qualification, e.institution, e.start, e.end);
                }
            }
        }
    }
}
