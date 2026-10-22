//! Every file in `snippets/` is a copy someone could make, named
//! `<expected>--<what>[@<application>].txt`. `<expected>` is the language's
//! extension, or `none` when the text must not be taken for code; the optional
//! `@<application>` is where it was copied from. Dropping a file in is how a
//! misread snippet becomes a regression test.

use copycopy_core::lang::detect_from;
use std::fs;

#[test]
fn every_snippet_reads_as_its_name_says() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snippets");
    let mut failures = Vec::new();
    let mut total = 0;
    for entry in fs::read_dir(dir).expect("snippets directory") {
        let path = entry.expect("entry").path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("file name");
        let (name, source) = stem.split_once('@').unwrap_or((stem, ""));
        let expected = name.split_once("--").map_or(name, |(lang, _)| lang);
        let text = fs::read_to_string(&path).expect("snippet");
        let got = detect_from(&text, source).map_or("none", |l| l.token());
        total += 1;
        if got != expected {
            failures.push(format!("{stem}: read as {got}"));
        }
    }
    assert!(total > 0, "no snippets found");
    assert!(
        failures.is_empty(),
        "{} of {total} misread:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
