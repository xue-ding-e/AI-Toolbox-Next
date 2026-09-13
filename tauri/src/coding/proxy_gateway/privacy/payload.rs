use serde_json::Value;

type TransformText<'a> = dyn FnMut(&str, bool) -> Result<String, String> + 'a;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Envelope,
    Message,
    Block,
    Tool,
    Function,
    Schema,
    Arguments,
    Business,
}

pub(super) fn transform(
    value: &mut Value,
    text: &mut TransformText<'_>,
    restoring: bool,
) -> Result<bool, String> {
    visit(value, Shape::Envelope, text, restoring)
}

pub(super) fn transform_business(
    value: &mut Value,
    text: &mut TransformText<'_>,
    encoded_json: bool,
) -> Result<bool, String> {
    match value {
        Value::String(value) => transform_string(value, text, false, encoded_json),
        _ => visit(value, Shape::Business, text, false),
    }
}

fn transform_string(
    value: &mut String,
    text: &mut TransformText<'_>,
    credential: bool,
    encoded_json: bool,
) -> Result<bool, String> {
    if encoded_json {
        if let Ok(mut nested @ (Value::Object(_) | Value::Array(_))) =
            serde_json::from_str::<Value>(value)
        {
            if visit(&mut nested, Shape::Business, text, false)? {
                *value = nested.to_string();
                return Ok(true);
            }
            return Ok(false);
        }
    }
    let replacement = text(value, credential)?;
    if *value == replacement {
        Ok(false)
    } else {
        *value = replacement;
        Ok(true)
    }
}

fn credential_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "password"
            | "passwd"
            | "pwd"
            | "api_key"
            | "apikey"
            | "api-key"
            | "access_token"
            | "client_secret"
            | "private_key"
    )
}

