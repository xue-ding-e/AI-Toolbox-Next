use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

const MAX_INVALID_RESPONSES_CIPHERS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InvalidResponsesCipherKey {
    provider_config_identity: [u8; 32],
    ciphertext_digest: [u8; 32],
}

#[derive(Debug, Default)]
struct InvalidResponsesCipherInner {
    keys: HashSet<InvalidResponsesCipherKey>,
    insertion_order: VecDeque<InvalidResponsesCipherKey>,
}

#[derive(Debug, Default)]
pub(super) struct InvalidResponsesCipherStore {
    inner: Mutex<InvalidResponsesCipherInner>,
}

impl InvalidResponsesCipherStore {
    pub(super) fn remember_from_body(
        &self,
        provider_config_identity: [u8; 32],
        body: &[u8],
    ) -> usize {
        let Ok(value) = serde_json::from_slice::<Value>(body) else {
            return 0;
        };
        let keys = encrypted_reasoning_keys(provider_config_identity, &value);
        if keys.is_empty() {
            return 0;
        }
        let Ok(mut inner) = self.inner.lock() else {
            return 0;
        };
        let mut inserted = 0;
        for key in keys {
            if inner.keys.insert(key.clone()) {
                inner.insertion_order.push_back(key);
                inserted += 1;
            }
        }
        while inner.keys.len() > MAX_INVALID_RESPONSES_CIPHERS {
            let Some(oldest_key) = inner.insertion_order.pop_front() else {
                break;
            };
            inner.keys.remove(&oldest_key);
        }
        inserted
    }

    pub(super) fn strip_known_from_body(
        &self,
        provider_config_identity: [u8; 32],
        body: &mut Value,
    ) -> usize {
        let Some(input) = body.get_mut("input").and_then(Value::as_array_mut) else {
            return 0;
        };
        let Ok(inner) = self.inner.lock() else {
            return 0;
        };
        let original_len = input.len();
        input.retain(|item| {
            encrypted_reasoning_content(item).is_none_or(|encrypted_content| {
                !inner
                    .keys
                    .contains(&cipher_key(provider_config_identity, encrypted_content))
            })
        });
        original_len.saturating_sub(input.len())
    }
}

fn encrypted_reasoning_keys(
    provider_config_identity: [u8; 32],
    body: &Value,
) -> Vec<InvalidResponsesCipherKey> {
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(encrypted_reasoning_content)
        .map(|encrypted_content| cipher_key(provider_config_identity, encrypted_content))
        .collect()
}

fn encrypted_reasoning_content(item: &Value) -> Option<&str> {
    (item.get("type").and_then(Value::as_str) == Some("reasoning"))
        .then(|| item.get("encrypted_content").and_then(Value::as_str))
        .flatten()
        .filter(|encrypted_content| !encrypted_content.trim().is_empty())
}

fn cipher_key(
    provider_config_identity: [u8; 32],
    encrypted_content: &str,
) -> InvalidResponsesCipherKey {
    InvalidResponsesCipherKey {
        provider_config_identity,
        ciphertext_digest: Sha256::digest(encrypted_content.as_bytes()).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_only_known_ciphers_for_the_same_provider() {
        let store = InvalidResponsesCipherStore::default();
        let provider_a = [1; 32];
        let provider_b = [2; 32];
        let rejected_body = serde_json::to_vec(&json!({
            "input": [{"type":"reasoning","encrypted_content":"cipher-old"}]
        }))
        .unwrap();
        assert_eq!(store.remember_from_body(provider_a, &rejected_body), 1);

        let mut same_provider = json!({
            "input": [
                {"type":"reasoning","encrypted_content":"cipher-old"},
                {"type":"reasoning","encrypted_content":"cipher-new"},
                {"type":"reasoning","summary":[{"type":"summary_text","text":"keep"}]},
                {"type":"message","role":"user","content":"hello"}
            ]
        });
        assert_eq!(
            store.strip_known_from_body(provider_a, &mut same_provider),
            1
        );
        assert_eq!(same_provider["input"].as_array().unwrap().len(), 3);
        assert_eq!(same_provider["input"][0]["encrypted_content"], "cipher-new");

        let mut other_provider = json!({
            "input": [{"type":"reasoning","encrypted_content":"cipher-old"}]
        });
        assert_eq!(
            store.strip_known_from_body(provider_b, &mut other_provider),
            0
        );
    }

    #[test]
    fn remembering_the_same_cipher_is_idempotent() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [{"type":"reasoning","encrypted_content":"cipher-old"}]
        }))
        .unwrap();

        assert_eq!(store.remember_from_body(provider, &body), 1);
        assert_eq!(store.remember_from_body(provider, &body), 0);
    }

    #[test]
    fn evicts_the_oldest_cipher_when_capacity_is_exceeded() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        for index in 0..=MAX_INVALID_RESPONSES_CIPHERS {
            let body = serde_json::to_vec(&json!({
                "input": [{
                    "type":"reasoning",
                    "encrypted_content": format!("cipher-{index}")
                }]
            }))
            .unwrap();
            assert_eq!(store.remember_from_body(provider, &body), 1);
        }

        let mut oldest = json!({
            "input": [{"type":"reasoning","encrypted_content":"cipher-0"}]
        });
        assert_eq!(store.strip_known_from_body(provider, &mut oldest), 0);

        let mut newest = json!({
            "input": [{
                "type":"reasoning",
                "encrypted_content": format!("cipher-{MAX_INVALID_RESPONSES_CIPHERS}")
            }]
        });
        assert_eq!(store.strip_known_from_body(provider, &mut newest), 1);
    }
}
