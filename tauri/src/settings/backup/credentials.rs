//! System credential store access for the optional backup encryption password.
//!
//! The password never enters AppSettings, backups, logs, or the frontend. Only the
//! on/off flag lives in settings; the secret itself is stored under a fixed service +
//! account in the OS credential store (Windows Credential Manager, macOS Keychain,
//! Linux Secret Service). Reads/writes are blocking OS calls, so async callers wrap
//! them in `spawn_blocking`.

use keyring::Entry;

const SERVICE: &str = "AI Toolbox";
const ACCOUNT: &str = "backup-encryption";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| {
        backup_error(
            "CREDENTIAL_STORE",
            "settings.backupSettings.encryption.errors.credentialStore",
            &error.to_string(),
        )
    })
}

/// Read the stored password. `Ok(None)` means no password has been set on this machine.
pub fn read_password() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(backup_error(
            "CREDENTIAL_STORE",
            "settings.backupSettings.encryption.errors.credentialStore",
            &error.to_string(),
        )),
    }
}

/// Store or replace the password. An empty password is rejected — clearing happens by
/// disabling the encryption switch, not by storing an empty secret.
pub fn store_password(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err(backup_error(
            "CREDENTIAL_STORE",
            "settings.backupSettings.encryption.errors.passwordEmpty",
            "empty backup encryption password",
        ));
    }
    entry()?
        .set_password(password)
        .map_err(|error| {
            backup_error(
                "CREDENTIAL_STORE",
                "settings.backupSettings.encryption.errors.credentialStore",
                &error.to_string(),
            )
        })
}

/// Remove the stored password entirely (used when rolling back a failed save that
/// had no previous credential).
pub fn delete_password() -> Result<(), String> {
    entry()?
        .delete_credential()
        .or_else(|error| match error {
            // Already gone is fine for rollback purposes.
            keyring::Error::NoEntry => Ok(()),
            other => Err(backup_error(
                "CREDENTIAL_STORE",
                "settings.backupSettings.encryption.errors.credentialStore",
                &other.to_string(),
            )),
        })
}

/// Build the JSON error payload shared by backup commands. `detail` never contains
/// passwords or tokens; it is a short technical reason for logs and the UI.
pub fn backup_error(error_type: &str, suggestion_key: &str, detail: &str) -> String {
    serde_json::json!({
        "type": error_type,
        "message": detail,
        "suggestion": suggestion_key,
    })
    .to_string()
}
