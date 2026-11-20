//! Text of PDFs in right-to-left and Indic scripts, read the way PDF viewers read it (129).
//!
//! `pdf-extract` reads each glyph's ToUnicode entry in content order. That reads Latin text well,
//! but right-to-left text comes out in visual order (each line backwards, «ةراس دمحلأا»), and
//! Indic clusters, which PDF writers spell with ActualText spans, come out in pieces and out of
//! place. Here the page is interpreted with hayro: a marked-content sequence's ActualText goes
//! with its first glyph (as PDFium and poppler read it), lines are the runs of glyphs on one
//! baseline, words are put left to right, and a right-to-left line is turned into reading order.
use hayro_interpret::font::Glyph;
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask, interpret_page,
};
use hayro_syntax::Pdf;

/// One glyph: its text (none for a glyph inside an ActualText span after the first), baseline y
/// and the left and right edges of its outline (none for a space).
struct Drawn {
    text: Option<String>,
    y: f64,
    edges: Option<(f64, f64)>,
}

#[derive(Default)]
struct Collect {
    glyphs: Vec<Drawn>,
    actual: Vec<Option<String>>,
    begun: usize,
    open: Vec<Option<String>>,
}

impl Device<'_> for Collect {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &kurbo::BezPath, _: kurbo::Affine, _: &Paint<'_>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn begin_marked_content(&mut self, _: &[u8], _: Option<i32>) {
        self.open.push(self.actual.get(self.begun).cloned().flatten());
        self.begun += 1;
    }
    fn end_marked_content(&mut self) {
        self.open.pop();
    }
    fn draw_glyph(&mut self, g: &Glyph<'_>, t: kurbo::Affine, gt: kurbo::Affine, _: &Paint<'_>, mode: &GlyphDrawMode) {
        if matches!(mode, GlyphDrawMode::Stroke(_)) || self.glyphs.len() > 200_000 {
            return;
        }
        let text = if let Some(actual) = self.open.iter_mut().find_map(Option::as_mut) {
            Some(std::mem::take(actual)).filter(|s| !s.is_empty())
        } else {
            match g.as_unicode() {
                Some(hayro_cmap::BfString::Char(c)) => Some(c.to_string()),
                Some(hayro_cmap::BfString::String(s)) => Some(s),
                None => None,
            }
        };
        let edges = match g {
            Glyph::Outline(o) if !o.outline().elements().is_empty() => {
                use kurbo::Shape;
                let b = (t * gt * o.outline()).bounding_box();
                Some((b.x0, b.x1))
            }
            _ => None,
        };
        self.glyphs.push(Drawn { text, y: (t * gt * kurbo::Point::ZERO).y, edges });
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// The page's ActualText per marked-content sequence, in the order they begin.
fn actual_texts(page: &hayro_syntax::page::Page<'_>) -> Vec<Option<String>> {
    use hayro_syntax::content::ops::TypedInstruction;
    use hayro_syntax::object::{Object, String as PdfString};
    let mut out = Vec::new();
    let mut ops = page.typed_operations();
    while let Some(op) = ops.next() {
        match op {
            TypedInstruction::BeginMarkedContent(_) => out.push(None),
            TypedInstruction::BeginMarkedContentWithProperties(bdc) => out.push(match bdc.1 {
                Object::Dict(d) => d.get::<PdfString<'_>>("ActualText").map(|s| text_string(s.as_bytes())),
                _ => None,
            }),
            _ => {}
        }
    }
    out
}

/// A PDF text string: UTF-16BE or UTF-8 after a byte order mark, else (near enough) PDFDocEncoding.
fn text_string(b: &[u8]) -> String {
    if let Some(utf16) = b.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = utf16.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else if let Some(utf8) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(utf8).into_owned()
    } else {
        b.iter().map(|&c| char::from(c)).collect()
    }
}

