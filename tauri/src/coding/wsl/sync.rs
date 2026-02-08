use std::path::Path;
use std::process::Command;
use super::types::{FileMapping, SyncResult, WSLDetectResult};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Windows CREATE_NO_WINDOW flag to prevent console window from appearing
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Create a WSL command with proper flags for Windows GUI apps
/// This prevents console windows from flashing when running in release mode
fn create_wsl_command() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new("wsl");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Decode WSL command output which may be UTF-16 LE (Windows) or UTF-8 (Linux)
/// Also strips null characters to prevent SurrealDB panics
fn decode_wsl_output(bytes: &[u8]) -> String {
    let result = if bytes.len() >= 2 && bytes[1] == 0 {
        // UTF-16 LE encoded (check for null bytes after ASCII chars)
        let utf16_data: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&utf16_data)
    } else {
        // UTF-8 encoded
        String::from_utf8_lossy(bytes).to_string()
    };
    // Strip null characters to prevent SurrealDB panics
    result.replace('\0', "")
}

/// Get the effective distro to use: if configured distro doesn't exist, 
/// try to find a matching one or use the first available distro
pub fn get_effective_distro(configured_distro: &str) -> Result<String, String> {
    let distros = get_wsl_distros()?;
    
    if distros.is_empty() {
        return Err("No WSL distros available".to_string());
    }
    
    // Check if configured distro exists exactly
    if distros.iter().any(|d| d == configured_distro) {
        return Ok(configured_distro.to_string());
    }
    
    // Try to find a distro that starts with the configured name (e.g., "Ubuntu" matches "Ubuntu-22.04")
    if let Some(matching) = distros.iter().find(|d| d.starts_with(configured_distro)) {
        log::info!(
            "WSL distro '{}' not found, using '{}' instead",
            configured_distro,
            matching
        );
        return Ok(matching.clone());
    }
    
    // Try to find a distro where configured name starts with it (e.g., "Ubuntu-22.04" matches "Ubuntu")
    if let Some(matching) = distros.iter().find(|d| configured_distro.starts_with(d.as_str())) {
        log::info!(
            "WSL distro '{}' not found, using '{}' instead",
            configured_distro,
            matching
        );
        return Ok(matching.clone());
    }
    
    // Fall back to first available distro
    let first = distros.first().unwrap().clone();
    log::warn!(
        "WSL distro '{}' not found, falling back to '{}'",
        configured_distro,
        first
    );
    Ok(first)
}

/// Detect if WSL is available and get list of distros
pub fn detect_wsl() -> WSLDetectResult {
    // Check if WSL is installed by running wsl --status
    let output = create_wsl_command()
        .args(["--status"])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                // WSL is available, get distros
                match get_wsl_distros() {
                    Ok(distros) => WSLDetectResult {
                        available: true,
                        distros,
                        error: None,
                    },
                    Err(e) => WSLDetectResult {
                        available: true,
                        distros: vec![],
                        error: Some(e),
                    },
                }
            } else {
                WSLDetectResult {
                    available: false,
                    distros: vec![],
                    error: Some("WSL command failed".to_string()),
                }
            }
        }
        Err(e) => WSLDetectResult {
            available: false,
            distros: vec![],
            error: Some(format!("Failed to run WSL command: {}", e)),
        },
    }
}

/// Get list of available WSL distros
pub fn get_wsl_distros() -> Result<Vec<String>, String> {
    let output = create_wsl_command()
        .args(["--list", "--quiet"])
        .output()
        .map_err(|e| format!("Failed to run wsl --list: {}", e))?;

    if !output.status.success() {
        return Err("WSL list command failed".to_string());
    }

    // wsl --list outputs UTF-16 LE on Windows
    let stdout = if output.stdout.len() >= 2 && output.stdout[1] == 0 {
        // UTF-16 LE encoded (BOM or check for null bytes after ASCII)
        let utf16_data: Vec<u16> = output.stdout
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&utf16_data)
    } else {
        // UTF-8 encoded
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    let distros: Vec<String> = stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    Ok(distros)
}

