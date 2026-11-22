//! Schema version 3: stable ids for every item, the move from older files and back, and hiding
//! items in a tailored copy (041). Fictional data only.
use cv_core::ids::{self, Pairing, carry};
use cv_core::model::{CustomItem, CustomSection, Job, Skill};
use cv_core::{Project, checks, document};

const EXAMPLE: &str = include_str!("../src/example.json");

fn ids_unique(p: &Project) {
    let all = p.all_ids();
    let set: std::collections::HashSet<&str> = all.iter().copied().collect();
    assert_eq!(set.len(), all.len(), "ids are unique: {all:?}");
    for id in &all {
        assert!(!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'), "id «{id}» has only [A-Za-z0-9_-]");
    }
}

fn every_item_has_an_id(p: &Project) {
    assert!(p.experience.iter().all(|j| !j.id.is_empty() && j.bullet_ids.len() == j.bullets.len()));
    assert!(p.education.iter().all(|x| !x.id.is_empty()));
    assert!(p.skills.iter().all(|x| !x.id.is_empty()));
    assert!(p.languages.iter().all(|x| !x.id.is_empty()));
    assert!(p.certifications.iter().all(|x| !x.id.is_empty()));
    assert!(p.custom_sections.iter().flat_map(|s| &s.items).all(|i| !i.id.is_empty() && i.bullet_ids.len() == i.bullets.len()));
}

fn with_custom_section(mut p: Project) -> Project {
    p.custom_sections.push(CustomSection {
        id: "frivillig".into(),
        title: "Frivillig arbeid".into(),
        items: vec![CustomItem { title: "Kasserer".into(), subtitle: "Idrettslaget Eksempel".into(), date: "2019".into(), bullets: vec!["Førte regnskapet for 300 medlemmer.".into(), "Fikk ned purringer med 40 %.".into()], ..Default::default() }],
    });
    p
}

#[test]
fn an_older_file_gets_the_same_ids_every_time_it_is_read() {
    let a = Project::from_json(EXAMPLE).expect("v2 example");
    let b = Project::from_json(EXAMPLE).expect("again");
    assert_eq!(a.version, 3);
    assert_eq!(a, b, "deterministic: the draft and its file compare equal after a restart");
    every_item_has_an_id(&a);
    ids_unique(&a);
    // Ids already in the file are kept; the new ones count per kind.
    assert_eq!(a.experience[0].id, "ex_1");
    assert_eq!(a.experience[0].bullet_ids[0], "b_1");
    assert_eq!(a.experience[1].bullet_ids[0], format!("b_{}", a.experience[0].bullets.len() + 1));
    assert_eq!(a.skills[0].id, "skill_1");
    assert_eq!(a.education[0].id, "edu_1");
}

#[test]
fn version_3_round_trips_and_keeps_every_id() {
    let mut p = with_custom_section(Project::from_json(EXAMPLE).expect("example"));
    p.normalize_ids();
    p.set_hidden(&p.skills[1].id.clone(), true);
    p.options.copy_of = "Ingrid_Haugland_CV_utkast.json".into();
    let json = p.to_json().expect("json");
    assert!(json.contains("\"version\": 3") && json.contains("\"bullet_ids\"") && json.contains("\"hidden\""));
    let back = Project::from_json(&json).expect("v3 reads");
    assert_eq!(back, p);
}

#[test]
fn version_2_and_back_loses_nothing_but_the_ids() {
    // v2 → v3 → v2: the same file as before (only the version line may differ).
    let original: serde_json::Value = serde_json::from_str(EXAMPLE).expect("json");
    let p = Project::from_json(EXAMPLE).expect("v2 reads");
    let down: serde_json::Value = serde_json::from_str(&p.to_v2_json().expect("v2 json")).expect("json");
    assert_eq!(down, original, "the version 2 file is exactly the one that was read");
    // v3 with hidden items → v2: older versions see the full CV, and read the file.
    let mut q = with_custom_section(p.clone());
    q.normalize_ids();
    q.set_hidden(&q.experience[1].id.clone(), true);
    let v2 = q.to_v2_json().expect("v2");
    assert!(!v2.contains("\"id\": \"skill_") && !v2.contains("bullet_ids") && !v2.contains("hidden") && v2.contains("\"version\": 2"));
    let again = Project::from_json(&v2).expect("v2 reads again");
    assert_eq!(again.experience.len(), q.experience.len(), "nothing hidden is lost going down");
    assert_eq!(again.custom_sections[0].items[0].bullets, q.custom_sections[0].items[0].bullets);
}

#[test]
fn a_file_from_a_newer_version_says_so() {
    let mut v: serde_json::Value = serde_json::from_str(EXAMPLE).expect("json");
    v["version"] = 4.into();
    v["something_new"] = true.into();
    let e = Project::from_json(&v.to_string()).expect_err("v4 is refused");
    assert!(matches!(e, cv_core::ProjectError::Version(4)), "{e}");
}

#[test]
fn broken_ids_are_repaired_without_touching_good_ones() {
    let mut p = Project::from_json(EXAMPLE).expect("example");
    let keep = p.experience[0].id.clone();
    p.experience[1].id = keep.clone();
    p.experience[1].bullet_ids.truncate(1);
    let dup = p.skills[1].id.clone();
    p.skills[0].id = dup.clone();
    p.education[0].id.clear();
    p.options.hidden = vec!["gone".into(), p.skills[1].id.clone(), p.skills[1].id.clone()];
    p.normalize_ids();
    every_item_has_an_id(&p);
    ids_unique(&p);
    assert_eq!(p.experience[0].id, keep, "the first of two equal ids keeps it");
    assert_eq!(p.skills[0].id, dup, "the first of two equal skill ids keeps it");
    assert_eq!(p.options.hidden, vec![dup], "unknown and repeated hidden ids are dropped");
    // New ids from edits are never the deterministic kind, so a deleted id does not come back.
    assert!(!p.education[0].id.starts_with("edu_1"), "{}", p.education[0].id);
}

#[test]
fn edits_keep_ids() {
    let old: Vec<String> = ["Planla drift", "Fulgte opp leverandører", "Skrev rapporter"].map(String::from).to_vec();
    let ids: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    let same = |new: &[&str], pairing| carry(&old, &ids, &new.iter().map(|s| s.to_string()).collect::<Vec<_>>(), pairing);
    // Moved: each keeps its own.
    assert_eq!(same(&["Skrev rapporter", "Planla drift", "Fulgte opp leverandører"], Pairing::Greedy), [Some("c".into()), Some("a".into()), Some("b".into())]);
    // One line edited: it keeps the id of the line it replaced.
    assert_eq!(same(&["Planla driften av 3 lagre", "Fulgte opp leverandører", "Skrev rapporter"], Pairing::Greedy), [Some("a".into()), Some("b".into()), Some("c".into())]);
    // A line deleted and one added: the others keep theirs, the new one has none yet.
    assert_eq!(same(&["Planla drift", "Skrev rapporter"], Pairing::Greedy), [Some("a".into()), Some("c".into())]);
    // A list rewritten with other things: only same-count changes are taken as edits.
    assert_eq!(same(&["Planla drift", "X", "Y", "Z"], Pairing::SameCount), [Some("a".into()), None, None, None]);
    assert_eq!(same(&["Planla drift", "X", "Y"], Pairing::SameCount), [Some("a".into()), Some("b".into()), Some("c".into())]);
}

#[test]
fn a_rewritten_list_or_translation_keeps_ids_and_what_is_hidden() {
    let mut p = Project::from_json(EXAMPLE).expect("example");
    let skill = p.skills[2].id.clone();
    let bullet = p.experience[0].bullet_ids[1].clone();
    p.set_hidden(&skill, true);
    p.set_hidden(&bullet, true);
    let before = p.clone();
    // As an AI tool does: the skills rewritten from names (no ids), the bullets translated.
    p.skills = before.skills.iter().map(|s| Skill { name: s.name.clone(), group: s.group.clone(), confirmed: true, ..Default::default() }).collect();
    p.experience[0].bullets = before.experience[0].bullets.iter().map(|b| format!("(en) {b}")).collect();
    p.keep_ids_from(&before);
    p.normalize_ids();
    assert_eq!(p.skills[2].id, skill);
    assert_eq!(p.experience[0].bullet_ids, before.experience[0].bullet_ids);
    assert!(p.is_hidden(&skill) && p.is_hidden(&bullet), "still hidden after the rewrite");
}

#[test]
fn hidden_items_are_left_out_of_every_output_but_kept() {
    let mut p = with_custom_section(Project::from_json(EXAMPLE).expect("example"));
    p.normalize_ids();
    p.approvals.facts_confirmed = true;
    let job = p.experience[1].clone();
    let bullet = (p.experience[0].bullet_ids[0].clone(), p.experience[0].bullets[0].clone());
    let skill = p.skills[0].clone();
    let item = p.custom_sections[0].items[0].clone();
    let full_text = checks::plain_text(&p);
    assert!(full_text.contains(&job.company) && full_text.contains(&bullet.1) && full_text.contains(&item.title));
    let pages_before = document::page_count(&document::build(&p));
    for id in [&job.id, &bullet.0, &skill.id, &item.id] {
        assert!(p.set_hidden(id, true));
    }
    assert!(!p.set_hidden(&skill.id, true), "hiding twice changes nothing");
    assert_eq!(p.hidden_count(), 4);
    // Out of the plain text, the Word and PDF document, the keyword match and the checks …
    let text = checks::plain_text(&p);
    for gone in [&job.company, &bullet.1, &item.title] {
        assert!(!text.contains(gone.as_str()), "«{gone}» is hidden but in the text");
    }
    assert!(!text.contains(&format!("{}\n", skill.name)) && !text.contains(&format!(", {}", skill.name)) && !text.contains(&format!("{}, ", skill.name)), "the hidden skill is in the text:\n{text}");
    let doc = document::build(&p);
    for (bytes, ext) in [(document::to_docx(&doc).expect("docx"), "docx"), (document::to_pdf(&doc).expect("pdf"), "pdf")] {
        let words = cv_core::import::text_from_bytes(&bytes, ext).expect("read back");
        assert!(!words.contains(&job.company) && !words.contains(&item.title), "the hidden job is in the {ext}");
        assert!(words.contains(&p.experience[0].company), "the shown job is in the {ext}");
    }
    assert!(document::page_count(&doc) <= pages_before);
    p.target.keywords = vec![skill.name.clone()];
    let found = checks::keyword_match(&p);
    assert!(found.missing.contains(&skill.name) || !checks::plain_text(&p).contains(&skill.name), "a hidden skill does not count as found");
    // … but still in the project, and back when shown again.
    assert!(p.experience.iter().any(|j| j.id == job.id) && p.skills.iter().any(|s| s.id == skill.id));
    assert!(p.set_hidden(&job.id, false));
    assert!(checks::plain_text(&p).contains(&job.company));
}

#[test]
fn hiding_does_not_block_the_download() {
    let mut p = Project::from_json(EXAMPLE).expect("example");
    p.approvals.facts_confirmed = true;
    assert!(checks::ready(&p).is_empty(), "{:?}", checks::ready(&p));
    // An unfinished job is hidden: it is not in the CV, so it does not hold it up.
    p.experience.push(Job { id: "draft".into(), end: Some(String::new()), ..Default::default() });
    assert!(!checks::ready(&p).is_empty());
    p.set_hidden("draft", true);
    assert!(checks::ready(&p).is_empty(), "{:?}", checks::ready(&p));
    // Every bullet of a job hidden: the job stays, with its title and dates only.
    for id in p.experience[0].bullet_ids.clone() {
        p.set_hidden(&id, true);
    }
    assert!(checks::ready(&p).is_empty(), "{:?}", checks::ready(&p));
    assert!(checks::plain_text(&p).contains(&p.experience[0].company));
}

#[test]
fn items_lists_every_id_with_its_text() {
    let mut p = with_custom_section(Project::from_json(EXAMPLE).expect("example"));
    p.normalize_ids();
    p.set_hidden(&p.experience[0].bullet_ids[0].clone(), true);
    let items = p.items();
    assert_eq!(items.len(), p.all_ids().len());
    let b = items.iter().find(|i| i.id == p.experience[0].bullet_ids[0]).expect("bullet");
    assert_eq!(b.kind, ids::Kind::Bullet);
    assert_eq!(b.parent.as_deref(), Some(p.experience[0].id.as_str()));
    assert_eq!(b.text, p.experience[0].bullets[0]);
    assert!(b.hidden);
}

#[test]
fn fresh_ids_are_unique() {
    let all: std::collections::HashSet<String> = (0..2000).map(|_| ids::fresh(ids::BULLET)).collect();
    assert_eq!(all.len(), 2000);
}

#[test]
fn json_resume_export_and_ad_requirements_see_only_this_copy() {
    let mut p = Project::from_json(EXAMPLE).expect("example");
    let job = p.experience[1].clone();
    assert!(cv_core::jsonresume::write(&p).contains(&job.company));
    p.set_hidden(&job.id, true);
    assert!(!cv_core::jsonresume::write(&p).contains(&job.company), "a hidden job is not exported");
    // A hidden skill does not back a requirement from the ad.
    let skill = p.skills[0].name.clone();
    let r = cv_core::requirements::Requirement { text: skill.clone(), need: cv_core::requirements::Need::Must, terms: vec![skill.clone()] };
    assert!(cv_core::requirements::backing(&p, &r).missing.is_empty());
    p.set_hidden(&p.skills[0].id.clone(), true);
    if !checks::plain_text(&p).contains(&skill) {
        assert!(!cv_core::requirements::backing(&p, &r).missing.is_empty(), "the hidden skill still backs «{skill}»");
    }
}
