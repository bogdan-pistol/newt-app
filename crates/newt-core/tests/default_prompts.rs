//! Snapshot test for the eight bundled default prompts.
//!
//! If a default prompt's text or metadata changes — intentionally or not —
//! this test fails until the snapshot is updated with `cargo insta review`.

use newt_core::prompt::{self, DEFAULT_PROMPTS};

#[test]
fn default_prompts_parse_consistently() {
    let parsed: Vec<_> = DEFAULT_PROMPTS
        .iter()
        .map(|(filename, contents)| {
            let id = filename.trim_end_matches(".md");
            prompt::parse(id, contents).expect("default prompt should parse")
        })
        .collect();

    assert_eq!(parsed.len(), 8, "we ship eight default prompts");
    insta::assert_yaml_snapshot!(parsed);
}
