# Newt — Product Requirements & Technical RFC

> Project name: **Newt**.
> Status: Draft v0.2
> License: Apache 2.0
> Platforms (MVP): macOS 13+. Future: Windows, Linux.

---

## 1. Vision

Newt is a lightweight, system-wide AI rewriting tool for macOS. A user selects text in any app — a doc, a Slack message, an email, a code comment — triggers Newt via a global hotkey or right-click menu, picks a prompt (predefined or their own), and a popup streams an AI-rewritten version with a Replace button that drops it back in place.

It is open source, runs in the menu bar, uses minimal resources, and supports Bring-Your-Own-Key for major LLM providers. A hosted offering is layered on top of the same app for users who prefer pay-as-you-go without managing keys.

The product targets writers, engineers, support teams, students, and anyone who lives across many text surfaces and wants consistent, scriptable rewriting without copy-pasting into a chat window.

---

## 2. Goals

- **Universal**: works in any macOS app that supports standard copy/paste, with no per-app integrations.
- **Lightweight**: < 20 MB binary, < 50 MB RAM idle, no background CPU usage when not in use.
- **Fast**: time from hotkey to first streamed token under 1.5 s on a fast connection.
- **Private by default**: no telemetry without opt-in; selected text leaves the device only when a rewrite is invoked, and only to the user's chosen provider.
- **Extensible**: users can add, edit, and share prompts as plain files. Provider list grows without breaking changes.
- **Open source**: Apache 2.0, public roadmap, public anonymous usage stats.
- **Two-mode business model**: BYOK is free forever; hosted "Newt Cloud" is a metered paid tier sharing the same binary.

## 3. Non-goals (MVP)

- A full document editor or Grammarly-style inline underlines.
- Continuous background analysis of typing.
- Mobile or web app.
- Real-time collaboration.
- Custom-trained models.
- Workflow automation chains (output of one prompt feeding another).
- Per-prompt global hotkeys — there is exactly one hotkey, which opens the prompt picker.
- Rich context awareness (which app, surrounding paragraphs) — explicitly Phase 2.

---

## 4. Target users & key user stories

**Primary persona — "Maya, the cross-surface writer"**: PM who writes in Notion, Slack, Linear, Gmail, and Google Docs daily. Wants to tighten language, soften tone, or translate without breaking flow.

**Secondary persona — "Devon, the privacy-conscious engineer"**: prefers local models or their own OpenAI key, distrusts SaaS rewriters that ingest everything they type.

**User stories**

1. As a user, I select text anywhere, press ⌘+;, choose "Make it more concise", and see the rewrite in a popup.
2. As a user, I click Replace and the original selection is overwritten in place.
3. As a user, I right-click selected text and see "Rewrite with Newt" in the macOS Services menu.
4. As a user, I open Newt's settings and add a new prompt called "Rewrite as a Jira ticket" with my own instructions.
5. As a user, I bring my own OpenAI or Anthropic API key, stored securely in Keychain.
6. As a user, I can switch providers globally without restarting the app.
7. As a user, I can opt in to anonymous usage stats; until I do, nothing leaves my machine other than rewrite requests.

---

## 5. Product requirements

### 5.1 Triggering a rewrite

There are two — and only two — ways to trigger a rewrite. Both lead to the same prompt picker.

- **Global hotkey** (default ⌘+;, configurable). When pressed with text selected anywhere on the system, Newt captures the selection and shows the prompt picker.
- **macOS Services menu** entry "Rewrite with Newt…" appears in the right-click context menu and the Services submenu of every app's main menu.

There are no per-prompt direct hotkeys. The picker is fast enough — type a few characters, press Enter — that a single entry point keeps the model simple and the keyboard footprint small.

### 5.2 Selection capture

Newt captures selected text by simulating ⌘C, reading the pasteboard, and restoring the prior pasteboard contents. This works in essentially any app with standard Edit > Copy, including Electron and web-based apps where the macOS Accessibility API is unreliable. Requires Accessibility permission, requested once with a clear explanation screen on first launch.

### 5.3 Prompt picker UI

A small always-on-top window appears near the cursor:

- Search-as-you-type list of available prompts.
- Each prompt shows name, optional emoji, and short description.
- Selecting a prompt (Enter or click) immediately starts streaming the rewrite into the same window.
- Esc cancels and closes the window without modifying the source app.

### 5.4 Result UI

