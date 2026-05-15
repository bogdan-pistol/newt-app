<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import { api } from "./lib/api";
  import type { Prompt, ProviderStatus, SelectionPayload } from "./lib/types";
  import ProvidersSection from "./lib/ProvidersSection.svelte";
  import PromptsSection from "./lib/PromptsSection.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import PopupPicker from "./lib/PopupPicker.svelte";

  // Single window with two modes:
  //   "settings" — providers, prompts editor, onboarding (default).
  //   "rewrite"  — picker → streaming → auto-replace, full-window.
  // The Rust side emits events to switch us:
  //   `rewrite:selection`         → enter rewrite mode with captured text
  //   `rewrite:clipboard-trigger` → enter rewrite mode reading clipboard
  //   `app:show-settings`         → switch to settings mode (tray "Show Settings")
  type Mode = "settings" | "rewrite";

  let mode = $state<Mode>("settings");
  let selectionText = $state("");
  let selectionError = $state<string | null>(null);
  /**
   * Bumped on every entry into rewrite mode. Used as the `{#key}` for
   * PopupPicker so each new trigger fully remounts the component —
   * resetting its internal `stage`, `output`, `chosenPrompt`, etc.
   * Without this, a second ⌘+; press while the previous rewrite's
   * "done" state was still on screen would just show the old result.
   */
  let rewriteSeq = $state(0);

  let prompts = $state<Prompt[]>([]);
  let providers = $state<ProviderStatus[]>([]);
  let loadError = $state<string | null>(null);
  let loaded = $state(false);
  let accessibilityGranted = $state<boolean | null>(null);

  let unlistenSelection: UnlistenFn | null = null;
  let unlistenClipboard: UnlistenFn | null = null;
  let unlistenShowSettings: UnlistenFn | null = null;

  async function refresh() {
    loadError = null;
    try {
      const [p, pr] = await Promise.all([api.listPrompts(), api.listProviders()]);
      prompts = p;
      providers = pr;
    } catch (e) {
      loadError = String(e);
    } finally {
      loaded = true;
    }
  }

  async function enterRewriteMode(text: string) {
    selectionText = text;
    selectionError = null;
    rewriteSeq++;
    mode = "rewrite";
    await tick();
  }

  function exitRewriteMode() {
    mode = "settings";
    selectionText = "";
    selectionError = null;
    // Hide the window after dismiss — same close-to-tray behavior the
    // user already expects from the close button.
    getCurrentWindow().hide();
  }

  onMount(async () => {
    try {
      const status = await api.accessibilityStatus();
      accessibilityGranted = status.granted;
    } catch {
      accessibilityGranted = false;
    }
    await refresh();

    unlistenSelection = await api.onSelectionCaptured(
      async (payload: SelectionPayload) => {
        if (payload.error) {
          selectionError = payload.error;
          rewriteSeq++;
          mode = "rewrite";
          return;
        }
        if (payload.text && payload.text.trim()) {
          await enterRewriteMode(payload.text);
        }
      },
    );

    unlistenClipboard = await api.onClipboardTrigger(async () => {
      try {
        const text = await readText();
        if (text && text.trim()) {
          await enterRewriteMode(text);
        } else {
          selectionError = "Clipboard is empty.";
          rewriteSeq++;
          mode = "rewrite";
        }
      } catch (e) {
        selectionError = `Couldn't read clipboard: ${e}`;
        rewriteSeq++;
        mode = "rewrite";
      }
    });

    unlistenShowSettings = await listen("app:show-settings", () => {
      mode = "settings";
    });
  });

  onDestroy(() => {
    unlistenSelection?.();
    unlistenClipboard?.();
    unlistenShowSettings?.();
  });
</script>

{#if mode === "rewrite"}
  {#key rewriteSeq}
    <PopupPicker
      {prompts}
      {providers}
      text={selectionText}
      initialError={selectionError}
      onClose={exitRewriteMode}
    />
  {/key}
{:else}
  <main>
    <header>
      <h1>Newt</h1>
      <p class="subtitle">System-wide AI text rewriter</p>
    </header>

    {#if loadError}
      <div class="error" role="alert">
        <strong>Couldn't load:</strong> {loadError}
      </div>
    {/if}

    {#if accessibilityGranted === null}
      <div class="muted">Loading…</div>
    {:else if !accessibilityGranted}
      <Onboarding onGranted={() => (accessibilityGranted = true)} />
    {:else if !loaded && !loadError}
      <div class="muted">Loading…</div>
    {:else}
      <p class="hint top-hint">
        Press <kbd>⌘ ;</kbd> in any app, or right-click selected text → Services →
        <strong>Rewrite with Newt</strong>, to start a rewrite.
      </p>
      <ProvidersSection {providers} {refresh} />
      <hr />
      <PromptsSection {prompts} {refresh} />
    {/if}
  </main>
{/if}

<style>
  hr {
    border: none;
    border-top: 1px solid rgba(0, 0, 0, 0.08);
    margin: 1.75rem 0;
  }
  @media (prefers-color-scheme: dark) {
    hr {
      border-top-color: rgba(255, 255, 255, 0.08);
    }
  }
  kbd {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.82rem;
    padding: 0.05rem 0.35rem;
    border-radius: 4px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    background: rgba(0, 0, 0, 0.04);
  }
  @media (prefers-color-scheme: dark) {
    kbd {
      border-color: rgba(255, 255, 255, 0.15);
      background: rgba(255, 255, 255, 0.05);
    }
  }
  .top-hint {
    margin: 0 0 1.5rem;
    padding: 0.6rem 0.85rem;
    background: rgba(45, 108, 223, 0.05);
    border-left: 3px solid var(--accent, #2d6cdf);
    border-radius: 4px;
  }
  @media (prefers-color-scheme: dark) {
    .top-hint {
      background: rgba(45, 108, 223, 0.12);
    }
  }
</style>
