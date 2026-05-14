//! Tauri menu bar app for Newt.
//!
//! Thin shell over `newt-core`: every command is a small adapter that calls
//! into the engine. The engine is the source of truth — settings made in
//! the UI are visible to the CLI immediately and vice versa, because both
//! go through the same `Paths`-rooted state and the same `keychain` module.
//!
//! Streaming rewrites use Tauri's event bus (`rewrite:event`). The command
//! kicks off a worker thread that emits one event per `RewriteEvent` and
//! returns immediately, so the JS side gets a real stream without blocking
//! the IPC channel.

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use newt_core::{
    keychain,
    paths::Paths,
    prompt::{self, Prompt, PromptInput},
    provider::{
        Provider, RewriteEvent, RewriteRequest, anthropic::AnthropicProvider, mock::MockProvider,
        openai::OpenAiProvider,
    },
    rewrite,
};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, State, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
mod macos;

/// Cross-thread state for the capture/replace round-trip. The hotkey
/// handler stashes the source app's PID before stealing focus; the
/// `replace_selection` command pulls it back to re-focus that app before
/// pasting.
#[derive(Default)]
struct CaptureState {
    source_pid: Mutex<Option<i32>>,
}

/// Provider summary for the settings UI. Mirrors the shape of `newt
/// providers list --json` so the CLI and GUI stay aligned.
#[derive(Serialize)]
struct ProviderStatus {
    name: &'static str,
    description: &'static str,
    needs_key: bool,
    key_set: bool,
}

const PROVIDERS: &[(&str, bool, &str)] = &[
    ("mock", false, "deterministic test provider"),
    ("openai", true, "OpenAI chat completions"),
    ("anthropic", true, "Anthropic messages"),
];

fn paths() -> Result<Paths, String> {
    Paths::from_env().map_err(|e| format!("{e:#}"))
}

// ────────────────────────────────────── prompts ──────────────────────────────