- Streaming output in a single readable text area. The original selection is not shown — it already lives in the source app, and a side-by-side view would slow down the interaction and double the popup's footprint.
- A compact header shows the active prompt name and the model in use. A small "tweak prompt" pencil affordance next to the prompt name lets the user adjust the prompt and rerun without leaving the popup — this is the escape hatch when a rewrite drifts from intent.
- Action buttons in the footer: **Replace**, **Copy**, **Regenerate**, **Cancel**.
- Replace re-focuses the previously active app and pastes the result over the original selection.
- Copy puts the result on the clipboard and leaves the source untouched.
- Regenerate runs the same prompt again on the same input.
- Token count and approximate cost are shown when the provider exposes them.

### 5.5 Prompts

- Stored as plain text files in the user's app support directory, one prompt per file.
- Each prompt file contains: name, description, optional emoji, optional model override, and the prompt instructions. The instructions are sent verbatim as the LLM's system message; the user's selection is sent as a separate user message — this structural split keeps adversarial content in the selection from being interpreted as instructions. Phase 9+ may add context substitutions (source app, surrounding paragraphs) that interpolate into the instructions; the user channel always carries only the selection.
- Newt ships with a curated default set: Improve writing, Fix grammar, Make concise, Make formal, Make casual, Summarize, Bullet points, Translate to English.
- Settings UI provides a list editor and a "Reveal in Finder" affordance for power users who want to edit prompts directly.
- Prompts can be exported and imported as a single archive for sharing.

### 5.6 LLM providers

MVP supports:

- **OpenAI** (chat completions, streaming).
- **Anthropic** (messages, streaming).

Phase 2 adds:

- **Local** via Ollama or any OpenAI-compatible local runtime.
- **OpenAI-compatible** generic endpoint (Groq, OpenRouter, LM Studio, vLLM, etc.).
- **Newt Cloud** (hosted offering — see §5.10).

API keys are stored in macOS Keychain, never in plain config files.

### 5.7 Settings

- General: launch at login, the single global hotkey, default provider, default model.
- Providers: API key per provider, default model per provider.
- Prompts: list, add, edit, reorder, delete.
- Privacy: telemetry opt-in toggle with sample payload preview.
- About: version, license, links to repo and docs.

### 5.8 Permissions and first-run experience

- On first launch, Newt presents a short onboarding: explains why Accessibility permission is needed, opens System Settings to the right pane, and verifies the permission was granted before continuing.
- API key entry is optional at onboarding; the app is usable without keys if the user is only browsing settings or wants to wait.
- No data leaves the device until the user explicitly invokes a rewrite or toggles telemetry on.

### 5.9 Telemetry

- **Off by default**, opt-in via a single toggle.
- When enabled, Newt sends:
  - A locally-generated random install UUID (no link to email, IP, or API key).
  - App version and macOS major version.
  - Daily count of rewrites and which provider was used.
  - Crash reports (sanitized stack traces only).
- Never sent: prompt names, prompt contents, selected text, rewritten text, model names beyond family (e.g. "gpt-4-class" not specific deployment).
- The settings panel shows a live preview of the next outbound payload.
- Aggregate stats (DAU, install count, OS distribution) are published on a public stats page on the project site.

### 5.10 Newt Cloud (hosted)

A second provider option in the same binary that proxies requests to a Newt-operated backend, billed per token via Stripe. No separate "pro" build. Out of scope for MVP but the provider abstraction must be designed to accommodate it without rework.

### 5.11 Distribution

- Notarized DMG signed with an Apple Developer ID, hosted on GitHub Releases.
- Homebrew cask submitted to `homebrew-cask`.
- Sparkle for in-app auto-updates.
- No Mac App Store at launch (sandbox restrictions conflict with the clipboard-based selection capture).

---

## 6. Tech stack

Chosen to satisfy the lightweight, macOS-first, future-Windows constraints while keeping the engine portable and testable.

- **Core engine and CLI**: Rust. Small binaries, no runtime, fast startup, strong async story for streaming, easy cross-compilation when Windows lands.
- **System layer (macOS)**: Rust with thin Objective-C / Swift bridges where required (global hotkey, Services menu, NSPasteboard, NSWorkspace active-app tracking, Accessibility permission).
- **UI layer**: Tauri, with a web-tech frontend (React or Svelte + TypeScript) for the menu bar app, prompt picker, result popup, and settings window. Tauri keeps the bundle around 10–15 MB and shares the same Rust core as the CLI.
- **IPC between UI and engine**: Tauri's built-in command bridge in-process; a local Unix socket for the CLI and any out-of-process tooling.
- **Secrets**: macOS Keychain via the `security-framework` crate.
- **Configuration and prompts**: plain files in the standard Application Support directory; format finalized in Phase 0 (see §10).
- **Auto-update**: Sparkle, wired through Tauri's updater.
- **Packaging**: notarized DMG built in CI (GitHub Actions), Homebrew cask for distribution.
- **Telemetry backend (Phase 7)**: self-hosted Plausible or PostHog.
- **Hosted backend (future Newt Cloud)**: Rust service reusing the same provider abstraction, Stripe metered billing.

