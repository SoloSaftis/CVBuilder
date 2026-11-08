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
    present: "Present",
};

pub fn labels(project: &Project) -> &'static Labels {
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

pub fn contact_parts(project: &Project) -> Vec<String> {
    let c = &project.candidate;
    let mut parts: Vec<String> = [&c.email, &c.phone, &c.city].iter().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
    if !c.linkedin.is_empty() {
        parts.push(linkedin_text(&c.linkedin));
    }
    parts
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
