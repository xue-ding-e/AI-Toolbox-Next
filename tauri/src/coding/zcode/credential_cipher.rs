//! Reads the identity out of ZCode's encrypted credential values.
//!
//! `v2/credentials.json` is a flat `string -> string` map, and ZCode wraps every
//! value with `enc:v1:` AES-256-GCM before writing it. Switching accounts does
//! not need any of this: a switch copies whole snapshots verbatim, ciphertext
//! included, exactly as ZCode wrote them. *Labelling* an account does — the
//! email lives inside `oauth:<provider>:user_info`, which is one of those
//! ciphertext values — so this module implements the decrypt half of the scheme
//! and nothing else.
//!
//! Key derivation is `SHA-256(ZCODE_CREDENTIAL_SECRET)` when that variable is
//! set, and otherwise
//! `SHA-256("zcode-credential-fallback:<platform>:<home>:<user>")`. The fallback
//! is why a snapshot cannot travel between machines or user accounts: the
//! ciphertext is readable only where it was written.

use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Prefix marking a ciphertext value. Values without it are legacy plaintext
/// and are used as-is.
const ENCRYPTED_VALUE_PREFIX: &str = "enc:v1:";
const NONCE_LENGTH: usize = 12;
const AUTH_TAG_LENGTH: usize = 16;
const SECRET_ENV: &str = "ZCODE_CREDENTIAL_SECRET";

pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(ENCRYPTED_VALUE_PREFIX)
}

/// Node's `process.platform()` spelling, which is what the derivation string
/// is built from — not `std::env::consts::OS`.
fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other,
    }
}

/// Node's `os.userInfo().username`. Missing variables fall back to the same
/// placeholder ZCode uses, so a key derived here matches one derived there.
fn os_username() -> String {
    ["USERNAME", "USER", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The key ZCode derives its credential cipher from on this machine.
pub fn credential_secret(home: &Path) -> String {
    if let Ok(configured) = std::env::var(SECRET_ENV) {
        if !configured.trim().is_empty() {
            return configured;
        }
    }
    format!(
        "zcode-credential-fallback:{}:{}:{}",
        node_platform(),
        home.display(),
        os_username()
    )
}

fn decrypt_with_secret(value: &str, secret: &str) -> Option<String> {
    let body = value.strip_prefix(ENCRYPTED_VALUE_PREFIX)?;
    let mut parts = body.split('.');
    let (nonce, tag, ciphertext) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let nonce = URL_SAFE_NO_PAD.decode(nonce).ok()?;
    let tag = URL_SAFE_NO_PAD.decode(tag).ok()?;
    let ciphertext = URL_SAFE_NO_PAD.decode(ciphertext).ok()?;
    if nonce.len() != NONCE_LENGTH || tag.len() != AUTH_TAG_LENGTH {
        return None;
    }
    let key_bytes = Sha256::digest(secret.as_bytes());
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).ok()?);
    // ZCode keeps nonce, tag and ciphertext as three separate fields; `ring`
    // wants the tag appended to the ciphertext, so rejoin them here.
    let mut sealed = ciphertext;
    sealed.extend_from_slice(&tag);
    let nonce = nonce.try_into().ok()?;
    let plaintext = key
        .open_in_place(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut sealed)
        .ok()?;
    String::from_utf8(plaintext.to_vec()).ok()
}

/// Decrypts one credential value.
///
/// Returns `None` rather than an error for everything it cannot read: legacy
/// plaintext, a snapshot copied from another machine (the key no longer
/// matches), and a malformed payload all collapse to the same answer. Callers
/// only use this to *label* an account, so an unreadable value costs a display
/// name, never a failure.
pub fn decrypt_value(value: &str, secret: &str) -> Option<String> {
    if !is_encrypted(value) {
        return Some(value.to_string());
    }
    decrypt_with_secret(value, secret)
}

/// Reads one key out of a credentials map, decrypting when needed.
pub fn read_plain(credentials: &Value, key: &str, secret: &str) -> Option<String> {
    let raw = credentials.get(key)?.as_str()?;
    decrypt_value(raw, secret).filter(|value| !value.trim().is_empty())
}

