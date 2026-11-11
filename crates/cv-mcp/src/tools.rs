//! The tools an AI may use on the open CV. Each one is small and checked: dates must be readable,
//! ids must exist, and every change is described in one plain Norwegian line for the chat
//! («Endret profilteksten»), so the person sees what happened and can undo it.
use cv_core::checks::{self, Level};
use cv_core::model::{Certification, Education, Interest, InterestStatus, Job, Skill, SpokenLanguage};
use cv_core::model::{ContactField, CustomItem, CustomSection};
use cv_core::{Language, Project, document, layouts, text as cvtext};
use serde_json::{Map, Value, json};

/// A tool as offered to a model: name, what it does and the JSON Schema of its arguments.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
    /// True when the tool can change the CV.
    pub edits: bool,
}

/// What a tool call returned.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Sent back to the model.
    pub text: String,
    pub is_error: bool,
    /// When the CV changed: one line for the person, e.g. «Endret profilteksten».
    pub change: Option<String>,
}

impl Outcome {
    fn ok(text: impl Into<String>) -> Outcome {
        Outcome { text: text.into(), is_error: false, change: None }
    }
    fn changed(change: impl Into<String>) -> Outcome {
        let change = change.into();
        Outcome { text: format!("Done: {change}. The person sees the change in the preview now."), is_error: false, change: Some(change) }
    }
    fn error(text: impl Into<String>) -> Outcome {
        Outcome { text: text.into(), is_error: true, change: None }
    }
}

/// The rules every AI working on a CV follows. Sent as the MCP server's instructions and as
/// part of the chat's system prompt.
pub const INSTRUCTIONS: &str = "You are working on a person's CV in CVBuilder, which shows every change live. \
Use the cv_* tools: read the CV with cv_read before changing it, make changes with the editing tools, and run cv_check after larger edits. \
Rules that always apply: \
1) Never invent facts, employers, tools, numbers, results, dates or responsibilities that are not in the CV or that the person did not tell you. \
If a number would make a point stronger but you do not have it, write [tall] where the number goes; CVBuilder blocks export until the person fills it in. \
2) Write CV text in the CV's language (field \"cv_language\" in cv_read, e.g. Bokmål, English or Deutsch). Talk to the person in the language they write to you in. \
3) Follow recruiter and ATS practice: one column of real text, standard headings, 2-4 short bullets per job that start with a verb and show a result, the job ad's own words where they are true, \
no buzzwords, no \"I\", no personal data such as date of birth, marital status or nationality, at most two A4 pages. \
4) Keep answers short. Say what you changed and what the person should check or fill in.";

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

fn strings(description: &str) -> Value {
    json!({"type": "array", "items": {"type": "string"}, "description": description})
}

