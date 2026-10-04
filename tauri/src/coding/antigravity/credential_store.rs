//! Antigravity CLI (`agy`) credential-store bridge.
//!
//! ## Why this module exists
//!
//! `agy` does NOT read `~/.gemini/antigravity-cli/oauth_creds.json`. Empirical
//! evidence from a real Windows install:
//!
//! * Moving `oauth_creds.json` away did not break `agy models` (still
//!   authenticated), so the file is not the runtime credential source.
//! * The credential store holds `gemini:antigravity` (Windows Credential
//!   Manager, `LegacyGeneric:target=gemini:antigravity`, `UserName`
//!   `antigravity`, `Persist` LocalMachine), and overwriting that blob with a
//!   bogus value makes `agy models` fail with `401 UNAUTHENTICATED`.
//!
//! So account switching only works by rewriting that credential. `agy` refreshes
//! it itself on the next run when the stored `token.expiry` is in the past, which
//! is why writing a snapshot and letting the CLI refresh is sufficient.
//!
//! ## Gotchas (do not "simplify" these away)
//!
//! * `keyring` v4's `Entry::new(service, user)` always builds the Windows target
//!   `service.user`, so it cannot address `gemini:antigravity`. We use
//!   `keyring-core` plus the platform store crates and pass the explicit
//!   `target` attribute `agy` itself relies on.
//! * The blob is written as raw bytes. `set_password()` would encode UTF-16LE,
//!   which `agy` cannot read; `set_secret()`/`get_secret()` are mandatory.
//! * The payload must keep `agy`'s own field shape (a nested `token` object plus
//!   `auth_method`) and must preserve `refresh_token`/`id_token` verbatim.
//! * `token.expiry` must stay RFC3339 (`+08:00` offset and `Z` are both fine).

use std::collections::HashMap;

/// Injectable boundary for account-switch round trips without touching real logins.
pub(super) trait CredentialStore: Send + Sync {
    fn read(&self) -> Result<Option<String>, String>;
    fn replace(&self, snapshot: Option<&str>) -> Result<(), String>;
}

pub(super) struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn read(&self) -> Result<Option<String>, String> {
        read_credential_text()
    }

    fn replace(&self, snapshot: Option<&str>) -> Result<(), String> {
        match snapshot {
            Some(snapshot) => write_credential_text(snapshot),
            None => delete_credential(),
        }
    }
}

/// Credential-store target/account name used by `agy`.
pub const ANTIGRAVITY_CREDENTIAL_TARGET: &str = "gemini:antigravity";
/// Service part of the macOS Keychain `service`/`account` pair.
pub const ANTIGRAVITY_CREDENTIAL_SERVICE: &str = "gemini";
/// Account part of the macOS Keychain `service`/`account` pair.
pub const ANTIGRAVITY_CREDENTIAL_ACCOUNT: &str = "antigravity";

/// Read the raw credential blob. `Ok(None)` means no credential is stored.
pub fn read_credential_blob() -> Result<Option<Vec<u8>>, String> {
    with_entry(|entry| match entry.get_secret() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(credential_error("read", &error)),
    })
}

/// Read the credential blob and decode it as UTF-8 text.
pub fn read_credential_text() -> Result<Option<String>, String> {
    let Some(bytes) = read_credential_blob()? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| format!("Antigravity credential is not valid UTF-8: {error}"))
}

/// Overwrite the credential blob with raw UTF-8 bytes.
///
/// Never use the credential store's password API here: it encodes UTF-16LE and
/// `agy` would then fail to parse its own credential.
pub fn write_credential_text(contents: &str) -> Result<(), String> {
    write_credential_blob(contents.as_bytes())
}

/// Overwrite the credential blob with arbitrary bytes.
pub fn write_credential_blob(secret: &[u8]) -> Result<(), String> {
    with_entry(|entry| {
        entry
            .set_secret(secret)
            .map_err(|error| credential_error("write", &error))
    })
}

