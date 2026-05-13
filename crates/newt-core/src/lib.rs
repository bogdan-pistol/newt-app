//! Newt core engine: prompts, config, providers (later), and health checks.
//!
//! No UI dependencies. The CLI binary and (future) Tauri app both consume
//! this crate — see PRD §7.

pub mod config;
pub mod doctor;
pub mod paths;
pub mod prompt;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