/// Get running state of a specific WSL distro
/// Returns: "Running", "Stopped", or "Unknown"
pub fn get_wsl_distro_state(distro: &str) -> String {
    let output = match create_wsl_command()
        .args(["--list", "--verbose"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return "Unknown".to_string(),
    };

    if !output.status.success() {
        return "Unknown".to_string();
    }

    // wsl --list outputs UTF-16 LE on Windows
    let stdout = if output.stdout.len() >= 2 && output.stdout[1] == 0 {
        let utf16_data: Vec<u16> = output.stdout
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&utf16_data)
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    // Parse output to find the distro's state
    // Format: "  NAME                   STATE           VERSION"
    //         "* Ubuntu                 Running         2"
    let mut is_header_skipped = false;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Skip header line (starts with "NAME")
        if !is_header_skipped {
            let trimmed = line.trim_start();
            if trimmed.starts_with("NAME") {
                is_header_skipped = true;
            }
            continue;
        }

        // Parse line: [*] NAME STATE VERSION
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        // Get name and state from the line
        // Format: NAME STATE VERSION or * NAME STATE VERSION
        let (name, state) = if parts.len() >= 4 && parts[0] == "*" {
            // Default distro with * prefix
            (parts[1], parts[2])
        } else {
            // Normal distro
            (parts[0], parts[1])
        };

        if name == distro {
            // Normalize state to "Running" or "Stopped"
            return match state.to_lowercase().as_str() {
                "running" => "Running".to_string(),
                _ => "Stopped".to_string(),
            };
        }
    }

    "Unknown".to_string()
}

/// Expand environment variables in a path
pub fn expand_env_vars(path: &str) -> Result<String, String> {
    let mut result = path.to_string();

    // Common Windows environment variables
    let vars = [
        ("USERPROFILE", std::env::var("USERPROFILE")),
        ("APPDATA", std::env::var("APPDATA")),
        ("LOCALAPPDATA", std::env::var("LOCALAPPDATA")),
        ("HOME", std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))),
    ];

    for (var, value) in vars {
        if let Ok(val) = value {
            result = result.replace(&format!("%{}%", var), &val);
            result = result.replace(&format!("${}", var), &val);
        }
    }

    Ok(result)
}

/// Convert Windows path to WSL path
pub fn windows_to_wsl_path(windows_path: &str) -> Result<String, String> {
    let expanded = expand_env_vars(windows_path)?;

    // Convert C:\Users\... to /mnt/c/Users/...
    let wsl_path = expanded.replace('\\', "/");

    // Convert drive letter (e.g., C: to /mnt/c)
    if wsl_path.len() >= 2 && wsl_path.as_bytes()[1] == b':' {
        let drive = wsl_path.chars().next().unwrap().to_lowercase();
        let rest = &wsl_path[2..];
        // rest starts with "/" (e.g., "/Users/..."), so no extra "/" needed
        return Ok(format!("/mnt/{}{}", drive, rest));
    }

    Ok(wsl_path)
}

/// Sync a single file mapping to WSL
pub fn sync_file_mapping(mapping: &FileMapping, distro: &str) -> Result<Vec<String>, String> {
    let windows_path = expand_env_vars(&mapping.windows_path)?;

    if mapping.is_directory {
        // Directory mode: copy entire directory
        if !Path::new(&windows_path).exists() {
            return Ok(vec![]);
        }
        sync_directory(&windows_path, &mapping.wsl_path, distro)
    } else if mapping.is_pattern {
        // Pattern mode: handle wildcards
        sync_pattern_files(&windows_path, &mapping.wsl_path, distro)
    } else {
        // Single file mode
        if !Path::new(&windows_path).exists() {
            return Ok(vec![]);
        }
        sync_single_file(&windows_path, &mapping.wsl_path, distro)
    }
}

