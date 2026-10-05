//! JSONC 顶层键原地补丁助手。
//!
//! `~/.omo/omo.jsonc` 是 JSONC（允许 `//` / `/* */` 注释与尾逗号），且是 OpenCode 插件版
//! 与 OmO Native **共用的同一个文件**：插件版读 `[opencode]` 块，Native 读 `[native]` 块。
//! 用户会在文件里写注释，所以任何写入都必须是「保留注释的原地补丁」，不能整体序列化重写。
//!
//! 本模块从 `oh_my_openagent/commands.rs` 抽出，供 `oh_my_openagent`（`[opencode]` 块）
//! 与 `omo_native`（`[native]` 块）共用。抽出的行为与原实现逐字一致。

/// JSONC 顶层键的字符区间。
pub struct TopLevelKey {
    pub name: String,
    /// 键名开引号的下标。
    pub key_start: usize,
    /// 值的首字符下标。
    pub value_start: usize,
    /// 值之后的尾逗号（或根闭合大括号）下标。
    pub value_end: usize,
}

/// 扫描 JSONC 文档的顶层键，识别字符串与 `//` / `/* */` 注释。
/// 返回键列表与根对象闭合大括号的下标。
pub fn scan_top_level_keys(text: &str) -> (Vec<TopLevelKey>, usize) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut keys = Vec::new();
    let mut root_close = n;
    let mut i = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line = false;
    let mut in_block = false;

    while i < n {
        let c = bytes[i];
        if in_line {
            if c == b'\n' {
                in_line = false;
            }
            i += 1;
            continue;
        }
        if in_block {
            if c == b'*' && bytes.get(i + 1) == Some(&b'/') {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                in_line = true;
                i += 2;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                in_block = true;
                i += 2;
                continue;
            }
            b'"' => {
                let str_start = i;
                i += 1;
                let mut str_end = i;
                let mut str_in = true;
                let mut str_esc = false;
                while str_end < n {
                    let sc = bytes[str_end];
                    if str_esc {
                        str_esc = false;
                    } else if sc == b'\\' {
                        str_esc = true;
                    } else if sc == b'"' {
                        str_in = false;
                        str_end += 1;
                        break;
                    }
                    str_end += 1;
                }
                if str_in {
                    i = str_end;
                    continue;
                }
                let name = &text[str_start + 1..str_end - 1];
                // 跳过空白与注释，判断这个字符串是不是键（后面跟着 `:`）。
                let mut j = str_end;
                loop {
                    while j < n && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j + 1 < n && bytes[j] == b'/' && bytes[j + 1] == b'/' {
                        while j < n && bytes[j] != b'\n' {
                            j += 1;
                        }
                        continue;
                    }
                    if j + 1 < n && bytes[j] == b'/' && bytes[j + 1] == b'*' {
                        j += 2;
                        while j + 1 < n && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                            j += 1;
                        }
                        j += 2;
                        continue;
                    }
                    break;
                }
                if j < n && bytes[j] == b':' && depth == 1 {
                    let mut vs = j + 1;
                    while vs < n && bytes[vs].is_ascii_whitespace() {
                        vs += 1;
                    }
                    let ve = scan_value_end(text, vs);
                    keys.push(TopLevelKey {
                        name: name.to_string(),
                        key_start: str_start,
                        value_start: vs,
                        value_end: ve,
                    });
                    i = ve;
                    continue;
                }
                i = str_end;
                continue;
            }
            b'{' => {
                depth += 1;
                i += 1;
                continue;
            }
            b'}' => {
                if depth == 1 {
                    root_close = i;
                    i = n;
                    continue;
                }
                if depth > 0 {
                    depth -= 1;
                }
                i += 1;
                continue;
            }
            b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b']' => {
                if depth > 0 {
                    depth -= 1;
                }
                i += 1;
                continue;
            }
            _ => {
                i += 1;
                continue;
            }
        }
    }
    (keys, root_close)
}

