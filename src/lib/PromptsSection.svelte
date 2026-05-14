<script lang="ts">
  import { api } from "./api";
  import type { Prompt, PromptInput } from "./types";
  import Modal from "./Modal.svelte";

  type Props = {
    prompts: Prompt[];
    refresh: () => Promise<void>;
  };

  let { prompts, refresh }: Props = $props();

  type EditorMode = { kind: "create" } | { kind: "edit"; id: string };

  let editor = $state<EditorMode | null>(null);
  let saving = $state(false);
  let formError = $state<string | null>(null);
  let confirmDelete = $state<Prompt | null>(null);

  // Form state
  let formId = $state("");
  let formName = $state("");
  let formDescription = $state("");
  let formEmoji = $state("");
  let formModel = $state("");
  let formInstructions = $state("");

  function openCreate() {
    editor = { kind: "create" };
    formId = "";
    formName = "";
    formDescription = "";
    formEmoji = "";
    formModel = "";
    formInstructions = "";
    formError = null;
  }

  function openEdit(p: Prompt) {
    editor = { kind: "edit", id: p.id };
    formId = p.id;
    formName = p.name;
    formDescription = p.description;
    formEmoji = p.emoji ?? "";
    formModel = p.model ?? "";
    formInstructions = p.instructions;
    formError = null;
  }

  function buildInput(): PromptInput {
    return {
      name: formName,
      description: formDescription,
      emoji: formEmoji.trim() ? formEmoji.trim() : null,
      model: formModel.trim() ? formModel.trim() : null,
      instructions: formInstructions,
    };
  }

  async function save() {
    if (!editor) return;
    if (!formName.trim() || !formInstructions.trim()) {
      formError = "Name and instructions are required.";
      return;
    }
    if (editor.kind === "create" && !formId.trim()) {
      formError = "Id is required.";
      return;
    }
    saving = true;
    formError = null;
    try {
      const input = buildInput();
      if (editor.kind === "create") {
        await api.createPrompt(formId.trim(), input);
      } else {
        await api.updatePrompt(editor.id, input);
      }
      editor = null;
      await refresh();
    } catch (e) {
      formError = String(e);
    } finally {
      saving = false;
    }
  }

  async function deletePrompt(id: string) {
    try {
      await api.deletePrompt(id);
      confirmDelete = null;
      await refresh();
    } catch (e) {
      formError = String(e);
    }
  }
</script>

<section>
  <div class="section-header">
    <h2>Prompts</h2>
    <button class="secondary tight" onclick={openCreate}>+ New prompt</button>
  </div>
  {#if prompts.length === 0}
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
            <div class="actions">
              <button class="secondary tight" onclick={() => openEdit(p)}>
                Edit
              </button>
              <button
                class="danger tight"
                onclick={() => (confirmDelete = p)}
              >
                Delete
              </button>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<Modal
  open={editor !== null}
  title={editor?.kind === "create" ? "New prompt" : `Edit ${editor?.kind === "edit" ? editor.id : ""}`}
  onClose={() => (editor = null)}
>
  {#snippet children()}
    <div class="form">
      {#if editor?.kind === "create"}
        <label>
          <span>Id</span>
          <input
            type="text"
            bind:value={formId}
            placeholder="my-custom-prompt"
            spellcheck="false"
            autocomplete="off"
          />
          <small class="muted">Lowercase letters, digits, hyphens. 2–64 chars.</small>
        </label>
      {/if}
      <div class="form-row">
        <label class="grow">
          <span>Name</span>
          <input type="text" bind:value={formName} placeholder="Improve writing" />
        </label>
        <label class="emoji-input">
          <span>Emoji</span>
          <input type="text" bind:value={formEmoji} maxlength="4" placeholder="✨" />
        </label>
      </div>
      <label>
        <span>Description</span>
        <input
          type="text"
          bind:value={formDescription}
          placeholder="Tighten and clarify prose without changing meaning"
        />
      </label>
      <label>
        <span>Model override (optional)</span>
        <input
          type="text"
          bind:value={formModel}
          placeholder="leave empty for provider default"
          spellcheck="false"
        />
      </label>
      <label>
        <span>Instructions</span>
        <textarea
          bind:value={formInstructions}
          rows="8"
          placeholder="Rewrite the user's text to be clearer…"
        ></textarea>
        <small class="muted">
          Pure instructions only. The user's selected text is sent as a
          separate user message — do not reference it here.
        </small>
      </label>
      {#if formError}
        <div class="error">{formError}</div>
      {/if}
    </div>
  {/snippet}
  {#snippet footer()}
    <button class="secondary" onclick={() => (editor = null)}>Cancel</button>
    <button class="primary" onclick={save} disabled={saving}>
      {saving ? "Saving…" : "Save"}
    </button>
  {/snippet}
</Modal>

<Modal
  open={confirmDelete !== null}
  title="Delete prompt?"
  onClose={() => (confirmDelete = null)}
>
  {#snippet children()}
    <p>
      Delete <strong>{confirmDelete?.name}</strong>
      <code>({confirmDelete?.id})</code>?
    </p>
    <p class="muted small">
      If this id matches a bundled default, the on-disk override is removed
      (the bundled default returns next time). User-created prompts are gone
      for good.
    </p>
  {/snippet}
  {#snippet footer()}
    <button class="secondary" onclick={() => (confirmDelete = null)}>
      Cancel
    </button>
    <button
      class="danger"
      onclick={() => confirmDelete && deletePrompt(confirmDelete.id)}
    >
      Delete
    </button>
  {/snippet}
</Modal>

<style>
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 0.75rem;
  }
  .section-header h2 {
    margin-bottom: 0;
  }
  .actions {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.5rem;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .form label > span {
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
    font-weight: 600;
  }
  .form-row {
    display: flex;
    gap: 0.6rem;
  }
  .grow {
    flex: 1;
  }
  .emoji-input {
    width: 5rem;
  }
  .small {
    font-size: 0.85rem;
  }
</style>