/// Sync a single file
pub fn sync_single_file(windows_path: &str, wsl_path: &str, distro: &str) -> Result<Vec<String>, String> {
    let wsl_source_path = windows_to_wsl_path(windows_path)?;

    // Expand ~ in WSL path
    let wsl_target_path = wsl_path.replace("~", "$HOME");

    // Create the WSL command
    let command = format!(
        "mkdir -p \"$(dirname \"{}\")\" && cp -f \"{}\" \"{}\"",
        wsl_target_path, wsl_source_path, wsl_target_path
    );

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to execute WSL command: {}", e))?;

    if output.status.success() {
        Ok(vec![format!("{} -> {}", windows_path, wsl_path)])
    } else {
        let stderr = decode_wsl_output(&output.stderr);
        Err(format!("WSL sync failed: {}", stderr.trim()))
    }
}

/// Sync a directory (recursive copy)
pub fn sync_directory(windows_path: &str, wsl_path: &str, distro: &str) -> Result<Vec<String>, String> {
    let wsl_source_path = windows_to_wsl_path(windows_path)?;

    // Expand ~ in WSL path
    let wsl_target_path = wsl_path.replace("~", "$HOME");

    // First, check if source path exists in WSL
    let check_command = format!("if [ -e \"{}\" ]; then echo exists; else echo notfound; fi", wsl_source_path);
    let check_output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &check_command])
        .output()
        .map_err(|e| format!("Failed to check WSL source path: {}", e))?;

    let check_result = decode_wsl_output(&check_output.stdout).trim().to_string();
    if check_result == "notfound" {
        let source_path_expanded = std::path::Path::new(windows_path);
        if source_path_expanded.exists() {
            return Err(format!(
                "WSL directory sync failed: Windows path '{}' does not exist or is not accessible from WSL. \
                 Converted WSL path: '{}'. Please check if WSL can access Windows drives.",
                windows_path, wsl_source_path
            ));
        } else {
            return Ok(vec![]); // Source doesn't exist, skip sync
        }
    }

    // Create the WSL command to copy directory recursively
    // Use cp -rL to copy directory contents and dereference symlinks
    // -L flag ensures symlinks are followed and actual file contents are copied
    // This is important because Windows skills may be managed via symlinks/hardlinks
    let command = format!(
        "mkdir -p \"$(dirname \"{}\")\" && rm -rf \"{}\" && cp -rL \"{}\" \"{}\" 2>&1",
        wsl_target_path, wsl_target_path, wsl_source_path, wsl_target_path
    );

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to execute WSL directory command: {}", e))?;

    if output.status.success() {
        Ok(vec![format!("{} -> {}", windows_path, wsl_path)])
    } else {
        let stderr = decode_wsl_output(&output.stderr).trim().to_string();
        let stdout = decode_wsl_output(&output.stdout).trim().to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // Provide more detailed error information
        if stderr.is_empty() && stdout.is_empty() {
            Err(format!(
                "WSL directory sync failed: Command returned exit code {} but produced no output. \
                 Source: '{}', Target WSL: '{}', WSL converted source: '{}'",
                exit_code, windows_path, wsl_target_path, wsl_source_path
            ))
        } else if !stderr.is_empty() {
            Err(format!(
                "WSL directory sync failed: {}. Source: '{}', Target: '{}', Exit code: {}",
                stderr, windows_path, wsl_path, exit_code
            ))
        } else {
            Err(format!(
                "WSL directory sync failed: {}. Source: '{}', Target: '{}', Exit code: {}",
                stdout, windows_path, wsl_path, exit_code
            ))
        }
    }
}