/// Every tool, in the order a model should see them.
pub fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "cv_read",
            description: "Read the whole CV as JSON: contact, profile, target job and ad, jobs (with ids), education, skills, languages, certifications, interests, layout and language. Call this before editing.",
            schema: schema(json!({}), &[]),
            edits: false,
        },
        Tool {
            name: "cv_check",
            description: "Run CVBuilder's checks: what blocks export, the recruiter/ATS checklist, which keywords from the ad are found or missing, and the page count (must be at most 2).",
            schema: schema(json!({}), &[]),
            edits: false,
        },
        Tool {
            name: "cv_update_profile",
            description: "Change contact details, headline or profile summary. Only the fields you pass are changed.",
            schema: schema(
                json!({
                    "name": {"type": "string"},
                    "headline": {"type": "string", "description": "Professional title, ideally the ad's own words for the role"},
                    "email": {"type": "string"},
                    "phone": {"type": "string", "description": "With country code, e.g. +47 900 00 000"},
                    "city": {"type": "string", "description": "City and country"},
                    "linkedin": {"type": "string", "description": "Full https:// address"},
                    "summary": {"type": "string", "description": "Short profile, 30-70 words"}
                }),
                &[],
            ),
            edits: true,
        },
        Tool {
            name: "cv_update_target",
            description: "Change the job being applied for: role, company, ad link, the ad text, or the list of important keywords from the ad.",
            schema: schema(
                json!({
                    "role": {"type": "string"},
                    "company": {"type": "string"},
                    "url": {"type": "string"},
                    "description": {"type": "string", "description": "The full job ad text"},
                    "keywords": strings("Important words from the ad; replaces the list")
                }),
                &[],
            ),
            edits: true,
        },
        Tool {
            name: "cv_job_save",
            description: "Add a job (leave out id) or change one (pass its id from cv_read). Only the fields you pass are changed. Dates are \"YYYY-MM\" or \"YYYY\"; end \"now\" marks the current job.",
            schema: schema(
                json!({
                    "id": {"type": "string", "description": "The job's id from cv_read; leave out to add a new job"},
                    "title": {"type": "string"},
                    "company": {"type": "string"},
                    "location": {"type": "string"},
                    "start": {"type": "string", "description": "YYYY-MM or YYYY"},
                    "end": {"type": "string", "description": "YYYY-MM, YYYY, or \"now\" for the current job"},
                    "bullets": strings("2-4 short bullets, each starting with a verb; replaces the job's bullets")
                }),
                &[],
            ),
            edits: true,
        },
        Tool {
            name: "cv_job_remove",
            description: "Remove a job by its id.",
            schema: schema(json!({"id": {"type": "string"}}), &["id"]),
            edits: true,
        },
        Tool {
            name: "cv_skills_set",
            description: "Replace the list of core skills. Group related skills (e.g. \"Automation\", \"Programming\"). Only skills the person really has.",
            schema: schema(
                json!({"skills": {"type": "array", "items": {"type": "object", "properties": {"group": {"type": "string"}, "name": {"type": "string"}}, "required": ["name"], "additionalProperties": false}}}),
                &["skills"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_education_set",
            description: "Replace the education list. Years are \"YYYY\".",
            schema: schema(
                json!({"education": {"type": "array", "items": {"type": "object", "properties": {
                    "qualification": {"type": "string"}, "institution": {"type": "string"}, "start": {"type": "string"}, "end": {"type": "string"}
                }, "required": ["qualification"], "additionalProperties": false}}}),
                &["education"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_certifications_set",
            description: "Replace the list of courses and certifications. Year is \"YYYY\".",
            schema: schema(
                json!({"certifications": {"type": "array", "items": {"type": "object", "properties": {
                    "name": {"type": "string"}, "issuer": {"type": "string"}, "year": {"type": "string"}
                }, "required": ["name"], "additionalProperties": false}}}),
                &["certifications"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_languages_set",
            description: "Replace the list of spoken languages and levels (e.g. Norwegian: native, English: fluent).",
            schema: schema(
                json!({"languages": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "level": {"type": "string"}}, "required": ["name"], "additionalProperties": false}}}),
                &["languages"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_interests_set",
            description: "Replace the interests. At most three can be shown (shown: true). Status is current, former or interest.",
            schema: schema(
                json!({
                    "include": {"type": "boolean", "description": "Whether the CV has an interests line at all"},
                    "interests": {"type": "array", "items": {"type": "object", "properties": {
                        "label": {"type": "string"}, "status": {"type": "string", "enum": ["current", "former", "interest"]}, "shown": {"type": "boolean"}
                    }, "required": ["label"], "additionalProperties": false}}
                }),
                &["interests"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_list_designs",
            description: "List the designs the CV can use: id, name, description, the kind of job it suits, and whether it has two columns (two columns look good but older applicant-tracking systems read them less reliably; prefer one column for large job portals).",
            schema: schema(json!({}), &[]),
            edits: false,
        },
        Tool {
            name: "cv_set_layout",
            description: "Choose the design by id (from cv_list_designs, e.g. classic, modern, nordic, compact). \"compact\" fits the most text per page.",
            schema: schema(json!({"layout": {"type": "string"}}), &["layout"]),
            edits: true,
        },
        Tool {
            name: "cv_set_colors",
            description: "Set the CV's colours, independent of the design. palette: accent (headings), ink (body text), muted (dates), rule (lines), header_fill (band behind the header, or \"none\"), header_text, heading_fill (band behind headings, or \"none\"); each \"#RRGGBB\", null to use the design's own. section_colors: a colour per section key (profile, skills, experience, education, certifications, languages, interests or a custom section id), null to remove. reset: true goes back to the design's colours. Keep text readable: dark text, headings with contrast.",
            schema: schema(
                json!({
                    "palette": {"type": "object", "properties": {
                        "accent": {"type": ["string", "null"]}, "ink": {"type": ["string", "null"]}, "muted": {"type": ["string", "null"]}, "rule": {"type": ["string", "null"]},
                        "header_fill": {"type": ["string", "null"]}, "header_text": {"type": ["string", "null"]}, "heading_fill": {"type": ["string", "null"]}
                    }, "additionalProperties": false},
                    "section_colors": {"type": "object", "additionalProperties": {"type": ["string", "null"]}},
                    "reset": {"type": "boolean"}
                }),
                &[],
            ),
            edits: true,
        },
        Tool {
            name: "cv_page_setup",
            description: "Page layout: size (a4, letter, legal), margins (normal, narrow, moderate, wide, custom with custom_mm [top, right, bottom, left]), columns (design, one, left, right – a sidebar for any design), text_delta (-1.5 to 2 points), spacing (0.8 to 1.3), page_breaks (section keys that start on a new page) and page_breaks_after (section keys after which the next section starts on a new page) – this is how the CV gets more pages; there is no page limit, but recruiters strongly advise one page for short careers and two for most people. Only the fields you pass change; reset: true restores the defaults.",
            schema: schema(
                json!({
                    "size": {"type": "string", "enum": ["a4", "letter", "legal"]},
                    "margins": {"type": "string", "enum": ["normal", "narrow", "moderate", "wide", "custom"]},
                    "custom_mm": {"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4},
                    "columns": {"type": "string", "enum": ["design", "one", "left", "right"]},
                    "text_delta": {"type": "number"},
                    "spacing": {"type": "number"},
                    "page_breaks": strings("Section keys that start on a new page"),
                    "page_breaks_after": strings("Section keys after which the next section starts on a new page"),
                    "reset": {"type": "boolean"}
                }),
                &[],
            ),
            edits: true,
        },
        Tool {
            name: "cv_set_headings",
            description: "Set the language of the CV's headings and dates. language: a code (de, fr, pt-BR, sr-Latn …) or the language's own name (Deutsch). Every language from cv_list_languages has built-in headings and dates. For any other language pass every heading yourself (profile, skills, experience, education, certifications, languages, interests, contact, present = the word for a current job's end date). Translate the CV text itself with the other tools.",
            schema: schema(
                json!({
                    "language": {"type": "string"},
                    "profile": {"type": "string"}, "skills": {"type": "string"}, "experience": {"type": "string"}, "education": {"type": "string"},
                    "certifications": {"type": "string"}, "languages": {"type": "string"}, "interests": {"type": "string"}, "contact": {"type": "string"}, "present": {"type": "string"}
                }),
                &["language"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_contact_set",
            description: "Set extra contact lines (e.g. GitHub, portfolio, driving licence) and the order of the contact line. Keys in order: email, phone, city, linkedin and the ids of extra lines. Extra lines replace the existing extra lines.",
            schema: schema(
                json!({
                    "extra": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "string"}, "label": {"type": "string"}, "value": {"type": "string"}}, "required": ["value"], "additionalProperties": false}},
                    "order": strings("Contact keys in the order they should appear")
                }),
                &[],
            ),
            edits: true,
        },
        Tool {
            name: "cv_section_order",
            description: "Set the order of the CV's sections. Keys: profile, skills, experience, education, certifications, languages, interests and custom section ids. The recommended order is the default; change it only when the person asks or it clearly helps.",
            schema: schema(json!({"order": strings("Section keys in order")}), &["order"]),
            edits: true,
        },
        Tool {
            name: "cv_custom_section_save",
            description: "Add a section of the person's own (leave out id), such as Projects, Volunteering or Publications, or replace one (pass its id). Items have a title, optional subtitle (organisation or place), optional date text and bullets.",
            schema: schema(
                json!({
                    "id": {"type": "string"},
                    "title": {"type": "string"},
                    "items": {"type": "array", "items": {"type": "object", "properties": {
                        "title": {"type": "string"}, "subtitle": {"type": "string"}, "date": {"type": "string"}, "bullets": strings("Short bullets, each starting with a verb")
                    }, "required": ["title"], "additionalProperties": false}}
                }),
                &["title", "items"],
            ),
            edits: true,
        },
        Tool {
            name: "cv_custom_section_remove",
            description: "Remove a section of the person's own by its id.",
            schema: schema(json!({"id": {"type": "string"}}), &["id"]),
            edits: true,
        },
        Tool {
            name: "cv_list_languages",
            description: "List the languages a CV can be in with built-in headings and dates: code and the language's own name.",
            schema: schema(json!({}), &[]),
            edits: false,
        },
        Tool {
            name: "cv_set_language",
            description: "Set the CV's language: its headings and dates. language: a code from cv_list_languages (no = Norwegian bokmål, en = English, de, fr, pt-BR …) or the language's own name. Rewrite the text yourself with the other tools when translating.",
            schema: schema(json!({"language": {"type": "string", "description": "A code such as no, en, de or pt-BR, or the language's own name"}}), &["language"]),
            edits: true,
        },
    ]
}

pub fn is_editing(name: &str) -> bool {
    tools().iter().any(|t| t.name == name && t.edits)
}

/// Runs one tool on the project.
pub fn call(project: &mut Project, name: &str, args: &Value) -> Outcome {
    let empty = Map::new();
    let a = args.as_object().unwrap_or(&empty);
    match name {
        "cv_read" => Outcome::ok(read(project)),
        "cv_check" => Outcome::ok(check(project)),
        "cv_update_profile" => update_profile(project, a),
        "cv_update_target" => update_target(project, a),
        "cv_job_save" => job_save(project, a),
        "cv_job_remove" => job_remove(project, a),
        "cv_skills_set" => skills_set(project, a),
        "cv_education_set" => education_set(project, a),
        "cv_certifications_set" => certifications_set(project, a),
        "cv_languages_set" => languages_set(project, a),
        "cv_interests_set" => interests_set(project, a),
        "cv_list_designs" => Outcome::ok(serde_json::to_string_pretty(&layouts::all().iter().map(|d| json!({"id": d.id, "name": d.name, "description": d.description, "category": layouts::category_label(&d.category), "two_columns": d.columns.is_some()})).collect::<Vec<_>>()).unwrap_or_default()),
        "cv_set_layout" => set_layout(project, a),
        "cv_set_colors" => set_colors(project, a),
        "cv_page_setup" => page_setup(project, a),
        "cv_set_headings" => set_headings(project, a),
        "cv_contact_set" => contact_set(project, a),
        "cv_section_order" => section_order(project, a),
        "cv_custom_section_save" => custom_section_save(project, a),
        "cv_custom_section_remove" => custom_section_remove(project, a),
        "cv_list_languages" => Outcome::ok(list_languages()),
        "cv_set_language" => set_language(project, a),
        _ => Outcome::error(format!("Unknown tool {name}. Use one of: {}.", tools().iter().map(|t| t.name).collect::<Vec<_>>().join(", "))),
    }
}

fn text(a: &Map<String, Value>, key: &str) -> Option<String> {
    a.get(key).and_then(Value::as_str).map(|s| s.trim().to_string())
}

fn list(a: &Map<String, Value>, key: &str) -> Option<Vec<String>> {
    a.get(key).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
}

fn objects<'a>(a: &'a Map<String, Value>, key: &str) -> Result<Vec<&'a Map<String, Value>>, Outcome> {
    a.get(key).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_object).collect()).ok_or_else(|| Outcome::error(format!("\"{key}\" must be a list.")))
}

