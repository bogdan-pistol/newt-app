//! Deterministic mock provider for tests and the offline pipeline.
//!
//! Behavior is fully determined at construction time — same inputs always
//! produce the same event sequence — so it's safe to assert on streamed
//! output in snapshot tests without flake.

use anyhow::Result;

use super::{Provider, RewriteEvent, RewriteRequest};

/// A deterministic provider that emits a pre-decided sequence of tokens.
///
/// Use [`MockProvider::echo`] to derive tokens from a string by simple
/// whitespace splitting, or [`MockProvider::tokens`] to script tokens
/// verbatim when you need byte-for-byte control.
pub struct MockProvider {
    tokens: Vec<String>,
}

impl MockProvider {
    /// Produce a mock that streams `text` back, split into whitespace-prefixed
    /// word tokens (`"hello world"` → `["hello", " world"]`). This mimics how
    /// real LLM streams concatenate token-by-token.
    pub fn echo(text: impl Into<String>) -> Self {
        Self {
            tokens: tokenize(&text.into()),
        }
    }

    /// Produce a mock that emits exactly the given tokens, in order. Useful
    /// when the test needs a specific token boundary or non-whitespace split.
    pub fn tokens(tokens: Vec<String>) -> Self {
        Self { tokens }
    }
}

impl Provider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        on_event: &mut dyn FnMut(RewriteEvent),
    ) -> Result<()> {
        for tok in &self.tokens {
            on_event(RewriteEvent::Token { text: tok.clone() });
        }
        let system_words = request.system.as_deref().map(count_words).unwrap_or(0);
        let user_words = count_words(&request.user);
        on_event(RewriteEvent::Usage {
            prompt_tokens: system_words + user_words,
            completion_tokens: self.tokens.iter().map(|t| count_words(t)).sum(),
        });
        on_event(RewriteEvent::Done);
        Ok(())
    }
}

/// Split a string into "tokens": each token carries its leading whitespace,
/// so concatenating them reproduces the input modulo trailing whitespace.
///
/// `"hello  world"` → `["hello", "  world"]`
fn tokenize(s: &str) -> Vec<String> {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(words.len());
    let mut cursor = 0usize;
    for word in &words {
        let word_pos = s[cursor..].find(word).expect("word came from s") + cursor;
        let mut tok = String::new();
        // Include any whitespace between the previous cursor and this word
        // (skipping any leading whitespace before the very first word).
        if !out.is_empty() {
            tok.push_str(&s[cursor..word_pos]);
        }
        tok.push_str(word);
        out.push(tok);
        cursor = word_pos + word.len();
    }
    out
}

fn count_words(s: &str) -> u32 {
    s.split_whitespace().count() as u32
}
