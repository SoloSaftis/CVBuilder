//! Export a project file in every built-in design: `export_project <project.json> <out-dir> [design-id …]`.
use cv_core::{Project, document, layouts};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("project file missing")?;
    let out = args.next().unwrap_or_else(|| ".".into());
    let only: Vec<String> = args.collect();
    let mut project = Project::from_json(&std::fs::read_to_string(input)?)?;
    for design in layouts::all() {
        if !only.is_empty() && !only.contains(&design.id) {
            continue;
        }
        project.options.layout = design.id.clone();
        let doc = document::build(&project);
        std::fs::write(format!("{out}/{}.docx", design.id), document::to_docx(&doc)?)?;
        std::fs::write(format!("{out}/{}.pdf", design.id), document::to_pdf(&doc)?)?;
        println!("{}: {} page(s)", design.id, document::page_count(&doc));
    }
    Ok(())
}
