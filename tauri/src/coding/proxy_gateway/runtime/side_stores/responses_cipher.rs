use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

const MAX_INVALID_RESPONSES_CIPHERS: usize = 4096;
const ENCRYPTED_CONTENT_LABELS: [&str; 2] = ["encrypted content ", "encrypted_content "];
const ENCRYPTED_CONTENT_ERROR_SUFFIXES: [&str; 2] =
    [" could not be verified", " could not be decrypted"];
const MIN_STANDALONE_CIPHER_TOKEN_LENGTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportedEncryptedContent<'a> {
    Full(&'a str),
    Truncated { prefix: &'a str, suffix: &'a str },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InvalidResponsesCipherKey {
    provider_config_identity: [u8; 32],
    ciphertext_digest: [u8; 32],
}

#[derive(Debug, Default)]
struct InvalidResponsesCipherInner {
    keys: HashSet<InvalidResponsesCipherKey>,
    insertion_order: VecDeque<InvalidResponsesCipherKey>,
    provider_entry_counts: HashMap<[u8; 32], usize>,
}

#[derive(Debug, Default)]
pub(super) struct InvalidResponsesCipherStore {
    inner: Mutex<InvalidResponsesCipherInner>,
}

impl InvalidResponsesCipherStore {
    pub(super) fn remember_rejected_from_body(
        &self,
        provider_config_identity: [u8; 32],
        body: &[u8],
        error_message: &str,
    ) -> usize {
        let Ok(value) = serde_json::from_slice::<Value>(body) else {
            return 0;
        };
        let encrypted_contents = encrypted_reasoning_contents(&value);
        let reported_contents = extract_reported_encrypted_contents(error_message);
        let rejected_contents =
            select_rejected_encrypted_contents(&encrypted_contents, &reported_contents);
        let keys = rejected_contents
            .into_iter()
            .map(|encrypted_content| cipher_key(provider_config_identity, encrypted_content))
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return 0;
        }
        let Ok(mut inner) = self.inner.lock() else {
            return 0;
        };
        let mut inserted = 0;
        for key in keys {
            if inner.keys.insert(key.clone()) {
                *inner
                    .provider_entry_counts
                    .entry(key.provider_config_identity)
                    .or_default() += 1;
                inner.insertion_order.push_back(key);
                inserted += 1;
            }
        }
        while inner.keys.len() > MAX_INVALID_RESPONSES_CIPHERS {
            let Some(oldest_key) = inner.insertion_order.pop_front() else {
                break;
            };
            if inner.keys.remove(&oldest_key) {
                let provider_config_identity = oldest_key.provider_config_identity;
                if let Some(count) = inner
                    .provider_entry_counts
                    .get_mut(&provider_config_identity)
                {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        inner
                            .provider_entry_counts
                            .remove(&provider_config_identity);
                    }
                }
            }
        }
        inserted
    }

    pub(super) fn has_entries_for_provider(&self, provider_config_identity: [u8; 32]) -> bool {
        self.inner.lock().ok().is_some_and(|inner| {
            inner
                .provider_entry_counts
                .contains_key(&provider_config_identity)
        })
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

fn encrypted_reasoning_contents(body: &Value) -> Vec<&str> {
    let mut seen = HashSet::new();
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(encrypted_reasoning_content)
        .filter(|encrypted_content| seen.insert(*encrypted_content))
        .collect()
}

fn encrypted_reasoning_content(item: &Value) -> Option<&str> {
    (item.get("type").and_then(Value::as_str) == Some("reasoning"))
        .then(|| item.get("encrypted_content").and_then(Value::as_str))
        .flatten()
        .filter(|encrypted_content| !encrypted_content.trim().is_empty())
}

fn select_rejected_encrypted_contents<'a>(
    encrypted_contents: &[&'a str],
    reported_contents: &[ReportedEncryptedContent<'_>],
) -> Vec<&'a str> {
    if !reported_contents.is_empty() {
        let matches = encrypted_contents
            .iter()
            .copied()
            .filter(|encrypted_content| {
                reported_contents.iter().any(|reported| match reported {
                    ReportedEncryptedContent::Full(reported_content) => {
                        encrypted_content == reported_content
                    }
                    ReportedEncryptedContent::Truncated { prefix, suffix } => {
                        encrypted_content.starts_with(prefix) && encrypted_content.ends_with(suffix)
                    }
                })
            })
            .take(2)
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            return matches;
        }
        if matches.len() > 1 {
            return Vec::new();
        }
    }

    (encrypted_contents.len() == 1)
        .then(|| encrypted_contents.to_vec())
        .unwrap_or_default()
}

