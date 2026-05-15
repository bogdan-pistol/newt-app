<script lang="ts">
  // Hotkey is currently hardcoded in the Rust side. When PRD §5.7's
  // "configurable hotkey" lands, this becomes a real form control.
  const hotkey = "⌘ ;";
</script>

<section class="page">
  <header class="page-header">
    <h2>General</h2>
    <p class="muted">How Newt is summoned and where its files live.</p>
  </header>

  <div class="rows">
    <div class="row">
      <div class="row-label">Global hotkey</div>
      <div class="row-value">
        <kbd>{hotkey}</kbd>
        <span class="muted small">Configurable in a future update.</span>
      </div>
    </div>

    <div class="row">
      <div class="row-label">Right-click menu</div>
      <div class="row-value">
        <span>Services → Rewrite with Newt</span>
        <span class="muted small">
          Enable in System Settings → Keyboard → Keyboard Shortcuts → Services
          → Text if you don't see it.
        </span>
      </div>
    </div>

    <div class="row">
      <div class="row-label">Prompt files</div>
      <div class="row-value">
        <code>~/Library/Application Support/Newt/prompts/</code>
        <span class="muted small">
          Edit directly with any text editor; changes apply on next rewrite.
        </span>
      </div>
    </div>
  </div>
</section>

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
  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .row {
    display: grid;
    grid-template-columns: 140px 1fr;
    align-items: start;
    gap: 1rem;
    padding: 0.65rem 0;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
  }
  .row:first-child {
    border-top: none;
    padding-top: 0;
  }
  @media (prefers-color-scheme: dark) {
    .row {
      border-top-color: rgba(255, 255, 255, 0.06);
    }
  }
  .row-label {
    font-size: 0.88rem;
    font-weight: 500;
    opacity: 0.85;
    padding-top: 0.15rem;
  }
  .row-value {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.92rem;
    min-width: 0;
  }
  .row-value code {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.82rem;
    padding: 0.2rem 0.45rem;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.04);
    word-break: break-all;
  }
  @media (prefers-color-scheme: dark) {
    .row-value code {
      background: rgba(255, 255, 255, 0.06);
    }
  }
  kbd {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.85rem;
    padding: 0.1rem 0.45rem;
    border-radius: 4px;
    border: 1px solid rgba(0, 0, 0, 0.18);
    background: rgba(0, 0, 0, 0.04);
    align-self: flex-start;
  }
  @media (prefers-color-scheme: dark) {
    kbd {
      border-color: rgba(255, 255, 255, 0.18);
      background: rgba(255, 255, 255, 0.06);
    }
  }
  .small {
    font-size: 0.82rem;
    line-height: 1.45;
  }
</style>
