//! Integration tests for the simulate ops and `doctor::run_full`.

use newt_core::{
    doctor::{self, Status},
    paths::Paths,
    prompt,
    provider::{RewriteEvent, mock::MockProvider},
    simulate,
};

#[test]
fn replace_buffer_round_trip_via_engine_api() {
    let tmp = tempdir();
    let paths = Paths::with_home(&tmp);

    assert_eq!(simulate::read_replace_buffer(&paths).unwrap(), "");
    simulate::write_replace_buffer(&paths, "first").unwrap();
    assert_eq!(simulate::read_replace_buffer(&paths).unwrap(), "first");
    simulate::write_replace_buffer(&paths, "second").unwrap();
    assert_eq!(simulate::read_replace_buffer(&paths).unwrap(), "second");
}

#[test]
fn simulate_selection_writes_streamed_output_to_replace_buffer() {
    let tmp = tempdir();
    let paths = Paths::with_home(&tmp);
    let provider = MockProvider::tokens(vec!["one".into(), " two".into(), " three".into()]);

    let mut streamed = String::new();
    let returned = simulate::selection(
        &paths,
        "improve-writing",
        "ignored input",
        &provider,
        &mut |event| {
            if let RewriteEvent::Token { text } = event {
                streamed.push_str(&text);
            }
        },
    )
    .expect("simulate::selection runs");

    let buffer = simulate::read_replace_buffer(&paths).unwrap();
    assert_eq!(streamed, "one two three");
    assert_eq!(returned, streamed);
    assert_eq!(buffer, streamed);
}

#[test]
fn doctor_full_reports_all_phase1_checks_pass_on_clean_install() {
    let tmp = tempdir();
    let paths = Paths::with_home(&tmp);
    // Mirror main(): seed defaults on first use. Without this, the prompts
    // dir is empty and `check_prompts_parse` rightfully fails.
    prompt::seed_defaults_if_empty(&paths).unwrap();

    let report = doctor::run_full(&paths).expect("doctor::run_full");

    assert!(
        report.ok(),
        "doctor --full should be green; got: {:?}",
        report.checks
    );

    let names: Vec<&str> = report.checks.iter().map(|c| c.name).collect();
    for expected in [
        "home directory",
        "prompts directory",
        "prompts loadable",
        "config file",
        "mock pipeline",
        "all prompts render",
        "replace buffer",
    ] {
        assert!(
            names.contains(&expected),
            "missing check `{expected}`; got {names:?}"
        );
    }

    for check in &report.checks {
        assert_ne!(
            check.status,
            Status::Fail,
            "{} failed: {}",
            check.name,
            check.message
        );
    }
}

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
