//! Server-Sent Events (SSE) parser shared by OpenAI and Anthropic providers.
//!
//! Format reference: <https://html.spec.whatwg.org/multipage/server-sent-events.html>
//!
//! In practice both providers use a tiny subset:
//!
//! ```text
//! event: message_start            (Anthropic; OpenAI omits the event line)
//! data: {"json": "payload"}
//! data: continuation              (rarely; we concatenate with `\n`)
//!
//! event: ...
//! data: ...
//! ```
//!
//! Events are delimited by blank lines. Lines starting with `:` are comments
//! (used by some servers as keep-alives) and ignored. We dispatch one
//! `SseEvent` per blank-line-terminated record.
//!
//! This parser is intentionally minimal: it does not handle reconnection
//! (`id:` / `retry:` fields), since the providers we use do not rely on
//! resumable streams.

use std::io::BufRead;

use anyhow::{Context, Result};

/// One parsed SSE event. `event` is `None` when the producer emits only `data:`
/// lines (which is OpenAI's style — every event is implicitly a `message`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Read SSE events from `reader` until EOF, invoking `on_event` for each
/// completed event. Returns when the stream ends or `on_event` returns `Err`.
pub fn for_each_event<R: BufRead>(
    reader: R,
    mut on_event: impl FnMut(SseEvent) -> Result<()>,
) -> Result<()> {
    let mut current_event: Option<String> = None;
    let mut current_data = String::new();

    for line in reader.lines() {
        let line = line.context("reading SSE line")?;

        // Blank line → dispatch current event (if any).
        if line.is_empty() {
            if !current_data.is_empty() || current_event.is_some() {
                on_event(SseEvent {
                    event: current_event.take(),
                    data: std::mem::take(&mut current_data),
                })?;
            }
            continue;
        }

        // Comment line → ignore.
        if line.starts_with(':') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("event:") {
            current_event = Some(rest.trim_start().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !current_data.is_empty() {
                current_data.push('\n');
            }
            current_data.push_str(rest.trim_start());
        }
        // Unknown field names (`id:`, `retry:`, etc.) are ignored — we do
        // not need them and silently accepting them is the spec's behaviour.
    }

    // Flush any trailing event that wasn't terminated by a blank line.
    if !current_data.is_empty() || current_event.is_some() {
        on_event(SseEvent {
            event: current_event,
            data: current_data,
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn collect(input: &str) -> Vec<SseEvent> {
        let mut out = Vec::new();
        for_each_event(Cursor::new(input), |e| {
            out.push(e);
            Ok(())
        })
        .unwrap();
        out
    }

    #[test]
    fn parses_openai_style_data_only_events() {
        let input = "data: {\"a\":1}\n\ndata: {\"a\":2}\n\ndata: [DONE]\n\n";
        let events = collect(input);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].data, "{\"a\":1}");
        assert_eq!(events[2].data, "[DONE]");
    }

    #[test]
    fn parses_anthropic_style_named_events() {
        let input = "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: content_block_delta\ndata: {\"text\":\"hi\"}\n\n";
        let events = collect(input);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.as_deref(), Some("message_start"));
        assert_eq!(events[1].event.as_deref(), Some("content_block_delta"));
        assert_eq!(events[1].data, "{\"text\":\"hi\"}");
    }

    #[test]
    fn ignores_comment_lines() {
        let input = ": keep-alive\n\ndata: {\"x\":1}\n\n: another\n\n";
        let events = collect(input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"x\":1}");
    }

    #[test]
    fn concatenates_multi_line_data_with_newline() {
        let input = "data: line1\ndata: line2\n\n";
        let events = collect(input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn flushes_trailing_event_without_blank_terminator() {
        let input = "data: {\"end\":true}";
        let events = collect(input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"end\":true}");
    }
}
