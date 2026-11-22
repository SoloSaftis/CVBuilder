//! What the AI may help with, and the rules it must follow. Every answer comes back as a
//! suggestion the person reviews and accepts; nothing is changed in the CV automatically.
//!
//! The shared rule for every task: never invent facts. A number the CV does not contain is
//! never made up; where a figure would make a point stronger the model writes «[tall]», and the
//! builder's placeholder check blocks export until the person fills it in or removes it.
use cv_core::Project;
use serde::Deserialize;

use crate::providers::{AiFailure, Endpoint, complete};

const RULES_NO: &str = "Du hjelper en person med å forbedre sin egen CV på norsk bokmål. Regler som alltid gjelder: \
Finn aldri på fakta, arbeidsgivere, verktøy, tall, resultater eller ansvar som ikke står i teksten du får. \
Hvis et tall ville gjort et punkt sterkere, men tallet ikke står i teksten, skriv «[tall]» der tallet skulle stått. \
Skriv kort, konkret og uten moteord som «løsningsorientert» eller «engasjert». Start punkter med et verb. Ikke skriv «jeg». \
Ikke ta med personopplysninger som fødselsdato, sivilstand eller nasjonalitet. Svar bare med JSON i formatet du blir bedt om.";

const RULES_EN: &str = "You help a person improve their own CV in English. Rules that always apply: \
Never invent facts, employers, tools, numbers, results or responsibilities that are not in the text you are given. \
If a number would make a point stronger but it is not in the text, write \"[tall]\" where the number would go. \
Be short and concrete, without buzzwords. Start bullets with a verb. Do not write \"I\". \
Do not include personal data such as date of birth, marital status or nationality. Reply only with JSON in the format requested.";

fn rules(project: &Project) -> &'static str {
    if project.is_english() { RULES_EN } else { RULES_NO }
}

/// The JSON object in a reply, even if a model wraps it in prose or a code fence.
fn json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

fn parse<T: for<'de> Deserialize<'de>>(endpoint: &Endpoint, text: &str) -> Result<T, AiFailure> {
    json_object(text).and_then(|j| serde_json::from_str(j).ok()).ok_or(AiFailure::Unreadable(endpoint.service.name()))
}

fn target(project: &Project) -> String {
    let t = &project.target;
    format!("Stilling: {}\nArbeidsgiver: {}\nViktige ord fra annonsen: {}\nAnnonse:\n{}", t.role, t.company, t.keywords.join(", "), t.description.chars().take(6000).collect::<String>())
}

#[derive(Deserialize)]
struct Bullets {
    bullets: Vec<String>,
}

/// Rewrites one job's bullets as short action-and-result points aimed at the target job.
pub fn improve_bullets(endpoint: &Endpoint, model: &str, project: &Project, job: usize) -> Result<Vec<String>, AiFailure> {
    let j = project.experience.get(job).ok_or(AiFailure::Other("CVBuilder", "fant ikke jobben".into()))?;
    let user = format!(
        "{}\n\nJobb: {} hos {}\nNåværende punkter:\n- {}\n\nSkriv punktene om til 2–5 korte punkter (høyst to linjer hver) som viser handling og resultat, og bruker annonsens ord der det er sant. \
Behold alle fakta, ikke legg til noe nytt. Svar som JSON: {{\"bullets\": [\"…\"]}}",
        target(project),
        j.title,
        j.company,
        j.bullets.join("\n- ")
    );
    let reply: Bullets = parse(endpoint, &complete(endpoint, model, rules(project), &user)?)?;
    let bullets: Vec<String> = reply.bullets.into_iter().map(|b| b.trim().trim_start_matches(['-', '•']).trim().to_string()).filter(|b| !b.is_empty()).take(5).collect();
    if bullets.is_empty() { Err(AiFailure::Unreadable(endpoint.service.name())) } else { Ok(bullets) }
}

#[derive(Deserialize)]
struct Summary {
    summary: String,
}

