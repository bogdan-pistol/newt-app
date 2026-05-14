// Type contracts shared with the Rust backend (src-tauri/src/lib.rs).
// Keep these in sync with the Serialize types on the Rust side.

export type Prompt = {
  id: string;
  name: string;
  description: string;
  emoji: string | null;
  model: string | null;
  instructions: string;
};

export type PromptInput = {
  name: string;
  description: string;
  emoji: string | null;
  model: string | null;
  instructions: string;
};

export type ProviderStatus = {
  name: string;
  description: string;
  needs_key: boolean;
  key_set: boolean;
};

export type ProviderTestResult = {
  ok: boolean;
  response_chars: number;
};

export type RewriteEvent =
  | { type: "token"; text: string }
  | { type: "usage"; prompt_tokens: number; completion_tokens: number }
  | { type: "done" }
  | { type: "error"; message: string };

export type AccessibilityStatus = {
  granted: boolean;
};

/**
 * Payload for the `rewrite:selection` event the hotkey handler emits
 * after attempting selection capture. `text` is the captured text on
 * success; otherwise `text` is null and `error` carries the reason
 * (typically "Accessibility permission required" on first run).
 */
export type SelectionPayload = {
  text: string | null;
  error: string | null;
};
