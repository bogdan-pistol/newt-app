//! macOS Keychain wrapper for provider API keys (PRD §5.6, §6).
//!
//! All keys live under one Keychain service identifier so they're easy to
//! find in Keychain Access.app: open it, search for `dev.newt.providers`,
//! and you'll see one entry per provider with that provider's name as the
//! account. Plain-text config files never touch keys.
//!
//! Missing-key is modelled as `Ok(None)` rather than an error — callers
//! routinely want to ask "do I have a key for this provider?" and a missing
//! item is the normal answer for first-run state.

use anyhow::{Context, Result};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

/// Keychain service identifier. Stable across app versions.
pub const SERVICE: &str = "dev.newt.providers";

/// Store an API key for `provider` (e.g. `"openai"`, `"anthropic"`),
/// overwriting any existing entry.
pub fn set_key(provider: &str, key: &str) -> Result<()> {
    set_generic_password(SERVICE, provider, key.as_bytes())
        .with_context(|| format!("storing key for `{provider}` in Keychain"))?;
    Ok(())
}

/// Read the stored API key for `provider`. Returns `Ok(None)` when no
/// entry exists, `Err` only on Keychain access failures (locked, denied).
pub fn get_key(provider: &str) -> Result<Option<String>> {
    match get_generic_password(SERVICE, provider) {
        Ok(bytes) => {
            let key = String::from_utf8(bytes)
                .with_context(|| format!("decoding key for `{provider}` as UTF-8"))?;
            Ok(Some(key))
        }
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading key for `{provider}` from Keychain")),
    }
}

/// Delete the stored API key for `provider`. Returns `true` if an entry
/// existed and was removed, `false` if there was nothing to remove.
pub fn delete_key(provider: &str) -> Result<bool> {
    match delete_generic_password(SERVICE, provider) {
        Ok(()) => Ok(true),
        Err(e) if is_not_found(&e) => Ok(false),
        Err(e) => Err(e).with_context(|| format!("deleting key for `{provider}` from Keychain")),
    }
}

/// Whether a Keychain error is "item not found" (errSecItemNotFound).
fn is_not_found(err: &security_framework::base::Error) -> bool {
    err.code() == -25300
}
