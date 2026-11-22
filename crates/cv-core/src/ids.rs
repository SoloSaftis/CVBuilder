//! Stable ids for the items of a CV (project schema version 3), and what is hidden in this copy.
//!
//! Every job, bullet, education, skill, language, course, the person's own sections and their
//! entries (and, as before, interests and extra contact lines) carry an id that stays the same
//! while the item is edited, moved or translated. A bullet's id sits in `bullet_ids`, beside
//! `bullets` at the same position, so the bullets themselves stay plain text for every program
//! that reads the file (the HTML builder, scripts, the AI tools).
//!
//! What uses the ids:
//! - 041: `options.hidden` lists the ids left out of this CV. Hidden items stay in the form and
//!   the file, but not in the preview, PDF, Word, plain text, checks and page count
//!   ([`Project::visible`]). A tailored copy is the same file with other hide choices.
//! - Later work keys its own bookkeeping by the same ids (which lines the AI wrote and the person
//!   has not reviewed; which item of a language version was translated from which), so it never
//!   needs another schema change to find an item again: [`Project::items`] lists them all.
//!
//! The rules both apps follow (the HTML builder's `contract.js` does exactly the same):
//! - Ids are `[A-Za-z0-9_-]`, unique in the whole project.
//! - Reading an older file gives every item a deterministic id (`{prefix}_{n}`, the first `n`
//!   not taken for that prefix), so opening the same file twice gives the same ids.
//! - Items added later get an id that is never reused (`{prefix}_{time}{counter}` in base 36),
//!   so an id deleted here never comes back as another item in a linked copy.
//! - An edit keeps the id: an unchanged text keeps its own, and a changed text takes the id of
//!   the text it replaced ([`carry`]).
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::model::Project;

pub const JOB: &str = "job";
pub const BULLET: &str = "b";
pub const EDUCATION: &str = "edu";
pub const SKILL: &str = "skill";
pub const LANGUAGE: &str = "lang";
pub const CERTIFICATION: &str = "cert";
pub const INTEREST: &str = "interest";
pub const SECTION: &str = "section";
pub const ITEM: &str = "item";
pub const CONTACT: &str = "contact";

/// What an id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Job,
    Bullet,
    Education,
    Skill,
    Language,
    Certification,
    Interest,
    Section,
    Item,
    Contact,
}

/// One item of the CV, found by its id: what it is, the id of the job or entry it belongs to (for
/// a bullet), and its text in one line.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ItemRef {
    pub id: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub text: String,
    pub hidden: bool,
}

/// How the ids of texts that changed are carried over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pairing {
    /// Changed texts take the ids of the texts they replaced, in order, as far as they go:
    /// right for lines typed in a box, where one line is edited at a time.
    Greedy,
    /// Only when as many texts changed as were replaced (an edit in place, or a translation);
    /// otherwise the changed ones are new: right for lists rewritten as a whole.
    SameCount,
}

/// The ids for `new` texts, given the `old` texts and their ids: an unchanged text keeps its id
/// (also when it moved), and changed texts take the ids of the replaced ones as `pairing` says.
/// `None`: a new item, which needs a new id.
pub fn carry(old: &[String], old_ids: &[String], new: &[String], pairing: Pairing) -> Vec<Option<String>> {
    let key = |s: &str| s.trim().to_string();
    let mut used = vec![false; old.len()];
    let mut out: Vec<Option<String>> = vec![None; new.len()];
    for (n, text) in new.iter().enumerate() {
        let k = key(text);
        if let Some(o) = (0..old.len()).find(|&o| !used[o] && key(&old[o]) == k && old_ids.get(o).is_some_and(|id| !id.is_empty())) {
            used[o] = true;
            out[n] = old_ids.get(o).cloned();
        }
    }
    let free_old: Vec<usize> = (0..old.len()).filter(|&o| !used[o] && old_ids.get(o).is_some_and(|id| !id.is_empty())).collect();
    let free_new: Vec<usize> = (0..new.len()).filter(|&n| out[n].is_none()).collect();
    if pairing == Pairing::Greedy || free_old.len() == free_new.len() {
        for (n, o) in free_new.into_iter().zip(free_old) {
            out[n] = old_ids.get(o).cloned();
        }
    }
    out
}