fn extract_reported_encrypted_contents(message: &str) -> Vec<ReportedEncryptedContent<'_>> {
    if let Some(reported_content) = extract_legacy_reported_encrypted_content(message) {
        return vec![reported_content];
    }
    let mut reported_contents = extract_truncated_encrypted_contents(message);
    reported_contents.extend(extract_standalone_cipher_tokens(message));
    reported_contents
}

fn extract_truncated_encrypted_contents(message: &str) -> Vec<ReportedEncryptedContent<'_>> {
    let mut reported_contents = Vec::new();
    let mut search_start = 0;
    while let Some((ellipsis_start, ellipsis_length)) = find_next_ellipsis(message, search_start) {
        let prefix_start = message[..ellipsis_start]
            .char_indices()
            .rev()
            .take_while(|(_, character)| is_cipher_token_character(*character))
            .last()
            .map(|(index, _)| index)
            .unwrap_or(ellipsis_start);
        let suffix_start = ellipsis_start + ellipsis_length;
        let suffix_end = message[suffix_start..]
            .char_indices()
            .take_while(|(_, character)| is_cipher_token_character(*character))
            .last()
            .map(|(index, character)| suffix_start + index + character.len_utf8())
            .unwrap_or(suffix_start);
        if prefix_start < ellipsis_start && suffix_start < suffix_end {
            reported_contents.push(ReportedEncryptedContent::Truncated {
                prefix: &message[prefix_start..ellipsis_start],
                suffix: &message[suffix_start..suffix_end],
            });
        }
        search_start = suffix_start;
    }
    reported_contents
}

fn find_next_ellipsis(message: &str, search_start: usize) -> Option<(usize, usize)> {
    let remaining = &message[search_start..];
    [("...", 3), ("……", 6), ("…", 3)]
        .into_iter()
        .filter_map(|(ellipsis, byte_length)| {
            remaining
                .find(ellipsis)
                .map(|index| (search_start + index, byte_length))
        })
        .min_by_key(|(index, _)| *index)
}

fn extract_standalone_cipher_tokens(message: &str) -> Vec<ReportedEncryptedContent<'_>> {
    let mut tokens = Vec::new();
    let mut token_start = None;
    for (index, character) in message
        .char_indices()
        .chain(std::iter::once((message.len(), ' ')))
    {
        if is_cipher_token_character(character) {
            token_start.get_or_insert(index);
        } else if let Some(start) = token_start.take() {
            let token = &message[start..index];
            if token.len() >= MIN_STANDALONE_CIPHER_TOKEN_LENGTH {
                tokens.push(ReportedEncryptedContent::Full(token));
            }
        }
    }
    tokens
}

fn is_cipher_token_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '+' | '/' | '=')
}

fn extract_legacy_reported_encrypted_content(
    message: &str,
) -> Option<ReportedEncryptedContent<'_>> {
    let lower = message.to_ascii_lowercase();
    for label in ENCRYPTED_CONTENT_LABELS {
        let Some(label_start) = lower.find(label) else {
            continue;
        };
        let content_start = label_start + label.len();
        let remaining_lower = &lower[content_start..];
        let Some(content_end) = ENCRYPTED_CONTENT_ERROR_SUFFIXES
            .iter()
            .filter_map(|suffix| remaining_lower.find(suffix))
            .min()
        else {
            continue;
        };
        let reported_content = trim_reported_content_wrapper(
            message[content_start..content_start + content_end].trim(),
        );
        if reported_content.is_empty() {
            continue;
        }
        if let Some((prefix, suffix)) = reported_content.split_once("...") {
            let prefix = prefix.trim();
            let suffix = suffix.trim();
            if prefix.is_empty() || suffix.is_empty() {
                return None;
            }
            return Some(ReportedEncryptedContent::Truncated { prefix, suffix });
        }
        return Some(ReportedEncryptedContent::Full(reported_content));
    }
    None
}

