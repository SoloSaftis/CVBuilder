//! Every control in the window explains itself on hover. This test reads the app's source and
//! fails when a button, field, checkbox, menu or anything else you can click, type in or drag is
//! added without a tooltip (`on_hover_text` / `on_disabled_hover_text`), and when a helper that
//! takes a tip is given an empty one.
//!
//! How it reads the source: comments are removed and string literals emptied first, so text in
//! them never counts. For each interactive construction it takes the statement it is in (to the
//! `;`, or to the `{` of an `if`), and looks for a tooltip applied to it – not inside a closure
//! opened after it, so a menu's items do not count for the menu. When the statement binds the
//! response (`let r = …;`), the rest of the block is followed: a later `r.on_hover_text(…)`, a tip
//! helper such as `full_text_hint(r, …)`, or another statement using `r` that adds one.

/// What makes something interactive. `.name(` patterns must be method calls.
const CONSTRUCTS: &[&str] = &[
    ".button(",
    ".small_button(",
    ".checkbox(",
    ".radio(",
    ".radio_value(",
    ".selectable_label(",
    ".selectable_value(",
    ".menu_button(",
    ".color_edit_button_srgb(",
    ".color_edit_button_srgba(",
    ".hyperlink(",
    ".hyperlink_to(",
    ".link(",
    ".toggle_value(",
    "Button::new",
    "Button::image",
    "Checkbox::",
    "RadioButton::",
    "SelectableLabel::",
    "TextEdit::",
    "ComboBox::",
    "Slider::new",
    "DragValue::new",
    "Hyperlink::",
    "Sense::click",
    "Sense::drag",
    "Sense::click_and_drag",
];

/// What counts as a tooltip on the construction's response.
const TIPS: &[&str] = &[".on_hover_text(", ".on_disabled_hover_text(", ".on_hover_ui(", ".on_disabled_hover_ui("];

/// Helpers that take the response and add the tooltip themselves.
const SINKS: &[&str] = &["full_text_hint"];

/// Helpers that take a tip: none of them may be given an empty string.
const TIP_HELPERS: &[&str] = &["tool", "icon_button", "remove_button", "primary", "ghost", "ai_button", "field", "area", "lines", "date", "year_inline", "inline", "role", "band", "full_text_hint"];

/// The few places where a tooltip is added another way: (file, text on the line, why).
const ALLOWED: &[(&str, &str, &str)] = &[
    // icon_button picks its sense in a small `if`; the helper takes a tip and always ends with
    // `r.on_hover_text(tip)`, and the test checks that no call passes an empty one.
    ("theme.rs", "let sense = if enabled { egui::Sense::click() }", "icon_button takes a tip and adds it at its end"),
    // Builders that return the button: `tool` and `tool_menu` add the tip they are given, and the
    // `flat_icon` and `menu_item` calls chain `.on_hover_text` themselves.
    ("theme.rs", "egui::Button::new((icon, label, egui::RichText::new(ph::CARET_DOWN)", "tool_button: the tip is added by tool_menu"),
    ("theme.rs", "egui::Button::new((icon, label))", "tool_button: the tip is added by tool"),
    ("theme.rs", "let b = egui::Button::new(egui::RichText::new(icon).family(icons()).size(14.0)", "flat_icon: callers add the tip"),
    ("menu_ui.rs", "egui::Button::new((RichText::new(icon).family(theme::icons()).size(15.0)", "menu_item: callers add the tip"),
];

