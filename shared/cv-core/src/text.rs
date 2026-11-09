//! Shared wording and ordering, identical to the HTML builder so both apps produce the same text.
use crate::model::{Certification, Education, Interest, InterestStatus, Job, Project, Skill};

pub struct Labels {
    pub profile: &'static str,
    pub skills: &'static str,
    pub experience: &'static str,
    pub education: &'static str,
    pub certifications: &'static str,
    pub languages: &'static str,
    pub interests: &'static str,
    pub contact: &'static str,
    pub present: &'static str,
}

pub const NO: Labels = Labels {
    profile: "Profil",
    skills: "Kjernekompetanse",
    experience: "Arbeidserfaring",
    education: "Utdanning",
    certifications: "Kurs og sertifiseringer",
    languages: "Språk",
    interests: "Interesser",
    contact: "Kontakt",
    present: "nå",
};

pub const EN: Labels = Labels {
    profile: "Profile",
    skills: "Core skills",
    experience: "Professional experience",
    education: "Education",
    certifications: "Courses and certifications",
    languages: "Languages",
    interests: "Interests",
    contact: "Contact",
    present: "Present",
};

pub fn labels(project: &Project) -> &'static Labels {
    if let Some(h) = &project.options.headings {
        return crate::headings::labels(h);
    }
    if project.is_english() { &EN } else { &NO }
}

/// `2022-05` → `05.2022`.
fn month(value: &str) -> String {
    match value.split_once('-') {
        Some((year, month)) => format!("{month}.{year}"),
        None => value.to_string(),
    }
}

/// `05.2022 – 10.2024`, or `01.2026 – nå` for a current job.
pub fn period(job: &Job, labels: &Labels) -> String {
    if job.start.is_empty() {
        return String::new();
    }
    let end = match &job.end {
        None => labels.present.to_string(),
        Some(end) if end.is_empty() => String::new(),
        Some(end) => month(end),
    };
    format!("{} – {}", month(&job.start), end)
}

pub fn year_range(start: &str, end: &str) -> String {
    [start, end].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" – ")
}

/// Newest first; current jobs (no end date) on top.
pub fn sort_jobs(jobs: &[Job]) -> Vec<&Job> {
    let mut sorted: Vec<&Job> = jobs.iter().collect();
    sorted.sort_by(|a, b| {
        let end = |j: &Job| j.end.clone().unwrap_or_else(|| "9999-99".into());
        end(b).cmp(&end(a)).then_with(|| b.start.cmp(&a.start))
    });
    sorted
}

pub fn sort_education(items: &[Education]) -> Vec<&Education> {
    let mut sorted: Vec<&Education> = items.iter().collect();
    sorted.sort_by(|a, b| b.end.cmp(&a.end));
    sorted
}

pub fn sort_certifications(items: &[Certification]) -> Vec<&Certification> {
    let mut sorted: Vec<&Certification> = items.iter().collect();
    sorted.sort_by(|a, b| b.year.cmp(&a.year));
    sorted
}

/// Skills grouped in first-seen order: `[("Programmering", ["C/C++", "Python"])]`.
pub fn group_skills(skills: &[Skill]) -> Vec<(String, Vec<String>)> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for skill in skills {
        let key = skill.group.trim().to_string();
        match groups.iter_mut().find(|(group, _)| *group == key) {
            Some((_, names)) => names.push(skill.name.trim().to_string()),
            None => groups.push((key, vec![skill.name.trim().to_string()])),
        }
    }
    groups
}

pub fn linkedin_text(url: &str) -> String {
    url.trim_start_matches("https://").trim_start_matches("www.").trim_end_matches('/').to_string()
}

/// The contact line, in the person's order, without empty parts.
pub fn contact_parts(project: &Project) -> Vec<String> {
    contact_order(project).iter().map(|k| contact_value(project, k, false)).filter(|v| !v.is_empty()).collect()
}

