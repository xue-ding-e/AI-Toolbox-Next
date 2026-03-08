use std::fs;
use std::path::Path;

pub fn read_prompt_content_file(path: &Path, product_name: &str) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let prompt_content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} prompt file: {}", product_name, e))?;
    let prompt_content = prompt_content.trim().to_string();

    if prompt_content.is_empty() {
        return Ok(None);
    }

    Ok(Some(prompt_content))
}

pub fn write_prompt_content_file(
    path: &Path,
    prompt_content: Option<&str>,
    product_name: &str,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create {} prompt directory: {}", product_name, e)
            })?;
        }
    }

    let content = prompt_content
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");

    fs::write(path, content)
        .map_err(|e| format!("Failed to write {} prompt file: {}", product_name, e))?;

    Ok(())
}