fn read(p: &Project) -> String {
    let mut v = serde_json::to_value(p).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        // The portrait is a large image and the approval is the person's own; neither is for the model.
        if let Some(c) = o.get_mut("candidate").and_then(Value::as_object_mut) {
            let has = c.get("photo").and_then(Value::as_str).is_some_and(|s| !s.is_empty());
            c.insert("photo".into(), Value::String(if has { "(portrait added)".into() } else { String::new() }));
        }
        o.remove("approvals");
        o.remove("public_context");
        o.remove("version");
        let shown = p.options.headings.as_ref().map(|h| h.language.clone()).unwrap_or_else(|| if p.language == Language::En { "English".into() } else { "Bokmål".into() });
        o.insert("cv_language".into(), Value::String(shown));
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

fn today_index() -> i32 {
    let months = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 2_629_746).unwrap_or(0);
    i32::try_from(months).unwrap_or(0) + 1970 * 12
}

fn check(p: &Project) -> String {
    let pages = document::page_count(&document::build(p));
    let blocking: Vec<String> = checks::blocking(p).into_iter().filter(|b| !b.starts_with("approvals:")).collect();
    let (list, keywords) = checks::audit(p, pages.max(1), today_index());
    let advice: Vec<Value> = list
        .iter()
        .filter(|c| c.level != Level::Pass)
        .map(|c| json!({"level": if c.level == Level::Warn { "warn" } else { "info" }, "text": c.text, "tip": c.tip}))
        .collect();
    let passed: Vec<&str> = list.iter().filter(|c| c.level == Level::Pass).map(|c| c.text.as_str()).collect();
    serde_json::to_string_pretty(&json!({
        "pages": pages,
        "page_limit": 2,
        "blocks_export": blocking,
        "advice": advice,
        "passed": passed,
        "keywords_found": keywords.found,
        "keywords_missing": keywords.missing,
        "keyword_coverage_percent": keywords.coverage,
    }))
    .unwrap_or_default()
}

