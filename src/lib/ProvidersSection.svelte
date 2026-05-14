<script lang="ts">
  import { api } from "./api";
  import type { ProviderStatus, ProviderTestResult } from "./types";
  import Modal from "./Modal.svelte";

  type Props = {
    providers: ProviderStatus[];
    refresh: () => Promise<void>;
  };

  let { providers, refresh }: Props = $props();

  let editing = $state<string | null>(null);
  let keyDraft = $state("");
  let saving = $state(false);
  let testingName = $state<string | null>(null);
  let testResult = $state<Record<string, string>>({});
  let confirmRemove = $state<string | null>(null);

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
</script>

<section>
  <h2>Providers</h2>
  <ul class="providers">
    {#each providers as p (p.name)}
      <li>
        <div class="row">
          <div class="primary">
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
          <div class="secondary">{p.description}</div>
          {#if p.needs_key}
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
                  {testingName === p.name ? "…" : "Test"}
                </button>
                <button
                  class="danger tight"
                  onclick={() => (confirmRemove = p.name)}
                >
                  Remove
                </button>
              {/if}
            </div>
            {#if testResult[p.name]}
              <div class="test-result" class:err={testResult[p.name].startsWith("✗") || testResult[p.name].startsWith("error")}>
                {testResult[p.name]}
              </div>
            {/if}
          {/if}
        </div>
      </li>
    {/each}
  </ul>
</section>

<Modal
  open={editing !== null}
  title={editing ? `Set key for ${editing}` : ""}
  onClose={() => (editing = null)}
>
  {#snippet children()}
    <p class="muted small">
      Keys are stored in macOS Keychain (service <code>dev.newt.providers</code>).
      They never leave your machine except when sent to the corresponding provider.
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
  .actions {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.5rem;
  }
  .test-result {
    margin-top: 0.4rem;
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