/// "Tidligere svømmer", "Former: Swimming", "Interesse for kajakk" — like the HTML builder.
pub fn interest_text(item: &Interest, english: bool) -> String {
    let lower = item.label.chars().next().is_some_and(char::is_lowercase);
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| item.label.to_lowercase().starts_with(p));
    match item.status {
        InterestStatus::Former if !starts(&["tidligere", "former"]) => {
            format!("{}{}{}", if english { "Former" } else { "Tidligere" }, if lower { " " } else { ": " }, item.label)
        }
        InterestStatus::Interest if !starts(&["interesse for", "interest in"]) => {
            format!("{}{}", if english { "Interest in " } else { "Interesse for " }, item.label)
        }
        _ => item.label.clone(),
    }
}

pub fn selected_interests(project: &Project) -> Vec<String> {
    if !project.options.include_interests {
        return Vec::new();
    }
    project
        .interests
        .iter()
        .filter(|i| i.confirmed && project.options.interest_ids.contains(&i.id))
        .map(|i| interest_text(i, project.is_english()))
        .collect()
}

/// The recommended order of the contact line.
pub const CONTACT_KEYS: [&str; 4] = ["email", "phone", "city", "linkedin"];

/// The recommended order of the sections: what recruiters look for first.
pub const SECTION_KEYS: [&str; 7] = ["profile", "skills", "experience", "education", "certifications", "languages", "interests"];

/// The contact line's keys in the person's order: chosen keys first, then any not placed yet.
pub fn contact_order(project: &Project) -> Vec<String> {
    let c = &project.candidate;
    let known = |k: &str| CONTACT_KEYS.contains(&k) || c.extra.iter().any(|x| x.id == k);
    let mut out: Vec<String> = Vec::new();
    for k in &c.contact_order {
        if known(k) && !out.contains(k) {
            out.push(k.clone());
        }
    }
    for k in CONTACT_KEYS.iter().map(|k| k.to_string()).chain(c.extra.iter().map(|x| x.id.clone())) {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

/// One contact line part. `full` keeps the LinkedIn address whole (plain text for portals).
pub fn contact_value(project: &Project, key: &str, full: bool) -> String {
    let c = &project.candidate;
    match key {
        "email" => c.email.clone(),
        "phone" => c.phone.clone(),
        "city" => c.city.clone(),
        "linkedin" if full => c.linkedin.clone(),
        "linkedin" => if c.linkedin.is_empty() { String::new() } else { linkedin_text(&c.linkedin) },
        id => c.extra.iter().find(|x| x.id == id).map(|x| if x.value.trim().is_empty() { String::new() } else if x.label.trim().is_empty() { x.value.trim().to_string() } else { format!("{}: {}", x.label.trim(), x.value.trim()) }).unwrap_or_default(),
    }
}

/// The section keys in the person's order. Custom sections not placed yet come last.
pub fn section_order(project: &Project) -> Vec<String> {
    let known = |k: &str| SECTION_KEYS.contains(&k) || project.custom_sections.iter().any(|s| s.id == k);
    let mut out: Vec<String> = Vec::new();
    for k in &project.options.section_order {
        if known(k) && !out.contains(k) {
            out.push(k.clone());
        }
    }
    for k in SECTION_KEYS {
        if !out.iter().any(|x| x == k) {
            out.push(k.to_string());
        }
    }
    // Sections the person added and has not placed go at the bottom, in the order they were added.
    for s in &project.custom_sections {
        if !out.contains(&s.id) {
            out.push(s.id.clone());
        }
    }
    out
}

/// A section's heading in the CV's language.
pub fn section_title(project: &Project, key: &str) -> String {
    let l = labels(project);
    match key {
        "profile" => l.profile.into(),
        "skills" => l.skills.into(),
        "experience" => l.experience.into(),
        "education" => l.education.into(),
        "certifications" => l.certifications.into(),
        "languages" => l.languages.into(),
        "interests" => l.interests.into(),
        id => project.custom_sections.iter().find(|s| s.id == id).map(|s| s.title.clone()).unwrap_or_default(),
    }
}