pub(crate) fn is_rtl(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Text in a script `pdf-extract` reads badly: right to left, or an Indic script.
pub(crate) fn needs_layout_reading(text: &str) -> bool {
    text.chars().any(|c| is_rtl(c) || matches!(c, '\u{0900}'..='\u{0DFF}' | '\u{1000}'..='\u{109F}' | '\u{1780}'..='\u{17FF}'))
}

/// A Hebrew or Arabic vowel mark: sits anywhere over or under its letter.
fn is_mark(text: &str) -> bool {
    text.chars().all(|c| matches!(c, '\u{0591}'..='\u{05C7}' | '\u{064B}'..='\u{065F}' | '\u{0670}'))
}

struct Word {
    left: f64,
    text: String,
}

/// A line's words, left to right, in reading order: a line that is mostly right to left reads
/// from the right, with runs of left-to-right words (an e-mail, a phone number) kept in order.
fn line_text(mut words: Vec<Word>) -> String {
    words.sort_by(|a, b| a.left.total_cmp(&b.left));
    let rtl = |w: &Word| w.text.chars().any(is_rtl);
    let ltr = |w: &Word| w.text.chars().any(|c| c.is_alphanumeric() && !is_rtl(c));
    // The line's direction from its letters (digits go either way).
    let latin = |w: &Word| w.text.chars().any(|c| c.is_alphabetic() && !is_rtl(c));
    if words.iter().filter(|w| rtl(w)).count() <= words.iter().filter(|w| latin(w) && !rtl(w)).count() {
        return join(words.into_iter().map(|w| w.text));
    }
    // Runs: right-to-left words, and everything else between them.
    let mut runs: Vec<(bool, Vec<String>)> = Vec::new();
    for w in words {
        let r = rtl(&w) || !ltr(&w);
        match runs.last_mut() {
            Some((kind, run)) if *kind == r => run.push(w.text),
            _ => runs.push((r, vec![w.text])),
        }
    }
    runs.reverse();
    join(runs.into_iter().flat_map(|(r, mut run)| {
        if r {
            run.reverse();
        }
        run
    }))
}

/// Words joined with spaces, but punctuation drawn apart sticks to the word before it
/// («الدفع.», «40٪.», «पुणे, भारत»), as it was written.
fn join(words: impl Iterator<Item = String>) -> String {
    let mut out = String::new();
    for w in words {
        let sticks = w.chars().all(|c| matches!(c, '.' | ',' | '،' | ':' | ';' | '!' | '?' | '؟' | '٪' | '%' | ')' | '।'));
        if !out.is_empty() && !sticks {
            out.push(' ');
        }
        out.push_str(&w);
    }
    out
}

/// The PDF's text, line by line, or `None` when it cannot be read this way.
pub(crate) fn read(bytes: &[u8]) -> Option<String> {
    let pdf = Pdf::new(bytes.to_vec()).ok()?;
    let cache = InterpreterCache::new();
    let mut out = String::new();
    for page in pdf.pages().iter().take(50) {
        let mut c = Collect { actual: actual_texts(page), ..Default::default() };
        let mut ctx = Context::new(kurbo::Affine::IDENTITY, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), &cache, pdf.xref(), InterpreterSettings::default());
        interpret_page(page, &mut ctx, &mut c);
        page_text(&c.glyphs, &mut out);
        out.push('\n');
    }
    // Direction marks (around a date range's dash in right-to-left text) are invisible and only
    // get in the way of reading dates.
    out.retain(|c| !matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'));
    // A running header or footer with the page count («प्रिया शर्मा 1 / 1»), drawn first on the
    // page, would be read as the CV's first line.
    let footer = |l: &str| {
        let words: Vec<&str> = l.split_whitespace().collect();
        matches!(words.as_slice(), [.., a, "/", b] if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()))
    };
    Some(out.lines().filter(|l| !footer(l)).collect::<Vec<_>>().join("\n"))
}