fn visit(
    value: &mut Value,
    shape: Shape,
    text: &mut TransformText<'_>,
    restoring: bool,
) -> Result<bool, String> {
    if shape == Shape::Arguments {
        if let Value::String(arguments) = value {
            if let Ok(mut parsed) = serde_json::from_str::<Value>(arguments) {
                if visit(&mut parsed, Shape::Business, text, restoring)? {
                    *arguments = parsed.to_string();
                    return Ok(true);
                }
                return Ok(false);
            }
            if text(arguments, false)? != *arguments {
                return Err(
                    "privacy_tool_arguments_invalid: expected a JSON argument string".into(),
                );
            }
            return Ok(false);
        }
        return visit(value, Shape::Business, text, restoring);
    }
    match value {
        Value::String(value) => transform_string(
            value,
            text,
            false,
            matches!(shape, Shape::Business | Shape::Block | Shape::Message),
        ),
        Value::Array(values) => {
            let mut changed = false;
            for value in values {
                changed |= visit(value, shape, text, restoring)?;
            }
            Ok(changed)
        }
        Value::Object(object) => {
            // This guard is contextual: a business object's "signature" key has no protocol meaning.
            let signed = shape != Shape::Business
                && ["signature", "thoughtSignature", "thought_signature"]
                    .iter()
                    .any(|key| {
                        object
                            .get(*key)
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.is_empty())
                    });
            let block_type = object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            // Anthropic/Gemini thinking may receive its signature in a later stream event.
            let signed = signed
                || (restoring
                    && shape == Shape::Block
                    && (block_type == "thinking"
                        || object.get("thought").and_then(Value::as_bool) == Some(true)));
            if shape == Shape::Block
                && matches!(
                    block_type.as_str(),
                    "image"
                        | "image_url"
                        | "input_image"
                        | "input_audio"
                        | "audio"
                        | "file"
                        | "input_file"
                        | "document"
                        | "redacted_thinking"
                        | "compaction"
                )
            {
                return Ok(false);
            }
            let mut changed = false;
            for (key, value) in object.iter_mut() {
                if shape == Shape::Business {
                    if let Value::String(value) = value {
                        changed |= transform_string(value, text, credential_key(key), true)?;
                    } else {
                        changed |= visit(value, Shape::Business, text, restoring)?;
                    }
                    continue;
                }
                let child = match shape {
                    Shape::Envelope => match key.as_str() {
                        "model"
                        | "id"
                        | "object"
                        | "type"
                        | "status"
                        | "previous_response_id"
                        | "response_id"
                        | "item_id"
                        | "call_id"
                        | "stream_id"
                        | "event_id"
                        | "prompt_cache_key"
                        | "service_tier"
                        | "store"
                        | "include"
                        | "usage"
                        | "usageMetadata"
                        | "stop"
                        | "stop_sequences"
                        | "finish_reason"
                        | "generationConfig"
                        | "safetySettings"
                        | "tool_choice"
                        | "parallel_tool_calls"
                        | "reasoning" => continue,
                        "response_format" => Shape::Schema,
                        "text" if value.is_object() => Shape::Schema,
                        "content" if value.is_array() => Shape::Block,
                        "messages" | "message" | "delta" | "systemInstruction" | "content" => {
                            Shape::Message
                        }
                        "input" | "output" | "item" | "content_block" | "part" | "system" => {
                            Shape::Block
                        }
                        "contents" => Shape::Message,
                        "tools" | "functions" => Shape::Tool,
                        "response" | "choices" | "candidates" | "error" => Shape::Envelope,
                        "arguments" => Shape::Arguments,
                        // Metadata identities have no business-text semantics.
                        "metadata" => {
                            for (key, value) in value.as_object_mut().into_iter().flatten() {
                                if !matches!(
                                    key.as_str(),
                                    "session_id" | "conversation_id" | "user_id"
                                ) {
                                    changed |= visit(value, Shape::Business, text, restoring)?;
                                }
                            }
                            continue;
                        }
                        _ => Shape::Business,
                    },
                    Shape::Message => match key.as_str() {
                        "id" | "role" | "name" | "tool_call_id" | "type" | "signature"
                        | "thoughtSignature" | "thought_signature" | "encrypted_content" => {
                            continue
                        }
                        "content" | "parts" => Shape::Block,
                        "tool_calls" | "function_call" => Shape::Function,
                        _ => Shape::Business,
                    },
                    Shape::Block => match key.as_str() {
                        "id" | "type" | "role" | "name" | "call_id" | "tool_use_id"
                        | "tool_call_id" | "status" | "signature" | "thoughtSignature"
                        | "thought_signature" | "encrypted_content" | "source" | "inlineData"
                        | "inline_data" | "fileData" | "file_data" | "image_url" => continue,
                        "arguments" => Shape::Arguments,
                        "input" | "args" | "output" => Shape::Business,
                        "functionCall" | "functionResponse" | "tool_calls" | "function_call" => {
                            Shape::Function
                        }
                        "content" | "parts" | "summary" => Shape::Block,
                        _ => Shape::Business,
                    },
                    Shape::Function => match key.as_str() {
                        "id" | "type" | "name" | "call_id" | "index" => continue,
                        "function" => Shape::Function,
                        "arguments" => Shape::Arguments,
                        _ => Shape::Business,
                    },
                    Shape::Tool => match key.as_str() {
                        "name" | "type" | "strict" => continue,
                        "function" | "functionDeclarations" | "tools" => Shape::Tool,
                        "parameters" | "input_schema" | "parametersJsonSchema" => Shape::Schema,
                        "description" => Shape::Business,
                        _ => continue,
                    },
                    Shape::Schema => match key.as_str() {
                        "description" | "title" | "default" | "examples" | "example" | "const"
                        | "enum" => Shape::Business,
                        "properties" | "$defs" | "definitions" | "patternProperties" => {
                            for value in value
                                .as_object_mut()
                                .into_iter()
                                .flat_map(|object| object.values_mut())
                            {
                                changed |= visit(value, Shape::Schema, text, restoring)?;
                            }
                            continue;
                        }
                        "items"
                        | "allOf"
                        | "anyOf"
                        | "oneOf"
                        | "not"
                        | "additionalProperties"
                        | "schema"
                        | "json_schema"
                        | "format" => Shape::Schema,
                        _ => continue,
                    },
                    Shape::Business | Shape::Arguments => unreachable!(),
                };
                changed |= visit(value, child, text, restoring)?;
            }
            if signed && changed {
                return Err("privacy_signed_payload: sensitive signed content cannot be changed safely; start a new conversation without this signed history".into());
            }
            Ok(changed)
        }
        _ => Ok(false),
    }
}