/// Who a credentials map is logged in as.
#[derive(Debug, Clone, Default)]
pub struct ZcodeLoginIdentity {
    /// `oauth:active_provider` — `zai` or `bigmodel`.
    pub provider: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub username: Option<String>,
}

impl ZcodeLoginIdentity {
    /// Whether anything identifying was recovered. A credentials map whose
    /// values cannot be decrypted still counts as logged in, but carries no
    /// identity to match against.
    pub fn has_signal(&self) -> bool {
        !self.provider.is_empty()
            || self.user_id.is_some()
            || self.email.is_some()
            || self.username.is_some()
    }
}

/// Whether a credentials map holds an official login at all.
///
/// Mirrors ZCode's own check: an access token under any provider namespace, or
/// a `zcodejwttoken`. Providers are namespaced so a map can hold several
/// logins at once with `oauth:active_provider` picking the live one.
pub fn is_logged_in(credentials: &Value) -> bool {
    let has_value = |key: &str| {
        credentials
            .get(key)
            .and_then(|value| value.as_str())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    };
    if has_value("zcodejwttoken") {
        return true;
    }
    credentials
        .as_object()
        .map(|map| {
            map.keys()
                .any(|key| key.starts_with("oauth:") && key.ends_with(":access_token") && has_value(key))
        })
        .unwrap_or(false)
}

/// Decodes the payload of `zcodejwttoken`.
///
/// ZCode only ever reads `exp` out of this token locally and leaves signature
/// checking to the server, so the payload can be read without a key.
fn jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

