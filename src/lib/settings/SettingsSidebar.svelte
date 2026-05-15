<script lang="ts">
  type Section = "general" | "providers" | "prompts";

  type Item = {
    id: Section;
    label: string;
    icon: string;
  };

  let {
    active = $bindable<Section>(),
    items,
  }: {
    active: Section;
    items: Item[];
  } = $props();
</script>

<nav class="sidebar" aria-label="Settings sections">
  {#each items as item (item.id)}
    <button
      type="button"
      class="nav-item"
      class:active={active === item.id}
      onclick={() => (active = item.id)}
      aria-current={active === item.id ? "page" : undefined}
    >
      <span class="icon" aria-hidden="true">{item.icon}</span>
      <span class="label">{item.label}</span>
    </button>
  {/each}
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.85rem 0.5rem;
    border-right: 1px solid rgba(0, 0, 0, 0.08);
    width: 168px;
    flex-shrink: 0;
    background: rgba(0, 0, 0, 0.02);
  }
  @media (prefers-color-scheme: dark) {
    .sidebar {
      border-right-color: rgba(255, 255, 255, 0.06);
      background: rgba(255, 255, 255, 0.02);
    }
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: inherit;
    text-align: left;
    font: inherit;
    cursor: pointer;
    width: 100%;
    transition: background-color 0.08s ease;
  }
  .nav-item:hover {
    background: rgba(0, 0, 0, 0.04);
  }
  .nav-item.active {
    background: rgba(45, 108, 223, 0.14);
    color: var(--accent, #2d6cdf);
  }
  @media (prefers-color-scheme: dark) {
    .nav-item:hover {
      background: rgba(255, 255, 255, 0.06);
    }
    .nav-item.active {
      background: rgba(45, 108, 223, 0.22);
      color: #6ea3f0;
    }
  }
  .icon {
    font-size: 1rem;
    width: 1.2rem;
    text-align: center;
    flex-shrink: 0;
  }
  .label {
    font-size: 0.92rem;
    font-weight: 500;
  }
</style>
