<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api } from "./lib/api";
  import type { Prompt, ProviderStatus } from "./lib/types";
  import RewritePanel from "./lib/RewritePanel.svelte";
  import ProvidersSection from "./lib/ProvidersSection.svelte";
  import PromptsSection from "./lib/PromptsSection.svelte";
  import Onboarding from "./lib/Onboarding.svelte";

  let prompts = $state<Prompt[]>([]);
  let providers = $state<ProviderStatus[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);
  let accessibilityGranted = $state<boolean | null>(null);
  let rewritePanel = $state<{
    rewriteFromClipboard: () => Promise<void>;
  } | null>(null);

  async function refresh() {
    error = null;
    try {
      const [p, pr] = await Promise.all([api.listPrompts(), api.listProviders()]);
      prompts = p;
      providers = pr;
    } catch (e) {
      error = String(e);
    } finally {
      loaded = true;
    }
  }

  let unlistenTray: (() => void) | null = null;

  onMount(async () => {
    // Check Accessibility first — if not granted we render only the
    // Onboarding component, since selection-capture and Replace can't
    // function without it.
    try {
      const status = await api.accessibilityStatus();
      accessibilityGranted = status.granted;
    } catch {
      accessibilityGranted = false;
    }

    await refresh();
    unlistenTray = await api.onClipboardTrigger(() => {
      rewritePanel?.rewriteFromClipboard();
    });
  });

  onDestroy(() => {
    unlistenTray?.();
  });
</script>

<main>
  <header>
    <h1>Newt</h1>
    <p class="subtitle">System-wide AI text rewriter</p>
  </header>

  {#if error}
    <div class="error" role="alert">
      <strong>Couldn't load:</strong> {error}
    </div>
  {/if}

  {#if accessibilityGranted === null}
    <div class="muted">Loading…</div>
  {:else if !accessibilityGranted}
    <Onboarding onGranted={() => (accessibilityGranted = true)} />
  {:else if !loaded && !error}
    <div class="muted">Loading…</div>
  {:else}
    <RewritePanel bind:this={rewritePanel} {prompts} {providers} />
    <hr />
    <ProvidersSection {providers} {refresh} />
    <hr />
    <PromptsSection {prompts} {refresh} />
  {/if}
</main>

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
</style>
