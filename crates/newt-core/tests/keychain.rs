//! Keychain wrapper round-trip test.
//!
//! Skipped unless `NEWT_TEST_KEYCHAIN=1` is set. CI runners may have a
//! locked Keychain that prompts for unlock, which would hang headless tests;
//! this gate keeps the default `cargo test` run silent and offline.
//!
//! Uses a unique nonce-suffixed account name so the test can never clobber
//! a real API key, even if the user has Newt installed and configured.

use newt_core::keychain;

fn enabled() -> bool {
    std::env::var("NEWT_TEST_KEYCHAIN").as_deref() == Ok("1")
}

#[test]
fn round_trip_set_get_update_delete() {
    if !enabled() {
        eprintln!("skipping: set NEWT_TEST_KEYCHAIN=1 to run");
        return;
    }

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let account = format!("__newt_test_{nonce}");
    let value = "test-api-key-value";

    // Initially absent.
    assert_eq!(keychain::get_key(&account).unwrap(), None);

    // Set + read.
    keychain::set_key(&account, value).unwrap();
    assert_eq!(keychain::get_key(&account).unwrap().as_deref(), Some(value));

    // Set again (overwrite).
    keychain::set_key(&account, "updated").unwrap();
    assert_eq!(
        keychain::get_key(&account).unwrap().as_deref(),
        Some("updated")
    );

    // Delete: first call removes, second is a no-op (returns false).
    assert!(keychain::delete_key(&account).unwrap());
    assert!(!keychain::delete_key(&account).unwrap());
    assert_eq!(keychain::get_key(&account).unwrap(), None);
}