#[tauri::command]
fn list_prompts() -> Result<Vec<Prompt>, String> {
    let p = paths()?;
    prompt::seed_defaults_if_empty(&p).map_err(|e| format!("{e:#}"))?;
    prompt::list(&p).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn create_prompt(id: String, input: PromptInput) -> Result<(), String> {
    prompt::create(&paths()?, &id, &input).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn update_prompt(id: String, input: PromptInput) -> Result<(), String> {
    prompt::update(&paths()?, &id, &input).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn delete_prompt(id: String) -> Result<bool, String> {
    prompt::delete(&paths()?, &id).map_err(|e| format!("{e:#}"))
}

// ────────────────────────────────────── providers ────────────────────────────

#[tauri::command]
fn list_providers() -> Result<Vec<ProviderStatus>, String> {
    let mut out = Vec::with_capacity(PROVIDERS.len());
    for (name, needs_key, description) in PROVIDERS {
        let key_set = if *needs_key {
            keychain::get_key(name)
                .map_err(|e| format!("{e:#}"))?
                .is_some()
        } else {
            true
        };
        out.push(ProviderStatus {
            name,
            description,
            needs_key: *needs_key,
            key_set,
        });
    }
    Ok(out)
}

#[tauri::command]
fn set_provider_key(provider: String, key: String) -> Result<(), String> {
    require_real_provider(&provider)?;
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("key is empty".into());
    }
    keychain::set_key(&provider, trimmed).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn remove_provider_key(provider: String) -> Result<bool, String> {
    require_real_provider(&provider)?;
    keychain::delete_key(&provider).map_err(|e| format!("{e:#}"))
}

#[derive(Serialize)]
struct ProviderTestResult {
    ok: bool,
    response_chars: usize,
}

/// Tiny live rewrite to verify connectivity (≈$0.0001 in tokens).
#[tauri::command]
fn test_provider(provider: String) -> Result<ProviderTestResult, String> {
    require_real_provider(&provider)?;
    let prov = build_real_provider(&provider)?;
    let request = RewriteRequest {
        system: Some("Respond with exactly the word 'ok' and nothing else.".to_string()),
        user: "ping".to_string(),
        model: None,
    };
    let mut got_token = false;
    let mut got_done = false;
    let mut chars = 0usize;
    prov.rewrite(&request, &mut |event| match event {
        RewriteEvent::Token { text } => {
            got_token = true;
            chars += text.len();
        }
        RewriteEvent::Done => got_done = true,
        RewriteEvent::Usage { .. } => {}
    })
    .map_err(|e| format!("{e:#}"))?;

    if !(got_token && got_done) {
        return Err(format!(
            "{provider}: missing expected events (token={got_token} done={got_done})"
        ));
    }
    Ok(ProviderTestResult {
        ok: true,
        response_chars: chars,
    })
}

// ────────────────────────────────────── rewrite (streamed) ───────────────────

/// Tagged frontend payload — `RewriteEvent` plus a synthesized `error`
/// variant that the engine emits as `Result::Err`. Frontend listens for one
/// event channel `rewrite:event` and switches on `type`.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FrontendEvent {
    Token {
        text: String,
    },
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    Done,
    Error {
        message: String,
    },
}

impl From<RewriteEvent> for FrontendEvent {
    fn from(value: RewriteEvent) -> Self {
        match value {
            RewriteEvent::Token { text } => FrontendEvent::Token { text },
            RewriteEvent::Usage {
                prompt_tokens,
                completion_tokens,
            } => FrontendEvent::Usage {
                prompt_tokens,
                completion_tokens,
            },
            RewriteEvent::Done => FrontendEvent::Done,
        }
    }
}

/// Run a rewrite and stream events back via `rewrite:event`. Returns
/// immediately; the worker thread emits events as they arrive and emits
/// either `Done` or `Error` exactly once at the end.
#[tauri::command]
fn run_rewrite(
    app: AppHandle,
    provider_name: String,
    prompt_id: String,
    selection: String,
) -> Result<(), String> {
    let p = paths()?;
    let provider = build_provider(&provider_name)?;

    thread::spawn(move || {
        let result = rewrite::run(
            &p,
            &prompt_id,
            &selection,
            provider.as_ref(),
            &mut |event| {
                let _ = app.emit("rewrite:event", FrontendEvent::from(event));
            },
        );
        if let Err(e) = result {
            let _ = app.emit(
                "rewrite:event",
                FrontendEvent::Error {
                    message: format!("{e:#}"),
                },
            );
        }
    });

    Ok(())
}

// ────────────────────────────────────── helpers ──────────────────────────────

fn require_real_provider(name: &str) -> Result<(), String> {
    match name {
        "openai" | "anthropic" => Ok(()),
        other => Err(format!("not a key-bearing provider: `{other}`")),
    }
}

fn build_provider(name: &str) -> Result<Box<dyn Provider>, String> {
    match name {
        "mock" => Ok(Box::new(MockProvider::echo("[mock] rewrite output"))),
        "openai" | "anthropic" => build_real_provider(name),
        other => Err(format!("unknown provider: `{other}`")),
    }
}

fn build_real_provider(name: &str) -> Result<Box<dyn Provider>, String> {
    let key = keychain::get_key(name)
        .map_err(|e| format!("{e:#}"))?
        .ok_or_else(|| format!("no API key for `{name}` — set one in Settings → Providers"))?;
    Ok(match name {
        "openai" => Box::new(OpenAiProvider::new(key)),
        "anthropic" => Box::new(AnthropicProvider::new(key)),
        other => return Err(format!("not a real provider: `{other}`")),
    })
}

// ────────────────────────────────────── selection capture & replace ─────────

/// Payload for the `rewrite:selection` event the hotkey handler emits
/// after attempting selection capture. `text = Some(...)` means capture
/// succeeded; `text = None` plus an `error` means it failed (typically
/// "Accessibility permission required" on first run).
#[derive(Clone, Serialize)]
struct SelectionPayload {
    text: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct AccessibilityStatus {
    granted: bool,
}

#[tauri::command]
fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        granted: accessibility_granted(),
    }
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    // Trigger macOS's permission prompt first — the *first* call registers
    // Newt in System Settings → Privacy & Security → Accessibility and
    // shows the native "App would like to control this computer" dialog.
    // Subsequent calls are silent. We then `open` System Settings as a
    // fallback in case the user dismissed the dialog without granting.
    #[cfg(target_os = "macos")]
    {
        let _ = macos::request_accessibility_and_check();
    }
    open_ax_settings_panel().map_err(|e| format!("{e:#}"))
}

/// Replace the user's original selection by re-focusing the source app
/// and synthesising ⌘V with `text` on the clipboard. Restores the prior
/// clipboard contents afterwards. Returns `Err` if Accessibility isn't
/// granted, the source app PID isn't known, or any step fails.
#[tauri::command]
fn replace_selection(
    app: AppHandle,
    state: State<'_, CaptureState>,
    text: String,
) -> Result<(), String> {
    if !accessibility_granted() {
        return Err("Accessibility permission required to paste back".into());
    }
    let pid = state
        .source_pid
        .lock()
        .map_err(|e| format!("state poisoned: {e}"))?
        .ok_or_else(|| "no source app recorded — capture a selection first".to_string())?;

    // Save the user's current clipboard so we can restore it after the paste.
    // If reading fails (non-text contents), the paste will inevitably clobber
    // the clipboard — that's an unavoidable limitation of the ⌘V trick.
    let prior_clipboard = app.clipboard().read_text().ok();

    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("setting clipboard: {e}"))?;

    if !activate_app(pid) {
        return Err(format!("could not re-activate source app (pid={pid})"));
    }
    // Give the OS a moment to actually shift focus before we send ⌘V.
    std::thread::sleep(Duration::from_millis(80));

    simulate_paste().map_err(|e| format!("simulating ⌘V: {e}"))?;

    // Let the paste land before we restore.
    std::thread::sleep(Duration::from_millis(120));

    if let Some(prior) = prior_clipboard {
        let _ = app.clipboard().write_text(prior);
    }
    Ok(())
}

