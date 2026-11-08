//! Builds the CV as a WordCraft document. Word (.docx) and PDF are both written from this one
//! document and its one layout, so the page count and line breaks match between the two files.
use std::sync::Arc;

use base64::Engine as _;
use wordcraft_doc::para::{Anchor, Float, FloatAlign, InlineObject, Wrap};
use wordcraft_doc::props::{Border, BorderStyle, Borders, LineSpacing, NumRef, TabAlign, TabLeader, TabStop};
use wordcraft_doc::props::{CellProps, HeightRule, RowProps, TableProps};
use wordcraft_doc::{Align, Block, Cell, CharProps, Document, ListKind, Paragraph, PartKind, Rgb, Row, Table, TextColor, para_block};

use crate::layouts::{Align as HeaderAlign, DateStyle, Design, HeaderPlace, HeadingStyle, Side as ColumnSide, effective, rgb};
use crate::model::Project;
use crate::text;

/// A4 in points.
pub const PAGE_W: f32 = 595.28;
pub const PAGE_H: f32 = 841.89;
/// 17 mm side and top margins, like the HTML builder's preview.
pub const MARGIN: f32 = 48.19;
pub const TEXT_W: f32 = PAGE_W - 2.0 * MARGIN;

const MM: f32 = 72.0 / 25.4;

/// The page as the CV's page setup says: size and margins in points.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub page_w: f32,
    pub page_h: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Geometry {
    pub fn of(project: &Project) -> Geometry {
        use crate::model::{Margins, PageSize};
        let p = &project.options.page;
        let (page_w, page_h) = match p.size {
            PageSize::A4 => (PAGE_W, PAGE_H),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
        };
        let [top, right, bottom, left] = match p.margins {
            Margins::Normal => [MARGIN, MARGIN, 42.5, MARGIN],
            Margins::Narrow => [36.0, 36.0, 36.0, 36.0],
            Margins::Moderate => [19.0 * MM, 25.4 * MM, 19.0 * MM, 25.4 * MM],
            Margins::Wide => [72.0, 72.0, 72.0, 72.0],
            Margins::Custom => p.custom_mm.map(|v| v.clamp(5.0, 40.0) * MM),
        };
        Geometry { page_w, page_h, top, right, bottom: bottom.max(30.0), left }
    }
    pub fn text_w(&self) -> f32 {
        self.page_w - self.left - self.right
    }
}
/// Portrait box: 25 × 32 mm.
pub const PHOTO_W: f32 = 70.87;
pub const PHOTO_H: f32 = 90.71;

struct Builder<'a> {
    l: &'a Design,
    bullets: u32,
    body: Vec<Block>,
    /// The colour of the section being written.
    current: Rgb,
    /// The width of the column being written, points.
    width: f32,
}

fn color(value: Rgb) -> Option<TextColor> {
    Some(TextColor::Rgb(value))
}

