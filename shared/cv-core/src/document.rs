//! Builds the CV as a WordCraft document. Word (.docx) and PDF are both written from this one
//! document and its one layout, so the page count and line breaks match between the two files.
use std::sync::Arc;

use base64::Engine as _;
use wordcraft_doc::para::{Anchor, Float, FloatAlign, InlineObject, Wrap};
use wordcraft_doc::props::{Border, BorderStyle, Borders, LineSpacing, NumRef, TabAlign, TabLeader, TabStop};
use wordcraft_doc::{Align, Block, CharProps, Document, ListKind, Paragraph, PartKind, Rgb, TextColor, para_block};

use crate::layouts::{HeadingStyle, Layout, layout};
use crate::model::Project;
use crate::text;

/// A4 in points.
pub const PAGE_W: f32 = 595.28;
pub const PAGE_H: f32 = 841.89;
/// 17 mm side and top margins, like the HTML builder's preview.
pub const MARGIN: f32 = 48.19;
pub const TEXT_W: f32 = PAGE_W - 2.0 * MARGIN;
/// Portrait box: 25 × 32 mm.
pub const PHOTO_W: f32 = 70.87;
pub const PHOTO_H: f32 = 90.71;

struct Builder<'a> {
    l: &'a Layout,
    bullets: u32,
    body: Vec<Block>,
}

fn color(rgb: Rgb) -> Option<TextColor> {
    Some(TextColor::Rgb(rgb))
}

