//! LLM provider abstraction.
//!
//! Real providers (OpenAI, Anthropic, local models) and the test-only
//! `MockProvider` all implement this trait. The engine and CLI talk to the
//! trait, never to a specific provider, so adding a new backend is purely
//! additive (PRD §5.10).
//!
//! ## Streaming model
//!
//! `rewrite` is sync and callback-based: the provider invokes `on_event`
//! for each emitted [`RewriteEvent`]. This keeps the engine easy to reason
//! about — no async runtime, no futures, no executor — while still
//! exercising the streaming codepath end-to-end. When/if Tauri's async UI
//! needs to call into providers without blocking, we wrap calls in
//! `tokio::task::spawn_blocking` rather than coloring the whole trait async.
//!
//! Setup-time errors (auth, network unreachable) are returned as
//! `Result::Err`. Mid-stream success events flow only through the callback.

use anyhow::Result;
use serde::Serialize;

pub mod anthropic;
pub mod mock;
pub mod openai;
pub mod sse;

/// Inputs for a single rewrite call.
///
/// Instructions and content travel in separate channels: `system` carries
/// the prompt body (what to do) and `user` carries the selected text (what
/// to operate on). This separation is what makes the rewrite pipeline
/// resistant to prompt injection — content embedded in `user` cannot be
/// promoted to instruction status the way a single concatenated message
/// can. PRD §5.5.
#[derive(Debug, Clone)]
pub struct RewriteRequest {
    /// The prompt's instruction body. Sent as the LLM's system message.
    /// `None` is allowed for callers that have nothing to say (mock
    /// provider tests); real providers will simply omit the system message.
    pub system: Option<String>,
    /// The user's selected text — the content to operate on. Sent as the
    /// single user message. Never concatenated with `system`.
    pub user: String,
    /// Optional model override. `None` means the provider's default.
    pub model: Option<String>,
}

/// Events emitted by a provider during streaming.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RewriteEvent {
    /// A chunk of generated output. May be a single character, a word, or any
    /// arbitrary substring depending on the provider's tokenization.
    Token { text: String },
    /// Token-accounting summary. Emitted at most once, typically just before [`Done`].
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    /// End-of-stream sentinel. The provider emits this exactly once on success.
    Done,
}

/// An LLM provider capable of running a single rewrite call.
pub trait Provider {
    /// Stable identifier used in config and the CLI (e.g. `"mock"`, `"openai"`).
    fn name(&self) -> &'static str;

    /// Run the request, invoking `on_event` for each emitted event in order.
    /// Returns `Err` only for setup-time failures (no events have been emitted).
    fn rewrite(
        &self,
        request: &RewriteRequest,
        on_event: &mut dyn FnMut(RewriteEvent),
    ) -> Result<()>;
}