impl Builder<'_> {
    fn ink(&self) -> Rgb {
        rgb(&self.l.ink)
    }
    fn accent(&self) -> Rgb {
        rgb(&self.l.accent)
    }
    fn muted(&self) -> Rgb {
        rgb(&self.l.muted)
    }
    fn chars(&self, size: f32, bold: bool, value: Rgb, font: &str) -> CharProps {
        CharProps { font: Some(font.into()), size: Some(size), bold: Some(bold), color: color(value), ..Default::default() }
    }
    fn body_chars(&self) -> CharProps {
        self.chars(self.l.size, false, self.ink(), &self.l.body_font)
    }
    fn company_chars(&self) -> CharProps {
        CharProps { italic: Some(self.l.company_italic), ..self.chars(self.l.size, false, self.current, &self.l.body_font) }
    }
    fn space(&self, points: f32) -> f32 {
        points * self.l.gap
    }
    /// A paragraph made of runs, each with its own formatting.
    fn para(&self, runs: &[(&str, CharProps)], after: f32) -> Paragraph {
        let mut p = Paragraph::new();
        for (text, props) in runs {
            let at = p.len();
            let _ = p.insert_text(at, text, props);
        }
        p.props.space_before = Some(0.0);
        p.props.space_after = Some(after);
        p.props.line_spacing = Some(LineSpacing::Multiple(1.1 * self.l.gap.min(1.0) + 0.05));
        p
    }
    /// Left text with a detail (dates, place): right-aligned on the same line, or on the line
    /// below, as the design says.
    fn two_column(&mut self, left: &str, right: &str, left_props: CharProps, after: f32) {
        let muted = self.chars(self.l.size - 1.5, false, self.muted(), &self.l.body_font);
        if right.is_empty() || self.l.date_style == DateStyle::Right {
            let mut p = if right.is_empty() { self.para(&[(left, left_props)], after) } else { self.para(&[(left, left_props), (&format!("\t{right}"), muted)], after) };
            p.props.tabs = Some(vec![TabStop { pos: self.width, align: TabAlign::Right, leader: TabLeader::None }]);
            p.props.keep_next = Some(true);
            p.props.keep_lines = Some(true);
            self.body.push(Block::Para(p));
        } else {
            let mut p = self.para(&[(left, left_props)], 0.0);
            p.props.keep_next = Some(true);
            self.body.push(Block::Para(p));
            let mut d = self.para(&[(right, muted)], after);
            d.props.keep_next = Some(true);
            self.body.push(Block::Para(d));
        }
    }
    fn heading(&mut self, text: &str, key: &str) {
        let l = self.l;
        self.current = rgb(l.section_accent(key));
        let props = CharProps {
            small_caps: Some(l.heading_small_caps),
            spacing: (l.heading_spacing > 0.0).then_some(l.heading_spacing),
            ..self.chars(l.heading_size, true, self.current, &l.head_font)
        };
        let mut p = self.para(&[(&l.heading(text), props)], self.space(6.0)).styled("Heading1");
        p.props.space_before = Some(self.space(11.0));
        p.props.keep_next = Some(true);
        p.props.outline_level = Some(0);
        // A section with its own colour gets its line in that colour too.
        let rule = if l.section_colors.contains_key(key) { self.current } else { rgb(&l.rule) };
        let line = |width: f32, space: f32| Some(Border { style: BorderStyle::Single, width, color: Some(rule), space });
        match l.heading_style {
            HeadingStyle::Rule => p.props.borders = Some(Borders { bottom: line(0.6, 3.0), ..Default::default() }),
            HeadingStyle::RuleAbove => p.props.borders = Some(Borders { top: line(0.6, 3.0), ..Default::default() }),
            HeadingStyle::Double => p.props.borders = Some(Borders { top: line(0.6, 2.0), bottom: line(0.6, 2.0), ..Default::default() }),
            HeadingStyle::Bar => {
                p.props.borders = Some(Borders { left: line(2.4, 5.0), ..Default::default() });
                p.props.indent_left = Some(7.0);
            }
            HeadingStyle::Band => {
                p.props.shading = l.heading_fill.as_deref().map(rgb);
                p.props.indent_left = Some(4.0);
            }
            HeadingStyle::Plain => {}
        }
        self.body.push(Block::Para(p));
    }
    fn bullet(&mut self, text: &str, last: bool) {
        let mut p = self.para(&[(text, self.body_chars())], if last { self.space(8.0) } else { self.space(2.5) }).styled("ListParagraph");
        p.props.numbering = Some(NumRef { num: self.bullets, level: 0 });
        p.props.indent_left = Some(17.0);
        p.props.indent_first = Some(-11.0);
        p.props.keep_lines = Some(true);
        self.body.push(Block::Para(p));
    }
}

