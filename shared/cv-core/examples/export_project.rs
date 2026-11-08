//! Export a project file in every layout: `export_project <project.json> <out-dir>`.
use cv_core::{LayoutId, Project, document};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("project file missing")?;
    let out = args.next().unwrap_or_else(|| ".".into());
    let mut project = Project::from_json(&std::fs::read_to_string(input)?)?;
    for (id, name) in [(LayoutId::Classic, "classic"), (LayoutId::Modern, "modern"), (LayoutId::Nordic, "nordic"), (LayoutId::Compact, "compact")] {
        project.options.layout = id;
        let doc = document::build(&project);
        std::fs::write(format!("{out}/{name}.docx"), document::to_docx(&doc)?)?;
        std::fs::write(format!("{out}/{name}.pdf"), document::to_pdf(&doc)?)?;
        println!("{name}: {} page(s)", document::page_count(&doc));
    }
    Ok(())
}
