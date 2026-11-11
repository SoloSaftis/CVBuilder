//! Project files move between the HTML builder and this app without loss.
use cv_core::Project;

// Fictional data only (the shared example), never a real person's CV.
const EXAMPLE: &str = include_str!("../src/example.json");

#[test]
fn reads_html_builder_project_and_round_trips() {
    let project = Project::from_json(EXAMPLE).expect("example project parses");
    assert_eq!(project.experience.len(), 2);
    assert!(project.candidate.name.contains("Haugland"));
    let again = Project::from_json(&project.to_json().expect("serialises")).expect("re-parses");
    assert_eq!(project, again);
}

#[test]
fn version_1_files_get_new_fields_and_lose_approval() {
    let mut value: serde_json::Value = serde_json::from_str(EXAMPLE).expect("json");
    value["version"] = 1.into();
    value["approvals"]["facts_confirmed"] = true.into();
    for key in ["linkedin"] { value["candidate"].as_object_mut().expect("object").remove(key); }
    value.as_object_mut().expect("object").remove("certifications");
    value["options"].as_object_mut().expect("object").remove("layout");
    let project = Project::from_json(&value.to_string()).expect("HTML builder file parses");
    assert_eq!(project.version, 2);
    assert!(project.certifications.is_empty());
    assert!(!project.approvals.facts_confirmed);
}

#[test]
fn unknown_fields_are_rejected() {
    let mut value: serde_json::Value = serde_json::from_str(EXAMPLE).expect("json");
    value["extra"] = true.into();
    assert!(Project::from_json(&value.to_string()).is_err());
}
