//! Health checks. PRD §5.7 / §8.6.
//!
//! Each check produces a structured `Check` record with a status and a
//! short message. The CLI renders these as a human-readable list or as
//! JSON via `--json`. This is the canonical "is Newt healthy" surface
//! for both humans and AI agents.

use anyhow::Result;
use serde::Serialize;

use crate::{config::Config, paths::Paths, prompt};

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
    let mut checks = Vec::new();

    checks.push(check_home_dir(paths));
    checks.push(check_prompts_dir(paths));
    checks.push(check_prompts_parse(paths));
    checks.push(check_config_parse(paths));

    Ok(Report { checks })
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
            status: Status::Warn,
            message: "no prompts found (run any newt command to seed defaults)".into(),
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
                    format!("not present (defaults in use)")
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
