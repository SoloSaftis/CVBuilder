//! Designs, the contact line order, custom contact fields, section order and custom sections.
use cv_core::model::{ContactField, CustomItem, CustomSection};
use cv_core::{Project, checks, document, layouts, text};

#[test]
fn every_built_in_design_is_valid_and_unique_and_renders() {
    let all = layouts::all();
    assert!(all.len() >= 4);
    let mut ids = std::collections::HashSet::new();
    let mut p = Project::example();
    for d in &all {
        assert!(d.problems().is_empty(), "{}: {:?}", d.id, d.problems());
        assert!(ids.insert(d.id.clone()), "duplicate id {}", d.id);
        p.options.layout = d.id.clone();
        let doc = document::build(&p);
        let pages = document::page_count(&doc);
        assert!((1..=2).contains(&pages), "{} gives {pages} pages for the example", d.id);
        assert!(!document::to_pdf(&doc).expect("pdf").is_empty());
    }
}

#[test]
fn no_remake_is_silently_dropped() {
    // An invalid entry in designs.json would be skipped at start; catch it here instead.
    let file: Vec<layouts::Design> = serde_json::from_str(include_str!("../src/designs.json")).expect("designs.json reads");
    for d in &file {
        assert!(d.problems().is_empty(), "{}: {:?}", d.id, d.problems());
        assert!(!d.credit.is_empty(), "{} needs its credit", d.id);
    }
    assert_eq!(layouts::BUILT_IN.len(), 4 + file.len());
}

#[test]
fn the_four_originals_keep_their_ids_for_old_project_files() {
    for id in ["classic", "modern", "nordic", "compact"] {
        assert!(layouts::exists(id), "{id}");
    }
    let mut p = Project::example();
    p.options.layout = "not-on-this-computer".into();
    assert_eq!(document::page_count(&document::build(&p)), 1, "an unknown design falls back to Klassisk");
}

#[test]
fn a_design_file_round_trips_and_bad_files_are_explained() {
    let mut own = layouts::design("modern");
    own.id = "min-design".into();
    let file = layouts::to_file(&own);
    assert!(file.contains("\"format\": \"cvbuilder-design/1\""));
    assert!(!file.contains("imported"), "internal flags stay out of the file");
    let back = layouts::parse(&file).expect("valid");
    assert!(back.imported);
    assert_eq!(back.accent, own.accent);

    let clash = layouts::parse(&layouts::to_file(&layouts::design("classic"))).expect_err("built-in id");
    assert!(clash.contains("innebygd"));
    let mut bad = own.clone();
    bad.body_font = "Comic Sans MS".into();
    bad.ink = "#DDDDDD".into();
    bad.size = 20.0;
    let errors = layouts::parse(&layouts::to_file(&bad)).expect_err("bad");
    assert!(errors.contains("body_font") && errors.contains("ink") && errors.contains("size"), "{errors}");
    assert!(layouts::parse("{\"hello\": 1}").is_err());

    layouts::set_imported(vec![back]);
    assert!(layouts::exists("min-design"));
    layouts::set_imported(Vec::new());
}

#[test]
fn contact_line_follows_the_chosen_order_with_custom_fields() {
    let mut p = Project::example();
    p.candidate.extra = vec![ContactField { id: "x1".into(), label: "Førerkort".into(), value: "B".into() }, ContactField { id: "x2".into(), label: String::new(), value: "github.com/ingrid".into() }];
    let default = text::contact_parts(&p);
    assert_eq!(default.first().map(String::as_str), Some(p.candidate.email.as_str()));
    assert_eq!(default.last().map(String::as_str), Some("github.com/ingrid"));
    p.candidate.contact_order = vec!["city".into(), "x1".into(), "nonsense".into()];
    let ordered = text::contact_parts(&p);
    assert_eq!(ordered[0], p.candidate.city);
    assert_eq!(ordered[1], "Førerkort: B");
    assert_eq!(ordered.len(), 6, "every part once, unknown keys ignored");
    assert!(checks::plain_text(&p).lines().nth(2).expect("contact").starts_with(&p.candidate.city));
}

