//! Test-mode simulation of the macOS clipboard capture and paste-back paths.
//!
//! In Phase 1 there is no real macOS integration yet — `simulate::selection`
//! pretends "the OS handed us this text" and runs the rewrite pipeline; the
//! "paste-back" target is a file under `<home>/test/replace-buffer.txt` that
//! a test harness or `newt simulate-replace` can read.
//!
//! When Phase 4 adds real selection capture (clipboard ⌘C trick + restore)
//! and paste-back, these functions become the test-only entry points that
//! bypass the OS hops; the rewrite pipeline they call into stays the same.
//!
//! This is the contract PRD §8.2 commits to.

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::{
    paths::Paths,
    provider::{Provider, RewriteEvent},
    rewrite,
};

/// Path of the file that stands in for the paste-back target.
pub fn replace_buffer_path(paths: &Paths) -> PathBuf {
    paths.home.join("test").join("replace-buffer.txt")
}

/// Read the current contents of the replace buffer. Empty string if not yet written.
pub fn read_replace_buffer(paths: &Paths) -> Result<String> {
    let path = replace_buffer_path(paths);
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))
}

/// Write `text` to the replace buffer, creating the directory if needed.
pub fn write_replace_buffer(paths: &Paths, text: &str) -> Result<()> {
    let path = replace_buffer_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, text)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Simulate the full "selected text → rewritten → pasted back" flow.
///
/// Runs the same `rewrite::run` pipeline the user would trigger via the
/// (future) hotkey, forwards every event through `on_event`, and writes the
/// concatenation of all `Token` events to the replace buffer when the
/// pipeline succeeds. Returns the final concatenated text.
pub fn selection(
    paths: &Paths,
    prompt_id: &str,
    selection: &str,
    provider: &dyn Provider,
    on_event: &mut dyn FnMut(RewriteEvent),
) -> Result<String> {
    let mut collected = String::new();
    {
        let collected = &mut collected;
        let mut tee = |event: RewriteEvent| {
            if let RewriteEvent::Token { text } = &event {
                collected.push_str(text);
            }
            on_event(event);
        };
        rewrite::run(paths, prompt_id, selection, provider, &mut tee)?;
    }
    write_replace_buffer(paths, &collected)?;
    Ok(collected)
}
