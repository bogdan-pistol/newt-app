//! macOS Services menu integration.
//!
//! Registers Newt as a Services provider so the system "Rewrite with Newt"
//! menu item (declared via `NSServices` in Info.plist) routes back into the
//! app. Selecting it invokes the `rewriteSelection:userData:error:`
//! selector on our provider object — the same flow the global ⌘+; hotkey
//! takes, but driven by the system's pasteboard handoff instead of a
//! synthesised ⌘C.
//!
//! Gated on `cfg(target_os = "macos")` at the `mod macos_services;`
//! declaration in `lib.rs`.

use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{MainThreadMarker, NSArray, NSString};
use tauri::{AppHandle, Emitter, Manager};

use crate::{CaptureState, SelectionPayload};

/// Tauri AppHandle stashed at startup so the ObjC service callback (which
/// has no access to `tauri::State`) can reach the app. Set once in
/// `lib.rs`'s `setup()`; read from the service handler.
pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

define_class!(
    // SAFETY:
    // - The superclass NSObject does not have any subclassing requirements.
    // - `NewtServiceProvider` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NewtServiceProvider"]
    pub struct NewtServiceProvider;

    impl NewtServiceProvider {
        /// macOS Services callback. Selector: `rewriteSelection:userData:error:`.
        /// Wired via the `NSMessage = "rewriteSelection"` key in Info.plist.
        ///
        /// SAFETY: Signature matches Apple's Services contract: the
        /// pasteboard and userData are non-null inputs we only read from,
        /// and we leave `error` as null (no synchronous return value).
        #[unsafe(method(rewriteSelection:userData:error:))]
        fn rewrite_selection(
            &self,
            pboard: &NSPasteboard,
            _user_data: *const NSString,
            _error: *mut *mut NSString,
        ) {
            // Pull text out of the pasteboard before doing anything async —
            // the system reuses this pasteboard slot once we return.
            let text = unsafe { pboard.stringForType(NSPasteboardTypeString) }
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            handle_rewrite(text);
        }
    }
);

impl NewtServiceProvider {
    /// Construct a fresh provider. Caller retains it via the
    /// `NSApplication.servicesProvider` strong reference; we additionally
    /// leak ours into a `static` so the registration lives for the entire
    /// app lifetime even if the registering scope drops the `Retained`.
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        // SAFETY: `init` on NSObject is a no-arg initialiser.
        unsafe { msg_send![alloc, init] }
    }
}

/// Forward the captured selection into the same flow the hotkey uses.
///
/// PID-capture caveat: by the time a Service selector fires, macOS has
/// *already* activated Newt — the receiving app comes forward before the
/// service callback runs. So `NSWorkspace.frontmostApplication` here
/// returns us, not the source app, and walking `runningApplications` for
/// `isActive()` peers fails too (only one app is active at a time).
/// Instead we read the source PID from `macos_frontmost`, which has been
/// tracking workspace activations since app launch.
fn handle_rewrite(text: Option<String>) {
    let Some(app) = APP_HANDLE.get() else {
        eprintln!("services: AppHandle not initialised yet, dropping invocation");
        return;
    };

    // Record the source PID so the user's "Replace" later returns there.
    if let Some(pid) = crate::macos_frontmost::most_recent_non_newt_pid()
        && let Some(state) = app.try_state::<CaptureState>()
        && let Ok(mut guard) = state.source_pid.lock()
    {
        *guard = Some(pid);
    }

    // Bring the popup picker forward (anchored near cursor) and emit the
    // selection payload. The frontend listens for `rewrite:selection` and,
    // when `text` is `Some`, runs the rewrite once a prompt is picked.
    crate::show_main_window(app);

    let payload = match text {
        Some(t) => SelectionPayload {
            text: Some(t),
            error: None,
        },
        None => SelectionPayload {
            text: None,
            error: Some("Services pasteboard was empty — no text to rewrite.".into()),
        },
    };
    let _ = app.emit("rewrite:selection", payload);
}

/// Install our provider on `NSApplication.sharedApplication` and tell the
/// Services machinery we accept plain-text sends. Must be called on the
/// main thread (Tauri's `setup` callback runs there).
pub fn register(mtm: MainThreadMarker) {
    let provider = NewtServiceProvider::new(mtm);

    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: provider conforms (informally) to the Services-handler
    // contract — the only selector the system invokes on it is the one
    // declared via `NSMessage` in Info.plist, which we implement above.
    // `setServicesProvider:` retains its argument, so the provider stays
    // alive for the lifetime of the shared NSApplication (the process).
    unsafe {
        app.setServicesProvider(Some(provider.as_ref() as &AnyObject));
    }

    // Hint to the Services menu: we accept plain-text sends, return
    // nothing synchronously (rewrite is async — user clicks Replace).
    let send_types = NSArray::from_slice(&[unsafe { NSPasteboardTypeString }]);
    let return_types: Retained<NSArray<NSString>> = NSArray::new();
    app.registerServicesMenuSendTypes_returnTypes(&send_types, &return_types);

    // `provider` drops here; NSApplication still holds a strong ref.
    drop(provider);
}