/// The source with comments blanked and string and char literals emptied (`""` stays `""`, any
/// other string becomes `"s"`), with the line breaks kept so line numbers still match.
fn strip(src: &str) -> Vec<char> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::with_capacity(c.len());
    let mut i = 0;
    let ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if ch == '/' && next == Some('*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.extend([' ', ' ']);
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.extend([' ', ' ']);
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(if c[i] == '\n' { '\n' } else { ' ' });
                    i += 1;
                }
            }
        } else if (ch == 'r' || ch == 'b') && !i.checked_sub(1).is_some_and(|p| ident(c[p])) && raw_start(&c, i).is_some() {
            // r"…", r#"…"#, br"…"
            let (hashes, body) = raw_start(&c, i).unwrap_or((0, i));
            let mut j = body;
            let mut empty = true;
            let mut newlines = 0;
            loop {
                if j >= c.len() {
                    break;
                }
                if c[j] == '"' && (0..hashes).all(|h| c.get(j + 1 + h) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                if c[j] == '\n' {
                    newlines += 1;
                }
                empty = false;
                j += 1;
            }
            out.extend(if empty { "\"\"".chars() } else { "\"s\"".chars() });
            out.extend(std::iter::repeat_n('\n', newlines));
            i = j;
        } else if ch == '"' || (ch == 'b' && next == Some('"') && !i.checked_sub(1).is_some_and(|p| ident(c[p]))) {
            let mut j = if ch == 'b' { i + 2 } else { i + 1 };
            let mut empty = true;
            let mut newlines = 0;
            while j < c.len() && c[j] != '"' {
                if c[j] == '\\' {
                    j += 1;
                }
                if c.get(j) == Some(&'\n') {
                    newlines += 1;
                }
                empty = false;
                j += 1;
            }
            out.extend(if empty { "\"\"".chars() } else { "\"s\"".chars() });
            out.extend(std::iter::repeat_n('\n', newlines));
            i = j + 1;
        } else if ch == '\'' {
            // A char literal ('x', '\n', '\'', '\u{1F600}') or a lifetime ('a).
            if next == Some('\\') {
                let mut j = i + 2;
                while j < c.len() && c[j] != '\'' {
                    j += 1;
                }
                out.extend(['\'', 'c', '\'']);
                i = j + 1;
            } else if c.get(i + 2) == Some(&'\'') {
                out.extend(['\'', 'c', '\'']);
                i += 3;
            } else {
                out.push(ch);
                i += 1;
            }
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// At `r"`, `r#"`, `br"`: the number of `#` and where the body starts.
fn raw_start(c: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if c.get(j) == Some(&'b') {
        j += 1;
    }
    if c.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while c.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    (c.get(j) == Some(&'"')).then_some((hashes, j + 1))
}

fn is_ident(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn starts(code: &[char], at: usize, pat: &str) -> bool {
    pat.chars().enumerate().all(|(k, p)| code.get(at + k) == Some(&p))
}

/// Where `pat` occurs in the code, as a whole word at its start when it begins with a letter.
fn find_all(code: &[char], pat: &str) -> Vec<usize> {
    let first = pat.chars().next().unwrap_or(' ');
    (0..code.len()).filter(|&i| starts(code, i, pat) && !(is_ident(first) && i > 0 && is_ident(code[i - 1]))).collect()
}

/// The statement around `at`: back to the `;`, `{` or `}` before it, forward to its `;`, to the
/// `{` of an `if`/`match`, or to the `}` or `)` that closes the block or call it is in.
fn statement(code: &[char], at: usize) -> (usize, usize) {
    let mut depth = 0i32;
    let mut start = 0;
    let mut i = at;
    while i > 0 {
        i -= 1;
        // An open `(` or `[` is a call or list the construction is inside: the statement
        // goes on before it (`let r = ui.add(…)`). An open `{` is the block it is in.
        match code[i] {
            ')' | ']' => depth += 1,
            '(' | '[' => depth -= 1,
            '{' | ';' if depth <= 0 => {
                start = i + 1;
                break;
            }
            // A `}` before an open `(` ends the statement before (an `if` block just above
            // `let r = ui.add(…)`), like `;` does.
            '}' if depth <= 0 => {
                start = i + 1;
                break;
            }
            '{' => depth -= 1,
            '}' => depth += 1,
            _ => {}
        }
    }
    let mut depth = 0i32;
    let mut end = code.len();
    let mut i = at;
    while i < code.len() {
        match code[i] {
            '(' | '[' => depth += 1,
            '{' if depth <= 0 => {
                end = i;
                break;
            }
            '{' => depth += 1,
            ';' if depth <= 0 => {
                end = i;
                break;
            }
            '}' if depth <= 0 => {
                end = i;
                break;
            }
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    (start, end)
}

/// Whether a tooltip is put on what starts at `at`: in its own method chain (not inside a
/// closure or argument list opened after it), or through a sink helper around it.
fn tipped_here(code: &[char], at: usize, start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for i in at..end {
        match code[i] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if depth <= 0 && TIPS.iter().any(|t| starts(code, i, t)) {
            return true;
        }
    }
    let text: String = code[start..end].iter().collect();
    SINKS.iter().any(|s| text.contains(&format!("{s}(")))
}

/// The names bound by a `let` statement (`let r`, `let mut r`, `let (rect, r)`).
fn bound_names(code: &[char], start: usize, end: usize) -> Vec<String> {
    let text: String = code[start..end].iter().collect();
    let text = text.trim_start();
    let Some(rest) = text.strip_prefix("let ") else {
        return Vec::new();
    };
    let Some(eq) = rest.find('=') else {
        return Vec::new();
    };
    let pattern = &rest[..eq];
    let pattern = pattern.split(':').next().unwrap_or("");
    pattern.split(|ch: char| !is_ident(ch)).filter(|w| !w.is_empty() && *w != "mut" && *w != "_" && !w.starts_with(|ch: char| ch.is_uppercase())).map(String::from).collect()
}

/// The end of the block that `from` is in.
fn block_end(code: &[char], from: usize) -> usize {
    let mut depth = 0i32;
    for (i, ch) in code.iter().enumerate().skip(from) {
        match ch {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    code.len()
}

/// Whether what starts at `at` ends up with a tooltip. `hops` limits how far bindings are followed.
fn tipped(code: &[char], at: usize, hops: u8) -> bool {
    let (start, end) = statement(code, at);
    if tipped_here(code, at, start, end) {
        return true;
    }
    if hops == 0 {
        return false;
    }
    let names = bound_names(code, start, end);
    if names.is_empty() {
        return false;
    }
    let stop = block_end(code, end);
    for name in names {
        for use_at in find_all(&code[..stop], &name).into_iter().filter(|&u| u > end) {
            let after = code.get(use_at + name.len()).copied().unwrap_or(' ');
            let before = use_at.checked_sub(1).map(|p| code[p]).unwrap_or(' ');
            if is_ident(after) || before == '.' {
                continue;
            }
            if tipped(code, use_at, hops - 1) {
                return true;
            }
        }
    }
    false
}

fn line_of(code: &[char], at: usize) -> usize {
    code[..at].iter().filter(|&&c| c == '\n').count() + 1
}

/// Each construction in `src` that gets no tooltip, as «file:line: the line».
fn missing(file: &str, src: &str) -> Vec<String> {
    let code = strip(src);
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pat in CONSTRUCTS {
        for at in find_all(&code, pat) {
            let line = line_of(&code, at);
            let text = lines.get(line - 1).copied().unwrap_or("").trim();
            if ALLOWED.iter().any(|(f, snippet, _)| *f == file && text.contains(snippet)) {
                continue;
            }
            // `ComboBox::from_id_salt(…)` matches once; a second pattern on the same spot does not count twice.
            if !tipped(&code, at, 4) && seen.insert((line, at)) {
                out.push(format!("{file}:{line}: {text}"));
            }
        }
    }
    out
}

/// Each call to a tip helper in `src` that passes an empty string.
fn empty_tips(file: &str, src: &str) -> Vec<String> {
    let code = strip(src);
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for helper in TIP_HELPERS {
        let call = format!("{helper}(");
        for at in find_all(&code, &call) {
            let before: String = code[..at].iter().rev().take(4).collect::<String>().chars().rev().collect();
            if before.ends_with('.') || before.ends_with("fn ") {
                continue;
            }
            // The arguments, split at the top-level commas.
            let mut depth = 0i32;
            let mut arg = String::new();
            let mut args = Vec::new();
            for &ch in &code[at + call.len()..] {
                match ch {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' if depth == 0 => break,
                    ')' | ']' | '}' => depth -= 1,
                    ',' if depth == 0 => {
                        args.push(std::mem::take(&mut arg));
                        continue;
                    }
                    _ => {}
                }
                arg.push(ch);
            }
            args.push(arg);
            if args.iter().any(|a| a.trim() == "\"\"") {
                let line = line_of(&code, at);
                out.push(format!("{file}:{line}: {}", lines.get(line - 1).copied().unwrap_or("").trim()));
            }
        }
    }
    out
}

fn sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("src") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name.ends_with(".rs") && name != "tooltips.rs" {
            out.push((name, std::fs::read_to_string(&path).expect("read")));
        }
    }
    out.sort();
    out
}

#[test]
fn every_control_has_a_tooltip() {
    let files = sources();
    assert!(files.len() > 10, "found only {} source files", files.len());
    let gaps: Vec<String> = files.iter().flat_map(|(f, s)| missing(f, s)).collect();
    assert!(gaps.is_empty(), "controls without a tooltip (add .on_hover_text(i18n::t(\"…\")), or pass a tip to the helper):\n{}", gaps.join("\n"));
}

#[test]
fn no_helper_gets_an_empty_tip() {
    let empty: Vec<String> = sources().iter().flat_map(|(f, s)| empty_tips(f, s)).collect();
    assert!(empty.is_empty(), "tip helpers given an empty tip:\n{}", empty.join("\n"));
}

#[test]
fn the_allow_list_is_used() {
    let files = sources();
    for (file, snippet, why) in ALLOWED {
        assert!(!why.is_empty());
        assert!(files.iter().any(|(f, s)| f == file && s.contains(snippet)), "allow-list entry not found in {file}: {snippet}");
    }
}

#[test]
fn the_scan_finds_what_it_should() {
    let found = |src: &str| missing("x.rs", src).len();
    // Missing tooltips.
    assert_eq!(found("fn f() { if ui.button(\"x\").clicked() { go(); } }"), 1);
    assert_eq!(found("fn f() { ui.add(egui::TextEdit::singleline(&mut s)); }"), 1);
    assert_eq!(found("fn f() { let r = ui.add(egui::Button::new(\"x\")); if r.clicked() {} }"), 1);
    assert_eq!(found("fn f() { ui.menu_button(\"m\", |ui| { ui.button(\"a\").on_hover_text(\"t\"); }); }"), 1, "an item's tooltip is not the menu's");
    assert_eq!(found("fn f() { egui::ComboBox::from_id_salt(\"c\").show_ui(ui, |ui| { ui.selectable_value(&mut a, 1, \"x\").on_hover_text(\"t\"); }); }"), 1);
    assert_eq!(found("fn f() { let (rect, r) = ui.allocate_exact_size(v, egui::Sense::click()); if r.clicked() {} }"), 1);
    // With tooltips.
    assert_eq!(found("fn f() { if ui.button(\"x\").on_hover_text(\"t\").clicked() {} }"), 0);
    assert_eq!(found("fn f() { ui.add_enabled(on, egui::Button::new(\"x\")).on_disabled_hover_text(\"t\"); }"), 0);
    assert_eq!(found("fn f() { let r = ui.add(egui::TextEdit::singleline(&mut s)); full_text_hint(r, \"t\", &s); }"), 0);
    assert_eq!(found("fn f() { let b = egui::Button::new(\"x\"); let r = ui.add(b); if r.on_hover_text(\"t\").clicked() {} }"), 0);
    assert_eq!(found("fn f() { ui.menu_button(\"m\", |ui| {}).response.on_hover_text(\"t\"); }"), 0);
    assert_eq!(found("fn f() { egui::ComboBox::from_id_salt(\"c\").show_ui(ui, |ui| {}).response.on_hover_text(\"t\"); }"), 0);
    // Comments and strings do not count, either way.
    assert_eq!(found("fn f() { // ui.button(\"x\")\n /* ui.button( */ let s = \"ui.button(\"; }"), 0);
    assert_eq!(found("fn f() { if ui.button(\"x\").clicked() {} // .on_hover_text(\n let s = \".on_hover_text(\"; }"), 1);
    assert_eq!(found("fn f<'a>(c: char) { let q = '\"'; if ui.button(r#\"a \"b\"\"#).clicked() {} }"), 1);
    // Empty tips.
    assert_eq!(empty_tips("x.rs", "fn f() { tool(ui, \"a\", \"\", true); theme::primary(ui, \"a\", \"t\", true); }").len(), 1);
    assert_eq!(empty_tips("x.rs", "fn tool(ui: &mut Ui) {} fn f() { s.lines(\"\"); }").len(), 0);
}
