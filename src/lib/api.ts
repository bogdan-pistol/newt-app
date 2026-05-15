// Thin typed wrapper over Tauri's invoke + listen so the rest of the
// frontend doesn't import from @tauri-apps/api everywhere.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AccessibilityStatus,
  Prompt,
  PromptInput,
  ProviderStatus,
  ProviderTestResult,
  RewriteEvent,
  SelectionPayload,
} from "./types";

export const api = {
  listPrompts: () => invoke<Prompt[]>("list_prompts"),
  createPrompt: (id: string, input: PromptInput) =>
    invoke<void>("create_prompt", { id, input }),
  updatePrompt: (id: string, input: PromptInput) =>
    invoke<void>("update_prompt", { id, input }),
  deletePrompt: (id: string) => invoke<boolean>("delete_prompt", { id }),

  listProviders: () => invoke<ProviderStatus[]>("list_providers"),
  setProviderKey: (provider: string, key: string) =>
    invoke<void>("set_provider_key", { provider, key }),
  removeProviderKey: (provider: string) =>
    invoke<boolean>("remove_provider_key", { provider }),
  testProvider: (provider: string) =>
    invoke<ProviderTestResult>("test_provider", { provider }),

  runRewrite: (providerName: string, promptId: string, selection: string) =>
    invoke<void>("run_rewrite", { providerName, promptId, selection }),

  onRewriteEvent: (
    handler: (event: RewriteEvent) => void,
  ): Promise<UnlistenFn> =>
    listen<RewriteEvent>("rewrite:event", (e) => handler(e.payload)),

  /**
   * Fires when the user wants to rewrite the current clipboard contents —
   * either via the tray menu's "Rewrite Clipboard…" item or via the
   * global hotkey (default ⌘+;). Frontend reads the clipboard, runs the
   * pipeline, and shows the streaming result in the rewrite panel.
   */
  onClipboardTrigger: (handler: () => void): Promise<UnlistenFn> =>
    listen("rewrite:clipboard-trigger", () => handler()),

  /**
   * Fires when the global hotkey captures a fresh selection from the
   * focused app via the ⌘C trick. `payload.text` is the captured text on
   * success, or null with an `error` reason on failure (most commonly,
   * Accessibility permission isn't granted).
   */
  onSelectionCaptured: (
    handler: (payload: SelectionPayload) => void,
  ): Promise<UnlistenFn> =>
    listen<SelectionPayload>("rewrite:selection", (e) => handler(e.payload)),

  // Accessibility permission + paste-back (PRD §5.2, §5.8).
  accessibilityStatus: () => invoke<AccessibilityStatus>("accessibility_status"),
  openAccessibilitySettings: () =>
    invoke<void>("open_accessibility_settings"),
  replaceSelection: (text: string) =>
    invoke<void>("replace_selection", { text }),
  /**
   * Undo the just-pasted rewrite by re-focusing the source app and
   * synthesising ⌘Z. Used by the popup's "Undo" button after an
   * auto-replace — letting the source app's native undo stack handle the
   * un-replace is cleaner than trying to re-paste the original text.
   */
  undoInSource: () => invoke<void>("undo_in_source"),
};
