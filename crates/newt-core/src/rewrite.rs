//! The rewrite pipeline: prompt + selection + provider → streamed events.
//!
//! Loads the named prompt (from disk or bundled defaults) and dispatches a
//! `RewriteRequest` where the prompt's instructions live in the `system`
//! channel and the user's selection lives in the `user` channel — never
//! concatenated. This is the structural separation that makes the pipeline
//! resistant to prompt injection.

use anyhow::Result;

use crate::{
    paths::Paths,
    prompt,
    provider::{Provider, RewriteEvent, RewriteRequest},
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
    let request = RewriteRequest {
        system: Some(prompt.instructions),
        user: selection.to_string(),
        model: prompt.model,
    };
    provider.rewrite(&request, on_event)
}