fn set(field: &mut String, value: Option<String>, label: &'static str, changed: &mut Vec<&'static str>) {
    if let Some(v) = value {
        if *field != v {
            *field = v;
            changed.push(label);
        }
    }
}

fn joined(changed: &[&str]) -> String {
    match changed {
        [] => String::new(),
        [one] => (*one).to_string(),
        [rest @ .., last] => format!("{} og {last}", rest.join(", ")),
    }
}

fn update_profile(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let c = &mut p.candidate;
    let mut changed = Vec::new();
    set(&mut c.name, text(a, "name"), "navnet", &mut changed);
    set(&mut c.headline, text(a, "headline"), "overskriften", &mut changed);
    set(&mut c.email, text(a, "email"), "e-posten", &mut changed);
    set(&mut c.phone, text(a, "phone"), "telefonnummeret", &mut changed);
    set(&mut c.city, text(a, "city"), "bostedet", &mut changed);
    set(&mut c.linkedin, text(a, "linkedin"), "LinkedIn-adressen", &mut changed);
    set(&mut c.summary, text(a, "summary"), "profilteksten", &mut changed);
    if changed.is_empty() { Outcome::ok("Nothing changed: the values were the same.") } else { Outcome::changed(format!("Endret {}", joined(&changed))) }
}

fn update_target(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let t = &mut p.target;
    let mut changed = Vec::new();
    set(&mut t.role, text(a, "role"), "stillingen", &mut changed);
    set(&mut t.company, text(a, "company"), "arbeidsgiveren", &mut changed);
    set(&mut t.url, text(a, "url"), "annonselenken", &mut changed);
    set(&mut t.description, text(a, "description"), "annonseteksten", &mut changed);
    if let Some(k) = list(a, "keywords") {
        if t.keywords != k {
            t.keywords = k;
            changed.push("fagordene fra annonsen");
        }
    }
    if changed.is_empty() { Outcome::ok("Nothing changed: the values were the same.") } else { Outcome::changed(format!("Endret {}", joined(&changed))) }
}

/// A date the model gave: `YYYY-MM` or `YYYY` (any form a person could type is accepted too).
fn month(value: &str, field: &str) -> Result<String, Outcome> {
    checks::parse_month(value).ok_or_else(|| Outcome::error(format!("{field} \"{value}\" is not a date. Use YYYY-MM or YYYY.")))
}