/// Writes a 30–70 word profile from what the CV already says.
pub fn write_summary(endpoint: &Endpoint, model: &str, project: &Project) -> Result<String, AiFailure> {
    let cv = cv_core::checks::plain_text(project);
    let user = format!(
        "{}\n\nCV:\n{}\n\nSkriv en profil på 40–60 ord: hvem personen er faglig, det viktigste personen kan som passer stillingen, og hva personen vil bidra med. \
Bruk bare det som står i CV-en. Svar som JSON: {{\"summary\": \"…\"}}",
        target(project),
        cv
    );
    let reply: Summary = parse(endpoint, &complete(endpoint, model, rules(project), &user)?)?;
    Ok(reply.summary.trim().to_string())
}

#[derive(Deserialize)]
struct Keywords {
    keywords: Vec<String>,
}

/// The skills, tools, certificates and duties a recruiter would search for in this ad.
pub fn keywords(endpoint: &Endpoint, model: &str, project: &Project) -> Result<Vec<String>, AiFailure> {
    let user = format!(
        "{}\n\nList 10–20 fagord, verktøy, sertifikater og oppgaver som en rekrutterer eller et søknadssystem ville søkt etter i denne annonsen. \
Bruk annonsens egne ord og skrivemåte. Ikke ta med generelle egenskaper som «selvstendig». Svar som JSON: {{\"keywords\": [\"…\"]}}",
        target(project)
    );
    let reply: Keywords = parse(endpoint, &complete(endpoint, model, rules(project), &user)?)?;
    Ok(reply.keywords.into_iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty() && k.chars().count() <= 80).take(30).collect())
}

#[derive(Deserialize)]
struct TranslatedJob {
    title: String,
    bullets: Vec<String>,
}

#[derive(Deserialize)]
struct Translation {
    headline: String,
    summary: String,
    jobs: Vec<TranslatedJob>,
    skills: Vec<[String; 2]>,
    education: Vec<String>,
    languages: Vec<[String; 2]>,
}

/// A copy of the CV in the other language (Bokmål ↔ English). Names of employers, schools and
/// products stay as they are. The person reviews the whole copy before using it.
pub fn translate(endpoint: &Endpoint, model: &str, project: &Project) -> Result<Project, AiFailure> {
    let to_english = !project.is_english();
    let source = serde_json::json!({
        "headline": project.candidate.headline,
        "summary": project.candidate.summary,
        "jobs": project.experience.iter().map(|j| serde_json::json!({"title": j.title, "bullets": j.bullets})).collect::<Vec<_>>(),
        "skills": project.skills.iter().map(|s| [s.group.clone(), s.name.clone()]).collect::<Vec<_>>(),
        "education": project.education.iter().map(|e| e.qualification.clone()).collect::<Vec<_>>(),
        "languages": project.languages.iter().map(|l| [l.name.clone(), l.level.clone()]).collect::<Vec<_>>(),
    });
    let user = format!(
        "Oversett denne CV-teksten til {}. Behold betydningen nøyaktig, ikke legg til eller fjern noe. Navn på arbeidsgivere, skoler, produkter og standarder (for eksempel IEC 61131-3) oversettes ikke. \
Stillingstitler oversettes bare hvis det finnes en vanlig tilsvarende tittel. Behold samme antall elementer og samme rekkefølge. Svar med JSON i nøyaktig samme form:\n{}",
        if to_english { "naturlig britisk/internasjonalt engelsk" } else { "naturlig norsk bokmål" },
        source
    );
    let t: Translation = parse(endpoint, &complete(endpoint, model, rules(project), &user)?)?;
    let shapes_match = t.jobs.len() == project.experience.len()
        && t.skills.len() == project.skills.len()
        && t.education.len() == project.education.len()
        && t.languages.len() == project.languages.len()
        && t.jobs.iter().zip(&project.experience).all(|(a, b)| a.bullets.len() == b.bullets.len());
    if !shapes_match {
        return Err(AiFailure::Other(endpoint.service.name(), "oversettelsen manglet deler av CV-en; prøv igjen".into()));
    }
    let mut copy = project.clone();
    copy.language = if to_english { cv_core::Language::En } else { cv_core::Language::No };
    copy.candidate.headline = t.headline;
    copy.candidate.summary = t.summary;
    for (job, tj) in copy.experience.iter_mut().zip(t.jobs) {
        job.title = tj.title;
        job.bullets = tj.bullets;
    }
    for (skill, [group, name]) in copy.skills.iter_mut().zip(t.skills) {
        skill.group = group;
        skill.name = name;
    }
    for (edu, q) in copy.education.iter_mut().zip(t.education) {
        edu.qualification = q;
    }
    for (lang, [name, level]) in copy.languages.iter_mut().zip(t.languages) {
        lang.name = name;
        lang.level = level;
    }
    copy.approvals.facts_confirmed = false;
    Ok(copy)
}