fn trim_reported_content_wrapper(content: &str) -> &str {
    content
        .strip_prefix('`')
        .and_then(|content| content.strip_suffix('`'))
        .or_else(|| {
            content
                .strip_prefix('"')
                .and_then(|content| content.strip_suffix('"'))
        })
        .or_else(|| {
            content
                .strip_prefix('\'')
                .and_then(|content| content.strip_suffix('\''))
        })
        .unwrap_or(content)
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
        assert_eq!(
            store.remember_rejected_from_body(provider_a, &rejected_body, "decrypt failed"),
            1
        );

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

        assert_eq!(
            store.remember_rejected_from_body(provider, &body, "decrypt failed"),
            1
        );
        assert_eq!(
            store.remember_rejected_from_body(provider, &body, "decrypt failed"),
            0
        );
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
            assert_eq!(
                store.remember_rejected_from_body(provider, &body, "decrypt failed"),
                1
            );
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
        assert!(store.has_entries_for_provider(provider));
    }

    #[test]
    fn provider_entry_presence_tracks_insertions_and_evictions() {
        let store = InvalidResponsesCipherStore::default();
        let oldest_provider = [1; 32];
        let newest_provider = [2; 32];
        assert!(!store.has_entries_for_provider(oldest_provider));

        let oldest_body = serde_json::to_vec(&json!({
            "input": [{"type":"reasoning","encrypted_content":"oldest"}]
        }))
        .unwrap();
        assert_eq!(
            store.remember_rejected_from_body(oldest_provider, &oldest_body, "decrypt failed"),
            1
        );
        assert!(store.has_entries_for_provider(oldest_provider));

        for index in 0..MAX_INVALID_RESPONSES_CIPHERS {
            let body = serde_json::to_vec(&json!({
                "input": [{
                    "type":"reasoning",
                    "encrypted_content": format!("newest-{index}")
                }]
            }))
            .unwrap();
            assert_eq!(
                store.remember_rejected_from_body(newest_provider, &body, "decrypt failed"),
                1
            );
        }

        assert!(!store.has_entries_for_provider(oldest_provider));
        assert!(store.has_entries_for_provider(newest_provider));
    }

    #[test]
    fn remembers_only_the_cipher_explicitly_named_by_a_multi_cipher_error() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"cipher-old"},
                {"type":"reasoning","encrypted_content":"cipher-valid"}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "The encrypted content cipher-old could not be verified",
            ),
            1
        );
        let mut next_body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(store.strip_known_from_body(provider, &mut next_body), 1);
        assert_eq!(next_body["input"][0]["encrypted_content"], "cipher-valid");
    }

    #[test]
    fn remembers_the_unique_cipher_matching_a_truncated_server_marker() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"gAAAAABq-first-AbCd=="},
                {"type":"reasoning","encrypted_content":"gAAAAABq-second-Pg=="}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "The encrypted content gAAA...Pg== could not be verified. Reason: Encrypted content could not be decrypted or parsed.",
            ),
            1
        );
        let mut next_body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(store.strip_known_from_body(provider, &mut next_body), 1);
        assert_eq!(
            next_body["input"][0]["encrypted_content"],
            "gAAAAABq-first-AbCd=="
        );
    }

    #[test]
    fn does_not_remember_an_ambiguous_truncated_server_marker() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"gAAAAABq-first-Pg=="},
                {"type":"reasoning","encrypted_content":"gAAAAABq-second-Pg=="}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "The encrypted content gAAA...Pg== could not be verified",
            ),
            0
        );
    }

    #[test]
    fn parses_quoted_full_ciphertext_and_rejects_incomplete_truncation_markers() {
        assert_eq!(
            extract_reported_encrypted_contents(
                "The encrypted content `cipher-full` could not be verified"
            ),
            vec![ReportedEncryptedContent::Full("cipher-full")]
        );
        assert_eq!(
            extract_reported_encrypted_contents(
                "The encrypted_content \"cipher-full\" could not be decrypted"
            ),
            vec![ReportedEncryptedContent::Full("cipher-full")]
        );
        assert_eq!(
            extract_reported_encrypted_contents(
                "The encrypted content ...Pg== could not be verified"
            ),
            Vec::<ReportedEncryptedContent<'_>>::new()
        );
        assert_eq!(
            extract_reported_encrypted_contents(
                "The encrypted content gAAA... could not be verified"
            ),
            Vec::<ReportedEncryptedContent<'_>>::new()
        );
    }

    #[test]
    fn extracts_truncated_cipher_tokens_without_relying_on_message_language() {
        assert_eq!(
            extract_reported_encrypted_contents("无法验证加密内容 gAAA...Pg==，请重试"),
            vec![ReportedEncryptedContent::Truncated {
                prefix: "gAAA",
                suffix: "Pg==",
            }]
        );
        assert_eq!(
            extract_reported_encrypted_contents("暗号化内容 gAAA…Pg== を検証できません"),
            vec![ReportedEncryptedContent::Truncated {
                prefix: "gAAA",
                suffix: "Pg==",
            }]
        );
        assert_eq!(
            extract_reported_encrypted_contents("密文 gAAA……Pg== 无效"),
            vec![ReportedEncryptedContent::Truncated {
                prefix: "gAAA",
                suffix: "Pg==",
            }]
        );
    }

    #[test]
    fn remembers_a_full_cipher_token_without_relying_on_message_language() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"gAAAAABq-full-Pg=="},
                {"type":"reasoning","encrypted_content":"gAAAAABq-other-Aw=="}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "暗号化内容 gAAAAABq-full-Pg== を検証できません",
            ),
            1
        );
        let mut next_body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(store.strip_known_from_body(provider, &mut next_body), 1);
        assert_eq!(
            next_body["input"][0]["encrypted_content"],
            "gAAAAABq-other-Aw=="
        );
    }

    #[test]
    fn multiple_reported_tokens_must_still_identify_one_request_cipher() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"gAAAA-first-Pg=="},
                {"type":"reasoning","encrypted_content":"gAAAA-second-Aw=="}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "候选 gAAA...Pg== 与 gAAA...Aw== 均被报告",
            ),
            0
        );
    }

    #[test]
    fn unrelated_truncated_text_does_not_select_a_request_cipher() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"gAAAA-first-Pg=="},
                {"type":"reasoning","encrypted_content":"gAAAA-second-Aw=="}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(provider, &body, "渠道 abc...xyz 暂时不可用"),
            0
        );
    }

    #[test]
    fn does_not_remember_ambiguous_multi_cipher_errors() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"cipher-a"},
                {"type":"reasoning","encrypted_content":"cipher-b"}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "encrypted_content could not be decrypted",
            ),
            0
        );
        let mut next_body = serde_json::from_slice::<Value>(&body).unwrap();
        assert_eq!(store.strip_known_from_body(provider, &mut next_body), 0);
    }

    #[test]
    fn repeated_occurrences_of_one_cipher_are_a_unique_candidate() {
        let store = InvalidResponsesCipherStore::default();
        let provider = [1; 32];
        let body = serde_json::to_vec(&json!({
            "input": [
                {"type":"reasoning","encrypted_content":"cipher-a"},
                {"type":"reasoning","encrypted_content":"cipher-a"}
            ]
        }))
        .unwrap();

        assert_eq!(
            store.remember_rejected_from_body(
                provider,
                &body,
                "encrypted_content could not be decrypted",
            ),
            1
        );
    }
}
