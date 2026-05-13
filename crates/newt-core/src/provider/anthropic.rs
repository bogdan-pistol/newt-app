//! Anthropic provider — messages API, streaming, with usage reporting.
//!
//! API: <https://docs.anthropic.com/en/api/messages-streaming>
//!
//! Wire format (SSE) — named events, JSON payloads:
//!
//! ```text
//! event: message_start
//! data: {"type":"message_start","message":{"usage":{"input_tokens":12,"output_tokens":1}}}
//!
//! event: content_block_delta
//! data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}
//!
//! event: message_delta
//! data: {"type":"message_delta","delta":{},"usage":{"output_tokens":2}}
//!
//! event: message_stop
//! data: {"type":"message_stop"}
//! ```
//!
//! Input tokens arrive once in `message_start`; output tokens are reported
//! cumulatively in `message_delta`. We coalesce both into a single
//! `RewriteEvent::Usage` emitted just before `Done`.

use std::io::BufReader;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::json;

use super::{Provider, RewriteEvent, RewriteRequest, sse};

/// Default Anthropic model — Claude Haiku 4.5, fast and inexpensive for
/// rewriting workloads.
pub const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// Cap on `max_tokens` for a rewrite. Rewrites are typically short; a high
/// cap is fine and avoids surprise truncation on long inputs.
const MAX_TOKENS: u32 = 4096;

pub struct AnthropicProvider {
    api_key: String,
    default_model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: DEFAULT_MODEL.to_string(),
        }
    }

    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        on_event: &mut dyn FnMut(RewriteEvent),
    ) -> Result<()> {
        let model = request.model.as_deref().unwrap_or(&self.default_model);
        let mut body = json!({
            "model": model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "messages": [{ "role": "user", "content": request.user }],
        });
        if let Some(sys) = &request.system
            && !sys.is_empty()
        {
            body["system"] = json!(sys);
        }

        let response = ureq::post(ENDPOINT)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", API_VERSION)
            .set("content-type", "application/json")
            .set("accept", "text/event-stream")
            .send_json(body);

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                bail!("Anthropic returned HTTP {code}: {body}");
            }
            Err(e) => return Err(anyhow!("Anthropic request failed: {e}")),
        };

        let reader = BufReader::new(response.into_reader());
        let mut input_tokens: u32 = 0;
        let mut output_tokens: u32 = 0;
        let mut emitted_done = false;

        sse::for_each_event(reader, |event| {
            let kind = event.event.as_deref().unwrap_or("");

            match kind {
                "message_start" => {
                    let payload: MessageStart = serde_json::from_str(&event.data)
                        .with_context(|| format!("parsing message_start: {}", event.data))?;
                    input_tokens = payload.message.usage.input_tokens;
                    output_tokens = payload.message.usage.output_tokens;
                }
                "content_block_delta" => {
                    let payload: ContentBlockDelta = serde_json::from_str(&event.data)
                        .with_context(|| format!("parsing content_block_delta: {}", event.data))?;
                    if payload.delta.kind == "text_delta" && !payload.delta.text.is_empty() {
                        on_event(RewriteEvent::Token {
                            text: payload.delta.text,
                        });
                    }
                }
                "message_delta" => {
                    let payload: MessageDelta = serde_json::from_str(&event.data)
                        .with_context(|| format!("parsing message_delta: {}", event.data))?;
                    output_tokens = payload.usage.output_tokens;
                }
                "message_stop" => {
                    on_event(RewriteEvent::Usage {
                        prompt_tokens: input_tokens,
                        completion_tokens: output_tokens,
                    });
                    on_event(RewriteEvent::Done);
                    emitted_done = true;
                }
                "error" => {
                    bail!("Anthropic stream error: {}", event.data);
                }
                _ => {} // ping, content_block_start/stop — irrelevant to us
            }
            Ok(())
        })?;

        // Defensive: if the server closed without sending message_stop, emit
        // a Usage + Done so consumers always see a clean termination.
        if !emitted_done {
            on_event(RewriteEvent::Usage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
            });
            on_event(RewriteEvent::Done);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct MessageStart {
    message: MessageMeta,
}

#[derive(Deserialize)]
struct MessageMeta {
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Deserialize)]
struct ContentBlockDelta {
    delta: TextDelta,
}

#[derive(Deserialize)]
struct TextDelta {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct MessageDelta {
    usage: MessageDeltaUsage,
}

#[derive(Deserialize)]
struct MessageDeltaUsage {
    #[serde(default)]
    output_tokens: u32,
}