The same Rust engine compiles for Windows and Linux when those phases arrive; only the system layer needs platform-specific implementations.

---

## 7. Architecture overview

Newt is structured as three logical components that can be tested in isolation:

1. **Core engine** — the headless brain. Owns prompts, providers, configuration, secrets, and the rewrite pipeline. Has no UI dependencies. Exposes a clean local API and a CLI front-end (see §8).
2. **System layer** — the macOS-specific glue: global hotkey registration, Services menu integration, selection capture via clipboard, active-app tracking, paste-back, Keychain access, Accessibility permission flow.
3. **UI layer** — the menu bar app, prompt picker window, result window, and settings window. Talks to the core engine over a local IPC channel (so the UI can be reloaded or replaced without restarting the daemon, and so the CLI shares the same code paths as the GUI).

Settings, prompts, and logs live under the standard macOS Application Support directory. Secrets live in Keychain. Nothing else is persisted.

---

## 8. Testability strategy (AI-validatable)

A core design constraint: every feature must be exercisable without a human driving the GUI, so an AI assistant can validate its own work end-to-end.

### 8.1 The `newt` CLI

The core engine ships with a first-class CLI that mirrors every meaningful product capability. The GUI is a thin client over the same engine. Anything the GUI can do, the CLI can do.

CLI surface (illustrative — not a full spec):

- `newt prompts list / show / add / edit / remove / import / export`
- `newt providers list / set-key / test`
- `newt rewrite --prompt <name> --input <file|->` → streams the rewrite to stdout.
- `newt rewrite --prompt <name> --text "..."` → one-shot rewrite.
- `newt config get / set` for any setting.
- `newt doctor` → checks permissions, provider connectivity, prompt validity, and prints a structured report.
- `newt simulate-selection --text "..."` → bypasses the macOS clipboard capture and feeds text directly into the rewrite pipeline as if it had been selected.
- `newt simulate-replace --text "..."` → exercises the paste-back path against a sandbox text field provided by the app, returning the resulting buffer state.

### 8.2 Headless mode for system features

System-layer features that normally require a GUI have headless equivalents:

- **Selection capture**: `simulate-selection` injects text into the same code path the clipboard trick feeds, so the rewrite pipeline can be tested without focusing another app.
- **Paste-back**: a hidden test target window inside Newt can receive paste-back, with its buffer readable via the CLI for assertions.
- **Hotkey**: a CLI command can trigger the same handler the global hotkey would.
- **Services menu**: a CLI command invokes the same Service handler.

### 8.3 Deterministic provider for tests

A built-in `mock` provider returns scripted responses. Tests use it to validate the full pipeline (prompt rendering, streaming, replace flow) without making real API calls or needing keys. Real providers are exercised in a separate, opt-in integration suite gated on environment variables.

### 8.4 Structured machine-readable output

Every CLI command supports `--json`. Rewrite streams emit newline-delimited JSON events (`token`, `done`, `error`, `usage`). This lets an AI agent script multi-step validations: invoke a rewrite, parse the stream, assert on token counts, then verify the replace path landed the right text in the test target.

### 8.5 Snapshot fixtures

Prompt templates, default config, and example rewrites have golden-file snapshots checked into the repo. CI fails on unintended diffs. An AI agent can regenerate snapshots with a single command and inspect the diff.

### 8.6 Smoke harness

A single `newt doctor --full` command runs an end-to-end self-test: loads each prompt, invokes the mock provider, exercises the simulate-selection and simulate-replace paths, and reports pass/fail per check in JSON. Intended as the canonical "is the app healthy" command for both humans and AI agents.

---

## 9. Phased milestones

Each phase ends in a locally-testable, demoable build. Phases are sequenced so an AI agent can validate the previous phase's CLI before the next phase is built on top of it.

### Phase 0 — Skeleton & CLI foundation

Goal: a runnable CLI with no LLM calls and no GUI. Establishes the engine, config, prompt store, and JSON output conventions.

Exit criteria:
- `newt prompts list` shows the bundled defaults.
- `newt config get/set` round-trips values.
- `newt doctor` passes with no providers configured.
- Snapshot tests exist for default prompts.

### Phase 1 — Mock rewrite pipeline

