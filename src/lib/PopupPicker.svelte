<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { api } from "./api";
  import type { Prompt, RewriteEvent } from "./types";

  type Props = {
    prompts: Prompt[];
    text: string;
    initialError: string | null;
    onClose: () => void;
  };

  let { prompts, text, initialError, onClose }: Props = $props();

  type Stage = "picker" | "streaming" | "replacing" | "done" | "error";

  let stage = $state<Stage>("picker");
  let query = $state("");
  let cursor = $state(0);
  let chosenPrompt = $state<Prompt | null>(null);
  let output = $state("");
  let usage = $state<{ prompt_tokens: number; completion_tokens: number } | null>(
    null,
  );
  let errorMessage = $state<string | null>(null);
  let undoing = $state(false);
  let queryInput: HTMLInputElement | null = $state(null);

  // Active provider from settings — single source of truth (no more
  // first-with-key heuristic). Falls back to mock if settings can't load.
  let activeProvider = $state("mock");

  // Reactive bridge from props → local state. If the parent passes an
  // `initialError` (e.g., the hotkey hit "no Accessibility permission"
  // before any pasteboard read could happen), surface it as the stage.
  $effect(() => {
    if (initialError) {
      stage = "error";
      errorMessage = initialError;
    }
  });

  // Provider preference: first key-bearing provider with a key set; mock
  // only as a last resort. This was the bug that produced "[mock] rewrite
  // output" earlier — `enabled[0]` always returned `mock` because mock
  // doesn't need a key.
  let provider = $derived(activeProvider);

  let filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return prompts;
    return prompts.filter((p) => {
      const hay = `${p.name} ${p.id} ${p.description ?? ""}`.toLowerCase();
      return hay.includes(q);
    });
  });

  $effect(() => {
    if (cursor >= filtered.length) cursor = Math.max(0, filtered.length - 1);
  });

  async function pickPrompt(p: Prompt) {
    if (!text.trim()) {
      errorMessage = "No text to rewrite.";
      stage = "error";
      return;
    }
    chosenPrompt = p;
    stage = "streaming";
    output = "";
    usage = null;
    errorMessage = null;

    const unlistenEvent = await api.onRewriteEvent((event: RewriteEvent) => {
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
          unlistenEvent();
          autoReplace();
          break;
        case "error":
          unlistenEvent();
          errorMessage = event.message;
          stage = "error";
          break;
      }
    });

    try {
      await api.runRewrite(provider, p.id, text);
    } catch (e) {
      unlistenEvent();
      errorMessage = String(e);
      stage = "error";
    }
  }

  async function autoReplace() {
    if (!output) {
      stage = "done";
      return;
    }
    stage = "replacing";
    try {
      await api.replaceSelection(output);
      stage = "done";
    } catch (e) {
      // Replace failed (e.g., source app gone). Show output + error so user
      // can copy manually; skip the auto-paste.
      errorMessage = `Replace failed: ${e}. Use Copy below.`;
      stage = "done";
    }
  }

  async function undo() {
    undoing = true;
    try {
      await api.undoInSource();
      onClose();
    } catch (e) {
      errorMessage = `Undo failed: ${e}`;
    } finally {
      undoing = false;
    }
  }

  async function copyOutput() {
    if (!output) return;
    try {
      await writeText(output);
    } catch (e) {
      errorMessage = `Couldn't copy: ${e}`;
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }
    if (stage === "picker") {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        cursor = Math.min(filtered.length - 1, cursor + 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        cursor = Math.max(0, cursor - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        const p = filtered[cursor];
        if (p) pickPrompt(p);
      }
    }
  }

  onMount(async () => {
    // Pull the user's chosen active provider from settings. If this fails
    // (corrupt config or whatever), we keep the default `mock` — the
    // popup will still work, just visibly using the test provider.
    try {
      const s = await api.getSettings();
      activeProvider = s.active_provider;
    } catch {
      // ignore; activeProvider stays "mock"
    }
    // Auto-focus the search input so typing filters immediately.
    await tick();
    queryInput?.focus();
  });

  onDestroy(() => {});
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="rewrite-view">
  {#if stage === "picker"}
    <div class="picker">
      <div class="search">
        <input
          bind:this={queryInput}
          bind:value={query}
          placeholder="Search prompts…"
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <ul class="prompt-list" role="listbox">
        {#each filtered as p, i (p.id)}
          <li
            class:selected={i === cursor}
            role="option"
            aria-selected={i === cursor}
            onclick={() => pickPrompt(p)}
            onkeydown={(e) => e.key === "Enter" && pickPrompt(p)}
            tabindex="-1"
          >
            <span class="emoji">{p.emoji ?? "  "}</span>
            <div class="prompt-meta">
              <div class="prompt-name">{p.name}</div>
              <div class="prompt-desc">{p.description}</div>
            </div>
          </li>
        {:else}
          <li class="empty">No prompts match.</li>
        {/each}
      </ul>
      <div class="footer">
        <span class="hint">↑↓ navigate · ↵ select · esc dismiss</span>
        <span class="provider">via {provider}</span>
      </div>
    </div>
  {:else if stage === "streaming" || stage === "replacing"}
    <div class="rewriting">
      <div class="header">
        <span class="emoji">{chosenPrompt?.emoji ?? "✨"}</span>
        <span class="prompt-name">{chosenPrompt?.name}</span>
        <span class="status">
          {stage === "streaming" ? "streaming…" : "pasting…"}
        </span>
      </div>
      <pre>{output}{stage === "streaming" ? "▍" : ""}</pre>
    </div>
  {:else if stage === "done"}
    <div class="rewriting done">
      <div class="header">
        <span class="emoji">{chosenPrompt?.emoji ?? "✨"}</span>
        <span class="prompt-name">{chosenPrompt?.name}</span>
        {#if usage}
          <span class="status">
            {usage.prompt_tokens}↑ / {usage.completion_tokens}↓
          </span>
        {/if}
      </div>
      <pre>{output}</pre>
      {#if errorMessage}
        <div class="error">{errorMessage}</div>
      {/if}
      <div class="footer">
        <button onclick={copyOutput} class="secondary tight">Copy</button>
        <button onclick={undo} disabled={undoing} class="secondary tight">
          {undoing ? "Undoing…" : "Undo"}
        </button>
        <button onclick={onClose} class="primary tight">Close ⎋</button>
      </div>
    </div>
  {:else if stage === "error"}
    <div class="error-state">
      <h3>Couldn't rewrite</h3>
      <p class="error">{errorMessage}</p>
      <div class="footer">
        <button onclick={onClose} class="primary tight">Close ⎋</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .rewrite-view {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }
  .picker,
  .rewriting,
  .error-state {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .search {
    padding: 0.85rem 1rem;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  }
  @media (prefers-color-scheme: dark) {
    .search {
      border-bottom-color: rgba(255, 255, 255, 0.08);
    }
  }
  .search input {
    width: 100%;
    border: none;
    background: transparent;
    font-size: 1.05rem;
    padding: 0.25rem 0;
    outline: none;
    color: inherit;
  }
  .prompt-list {
    list-style: none;
    margin: 0;
    padding: 0.25rem 0;
    overflow-y: auto;
    flex: 1;
  }
  .prompt-list li {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.55rem 1rem;
    cursor: pointer;
    border-left: 3px solid transparent;
  }
  .prompt-list li.selected {
    background: rgba(45, 108, 223, 0.1);
    border-left-color: var(--accent, #2d6cdf);
  }
  @media (prefers-color-scheme: dark) {
    .prompt-list li.selected {
      background: rgba(45, 108, 223, 0.18);
    }
  }
  .prompt-list li.empty {
    cursor: default;
    opacity: 0.55;
    font-style: italic;
  }
  .emoji {
    font-size: 1.15rem;
    width: 1.5rem;
    text-align: center;
  }
  .prompt-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .prompt-name {
    font-weight: 500;
    font-size: 0.98rem;
  }
  .prompt-desc {
    font-size: 0.82rem;
    opacity: 0.6;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 1rem;
    border-top: 1px solid rgba(0, 0, 0, 0.08);
    font-size: 0.78rem;
    opacity: 0.75;
    gap: 0.5rem;
  }
  @media (prefers-color-scheme: dark) {
    .footer {
      border-top-color: rgba(255, 255, 255, 0.08);
    }
  }
  .hint {
    flex: 1;
  }
  .provider {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    opacity: 0.6;
  }
  .header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.7rem 1rem;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
    font-size: 0.95rem;
  }
  @media (prefers-color-scheme: dark) {
    .header {
      border-bottom-color: rgba(255, 255, 255, 0.08);
    }
  }
  .header .status {
    margin-left: auto;
    font-size: 0.78rem;
    opacity: 0.6;
    text-transform: lowercase;
  }
  pre {
    margin: 0;
    padding: 1rem;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.9rem;
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
    overflow-y: auto;
  }
  .error {
    color: rgb(190, 50, 50);
    margin: 0 1rem 0.6rem;
    font-size: 0.88rem;
  }
  @media (prefers-color-scheme: dark) {
    .error {
      color: rgb(240, 110, 110);
    }
  }
  .error-state {
    padding: 2rem 1.25rem 0.75rem;
    align-items: center;
    text-align: center;
  }
  .error-state h3 {
    margin: 0 0 0.5rem;
    font-size: 1.1rem;
    font-weight: 600;
  }
  .error-state p {
    margin: 0 0 1rem;
  }
  .error-state .footer {
    border-top: none;
    width: 100%;
  }
</style>