/// Decodes the project's portrait (a `data:image/jpeg;base64,` URL) to JPEG bytes.
pub fn photo_bytes(project: &Project) -> Option<Vec<u8>> {
    if !project.options.photo {
        return None;
    }
    let encoded = project.candidate.photo.strip_prefix("data:image/jpeg;base64,")?;
    base64::engine::general_purpose::STANDARD.decode(encoded).ok()
}

/// The whole CV as a WordCraft document on A4: one column, or a sidebar when the design has one.
pub fn build(project: &Project) -> Document {
    let l = effective(project);
    let labels = text::labels(project);
    let mut doc = Document::new();
    let c = &project.candidate;
    doc.core.title = format!("{} – CV", c.name);
    doc.core.creator = c.name.clone();
    doc.core.subject = [project.target.role.as_str(), project.target.company.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" – ");
    doc.core.keywords = project.target.keywords.join(", ");
    doc.core.description = c.headline.clone();
    let g = Geometry::of(project);
    let text_w = g.text_w();
    let sec = &mut doc.last_section;
    sec.page_w = g.page_w;
    sec.page_h = g.page_h;
    sec.margin_top = g.top;
    sec.margin_left = g.left;
    sec.margin_right = g.right;
    sec.margin_bottom = g.bottom;
    sec.footer = (g.bottom / 2.0).min(21.0);
    let mut numbering = (*doc.numbering).clone();
    let bullets = numbering.add_list(ListKind::Bullet);
    doc.numbering = Arc::new(numbering);
    let photo = photo_bytes(project).map(|bytes| doc.add_media(bytes, "jpeg"));
    let order = text::section_order(project);

    let body: Vec<Block> = match l.columns.clone() {
        None => {
            let mut b = Builder { l: &l, bullets, body: Vec::new(), current: rgb(&l.accent), width: text_w };
            write_header(&mut b, project, photo, true, true);
            // A break "before" a section, or "after" one (the next section that has content).
            let mut pending = false;
            for key in &order {
                let at = b.body.len();
                write_section(&mut b, project, labels, key);
                if b.body.len() > at && at > 0 && (pending || project.options.page.page_breaks.contains(key)) {
                    if let Some(Block::Para(p)) = b.body.get_mut(at) {
                        p.props.page_break_before = Some(true);
                    }
                    pending = false;
                }
                if b.body.len() > at && project.options.page.page_breaks_after.contains(key) {
                    pending = true;
                }
            }
            b.body
        }
        Some(cols) => {
            // The sidebar's own colours: its text and headings on its fill, no bands inside it.
            let mut sd = l.clone();
            if let Some(t) = &cols.text {
                sd.ink = t.clone();
                sd.muted = t.clone();
            }
            if let Some(h) = &cols.heading {
                sd.accent = h.clone();
                sd.rule = h.clone();
                sd.section_colors.clear();
            }
            if sd.heading_style == HeadingStyle::Band {
                sd.heading_style = HeadingStyle::Rule;
            }
            if cols.header != HeaderPlace::Side {
                // The header band belongs to the header; in the sidebar only when the name is there.
                sd.header_fill = None;
                sd.header_text = None;
            }
            sd.header_align = HeaderAlign::Left;
            sd.date_style = DateStyle::Below;
            // A narrow column holds more when its spacing is a little tighter.
            sd.gap = (sd.gap * 0.9).max(0.75);
            const PAD: f32 = 9.0;
            const GAP: f32 = 16.0;
            let side_w = (text_w * cols.width).round();
            let main_w = text_w - side_w;
            let side_inner = side_w - 2.0 * PAD;
            let main_inner = main_w - GAP;
            if cols.header == HeaderPlace::Side {
                // The name on one line in the sidebar: smaller when it would not fit.
                let longest = c.name.split_whitespace().map(|w| w.chars().count()).max().unwrap_or(1).max(1) as f32;
                let whole = c.name.chars().count().max(1) as f32;
                let fit_line = side_inner / (whole * 0.56);
                let fit_word = side_inner / (longest * 0.62);
                sd.name_size = sd.name_size.min(if fit_line >= 16.0 { fit_line } else { fit_word }).max(14.0);
            }
            let mut side = Builder { l: &sd, bullets, body: Vec::new(), current: rgb(&sd.accent), width: side_inner };
            let mut main = Builder { l: &l, bullets, body: Vec::new(), current: rgb(&l.accent), width: main_inner };
            let contact_in_side = cols.sections.iter().any(|k| k == "contact");
            let mut top = Builder { l: &l, bullets, body: Vec::new(), current: rgb(&l.accent), width: text_w };
            match cols.header {
                HeaderPlace::Top => write_header(&mut top, project, photo, !contact_in_side, true),
                HeaderPlace::Main => write_header(&mut main, project, photo, !contact_in_side, false),
                HeaderPlace::Side => write_header(&mut side, project, photo, !contact_in_side, false),
            }
            if contact_in_side {
                let parts = text::contact_parts(project);
                if !parts.is_empty() {
                    side.heading(labels.contact, "contact");
                    for part in parts {
                        let p = side.para(&[(&part, side.chars(sd.size - 0.5, false, side.ink(), &sd.body_font))], 2.5);
                        side.body.push(Block::Para(p));
                    }
                }
            }
            // Where the main column starts a new page (index into main.body).
            let mut breaks: Vec<usize> = Vec::new();
            let mut pending = false;
            for key in &order {
                if cols.sections.iter().any(|k| k == key) {
                    write_section(&mut side, project, labels, key);
                } else {
                    let at = main.body.len();
                    write_section(&mut main, project, labels, key);
                    if main.body.len() > at && at > 0 && (pending || project.options.page.page_breaks.contains(key)) {
                        breaks.push(at);
                        pending = false;
                    }
                    if main.body.len() > at && project.options.page.page_breaks_after.contains(key) {
                        pending = true;
                    }
                }
            }
            let first = |blocks: &mut Vec<Block>| {
                // The first heading in a column starts at the top, not after a gap.
                if let Some(Block::Para(p)) = blocks.first_mut() {
                    p.props.space_before = Some(0.0);
                }
            };
            first(&mut side.body);
            first(&mut main.body);
            let ensure = |mut v: Vec<Block>| {
                if v.is_empty() {
                    v.push(Block::Para(Paragraph::new()));
                }
                v
            };
            // The main column in parts, one per page break; the sidebar fills the first part and
            // its colour continues beside the rest.
            let mut parts: Vec<Vec<Block>> = Vec::new();
            let mut rest = main.body;
            for at in breaks.iter().rev() {
                parts.push(rest.split_off(*at));
            }
            parts.push(rest);
            parts.reverse();
            let full_page = RowProps { height: Some(g.page_h - g.top - g.bottom - 40.0), height_rule: HeightRule::AtLeast, ..Default::default() };
            let mut side_blocks = Some(side.body);
            let mut out = top.body;
            for (i, mut part) in parts.into_iter().enumerate() {
                if i > 0 {
                    // A tiny paragraph that starts the next page; a table cannot start one itself.
                    let mut p = Paragraph::new();
                    let _ = p.insert_text(0, " ", &CharProps { size: Some(1.0), ..Default::default() });
                    p.props.page_break_before = Some(true);
                    p.props.space_after = Some(0.0);
                    p.props.line_spacing = Some(LineSpacing::Exactly(1.0));
                    out.push(Block::Para(p));
                    first(&mut part);
                }
                let side_cell = Cell {
                    props: CellProps { span: 1, width: Some(side_w), shading: cols.fill.as_deref().map(rgb), margins: Some([PAD, PAD, PAD, PAD]), ..Default::default() },
                    blocks: ensure(side_blocks.take().unwrap_or_default()).into_iter().map(Arc::new).collect(),
                    controls: Default::default(),
                };
                let (ml, mr) = if cols.side == ColumnSide::Left { (GAP, 0.0) } else { (0.0, GAP) };
                let main_cell = Cell {
                    props: CellProps { span: 1, width: Some(main_w), margins: Some([0.0, ml, 0.0, mr]), ..Default::default() },
                    blocks: ensure(part).into_iter().map(Arc::new).collect(),
                    controls: Default::default(),
                };
                let (cells, grid) = if cols.side == ColumnSide::Left { (vec![side_cell, main_cell], vec![side_w, main_w]) } else { (vec![main_cell, side_cell], vec![main_w, side_w]) };
                // With nothing above the columns, the sidebar runs to the foot of the page.
                let props = if i == 0 && cols.header == HeaderPlace::Top { RowProps::default() } else { full_page.clone() };
                out.push(Block::Table(Table {
                    props: TableProps { width: Some(text_w), fixed: true, cell_margins: Some([0.0; 4]), description: Some("CV i to spalter".into()), ..Default::default() },
                    grid,
                    rows: vec![Row { props, cells, controls: Default::default() }],
                    controls: Default::default(),
                }));
            }
            out
        }
    };

    // Footer: name on the left, "page / pages" on the right.
    let small = CharProps { font: Some(l.body_font.clone()), size: Some(8.0), color: color(rgb(&l.muted)), ..Default::default() };
    let mut footer = Paragraph::new();
    let _ = footer.insert_text(0, &format!("{}\t", c.name), &small);
    for (instr, sep) in [("PAGE", " / "), ("NUMPAGES", "")] {
        let at = footer.len();
        let _ = footer.insert_object(at, InlineObject::Field { instr: instr.into(), result: "1".into(), locked: false, dirty: true, code: false, form: String::new() }, &small);
        let at = footer.len();
        if !sep.is_empty() {
            let _ = footer.insert_text(at, sep, &small);
        }
    }
    footer.props.tabs = Some(vec![TabStop { pos: text_w, align: TabAlign::Right, leader: TabLeader::None }]);
    footer.props.align = Some(Align::Left);
    let footer_id = doc.add_part(PartKind::Footer, vec![para_block(footer)]);
    doc.last_section.footers.default = Some(footer_id);

    doc.body = body.into_iter().map(Arc::new).collect();
    doc.ensure_nonempty();
    doc
}

