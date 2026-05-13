//! Newt core engine: prompts, config, providers (later), and health checks.
//!
//! No UI dependencies. The CLI binary and (future) Tauri app both consume
//! this crate — see PRD §7.

pub mod config;
pub mod doctor;
pub mod keychain;
pub mod paths;
pub mod prompt;
pub mod provider;
pub mod rewrite;
pub mod simulate;
pub mod template;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
