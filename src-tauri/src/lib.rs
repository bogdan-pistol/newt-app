//! Tauri menu bar app for Newt.
//!
//! This is a thin shell over `newt-core`: every command here is a tiny
//! adapter that calls into the engine and serialises the result back to the
//! webview. The engine is the source of truth — settings made in the UI are
//! visible to the CLI immediately and vice versa, because both go through
//! the same `Paths`-rooted state.
//!
//! Phase 3 (read-only) wires `list_prompts` and `list_providers`. Editing
//! capabilities (set/remove keys, prompt CRUD) come in a follow-up.

use newt_core::{
    keychain,
    paths::Paths,
    prompt::{self, Prompt},
};
use serde::Serialize;
use tauri::{
    Manager, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

/// Provider summary for the settings UI. Matches the shape the CLI's
/// `providers list --json` emits, so the two surfaces stay aligned.
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

/// List the user's prompts (bundled defaults seeded on first call).
#[tauri::command]
fn list_prompts() -> Result<Vec<Prompt>, String> {
    let paths = Paths::from_env().map_err(|e| format!("{e:#}"))?;
    prompt::seed_defaults_if_empty(&paths).map_err(|e| format!("{e:#}"))?;
    prompt::list(&paths).map_err(|e| format!("{e:#}"))
}

/// List known providers and whether each has a key in Keychain.
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_prompts, list_providers])
        .setup(|app| {
            // Seed defaults at startup so the UI always sees a populated
            // prompts dir, mirroring what `newt-cli`'s `main()` does.
            if let Ok(paths) = Paths::from_env() {
                let _ = prompt::seed_defaults_if_empty(&paths);
            }

            // Tray menu: Show Settings · ─── · Quit.
            let show = MenuItem::with_id(app, "show", "Show Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Newt", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

            let icon = app
                .default_window_icon()
                .ok_or("missing default window icon")?
                .clone();

            TrayIconBuilder::with_id("newt-tray")
                .menu(&menu)
                .icon(icon)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

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