/// Sync files matching a pattern
pub fn sync_pattern_files(windows_pattern: &str, wsl_target_dir: &str, distro: &str) -> Result<Vec<String>, String> {
    // Convert Windows path to WSL path
    let wsl_source_dir = windows_to_wsl_path(windows_pattern)?;

    // Extract the directory and pattern
    let (wsl_source_base, pattern) = if let Some(last_slash) = wsl_source_dir.rfind('/') {
        let base = &wsl_source_dir[..last_slash];
        let pattern = &wsl_source_dir[last_slash + 1..];
        (base, pattern)
    } else {
        (".", &wsl_source_dir[..])
    };

    // Expand ~ in WSL path
    let wsl_target_dir_expanded = wsl_target_dir.replace("~", "$HOME");

    // Create the WSL command to sync pattern files
    let command = format!(
        "mkdir -p \"{}\" && \
         if [ -f \"{}\"/{} ]; then \
             cp -f \"{}\"/{} \"{}\" && \
             echo \"synced\"; \
         else \
             shopt -s nullglob dotglob; \
             files=\"{}\"/{}; \
             if [ -n \"$files\" ]; then \
                 cp -f $files \"{}\" 2>/dev/null && echo \"synced\" || true; \
             fi; \
         fi",
        wsl_target_dir_expanded,
        wsl_source_base, pattern,
        wsl_source_base, pattern,
        wsl_target_dir_expanded,
        wsl_source_base, pattern,
        wsl_target_dir_expanded
    );

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to execute WSL pattern command: {}", e))?;

    if output.status.success() {
        Ok(vec![format!("{} -> {}", windows_pattern, wsl_target_dir)])
    } else {
        let stderr = decode_wsl_output(&output.stderr).trim().to_string();
        let stdout = decode_wsl_output(&output.stdout).trim().to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // Pattern sync failures are often OK (just no files matching)
        if stderr.contains("cannot stat") || stderr.contains("No such file") || stderr.contains("No such file or directory") {
            Ok(vec![])
        } else if stderr.is_empty() && stdout.is_empty() {
            // Silent failure - might just be no files matching pattern
            Ok(vec![])
        } else if !stderr.is_empty() {
            Err(format!("WSL pattern sync failed: {}. Pattern: '{}', Target: '{}', Exit code: {}",
                stderr, windows_pattern, wsl_target_dir, exit_code))
        } else {
            Err(format!("WSL pattern sync failed: {}. Pattern: '{}', Target: '{}', Exit code: {}",
                stdout, windows_pattern, wsl_target_dir, exit_code))
        }
    }
}

/// Sync all enabled file mappings for a module (or all modules if module is None)
pub fn sync_mappings(
    mappings: &[FileMapping],
    distro: &str,
    module_filter: Option<&str>,
) -> SyncResult {
    let mut synced_files = vec![];
    let mut skipped_files = vec![];
    let mut errors = vec![];

    let filtered_mappings: Vec<_> = mappings
        .iter()
        .filter(|m| m.enabled)
        .filter(|m| module_filter.is_none() || Some(m.module.as_str()) == module_filter)
        .collect();

    for mapping in filtered_mappings {
        match sync_file_mapping(mapping, distro) {
            Ok(files) if files.is_empty() => {
                skipped_files.push(mapping.name.clone());
            }
            Ok(files) => {
                synced_files.extend(files);
            }
            Err(e) => {
                errors.push(format!("{}: {}", mapping.name, e));
            }
        }
    }

    SyncResult {
        success: errors.is_empty(),
        synced_files,
        skipped_files,
        errors,
    }
}

// ============================================================================
// WSL File Operations
// ============================================================================

