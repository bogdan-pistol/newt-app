<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import { api } from "./lib/api";
  import type { Prompt, ProviderStatus, SelectionPayload } from "./lib/types";
  import Onboarding from "./lib/Onboarding.svelte";
  import PopupPicker from "./lib/PopupPicker.svelte";
  import SettingsShell from "./lib/settings/SettingsShell.svelte";

  // Single window with two modes:
  //   "settings" — sidebar nav (General · Providers · Prompts).
  //   "rewrite"  — picker → streaming → auto-replace, full-window.
  // The Rust side emits events to switch us:
  //   `rewrite:selection`         → enter rewrite mode with captured text
  //   `rewrite:clipboard-trigger` → enter rewrite mode reading clipboard
  //   `app:show-settings`         → switch to settings mode (tray "Show Settings")
  type Mode = "settings" | "rewrite";

  let mode = $state<Mode>("settings");
  let selectionText = $state("");
  let selectionError = $state<string | null>(null);
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
      text={selectionText}
      initialError={selectionError}
      onClose={exitRewriteMode}
    />
  {/key}
{:else if accessibilityGranted === null || (!loaded && !loadError)}
  <div class="centered muted">Loading…</div>
{:else if !accessibilityGranted}
  <Onboarding onGranted={() => (accessibilityGranted = true)} />
{:else if loadError}
  <div class="centered error" role="alert">
    <strong>Couldn't load:</strong>
    {loadError}
  </div>
{:else}
  <SettingsShell {prompts} {providers} {refresh} />
{/if}

<style>
  .centered {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    padding: 2rem;
    text-align: center;
  }
</style>