/// Delete the credential, treating "already gone" as success.
pub fn delete_credential() -> Result<(), String> {
    with_entry(|entry| match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(credential_error("delete", &error)),
    })
}

/// Whether a credential currently exists in the store.
pub fn credential_exists() -> bool {
    matches!(read_credential_blob(), Ok(Some(_)))
}

fn credential_error(action: &str, error: &keyring_core::Error) -> String {
    format!(
        "Failed to {action} the Antigravity CLI credential \
         ('{ANTIGRAVITY_CREDENTIAL_TARGET}'): {error}"
    )
}

fn with_entry<T>(
    operation: impl FnOnce(&keyring_core::Entry) -> Result<T, String>,
) -> Result<T, String> {
    let entry = build_entry()?;
    operation(&entry)
}

/// Build an entry that resolves to the exact credential `agy` uses.
///
/// The store is created once per process; repeated calls only rebuild the
/// (cheap) entry wrapper.
fn build_entry() -> Result<keyring_core::Entry, String> {
    let (store, modifiers) = default_store()?;
    store
        .build(
            ANTIGRAVITY_CREDENTIAL_SERVICE,
            ANTIGRAVITY_CREDENTIAL_ACCOUNT,
            Some(&modifiers),
        )
        .map_err(|error| format!("Failed to open the Antigravity CLI credential entry: {error}"))
}

#[allow(clippy::type_complexity)]
fn default_store() -> Result<
    (
        std::sync::Arc<keyring_core::CredentialStore>,
        HashMap<&'static str, &'static str>,
    ),
    String,
> {
    static STORE: std::sync::OnceLock<
        Result<std::sync::Arc<keyring_core::CredentialStore>, String>,
    > = std::sync::OnceLock::new();
    static MODIFIERS: std::sync::OnceLock<HashMap<&'static str, &'static str>> =
        std::sync::OnceLock::new();

    let modifiers = MODIFIERS.get_or_init(platform_modifiers).clone();
    let store = STORE.get_or_init(create_store).clone()?;
    Ok((store, modifiers))
}

#[cfg(target_os = "windows")]
fn platform_modifiers() -> HashMap<&'static str, &'static str> {
    // Explicit target: overrides the store's default `user.service` composition
    // so we address the same generic credential `agy` writes.
    HashMap::from([("target", ANTIGRAVITY_CREDENTIAL_TARGET)])
}

#[cfg(not(target_os = "windows"))]
fn platform_modifiers() -> HashMap<&'static str, &'static str> {
    // Empty by design: the native keychain/Secret Service store maps the
    // service/account pair we pass to `build_entry` unchanged.
    HashMap::new()
}

#[cfg(target_os = "windows")]
fn create_store() -> Result<std::sync::Arc<keyring_core::CredentialStore>, String> {
    let store = windows_native_keyring_store::Store::new()
        .map_err(|error| format!("Failed to open the Windows credential store: {error}"))?;
    Ok(store)
}

#[cfg(target_os = "macos")]
fn create_store() -> Result<std::sync::Arc<keyring_core::CredentialStore>, String> {
    let store = apple_native_keyring_store::keychain::Store::new()
        .map_err(|error| format!("Failed to open the macOS Keychain: {error}"))?;
    Ok(store)
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn create_store() -> Result<std::sync::Arc<keyring_core::CredentialStore>, String> {
    let store = zbus_secret_service_keyring_store::Store::new()
        .map_err(|error| format!("Failed to open the Linux Secret Service store: {error}"))?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_names_match_agy_layout() {
        // The Windows target is a raw `service:user` pair, not keyring v4's
        // `service.user`; the macOS pair splits the same string.
        assert_eq!(ANTIGRAVITY_CREDENTIAL_TARGET, "gemini:antigravity");
        assert_eq!(ANTIGRAVITY_CREDENTIAL_SERVICE, "gemini");
        assert_eq!(ANTIGRAVITY_CREDENTIAL_ACCOUNT, "antigravity");
        assert!(!ANTIGRAVITY_CREDENTIAL_TARGET.contains('.'));
    }
}