/// Read a file from WSL and try to auto-convert GBK/GB2312 to UTF-8
pub fn read_wsl_file(distro: &str, wsl_path: &str) -> Result<String, String> {
    let wsl_target = wsl_path.replace("~", "$HOME");

    // Try reading as UTF-8 first
    let command = format!(
        "if [ -f \"{}\" ]; then cat \"{}\"; else echo ''; fi",
        wsl_target, wsl_target
    );

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to read WSL file: {}", e))?;

    // Check for WSL errors (distro not found, etc.) - these come as UTF-16 on Windows
    if !output.status.success() {
        let stderr = decode_wsl_output(&output.stderr);
        if stderr.contains("WSL_E_DISTRO_NOT_FOUND") || stderr.contains("not found") {
            return Err(format!("WSL distro '{}' not found", distro));
        }
        return Err(format!("WSL command failed: {}", stderr.trim()));
    }

    let content = String::from_utf8_lossy(&output.stdout).to_string();

    // Check if file has non-UTF-8 characters (contains replacement character U+FFFD)
    if content.contains('\u{FFFD}') {
        log::warn!("File {} appears to be non-UTF-8 (GBK/GB2312), attempting auto-conversion...", wsl_path);

        // Try to convert using iconv -f GBK -t UTF-8
        let convert_command = format!(
            "if [ -f \"{}\" ]; then iconv -f GBK -t UTF-8 \"{}\" 2>/dev/null || cat \"{}\"; else echo ''; fi",
            wsl_target, wsl_target, wsl_target
        );

        let convert_output = create_wsl_command()
            .args(["-d", distro, "--exec", "bash", "-c", &convert_command])
            .output()
            .map_err(|e| format!("Failed to convert WSL file encoding: {}", e))?;

        let converted_content = String::from_utf8_lossy(&convert_output.stdout).to_string();

        // If conversion succeeded (no U+FFFD), use converted content
        if !converted_content.contains('\u{FFFD}') && !converted_content.is_empty() {
            log::info!("Auto-converted {} from GBK to UTF-8", wsl_path);
            return Ok(converted_content);
        } else {
            log::warn!("Auto-conversion failed, file might be in unsupported encoding or corrupted");
        }
    }

    Ok(content)
}

/// Write content to a WSL file
pub fn write_wsl_file(distro: &str, wsl_path: &str, content: &str) -> Result<(), String> {
    let wsl_target = wsl_path.replace("~", "$HOME");

    // Use heredoc to write content, avoiding escape issues
    let command = format!(
        "mkdir -p \"$(dirname \"{}\")\" && cat > \"{}\"",
        wsl_target, wsl_target
    );

    let mut child = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn WSL command: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for WSL command: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("WSL write command failed".to_string())
    }
}

/// Create a symlink in WSL
pub fn create_wsl_symlink(distro: &str, target: &str, link_path: &str) -> Result<(), String> {
    let target_expanded = target.replace("~", "$HOME");
    let link_expanded = link_path.replace("~", "$HOME");

    let command = format!(
        "mkdir -p \"$(dirname \"{}\")\" && rm -rf \"{}\" && ln -s \"{}\" \"{}\"",
        link_expanded, link_expanded, target_expanded, link_expanded
    );

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to create symlink: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = decode_wsl_output(&output.stderr);
        Err(format!("WSL symlink failed: {}", stderr.trim()))
    }
}

/// Remove a file or directory in WSL
pub fn remove_wsl_path(distro: &str, wsl_path: &str) -> Result<(), String> {
    let wsl_target = wsl_path.replace("~", "$HOME");
    let command = format!("rm -rf \"{}\"", wsl_target);

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to remove WSL path: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = decode_wsl_output(&output.stderr);
        Err(format!("WSL remove failed: {}", stderr.trim()))
    }
}

/// List subdirectories in a WSL directory
pub fn list_wsl_dir(distro: &str, wsl_path: &str) -> Result<Vec<String>, String> {
    let wsl_target = wsl_path.replace("~", "$HOME");
    let command = format!(
        "if [ -d \"{}\" ]; then ls -1 \"{}\"; fi",
        wsl_target, wsl_target
    );

    let output = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
        .map_err(|e| format!("Failed to list WSL dir: {}", e))?;

    Ok(decode_wsl_output(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Check if a WSL symlink exists and points to the expected target
pub fn check_wsl_symlink_exists(distro: &str, link_path: &str, expected_target: &str) -> bool {
    let link_expanded = link_path.replace("~", "$HOME");
    let target_expanded = expected_target.replace("~", "$HOME");
    let command = format!(
        "[ -L \"{}\" ] && [ \"$(readlink \"{}\")\" = \"{}\" ] && echo yes || echo no",
        link_expanded, link_expanded, target_expanded
    );

    if let Ok(output) = create_wsl_command()
        .args(["-d", distro, "--exec", "bash", "-c", &command])
        .output()
    {
        decode_wsl_output(&output.stdout).trim() == "yes"
    } else {
        false
    }
}
