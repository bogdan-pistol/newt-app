//! Health checks. PRD §5.7 / §8.6.
//!
//! Each check produces a structured `Check` record with a status and a
//! short message. The CLI renders these as a human-readable list or as
//! JSON via `--json`. This is the canonical "is Newt healthy" surface
//! for both humans and AI agents.

use anyhow::Result;
use serde::Serialize;

use crate::{
    config::Config,
    paths::Paths,
    prompt,
    provider::{RewriteEvent, mock::MockProvider},
    simulate,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
    }
}

/// Run every Phase 0 check and collect results. Never errors — failures are
/// reported as `Status::Fail` so the report itself is always renderable.
pub fn run(paths: &Paths) -> Result<Report> {
    let checks = vec![
        check_home_dir(paths),
        check_prompts_dir(paths),
        check_prompts_parse(paths),
        check_config_parse(paths),
    ];

    Ok(Report { checks })
}

/// Run every check `run` runs, plus end-to-end checks that exercise the
/// rewrite pipeline against the mock provider and the simulated paste-back
/// path. PRD §8.6 — the canonical "is the app healthy" command.
pub fn run_full(paths: &Paths) -> Result<Report> {
    let mut report = run(paths)?;
    report.checks.push(check_mock_rewrite_pipeline(paths));
    report.checks.push(check_all_prompts_render(paths));
    report.checks.push(check_replace_buffer_round_trip(paths));
    Ok(report)
}

fn check_home_dir(paths: &Paths) -> Check {
    match paths.ensure_dirs() {
        Ok(()) => Check {
            name: "home directory",
            status: Status::Pass,
            message: format!("{}", paths.home.display()),
        },
        Err(e) => Check {
            name: "home directory",
            status: Status::Fail,
            message: format!("could not create {}: {e}", paths.home.display()),
        },
    }
}

fn check_prompts_dir(paths: &Paths) -> Check {
    let dir = paths.prompts_dir();
    if dir.is_dir() {
        Check {
            name: "prompts directory",
            status: Status::Pass,
            message: format!("{}", dir.display()),
        }
    } else {
        Check {
            name: "prompts directory",
            status: Status::Fail,
            message: format!("missing: {}", dir.display()),
        }
    }
}

fn check_prompts_parse(paths: &Paths) -> Check {
    match prompt::list(paths) {
        Ok(prompts) if prompts.is_empty() => Check {
            name: "prompts loadable",
            status: Status::Fail,
            message:
                "no prompts found (the CLI seeds defaults on every run; this should be impossible)"
                    .into(),
        },
        Ok(prompts) => Check {
            name: "prompts loadable",
            status: Status::Pass,
            message: format!("{} prompt(s) parse cleanly", prompts.len()),
        },
        Err(e) => Check {
            name: "prompts loadable",
            status: Status::Fail,
            message: format!("parse error: {e:#}"),
        },
    }
}

fn check_config_parse(paths: &Paths) -> Check {
    match Config::load(paths) {
        Ok(cfg) => {
            let n = cfg.iter().count();
            Check {
                name: "config file",
                status: Status::Pass,
                message: if paths.config_file().exists() {
                    format!("{} ({} key(s))", paths.config_file().display(), n)
                } else {
                    "not present (defaults in use)".to_string()
                },
            }
        }
        Err(e) => Check {
            name: "config file",
            status: Status::Fail,
            message: format!("parse error: {e:#}"),
        },
    }
}

/// `simulate::selection` against `improve-writing`, asserting the replace
/// buffer ends up with the same text that streamed through the events.
fn check_mock_rewrite_pipeline(paths: &Paths) -> Check {
    let canary = "doctor canary input";
    let provider = MockProvider::echo(format!("[mock] {canary}"));
    let mut streamed = String::new();

    let result = simulate::selection(paths, "improve-writing", canary, &provider, &mut |event| {
        if let RewriteEvent::Token { text } = event {
            streamed.push_str(&text);
        }
    });

    match result {
        Err(e) => Check {
            name: "mock pipeline",
            status: Status::Fail,
            message: format!("simulate::selection failed: {e:#}"),
        },
        Ok(buffer) if buffer != streamed => Check {
            name: "mock pipeline",
            status: Status::Fail,
            message: format!("buffer != streamed: {buffer:?} vs {streamed:?}"),
        },
        Ok(_) => Check {
            name: "mock pipeline",
            status: Status::Pass,
            message: format!("rewrite + paste-back round-trip ({} chars)", streamed.len()),
        },
    }
}

/// Render every bundled prompt against the mock provider; catches a broken
/// template that only shows up at rewrite time.
fn check_all_prompts_render(paths: &Paths) -> Check {
    // Use the bundled default ids as the source of truth — `prompt::load`
    // falls back to bundled when the prompts dir hasn't been seeded yet, so
    // this works on a fresh install and a populated one alike.
    let ids: Vec<&str> = prompt::DEFAULT_PROMPTS
        .iter()
        .map(|(filename, _)| filename.trim_end_matches(".md"))
        .collect();

    let provider = MockProvider::echo("ok");
    let mut failures = Vec::new();
    for id in &ids {
        if let Err(e) = simulate::selection(paths, id, "x", &provider, &mut |_| {}) {
            failures.push(format!("{id}: {e}"));
        }
    }

    if failures.is_empty() {
        Check {
            name: "all prompts render",
            status: Status::Pass,
            message: format!("{} prompt(s) ran end-to-end", ids.len()),
        }
    } else {
        Check {
            name: "all prompts render",
            status: Status::Fail,
            message: failures.join("; "),
        }
    }
}

/// Direct read/write of the replace buffer — exercises the paste-back
/// plumbing without the rewrite pipeline in the way.
fn check_replace_buffer_round_trip(paths: &Paths) -> Check {
    let canary = "doctor-replace-buffer-canary";
    if let Err(e) = simulate::write_replace_buffer(paths, canary) {
        return Check {
            name: "replace buffer",
            status: Status::Fail,
            message: format!("write failed: {e:#}"),
        };
    }
    match simulate::read_replace_buffer(paths) {
        Ok(got) if got == canary => Check {
            name: "replace buffer",
            status: Status::Pass,
            message: format!("{}", simulate::replace_buffer_path(paths).display()),
        },
        Ok(got) => Check {
            name: "replace buffer",
            status: Status::Fail,
            message: format!("read mismatch: got {got:?}"),
        },
        Err(e) => Check {
            name: "replace buffer",
            status: Status::Fail,
            message: format!("read failed: {e:#}"),
        },
    }
}
