//! OpenAI provider — chat completions, streaming, with usage reporting.
//!
//! API: <https://platform.openai.com/docs/api-reference/chat/create>
//!
//! Wire format (SSE):
//! ```text
//! data: {"choices":[{"delta":{"content":"hello"}}]}
//! data: {"choices":[{"delta":{"content":" world"}}]}
//! data: {"choices":[{"finish_reason":"stop","delta":{}}]}
//! data: {"usage":{"prompt_tokens":12,"completion_tokens":2,"total_tokens":14}}
//! data: [DONE]
//! ```
//!
//! `stream_options.include_usage = true` is required for the trailing usage
//! chunk; without it, we'd have no way to populate `RewriteEvent::Usage`.

use std::io::BufReader;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::json;

use super::{Provider, RewriteEvent, RewriteRequest, sse};

/// Default OpenAI model — cheap, fast, suitable for rewriting. Override per
/// prompt via the `model:` frontmatter or per-provider via config.
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAiProvider {
    api_key: String,
    default_model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Override the default model used when a request doesn't specify one.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }
}

impl Provider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn rewrite(
        &self,
        request: &RewriteRequest,
        on_event: &mut dyn FnMut(RewriteEvent),
    ) -> Result<()> {
        let model = request.model.as_deref().unwrap_or(&self.default_model);
        let body = json!({
            "model": model,
            "messages": [{ "role": "user", "content": request.prompt }],
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        let response = ureq::post(ENDPOINT)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .set("Accept", "text/event-stream")
            .send_json(body);

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                bail!("OpenAI returned HTTP {code}: {body}");
            }
            Err(e) => return Err(anyhow!("OpenAI request failed: {e}")),
        };

        let reader = BufReader::new(response.into_reader());
        let mut emitted_done = false;

        sse::for_each_event(reader, |event| {
            // OpenAI uses data-only events; ignore stray named events.
            if event.event.is_some() {
                return Ok(());
            }
            if event.data == "[DONE]" {
                if !emitted_done {
                    on_event(RewriteEvent::Done);
                    emitted_done = true;
                }
                return Ok(());
            }
            let chunk: ChatChunk = serde_json::from_str(&event.data)
                .with_context(|| format!("parsing OpenAI SSE chunk: {}", event.data))?;

            if let Some(choice) = chunk.choices.first()
                && let Some(content) = &choice.delta.content
                && !content.is_empty()
            {
                on_event(RewriteEvent::Token {
                    text: content.clone(),
                });
            }
            if let Some(usage) = chunk.usage {
                on_event(RewriteEvent::Usage {
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                });
            }
            Ok(())
        })?;

        // Some OpenAI deployments end the stream by closing the connection
        // before sending `[DONE]`; ensure consumers always see one Done.
        if !emitted_done {
            on_event(RewriteEvent::Done);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct ChatChunk {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}
