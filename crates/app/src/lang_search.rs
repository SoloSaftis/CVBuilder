//! Type to find a language in the long language lists (the menu language and the CV language,
//! 86 entries each). A search field sits at the top of the opened list and has the keyboard as
//! soon as the list opens; what is typed matches a language's name in the menu language
//! («Tysk»), its own name («Deutsch»), its English name («German») and its code («de»), with or
//! without accents. The names shown stay in the menu language; the own name is in the hover text.
//!
//! Keys: type to filter, ↓ and ↑ move the marked line, Enter picks it (with one match, or after
//! typing, the first line is marked), Escape closes the list.

use egui::{Id, Key, Margin, Modifiers};

use crate::{i18n, theme};

/// One language in a list.
pub(crate) struct LangItem<K> {
    /// What picking it gives back to the caller.
    pub key: K,
    /// Its name in the menu language, as shown.
    pub shown: String,
    /// Its own name («Deutsch»).
    pub own: String,
    /// Its code («de», «pt-BR»); empty when there is none.
    pub code: String,
    /// The hover text.
    pub tip: String,
}

impl<K> LangItem<K> {
    /// A language in a list. The English name of the code is looked up for the search.
    pub(crate) fn new(key: K, shown: String, own: impl Into<String>, code: impl Into<String>, tip: impl Into<String>) -> Self {
        LangItem { key, shown, own: own.into(), code: code.into(), tip: tip.into() }
    }

    /// How well it fits what is typed (lower is better), or `None` when it does not.
    fn rank(&self, query: &str) -> Option<u8> {
        let english = i18n::english_language_name(&self.code).unwrap_or_default();
        rank(query, &[&self.shown, &self.own, &english], &self.code)
    }
}

/// Lower case without accents, for comparing («Čeština» → «cestina»).
fn fold(s: &str) -> String {
    theme::sort_key(s.trim())
}

/// How well a language fits what is typed, by its names and its code (lower is better):
/// 0 when its code is what was typed or a name starts with it, 1 when a word in a name starts with
/// it, 2 when it is somewhere inside a name. `None` when it fits nowhere. Empty fits everything.
pub(crate) fn rank(query: &str, names: &[&str], code: &str) -> Option<u8> {
    let q = fold(query);
    if q.is_empty() {
        return Some(0);
    }
    let code = code.to_lowercase();
    let code_fits = !code.is_empty() && (code == q || code.split('-').next() == Some(q.as_str()) || (q.contains('-') && code.starts_with(&q)));
    let names: Vec<String> = names.iter().map(|n| fold(n)).filter(|n| !n.is_empty()).collect();
    if code_fits || names.iter().any(|n| n.starts_with(&q)) {
        return Some(0);
    }
    let word_start = |n: &String| n.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&q));
    if names.iter().any(word_start) {
        return Some(1);
    }
    names.iter().any(|n| n.contains(&q)).then_some(2)
}

/// The places in `items` that fit `query`, best first and otherwise in list order.
pub(crate) fn filter<K>(items: &[LangItem<K>], query: &str) -> Vec<usize> {
    let mut found: Vec<(u8, usize)> = items.iter().enumerate().filter_map(|(i, it)| it.rank(query).map(|r| (r, i))).collect();
    found.sort_by_key(|(r, i)| (*r, *i));
    found.into_iter().map(|(_, i)| i).collect()
}

/// What the search keeps while its list is open.
#[derive(Clone, Default)]
struct State {
    query: String,
    /// The marked line, by its place among the matches.
    marked: Option<usize>,
    /// The pass it was last drawn in: a gap means the list was closed and has opened again.
    pass: u64,
}

/// The search field and the list under it, inside an open dropdown. `pinned` lines first are set
/// apart by a line (the computer's own language) while nothing is typed. Gives back the key of
/// the language picked, and closes the list then.
pub(crate) fn list<K: Clone>(ui: &mut egui::Ui, id_salt: &str, items: &[LangItem<K>], selected: impl Fn(&K) -> bool, pinned: usize) -> Option<K> {
    let id = Id::unique(("lang-search", id_salt));
    let pass = ui.ctx().cumulative_pass_nr();
    let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let opened = pass > state.pass + 1;
    if opened {
        state = State::default();
    }
    state.pass = pass;

    let field_id = id.with("field");
    let has_focus = ui.memory(|m| m.has_focus(field_id));
    let matches = filter(items, &state.query);
    // The arrows move the marked line, not the cursor in the field.
    let (down, up) = if has_focus || opened {
        ui.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::ArrowUp)))
    } else {
        (false, false)
    };
    if down {
        state.marked = Some(state.marked.map_or(0, |m| (m + 1).min(matches.len().saturating_sub(1))));
    }
    if up {
        state.marked = state.marked.map(|m| m.saturating_sub(1));
    }
    let moved = down || up;

    let field = ui.add(
        egui::TextEdit::singleline(&mut state.query)
            .id(field_id)
            .hint_text(i18n::t("Søk etter språk …"))
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(8, 5)),
    );
    let field = field.on_hover_text(i18n::t("Skriv navnet på språket, på ditt språk eller på språket selv, eller koden (for eksempel de). Pil ned og Enter velger."));
    if opened {
        field.request_focus();
    }
    let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let searching = !state.query.trim().is_empty();
    let matches = if field.changed() {
        // Typing marks the best match, so Enter picks it.
        let m = filter(items, &state.query);
        state.marked = (!m.is_empty() && searching).then_some(0);
        m
    } else {
        matches
    };
    if let Some(m) = state.marked {
        if m >= matches.len() {
            state.marked = matches.len().checked_sub(1);
        }
    }

    let mut picked: Option<K> = None;
    if enter {
        if let Some(i) = state.marked.or((matches.len() == 1).then_some(0)).and_then(|m| matches.get(m)) {
            picked = Some(items[*i].key.clone());
        }
    }

    // The list scrolls under the field, so the field stays in sight.
    let max = (ui.ctx().content_rect().height() - 160.0).clamp(160.0, 560.0);
    egui::ScrollArea::vertical().id_salt(id.with("scroll")).max_height(max).auto_shrink([false, true]).show(ui, |ui| {
        if matches.is_empty() {
            ui.label(egui::RichText::new(i18n::t("Fant ikke språket")).color(theme::Tokens::get(ui.ctx()).faint));
        }
        for (place, &i) in matches.iter().enumerate() {
            if !searching && pinned > 0 && place == pinned {
                ui.separator();
            }
            let it = &items[i];
            let mut r = ui.selectable_label(selected(&it.key), it.shown.as_str()).on_hover_text(it.tip.as_str());
            if state.marked == Some(place) {
                r = r.highlight();
                if moved {
                    r.scroll_to_me(None);
                }
            }
            if r.clicked() {
                picked = Some(it.key.clone());
            }
        }
    });

    if picked.is_some() {
        ui.close();
        state = State::default();
    } else if enter {
        // Enter with nothing to pick keeps the field ready for more typing.
        field.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(id, state));
    picked
}

