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
use core_foundation::string::{CFString, CFStringRef};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

#[allow(non_camel_case_types)]
type AXUIElementRef = *mut c_void;
#[allow(non_camel_case_types)]
type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;

unsafe extern "C" {
    /// `AXIsProcessTrustedWithOptions(NULL)` returns the current
    /// Accessibility-trust state without prompting. Passing a CFDictionary
    /// with `kAXTrustedCheckOptionPrompt` set to true causes macOS to
    /// register the calling app in the Accessibility list and show its
    /// native "App would like to control this computer" dialog (first
    /// time only). Declared inline rather than via `accessibility-sys` for
    /// one symbol — ApplicationServices is linked transitively via AppKit.
    safe fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;

    /// Returns the system-wide AX element — entry point for queries that
    /// resolve via the focused application's hierarchy.
    safe fn AXUIElementCreateSystemWide() -> AXUIElementRef;

    /// Read an attribute (e.g. `kAXFocusedUIElementAttribute`,
    /// `kAXSelectedTextAttribute`) on an AX element. `value` receives a
    /// retained reference; caller must release.
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut *mut c_void,
    ) -> AXError;

    /// Write an attribute on an AX element. Used here to set
    /// `kAXSelectedTextAttribute` directly, replacing the focused
    /// element's selected text with our rewrite — no keystroke
    /// simulation, no focus race, no clipboard manipulation.
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *const c_void,
    ) -> AXError;

    /// Decrement reference count on a CoreFoundation type. We need this
    /// for the AX element we receive from `AXUIElementCopyAttributeValue`.
    fn CFRelease(cf: *const c_void);
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

/// Activate Newt itself so that windows we show take *keyboard* focus,
/// not just visual focus. Without this, `WebviewWindow::set_focus()` can
/// visually-highlight the popup while keystrokes (Esc, ↑/↓, Enter) still
/// go to whichever app the user was in. Required for the popup picker
/// to actually receive its hotkeys when summoned from a different app.
pub fn activate_self() {
    let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
        return;
    };
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    // `activateIgnoringOtherApps:` is deprecated since macOS 14 (replaced
    // by no-arg `activate`), but remains the most reliable cross-version
    // API for unsigned/menu-bar apps that need to focus a window from
    // the background.
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
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

/// Synthesise a ⌘Z keystroke — used for the popup's "Undo" button, which
/// re-focuses the source app and asks it to undo the just-pasted rewrite.
/// Letting the source app's native undo stack handle it is cleaner than
/// trying to re-paste the original text, and it matches user muscle
/// memory (⌘Z still works).
pub fn simulate_cmd_z() -> Result<(), String> {
    simulate_meta_chord(Key::Unicode('z'))
}

/// Replace the focused element's selected text directly via the
/// Accessibility API, skipping the clipboard + ⌘V dance entirely.
///
/// **Currently unused.** We tried wiring this in front of the
/// clipboard+⌘V path to fix Gmail-in-Chrome (browsers clear text
/// selection on focus loss, so ⌘V inserts after the original instead
/// of replacing it). In practice the AX call returned success
/// spuriously in apps where it didn't actually replace anything —
/// breaking Slack / WhatsApp / native apps that the clipboard path was
/// already handling correctly. Kept here for future use behind a
/// per-app or per-role guard if we figure out how to detect "this AX
/// element actually accepts AXSelectedText writes."
#[allow(dead_code)]
pub fn try_replace_selection_via_ax(text: &str) -> bool {
    let sys_wide = AXUIElementCreateSystemWide();
    if sys_wide.is_null() {
        return false;
    }

    let focused_attr = CFString::new("AXFocusedUIElement");
    let mut focused: *mut c_void = std::ptr::null_mut();
    // SAFETY: sys_wide is a valid AX element; focused_attr is a valid
    // CFStringRef; focused is a writable out-pointer.
    let err = unsafe {
        AXUIElementCopyAttributeValue(sys_wide, focused_attr.as_concrete_TypeRef(), &mut focused)
    };

    let success = if err == K_AX_ERROR_SUCCESS && !focused.is_null() {
        let selected_attr = CFString::new("AXSelectedText");
        let cf_text = CFString::new(text);
        // SAFETY: focused is a non-null AX element retained by AX (we
        // CFRelease it below); selected_attr and cf_text are valid CF
        // strings owned by the local CFString wrappers (still alive).
        let err = unsafe {
            AXUIElementSetAttributeValue(
                focused as AXUIElementRef,
                selected_attr.as_concrete_TypeRef(),
                cf_text.as_concrete_TypeRef() as *const c_void,
            )
        };
        // SAFETY: AXUIElementCopyAttributeValue retains the result; we
        // own the reference until CFRelease.
        unsafe { CFRelease(focused) };
        err == K_AX_ERROR_SUCCESS
    } else {
        false
    };

    // SAFETY: AXUIElementCreateSystemWide returns a retained reference;
    // we own it until CFRelease.
    unsafe { CFRelease(sys_wide as *const c_void) };
    success
}

/// The cursor's position in macOS screen coordinates (origin: bottom-left
/// of the primary display, y grows upward). Returns `None` if AppKit is
/// unavailable for any reason.
///
/// Currently unused — Phase 4 part 3 originally used this for cursor-
/// anchored popup placement, but we switched to Spotlight-style centering
/// for steadier UX. Kept here in case we resurrect a "popup near cursor"
/// option later.
#[allow(dead_code)]
pub fn cursor_screen_position() -> Option<(f64, f64)> {
    let event_class = objc2::class!(NSEvent);
    // SAFETY: `+[NSEvent mouseLocation]` is a no-arg class method
    // returning `NSPoint` (a C struct of two CGFloats). Safe to call from
    // any thread on macOS, though we typically call this from the
    // global-shortcut handler (main thread).
    let location: objc2_foundation::NSPoint =
        unsafe { objc2::msg_send![event_class, mouseLocation] };
    Some((location.x, location.y))
}

/// Height of the primary screen in physical-equivalent points. Used to
/// flip macOS's bottom-left-origin cursor coordinates into Tauri's
/// top-left origin. Returns `None` if no main screen exists.
///
/// Currently unused — see `cursor_screen_position`.
#[allow(dead_code)]
pub fn primary_screen_height() -> Option<f64> {
    let screen = objc2_app_kit::NSScreen::mainScreen(unsafe {
        objc2_foundation::MainThreadMarker::new_unchecked()
    })?;
    let frame = screen.frame();
    Some(frame.size.height)
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
