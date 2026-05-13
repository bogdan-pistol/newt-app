//! Live tests against real OpenAI / Anthropic providers.
//!
//! Each test silently skips when its API-key env var is missing, so the
//! default `cargo test` run is offline and free. Set `OPENAI_API_KEY`
//! and/or `ANTHROPIC_API_KEY` to opt in. Each invocation costs roughly
//! a small fraction of a cent in tokens.

use newt_core::provider::{
    Provider, RewriteEvent, RewriteRequest, anthropic::AnthropicProvider, openai::OpenAiProvider,
};

fn tiny_request() -> RewriteRequest {
    RewriteRequest {
        system: Some("Respond with exactly the word 'ok' and nothing else.".to_string()),
        user: "ping".to_string(),
        model: None,
    }
}

fn assert_full_event_sequence(provider: &dyn Provider) {
    let mut got_token = false;
    let mut got_usage = false;
    let mut got_done = false;
    let mut tokens_text = String::new();

    provider
        .rewrite(&tiny_request(), &mut |e| match e {
            RewriteEvent::Token { text } => {
                got_token = true;
                tokens_text.push_str(&text);
            }
            RewriteEvent::Usage { .. } => got_usage = true,
            RewriteEvent::Done => got_done = true,
        })
        .expect("rewrite call should succeed");

    assert!(got_token, "expected at least one Token event");
    assert!(got_usage, "expected a Usage event");
    assert!(got_done, "expected a Done event");
    assert!(
        !tokens_text.trim().is_empty(),
        "expected non-empty response text"
    );
}

#[test]
fn openai_streams_tokens_usage_and_done() {
    let Ok(key) = std::env::var("OPENAI_API_KEY") else {
        eprintln!("skipping: OPENAI_API_KEY not set");
        return;
    };
    assert_full_event_sequence(&OpenAiProvider::new(key));
}

#[test]
fn anthropic_streams_tokens_usage_and_done() {
    let Ok(key) = std::env::var("ANTHROPIC_API_KEY") else {
        eprintln!("skipping: ANTHROPIC_API_KEY not set");
        return;
    };
    assert_full_event_sequence(&AnthropicProvider::new(key));
}