/// The CV languages as list items: the computer's own first (when there is one), then A to Z.
/// Gives the items and how many are pinned to the top.
pub(crate) fn cv_languages(tip: impl Fn(&str) -> String) -> (Vec<LangItem<String>>, usize) {
    let (own, rest) = crate::translate_ui::cv_language_list();
    let pinned = usize::from(own.is_some());
    let items = own
        .into_iter()
        .chain(rest)
        .map(|name| {
            let code = cv_core::headings::code_of(name).unwrap_or_default();
            let shown = crate::translate_ui::shown_name(name);
            let tip = if fold(&shown) == fold(name) { tip(name) } else { format!("{name} · {}", tip(name)) };
            LangItem::new(name.to_string(), shown, name, code, tip)
        })
        .collect();
    (items, pinned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(shown: &str, own: &str, code: &str) -> LangItem<String> {
        LangItem::new(own.to_string(), shown.to_string(), own, code, "")
    }

    #[test]
    fn matches_every_name_and_the_code() {
        let names = ["Tysk", "Deutsch", "German"];
        assert_eq!(rank("tys", &names, "de"), Some(0), "the name in the menu language");
        assert_eq!(rank("DEUT", &names, "de"), Some(0), "the own name, any case");
        assert_eq!(rank("germ", &names, "de"), Some(0), "the English name");
        assert_eq!(rank("de", &names, "de"), Some(0), "the code");
        assert_eq!(rank("utsc", &names, "de"), Some(2), "inside a name");
        assert_eq!(rank("fransk", &names, "de"), None);
        assert_eq!(rank("", &names, "de"), Some(0), "nothing typed shows everything");
        assert_eq!(rank("   ", &names, "de"), Some(0));
    }

    #[test]
    fn codes_with_a_region_and_accents() {
        let pt = ["Portugisisk (Brasil)", "Português (Brasil)", "Portuguese (Brazil)"];
        assert_eq!(rank("pt", &pt, "pt-BR"), Some(0));
        assert_eq!(rank("pt-br", &pt, "pt-BR"), Some(0));
        assert_eq!(rank("pt-", &pt, "pt-BR"), Some(0));
        assert_eq!(rank("portugues", &pt, "pt-BR"), Some(0), "without the accent");
        assert_eq!(rank("bras", &pt, "pt-BR"), Some(1), "a later word");
        let cs = ["Tsjekkisk", "Čeština", "Czech"];
        assert_eq!(rank("cest", &cs, "cs"), Some(0));
        assert_eq!(rank("ČEŠ", &cs, "cs"), Some(0));
        // A code is matched whole, not as a part: «s» is no code.
        assert_eq!(rank("s", &["Arabisk", "العربية", "Arabic"], "ar"), Some(2));
    }

    #[test]
    fn own_script_finds_the_language() {
        let ar = ["Arabisk", "العربية", "Arabic"];
        assert_eq!(rank("العربية", &ar, "ar"), Some(0));
        assert_eq!(rank("عرب", &ar, "ar"), Some(2));
        assert_eq!(rank("日本", &["Japansk", "日本語", "Japanese"], "ja"), Some(0));
    }

    #[test]
    fn best_matches_come_first() {
        let items = vec![item("Engelsk", "English", "en"), item("Slovensk", "Slovenčina", "sk"), item("Svensk", "Svenska", "sv"), item("Tysk", "Deutsch", "de")];
        // «en»: the code (and the start of «Engelsk») first, then the names with «en» inside.
        assert_eq!(filter(&items, "en"), vec![0, 1, 2]);
        assert_eq!(filter(&items, "sv"), vec![2]);
        assert_eq!(filter(&items, ""), vec![0, 1, 2, 3]);
        assert!(filter(&items, "xyz").is_empty());
    }

    #[test]
    fn every_cv_language_can_be_found_by_its_own_name_and_code() {
        let (items, _) = cv_languages(|_| String::new());
        assert!(items.len() >= 80, "{} CV languages", items.len());
        for (i, it) in items.iter().enumerate() {
            assert!(filter(&items, &it.own).contains(&i), "«{}» not found by its own name", it.own);
            assert!(filter(&items, &it.shown).contains(&i), "«{}» not found by its shown name", it.shown);
            if !it.code.is_empty() {
                assert!(filter(&items, &it.code).contains(&i), "«{}» not found by its code {}", it.own, it.code);
            }
        }
    }
}
