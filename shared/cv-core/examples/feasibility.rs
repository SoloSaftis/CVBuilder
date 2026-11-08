//! Feasibility: build a small CV-shaped document, write DOCX and PDF, and count laid-out pages.
use std::sync::Arc;
use wordcraft_doc::props::NumRef;
use wordcraft_doc::{Block, CharProps, Document, ListKind, Paragraph};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document::new();
    doc.core.title = "Ola Nordmann – CV".into();
    let mut numbering = (*doc.numbering).clone();
    let bullets = numbering.add_list(ListKind::Bullet);
    doc.numbering = Arc::new(numbering);
    let mut body = vec![
        Block::Para(Paragraph::with_text("Ola Nordmann", CharProps::default()).styled("Title")),
        Block::Para(Paragraph::with_text("ola@example.no · +47 400 00 000 · Bergen, Norge", CharProps::default())),
        Block::Para(Paragraph::with_text("Arbeidserfaring", CharProps::default()).styled("Heading1")),
        Block::Para(Paragraph::with_text("Automasjonsingeniør – Æøå Teknikk AS", CharProps { bold: Some(true), ..Default::default() })),
    ];
    for line in ["Idriftsatte PLS-styring for 4 anlegg.", "Feilsøkte Modbus TCP og Profinet."] {
        let mut p = Paragraph::with_text(line, CharProps::default()).styled("ListParagraph");
        p.props.numbering = Some(NumRef { num: bullets, level: 0 });
        body.push(Block::Para(p));
    }
    doc.body = body.into_iter().map(Arc::new).collect();
    doc.ensure_nonempty();
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::write(format!("{out}/feasibility.docx"), wordcraft_docx::write(&doc)?)?;
    let pdf = wordcraft_pdf::export(&doc, &wordcraft_pdf::PdfOptions { tagged: true, ..Default::default() })?;
    std::fs::write(format!("{out}/feasibility.pdf"), pdf)?;
    let pages = wordcraft_layout::layout(&doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default()).pages.len();
    println!("feasibility: wrote DOCX and PDF; layout pages = {pages}");
    Ok(())
}