#[derive(Deserialize)]
struct SortedRequirement {
    text: String,
    need: String,
}

#[derive(Deserialize)]
struct SortedRequirements {
    requirements: Vec<SortedRequirement>,
}

/// The system and user text that asks an AI to sort the ad's requirements into must-haves and
/// nice-to-haves (to-do 045). `found` is what CVBuilder read offline, as a starting point. The
/// CV is not sent: only the ad.
pub fn requirements_request(project: &Project, found: &[(String, cv_core::requirements::Need)]) -> (String, String) {
    use cv_core::requirements::Need;
    let listed: Vec<String> = found.iter().map(|(t, n)| format!("- [{}] {t}", if *n == Need::Must { "must" } else { "nice" })).collect();
    let user = format!(
        "Annonse:
{}

CVBuilder fant disse kravene uten AI (kan ha feil):
{}

Les annonsen og lag listen over kravene til søkeren: hver kvalifikasjon, utdanning, erfaring, sertifikat, språk eller førerkort annonsen ber om. Merk hvert krav «must» hvis annonsen krever det, og «nice» hvis det er ønskelig, en fordel eller et pluss. Arbeidsoppgaver, hva arbeidsgiveren tilbyr og personlige egenskaper uten noe som kan vises i en CV er ikke krav. Bruk annonsens egne ord, på annonsens språk, ett krav per punkt, og finn aldri på krav som ikke står i annonsen. Svar som JSON: {{\"requirements\": [{{\"text\": \"…\", \"need\": \"must\"}}]}}",
        project.target.description.chars().take(8000).collect::<String>(),
        if listed.is_empty() { "(ingen)".to_string() } else { listed.join("
") }
    );
    (rules(project).to_string(), user)
}

/// The AI's sorted requirements, or None when the reply is not the JSON asked for. Unknown
/// kinds, empty and overlong lines are dropped; at most 40 are kept.
pub fn parse_requirements(reply: &str) -> Option<Vec<(String, cv_core::requirements::Need)>> {
    use cv_core::requirements::Need;
    let sorted: SortedRequirements = serde_json::from_str(json_object(reply)?).ok()?;
    let list: Vec<(String, Need)> = sorted
        .requirements
        .into_iter()
        .filter_map(|r| {
            let need = match r.need.trim().to_lowercase().as_str() {
                "must" | "krav" | "required" => Need::Must,
                "nice" | "fordel" | "ønskelig" | "preferred" => Need::Nice,
                _ => return None,
            };
            let text = r.text.trim().trim_start_matches(['-', '•', '*']).trim().to_string();
            (!text.is_empty() && text.chars().count() <= 300).then_some((text, need))
        })
        .take(40)
        .collect();
    Some(list)
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_json_inside_prose_and_fences() {
        assert_eq!(super::json_object("Here:\n```json\n{\"a\": 1}\n```"), Some("{\"a\": 1}"));
        assert_eq!(super::json_object("no json"), None);
    }

    #[test]
    fn sorted_requirements_are_read_and_checked() {
        use cv_core::requirements::Need;
        let reply = "Sure:
{\"requirements\": [{\"text\": \"- Fagbrev som elektriker\", \"need\": \"must\"}, {\"text\": \"KNX\", \"need\": \"Nice\"}, {\"text\": \"Firmabil\", \"need\": \"offer\"}, {\"text\": \" \", \"need\": \"must\"}]}";
        assert_eq!(super::parse_requirements(reply), Some(vec![("Fagbrev som elektriker".to_string(), Need::Must), ("KNX".to_string(), Need::Nice)]));
        assert_eq!(super::parse_requirements("no"), None);
        let mut p = cv_core::Project::example();
        p.target.description = "Krav
- Fagbrev".into();
        let (_, user) = super::requirements_request(&p, &[("Fagbrev".into(), Need::Must)]);
        assert!(user.contains("- [must] Fagbrev") && user.contains("Krav
- Fagbrev"));
        assert!(!user.contains(&p.candidate.name), "the CV itself is not sent");
    }
}