/// Reads who the credentials belong to.
///
/// `oauth:<provider>:user_info` is the primary source — it is a JSON string
/// holding `user_id`, `email`, `name` and `avatar`. When it is absent or
/// unreadable the JWT payload supplies `user_id`, which is enough to tell two
/// accounts of the same provider apart.
pub fn read_login_identity(credentials: &Value, secret: &str) -> ZcodeLoginIdentity {
    let provider = read_plain(credentials, "oauth:active_provider", secret)
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    let mut identity = ZcodeLoginIdentity {
        provider: provider.clone(),
        ..ZcodeLoginIdentity::default()
    };

    if !provider.is_empty() {
        if let Some(raw) = read_plain(credentials, &format!("oauth:{provider}:user_info"), secret) {
            if let Ok(info) = serde_json::from_str::<Value>(&raw) {
                let text = |key: &str| {
                    info.get(key)
                        .and_then(|value| value.as_str())
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                };
                identity.user_id = text("user_id").or_else(|| text("id"));
                identity.email = text("email");
                identity.username = text("name")
                    .or_else(|| text("username"))
                    .or_else(|| text("displayName"));
            }
        }
    }

    if identity.user_id.is_none() {
        identity.user_id = read_plain(credentials, "zcodejwttoken", secret)
            .and_then(|token| jwt_payload(&token))
            .and_then(|payload| {
                payload
                    .get("user_id")
                    .or_else(|| payload.get("sub"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            });
    }

    identity
}

/// A human-readable label for one identity: email, then username, then the raw
/// user id, then whatever the provider namespace offers.
pub fn identity_label(identity: &ZcodeLoginIdentity) -> Option<String> {
    identity
        .email
        .clone()
        .or_else(|| identity.username.clone())
        .or_else(|| identity.user_id.clone())
        .or_else(|| {
            (!identity.provider.is_empty()).then(|| identity.provider.clone())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Everything below uses a fixed secret, so the ciphertext is deterministic
    /// to produce and the tests do not depend on the machine's key.
    fn seal(plaintext: &str, secret: &str) -> String {
        let key_bytes = Sha256::digest(secret.as_bytes());
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap());
        let nonce_bytes = [7u8; NONCE_LENGTH];
        let mut sealed = plaintext.as_bytes().to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut sealed,
        )
        .unwrap();
        let (ciphertext, tag) = sealed.split_at(sealed.len() - AUTH_TAG_LENGTH);
        format!(
            "enc:v1:{}.{}.{}",
            URL_SAFE_NO_PAD.encode(nonce_bytes),
            URL_SAFE_NO_PAD.encode(tag),
            URL_SAFE_NO_PAD.encode(ciphertext)
        )
    }

    #[test]
    fn an_encrypted_value_round_trips() {
        let secret = "unit-test-secret";
        let sealed = seal("zai", secret);
        assert!(is_encrypted(&sealed));
        assert_eq!(decrypt_value(&sealed, secret).as_deref(), Some("zai"));
    }

    /// Produced by Node's `crypto` with the same fixed secret and IV, i.e. by
    /// the implementation ZCode itself uses. The round-trip test above only
    /// proves this module agrees with itself; this one proves the format and
    /// the key derivation match the other side.
    const NODE_VECTOR: &str = "enc:v1:CxwtPk9QYXKDlKW2.VRWCiSnVDqKsA1yeGzFeuA.li_57oKBy1XtHYng_8rCgtc";
    const NODE_VECTOR_SECRET: &str = "ai-toolbox-test-vector";

    #[test]
    fn the_cipher_matches_the_one_zcode_writes() {
        assert_eq!(
            decrypt_value(NODE_VECTOR, NODE_VECTOR_SECRET).as_deref(),
            Some("{\"user_id\":\"u-1\"}"),
        );
    }

    #[test]
    fn a_foreign_key_yields_no_value_instead_of_an_error() {
        let sealed = seal("zai", "the-writing-machine");
        assert_eq!(decrypt_value(&sealed, "another-machine"), None);
    }

    #[test]
    fn plaintext_values_pass_through() {
        assert_eq!(decrypt_value("zai", "unused").as_deref(), Some("zai"));
    }

    #[test]
    fn a_malformed_payload_is_not_decrypted() {
        for value in [
            "enc:v1:",
            "enc:v1:only.two",
            "enc:v1:a.b.c.d",
            "enc:v1:!!!.!!!.!!!",
        ] {
            assert_eq!(decrypt_value(value, "secret"), None, "{value}");
        }
    }

    #[test]
    fn the_identity_comes_from_the_user_info_blob() {
        let secret = "unit-test-secret";
        let credentials = json!({
            "oauth:active_provider": seal("zai", secret),
            "oauth:zai:user_info": seal(
                &json!({
                    "user_id": "u-1",
                    "email": "someone@example.com",
                    "name": "Someone",
                })
                .to_string(),
                secret,
            ),
        });

        let identity = read_login_identity(&credentials, secret);
        assert_eq!(identity.provider, "zai");
        assert_eq!(identity.email.as_deref(), Some("someone@example.com"));
        assert_eq!(identity.user_id.as_deref(), Some("u-1"));
        assert_eq!(identity_label(&identity).as_deref(), Some("someone@example.com"));
    }

    #[test]
    fn the_jwt_supplies_the_user_id_when_user_info_is_missing() {
        let secret = "unit-test-secret";
        let claims = json!({ "user_id": "u-2", "sub": "s-2" }).to_string();
        let token = format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(claims.as_bytes())
        );
        let credentials = json!({
            "oauth:active_provider": seal("bigmodel", secret),
            "zcodejwttoken": seal(&token, secret),
        });

        let identity = read_login_identity(&credentials, secret);
        assert_eq!(identity.provider, "bigmodel");
        assert_eq!(identity.user_id.as_deref(), Some("u-2"));
        assert_eq!(identity.email, None);
    }

    #[test]
    fn an_unreadable_map_carries_no_identity() {
        let credentials = json!({
            "oauth:active_provider": seal("zai", "another-machine"),
        });

        let identity = read_login_identity(&credentials, "this-machine");
        assert!(!identity.has_signal());
        assert_eq!(identity_label(&identity), None);
    }

    #[test]
    fn a_login_is_detected_from_either_namespace() {
        assert!(!is_logged_in(&json!({})));
        assert!(!is_logged_in(&json!({ "oauth:active_provider": "zai" })));
        assert!(is_logged_in(&json!({ "zcodejwttoken": "jwt" })));
        assert!(is_logged_in(&json!({ "oauth:zai:access_token": "token" })));
    }
}