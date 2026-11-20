//! «Hva et ATS leser»: our own exports read back with our own importer.
use cv_core::model::PageSize;
use cv_core::readback::{self, Field, State};
use cv_core::{Project, document};

fn back(p: &Project, word: bool) -> readback::ReadBack {
    let doc = document::build(p);
    let (bytes, ext) = if word { (document::to_docx(&doc).expect("docx"), "docx") } else { (document::to_pdf(&doc).expect("pdf"), "pdf") };
    readback::read_back(p, &bytes, ext).expect("read back")
}

fn lost(b: &readback::ReadBack) -> Vec<String> {
    b.fields.iter().filter(|f| f.state != State::Same).map(|f| format!("{:?} {:?}: {:?} -> {:?}", f.state, f.field, f.expected, f.read)).collect()
}

/// The one-column design is made for ATS: every field of the example comes back as it is,
/// from PDF and Word, on A4 and Letter.
#[test]
fn the_ats_design_reads_back_whole() {
    for (word, letter) in [(false, false), (true, false), (false, true)] {
        let mut p = Project::example();
        p.options.layout = "classic".into();
        if letter {
            p.options.page.size = PageSize::Letter;
        }
        let b = back(&p, word);
        assert!(b.fields.len() >= 10, "too few fields compared: {}", b.fields.len());
        assert!(lost(&b).is_empty(), "word={word} letter={letter}: {:#?}", lost(&b));
        assert!(b.text.contains(&p.candidate.name), "the text as read lacks the name");
    }
}

/// A two-column design is where an ATS loses things, and the read-back must say so rather than
/// pretend all is well.
#[test]
fn a_sidebar_design_shows_what_is_lost() {
    let mut p = Project::example();
    p.options.layout = "rr-pikachu-2".into();
    let b = back(&p, true);
    assert!(b.count(State::Missing) + b.count(State::Different) > 0, "a sidebar design read back whole: {:#?}", b.fields);
    assert!(b.fields.iter().any(|f| f.field == Field::Name));
}
