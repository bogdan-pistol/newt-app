<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";
  import type {
    AppSettings,
    ProviderStatus,
    ProviderTestResult,
  } from "./types";
  import Modal from "./Modal.svelte";

  type Props = {
    providers: ProviderStatus[];
    refresh: () => Promise<void>;
  };

  let { providers, refresh }: Props = $props();

  let settings = $state<AppSettings | null>(null);

  // Per-provider draft for the "Add model" input.
  let addDraft = $state<Record<string, string>>({});

  let editing = $state<string | null>(null);
  let keyDraft = $state("");
  let saving = $state(false);
  let testingName = $state<string | null>(null);
  let testResult = $state<Record<string, string>>({});
  let confirmRemove = $state<string | null>(null);

  async function loadSettings() {
    settings = await api.getSettings();
  }

  onMount(loadSettings);

  function openEdit(name: string) {
    editing = name;
    keyDraft = "";
  }

  async function saveKey() {
    if (!editing || !keyDraft.trim()) return;
    const name = editing;
    saving = true;
    try {
      await api.setProviderKey(name, keyDraft);
      editing = null;
      keyDraft = "";
      testResult[name] = "";
      await refresh();
    } catch (e) {
      testResult[name] = `error: ${e}`;
    } finally {
      saving = false;
    }
  }

  async function removeKey(name: string) {
    try {
      await api.removeProviderKey(name);
      confirmRemove = null;
      await refresh();
    } catch (e) {
      testResult[name] = `error: ${e}`;
    }
  }

  async function testKey(name: string) {
    testingName = name;
    testResult[name] = "";
    try {
      const res: ProviderTestResult = await api.testProvider(name);
      testResult[name] = res.ok
        ? `✓ ok (${res.response_chars} chars)`
        : "✗ unexpected event sequence";
    } catch (e) {
      testResult[name] = `✗ ${e}`;
    } finally {
      testingName = null;
    }
  }

  async function makeActive(name: string) {
    await api.setActiveProvider(name);
    await loadSettings();
  }

  async function selectModel(provider: string, model: string) {
    if (!settings) return;
    if (settings.models[provider]?.current === model) return;
    await api.setProviderCurrentModel(provider, model);
    await loadSettings();
  }

  async function addModel(provider: string) {
    if (!settings) return;
    const candidate = (addDraft[provider] ?? "").trim();
    if (!candidate) return;
    const current = settings.models[provider]?.models ?? [];
    if (current.includes(candidate)) {
      // Already in list — just select it.
      addDraft[provider] = "";
      await selectModel(provider, candidate);
      return;
    }
    const next = [...current, candidate];
    await api.setProviderModels(provider, next);
    // Newly added → make it current. Matches Cursor / Raycast behavior.
    await api.setProviderCurrentModel(provider, candidate);
    addDraft[provider] = "";
    await loadSettings();
  }

  async function removeModel(provider: string, model: string) {
    if (!settings) return;
    const current = settings.models[provider]?.models ?? [];
    const next = current.filter((m) => m !== model);
    await api.setProviderModels(provider, next);
    // If we just removed the current, the backend's getter falls back to
    // first remaining (or the bundled default), so reload + display will
    // reflect that without an extra call.
    await loadSettings();
  }

  function isActive(name: string) {
    return settings?.active_provider === name;
  }

  function canActivate(p: ProviderStatus) {
    return !p.needs_key || p.key_set;
  }
</script>