// Tiny helpers that route to the macOS-specific implementation when
// available, returning sensible fallbacks otherwise. Keeps the command
// bodies readable and lets non-macOS builds still compile cleanly.

fn accessibility_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::is_accessibility_granted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

fn open_ax_settings_panel() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::open_accessibility_settings()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

fn activate_app(pid: i32) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::activate_app_by_pid(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        false
    }
}

fn simulate_paste() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::simulate_cmd_v()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("paste-back is macOS-only".into())
    }
}

// ────────────────────────────────────── app entry ────────────────────────────

/// Default global hotkey — ⌘+; (Cmd + Semicolon). PRD §5.1, §10.
///
/// Carbon's `RegisterEventHotKey` (used by `tauri-plugin-global-shortcut`
/// on macOS) does not require Accessibility permission — that's only
/// needed later for simulating keystrokes (CGEventPost) when we wire real
/// selection capture and paste-back. So this hotkey ships standalone.
fn default_hotkey() -> Shortcut {
    Shortcut::new(Some(Modifiers::SUPER), Code::Semicolon)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // Fire only on key-press (not release) so a single tap
                    // doesn't trigger twice.
                    if event.state == ShortcutState::Pressed && shortcut == &default_hotkey() {
                        on_hotkey(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            list_prompts,
            create_prompt,
            update_prompt,
            delete_prompt,
            list_providers,
            set_provider_key,
            remove_provider_key,
            test_provider,
            run_rewrite,
            accessibility_status,
            open_accessibility_settings,
            replace_selection,
        ])
        .setup(|app| {
            app.manage(CaptureState::default());

            // Seed defaults at startup so the UI always sees a populated
            // prompts dir, mirroring what `newt-cli`'s `main()` does.
            if let Ok(p) = Paths::from_env() {
                let _ = prompt::seed_defaults_if_empty(&p);
            }

            // Tray menu: Show · Rewrite Clipboard · ─── · Quit.
            let show = MenuItem::with_id(app, "show", "Show Settings", true, None::<&str>)?;
            let rewrite_clipboard = MenuItem::with_id(
                app,
                "rewrite_clipboard",
                "Rewrite Clipboard… (⌘;)",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Quit Newt", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&show, &rewrite_clipboard, &separator, &quit])?;

            let icon = app
                .default_window_icon()
                .ok_or("missing default window icon")?
                .clone();

            TrayIconBuilder::with_id("newt-tray")
                .menu(&menu)
                .icon(icon)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "rewrite_clipboard" => trigger_clipboard_rewrite(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Register the global hotkey. PRD §10 calls out conflict
            // handling ("⌘+; is occasionally bound to spell-check in
            // macOS text views") — for now we log register failures
            // rather than crash. Friendly remap UX comes when the
            // settings panel grows a hotkey picker.
            if let Err(e) = app.global_shortcut().register(default_hotkey()) {
                eprintln!("could not register hotkey ⌘+;: {e}");
            }

            Ok(())
        })
        // Close-to-tray: hide the window instead of quitting the app when
        // the user clicks the close button. The app remains in the menu bar.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Hotkey handler: capture the current selection (the ⌘C trick from
/// PRD §5.2), bring Newt's window forward, and emit `rewrite:selection`
/// with the captured text. The frontend listens, runs the pipeline, and
/// later calls `replace_selection` to paste the rewrite back.
fn on_hotkey(app: &AppHandle) {
    if !accessibility_granted() {
        // Surface to the user via the window so they can grant permission.
        show_main_window(app);
        let _ = app.emit(
            "rewrite:selection",
            SelectionPayload {
                text: None,
                error: Some(
                    "Accessibility permission needed for selection capture. Open Settings → Privacy & Security → Accessibility and toggle Newt on."
                        .into(),
                ),
            },
        );
        return;
    }

    // Order matters: capture *before* stealing focus.

    // 1. Remember which app the user is in so Replace can return there.
    let pid = frontmost_pid();
    if let Some(state) = app.try_state::<CaptureState>()
        && let Ok(mut guard) = state.source_pid.lock()
    {
        *guard = pid;
    }

    // 2. Save the user's current clipboard so the trick is non-destructive.
    let prior_clipboard = app.clipboard().read_text().ok();

    // 3. Send ⌘C to the still-frontmost source app.
    if let Err(e) = simulate_copy() {
        show_main_window(app);
        let _ = app.emit(
            "rewrite:selection",
            SelectionPayload {
                text: None,
                error: Some(format!("could not simulate ⌘C: {e}")),
            },
        );
        return;
    }

    // 4. Wait briefly — the OS needs time to flip the pasteboard.
    std::thread::sleep(Duration::from_millis(80));

    // 5. Read the now-updated clipboard. Empty string is treated as
    //    "no selection" (better UX than an error).
    let captured = app.clipboard().read_text().ok().filter(|s| !s.is_empty());

    // 6. Restore prior clipboard.
    if let Some(prior) = prior_clipboard {
        let _ = app.clipboard().write_text(prior);
    }

    // 7. Now we can show our window and emit the captured text.
    show_main_window(app);
    let _ = app.emit(
        "rewrite:selection",
        SelectionPayload {
            text: captured,
            error: None,
        },
    );
}

fn frontmost_pid() -> Option<i32> {
    #[cfg(target_os = "macos")]
    {
        macos::frontmost_app_pid()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn simulate_copy() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::simulate_cmd_c()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("selection capture is macOS-only".into())
    }
}

fn trigger_clipboard_rewrite(app: &AppHandle) {
    show_main_window(app);
    let _ = app.emit("rewrite:clipboard-trigger", ());
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