#[test]
fn sections_follow_the_chosen_order_and_custom_sections_show() {
    let mut p = Project::example();
    p.custom_sections = vec![CustomSection {
        id: "s1".into(),
        title: "Prosjekter".into(),
        items: vec![CustomItem { title: "Ny barnehage".into(), subtitle: "Trondheim kommune".into(), date: "2023".into(), bullets: vec!["Koordinerte 12 underleverandører.".into()] }],
    }];
    let order = text::section_order(&p);
    let at = |k: &str| order.iter().position(|x| x == k).expect(k);
    assert_eq!(at("s1"), order.len() - 1, "a new section goes at the bottom");
    let plain = checks::plain_text(&p);
    assert!(plain.contains("PROSJEKTER") && plain.contains("Ny barnehage | Trondheim kommune | 2023") && plain.contains("- Koordinerte 12"));
    p.options.section_order = vec!["education".into(), "profile".into()];
    let plain = checks::plain_text(&p);
    let edu = plain.find(&text::labels(&p).education.to_uppercase()).expect("education");
    let profile = plain.find(&text::labels(&p).profile.to_uppercase()).expect("profile");
    assert!(edu < profile);
    // Words in a custom section count for the keyword check.
    p.target.keywords = vec!["barnehage".into()];
    assert_eq!(checks::keyword_match(&p).found, vec!["barnehage".to_string()]);
}

#[test]
fn old_files_without_the_new_fields_still_load_and_save_the_same() {
    let p = Project::example();
    let json = p.to_json().expect("json");
    assert!(!json.contains("custom_sections") && !json.contains("contact_order") && !json.contains("section_order"), "empty new fields are not written, so the HTML builder reads the file");
    assert_eq!(Project::from_json(&json).expect("read"), p);
}

#[test]
fn colours_are_independent_of_the_design() {
    use cv_core::model::Palette;
    let mut p = Project::example();
    p.options.layout = "rr-gengar".into();
    let base = layouts::effective(&p);
    assert!(base.header_fill.is_some(), "Gengar has a header band");

    // A whole palette over the design: the main colour wins, bands can be removed.
    p.options.palette = Palette { accent: Some("#1F3A5F".into()), header_fill: Some("none".into()), heading_fill: Some("none".into()), ..Default::default() };
    let d = layouts::effective(&p);
    assert_eq!(d.accent, "#1F3A5F");
    assert!(d.header_fill.is_none() && d.heading_style != layouts::HeadingStyle::Band);
    assert_eq!(d.body_font, base.body_font, "fonts and layout stay the design's");

    // A band in any colour gets readable text by itself.
    p.options.palette = Palette { header_fill: Some("#F2E8C9".into()), ..Default::default() };
    let d = layouts::effective(&p);
    assert_eq!(d.header_text.as_deref(), Some("#1A1A1A"));

    // A colour per section, over the palette.
    p.options.palette = Palette::default();
    p.options.section_colors.insert("experience".into(), "#2E5E3E".into());
    p.options.section_colors.insert("skills".into(), "not a colour".into());
    let d = layouts::effective(&p);
    assert_eq!(d.section_accent("experience"), "#2E5E3E");
    assert_eq!(d.section_accent("skills"), d.accent, "an unreadable value is ignored");
    assert_eq!(document::page_count(&document::build(&p)), 1);

    // Saved only when used, so older files and the HTML builder are unaffected.
    let json = p.to_json().expect("json");
    assert!(json.contains("section_colors") && !json.contains("\"palette\""));
    assert_eq!(Project::from_json(&json).expect("read"), p);
}

#[test]
fn ready_made_palettes_are_readable_and_faint_colours_are_warned_about() {
    let mut p = Project::example();
    for (name, palette) in layouts::palettes() {
        p.options.palette = palette;
        let d = layouts::effective(&p);
        assert!(d.color_warnings().is_empty(), "{name}: {:?}", d.color_warnings());
    }
    p.options.palette.ink = Some("#DDDDDD".into());
    assert!(!layouts::effective(&p).color_warnings().is_empty());
}

#[test]
fn a_design_file_may_bring_colours_per_section() {
    let mut d = layouts::design("classic");
    d.id = "fargerik".into();
    d.section_colors.insert("experience".into(), "#7A2638".into());
    let back = layouts::parse(&layouts::to_file(&d)).expect("valid");
    assert_eq!(back.section_accent("experience"), "#7A2638");
    d.section_colors.insert("hobbyer".into(), "#FFFF00".into());
    let err = layouts::parse(&layouts::to_file(&d)).expect_err("bad key and faint colour");
    assert!(err.contains("hobbyer"), "{err}");
}

