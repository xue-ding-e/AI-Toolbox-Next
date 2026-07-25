use super::super::llm::MessageContent;
use serde_json::{json, Value};

pub(crate) const TOOL_RESULT_MEDIA_MOVED_MARKER: &str =
    "[ai-toolbox: tool result media moved to the following user message]";
pub(crate) const TOOL_RESULT_MEDIA_ATTACHED_MARKER: &str =
    "[ai-toolbox: tool result media attached as native media]";

const WHOLE_DATA_URL_MIN_BYTES: usize = 8 * 1024;
const BASE64ISH_MIN_BYTES: usize = 16 * 1024;
const MAX_MEDIA_TRAVERSAL_DEPTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolImageScope {
    AnyImage,
    InlineImage,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolImage {
    pub(crate) url: String,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolResultMediaPlan {
    pub(crate) cleaned: Value,
    pub(crate) original_was_text: bool,
    pub(crate) images: Vec<ToolImage>,
}

pub(crate) fn plan_tool_result_images(
    content: &MessageContent,
    scope: ToolImageScope,
    replacement_text: &str,
) -> Option<ToolResultMediaPlan> {
    let original_was_text = matches!(content, MessageContent::Text(_));
    let mut cleaned = message_content_value(content);
    let replacement = json!({
        "type": "text",
        "text": replacement_text
    });
    let mut images = Vec::new();
    let replaced = strip_images_at_depth(
        &mut cleaned,
        &mut images,
        scope,
        &replacement,
        replacement_text,
        0,
    );
    if replaced == 0 {
        return None;
    }

    clamp_base64ish_strings(&mut cleaned);
    Some(ToolResultMediaPlan {
        cleaned,
        original_was_text,
        images,
    })
}

pub(crate) fn cleaned_tool_result_text(plan: &ToolResultMediaPlan) -> String {
    if plan.original_was_text {
        plan.cleaned.as_str().unwrap_or_default().to_string()
    } else {
        serde_json::to_string(&plan.cleaned).unwrap_or_default()
    }
}

pub(crate) fn inline_image_data(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim();
    let comma_index = trimmed.find(',')?;
    if comma_index + 1 >= trimmed.len() {
        return None;
    }
    let header = &trimmed[..comma_index];
    let normalized_header = header.to_ascii_lowercase();
    if !normalized_header.starts_with("data:image/") || !normalized_header.ends_with(";base64") {
        return None;
    }
    let media_type_end = header.find(';')?;
    let media_type = header.get("data:".len()..media_type_end)?;
    if media_type.is_empty() {
        return None;
    }
    Some((
        media_type.to_string(),
        trimmed[comma_index + 1..].to_string(),
    ))
}

fn message_content_value(content: &MessageContent) -> Value {
    match content {
        MessageContent::Text(text) => Value::String(text.clone()),
        MessageContent::Parts(parts) => {
            serde_json::to_value(parts).unwrap_or_else(|_| Value::Array(Vec::new()))
        }
        MessageContent::Empty => Value::Null,
    }
}

fn strip_images_at_depth(
    value: &mut Value,
    images: &mut Vec<ToolImage>,
    scope: ToolImageScope,
    replacement: &Value,
    replacement_text: &str,
    depth: usize,
) -> usize {
    if depth > MAX_MEDIA_TRAVERSAL_DEPTH {
        return 0;
    }

    match value {
        Value::String(text) => {
            if text.trim().len() >= WHOLE_DATA_URL_MIN_BYTES {
                if let Some(image) = image_from_url(text, None, scope) {
                    images.push(image);
                    *text = replacement_text.to_string();
                    return 1;
                }
            }

            let trimmed = text.trim();
            if trimmed.is_empty() {
                return 0;
            }
            let Ok(mut parsed) = serde_json::from_str::<Value>(trimmed) else {
                return 0;
            };
            let replaced = strip_images_at_depth(
                &mut parsed,
                images,
                scope,
                replacement,
                replacement_text,
                depth + 1,
            );
            if replaced > 0 {
                clamp_base64ish_strings(&mut parsed);
                *text = serde_json::to_string(&parsed).unwrap_or_default();
            }
            replaced
        }
        Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                strip_images_at_depth(
                    item,
                    images,
                    scope,
                    replacement,
                    replacement_text,
                    depth + 1,
                )
            })
            .sum(),
        Value::Object(_) => {
            if let Some(image) = tool_image_from_value(value, scope) {
                images.push(image);
                *value = replacement.clone();
                return 1;
            }

            value
                .as_object_mut()
                .and_then(|object| object.get_mut("content"))
                .map(|content| {
                    strip_images_at_depth(
                        content,
                        images,
                        scope,
                        replacement,
                        replacement_text,
                        depth + 1,
                    )
                })
                .unwrap_or(0)
        }
        _ => 0,
    }
}