Goal: end-to-end rewrite flow against the mock provider, fully scriptable.

Exit criteria:
- `newt rewrite --prompt improve-writing --text "hello world"` streams a deterministic mock response as NDJSON.
- `simulate-selection` and `simulate-replace` work and round-trip through the engine.
- `newt doctor --full` runs a green end-to-end check using only the mock provider.

### Phase 2 — Real providers (BYOK)

Goal: real OpenAI and Anthropic calls, keys in Keychain.

Exit criteria:
- `newt providers set-key openai` stores in Keychain.
- `newt providers test openai` confirms connectivity.
- `newt rewrite` against a real provider works from the CLI, streams tokens, reports usage.
- Integration test suite runs against both providers when keys are present, skipped otherwise.

### Phase 3 — Menu bar app and settings UI

Goal: a usable GUI for everything the CLI already does. No system-wide hotkey or selection capture yet.

Exit criteria:
- Menu bar icon, Settings window, Prompts editor, Providers panel.
- A "Rewrite from clipboard" menu item exercises the full pipeline against the current clipboard.
- Settings changes made in the UI are visible via the CLI and vice versa.

### Phase 4 — Selection capture and result popup

Goal: the Grammarly-style core experience for any app, driven by the single global hotkey.

Exit criteria:
- Global hotkey (default ⌘+;) captures the current selection from any app, shows the prompt picker, and streams a result.
- Replace button pastes the result back over the original selection.
- Accessibility permission onboarding is in place.
- `simulate-selection` and `simulate-replace` continue to pass (regression coverage for the headless paths).

### Phase 5 — Services menu integration

Goal: native right-click integration as the second entry point.

Exit criteria:
- "Rewrite with Newt…" appears in the right-click context menu and Services submenu of arbitrary apps.
- Selecting the entry opens the same prompt picker used by the hotkey.

### Phase 6 — Distribution

Goal: a real release users can install.

Exit criteria:
- Signed and notarized DMG built in CI.
- Homebrew cask published.
- Sparkle auto-update channel working between two test versions.
- Public landing page with install instructions.

### Phase 7 — Telemetry (opt-in) and public stats

Goal: visibility into usage without compromising privacy.

Exit criteria:
- Telemetry toggle off by default, with payload preview in settings.
- Self-hosted analytics endpoint receives events when toggled on.
- Public `/stats` page renders aggregate numbers.

### Phase 8 — Local and OpenAI-compatible providers

Goal: privacy-first users can run fully offline; advanced users can plug in any compatible API.

Exit criteria:
- Ollama provider works against a local model.
- Generic OpenAI-compatible endpoint configurable via settings or CLI.

### Future / Phase 9+

- Newt Cloud hosted provider with metered billing.
- Windows port (cross-platform stack already chosen with this in mind).
- Context awareness: include surrounding paragraphs and source app name as additional template variables.
- Prompt sharing hub or registry.
- Linux support.

---

## 10. Risks and open questions

- **Sandbox vs. selection capture**: confirm the clipboard-based capture survives Apple's notarization and any future hardening. Fallback: AXSelectedText for compliant apps.
- **Active-app tracking for paste-back**: re-focusing the source app after the popup steals focus is fiddly across Spaces and full-screen apps. Needs early prototyping.
- **Hotkey conflicts**: ⌘+; is rarely used by mainstream apps (occasionally bound to spell-check in macOS text views), but verify on first launch that the chosen hotkey is actually free on the user's system; if not, surface a friendly "this shortcut is in use — pick another" prompt during onboarding instead of silently failing.
- **Streaming UI jank**: ensuring smooth token streaming in the popup without dropping frames. Validate during Phase 4.
- **Prompt format**: settle on YAML vs. TOML vs. plain text with frontmatter before Phase 0 ships, since prompts are user-editable and breaking changes are expensive.
- **Keychain UX on locked machines**: confirm behavior when Keychain is locked at launch.
- **Cost surprises with BYOK**: should Newt warn when a single rewrite is projected to exceed a configurable token budget?
- **Trademark/namespace check**: verify no active trademark on "Newt" in writing/AI software, and confirm Homebrew cask name `newt` and Apple Developer notarization name are available before launch.

---

## 11. Success metrics

Once telemetry is live (Phase 7), the project tracks:

- Weekly active users.
- Rewrites per active user per week.
- Provider mix (BYOK vs. local vs. cloud).
- Crash-free session rate.
- 30-day retention from install.
- GitHub stars and contributor count as ecosystem signals.

No content metrics are collected — quality is judged via user feedback channels (issues, discussions, surveys) rather than instrumentation.
