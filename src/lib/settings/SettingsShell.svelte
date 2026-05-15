<script lang="ts">
  import SettingsSidebar from "./SettingsSidebar.svelte";
  import GeneralSection from "./GeneralSection.svelte";
  import ProvidersSection from "../ProvidersSection.svelte";
  import PromptsSection from "../PromptsSection.svelte";
  import type { Prompt, ProviderStatus } from "../types";

  let {
    prompts,
    providers,
    refresh,
  }: {
    prompts: Prompt[];
    providers: ProviderStatus[];
    refresh: () => Promise<void>;
  } = $props();

  type Section = "general" | "providers" | "prompts";
  let active = $state<Section>("general");

  const items = [
    { id: "general" as const, label: "General", icon: "⚙️" },
    { id: "providers" as const, label: "Providers", icon: "🔑" },
    { id: "prompts" as const, label: "Prompts", icon: "📝" },
  ];
</script>

<div class="shell">
  <SettingsSidebar bind:active {items} />
  <div class="content">
    {#if active === "general"}
      <GeneralSection />
    {:else if active === "providers"}
      <ProvidersSection {providers} {refresh} />
    {:else if active === "prompts"}
      <PromptsSection {prompts} {refresh} />
    {/if}
  </div>
</div>

<style>
  .shell {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem 1.5rem 1.75rem;
    min-width: 0;
  }
</style>
