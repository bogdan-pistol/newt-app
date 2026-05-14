<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api } from "./lib/api";
  import type { Prompt, ProviderStatus } from "./lib/types";
  import RewritePanel from "./lib/RewritePanel.svelte";
  import ProvidersSection from "./lib/ProvidersSection.svelte";
  import PromptsSection from "./lib/PromptsSection.svelte";

  let prompts = $state<Prompt[]>([]);
  let providers = $state<ProviderStatus[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);
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
    await refresh();
    unlistenTray = await api.onTrayRewriteClipboard(() => {
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

  {#if !loaded && !error}
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
