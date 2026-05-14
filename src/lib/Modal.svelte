<script lang="ts">
  type Props = {
    open: boolean;
    title: string;
    onClose: () => void;
    children: import("svelte").Snippet;
    footer?: import("svelte").Snippet;
  };

  let { open, title, onClose, children, footer }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div
    class="overlay"
    role="presentation"
    onclick={onClose}
    onkeydown={(e) => e.key === "Enter" && onClose()}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <header>
        <h3>{title}</h3>
        <button class="close" onclick={onClose} aria-label="Close">×</button>
      </header>
      <div class="body">
        {@render children()}
      </div>
      {#if footer}
        <footer>{@render footer()}</footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 1rem;
  }
  .modal {
    background: var(--bg, #fff);
    color: var(--fg, #1a1a1a);
    border-radius: 10px;
    box-shadow:
      0 10px 30px rgba(0, 0, 0, 0.2),
      0 0 0 1px rgba(0, 0, 0, 0.06);
    max-width: 540px;
    width: 100%;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  @media (prefers-color-scheme: dark) {
    .modal {
      --bg: #232323;
      --fg: #e6e6e6;
      box-shadow:
        0 10px 30px rgba(0, 0, 0, 0.4),
        0 0 0 1px rgba(255, 255, 255, 0.06);
    }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.1rem;
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  }
  @media (prefers-color-scheme: dark) {
    header {
      border-bottom-color: rgba(255, 255, 255, 0.08);
    }
  }
  header h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
  }
  .close {
    background: none;
    border: none;
    font-size: 1.4rem;
    line-height: 1;
    cursor: pointer;
    color: inherit;
    opacity: 0.6;
    padding: 0 0.25rem;
  }
  .close:hover {
    opacity: 1;
  }
  .body {
    padding: 1rem 1.1rem;
    overflow-y: auto;
  }
  footer {
    padding: 0.75rem 1.1rem;
    border-top: 1px solid rgba(0, 0, 0, 0.08);
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  @media (prefers-color-scheme: dark) {
    footer {
      border-top-color: rgba(255, 255, 255, 0.08);
    }
  }
</style>