fn new_id(p: &Project) -> String {
    (1..).map(|n| format!("job_ai_{n}")).find(|id| !p.experience.iter().any(|j| &j.id == id)).unwrap_or_else(|| "job_ai".into())
}

fn job_label(j: &Job) -> String {
    match (j.title.is_empty(), j.company.is_empty()) {
        (false, false) => format!("{} hos {}", j.title, j.company),
        (false, true) => j.title.clone(),
        (true, false) => j.company.clone(),
        (true, true) => "en jobb".into(),
    }
}

fn job_save(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let start = match text(a, "start").map(|s| month(&s, "start")).transpose() {
        Ok(v) => v,
        Err(e) => return e,
    };
    let end = match text(a, "end") {
        Some(e) if checks::is_now(&e) => Some(None),
        Some(e) => match month(&e, "end") {
            Ok(m) => Some(Some(m)),
            Err(e) => return e,
        },
        None => None,
    };
    let (index, added) = match text(a, "id").filter(|id| !id.is_empty()) {
        Some(id) => match p.experience.iter().position(|j| j.id == id) {
            Some(i) => (i, false),
            None => return Outcome::error(format!("No job has id \"{id}\". Call cv_read for the ids, or leave out id to add a job.")),
        },
        None => {
            let id = new_id(p);
            p.experience.push(Job { id, end: Some(String::new()), confirmed: true, ..Default::default() });
            (p.experience.len() - 1, true)
        }
    };
    let Some(j) = p.experience.get_mut(index) else { return Outcome::error("The job could not be found.") };
    let mut changed = Vec::new();
    set(&mut j.title, text(a, "title"), "tittelen", &mut changed);
    set(&mut j.company, text(a, "company"), "arbeidsgiveren", &mut changed);
    set(&mut j.location, text(a, "location"), "stedet", &mut changed);
    set(&mut j.start, start, "startdatoen", &mut changed);
    if let Some(e) = end {
        if j.end != e {
            j.end = e;
            changed.push("sluttdatoen");
        }
    }
    if let Some(b) = list(a, "bullets") {
        if j.bullets != b {
            j.bullets = b;
            changed.push("punktene");
        }
    }
    let label = job_label(j);
    let id = j.id.clone();
    if added {
        let mut o = Outcome::changed(format!("La til jobben {label}"));
        o.text = format!("Added the job with id \"{id}\". The person sees it in the preview now.");
        o
    } else if changed.is_empty() {
        Outcome::ok("Nothing changed: the values were the same.")
    } else {
        Outcome::changed(format!("Endret {} for {label}", joined(&changed)))
    }
}

fn job_remove(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let id = text(a, "id").unwrap_or_default();
    match p.experience.iter().position(|j| j.id == id) {
        Some(i) => {
            let j = p.experience.remove(i);
            Outcome::changed(format!("Fjernet jobben {}", job_label(&j)))
        }
        None => Outcome::error(format!("No job has id \"{id}\".")),
    }
}

fn skills_set(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let items = match objects(a, "skills") {
        Ok(i) => i,
        Err(e) => return e,
    };
    let skills: Vec<Skill> = items
        .iter()
        .filter_map(|o| {
            let name = text(o, "name").filter(|n| !n.is_empty())?;
            // Keep what the person wrote as evidence for a skill that stays.
            let evidence = p.skills.iter().find(|s| s.name.eq_ignore_ascii_case(&name)).map(|s| s.evidence.clone()).unwrap_or_default();
            Some(Skill { group: text(o, "group").unwrap_or_default(), name, evidence, confirmed: true })
        })
        .collect();
    let n = skills.len();
    p.skills = skills;
    Outcome::changed(format!("Oppdaterte kjernekompetansen ({n} punkter)"))
}

