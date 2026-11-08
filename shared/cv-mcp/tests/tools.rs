use cv_core::Project;
use cv_mcp::{handle, tools};
use serde_json::{Value, json};

fn rpc(project: &mut Project, message: Value) -> Value {
    serde_json::from_str(&handle(project, &message.to_string()).expect("a reply")).expect("json")
}

#[test]
fn tool_names_are_valid_for_every_client() {
    // MCP clients and model APIs accept only [A-Za-z0-9_-], up to 64 characters.
    for t in tools() {
        assert!(t.name.len() <= 64 && t.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'), "{}", t.name);
        assert_eq!(t.schema["type"], "object", "{}", t.name);
    }
}

#[test]
fn initialize_list_and_call_over_json_rpc() {
    let mut p = Project::example();
    let init = rpc(&mut p, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "cvbuilder");
    assert!(handle(&mut p, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string()).is_none());
    let unknown_version = rpc(&mut p, json!({"jsonrpc": "2.0", "id": 9, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}));
    assert_eq!(unknown_version["result"]["protocolVersion"], "2025-06-18");
    let list = rpc(&mut p, json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert!(list["result"]["tools"].as_array().expect("tools").len() >= 10);
    let call = rpc(&mut p, json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "cv_update_profile", "arguments": {"summary": "Ny profil."}}}));
    assert_eq!(call["result"]["isError"], false);
    assert_eq!(p.candidate.summary, "Ny profil.");
    let bad = rpc(&mut p, json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "rm_rf"}}));
    assert_eq!(bad["error"]["code"], -32602);
    assert_eq!(serde_json::from_str::<Value>(&handle(&mut p, "{oops").expect("reply")).expect("json")["error"]["code"], -32700);
}

#[test]
fn read_leaves_out_the_portrait_and_approval() {
    let mut p = Project::example();
    p.candidate.photo = "data:image/jpeg;base64,AAAA".into();
    let out = cv_mcp::call(&mut p, "cv_read", &json!({}));
    assert!(!out.text.contains("base64"));
    assert!(!out.text.contains("facts_confirmed"));
    assert!(out.text.contains(&p.experience[0].id));
}

#[test]
fn jobs_are_added_changed_and_removed_with_checked_dates() {
    let mut p = Project::default();
    let added = cv_mcp::call(&mut p, "cv_job_save", &json!({"title": "Ingeniør", "company": "Firma AS", "start": "2021-09", "end": "now", "bullets": ["Leverte X", " "]}));
    assert!(!added.is_error, "{}", added.text);
    assert_eq!(p.experience.len(), 1);
    let j = &p.experience[0];
    assert_eq!((j.start.as_str(), j.end.clone(), j.bullets.clone()), ("2021-09", None, vec!["Leverte X".to_string()]));
    let id = j.id.clone();
    let bad = cv_mcp::call(&mut p, "cv_job_save", &json!({"id": id, "start": "last spring"}));
    assert!(bad.is_error);
    assert_eq!(p.experience[0].start, "2021-09", "a bad date changes nothing");
    let changed = cv_mcp::call(&mut p, "cv_job_save", &json!({"id": id, "end": "2024-03"}));
    assert_eq!(p.experience[0].end.as_deref(), Some("2024-03"));
    assert!(changed.change.expect("change").contains("sluttdatoen"));
    assert!(cv_mcp::call(&mut p, "cv_job_save", &json!({"id": "nope"})).is_error);
    assert!(cv_mcp::call(&mut p, "cv_job_remove", &json!({"id": id})).change.is_some());
    assert!(p.experience.is_empty());
}

#[test]
fn lists_layout_language_and_checks() {
    let mut p = Project::example();
    cv_mcp::call(&mut p, "cv_skills_set", &json!({"skills": [{"group": "Automasjon", "name": "PLS"}, {"name": ""}]}));
    assert_eq!(p.skills.len(), 1);
    cv_mcp::call(&mut p, "cv_interests_set", &json!({"interests": [{"label": "a"}, {"label": "b"}, {"label": "c"}, {"label": "d"}]}));
    assert_eq!(p.options.interest_ids.len(), 3, "at most three are shown");
    assert!(cv_mcp::call(&mut p, "cv_education_set", &json!({"education": [{"qualification": "Bachelor", "start": "2010", "end": "someday"}]})).is_error);
    assert!(cv_mcp::call(&mut p, "cv_set_layout", &json!({"layout": "compact"})).change.is_some());
    assert_eq!(p.options.layout, "compact");
    assert!(cv_mcp::call(&mut p, "cv_set_layout", &json!({"layout": "fancy"})).is_error);
    cv_mcp::call(&mut p, "cv_set_language", &json!({"language": "en"}));
    assert!(p.is_english());
    let check: Value = serde_json::from_str(&cv_mcp::call(&mut p, "cv_check", &json!({})).text).expect("json");
    assert!(check["pages"].as_u64().expect("pages") >= 1);
    assert!(check["blocks_export"].as_array().expect("list").iter().all(|b| !b.as_str().unwrap_or("").starts_with("approvals:")));
}

