// Thin typed wrapper over Tauri's invoke + listen so the rest of the
// frontend doesn't import from @tauri-apps/api everywhere.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Prompt,
  PromptInput,
  ProviderStatus,
  ProviderTestResult,
  RewriteEvent,
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
};
