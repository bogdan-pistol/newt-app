<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api } from "./api";

  type Props = {
    onGranted: () => void;
  };

  let { onGranted }: Props = $props();

  let pollHandle: ReturnType<typeof setInterval> | null = null;
  let opening = $state(false);
  let error = $state<string | null>(null);

  async function poll() {
    try {
      const status = await api.accessibilityStatus();
      if (status.granted) {
        if (pollHandle) clearInterval(pollHandle);
        pollHandle = null;
        onGranted();
      }
    } catch {
      // Treat as "not yet" — the next poll tick will retry.
    }
  }

  async function openSettings() {
    opening = true;
    error = null;
    try {
      await api.openAccessibilitySettings();
    } catch (e) {
      error = String(e);
    } finally {
      // Leave the button in "Opening…" state for a beat so the user knows
      // their click was received before macOS surfaces System Settings.
      setTimeout(() => (opening = false), 1500);
    }
  }

  onMount(() => {
    poll();
    pollHandle = setInterval(poll, 1500);
  });

  onDestroy(() => {
    if (pollHandle) clearInterval(pollHandle);
  });
</script>

<div class="onboarding">
  <h2>Grant Accessibility permission</h2>
  <p>
    Newt needs Accessibility permission to read text you've selected in
    other apps and to paste rewrites back. Without it, you'd be ⌘C-ing
    and ⌘V-ing every time.
  </p>
  <p class="muted small">
    Selected text only leaves your device when you actually trigger a
    rewrite — the same network hop as typing into a chat with your
    chosen provider. Permission is granted in macOS System Settings.
  </p>
  <div class="action">
    <button class="primary" onclick={openSettings} disabled={opening}>
      {opening ? "Opening Settings…" : "Open System Settings"}
    </button>
  </div>
  <p class="muted small">
    Flip <strong>Newt</strong> on under
    <strong>Privacy & Security → Accessibility</strong>. This screen
    refreshes automatically once permission is granted.
  </p>
  {#if error}
    <div class="error">{error}</div>
  {/if}
</div>

<style>
  .onboarding {
    max-width: 480px;
    margin: 3rem auto 0;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    text-align: center;
  }
  h2 {
    margin: 0 0 0.25rem;
    font-size: 1.25rem;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0;
    opacity: 1;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 0.85rem;
    line-height: 1.5;
  }
  .action {
    margin: 0.5rem 0 0.25rem;
  }
</style>