/// An id that has not been given out before (by this program, at this time): for items added now.
pub fn fresh(prefix: &str) -> String {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    format!("{prefix}_{}{}", base36(ms), base36(u64::from(SEQ.fetch_add(1, Ordering::Relaxed))))
}

fn base36(mut n: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Gives out new ids that are not in `taken`.
struct Ids {
    taken: HashSet<String>,
    next: HashMap<&'static str, u32>,
    deterministic: bool,
}

impl Ids {
    fn new_id(&mut self, prefix: &'static str) -> String {
        loop {
            let id = if self.deterministic {
                let n = self.next.entry(prefix).or_insert(1);
                let id = format!("{prefix}_{n}");
                *n += 1;
                id
            } else {
                fresh(prefix)
            };
            if self.taken.insert(id.clone()) {
                return id;
            }
        }
    }
}

/// Fills an empty id, and (when `rename`) one already used earlier in the walk.
fn fix(id: &mut String, prefix: &'static str, rename: bool, seen: &mut HashSet<String>, ids: &mut Ids) {
    if id.is_empty() || (rename && seen.contains(id.as_str())) {
        *id = ids.new_id(prefix);
    }
    seen.insert(id.clone());
}

fn fit(bullet_ids: &mut Vec<String>, len: usize) {
    bullet_ids.truncate(len);
    bullet_ids.resize(len, String::new());
}

impl Project {
    /// Every item has an id, unique in the project, and every bullet one too; `options.hidden`
    /// only names items that exist. Ids that are already fine never change. Call after any edit.
    pub fn normalize_ids(&mut self) {
        self.assign_ids(false);
    }

    /// As [`Project::normalize_ids`], with the deterministic ids used when an older file is read.
    pub(crate) fn assign_ids(&mut self, deterministic: bool) {
        for j in &mut self.experience {
            fit(&mut j.bullet_ids, j.bullets.len());
        }
        for s in &mut self.custom_sections {
            for i in &mut s.items {
                fit(&mut i.bullet_ids, i.bullets.len());
            }
        }
        let mut ids = Ids { taken: self.all_ids().into_iter().map(String::from).collect(), next: HashMap::new(), deterministic };
        let mut seen = HashSet::new();
        // Ids other parts of the file point to (the contact order, the chosen interests, the section
        // order and colours) are only filled in, never renamed.
        for x in &mut self.candidate.extra {
            fix(&mut x.id, CONTACT, false, &mut seen, &mut ids);
        }
        for j in &mut self.experience {
            fix(&mut j.id, JOB, true, &mut seen, &mut ids);
            for b in &mut j.bullet_ids {
                fix(b, BULLET, true, &mut seen, &mut ids);
            }
        }
        for x in &mut self.education {
            fix(&mut x.id, EDUCATION, true, &mut seen, &mut ids);
        }
        for x in &mut self.skills {
            fix(&mut x.id, SKILL, true, &mut seen, &mut ids);
        }
        for x in &mut self.languages {
            fix(&mut x.id, LANGUAGE, true, &mut seen, &mut ids);
        }
        for x in &mut self.certifications {
            fix(&mut x.id, CERTIFICATION, true, &mut seen, &mut ids);
        }
        for x in &mut self.interests {
            fix(&mut x.id, INTEREST, false, &mut seen, &mut ids);
        }
        for s in &mut self.custom_sections {
            fix(&mut s.id, SECTION, false, &mut seen, &mut ids);
            for i in &mut s.items {
                fix(&mut i.id, ITEM, true, &mut seen, &mut ids);
                for b in &mut i.bullet_ids {
                    fix(b, BULLET, true, &mut seen, &mut ids);
                }
            }
        }
        let mut kept = HashSet::new();
        self.options.hidden.retain(|id| seen.contains(id) && kept.insert(id.clone()));
    }

    /// Every id in the project, in the order of the file (duplicates included).
    pub fn all_ids(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.candidate.extra.iter().map(|x| x.id.as_str()).collect();
        for j in &self.experience {
            out.push(&j.id);
            out.extend(j.bullet_ids.iter().map(String::as_str));
        }
        out.extend(self.education.iter().map(|x| x.id.as_str()));
        out.extend(self.skills.iter().map(|x| x.id.as_str()));
        out.extend(self.languages.iter().map(|x| x.id.as_str()));
        out.extend(self.certifications.iter().map(|x| x.id.as_str()));
        out.extend(self.interests.iter().map(|x| x.id.as_str()));
        for s in &self.custom_sections {
            out.push(&s.id);
            for i in &s.items {
                out.push(&i.id);
                out.extend(i.bullet_ids.iter().map(String::as_str));
            }
        }
        out.retain(|id| !id.is_empty());
        out
    }

    /// After a change made without regard to ids (an AI tool that rewrote a list, a translation),
    /// gives the changed items the ids they had in `before`: see [`carry`]. Then
    /// [`Project::normalize_ids`] gives the rest new ones.
    pub fn keep_ids_from(&mut self, before: &Project) {
        let bullets = |new_b: &[String], new_ids: &mut Vec<String>, old_b: &[String], old_ids: &[String]| {
            if new_b == old_b && new_ids.len() == new_b.len() {
                return;
            }
            // Ids the change did not touch (still exactly the old ones) belong to the old texts.
            if new_ids.as_slice() == old_ids || new_ids.iter().all(String::is_empty) {
                *new_ids = carry(old_b, old_ids, new_b, Pairing::Greedy).into_iter().map(Option::unwrap_or_default).collect();
            }
        };
        for j in &mut self.experience {
            if let Some(o) = before.experience.iter().find(|o| !o.id.is_empty() && o.id == j.id) {
                bullets(&j.bullets, &mut j.bullet_ids, &o.bullets, &o.bullet_ids);
            }
        }
        for s in &mut self.custom_sections {
            let Some(os) = before.custom_sections.iter().find(|o| o.id == s.id) else { continue };
            relist(&mut s.items, &os.items, |i| i.title.clone(), |i| &mut i.id, |i| &i.id);
            for i in &mut s.items {
                if let Some(o) = os.items.iter().find(|o| !o.id.is_empty() && o.id == i.id) {
                    bullets(&i.bullets, &mut i.bullet_ids, &o.bullets, &o.bullet_ids);
                }
            }
        }
        relist(&mut self.education, &before.education, |e| format!("{}\n{}", e.qualification, e.institution), |e| &mut e.id, |e| &e.id);
        relist(&mut self.skills, &before.skills, |s| s.name.to_lowercase(), |s| &mut s.id, |s| &s.id);
        relist(&mut self.languages, &before.languages, |l| l.name.to_lowercase(), |l| &mut l.id, |l| &l.id);
        relist(&mut self.certifications, &before.certifications, |c| c.name.to_lowercase(), |c| &mut c.id, |c| &c.id);
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        !id.is_empty() && self.options.hidden.iter().any(|h| h == id)
    }

    /// Shows or hides the item `id` in this CV. Returns whether anything changed.
    pub fn set_hidden(&mut self, id: &str, hidden: bool) -> bool {
        if id.is_empty() || self.is_hidden(id) == hidden {
            return false;
        }
        if hidden {
            self.options.hidden.push(id.to_string());
        } else {
            self.options.hidden.retain(|h| h != id);
        }
        true
    }

    /// How many items are hidden in this CV (a hidden job counts once, not once per bullet).
    pub fn hidden_count(&self) -> usize {
        let all: HashSet<&str> = self.all_ids().into_iter().collect();
        self.options.hidden.iter().filter(|h| all.contains(h.as_str())).count()
    }

    /// The CV as it is shown, printed and checked: without the hidden items. Borrowed when
    /// nothing is hidden. The copy has no hide list of its own.
    pub fn visible(&self) -> Cow<'_, Project> {
        if self.options.hidden.is_empty() {
            return Cow::Borrowed(self);
        }
        let hidden: HashSet<&str> = self.options.hidden.iter().map(String::as_str).collect();
        let shown = |id: &String| !hidden.contains(id.as_str());
        let mut p = self.clone();
        let keep_bullets = |bullets: &mut Vec<String>, ids: &mut Vec<String>| {
            let mut i = 0;
            bullets.retain(|_| {
                let keep = ids.get(i).is_none_or(shown);
                i += 1;
                keep
            });
            ids.retain(|id| shown(id));
        };
        p.experience.retain(|j| shown(&j.id));
        for j in &mut p.experience {
            keep_bullets(&mut j.bullets, &mut j.bullet_ids);
        }
        p.education.retain(|x| shown(&x.id));
        p.skills.retain(|x| shown(&x.id));
        p.languages.retain(|x| shown(&x.id));
        p.certifications.retain(|x| shown(&x.id));
        p.custom_sections.retain(|s| shown(&s.id));
        for s in &mut p.custom_sections {
            s.items.retain(|x| shown(&x.id));
            for i in &mut s.items {
                keep_bullets(&mut i.bullets, &mut i.bullet_ids);
            }
        }
        p.options.hidden.clear();
        Cow::Owned(p)
    }

    /// Every item that has an id, in the order of the file: for tools and features that refer to
    /// items by id (hiding, reviewing lines, keeping language versions in step).
    pub fn items(&self) -> Vec<ItemRef> {
        let mut out = Vec::new();
        let mut push = |id: &str, kind: Kind, parent: Option<&str>, text: String| {
            if !id.is_empty() {
                out.push(ItemRef { id: id.to_string(), kind, parent: parent.map(String::from), text, hidden: self.is_hidden(id) });
            }
        };
        let join = |parts: &[&str]| parts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        for j in &self.experience {
            push(&j.id, Kind::Job, None, join(&[&j.title, &j.company]));
            for (id, b) in j.bullet_ids.iter().zip(&j.bullets) {
                push(id, Kind::Bullet, Some(&j.id), b.clone());
            }
        }
        for x in &self.education {
            push(&x.id, Kind::Education, None, join(&[&x.qualification, &x.institution]));
        }
        for x in &self.skills {
            push(&x.id, Kind::Skill, None, join(&[&x.group, &x.name]));
        }
        for x in &self.languages {
            push(&x.id, Kind::Language, None, join(&[&x.name, &x.level]));
        }
        for x in &self.certifications {
            push(&x.id, Kind::Certification, None, join(&[&x.name, &x.issuer]));
        }
        for x in &self.interests {
            push(&x.id, Kind::Interest, None, x.label.clone());
        }
        for s in &self.custom_sections {
            push(&s.id, Kind::Section, None, s.title.clone());
            for i in &s.items {
                push(&i.id, Kind::Item, Some(&s.id), join(&[&i.title, &i.subtitle]));
                for (id, b) in i.bullet_ids.iter().zip(&i.bullets) {
                    push(id, Kind::Bullet, Some(&i.id), b.clone());
                }
            }
        }
        for x in &self.candidate.extra {
            push(&x.id, Kind::Contact, None, join(&[&x.label, &x.value]));
        }
        out
    }
}

/// Gives items of a rewritten list that have no id the id of the item they replaced in `old`.
fn relist<T>(new: &mut [T], old: &[T], key: impl Fn(&T) -> String, id_mut: impl Fn(&mut T) -> &mut String, id: impl Fn(&T) -> &String) {
    let present: HashSet<String> = new.iter().map(|x| id(x).clone()).filter(|i| !i.is_empty()).collect();
    let (old_keys, old_ids): (Vec<String>, Vec<String>) = old.iter().filter(|o| !id(o).is_empty() && !present.contains(id(o))).map(|o| (key(o), id(o).clone())).unzip();
    let open: Vec<usize> = (0..new.len()).filter(|&n| id(&new[n]).is_empty()).collect();
    let new_keys: Vec<String> = open.iter().map(|&n| key(&new[n])).collect();
    for (n, carried) in open.into_iter().zip(carry(&old_keys, &old_ids, &new_keys, Pairing::SameCount)) {
        if let Some(c) = carried {
            *id_mut(&mut new[n]) = c;
        }
    }
}
