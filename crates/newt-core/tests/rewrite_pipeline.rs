//! End-to-end test of the rewrite pipeline against the mock provider.
//!
//! Uses bundled defaults via `prompt::load`'s fallback path, so no on-disk
//! prompt seeding is required and the test is hermetic.

use newt_core::{
    paths::Paths,
    provider::{RewriteEvent, mock::MockProvider},
    rewrite,
};

#[test]
fn pipeline_renders_template_and_streams_mock_events() {
    let tmp = tempdir();
    let paths = Paths::with_home(&tmp);
    let provider = MockProvider::tokens(vec!["[mock]".into(), " ok".into()]);

    let mut events = Vec::new();
    rewrite::run(
        &paths,
        "improve-writing",
        "hello world",
        &provider,
        &mut |e| events.push(e),
    )
    .expect("pipeline runs against bundled default prompt");

    // Tokens come through verbatim.
    assert_eq!(events[0], RewriteEvent::Token { text: "[mock]".into() });
    assert_eq!(events[1], RewriteEvent::Token { text: " ok".into() });

    // Usage and Done come last, in order.
    let RewriteEvent::Usage { prompt_tokens, completion_tokens } = events[2] else {
        panic!("expected Usage event, got {:?}", events[2]);
    };
    assert!(prompt_tokens > 0, "rendered prompt should be non-empty");
    assert_eq!(completion_tokens, 2);
    assert_eq!(events[3], RewriteEvent::Done);
}

#[test]
fn unknown_prompt_id_is_a_clean_error() {
    let tmp = tempdir();
    let paths = Paths::with_home(&tmp);
    let provider = MockProvider::echo("ignored");

    let err = rewrite::run(
        &paths,
        "no-such-prompt",
        "x",
        &provider,
        &mut |_| panic!("provider should not be invoked when prompt is missing"),
    )
    .expect_err("unknown prompt id should error");

    assert!(
        err.to_string().contains("no-such-prompt"),
        "error should name the missing prompt: {err}"
    );
}

/// Allocate a fresh temp dir for the test home. We don't need cleanup —
/// `cargo test` runs in `target/` and the OS handles `/tmp` rotation.
fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!(
        "newt-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    base
}