/// Name, headline and contact line (or no contact line when the sidebar lists it). A floating
/// portrait goes in the top-right corner; in a column it stands above the name.
fn write_header(b: &mut Builder<'_>, project: &Project, photo: Option<String>, contact_line: bool, float_photo: bool) {
    let l = b.l;
    let c = &project.candidate;
    let band = l.header_fill.as_deref().map(rgb);
    let on_band = |normal: Rgb| l.header_text.as_deref().map(rgb).unwrap_or(normal);
    let center = l.header_align == HeaderAlign::Center;
    let header_indent = if photo.is_some() && float_photo { PHOTO_W + 22.0 } else { 0.0 };
    // Padding around the band, drawn in the band's own colour, the same on every line so the
    // band's edges line up.
    let pad = |space: f32| Some(Border { style: BorderStyle::Single, width: 6.0, color: band, space });
    let dress = |p: &mut Paragraph| {
        p.props.indent_right = Some(header_indent);
        if center {
            p.props.align = Some(Align::Center);
            p.props.indent_left = Some(header_indent);
        }
        if band.is_some() {
            // One unbroken band: no spacing between the lines (it would show as white gaps).
            p.props.shading = band;
            p.props.space_after = Some(0.0);
            p.props.indent_left = Some(p.props.indent_left.unwrap_or(0.0) + 8.0);
            p.props.indent_right = Some(header_indent + 8.0);
            p.props.borders = Some(Borders { left: pad(0.0), right: pad(0.0), ..Default::default() });
        }
    };

    let name_text = if l.name_upper { c.name.to_uppercase() } else { c.name.clone() };
    let name_props = CharProps { spacing: (l.name_spacing > 0.0).then_some(l.name_spacing), ..b.chars(l.name_size, l.name_bold, on_band(b.ink()), &l.head_font) };
    let mut name = b.para(&[(&name_text, name_props)], 4.0).styled("Title");
    dress(&mut name);
    if band.is_some() {
        name.props.borders = Some(Borders { top: pad(2.0), left: pad(0.0), right: pad(0.0), ..Default::default() });
    }
    if let Some(media) = photo.clone() {
        if float_photo {
            let float = Float { wrap: Wrap::InFrontOfText, h_rel: Anchor::Margin, v_rel: Anchor::Margin, h_align: Some(FloatAlign::End), v_align: Some(FloatAlign::Start), ..Default::default() };
            let image = InlineObject::Image { media, w: PHOTO_W, h: PHOTO_H, alt: format!("Portrett av {}", c.name), float, crop: [0.0; 4], fx: Default::default(), ole: None };
            let _ = name.insert_object(0, image, &CharProps::default());
        } else {
            // In a column the portrait stands on its own line above the name.
            let w = PHOTO_W.min(b.width);
            let image = InlineObject::Image { media, w, h: w * PHOTO_H / PHOTO_W, alt: format!("Portrett av {}", c.name), float: Float::default(), crop: [0.0; 4], fx: Default::default(), ole: None };
            let mut p = Paragraph::new();
            let _ = p.insert_object(0, image, &CharProps::default());
            p.props.space_after = Some(8.0);
            b.body.push(Block::Para(p));
        }
    }
    b.body.push(Block::Para(name));
    let mut headline = b.para(&[(&c.headline, b.chars(l.size + 1.5, true, on_band(b.accent()), &l.body_font))], 4.0);
    dress(&mut headline);
    b.body.push(Block::Para(headline));
    if !contact_line {
        if let Some(Block::Para(last)) = b.body.last_mut() {
            last.props.space_after = Some(if band.is_some() { 14.0 } else { 8.0 });
        }
        return;
    }
    let mut contact = b.para(&[(&text::contact_parts(project).join("  ·  "), b.chars(l.size - 1.0, false, on_band(b.muted()), &l.body_font))], 6.0);
    dress(&mut contact);
    if band.is_some() {
        contact.props.borders = Some(Borders { bottom: pad(2.0), left: pad(0.0), right: pad(0.0), ..Default::default() });
        contact.props.space_after = Some(14.0);
    }
    if photo.is_some() && float_photo {
        // Keep the first heading below the portrait.
        contact.props.space_after = Some(20.0);
    }
    b.body.push(Block::Para(contact));
}

