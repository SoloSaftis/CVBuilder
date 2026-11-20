//! Prints the text CVBuilder reads from a CV file, as the import sees it: `pdf_text <file>`.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("CV file missing")?;
    println!("{}", cv_core::import::text_from_file(std::path::Path::new(&path))?);
    Ok(())
}
