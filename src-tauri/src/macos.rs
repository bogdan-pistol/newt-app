//! macOS-specific glue: Accessibility permission checks, frontmost-app
//! tracking, and synthesised ⌘C / ⌘V via CGEventPost.
//!
//! The whole module is gated on `cfg(target_os = "macos")` at the
//! `mod macos;` declaration in `lib.rs`, so non-macOS builds skip it
//! entirely. PRD §5.2, §5.4, §5.8.

use std::ffi::c_void;
use std::time::Duration;

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

unsafe extern "C" {
    /// `AXIsProcessTrustedWithOptions(NULL)` returns the current
    /// Accessibility-trust state without prompting. Passing a CFDictionary
    /// with `kAXTrustedCheckOptionPrompt` set to true causes macOS to
    /// register the calling app in the Accessibility list and show its
    /// native "App would like to control this computer" dialog (first
    /// time only). Declared inline rather than via `accessibility-sys` for
    /// one symbol — ApplicationServices is linked transitively via AppKit.
    safe fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

/// Whether Newt is trusted for Accessibility (needed for `CGEventPost` to
/// actually deliver synthesised keystrokes to other apps). Non-prompting.
pub fn is_accessibility_granted() -> bool {
    AXIsProcessTrustedWithOptions(std::ptr::null())
}

/// Like `is_accessibility_granted` but with the side effect of registering
/// Newt in System Settings → Privacy & Security → Accessibility (with the
/// toggle off, ready for the user to flip on). The first call also pops up
/// macOS's native "App would like to control this computer" dialog.
pub fn request_accessibility_and_check() -> bool {
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();
    let pairs: &[(CFString, CFBoolean)] = &[(key, value)];
    let options = CFDictionary::from_CFType_pairs(pairs);
    AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const c_void)
}

/// Open System Settings to Privacy & Security → Accessibility, where the
/// user can flip Newt's switch on. Works on macOS 13+.
pub fn open_accessibility_settings() -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()?;
    Ok(())
}

/// PID of the app that's currently frontmost. Caller saves this on hotkey
/// press *before* Newt's window steals focus, so Replace can restore it.
pub fn frontmost_app_pid() -> Option<i32> {
    let workspace = NSWorkspace::sharedWorkspace();
    let frontmost = workspace.frontmostApplication()?;
    Some(frontmost.processIdentifier())
}

/// Bring the app with the given PID back to the foreground. Returns
/// whether the activation request was accepted (not whether the app
/// actually became frontmost — macOS may refuse for various reasons).
pub fn activate_app_by_pid(pid: i32) -> bool {
    let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return false;
    };
    // Empty options = bring the app forward without forcing any
    // particular window-activation policy. The newer no-arg `activate()`
    // exists in the macOS 14+ ObjC API but isn't in this version of the
    // bindings yet; activateWithOptions is the cross-version path.
    app.activateWithOptions(NSApplicationActivationOptions::empty())
}

/// Synthesise a ⌘C keystroke. Requires Accessibility permission — without
/// it `CGEventPost` succeeds silently but the keystroke is not delivered.
pub fn simulate_cmd_c() -> Result<(), String> {
    simulate_meta_chord(Key::Unicode('c'))
}

/// Synthesise a ⌘V keystroke. Same Accessibility caveat as `simulate_cmd_c`.
pub fn simulate_cmd_v() -> Result<(), String> {
    simulate_meta_chord(Key::Unicode('v'))
}

/// Press Meta (⌘), tap `key`, release Meta. The small sleeps between
/// events give the OS time to process them as a chord rather than three
/// disconnected key events — without them, some apps (notably Slack and
/// Electron-based clients) drop the modifier mid-flight.
fn simulate_meta_chord(key: Key) -> Result<(), String> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("enigo init failed: {e}"))?;

    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|e| format!("press ⌘: {e}"))?;
    std::thread::sleep(Duration::from_millis(20));
    enigo
        .key(key, Direction::Click)
        .map_err(|e| format!("tap key: {e}"))?;
    std::thread::sleep(Duration::from_millis(20));
    enigo
        .key(Key::Meta, Direction::Release)
        .map_err(|e| format!("release ⌘: {e}"))?;
    Ok(())
}
