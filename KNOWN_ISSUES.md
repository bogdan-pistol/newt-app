# Known Issues

A running list of friction we've accepted (or punted on) so future work can find them. PR-friendly: each entry has the symptom, status, workaround, and why it isn't fixed yet.

## Gmail in Chrome — Replace appends instead of replacing

**Symptom:** Clicking **Replace** after a rewrite inserts the rewrite *after* the original selection in Gmail (and other Chrome web pages with `contenteditable` text), instead of replacing it. The original text stays.

**Status:** Known limitation, browser-side. Confirmed in Chrome and Firefox. **Does NOT affect** Safari, Slack, WhatsApp, Notes, Mail, TextEdit, or other native/Electron apps.

**Why:** Chrome clears text selection in `contenteditable` elements when focus shifts away from the tab. By the time Newt re-focuses Chrome and sends ⌘V, the cursor is at the end of where the selection used to be — so ⌘V inserts.

**Workaround:** Use the **Copy** button in the rewrite popup and paste manually. Or use Safari for Gmail (Safari preserves selection through focus changes, and Replace works there).

**Why we haven't fixed it:** We tried writing the rewrite directly via the Accessibility API (`AXSelectedText` write). The call returns `kAXErrorSuccess` spuriously in apps where it doesn't actually replace anything, which broke Slack / WhatsApp / native apps that the clipboard + ⌘V path was already handling correctly. The helper is kept in [src-tauri/src/macos.rs](src-tauri/src/macos.rs) as `try_replace_selection_via_ax` with `#[allow(dead_code)]`. To re-enable it cleanly we'd need a per-app or per-AX-role guard ("only attempt if focused element is an `AXTextField`/`AXTextArea`") — open question whether that filter would actually pick out the apps that work.

## Dev-mode Accessibility re-grant on every build

**Symptom:** After every `cargo tauri build --debug`, Newt's Accessibility permission appears revoked in System Settings → Privacy & Security → Accessibility. The toggle still shows on, but the underlying check returns false. Hotkey / Replace stop working until you flip the toggle off and on.

**Status:** Dev-build friction only. Goes away in Phase 6 with Developer ID notarization (stable code signature across releases).

**Why:** macOS ties Accessibility permission to the binary's code signature. Ad-hoc signing produces a different signature for every build (signature is deterministic from the binary's content hash). To macOS, each rebuild is a "new app."

**Workaround:** When iterating, skip rebuilds where possible. `cargo tauri dev` works without producing a new bundle, but it has its own focus-attribution caveat (macOS attributes Accessibility to the parent shell — VS Code's terminal, etc. — rather than Newt). For Phase 4 / 5 testing, the pattern is: `cargo tauri build --debug && open target/debug/bundle/macos/Newt.app`, then re-toggle in System Settings.

## Tempdir collision in test parallelism

**Symptom:** `cargo test --workspace` intermittently fails one of `replace_buffer_round_trip_via_engine_api` (in `tests/simulate_and_doctor.rs`) or 1–2 prompt CRUD tests (in `tests/prompt_crud.rs`). Roughly 1 in ~5-10 parallel runs on macOS. CI on develop has flaked once.

**Status:** Queued for a small follow-up PR. Pre-existing — predates Phase 5 / Phase 4 part 3.

**Why:** Both test files generate temp directory names from `SystemTime::now().as_nanos()`. When two tests in different binaries run on the same nanosecond clock tick, they pick the same path and collide.

**Fix:** Switch the nonce to either `Uuid::new_v4()` (adds the `uuid` crate as a dev-dep) or a per-test atomic counter combined with the existing nanosecond + PID. Either is ~20 LOC.

**Reliable repro:** `cargo test --workspace` repeatedly. `--test-threads=1` is a workaround that masks it.

## Per-prompt `model:` override is deprecated

**Symptom:** A `model:` field in a prompt's YAML frontmatter (e.g. `model: claude-sonnet-4-6`) is parsed but **ignored**. Every rewrite uses the active provider's configured model from Settings, not the prompt's per-file override.

**Status:** Deliberate, post-Phase 5 simplification. The `model` field stays in the parser so existing user prompts don't error, but the engine no longer reads it.

**Why:** Per-prompt overrides made multi-provider setups confusing — a model name from OpenAI is meaningless on Anthropic, and "where do I configure the model" became a guessing game (settings? prompt frontmatter? popup default?). Shipping one source of truth (Settings → Providers) is cleaner UX even if it loses some granularity.

**If you want model-per-task back:** the cleanest re-introduction is "presets" — named (provider, model) pairs that prompts can reference by name. Out of scope for now.

## Phase 4 part 3 popup history (closed)

We initially built the rewrite UI as a separate Tauri window with `transparent: true` + `decorations: false` + `alwaysOnTop: true`. The combination of focus-stealing rules, transparent-window quirks, and dual-window CSS scoping produced a long string of bugs (Esc not firing, "[mock] rewrite output" because of provider-selection logic, popup truncation near edges, "no selection captured" race, state persisting across invocations, etc.).

We backed out to **a single Tauri window with mode-switching**: the same window renders settings *or* rewrite UI based on a `mode` state in `App.svelte`. Hotkey / Services / tray "Rewrite Clipboard" all show the main window and emit an event that flips the frontend into rewrite mode. See [.claude/projects/-Users-bpistol-Personal-git-newt-app/memory/project_phase_4_popup_near_cursor.md](.claude/projects/-Users-bpistol-Personal-git-newt-app/memory/project_phase_4_popup_near_cursor.md) for the full architectural journey if you want the receipts.

A "true floating popup near cursor" (PRD §5.4) remains a long-term aspiration. It's blocked on either (a) a native NSPanel implementation via objc2 (heavy lift, ~300+ LOC), or (b) Phase 6 signing making focus-stealing rules friendlier for unsigned dev builds. Not a near-term priority — single-window mode-switching is reliable and the user shipped happy with it.
