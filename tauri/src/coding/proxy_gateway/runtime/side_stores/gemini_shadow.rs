use futures_util::{stream, Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const MAX_GEMINI_SHADOW_SESSIONS: usize = 200;
const MAX_GEMINI_SHADOW_TURNS_PER_SESSION: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GeminiShadowSessionKey {
    provider_id: String,
    session_id: String,
}

impl GeminiShadowSessionKey {
    pub(crate) fn new(provider_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            session_id: session_id.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct GeminiShadowTurn {
    assistant_content: Value,
    function_call_names: Vec<String>,
}

#[derive(Debug, Default)]
struct GeminiShadowInner {
    sessions: HashMap<GeminiShadowSessionKey, VecDeque<GeminiShadowTurn>>,
    session_order: VecDeque<GeminiShadowSessionKey>,
}

#[derive(Debug, Default)]
pub(crate) struct GeminiShadowStore {
    inner: Mutex<GeminiShadowInner>,
}

impl GeminiShadowStore {
    pub(super) fn record_response(&self, key: GeminiShadowSessionKey, response: &Value) -> usize {
        let turns = response
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|candidate| candidate.get("content"))
            .filter_map(gemini_shadow_turn_from_content)
            .collect::<Vec<_>>();
        if turns.is_empty() {
            return 0;
        }
        let Ok(mut inner) = self.inner.lock() else {
            return 0;
        };
        touch_session_order(&mut inner.session_order, &key);
        let session = inner.sessions.entry(key).or_default();
        let changed = turns.len();
        for turn in turns {
            session.push_back(turn);
            while session.len() > MAX_GEMINI_SHADOW_TURNS_PER_SESSION {
                session.pop_front();
            }
        }
        prune_sessions(&mut inner);
        changed
    }

    pub(super) fn enrich_request(&self, key: &GeminiShadowSessionKey, body: &mut Value) -> usize {
        let Some(contents) = body.get_mut("contents").and_then(Value::as_array_mut) else {
            return 0;
        };
        if !request_needs_function_call_replay(contents) {
            return 0;
        }
        let requested_names = function_response_names(contents);
        let Some(turn) = self.latest_matching_turn(key, &requested_names) else {
            return 0;
        };
        let insert_index = contents
            .iter()
            .position(content_has_function_response)
            .unwrap_or(contents.len());
        contents.insert(insert_index, turn.assistant_content);
        1
    }

    fn latest_matching_turn(
        &self,
        key: &GeminiShadowSessionKey,
        requested_names: &[String],
    ) -> Option<GeminiShadowTurn> {
        let Ok(mut inner) = self.inner.lock() else {
            return None;
        };
        let turn = inner.sessions.get(key).and_then(|turns| {
            turns
                .iter()
                .rev()
                .find(|turn| {
                    requested_names.is_empty()
                        || requested_names
                            .iter()
                            .any(|name| turn.function_call_names.iter().any(|item| item == name))
                })
                .cloned()
        });
        if turn.is_some() {
            touch_session_order(&mut inner.session_order, key);
        }
        turn
    }
}

pub(crate) fn record_gemini_sse_stream(
    inner: Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>,
    store: Arc<GeminiShadowStore>,
    key: GeminiShadowSessionKey,
) -> Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>> {
    struct State {
        inner: Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>,
        store: Arc<GeminiShadowStore>,
        key: GeminiShadowSessionKey,
        buffer: Vec<u8>,
    }

    Box::pin(stream::unfold(
        State {
            inner,
            store,
            key,
            buffer: Vec::new(),
        },
        |mut state| async move {
            let item = state.inner.next().await?;
            if let Ok(bytes) = &item {
                state.buffer.extend_from_slice(bytes);
                while let Some(block) = take_sse_block(&mut state.buffer) {
                    inspect_gemini_sse_block(&block, &state.store, &state.key);
                }
            }
            Some((item, state))
        },
    ))
}

fn inspect_gemini_sse_block(block: &[u8], store: &GeminiShadowStore, key: &GeminiShadowSessionKey) {
    let Ok(block) = std::str::from_utf8(block) else {
        return;
    };
    let mut data_parts = Vec::new();
    for line in block.lines() {
        let Some((field, value)) = line.split_once(':') else {
            continue;
        };
        if field == "data" {
            data_parts.push(value.trim_start().to_string());
        }
    }
    let data = data_parts.join("\n");
    if data.trim().is_empty() || data.trim() == "[DONE]" {
        return;
    }
    if let Ok(value) = serde_json::from_str::<Value>(&data) {
        store.record_response(key.clone(), &value);
    }
}

fn take_sse_block(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let position = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2))
        .or_else(|| {
            buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| (index, 4))
        })?;
    let block = buffer[..position.0].to_vec();
    buffer.drain(..position.0 + position.1);
    Some(block)
}