fn page_text(glyphs: &[Drawn], out: &mut String) {
    let mut words: Vec<Word> = Vec::new();
    let mut word: Vec<String> = Vec::new();
    let mut left = f64::INFINITY;
    let mut last_y: Option<f64> = None;
    let mut last_edges: Option<(f64, f64)> = None;
    let mut width = 0.0f64;
    let mut letters = 0usize;
    let flush_word = |word: &mut Vec<String>, left: &mut f64, words: &mut Vec<Word>| {
        if word.concat().chars().any(is_rtl) {
            word.reverse();
            // Digits and Latin letters inside a right-to-left word («40٪») read left to right.
            let mut k = 0;
            while k < word.len() {
                let ltr = |t: &String| t.chars().any(|c| c.is_ascii_alphanumeric());
                if word.get(k).is_some_and(ltr) {
                    let end = (k..word.len()).find(|&j| word.get(j).is_some_and(|t| !ltr(t) && !t.chars().all(|c| matches!(c, '.' | ',' | ':')))).unwrap_or(word.len());
                    if let Some(run) = word.get_mut(k..end) {
                        run.reverse();
                    }
                    k = end;
                } else {
                    k += 1;
                }
            }
        }
        let text = word.concat();
        if !text.trim().is_empty() {
            words.push(Word { left: *left, text: text.trim().to_string() });
        }
        word.clear();
        *left = f64::INFINITY;
    };
    for g in glyphs {
        // A glyph with no text of its own (a dot drawn apart, the rest of an ActualText span) or a
        // vowel mark sits above or below the line and does not start a new one.
        let mark = g.text.as_deref().is_none_or(is_mark);
        let edges = g.edges.filter(|_| !mark);
        let new_line = !mark && last_y.is_some_and(|l| (l - g.y).abs() > 4.0);
        // A letter wholly left of the last one (a jump back), or well to the right of it.
        let avg = if letters > 0 { width / letters as f64 } else { 0.0 };
        let apart = matches!((last_edges, edges), (Some((l0, l1)), Some((x0, x1))) if x1 < l0 || (avg > 0.0 && x0 - l1 > avg * 1.5));
        if new_line {
            flush_word(&mut word, &mut left, &mut words);
            out.push_str(&line_text(std::mem::take(&mut words)));
            out.push('\n');
            last_edges = None;
            width = 0.0;
            letters = 0;
        } else if apart {
            flush_word(&mut word, &mut left, &mut words);
        }
        if !mark {
            last_y = Some(g.y);
        }
        if let Some((x0, x1)) = edges {
            last_edges = Some((x0, x1));
            left = left.min(x0);
            width += (x1 - x0).max(0.0);
            letters += 1;
        }
        let Some(text) = &g.text else { continue };
        if text.chars().all(char::is_whitespace) {
            flush_word(&mut word, &mut left, &mut words);
        } else {
            word.push(text.clone());
        }
    }
    flush_word(&mut word, &mut left, &mut words);
    out.push_str(&line_text(words));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(left: f64, text: &str) -> Word {
        Word { left, text: text.into() }
    }

    #[test]
    fn right_to_left_lines_read_from_the_right() {
        // Visual order, left to right: «أحمد» is on the left, «سارة» on the right.
        assert_eq!(line_text(vec![w(10.0, "أحمد"), w(50.0, "سارة")]), "سارة أحمد");
        // An e-mail in an Arabic line keeps its own order.
        assert_eq!(line_text(vec![w(0.0, "a@b.no"), w(30.0, "·"), w(40.0, "السعودية"), w(80.0, "الرياض")]), "الرياض السعودية · a@b.no");
        assert_eq!(line_text(vec![w(20.0, "world"), w(0.0, "Hello")]), "Hello world");
        // A page number beside a running header in Arabic.
        assert_eq!(line_text(vec![w(0.0, "1"), w(5.0, "/"), w(10.0, "1"), w(40.0, "الأحمد"), w(80.0, "سارة")]), "سارة الأحمد 1 / 1");
    }

    #[test]
    fn scripts_that_need_it() {
        assert!(needs_layout_reading("سارة"));
        assert!(needs_layout_reading("प्रिया"));
        assert!(!needs_layout_reading("Kari Nordmann, Bodø"));
    }
}
