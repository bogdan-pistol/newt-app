//! Tracks the most-recently-activated non-Newt application on macOS.
//!
//! ## Why
//!
//! macOS Services activate the receiving app *before* delivering the
//! Service selector. By the time `NewtServiceProvider::rewriteSelection:…`
//! fires, `NSWorkspace.frontmostApplication` already returns Newt — so we
//! can't use it to figure out which app the user invoked the Service from.
//!
//! Walking `runningApplications` for an `isActive()` peer doesn't help
//! either: macOS only marks one app active at a time (the frontmost one),
//! and that's now us.
//!
//! ## What
//!
//! We subscribe to `NSWorkspaceDidActivateApplicationNotification` at app
//! launch and remember the PID of every non-Newt app as it becomes
//! frontmost. The Services handler reads this stash to recover the source
//! app. Hotkey path doesn't need it (it captures `frontmostApplication`
//! *before* showing our window, while the source is still frontmost).
//!
//! Gated on `cfg(target_os = "macos")` at the `mod macos_frontmost;`
//! declaration in `lib.rs`.

use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSWorkspaceDidActivateApplicationNotification;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey};
use objc2_foundation::{MainThreadMarker, NSDictionary, NSNotification, NSString};

/// Latest-known PID of a non-Newt frontmost app. Written by the
/// observer's notification callback, read by the Services handler.
static MOST_RECENT_NON_NEWT_PID: Mutex<Option<i32>> = Mutex::new(None);

/// Set once `install()` has run, to make a second call a no-op. The
/// observer instance itself can't live in a `static` (it's
/// `MainThreadOnly`, hence neither `Send` nor `Sync`), so we leak it via
/// `mem::forget` instead — `NSNotificationCenter` doesn't retain its
/// observers, so we have to keep it alive ourselves for the process
/// lifetime.
static INSTALLED: OnceLock<()> = OnceLock::new();

define_class!(
    // SAFETY:
    // - The superclass NSObject does not have any subclassing requirements.
    // - `FrontmostObserver` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NewtFrontmostObserver"]
    pub struct FrontmostObserver;

    impl FrontmostObserver {
        /// Selector: `applicationDidActivate:`. Wired up via
        /// `NSNotificationCenter.addObserver:selector:name:object:` in
        /// `install()` for `NSWorkspaceDidActivateApplicationNotification`.
        ///
        /// SAFETY: The signature matches what the notification center
        /// invokes — a single `NSNotification *` argument and no return.
        #[unsafe(method(applicationDidActivate:))]
        fn application_did_activate(&self, notification: &NSNotification) {
            record_activation_from_notification(notification);
        }
    }
);

impl FrontmostObserver {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        // SAFETY: NSObject's `init` is a no-arg initialiser.
        unsafe { msg_send![alloc, init] }
    }
}

/// PID of the most recently activated app that isn't Newt. Returns `None`
/// only if no non-Newt app has ever been frontmost since `install()` ran
/// (rare — on real systems Newt is launched *from* some other app).
pub fn most_recent_non_newt_pid() -> Option<i32> {
    MOST_RECENT_NON_NEWT_PID.lock().ok().and_then(|g| *g)
}

/// Subscribe an observer to workspace activation notifications and seed
/// the initial state from whatever's frontmost right now. Must be called
/// on the main thread (Tauri's `setup` callback runs there). Idempotent —
/// a second call is a no-op.
pub fn install(mtm: MainThreadMarker) {
    if INSTALLED.get().is_some() {
        return;
    }

    let observer = FrontmostObserver::new(mtm);

    // Seed: whoever is frontmost when we install (probably the app that
    // launched us, or Finder) — we want to know about them in case the
    // user invokes a Service before any other activation happens.
    let workspace = NSWorkspace::sharedWorkspace();
    if let Some(front) = workspace.frontmostApplication() {
        store_pid_if_not_self(front.processIdentifier());
    }

    // Subscribe to workspace's notification center (NOT the default
    // center — workspace notifications are posted to its own).
    let center = workspace.notificationCenter();
    let observer_any: &AnyObject = &observer;
    // SAFETY: `observer_any` implements `applicationDidActivate:` (defined
    // above) with the right signature for an NSNotification handler. The
    // selector and notification name are valid statics. `object: nil`
    // means we want notifications from any sender.
    unsafe {
        center.addObserver_selector_name_object(
            observer_any,
            sel!(applicationDidActivate:),
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
        );
    }

    // `NSNotificationCenter` doesn't retain its observers, so we have to
    // keep `observer` alive ourselves until the process exits. Leaking a
    // single `Retained` at startup is the right tool here — there's no
    // teardown path (we want the observer running until the very end).
    std::mem::forget(observer);

    let _ = INSTALLED.set(());
}

/// Pull the `NSRunningApplication` out of the notification's userInfo
/// (key: `NSWorkspaceApplicationKey`) and stash its PID if it isn't ours.
fn record_activation_from_notification(notification: &NSNotification) {
    let Some(user_info) = notification.userInfo() else {
        return;
    };
    // The default `NSDictionary` is `<AnyObject, AnyObject>`. We know the
    // userInfo for this notification has `NSString` keys and an
    // `NSRunningApplication` under `NSWorkspaceApplicationKey`.
    // SAFETY: This is the documented contract for
    // NSWorkspaceDidActivateApplicationNotification's userInfo.
    let user_info: &NSDictionary<NSString, AnyObject> = unsafe { user_info.cast_unchecked() };
    // SAFETY: `NSWorkspaceApplicationKey` is a valid global NSString.
    let key: &NSString = unsafe { NSWorkspaceApplicationKey };
    let Some(value) = user_info.objectForKey(key) else {
        return;
    };
    let Ok(running) = value.downcast::<NSRunningApplication>() else {
        return;
    };
    store_pid_if_not_self(running.processIdentifier());
}

fn store_pid_if_not_self(pid: i32) {
    let our_pid = std::process::id() as i32;
    if pid == our_pid {
        return;
    }
    if let Ok(mut guard) = MOST_RECENT_NON_NEWT_PID.lock() {
        *guard = Some(pid);
    }
}