/// 从 `start` 起扫描一个 JSONC 值，返回值的结束下标
/// （即指向尾逗号或根闭合大括号）。
fn scan_value_end(text: &str, mut i: usize) -> usize {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line = false;
    let mut in_block = false;
    while i < n {
        let c = bytes[i];
        if in_line {
            if c == b'\n' {
                in_line = false;
            }
            i += 1;
            continue;
        }
        if in_block {
            if c == b'*' && bytes.get(i + 1) == Some(&b'/') {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                in_line = true;
                i += 2;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                in_block = true;
                i += 2;
                continue;
            }
            b'"' => {
                in_string = true;
                i += 1;
                continue;
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
                continue;
            }
            b'}' | b']' => {
                if depth == 0 {
                    return i;
                }
                depth -= 1;
                i += 1;
                continue;
            }
            b',' if depth == 0 => {
                return i;
            }
            _ => {
                i += 1;
                continue;
            }
        }
    }
    i
}

/// 原地替换或插入一个顶层块键，保留其他顶层块与全部注释。
///
/// `block_name` 形如 `"[native]"`；`block_json` 是该块值的 JSON 文本。
/// `extras` 是额外要确保存在的顶层键（键名 → 值的 JSON 文本），已存在则替换。
///
/// 幂等：对同一份文本连续写入同一个块，结果逐字相同（值后原有的空白被保留，
/// 不会在第一次写入时被吞掉、第二次写入时再变化）。
pub fn patch_top_level_block(
    raw: &str,
    block_name: &str,
    block_json: &str,
    extras: &[(String, String)],
) -> String {
    let (keys, root_close) = scan_top_level_keys(raw);
    let find = |name: &str| keys.iter().find(|k| k.name == name);

    // 每项 = (替换起点, 替换终点, 光标终点, 新文本)。
    // `scan_value_end` 会一路扫到分隔符（尾逗号或根大括号），把值与分隔符之间的
    // 空白也算进区间。替换终点要停在值本身，但光标必须走到分隔符，且新文本要把
    // 那段空白原样带回来——否则第一次写入会吃掉它，第二次写入的产物就与第一次不同。
    let mut replacements: Vec<(usize, usize, usize, String)> = Vec::new();
    let mut insertions: Vec<String> = Vec::new();

    let trailing_whitespace_start = |end: usize| -> usize {
        let bytes = raw.as_bytes();
        let mut trimmed = end;
        while trimmed > 0 && bytes[trimmed - 1].is_ascii_whitespace() {
            trimmed -= 1;
        }
        trimmed
    };

    let mut push_replacement = |k: &TopLevelKey, value_json: &str| {
        let value_trimmed_end = trailing_whitespace_start(k.value_end);
        replacements.push((
            k.value_start,
            value_trimmed_end,
            k.value_end,
            format!("{}{}", value_json, &raw[value_trimmed_end..k.value_end]),
        ));
    };

    if let Some(k) = find(block_name) {
        push_replacement(k, block_json);
    } else {
        insertions.push(format!("\"{}\": {}", block_name, block_json));
    }

    for (name, value_json) in extras {
        if let Some(k) = find(name) {
            push_replacement(k, value_json);
        } else {
            insertions.push(format!("\"{}\": {}", name, value_json));
        }
    }

    replacements.sort_by_key(|(start, _, _, _)| *start);

    let mut out = String::with_capacity(raw.len() + 256);
    let mut cursor = 0usize;
    for (start, _end, cursor_end, new_text) in &replacements {
        out.push_str(&raw[cursor..*start]);
        out.push_str(new_text);
        cursor = *cursor_end;
    }
    out.push_str(&raw[cursor..root_close]);

    // 缺失的键插在根大括号之前，逗号分隔、每行一个。
    if !insertions.is_empty() {
        // 先把已有内容的尾部空白裁掉，再补逗号，避免留下 `} ,` 这种带空格的产物。
        let trimmed_len = out.trim_end().len();
        out.truncate(trimmed_len);
        let needs_comma = !out.is_empty() && !out.ends_with(',') && !out.ends_with('{');
        if needs_comma {
            out.push(',');
        }
        for (index, insertion) in insertions.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("\n  ");
            out.push_str(insertion);
        }
        out.push('\n');
    }

    out.push_str(&raw[root_close..]);
    out
}