/// One section of the CV, written by the builder of the column it belongs in.
fn write_section(b: &mut Builder<'_>, project: &Project, labels: &text::Labels, key: &str) {
    let l = b.l;
    let c = &project.candidate;
    match key {
        "profile" => {
            if c.summary.trim().is_empty() {
                return;
            }
            b.heading(labels.profile, "profile");
            let p = b.para(&[(&c.summary, b.body_chars())], b.space(4.0));
            b.body.push(Block::Para(p));
        }
        "skills" => {
            let groups = text::group_skills(&project.skills);
            if groups.is_empty() {
                return;
            }
            b.heading(labels.skills, "skills");
            for (group, names) in groups {
                let bold = b.chars(l.size, true, b.ink(), &l.body_font);
                let label = if group.is_empty() { String::new() } else { format!("{group}: ") };
                let p = b.para(&[(&label, bold), (&names.join(", "), b.body_chars())], b.space(2.5));
                b.body.push(Block::Para(p));
            }
        }
        "experience" => {
            let jobs = text::sort_jobs(&project.experience);
            if jobs.is_empty() {
                return;
            }
            b.heading(labels.experience, "experience");
            for job in jobs {
                let title_props = b.chars(l.size, true, b.ink(), &l.body_font);
                b.two_column(&job.title, &text::period(job, labels), title_props, 1.0);
                let company_props = b.company_chars();
                b.two_column(&job.company, &job.location, company_props, 3.0);
                for (i, bullet) in job.bullets.iter().enumerate() {
                    b.bullet(bullet, i + 1 == job.bullets.len());
                }
            }
        }
        "education" => {
            let education = text::sort_education(&project.education);
            if education.is_empty() {
                return;
            }
            b.heading(labels.education, "education");
            for item in education {
                let props = b.chars(l.size, true, b.ink(), &l.body_font);
                b.two_column(&item.qualification, &text::year_range(&item.start, &item.end), props, 1.0);
                let place = b.para(&[(&item.institution, b.company_chars())], b.space(6.0));
                b.body.push(Block::Para(place));
            }
        }
        "certifications" => {
            let certs = text::sort_certifications(&project.certifications);
            if certs.is_empty() {
                return;
            }
            b.heading(labels.certifications, "certifications");
            for cert in certs {
                let rest: Vec<&str> = [cert.issuer.as_str(), cert.year.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
                let detail = if rest.is_empty() { String::new() } else { format!(" · {}", rest.join(" · ")) };
                let p = b.para(&[(&cert.name, b.chars(l.size, true, b.ink(), &l.body_font)), (&detail, b.chars(l.size, false, b.muted(), &l.body_font))], b.space(2.5));
                b.body.push(Block::Para(p));
            }
        }
        "languages" => {
            if project.languages.is_empty() {
                return;
            }
            b.heading(labels.languages, "languages");
            let line = project.languages.iter().map(|x| format!("{}: {}", x.name, x.level)).collect::<Vec<_>>().join(" · ");
            let p = b.para(&[(&line, b.body_chars())], b.space(4.0));
            b.body.push(Block::Para(p));
        }
        "interests" => {
            let interests = text::selected_interests(project);
            if interests.is_empty() {
                return;
            }
            b.heading(labels.interests, "interests");
            let p = b.para(&[(&interests.join(" · "), b.body_chars())], b.space(4.0));
            b.body.push(Block::Para(p));
        }
        id => {
            let Some(section) = project.custom_sections.iter().find(|s| s.id == id) else { return };
            let items: Vec<_> = section.items.iter().filter(|i| !i.title.trim().is_empty() || !i.bullets.is_empty()).collect();
            if section.title.trim().is_empty() || items.is_empty() {
                return;
            }
            b.heading(&section.title, id);
            for item in items {
                let props = b.chars(l.size, true, b.ink(), &l.body_font);
                let lone = item.subtitle.is_empty() && item.bullets.is_empty();
                b.two_column(&item.title, &item.date, props, if lone { b.space(4.0) } else { 1.0 });
                if !item.subtitle.is_empty() {
                    let p = b.para(&[(&item.subtitle, b.company_chars())], if item.bullets.is_empty() { b.space(6.0) } else { 3.0 });
                    b.body.push(Block::Para(p));
                }
                for (i, bullet) in item.bullets.iter().enumerate() {
                    b.bullet(bullet, i + 1 == item.bullets.len());
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Word export failed: {0}")]
    Docx(#[from] wordcraft_docx::DocxError),
    #[error("PDF export failed: {0}")]
    Pdf(#[from] wordcraft_pdf::PdfError),
}

/// Number of A4 pages the CV fills with this layout.
pub fn page_count(doc: &Document) -> usize {
    wordcraft_layout::layout(doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default()).pages.len()
}

pub fn to_docx(doc: &Document) -> Result<Vec<u8>, ExportError> {
    Ok(wordcraft_docx::write(doc)?)
}

/// Tagged PDF with embedded font subsets and selectable text.
pub fn to_pdf(doc: &Document) -> Result<Vec<u8>, ExportError> {
    let options = wordcraft_pdf::PdfOptions { tagged: true, links: true, ..Default::default() };
    Ok(wordcraft_pdf::export(doc, &options)?)
}
