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

use std::thread;

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
    AppHandle, Emitter, Manager, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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
        ])
        .setup(|app| {
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

/// Hotkey handler: bring window forward and trigger a clipboard rewrite,
/// matching the tray menu's "Rewrite Clipboard…" UX. The frontend reads
/// the current clipboard and runs the pipeline.
fn on_hotkey(app: &AppHandle) {
    trigger_clipboard_rewrite(app);
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
