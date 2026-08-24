//! Minimal YAML frontmatter parser for SKILL.md files.
//!
//! Only handles the subset used by skill frontmatter: top-level `key: value`
//! pairs between `---` markers, with plain/quoted scalars and block scalars
//! (`|`, `>`, and their `-`/`+` chomping variants). Block content is collected
//! from the indented continuation lines so a `description: |` value does not
//! leak the literal `|` indicator into the cached description.

use std::path::Path;

/// Parse the `name` and `description` fields from a SKILL.md YAML frontmatter.
/// Returns `(name, description)`.
pub fn parse_skill_md_frontmatter(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    parse_frontmatter_text(&text)
}

/// Parse name/description from frontmatter text (the raw SKILL.md content).
pub fn parse_frontmatter_text(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }

    let mut name = None;
    let mut description = None;
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("name:") {
            let v = normalize_scalar(value);
            if !v.is_empty() {
                name = Some(v);
            }
        } else if let Some(value) = trimmed.strip_prefix("description:") {
            let v = normalize_scalar(value);
            if is_block_scalar_indicator(&v) {
                if let Some(block) = collect_block_scalar(&mut lines) {
                    if !block.is_empty() {
                        description = Some(block);
                    }
                }
            } else if !v.is_empty() {
                description = Some(v);
            }
        }
    }
    (name, description)
}

fn normalize_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

/// A YAML block scalar indicator: `|`, `>`, optionally followed by `-`/`+` or a
/// digit (e.g. `|-`, `>+`, `|2-`). Such a value means the real content lives
/// on the following indented lines.
fn is_block_scalar_indicator(value: &str) -> bool {
    let v = value.trim();
    v.starts_with('|') || v.starts_with('>')
}

/// Collect the indented continuation lines of a block scalar. Stops at the
/// first non-blank, non-indented line or the closing `---`. The content is
/// returned joined with newlines (common indentation stripped per line); the
/// caller decides whether to flatten it for one-line display.
fn collect_block_scalar<'a>(lines: &mut std::str::Lines<'a>) -> Option<String> {
    let mut content_lines: Vec<String> = Vec::new();
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        // A block content line is indented (leading whitespace) or blank.
        // A non-blank, non-indented line ends the block.
        if !line.starts_with(|c: char| c.is_whitespace()) && !trimmed.is_empty() {
            break;
        }
        content_lines.push(trimmed.to_string());
    }
    // Strip trailing blank lines (noise for display).
    while content_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        content_lines.pop();
    }
    if content_lines.iter().all(|s| s.is_empty()) {
        return None;
    }
    Some(content_lines.join("\n").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_scalar() {
        let text = "---\nname: review\ndescription: Review frontend code\n---\n";
        let (name, desc) = parse_frontmatter_text(text);
        assert_eq!(name.as_deref(), Some("review"));
        assert_eq!(desc.as_deref(), Some("Review frontend code"));
    }

    #[test]
    fn parses_quoted_scalar() {
        let text = "---\nname: \"foo\"\ndescription: \"A quoted desc\"\n---\n";
        let (name, desc) = parse_frontmatter_text(text);
        assert_eq!(name.as_deref(), Some("foo"));
        assert_eq!(desc.as_deref(), Some("A quoted desc"));
    }

    #[test]
    fn parses_literal_block_scalar_pipe() {
        // Mimics the MiniMax frontend-dev SKILL.md: `description: |` followed
        // by indented continuation lines. The `|` must NOT leak into the value.
        let text = concat!(
            "---\n",
            "name: frontend-dev\n",
            "description: |\n",
            "  Full-stack frontend development combining premium UI design, cinematic animations,\n",
            "  AI-generated media assets, persuasive copywriting, and visual art.\n",
            "license: MIT\n",
            "---\n",
        );
        let (name, desc) = parse_frontmatter_text(text);
        assert_eq!(name.as_deref(), Some("frontend-dev"));
        let d = desc.expect("description");
        assert!(
            !d.contains('|'),
            "block indicator must not leak: {d}"
        );
        assert!(d.contains("Full-stack frontend development"));
        assert!(d.contains("cinematic animations"));
        assert!(d.contains("visual art."));
    }

    #[test]
    fn parses_folded_block_scalar_gt() {
        let text = concat!(
            "---\n",
            "name: foo\n",
            "description: >\n",
            "  First line\n",
            "  second line\n",
            "---\n",
        );
        let (name, desc) = parse_frontmatter_text(text);
        assert_eq!(name.as_deref(), Some("foo"));
        let d = desc.expect("description");
        assert!(!d.contains('>'));
        assert!(d.contains("First line"));
        assert!(d.contains("second line"));
    }

    #[test]
    fn parses_block_scalar_with_chomping_indicator() {
        // `|-` strips the trailing newline.
        let text = concat!(
            "---\n",
            "name: foo\n",
            "description: |-\n",
            "  line one\n",
            "  line two\n",
            "---\n",
        );
        let (_, desc) = parse_frontmatter_text(text);
        let d = desc.expect("description");
        assert!(!d.contains('|'));
        assert!(d.contains("line one"));
        assert!(d.contains("line two"));
    }

    #[test]
    fn missing_frontmatter_returns_none() {
        let text = "# just a heading, no frontmatter\nsome body\n";
        let (name, desc) = parse_frontmatter_text(text);
        assert_eq!(name, None);
        assert_eq!(desc, None);
    }
}