fn tool_image_from_value(value: &Value, scope: ToolImageScope) -> Option<ToolImage> {
    let part_type = value.get("type").and_then(Value::as_str);
    match part_type {
        Some("input_image" | "image_url") => normalized_image_url(value, scope),
        Some("image") => typed_image(value, scope),
        None => {
            let image = normalized_image_url(value, scope)?;
            inline_image_data(&image.url).map(|_| image)
        }
        _ => None,
    }
}

fn normalized_image_url(value: &Value, scope: ToolImageScope) -> Option<ToolImage> {
    let image_url = value.get("image_url")?;
    let (url, nested_detail) = match image_url {
        Value::String(url) => (url.as_str(), None),
        Value::Object(object) => (
            object.get("url").and_then(Value::as_str)?,
            object
                .get("detail")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        ),
        _ => return None,
    };
    let detail = nested_detail.or_else(|| {
        value
            .get("detail")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    image_from_url(url, detail, scope)
}

fn typed_image(value: &Value, scope: ToolImageScope) -> Option<ToolImage> {
    if let Some(source) = value.get("source").and_then(Value::as_object) {
        let media_type = source
            .get("media_type")
            .or_else(|| source.get("mime_type"))
            .or_else(|| source.get("mimeType"))
            .and_then(Value::as_str);
        if media_type.is_some_and(|mime| !is_image_mime_type(mime)) {
            return None;
        }
        if let Some(url) = source
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.trim().is_empty())
        {
            return image_from_url(url, None, scope);
        }
        if let Some(data) = source
            .get("data")
            .and_then(Value::as_str)
            .filter(|data| !data.is_empty())
        {
            if data
                .get(.."data:".len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
            {
                return image_from_url(data, None, scope);
            }
            let media_type = media_type.unwrap_or("image/png");
            return image_from_url(&format!("data:{media_type};base64,{data}"), None, scope);
        }
    }

    let data = value
        .get("data")
        .and_then(Value::as_str)
        .filter(|data| !data.is_empty())?;
    let media_type = value
        .get("mimeType")
        .or_else(|| value.get("mime_type"))
        .and_then(Value::as_str)
        .filter(|mime| is_image_mime_type(mime))?;
    image_from_url(&format!("data:{media_type};base64,{data}"), None, scope)
}

fn image_from_url(url: &str, detail: Option<String>, scope: ToolImageScope) -> Option<ToolImage> {
    let trimmed = url.trim();
    let is_inline = inline_image_data(trimmed).is_some();
    let is_remote = trimmed
        .get(.."http://".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || trimmed
            .get(.."https://".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"));
    if !is_inline && !is_remote {
        return None;
    }
    if scope == ToolImageScope::InlineImage && !is_inline {
        return None;
    }
    Some(ToolImage {
        url: trimmed.to_string(),
        detail,
    })
}

fn is_image_mime_type(value: &str) -> bool {
    value
        .get(.."image/".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("image/"))
}

fn clamp_base64ish_strings(value: &mut Value) {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            let is_large_data_url = trimmed.len() >= WHOLE_DATA_URL_MIN_BYTES
                && trimmed
                    .get(.."data:".len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"));
            let is_large_base64 = trimmed.len() >= BASE64ISH_MIN_BYTES
                && trimmed.bytes().all(|byte| {
                    matches!(
                        byte,
                        b'a'..=b'z'
                            | b'A'..=b'Z'
                            | b'0'..=b'9'
                            | b'+'
                            | b'/'
                            | b'='
                    )
                });
            if is_large_data_url || is_large_base64 {
                let byte_len = text.len();
                *text = format!("[ai-toolbox: omitted {byte_len} bytes]");
            }
        }
        Value::Array(items) => {
            for item in items {
                clamp_base64ish_strings(item);
            }
        }
        Value::Object(object) => {
            for nested in object.values_mut() {
                clamp_base64ish_strings(nested);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::proxy_gateway::transformer::llm::{ImageUrl, MessageContentPart};

    #[test]
    fn extracts_ir_image_and_drops_cache_metadata() {
        let content = MessageContent::Parts(vec![MessageContentPart {
            part_type: "image_url".to_string(),
            image_url: Some(ImageUrl {
                url: "data:image/png;base64,YWJj".to_string(),
                detail: Some("high".to_string()),
            }),
            cache_control: Some(json!({"type": "ephemeral"})),
            ..Default::default()
        }]);

        let plan = plan_tool_result_images(
            &content,
            ToolImageScope::AnyImage,
            TOOL_RESULT_MEDIA_MOVED_MARKER,
        )
        .expect("image should be extracted");

        assert_eq!(plan.images[0].url, "data:image/png;base64,YWJj");
        assert_eq!(plan.images[0].detail.as_deref(), Some("high"));
        assert_eq!(plan.cleaned[0]["type"], "text");
        assert!(plan.cleaned[0].get("cache_control").is_none());
    }

    #[test]
    fn extracts_nested_mcp_image_from_json_string() {
        let content = MessageContent::Text(
            json!({
                "content": [
                    {"type": "text", "text": "caption"},
                    {"type": "image", "mimeType": "image/webp", "data": "ZGF0YQ=="}
                ]
            })
            .to_string(),
        );

        let plan = plan_tool_result_images(
            &content,
            ToolImageScope::AnyImage,
            TOOL_RESULT_MEDIA_ATTACHED_MARKER,
        )
        .expect("MCP image should be extracted");

        assert_eq!(plan.images[0].url, "data:image/webp;base64,ZGF0YQ==");
        assert!(!cleaned_tool_result_text(&plan).contains("ZGF0YQ=="));
    }

    #[test]
    fn inline_scope_keeps_remote_and_malformed_urls_in_legacy_content() {
        for url in [
            "https://example.com/image.png",
            "data:image/png,NOT_BASE64",
            "data:image/png;base64,",
        ] {
            let content = MessageContent::Text(
                json!({
                    "type": "image_url",
                    "image_url": {"url": url}
                })
                .to_string(),
            );
            assert!(plan_tool_result_images(
                &content,
                ToolImageScope::InlineImage,
                TOOL_RESULT_MEDIA_ATTACHED_MARKER,
            )
            .is_none());
        }
    }

    #[test]
    fn whole_string_data_url_respects_size_threshold() {
        let small = MessageContent::Text("data:image/png;base64,YWJj".to_string());
        assert!(plan_tool_result_images(
            &small,
            ToolImageScope::AnyImage,
            TOOL_RESULT_MEDIA_MOVED_MARKER,
        )
        .is_none());

        let large_url = format!(
            "data:image/png;base64,{}",
            "A".repeat(WHOLE_DATA_URL_MIN_BYTES)
        );
        let large = MessageContent::Text(large_url.clone());
        let plan = plan_tool_result_images(
            &large,
            ToolImageScope::AnyImage,
            TOOL_RESULT_MEDIA_MOVED_MARKER,
        )
        .expect("large whole-string data URL should be extracted");

        assert_eq!(plan.images[0].url, large_url);
        assert_eq!(
            cleaned_tool_result_text(&plan),
            TOOL_RESULT_MEDIA_MOVED_MARKER
        );
    }

    #[test]
    fn media_path_clamps_residual_base64_but_keeps_long_text() {
        let long_text = "ordinary OCR text with spaces. ".repeat(1000);
        let content = MessageContent::Text(
            json!({
                "content": [
                    {
                        "type": "input_image",
                        "image_url": "data:image/png;base64,IMAGE"
                    },
                    {"type": "text", "text": long_text},
                    {"type": "video", "data": "A".repeat(20_000)}
                ]
            })
            .to_string(),
        );

        let plan = plan_tool_result_images(
            &content,
            ToolImageScope::AnyImage,
            TOOL_RESULT_MEDIA_ATTACHED_MARKER,
        )
        .expect("image should enable bounded cleanup");
        let cleaned = cleaned_tool_result_text(&plan);

        assert!(cleaned.contains("ordinary OCR text with spaces"));
        assert!(cleaned.contains("[ai-toolbox: omitted 20000 bytes]"));
        assert!(!cleaned.contains(&"A".repeat(64)));
    }

    #[test]
    fn no_media_returns_none_without_rewriting_content() {
        let content = MessageContent::Text("{ \"status\": \"ok\" }".to_string());
        assert!(plan_tool_result_images(
            &content,
            ToolImageScope::AnyImage,
            TOOL_RESULT_MEDIA_ATTACHED_MARKER,
        )
        .is_none());
    }
}
