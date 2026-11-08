//! The Rust rules must say exactly what the HTML builder says about the same project.
use cv_core::{Project, checks};

#[test]
fn rust_and_html_builder_agree() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("parity.json")).expect("fixture");
    for (name, case) in fixture.as_object().expect("object") {
        let project = Project::from_json(&case["project"].to_string()).expect("project");
        let ready: Vec<String> = serde_json::from_value(case["ready"].clone()).expect("ready");
        assert_eq!(checks::ready(&project), ready, "{name}: ready()");
        let (list, matched) = checks::audit(&project, 2, 2026 * 12 + 10);
        let ours: Vec<(String, String)> = list.iter().map(|c| (serde_json::to_value(c.level).expect("level").as_str().expect("str").to_string(), c.text.clone())).collect();
        let theirs: Vec<(String, String)> = case["checks"].as_array().expect("checks").iter().map(|c| (c["level"].as_str().expect("l").to_string(), c["text"].as_str().expect("t").to_string())).collect();
        assert_eq!(ours, theirs, "{name}: audit()");
        assert_eq!(serde_json::to_value(&matched).expect("match")["missing"], case["match"]["missing"], "{name}: keywords");
        assert_eq!(checks::plain_text(&project), case["plain"].as_str().expect("plain"), "{name}: plain text");
    }
}

#[test]
fn dates_are_read_like_the_html_builder() {
    for (input, expected) in [("09.2021", Some("2021-09")), ("9/2021", Some("2021-09")), ("2021-09", Some("2021-09")), ("sep. 2021", Some("2021-09")), ("mars 2019", Some("2019-03")), ("13.2021", None), ("hello", None), ("", Some("")), ("2010", Some("2010"))] {
        assert_eq!(checks::parse_month(input).as_deref(), expected, "{input}");
    }
    for (input, expected) in [("2016", Some("2016")), ("aug. 2016", Some("2016")), ("16", None)] {
        assert_eq!(checks::parse_year(input).as_deref(), expected, "{input}");
    }
}

#[test]
fn now_means_current_job() {
    for word in ["nå", "Nå", "d.d.", "present", "now"] {
        assert!(checks::is_now(word), "{word}");
    }
    assert!(!checks::is_now("2010"));
}