fn education_set(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let items = match objects(a, "education") {
        Ok(i) => i,
        Err(e) => return e,
    };
    let mut out = Vec::new();
    for o in items {
        let year = |key: &str| -> Result<String, Outcome> {
            let v = text(o, key).unwrap_or_default();
            checks::parse_year(&v).ok_or_else(|| Outcome::error(format!("Education {key} \"{v}\" is not a year. Use YYYY.")))
        };
        let (start, end) = match (year("start"), year("end")) {
            (Ok(s), Ok(e)) => (s, e),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        out.push(Education { qualification: text(o, "qualification").unwrap_or_default(), institution: text(o, "institution").unwrap_or_default(), start, end });
    }
    let n = out.len();
    p.education = out;
    Outcome::changed(format!("Oppdaterte utdanningen ({n})"))
}

fn certifications_set(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let items = match objects(a, "certifications") {
        Ok(i) => i,
        Err(e) => return e,
    };
    let mut out = Vec::new();
    for o in items {
        let v = text(o, "year").unwrap_or_default();
        let Some(year) = checks::parse_year(&v) else { return Outcome::error(format!("Certification year \"{v}\" is not a year. Use YYYY.")) };
        out.push(Certification { name: text(o, "name").unwrap_or_default(), issuer: text(o, "issuer").unwrap_or_default(), year });
    }
    let n = out.len();
    p.certifications = out;
    Outcome::changed(format!("Oppdaterte kurs og sertifiseringer ({n})"))
}

fn languages_set(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let items = match objects(a, "languages") {
        Ok(i) => i,
        Err(e) => return e,
    };
    p.languages = items.iter().map(|o| SpokenLanguage { name: text(o, "name").unwrap_or_default(), level: text(o, "level").unwrap_or_default() }).collect();
    Outcome::changed("Oppdaterte språkene")
}

fn interests_set(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let items = match objects(a, "interests") {
        Ok(i) => i,
        Err(e) => return e,
    };
    let mut interests = Vec::new();
    let mut shown = Vec::new();
    for (n, o) in items.iter().enumerate() {
        let label = text(o, "label").unwrap_or_default();
        if label.is_empty() {
            continue;
        }
        let status = match text(o, "status").as_deref() {
            Some("former") => InterestStatus::Former,
            Some("interest") => InterestStatus::Interest,
            _ => InterestStatus::Current,
        };
        let old = p.interests.iter().find(|i| i.label.eq_ignore_ascii_case(&label));
        let id = old.map(|i| i.id.clone()).unwrap_or_else(|| format!("interest_ai_{}", n + 1));
        if o.get("shown").and_then(Value::as_bool).unwrap_or(true) && shown.len() < 3 {
            shown.push(id.clone());
        }
        interests.push(Interest {
            id,
            label,
            status,
            description: old.map(|i| i.description.clone()).unwrap_or_default(),
            category: old.map(|i| i.category.clone()).unwrap_or_else(|| "other".into()),
            tags: old.map(|i| i.tags.clone()).unwrap_or_default(),
            confirmed: true,
            specificity: old.map(|i| i.specificity).unwrap_or(1),
        });
    }
    p.interests = interests;
    p.options.interest_ids = shown;
    if let Some(include) = a.get("include").and_then(Value::as_bool) {
        p.options.include_interests = include;
    }
    Outcome::changed("Oppdaterte interessene")
}

fn set_layout(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let id = text(a, "layout").unwrap_or_default();
    if !layouts::exists(&id) {
        return Outcome::error(format!("Unknown design {id:?}. Call cv_list_designs for the ids."));
    }
    let name = layouts::design(&id).name;
    p.options.layout = id;
    Outcome::changed(format!("Byttet design til {name}"))
}

fn set_colors(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    if !["reset", "palette", "section_colors"].iter().any(|k| a.contains_key(*k)) {
        return Outcome::ok("Nothing changed: pass palette, section_colors and/or reset.");
    }
    if a.get("reset").and_then(Value::as_bool) == Some(true) {
        p.options.palette = Default::default();
        p.options.section_colors.clear();
    }
    let mut bad = Vec::new();
    if let Some(pal) = a.get("palette").and_then(Value::as_object) {
        let fields: [(&str, &mut Option<String>); 7] = [
            ("accent", &mut p.options.palette.accent),
            ("ink", &mut p.options.palette.ink),
            ("muted", &mut p.options.palette.muted),
            ("rule", &mut p.options.palette.rule),
            ("header_fill", &mut p.options.palette.header_fill),
            ("header_text", &mut p.options.palette.header_text),
            ("heading_fill", &mut p.options.palette.heading_fill),
        ];
        for (key, slot) in fields {
            match pal.get(key) {
                Some(Value::Null) => *slot = None,
                Some(Value::String(v)) => {
                    let none_ok = matches!(key, "header_fill" | "heading_fill");
                    if layouts::palette_value_ok(v) && (none_ok || v != "none") {
                        *slot = Some(v.to_uppercase().replace("NONE", "none"));
                    } else {
                        bad.push(key.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(map) = a.get("section_colors").and_then(Value::as_object) {
        for (key, v) in map {
            match v {
                Value::Null => {
                    p.options.section_colors.remove(key);
                }
                Value::String(c) if layouts::palette_value_ok(c) && c != "none" => {
                    p.options.section_colors.insert(key.clone(), c.to_uppercase());
                }
                _ => bad.push(format!("section_colors.{key}")),
            }
        }
    }
    let warnings = layouts::effective(p).color_warnings();
    let mut o = Outcome::changed("Endret fargene");
    o.text = format!(
        "Done.{}{}",
        if bad.is_empty() { String::new() } else { format!(" Ignored (not a #RRGGBB colour): {}.", bad.join(", ")) },
        if warnings.is_empty() { String::new() } else { format!(" Readability warnings: {}", warnings.join(" ")) }
    );
    o
}

fn page_setup(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    use cv_core::model::{ColumnsChoice, Margins, PageSetup, PageSize};
    if a.get("reset").and_then(Value::as_bool) == Some(true) {
        p.options.page = PageSetup::default();
        p.options.page_limit = 2;
    }
    let page = &mut p.options.page;
    let mut bad = Vec::new();
    match text(a, "size").as_deref() {
        Some("a4") => page.size = PageSize::A4,
        Some("letter") => page.size = PageSize::Letter,
        Some("legal") => page.size = PageSize::Legal,
        Some(other) => bad.push(format!("size {other}")),
        None => {}
    }
    match text(a, "margins").as_deref() {
        Some("normal") => page.margins = Margins::Normal,
        Some("narrow") => page.margins = Margins::Narrow,
        Some("moderate") => page.margins = Margins::Moderate,
        Some("wide") => page.margins = Margins::Wide,
        Some("custom") => page.margins = Margins::Custom,
        Some(other) => bad.push(format!("margins {other}")),
        None => {}
    }
    if let Some(mm) = a.get("custom_mm").and_then(Value::as_array) {
        let v: Vec<f32> = mm.iter().filter_map(Value::as_f64).map(|x| (x as f32).clamp(5.0, 40.0)).collect();
        if let [t, r, b, l] = v[..] {
            page.custom_mm = [t, r, b, l];
            page.margins = Margins::Custom;
        } else {
            bad.push("custom_mm needs four numbers".into());
        }
    }
    match text(a, "columns").as_deref() {
        Some("design") => page.columns = None,
        Some("one") => page.columns = Some(ColumnsChoice::One),
        Some("left") => page.columns = Some(ColumnsChoice::Left),
        Some("right") => page.columns = Some(ColumnsChoice::Right),
        Some(other) => bad.push(format!("columns {other}")),
        None => {}
    }
    if let Some(v) = a.get("text_delta").and_then(Value::as_f64) {
        page.text_delta = (v as f32).clamp(-1.5, 2.0);
    }
    if let Some(v) = a.get("spacing").and_then(Value::as_f64) {
        page.spacing = (v as f32).clamp(0.8, 1.3);
    }
    if let Some(keys) = list(a, "page_breaks") {
        let order = cvtext::section_order(p);
        let (ok, unknown): (Vec<String>, Vec<String>) = keys.into_iter().partition(|k| order.contains(k));
        bad.extend(unknown.into_iter().map(|k| format!("page_breaks {k}")));
        p.options.page.page_breaks = ok;
    }
    if let Some(keys) = list(a, "page_breaks_after") {
        let order = cvtext::section_order(p);
        let (ok, unknown): (Vec<String>, Vec<String>) = keys.into_iter().partition(|k| order.contains(k));
        bad.extend(unknown.into_iter().map(|k| format!("page_breaks_after {k}")));
        p.options.page.page_breaks_after = ok;
    }
    let pages = document::page_count(&document::build(p));
    let mut o = Outcome::changed("Endret sideoppsettet");
    let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 2_629_746).unwrap_or(0) as i32 + 1970 * 12;
    let advice = cv_core::checks::recommended_pages(p, today);
    o.text = format!("Done. The CV is now {pages} page(s); recruiters would advise {advice}.{}", if bad.is_empty() { String::new() } else { format!(" Ignored: {}.", bad.join(", ")) });
    o
}

/// A built-in CV language by its own name («Deutsch») or its code («de», «pt-BR»), any case.
fn built_in_language(wanted: &str) -> Option<&'static str> {
    let w = wanted.trim();
    cv_core::headings::built_in_languages()
        .into_iter()
        .find(|n| n.eq_ignore_ascii_case(w) || n.to_lowercase() == w.to_lowercase() || cv_core::headings::code_of(n).is_some_and(|c| c.eq_ignore_ascii_case(w)))
}

/// Every language the CV can be in without writing headings yourself.
fn list_languages() -> String {
    let mut all = vec![json!({"code": "nb", "name": "Bokmål"}), json!({"code": "en", "name": "English"})];
    all.extend(cv_core::headings::built_in_languages().into_iter().map(|n| json!({"code": cv_core::headings::code_of(n).unwrap_or_default(), "name": n})));
    serde_json::to_string_pretty(&all).unwrap_or_default()
}

fn set_headings(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let typed = text(a, "language").unwrap_or_default();
    // A code or any spelling of a built-in language gives that language's own name.
    let language = built_in_language(&typed).map_or(typed, str::to_string);
    let lower = language.to_lowercase();
    if ["norsk", "bokmål", "bokmal", "norwegian", "no", "nb"].contains(&lower.as_str()) {
        p.options.headings = None;
        p.language = Language::No;
        return Outcome::changed("Overskriftene er på bokmål");
    }
    if ["english", "engelsk", "en"].contains(&lower.as_str()) {
        p.options.headings = None;
        p.language = Language::En;
        return Outcome::changed("Overskriftene er på engelsk");
    }
    if language.is_empty() {
        return Outcome::error("Pass the language's own name, e.g. Deutsch.");
    }
    let mut h = cv_core::headings::built_in(&language).unwrap_or_else(|| cv_core::headings::Headings { language: language.clone(), ..Default::default() });
    for (key, field) in [
        ("profile", &mut h.profile),
        ("skills", &mut h.skills),
        ("experience", &mut h.experience),
        ("education", &mut h.education),
        ("certifications", &mut h.certifications),
        ("languages", &mut h.languages),
        ("interests", &mut h.interests),
        ("contact", &mut h.contact),
        ("present", &mut h.present),
    ] {
        if let Some(v) = text(a, key).filter(|v| !v.is_empty()) {
            *field = v;
        }
    }
    let complete = h.is_complete();
    // Dates and interests follow English conventions outside Norwegian.
    p.language = Language::En;
    p.options.headings = Some(h);
    let mut o = Outcome::changed(format!("Overskriftene er på {language}"));
    if !complete {
        o.text = format!("Done, but some headings were missing and show in English until you pass them: profile, skills, experience, education, certifications, languages, interests, contact, present for {language}.");
    }
    o
}

fn contact_set(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let mut changed = Vec::new();
    if let Some(items) = a.get("extra").and_then(Value::as_array) {
        let mut extra = Vec::new();
        for (n, o) in items.iter().filter_map(Value::as_object).enumerate() {
            let value = text(o, "value").unwrap_or_default();
            if value.is_empty() {
                continue;
            }
            let id = text(o, "id").filter(|i| !i.is_empty() && !cvtext::CONTACT_KEYS.contains(&i.as_str())).unwrap_or_else(|| format!("contact_ai_{}", n + 1));
            extra.push(ContactField { id, label: text(o, "label").unwrap_or_default(), value });
        }
        p.candidate.extra = extra;
        changed.push("ekstra kontaktlinjer");
    }
    if let Some(order) = list(a, "order") {
        p.candidate.contact_order = order;
        changed.push("rekkefølgen i kontaktlinjen");
    }
    if changed.is_empty() {
        return Outcome::ok("Nothing changed: pass extra and/or order.");
    }
    let mut o = Outcome::changed(format!("Endret {}", joined(&changed)));
    o.text = format!("Done. The contact line is now: {}", cvtext::contact_parts(p).join(" · "));
    o
}

fn section_order(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let order = list(a, "order").unwrap_or_default();
    let unknown: Vec<&String> = order.iter().filter(|k| !cvtext::SECTION_KEYS.contains(&k.as_str()) && !p.custom_sections.iter().any(|s| &s.id == *k)).collect();
    if !unknown.is_empty() {
        return Outcome::error(format!("Unknown section keys: {unknown:?}."));
    }
    p.options.section_order = order;
    let mut o = Outcome::changed("Endret rekkefølgen på seksjonene");
    o.text = format!("Done. Section order: {}", cvtext::section_order(p).join(", "));
    o
}

fn custom_section_save(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let title = text(a, "title").unwrap_or_default();
    if title.is_empty() {
        return Outcome::error("The section needs a title.");
    }
    let items = match objects(a, "items") {
        Ok(i) => i,
        Err(e) => return e,
    };
    let items: Vec<CustomItem> = items
        .iter()
        .map(|o| CustomItem { title: text(o, "title").unwrap_or_default(), subtitle: text(o, "subtitle").unwrap_or_default(), date: text(o, "date").unwrap_or_default(), bullets: list(o, "bullets").unwrap_or_default() })
        .collect();
    match text(a, "id").filter(|i| !i.is_empty()) {
        Some(id) => match p.custom_sections.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                s.title = title.clone();
                s.items = items;
                Outcome::changed(format!("Endret seksjonen «{title}»"))
            }
            None => Outcome::error(format!("No section has id \"{id}\".")),
        },
        None => {
            let id = (1..).map(|n| format!("section_ai_{n}")).find(|id| !p.custom_sections.iter().any(|s| &s.id == id)).unwrap_or_default();
            p.custom_sections.push(CustomSection { id: id.clone(), title: title.clone(), items });
            let mut o = Outcome::changed(format!("La til seksjonen «{title}»"));
            o.text = format!("Added the section with id \"{id}\". The person sees it in the preview now.");
            o
        }
    }
}

fn custom_section_remove(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let id = text(a, "id").unwrap_or_default();
    match p.custom_sections.iter().position(|s| s.id == id) {
        Some(i) => {
            let s = p.custom_sections.remove(i);
            p.options.section_order.retain(|k| k != &id);
            Outcome::changed(format!("Fjernet seksjonen «{}»", s.title))
        }
        None => Outcome::error(format!("No section has id \"{id}\".")),
    }
}

fn set_language(p: &mut Project, a: &Map<String, Value>) -> Outcome {
    let wanted = text(a, "language").unwrap_or_default();
    let lower = wanted.trim().to_lowercase();
    let norwegian = ["no", "nb", "norsk", "bokmål", "bokmal", "norwegian"].contains(&lower.as_str());
    let english = ["en", "en-us", "english", "engelsk"].contains(&lower.as_str());
    // Only the language is passed on, so the headings and dates are the built-in ones.
    let only = |language: &str| Map::from_iter([("language".to_string(), json!(language))]);
    match built_in_language(&wanted) {
        _ if norwegian => set_headings(p, &only("no")),
        _ if english => set_headings(p, &only("en")),
        Some(name) => set_headings(p, &only(name)),
        None => Outcome::error(format!("CVBuilder has no built-in headings for {wanted:?}. Call cv_list_languages for the codes, or use cv_set_headings and pass every heading yourself.")),
    }
}