impl Builder<'_> {
    fn chars(&self, size: f32, bold: bool, rgb: Rgb, font: &str) -> CharProps {
        CharProps { font: Some(font.into()), size: Some(size), bold: Some(bold), color: color(rgb), ..Default::default() }
    }
    fn body_chars(&self) -> CharProps {
        self.chars(self.l.size, false, self.l.ink, self.l.body_font)
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
    /// Left text with a right-aligned detail on the same line (dates, place), joined by a tab.
    fn two_column(&self, left: &str, right: &str, left_props: CharProps, after: f32) -> Paragraph {
        let muted = self.chars(self.l.size - 1.5, false, self.l.muted, self.l.body_font);
        let mut p = if right.is_empty() { self.para(&[(left, left_props)], after) } else { self.para(&[(left, left_props), (&format!("\t{right}"), muted)], after) };
        p.props.tabs = Some(vec![TabStop { pos: TEXT_W, align: TabAlign::Right, leader: TabLeader::None }]);
        p.props.keep_next = Some(true);
        p.props.keep_lines = Some(true);
        p
    }
    fn heading(&mut self, text: &str) {
        let l = self.l;
        let props = self.chars(l.size + if l.upper_headings { 0.5 } else { 1.5 }, true, l.accent, l.head_font);
        let mut p = self.para(&[(&l.heading(text), props)], self.space(6.0)).styled("Heading1");
        p.props.space_before = Some(self.space(11.0));
        p.props.keep_next = Some(true);
        p.props.outline_level = Some(0);
        let line = |width: f32, space: f32| Some(Border { style: BorderStyle::Single, width, color: Some(l.rule), space });
        match l.heading_style {
            HeadingStyle::Rule => p.props.borders = Some(Borders { bottom: line(0.6, 3.0), ..Default::default() }),
            HeadingStyle::Bar => {
                p.props.borders = Some(Borders { left: line(2.4, 5.0), ..Default::default() });
                p.props.indent_left = Some(7.0);
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

/// The whole CV as a WordCraft document on A4.
pub fn build(project: &Project) -> Document {
    let l = layout(project.options.layout);
    let labels = text::labels(project);
    let mut doc = Document::new();
    let c = &project.candidate;
    doc.core.title = format!("{} – CV", c.name);
    doc.core.creator = c.name.clone();
    doc.core.subject = [project.target.role.as_str(), project.target.company.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" – ");
    doc.core.keywords = project.target.keywords.join(", ");
    doc.core.description = c.headline.clone();
    let sec = &mut doc.last_section;
    sec.page_w = PAGE_W;
    sec.page_h = PAGE_H;
    sec.margin_top = MARGIN;
    sec.margin_left = MARGIN;
    sec.margin_right = MARGIN;
    sec.margin_bottom = 42.5;
    sec.footer = 21.0;
    let mut numbering = (*doc.numbering).clone();
    let bullets = numbering.add_list(ListKind::Bullet);
    doc.numbering = Arc::new(numbering);
    let photo = photo_bytes(project).map(|bytes| doc.add_media(bytes, "jpeg"));
    let mut b = Builder { l, bullets, body: Vec::new() };

    // Header: name, headline, contact. Plain body text, never in the page header or a text box.
    let header_indent = if photo.is_some() { PHOTO_W + 22.0 } else { 0.0 };
    let mut name = b.para(&[(&c.name, b.chars(l.name_size, l.name_bold, l.ink, l.head_font))], 4.0).styled("Title");
    name.props.indent_right = Some(header_indent);
    if let Some(media) = photo {
        let float = Float { wrap: Wrap::InFrontOfText, h_rel: Anchor::Margin, v_rel: Anchor::Margin, h_align: Some(FloatAlign::End), v_align: Some(FloatAlign::Start), ..Default::default() };
        let image = InlineObject::Image { media, w: PHOTO_W, h: PHOTO_H, alt: format!("Portrett av {}", c.name), float, crop: [0.0; 4], fx: Default::default(), ole: None };
        let _ = name.insert_object(0, image, &CharProps::default());
    }
    b.body.push(Block::Para(name));
    let mut headline = b.para(&[(&c.headline, b.chars(l.size + 1.5, true, l.accent, l.body_font))], 4.0);
    headline.props.indent_right = Some(header_indent);
    b.body.push(Block::Para(headline));
    let mut contact = b.para(&[(&text::contact_parts(project).join("  ·  "), b.chars(l.size - 1.0, false, l.muted, l.body_font))], 6.0);
    contact.props.indent_right = Some(header_indent);
    if photo_bytes(project).is_some() {
        // Keep the first heading below the portrait.
        contact.props.space_after = Some(20.0);
    }
    b.body.push(Block::Para(contact));

    if !c.summary.trim().is_empty() {
        b.heading(labels.profile);
        let p = b.para(&[(&c.summary, b.body_chars())], b.space(4.0));
        b.body.push(Block::Para(p));
    }
    let groups = text::group_skills(&project.skills);
    if !groups.is_empty() {
        b.heading(labels.skills);
        for (group, names) in groups {
            let bold = b.chars(l.size, true, l.ink, l.body_font);
            let label = if group.is_empty() { String::new() } else { format!("{group}: ") };
            let p = b.para(&[(&label, bold), (&names.join(", "), b.body_chars())], b.space(2.5));
            b.body.push(Block::Para(p));
        }
    }
    let jobs = text::sort_jobs(&project.experience);
    if !jobs.is_empty() {
        b.heading(labels.experience);
        for job in jobs {
            let title = b.two_column(&job.title, &text::period(job, labels), b.chars(l.size, true, l.ink, l.body_font), 1.0);
            let company = b.two_column(&job.company, &job.location, b.chars(l.size, false, l.accent, l.body_font), 3.0);
            b.body.push(Block::Para(title));
            b.body.push(Block::Para(company));
            for (i, bullet) in job.bullets.iter().enumerate() {
                b.bullet(bullet, i + 1 == job.bullets.len());
            }
        }
    }
    let education = text::sort_education(&project.education);
    if !education.is_empty() {
        b.heading(labels.education);
        for item in education {
            let top = b.two_column(&item.qualification, &text::year_range(&item.start, &item.end), b.chars(l.size, true, l.ink, l.body_font), 1.0);
            let place = b.para(&[(&item.institution, b.chars(l.size, false, l.accent, l.body_font))], b.space(6.0));
            b.body.push(Block::Para(top));
            b.body.push(Block::Para(place));
        }
    }
    let certs = text::sort_certifications(&project.certifications);
    if !certs.is_empty() {
        b.heading(labels.certifications);
        for cert in certs {
            let rest: Vec<&str> = [cert.issuer.as_str(), cert.year.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
            let detail = if rest.is_empty() { String::new() } else { format!(" · {}", rest.join(" · ")) };
            let p = b.para(&[(&cert.name, b.chars(l.size, true, l.ink, l.body_font)), (&detail, b.chars(l.size, false, l.muted, l.body_font))], b.space(2.5));
            b.body.push(Block::Para(p));
        }
    }
    if !project.languages.is_empty() {
        b.heading(labels.languages);
        let line = project.languages.iter().map(|x| format!("{}: {}", x.name, x.level)).collect::<Vec<_>>().join(" · ");
        let p = b.para(&[(&line, b.body_chars())], 0.0);
        b.body.push(Block::Para(p));
    }
    let interests = text::selected_interests(project);
    if !interests.is_empty() {
        b.heading(labels.interests);
        let p = b.para(&[(&interests.join(" · "), b.body_chars())], 0.0);
        b.body.push(Block::Para(p));
    }

    // Footer: name on the left, "page / pages" on the right.
    let small = CharProps { font: Some(l.body_font.into()), size: Some(8.0), color: color(l.muted), ..Default::default() };
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
    footer.props.tabs = Some(vec![TabStop { pos: TEXT_W, align: TabAlign::Right, leader: TabLeader::None }]);
    footer.props.align = Some(Align::Left);
    let footer_id = doc.add_part(PartKind::Footer, vec![para_block(footer)]);
    doc.last_section.footers.default = Some(footer_id);

    doc.body = b.body.into_iter().map(Arc::new).collect();
    doc.ensure_nonempty();
    doc
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