<section class="page">
  <header class="page-header">
    <h2>Providers</h2>
    <p class="muted">
      Pick which provider runs your rewrites and which model to use. Keys
      live in macOS Keychain.
    </p>
  </header>

  <div class="provider-cards">
    {#each providers as p (p.name)}
      <div class="card" class:active={isActive(p.name)}>
        <div class="card-header">
          <div class="title">
            <span class="provider-name">{p.name}</span>
            <span
              class="status"
              class:ok={!p.needs_key || p.key_set}
              class:missing={p.needs_key && !p.key_set}
            >
              {#if !p.needs_key}
                no key needed
              {:else if p.key_set}
                key set
              {:else}
                no key
              {/if}
            </span>
          </div>
          {#if isActive(p.name)}
            <span class="active-badge">Active</span>
          {:else}
            <button
              class="secondary tight"
              onclick={() => makeActive(p.name)}
              disabled={!canActivate(p)}
              title={canActivate(p) ? "" : "Set an API key first"}
            >
              Make active
            </button>
          {/if}
        </div>

        <p class="description muted">{p.description}</p>

        {#if p.needs_key && settings?.models[p.name]}
          <div class="models-block">
            <div class="field-label">Models</div>
            <div class="chips">
              {#each settings.models[p.name].models as model (model)}
                {@const current = settings.models[p.name].current === model}
                <div class="chip" class:current>
                  <button
                    class="chip-pick"
                    type="button"
                    onclick={() => selectModel(p.name, model)}
                    title={current ? "Currently selected" : "Use this model"}
                  >
                    <span class="dot" aria-hidden="true"></span>
                    <span class="chip-label">{model}</span>
                  </button>
                  <button
                    class="chip-remove"
                    type="button"
                    aria-label="Remove model"
                    onclick={() => removeModel(p.name, model)}
                    title="Remove from list"
                  >
                    ×
                  </button>
                </div>
              {/each}
            </div>
            <form
              class="add-row"
              onsubmit={(e) => {
                e.preventDefault();
                addModel(p.name);
              }}
            >
              <input
                type="text"
                bind:value={addDraft[p.name]}
                placeholder={p.name === "openai"
                  ? "Add a model — e.g. gpt-4o"
                  : "Add a model — e.g. claude-sonnet-4-6"}
                spellcheck="false"
                autocomplete="off"
              />
              <button
                class="secondary tight"
                type="submit"
                disabled={!(addDraft[p.name] ?? "").trim()}
              >
                Add
              </button>
            </form>
          </div>

          <div class="actions">
            <button class="secondary tight" onclick={() => openEdit(p.name)}>
              {p.key_set ? "Replace key" : "Set key"}
            </button>
            {#if p.key_set}
              <button
                class="secondary tight"
                onclick={() => testKey(p.name)}
                disabled={testingName === p.name}
              >
                {testingName === p.name ? "Testing…" : "Test"}
              </button>
              <button
                class="danger tight"
                onclick={() => (confirmRemove = p.name)}
              >
                Remove key
              </button>
            {/if}
          </div>
          {#if testResult[p.name]}
            <div
              class="test-result"
              class:err={testResult[p.name].startsWith("✗") ||
                testResult[p.name].startsWith("error")}
            >
              {testResult[p.name]}
            </div>
          {/if}
        {/if}
      </div>
    {/each}
  </div>
</section>

<Modal
  open={editing !== null}
  title={editing ? `Set key for ${editing}` : ""}
  onClose={() => (editing = null)}
>
  {#snippet children()}
    <p class="muted small">
      Keys are stored in macOS Keychain (service
      <code>dev.newt.providers</code>). They never leave your machine except
      when sent to the corresponding provider.
    </p>
    <label class="key-input">
      <span>API key</span>
      <input
        type="password"
        bind:value={keyDraft}
        placeholder="sk-…"
        autocomplete="off"
        spellcheck="false"
      />
    </label>
  {/snippet}
  {#snippet footer()}
    <button class="secondary" onclick={() => (editing = null)}>Cancel</button>
    <button
      class="primary"
      onclick={saveKey}
      disabled={saving || !keyDraft.trim()}
    >
      {saving ? "Saving…" : "Save"}
    </button>
  {/snippet}
</Modal>

<Modal
  open={confirmRemove !== null}
  title="Remove API key?"
  onClose={() => (confirmRemove = null)}
>
  {#snippet children()}
    <p>
      Remove the stored key for <strong>{confirmRemove}</strong> from Keychain?
      You can add it back any time from this Settings panel.
    </p>
  {/snippet}
  {#snippet footer()}
    <button class="secondary" onclick={() => (confirmRemove = null)}>
      Cancel
    </button>
    <button
      class="danger"
      onclick={() => confirmRemove && removeKey(confirmRemove)}
    >
      Remove
    </button>
  {/snippet}
</Modal>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }
  .page-header h2 {
    margin: 0 0 0.2rem;
    font-size: 1.15rem;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0;
    opacity: 1;
  }
  .page-header .muted {
    margin: 0;
    font-size: 0.88rem;
  }

  .provider-cards {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .card {
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 10px;
    padding: 0.85rem 1rem 0.95rem;
    background: rgba(0, 0, 0, 0.015);
    transition:
      border-color 0.1s ease,
      background 0.1s ease;
  }
  .card.active {
    border-color: var(--accent, #2d6cdf);
    background: rgba(45, 108, 223, 0.05);
  }
  @media (prefers-color-scheme: dark) {
    .card {
      border-color: rgba(255, 255, 255, 0.08);
      background: rgba(255, 255, 255, 0.02);
    }
    .card.active {
      background: rgba(45, 108, 223, 0.12);
    }
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .provider-name {
    font-weight: 600;
    font-size: 1rem;
  }
  .description {
    margin: 0.2rem 0 0.6rem;
    font-size: 0.86rem;
  }
  .active-badge {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0.18rem 0.55rem;
    border-radius: 999px;
    background: var(--accent, #2d6cdf);
    color: white;
    font-weight: 600;
  }

  .models-block {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    margin-top: 0.5rem;
  }
  .field-label {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
    font-weight: 600;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    border-radius: 999px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    background: rgba(0, 0, 0, 0.02);
    overflow: hidden;
    transition: border-color 0.08s ease, background 0.08s ease;
  }
  .chip.current {
    border-color: var(--accent, #2d6cdf);
    background: rgba(45, 108, 223, 0.12);
  }
  .chip:hover {
    border-color: rgba(0, 0, 0, 0.3);
  }
  @media (prefers-color-scheme: dark) {
    .chip {
      border-color: rgba(255, 255, 255, 0.18);
      background: rgba(255, 255, 255, 0.04);
    }
    .chip.current {
      background: rgba(45, 108, 223, 0.22);
    }
    .chip:hover {
      border-color: rgba(255, 255, 255, 0.35);
    }
  }
  .chip-pick {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    border: none;
    background: transparent;
    padding: 0.25rem 0.55rem 0.25rem 0.65rem;
    color: inherit;
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .chip-pick:hover {
    background: rgba(0, 0, 0, 0.04);
  }
  @media (prefers-color-scheme: dark) {
    .chip-pick:hover {
      background: rgba(255, 255, 255, 0.06);
    }
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    border: 1.5px solid currentColor;
    opacity: 0.55;
    flex-shrink: 0;
  }
  .chip.current .dot {
    background: var(--accent, #2d6cdf);
    border-color: var(--accent, #2d6cdf);
    opacity: 1;
  }
  .chip-label {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.82rem;
  }
  .chip-remove {
    border: none;
    background: transparent;
    padding: 0 0.5rem;
    cursor: pointer;
    color: inherit;
    opacity: 0.5;
    font-size: 1.05rem;
    line-height: 1;
    align-self: stretch;
  }
  .chip-remove:hover {
    opacity: 1;
    background: rgba(220, 50, 50, 0.12);
    color: rgb(180, 50, 50);
  }
  @media (prefers-color-scheme: dark) {
    .chip-remove:hover {
      color: rgb(240, 110, 110);
    }
  }

  .add-row {
    display: flex;
    gap: 0.45rem;
    margin-top: 0.2rem;
  }
  .add-row input {
    flex: 1;
    min-width: 0;
  }

  .actions {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.85rem;
    flex-wrap: wrap;
  }
  .test-result {
    margin-top: 0.45rem;
    font-size: 0.8rem;
    opacity: 0.7;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
  .test-result.err {
    color: rgb(200, 60, 60);
  }
  .key-input {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .key-input span {
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
    font-weight: 600;
  }
  .small {
    font-size: 0.85rem;
  }
</style>