/// 从 JSONC 文本里原地删除一个顶层键（保留注释），同时处理相邻逗号。
/// 返回补丁后的文本与是否找到该键。
pub fn remove_top_level_key(text: &str, name: &str) -> (String, bool) {
    let (keys, root_close) = scan_top_level_keys(text);
    let Some(k) = keys.iter().find(|k| k.name == name) else {
        return (text.to_string(), false);
    };
    let bytes = text.as_bytes();
    let has_comma = k.value_end < root_close && bytes[k.value_end] == b',';
    let mut out = String::with_capacity(text.len());
    if has_comma {
        // 删掉 [key_start, 逗号+1)，保留前一个逗号，后续键仍然合法。
        out.push_str(&text[..k.key_start]);
        out.push_str(&text[k.value_end + 1..]);
    } else {
        // 最后一个键：连前面的逗号一起删，避免在 `}` 前留下尾逗号。
        let mut start = k.key_start;
        let mut before = k.key_start;
        while before > 0 && bytes[before - 1].is_ascii_whitespace() {
            before -= 1;
        }
        if before > 0 && bytes[before - 1] == b',' {
            start = before - 1;
        }
        out.push_str(&text[..start]);
        out.push_str(&text[root_close..]);
    }
    (out, true)
}

/// 读取 JSONC 文本的顶层键名列表（用于判断「除控制键外是否还有内容」）。
pub fn top_level_key_names(text: &str) -> Vec<String> {
    scan_top_level_keys(text).0.into_iter().map(|k| k.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_replaces_existing_block_and_preserves_comments() {
        let raw = r#"{
  // 我的注释
  "codegraph": { "daemon": true },
  "[native]": { "agents": {} },
  "[opencode]": { "plugin": true }
}"#;
        let patched =
            patch_top_level_block(raw, "[native]", r#"{ "agents": { "explore": {} } }"#, &[]);
        let obj: serde_json::Value = json5::from_str(&patched).unwrap();
        assert_eq!(obj["[native]"]["agents"]["explore"], serde_json::json!({}));
        assert_eq!(obj["[opencode]"]["plugin"], serde_json::json!(true));
        assert!(patched.contains("// 我的注释"));
    }

    #[test]
    fn patch_inserts_missing_block_without_touching_others() {
        let raw = r#"{
  "[opencode]": { "plugin": true }
}"#;
        let patched = patch_top_level_block(raw, "[native]", r#"{ "agents": {} }"#, &[]);
        let obj: serde_json::Value = json5::from_str(&patched).unwrap();
        assert_eq!(obj["[native]"]["agents"], serde_json::json!({}));
        assert_eq!(obj["[opencode]"]["plugin"], serde_json::json!(true));
    }

    #[test]
    fn patch_updates_extras() {
        let raw = r#"{ "_migrations": ["old"] }"#;
        let patched = patch_top_level_block(
            raw,
            "[native]",
            "{}",
            &[("_migrations".to_string(), r#"["new"]"#.to_string())],
        );
        let obj: serde_json::Value = json5::from_str(&patched).unwrap();
        assert_eq!(obj["_migrations"], serde_json::json!(["new"]));
    }

    #[test]
    fn remove_handles_middle_and_last_keys() {
        let (middle, found) = remove_top_level_key(r#"{ "a": 1, "b": 2, "c": 3 }"#, "b");
        assert!(found);
        let obj: serde_json::Value = json5::from_str(&middle).unwrap();
        assert!(obj.get("b").is_none());
        assert_eq!(obj["a"], serde_json::json!(1));
        assert_eq!(obj["c"], serde_json::json!(3));

        let (last, found) = remove_top_level_key(r#"{ "a": 1, "b": 2 }"#, "b");
        assert!(found);
        let obj: serde_json::Value = json5::from_str(&last).unwrap();
        assert!(obj.get("b").is_none());
        assert_eq!(obj["a"], serde_json::json!(1));
    }

    #[test]
    fn remove_reports_missing_key() {
        let (out, found) = remove_top_level_key(r#"{ "a": 1 }"#, "missing");
        assert!(!found);
        assert_eq!(out, r#"{ "a": 1 }"#);
    }

    #[test]
    fn scan_ignores_keys_inside_nested_objects_and_strings() {
        let raw = r#"{
  "outer": { "inner": 1 },
  "text": "a \"fake\": key",
  "real": 2
}"#;
        let names = top_level_key_names(raw);
        assert_eq!(names, vec!["outer", "text", "real"]);
    }
}
