//! Unit tests for the mock provider — confirms the event sequence is
//! deterministic and matches both constructors' contracts.

use newt_core::provider::{Provider, RewriteEvent, RewriteRequest, mock::MockProvider};

fn collect(provider: &dyn Provider, prompt: &str) -> Vec<RewriteEvent> {
    let req = RewriteRequest {
        prompt: prompt.to_string(),
        model: None,
    };
    let mut events = Vec::new();
    provider
        .rewrite(&req, &mut |e| events.push(e))
        .expect("mock never errors");
    events
}

#[test]
fn echo_splits_input_into_whitespace_prefixed_tokens() {
    let events = collect(&MockProvider::echo("hello world"), "any prompt");

    assert_eq!(
        events,
        vec![
            RewriteEvent::Token {
                text: "hello".into()
            },
            RewriteEvent::Token {
                text: " world".into()
            },
            RewriteEvent::Usage {
                prompt_tokens: 2,
                completion_tokens: 2,
            },
            RewriteEvent::Done,
        ]
    );
}

#[test]
fn tokens_emits_scripted_sequence_verbatim() {
    let events = collect(
        &MockProvider::tokens(vec!["foo".into(), "-bar".into(), "!".into()]),
        "two words",
    );

    let expected = vec![
        RewriteEvent::Token { text: "foo".into() },
        RewriteEvent::Token {
            text: "-bar".into(),
        },
        RewriteEvent::Token { text: "!".into() },
        RewriteEvent::Usage {
            prompt_tokens: 2,
            completion_tokens: 3, // each token counts as one whitespace-separated word
        },
        RewriteEvent::Done,
    ];
    assert_eq!(events, expected);
}

#[test]
fn empty_input_yields_no_tokens_but_still_emits_usage_and_done() {
    let events = collect(&MockProvider::echo(""), "");

    assert_eq!(
        events,
        vec![
            RewriteEvent::Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
            },
            RewriteEvent::Done,
        ]
    );
}
