<script lang="ts">
  import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { onMount, onDestroy } from "svelte";
  import { api } from "./api";
  import type { Prompt, ProviderStatus, RewriteEvent } from "./types";

  type Props = {
    prompts: Prompt[];
    providers: ProviderStatus[];
  };

  let { prompts, providers }: Props = $props();

  let providerName = $state("openai");
  let promptId = $state("improve-writing");
  let selection = $state("");
  let output = $state("");
  let usage = $state<{ prompt_tokens: number; completion_tokens: number } | null>(
    null,
  );
  let status = $state<"idle" | "streaming" | "done" | "error">("idle");
  let errorMessage = $state<string | null>(null);
  let copied = $state(false);
  let replaced = $state(false);
  /**
   * True when the current selection came in via the global hotkey's
   * capture flow (not manual paste / clipboard read). Replace is only
   * meaningful in that case — there's no source app to paste back to
   * otherwise.
   */
  let capturedFromSource = $state(false);
  let unlistenSelection: (() => void) | null = null;

  // Pre-select a sensible default once we have data.
  $effect(() => {
    if (prompts.length > 0 && !prompts.find((p) => p.id === promptId)) {
      promptId = prompts[0].id;
    }
    const enabled = providers.filter((p) => !p.needs_key || p.key_set);
    if (enabled.length > 0 && !enabled.find((p) => p.name === providerName)) {
      providerName = enabled[0].name;
    }
  });

  let availableProviders = $derived(
    providers.filter((p) => !p.needs_key || p.key_set),
  );

  async function readClipboard() {
    try {
      const text = await readText();
      selection = text ?? "";
      // Manual clipboard read — there's no source app to paste back to.
      capturedFromSource = false;
    } catch (e) {
      errorMessage = `Couldn't read clipboard: ${e}`;
    }
  }

  // Public method invoked from App when the tray "Rewrite Clipboard…" item
  // fires — exported so the parent can call it via bind:this.
  export async function rewriteFromClipboard() {
    await readClipboard();
    if (selection.trim()) {
      await rewrite();
    }
  }

  async function rewrite() {
    if (!selection.trim()) {
      errorMessage = "Selection is empty.";
      return;
    }
    output = "";
    usage = null;
    errorMessage = null;
    status = "streaming";

    const unlisten = await api.onRewriteEvent((event: RewriteEvent) => {
      switch (event.type) {
        case "token":
          output += event.text;
          break;
        case "usage":
          usage = {
            prompt_tokens: event.prompt_tokens,
            completion_tokens: event.completion_tokens,
          };
          break;
        case "done":
          status = "done";
          unlisten();
          break;
        case "error":
          status = "error";
          errorMessage = event.message;
          unlisten();
          break;
      }
    });

    try {
      await api.runRewrite(providerName, promptId, selection);
    } catch (e) {
      status = "error";
      errorMessage = String(e);
      unlisten();
    }
  }

  async function copyOutput() {
    if (!output) return;
    try {
      await writeText(output);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (e) {
      errorMessage = `Couldn't copy: ${e}`;
    }
  }

  async function replaceInSourceApp() {
    if (!output) return;
    try {
      await api.replaceSelection(output);
      replaced = true;
      setTimeout(() => (replaced = false), 1500);
    } catch (e) {
      errorMessage = `Replace failed: ${e}`;
    }
  }

  // Auto-read clipboard once on mount so the panel is immediately useful.
  onMount(async () => {
    readClipboard();

    // Listen for hotkey-captured selections. When the backend successfully
    // grabs text via the ⌘C trick it emits this event with the captured
    // text; we prefill the input and auto-run the rewrite. Errors (e.g.
    // missing Accessibility permission, though that case is normally
    // gated upstream by the Onboarding screen) surface inline.
    unlistenSelection = await api.onSelectionCaptured((payload) => {
      if (payload.error) {
        errorMessage = payload.error;
        return;
      }
      if (payload.text && payload.text.trim()) {
        selection = payload.text;
        capturedFromSource = true;
        rewrite();
      }
    });
  });

  onDestroy(() => {
    unlistenSelection?.();
  });
</script>

<section class="rewrite-panel">
  <h2>Rewrite</h2>
  <div class="controls">
    <label>
      <span>Provider</span>
      <select bind:value={providerName}>
        {#each availableProviders as p (p.name)}
          <option value={p.name}>{p.name}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>Prompt</span>
      <select bind:value={promptId}>
        {#each prompts as p (p.id)}
          <option value={p.id}>{p.emoji ?? ""} {p.name}</option>
        {/each}
      </select>
    </label>
  </div>

  <label class="selection-label">
    <span>Input</span>
    <textarea
      bind:value={selection}
      placeholder="Type, paste, or click 'Read clipboard' to load text…"
      rows="4"
    ></textarea>
  </label>

  <div class="actions">
    <button onclick={readClipboard} class="secondary">Read clipboard</button>
    <button
      onclick={rewrite}
      disabled={status === "streaming" || !selection.trim() || availableProviders.length === 0}
      class="primary"
    >
      {status === "streaming" ? "Streaming…" : "Rewrite"}
    </button>
  </div>

  {#if errorMessage}
    <div class="error" role="alert">{errorMessage}</div>
  {/if}

  {#if output || status === "streaming"}
    <div class="output">
      <div class="output-header">
        <span class="muted">
          Output
          {#if usage}
            · {usage.prompt_tokens} in / {usage.completion_tokens} out
          {/if}
          {#if status === "done"}· done{/if}
        </span>
        <button onclick={copyOutput} disabled={!output} class="secondary tight">
          {copied ? "✓ Copied" : "Copy"}
        </button>
        {#if capturedFromSource}
          <button
            onclick={replaceInSourceApp}
            disabled={!output || status === "streaming"}
            class="primary tight"
          >
            {replaced ? "✓ Replaced" : "Replace"}
          </button>
        {/if}
      </div>
      <pre>{output}{status === "streaming" ? "▍" : ""}</pre>
    </div>
  {/if}

  {#if availableProviders.length === 0}
    <p class="hint">
      No providers available — set an API key in the Providers section below
      (or test the pipeline with the <code>mock</code> provider).
    </p>
  {/if}
</section>

<style>
  .rewrite-panel {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .controls {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }
  .controls label,
  .selection-label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .controls label span,
  .selection-label span {
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
    font-weight: 600;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  .output {
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 8px;
    overflow: hidden;
  }
  @media (prefers-color-scheme: dark) {
    .output {
      border-color: rgba(255, 255, 255, 0.08);
    }
  }
  .output-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.85rem;
    background: rgba(0, 0, 0, 0.03);
    font-size: 0.82rem;
    gap: 0.4rem;
  }
  @media (prefers-color-scheme: dark) {
    .output-header {
      background: rgba(255, 255, 255, 0.03);
    }
  }
  pre {
    margin: 0;
    padding: 0.85rem;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.85rem;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