#[test]
fn designs_contact_line_order_and_own_sections() {
    let mut p = Project::example();
    let list: Value = serde_json::from_str(&cv_mcp::call(&mut p, "cv_list_designs", &json!({})).text).expect("json");
    assert!(list.as_array().expect("list").iter().any(|d| d["id"] == "rr-onyx"));
    assert!(cv_mcp::call(&mut p, "cv_set_layout", &json!({"layout": "rcv-harvard"})).change.is_some());
    assert_eq!(p.options.layout, "rcv-harvard");

    let out = cv_mcp::call(&mut p, "cv_contact_set", &json!({"extra": [{"label": "GitHub", "value": "github.com/ingrid"}, {"value": ""}], "order": ["linkedin", "contact_ai_1", "email"]}));
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(p.candidate.extra.len(), 1, "empty lines are dropped");
    let parts = cv_core::text::contact_parts(&p);
    assert!(parts[0].starts_with("linkedin.com") && parts[1] == "GitHub: github.com/ingrid", "{parts:?}");

    let added = cv_mcp::call(&mut p, "cv_custom_section_save", &json!({"title": "Prosjekter", "items": [{"title": "Ny barnehage", "date": "2023", "bullets": ["Koordinerte 12 underleverandører."]}]}));
    assert!(added.text.contains("section_ai_1"));
    assert!(cv_mcp::call(&mut p, "cv_section_order", &json!({"order": ["profile", "section_ai_1", "nonsense"]})).is_error);
    assert!(!cv_mcp::call(&mut p, "cv_section_order", &json!({"order": ["section_ai_1", "profile"]})).is_error);
    assert_eq!(cv_core::text::section_order(&p)[0], "section_ai_1");
    assert!(cv_mcp::call(&mut p, "cv_custom_section_remove", &json!({"id": "section_ai_1"})).change.is_some());
    assert!(p.custom_sections.is_empty() && !p.options.section_order.contains(&"section_ai_1".to_string()));
}

#[test]
fn colours_through_mcp() {
    let mut p = Project::example();
    let out = cv_mcp::call(&mut p, "cv_set_colors", &json!({"palette": {"accent": "#2e5e3e", "header_fill": "none", "ink": "blue"}, "section_colors": {"experience": "#7A2638"}}));
    assert!(out.change.is_some() && out.text.contains("ink"), "{}", out.text);
    assert_eq!(p.options.palette.accent.as_deref(), Some("#2E5E3E"));
    assert_eq!(p.options.palette.header_fill.as_deref(), Some("none"));
    assert!(p.options.palette.ink.is_none(), "a bad colour is ignored");
    assert_eq!(p.options.section_colors.get("experience").map(String::as_str), Some("#7A2638"));
    let faint = cv_mcp::call(&mut p, "cv_set_colors", &json!({"palette": {"ink": "#EEEEEE"}}));
    assert!(faint.text.contains("Readability"), "{}", faint.text);
    cv_mcp::call(&mut p, "cv_set_colors", &json!({"reset": true}));
    assert!(p.options.palette.is_empty() && p.options.section_colors.is_empty());
}

#[test]
fn page_setup_through_mcp_adds_pages_and_columns() {
    let mut p = Project::example();
    let out = cv_mcp::call(&mut p, "cv_page_setup", &json!({"margins": "narrow", "columns": "left", "page_breaks": ["education", "nonsense"], "page_breaks_after": ["profile"]}));
    assert!(out.text.contains("3 page(s)") && out.text.contains("advise 2") && out.text.contains("nonsense"), "{}", out.text);
    assert_eq!(p.options.page.page_breaks, vec!["education".to_string()]);
    assert_eq!(p.options.page.page_breaks_after, vec!["profile".to_string()]);
    assert!(cv_core::layouts::effective(&p).columns.is_some());
    cv_mcp::call(&mut p, "cv_page_setup", &json!({"reset": true}));
    assert!(p.options.page.is_default());
}

#[test]
fn headings_in_any_language() {
    let mut p = Project::example();
    assert!(cv_mcp::call(&mut p, "cv_set_headings", &json!({"language": "Deutsch"})).change.is_some());
    let plain = cv_core::checks::plain_text(&p);
    assert!(plain.contains("BERUFSERFAHRUNG") && plain.contains("heute"), "{plain}");
    let out = cv_mcp::call(&mut p, "cv_set_headings", &json!({"language": "Kiswahili", "experience": "Uzoefu wa kazi"}));
    assert!(out.text.contains("missing"), "{}", out.text);
    assert!(cv_core::checks::plain_text(&p).contains("UZOEFU WA KAZI"));
    assert!(cv_core::checks::plain_text(&p).contains("EDUCATION"), "missing ones fall back to English");
    cv_mcp::call(&mut p, "cv_set_headings", &json!({"language": "Norsk"}));
    assert!(p.options.headings.is_none() && !p.is_english());
    assert!(cv_core::checks::plain_text(&p).contains("ARBEIDSERFARING"));
}
