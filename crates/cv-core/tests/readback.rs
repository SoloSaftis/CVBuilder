//! «Hva et ATS leser»: our own exports read back with our own importer.
use cv_core::model::PageSize;
use cv_core::readback::{self, State};
use cv_core::{Project, document};

fn back(p: &Project, word: bool) -> readback::ReadBack {
    let doc = document::build(p);
    let (bytes, ext) = if word { (document::to_docx(&doc).expect("docx"), "docx") } else { (document::to_pdf(&doc).expect("pdf"), "pdf") };
    readback::read_back(p, &bytes, ext).expect("read back")
}

#[test]
fn explore() {
    for (layout, word, letter) in [("classic", false, false), ("classic", true, false), ("rr-azurill-2", false, false), ("rr-pikachu-2", true, false), ("classic", false, true)] {
        let mut p = Project::example();
        p.options.layout = layout.into();
        if letter { p.options.page.size = PageSize::Letter; }
        let b = back(&p, word);
        eprintln!("== {layout} word={word} letter={letter}");
        for f in &b.fields { if f.state != State::Same { eprintln!("  {:?} {:?}: {:?} -> {:?}", f.state, f.field, f.expected, f.read); } }
        eprintln!("  same {}", b.count(State::Same));
    }
}