fn gemini_shadow_turn_from_content(content: &Value) -> Option<GeminiShadowTurn> {
    let parts = content.get("parts").and_then(Value::as_array)?;
    let function_call_names = parts
        .iter()
        .filter(|part| part.get("functionCall").is_some() && part_signature(part).is_some())
        .filter_map(|part| part.pointer("/functionCall/name").and_then(Value::as_str))
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let has_part_signature = parts.iter().any(|part| part_signature(part).is_some());
    if function_call_names.is_empty() && !has_part_signature {
        return None;
    }
    Some(GeminiShadowTurn {
        assistant_content: normalize_assistant_content(content),
        function_call_names,
    })
}

fn normalize_assistant_content(content: &Value) -> Value {
    let mut content = content.clone();
    if content.get("role").and_then(Value::as_str).is_none() {
        if let Some(object) = content.as_object_mut() {
            object.insert("role".to_string(), json!("model"));
        }
    }
    content
}

fn request_needs_function_call_replay(contents: &[Value]) -> bool {
    let has_function_response = contents.iter().any(content_has_function_response);
    let has_function_call = contents.iter().any(content_has_function_call);
    has_function_response && !has_function_call
}

fn function_response_names(contents: &[Value]) -> Vec<String> {
    contents
        .iter()
        .flat_map(|content| {
            content
                .get("parts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|part| part.get("functionResponse"))
        .filter_map(|response| response.get("name").and_then(Value::as_str))
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn content_has_function_call(content: &Value) -> bool {
    content
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|part| part.get("functionCall").is_some())
}

fn content_has_function_response(content: &Value) -> bool {
    content
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|part| part.get("functionResponse").is_some())
}

fn part_signature(part: &Value) -> Option<&str> {
    part.get("thoughtSignature")
        .or_else(|| part.get("thought_signature"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn touch_session_order(order: &mut VecDeque<GeminiShadowSessionKey>, key: &GeminiShadowSessionKey) {
    if let Some(index) = order.iter().position(|existing| existing == key) {
        order.remove(index);
    }
    order.push_back(key.clone());
}

fn prune_sessions(inner: &mut GeminiShadowInner) {
    while inner.sessions.len() > MAX_GEMINI_SHADOW_SESSIONS {
        let Some(key) = inner.session_order.pop_front() else {
            break;
        };
        inner.sessions.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_latest_signed_function_call_before_function_response() {
        let store = GeminiShadowStore::default();
        let key = GeminiShadowSessionKey::new("provider-a", "session-a");
        store.record_response(
            key.clone(),
            &json!({
                "candidates": [{
                    "content": {
                        "role": "model",
                        "parts": [{
                            "functionCall": {"name": "read_file", "args": {"path": "README.md"}},
                            "thoughtSignature": "sig-1"
                        }]
                    }
                }]
            }),
        );

        let mut request = json!({
            "contents": [{
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "name": "read_file",
                        "response": {"content": "ok"}
                    }
                }]
            }]
        });

        assert_eq!(store.enrich_request(&key, &mut request), 1);
        let contents = request["contents"].as_array().unwrap();
        assert_eq!(contents[0]["role"], "model");
        assert_eq!(contents[0]["parts"][0]["thoughtSignature"], "sig-1");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["name"],
            "read_file"
        );
    }

    #[tokio::test]
    async fn records_signed_function_call_from_sse_stream_for_later_replay() {
        let store = Arc::new(GeminiShadowStore::default());
        let key = GeminiShadowSessionKey::new("provider-a", "session-stream");
        let sse = concat!(
            "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"functionCall\":{\"name\":\"lookup\",\"args\":{\"query\":\"rust\"}},\"thoughtSignature\":\"sig-stream\"}]}}]}\n\n",
            "data: {\"usageMetadata\":{\"totalTokenCount\":8}}\n\n"
        );
        let inner = Box::pin(stream::iter(vec![Ok(sse.as_bytes().to_vec())]));
        let mut wrapped = record_gemini_sse_stream(inner, store.clone(), key.clone());

        let mut forwarded = Vec::new();
        while let Some(chunk) = wrapped.next().await {
            forwarded.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(forwarded, sse.as_bytes());

        let mut request = json!({
            "contents": [{
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "name": "lookup",
                        "response": {"content": "ok"}
                    }
                }]
            }]
        });

        assert_eq!(store.enrich_request(&key, &mut request), 1);
        let contents = request["contents"].as_array().unwrap();
        assert_eq!(contents[0]["role"], "model");
        assert_eq!(contents[0]["parts"][0]["thoughtSignature"], "sig-stream");
        assert_eq!(contents[0]["parts"][0]["functionCall"]["name"], "lookup");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["name"],
            "lookup"
        );
    }
}
