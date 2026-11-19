//! Reads a CV file without AI and exports it as PDF in the given designs, for checking import and
//! PDF text together: `cv_to_pdf <cv file> <out-dir> <design-id …>`.
use cv_core::{document, import};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("CV file missing")?;
    let out = args.next().unwrap_or_else(|| ".".into());
    let stem = std::path::Path::new(&input).file_stem().and_then(|s| s.to_str()).unwrap_or("cv").to_string();
    let mut project = import::read_cv(&import::text_from_file(std::path::Path::new(&input))?).project;
    for id in args {
        project.options.layout = id.clone();
        let doc = document::build(&project);
        std::fs::write(format!("{out}/{stem}-{id}.pdf"), document::to_pdf(&doc)?)?;
        println!("{stem}-{id}.pdf");
    }
    Ok(())
}
