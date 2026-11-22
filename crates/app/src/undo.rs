//! The big undo: a few named steps back through the big changes (a new CV, an import, filling in
//! from the ad, a deleted job, the AI's changes …), and forward again with «Gjør om». Typing in a
//! field is not a step; egui's text fields have their own undo for that.

/// How many big changes «Angre» can go back. Each keeps a whole CV in memory (with its photo,
/// which CVBuilder keeps small), so a handful costs little.
pub const STEPS: usize = 5;

/// The names of the big changes, as the Undo and Redo buttons show them («Angre: importer CV»):
/// Norwegian keys, translated when they are shown.
pub mod step {
    pub const NEW: &str = "ny CV";
    pub const EXAMPLE: &str = "vis eksemplet";
    pub const OPEN: &str = "åpne utkast";
    pub const IMPORT: &str = "importer CV";
    pub const AD: &str = "fyll ut fra annonsen";
    pub const LANGUAGE: &str = "bytt CV-språk";
    pub const DELETE_JOB: &str = "slett jobb";
    pub const DELETE_SECTION: &str = "slett seksjon";
    /// What the AI in the chat changed in one answer.
    pub const AI: &str = "AI-endringer";
    /// What another AI app connected over MCP changed (one step for a run of changes).
    pub const AI_APP: &str = "endringer fra AI-appen";
    /// «Angre disse endringene» in the AI panel, which can itself be undone.
    pub const AI_UNDO: &str = "angre i AI-panelet";
    /// A spelling suggestion put in everywhere (the spell check in the checklist).
    pub const SPELLING: &str = "rett skrivefeil";

    #[cfg(test)]
    pub const ALL: [&str; 12] = [NEW, EXAMPLE, OPEN, IMPORT, AD, LANGUAGE, DELETE_JOB, DELETE_SECTION, AI, AI_APP, AI_UNDO, SPELLING];
}

/// One big change: the state on the other side of it, and what the change is called.
struct Step<T> {
    state: T,
    name: &'static str,
}

/// The steps that can be undone and done again, newest last.
pub struct History<T> {
    undo: Vec<Step<T>>,
    redo: Vec<Step<T>>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        History { undo: Vec::new(), redo: Vec::new() }
    }
}

impl<T> History<T> {
    /// `before` is the state from before the big change called `name`. The oldest step goes
    /// when there are [`STEPS`] already; what was undone can no longer be done again.
    pub fn push(&mut self, before: T, name: &'static str) {
        if self.undo.len() >= STEPS {
            self.undo.remove(0);
        }
        self.undo.push(Step { state: before, name });
        self.redo.clear();
    }

    /// Goes back one step from `now`: the state to load and the step's name. `now` is kept for
    /// [`History::redo`] under the same name.
    pub fn undo(&mut self, now: T) -> Option<(T, &'static str)> {
        let Step { state, name } = self.undo.pop()?;
        self.redo.push(Step { state: now, name });
        Some((state, name))
    }

    /// Does the last undone step again from `now`, which can then be undone again.
    pub fn redo(&mut self, now: T) -> Option<(T, &'static str)> {
        let Step { state, name } = self.redo.pop()?;
        self.undo.push(Step { state: now, name });
        Some((state, name))
    }

    /// An edit by hand: what was undone stays undone.
    pub fn edited(&mut self) {
        self.redo.clear();
    }

    /// The name (key) of the step «Angre» would undo.
    pub fn undo_name(&self) -> Option<&'static str> {
        self.undo.last().map(|s| s.name)
    }

    /// The name (key) of the step «Gjør om» would do again.
    pub fn redo_name(&self) -> Option<&'static str> {
        self.redo.last().map(|s| s.name)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_name_the_step() {
        let mut h = History::default();
        assert_eq!((h.undo_name(), h.redo_name()), (None, None));
        h.push("example", step::NEW);
        h.push("blank", step::IMPORT);
        assert_eq!(h.undo_name(), Some(step::IMPORT));
        assert_eq!(h.undo("imported"), Some(("blank", step::IMPORT)));
        // Redo names the same step it brings back; Undo now names the one before.
        assert_eq!((h.undo_name(), h.redo_name()), (Some(step::NEW), Some(step::IMPORT)));
        assert_eq!(h.undo("blank"), Some(("example", step::NEW)));
        assert!(!h.can_undo() && h.undo("example").is_none());
        assert_eq!(h.redo("example"), Some(("blank", step::NEW)));
        assert_eq!(h.redo("blank"), Some(("imported", step::IMPORT)));
        assert!(!h.can_redo() && h.redo("imported").is_none());
        assert_eq!(h.undo_name(), Some(step::IMPORT));
    }

    #[test]
    fn a_new_step_or_an_edit_ends_redo() {
        let mut h = History::default();
        h.push(1, step::DELETE_JOB);
        h.undo(2);
        assert!(h.can_redo());
        h.edited();
        assert!(!h.can_redo() && !h.can_undo());
        h.push(1, step::DELETE_JOB);
        h.undo(2);
        h.push(3, step::AD);
        assert!(!h.can_redo());
        assert_eq!(h.undo_name(), Some(step::AD));
    }

    #[test]
    fn keeps_the_last_five_steps() {
        let mut h = History::default();
        let names = [step::NEW, step::IMPORT, step::AD, step::LANGUAGE, step::DELETE_JOB, step::DELETE_SECTION, step::AI];
        for (i, &n) in names.iter().enumerate() {
            h.push(i, n);
        }
        let mut back = Vec::new();
        while let Some((state, name)) = h.undo(99) {
            back.push((state, name));
        }
        assert_eq!(back.len(), STEPS);
        assert_eq!(back.first(), Some(&(6, step::AI)));
        // The two oldest are gone; the oldest left is the third change.
        assert_eq!(back.last(), Some(&(2, step::AD)));
    }

    #[test]
    fn every_step_name_has_english() {
        let missing: Vec<_> = step::ALL.iter().filter(|n| !crate::i18n::has_english(n)).collect();
        assert!(missing.is_empty(), "step names without English: {missing:?}");
        for key in ["Angre: {step}", "Gjør om: {step}", "Angret: {step}.", "Gjort om: {step}."] {
            assert!(crate::i18n::has_english(key), "{key}");
        }
    }
}
