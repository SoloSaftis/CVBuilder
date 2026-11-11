//! Prints the text CVBuilder reads from a CV file, or with `--read` the CV it fills in from it:
//! `cargo run -p cv-core --example cvtext -- [--read] <file>`
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let read = args.iter().any(|a| a == "--read");
    let path = args.iter().find(|a| *a != "--read").expect("file");
    match cv_core::import::text_from_file(std::path::Path::new(path)) {
        Ok(t) if read => {
            let r = cv_core::import::read_cv(&t);
            println!("{}", r.project.to_json().unwrap_or_default());
            println!("--- found: {}\n--- rest:\n{}", r.found.join(", "), r.rest);
        }
        Ok(t) => println!("{t}"),
        Err(e) => eprintln!("{e}"),
    }
}
