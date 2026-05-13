//! The rewrite pipeline: prompt + selection + provider → streamed events.
//!
//! Loads the named prompt (from disk or bundled defaults), renders the
//! template with the user's selection, and hands the result to the provider.
//! Events flow back through `on_event`. This is the single function that
//! everything user-facing — CLI, future GUI, future Tauri commands — calls
//! to perform a rewrite.

use anyhow::Result;

use crate::{
    paths::Paths,
    prompt,
    provider::{Provider, RewriteEvent, RewriteRequest},
    template,
};

/// Run a rewrite end-to-end. The provider's events are forwarded verbatim
/// through `on_event` — the pipeline itself does not synthesize or filter
/// any events.
pub fn run(
    paths: &Paths,
    prompt_id: &str,
    selection: &str,
    provider: &dyn Provider,
    on_event: &mut dyn FnMut(RewriteEvent),
) -> Result<()> {
    let prompt = prompt::load(paths, prompt_id)?;
    let rendered = template::render(&prompt.template, selection);
    let request = RewriteRequest {
        prompt: rendered,
        model: prompt.model.clone(),
    };
    provider.rewrite(&request, on_event)
}
