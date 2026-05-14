<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  type Prompt = {
    id: string;
    name: string;
    description: string;
    emoji: string | null;
    model: string | null;
    instructions: string;
  };

  type ProviderStatus = {
    name: string;
    description: string;
    needs_key: boolean;
    key_set: boolean;
  };

  let prompts = $state<Prompt[]>([]);
  let providers = $state<ProviderStatus[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  async function refresh() {
    error = null;
    try {
      const [p, pr] = await Promise.all([
        invoke<Prompt[]>("list_prompts"),
        invoke<ProviderStatus[]>("list_providers"),
      ]);
      prompts = p;
      providers = pr;
    } catch (e) {
      error = String(e);
    } finally {
      loaded = true;
    }
  }

  $effect(() => {
    refresh();
  });
</script>

<main>
  <header>
    <h1>Newt</h1>
    <p class="subtitle">System-wide AI text rewriter</p>
  </header>

  {#if error}
    <div class="error" role="alert">
      <strong>Couldn't load settings:</strong> {error}
    </div>
  {/if}

  <section>
    <h2>Providers</h2>
    {#if !loaded && !error}
      <div class="muted">Loading…</div>
    {:else}
      <ul class="providers">
        {#each providers as p (p.name)}
          <li>
            <div class="row">
              <div class="primary">
                <span class="provider-name">{p.name}</span>
                <span class="status" class:ok={!p.needs_key || p.key_set} class:missing={p.needs_key && !p.key_set}>
                  {#if !p.needs_key}
                    no key needed
                  {:else if p.key_set}
                    key set
                  {:else}
                    no key
                  {/if}
                </span>
              </div>
              <div class="secondary">{p.description}</div>
            </div>
          </li>
        {/each}
      </ul>
      <p class="hint">
        Add or remove keys from the CLI:
        <code>newt providers set-key openai</code>.
        Editing UI lands in a follow-up.
      </p>
    {/if}
  </section>

  <section>
    <h2>Prompts</h2>
    {#if !loaded && !error}
      <div class="muted">Loading…</div>
    {:else if prompts.length === 0}
      <div class="muted">(no prompts)</div>
    {:else}
      <ul class="prompts">
        {#each prompts as p (p.id)}
          <li>
            <div class="row">
              <div class="primary">
                <span class="emoji">{p.emoji ?? "  "}</span>
                <span class="prompt-name">{p.name}</span>
                <code class="prompt-id">{p.id}</code>
              </div>
              <div class="secondary">{p.description}</div>
            </div>
          </li>
        {/each}
      </ul>
      <p class="hint">
        Reveal in Finder:
        <code>open ~/Library/Application\ Support/Newt/prompts</code>.
        In-app editing lands in a follow-up.
      </p>
    {/if}
  </section>
</main>