#[test]
fn a_sidebar_design_renders_every_section_once_and_warns() {
    use cv_core::layouts::{Columns, HeaderPlace, Side};
    let mut d = layouts::design("modern");
    d.id = "test-sidebar".into();
    d.columns = Some(Columns { side: Side::Left, width: 0.33, fill: Some("#1D3B4F".into()), text: Some("#FFFFFF".into()), heading: Some("#FFFFFF".into()), sections: vec!["contact".into(), "skills".into(), "languages".into(), "interests".into()], header: HeaderPlace::Main });
    assert!(d.problems().is_empty(), "{:?}", d.problems());
    assert!(d.ats_warning().is_some());
    layouts::set_imported(vec![d]);
    let mut p = Project::example();
    p.options.layout = "test-sidebar".into();
    let doc = document::build(&p);
    let pdf = document::to_pdf(&doc).expect("pdf");
    let docx = document::to_docx(&doc).expect("docx");
    assert!(document::page_count(&doc) >= 1);
    // Every part of the CV is real text in the file.
    let dir = std::env::temp_dir().join(format!("cvbuilder-sidebar-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    for (name, bytes) in [("s.pdf", &pdf), ("s.docx", &docx)] {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write");
        let text = cv_core::import::text_from_file(&path).expect("text");
        for needle in [p.candidate.name.as_str(), "Fjellvegg", "Langrenn", "Norsk", p.candidate.email.as_str()] {
            assert!(text.contains(needle), "{name}: {needle} missing");
        }
    }
    if let Ok(out) = std::env::var("CVBUILDER_SIDEBAR_OUT") {
        std::fs::write(out, &pdf).expect("write sample");
    }
    layouts::set_imported(Vec::new());
}

#[test]
fn page_setup_size_margins_columns_text_and_page_breaks() {
    use cv_core::model::{ColumnsChoice, Margins, PageSize};
    let mut p = Project::example();
    let base = document::build(&p);
    assert_eq!(document::page_count(&base), 1);
    assert!((base.last_section.page_w - 595.28).abs() < 0.1);

    p.options.page.size = PageSize::Letter;
    p.options.page.margins = Margins::Wide;
    let doc = document::build(&p);
    assert_eq!(doc.last_section.page_w, 612.0);
    assert_eq!(doc.last_section.margin_left, 72.0);

    // A page break before a section makes another page: the person can add pages.
    p.options.page = Default::default();
    p.options.page.page_breaks = vec!["education".into()];
    assert_eq!(document::page_count(&document::build(&p)), 2);

    // Columns for any design, and back to one column.
    p.options.page = Default::default();
    p.options.page.columns = Some(ColumnsChoice::Right);
    let d = layouts::effective(&p);
    assert!(d.columns.as_ref().is_some_and(|c| c.side == layouts::Side::Right));
    assert!(d.problems().is_empty(), "{:?}", d.problems());
    assert_eq!(document::page_count(&document::build(&p)), 1);
    p.options.layout = "rr-chikorita-2".into();
    p.options.page.columns = Some(ColumnsChoice::One);
    assert!(layouts::effective(&p).columns.is_none());

    // Bigger text and more air make the same CV longer.
    p.options.layout = "classic".into();
    p.options.page = Default::default();
    p.options.page.text_delta = 2.0;
    p.options.page.spacing = 1.3;
    let d = layouts::effective(&p);
    assert!(d.size > 12.0 && d.gap > 1.2);
    assert!(document::page_count(&document::build(&p)) >= 1);

    // Saved only when changed, and read back.
    p.options.page = Default::default();
    assert!(!p.to_json().expect("json").contains("\"page\""));
    p.options.page.margins = Margins::Narrow;
    let back = Project::from_json(&p.to_json().expect("json")).expect("read");
    assert_eq!(back.options.page.margins, Margins::Narrow);
}

#[test]
fn a_new_page_after_a_section_starts_the_next_one_on_a_new_page() {
    let mut p = Project::example();
    p.options.page.page_breaks_after = vec!["experience".into()];
    assert_eq!(document::page_count(&document::build(&p)), 2, "one column");
    p.options.layout = "rr-gengar-2".into();
    p.options.page.page_breaks_after = vec!["profile".into()];
    assert_eq!(document::page_count(&document::build(&p)), 2, "two columns: experience moves to page two");
    // After the last section there is nothing to move, so no empty page.
    p.options.layout = "classic".into();
    p.options.page.page_breaks_after = vec!["interests".into()];
    assert_eq!(document::page_count(&document::build(&p)), 1);
}

#[test]
fn page_advice_fits_the_career() {
    use cv_core::checks::recommended_pages;
    let today = 2026 * 12 + 10;
    let mut p = Project::example();
    assert_eq!(recommended_pages(&p, today), 2, "the example has eight years in two jobs");
    p.experience.truncate(1);
    p.experience[0].start = "2025-01".into();
    assert_eq!(recommended_pages(&p, today), 1, "under three years: one page");
    p.experience.clear();
    assert_eq!(recommended_pages(&p, today), 1, "no jobs yet: one page");
}
