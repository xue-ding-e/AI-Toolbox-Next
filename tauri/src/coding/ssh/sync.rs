use std::path::Path;
use std::process::Command;
use super::types::{SSHConnection, SSHConnectionResult, SSHFileMapping, SyncResult};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Windows CREATE_NO_WINDOW flag to prevent console window from appearing
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ============================================================================
// SSH Command Builders
// ============================================================================

/// Build common SSH args for a connection
fn build_ssh_args(conn: &SSHConnection) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        conn.port.to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
    ];

    if conn.auth_method == "key" && !conn.private_key_path.is_empty() {
        args.push("-i".to_string());
        args.push(conn.private_key_path.clone());
        if conn.passphrase.is_empty() {
            args.push("-o".to_string());
            args.push("BatchMode=yes".to_string());
        }
    }

    args
}

/// Build SCP port args (uses -P uppercase for port)
fn build_scp_args(conn: &SSHConnection) -> Vec<String> {
    let mut args = vec![
        "-P".to_string(),
        conn.port.to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(),
        "ConnectTimeout=10".to_string(),
    ];

    if conn.auth_method == "key" && !conn.private_key_path.is_empty() {
        args.push("-i".to_string());
        args.push(conn.private_key_path.clone());
        if conn.passphrase.is_empty() {
            args.push("-o".to_string());
            args.push("BatchMode=yes".to_string());
        }
    }

    args
}

/// Create an SSH command for a connection
fn create_ssh_command(conn: &SSHConnection) -> Command {
    let target = format!("{}@{}", conn.username, conn.host);

    if conn.auth_method == "password" && !conn.password.is_empty() {
        let mut cmd = Command::new("sshpass");
        cmd.args(["-p", &conn.password, "ssh"]);
        for arg in build_ssh_args(conn) {
            cmd.arg(&arg);
        }
        cmd.arg(&target);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    } else {
        let mut cmd = Command::new("ssh");
        for arg in build_ssh_args(conn) {
            cmd.arg(&arg);
        }
        cmd.arg(&target);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
}

/// Create an SCP command for a connection
fn create_scp_command(conn: &SSHConnection) -> Command {
    if conn.auth_method == "password" && !conn.password.is_empty() {
        let mut cmd = Command::new("sshpass");
        cmd.args(["-p", &conn.password, "scp"]);
        for arg in build_scp_args(conn) {
            cmd.arg(&arg);
        }
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    } else {
        let mut cmd = Command::new("scp");
        for arg in build_scp_args(conn) {
            cmd.arg(&arg);
        }
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
}

// ============================================================================
// Connection Testing
// ============================================================================

/// Test SSH connection
pub fn test_connection(conn: &SSHConnection) -> SSHConnectionResult {
    let mut cmd = create_ssh_command(conn);
    cmd.args(["echo __connected__ && uname -a"]);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() && stdout.contains("__connected__") {
                let server_info = stdout
                    .lines()
                    .find(|line| !line.contains("__connected__"))
                    .map(|s| s.trim().to_string());
                SSHConnectionResult {
                    connected: true,
                    error: None,
                    server_info,
                }
            } else {
                SSHConnectionResult {
                    connected: false,
                    error: Some(if stderr.is_empty() {
                        "Connection failed".to_string()
                    } else {
                        stderr.trim().to_string()
                    }),
                    server_info: None,
                }
            }
        }
        Err(e) => SSHConnectionResult {
            connected: false,
            error: Some(format!("Failed to execute ssh command: {}", e)),
            server_info: None,
        },
    }
}

// ============================================================================
// Path Expansion
// ============================================================================

/// Expand local path: ~, $HOME, %USERPROFILE%
pub fn expand_local_path(path: &str) -> Result<String, String> {
    let mut result = path.to_string();

    // Expand ~ to home directory
    if result.starts_with("~/") || result == "~" {
        if let Some(home) = dirs::home_dir() {
            result = result.replacen("~", &home.to_string_lossy(), 1);
        }
    }

    // Common environment variables
    let vars = [
        ("USERPROFILE", std::env::var("USERPROFILE")),
        ("APPDATA", std::env::var("APPDATA")),
        ("LOCALAPPDATA", std::env::var("LOCALAPPDATA")),
        (
            "HOME",
            std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")),
        ),
    ];

    for (var, value) in vars {
        if let Ok(val) = value {
            result = result.replace(&format!("%{}%", var), &val);
            result = result.replace(&format!("${}", var), &val);
        }
    }

    Ok(result)
}

// ============================================================================
// File Sync Operations
// ============================================================================

/// Sync a single file to remote via scp
pub fn sync_single_file(
    local_path: &str,
    remote_path: &str,
    conn: &SSHConnection,
) -> Result<Vec<String>, String> {
    let expanded = expand_local_path(local_path)?;

    if !Path::new(&expanded).exists() {
        return Ok(vec![]);
    }

    let remote_target = remote_path.replace("~", "$HOME");
    let target = format!("{}@{}", conn.username, conn.host);

    // Create remote directory
    let mkdir_cmd = format!("mkdir -p \"$(dirname \"{}\")\"", remote_target);
    let mut ssh = create_ssh_command(conn);
    ssh.arg(&mkdir_cmd);
    let output = ssh
        .output()
        .map_err(|e| format!("Failed to create remote directory: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to create remote directory: {}", stderr.trim()));
    }

    // SCP the file
    let remote_dest = format!("{}:{}", target, remote_path);
    let mut scp = create_scp_command(conn);
    scp.args([&expanded, &remote_dest]);

    let output = scp
        .output()
        .map_err(|e| format!("Failed to execute scp: {}", e))?;

    if output.status.success() {
        Ok(vec![format!("{} -> {}", local_path, remote_path)])
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("SCP failed: {}", stderr.trim()))
    }
}

/// Sync a directory to remote via scp -r
pub fn sync_directory(
    local_path: &str,
    remote_path: &str,
    conn: &SSHConnection,
) -> Result<Vec<String>, String> {
    let expanded = expand_local_path(local_path)?;

    if !Path::new(&expanded).exists() {
        return Ok(vec![]);
    }

    let remote_target = remote_path.replace("~", "$HOME");
    let target = format!("{}@{}", conn.username, conn.host);

    // Create remote parent directory and remove existing
    let mkdir_cmd = format!(
        "mkdir -p \"$(dirname \"{}\")\" && rm -rf \"{}\"",
        remote_target, remote_target
    );
    let mut ssh = create_ssh_command(conn);
    ssh.arg(&mkdir_cmd);
    let output = ssh
        .output()
        .map_err(|e| format!("Failed to prepare remote directory: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to prepare remote directory: {}",
            stderr.trim()
        ));
    }

    // SCP -r the directory
    let remote_dest = format!("{}:{}", target, remote_path);
    let mut scp = create_scp_command(conn);
    scp.args(["-r", &expanded, &remote_dest]);

    let output = scp
        .output()
        .map_err(|e| format!("Failed to execute scp: {}", e))?;

    if output.status.success() {
        Ok(vec![format!("{} -> {}", local_path, remote_path)])
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("SCP directory sync failed: {}", stderr.trim()))
    }
}

/// Sync files matching a glob pattern to remote
pub fn sync_pattern_files(
    local_pattern: &str,
    remote_dir: &str,
    conn: &SSHConnection,
) -> Result<Vec<String>, String> {
    let expanded = expand_local_path(local_pattern)?;

    // Use glob to find matching files
    let matches: Vec<_> = glob::glob(&expanded)
        .map_err(|e| format!("Invalid glob pattern: {}", e))?
        .filter_map(|entry| entry.ok())
        .collect();

    if matches.is_empty() {
        return Ok(vec![]);
    }

    let remote_target = remote_dir.replace("~", "$HOME");
    let target = format!("{}@{}", conn.username, conn.host);

    // Create remote directory
    let mkdir_cmd = format!("mkdir -p \"{}\"", remote_target);
    let mut ssh = create_ssh_command(conn);
    ssh.arg(&mkdir_cmd);
    let _ = ssh.output();

    let mut synced = vec![];
    for file_path in &matches {
        let file_str = file_path.to_string_lossy().to_string();
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let remote_dest = format!("{}:{}/{}", target, remote_dir, file_name);
        let mut scp = create_scp_command(conn);
        scp.args([&file_str, &remote_dest]);

        match scp.output() {
            Ok(output) if output.status.success() => {
                synced.push(format!("{} -> {}/{}", file_str, remote_dir, file_name));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::warn!("SCP pattern file failed for {}: {}", file_str, stderr.trim());
            }
            Err(e) => {
                log::warn!("SCP pattern file failed for {}: {}", file_str, e);
            }
        }
    }

    Ok(synced)
}

/// Sync a single file mapping
pub fn sync_file_mapping(
    mapping: &SSHFileMapping,
    conn: &SSHConnection,
) -> Result<Vec<String>, String> {
    if mapping.is_directory {
        sync_directory(&mapping.local_path, &mapping.remote_path, conn)
    } else if mapping.is_pattern {
        sync_pattern_files(&mapping.local_path, &mapping.remote_path, conn)
    } else {
        sync_single_file(&mapping.local_path, &mapping.remote_path, conn)
    }
}

/// Sync all enabled file mappings
pub fn sync_mappings(
    mappings: &[SSHFileMapping],
    conn: &SSHConnection,
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
        match sync_file_mapping(mapping, conn) {
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
// Remote File Operations
// ============================================================================

/// Read a file from remote server
pub fn read_remote_file(conn: &SSHConnection, path: &str) -> Result<String, String> {
    let remote_path = path.replace("~", "$HOME");

    let command = format!(
        "if [ -f \"{}\" ]; then cat \"{}\"; else echo ''; fi",
        remote_path, remote_path
    );

    let mut ssh = create_ssh_command(conn);
    ssh.arg(&command);

    let output = ssh
        .output()
        .map_err(|e| format!("Failed to read remote file: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("SSH command failed: {}", stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Write content to a remote file
pub fn write_remote_file(conn: &SSHConnection, path: &str, content: &str) -> Result<(), String> {
    let remote_path = path.replace("~", "$HOME");

    let command = format!(
        "mkdir -p \"$(dirname \"{}\")\" && cat > \"{}\"",
        remote_path, remote_path
    );

    let mut ssh = create_ssh_command(conn);
    ssh.arg(&command);
    ssh.stdin(std::process::Stdio::piped());

    let mut child = ssh
        .spawn()
        .map_err(|e| format!("Failed to spawn SSH command: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for SSH command: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("SSH write command failed".to_string())
    }
}

/// Create a symlink on remote
pub fn create_remote_symlink(
    conn: &SSHConnection,
    target: &str,
    link_path: &str,
) -> Result<(), String> {
    let target_expanded = target.replace("~", "$HOME");
    let link_expanded = link_path.replace("~", "$HOME");

    let command = format!(
        "mkdir -p \"$(dirname \"{}\")\" && rm -rf \"{}\" && ln -s \"{}\" \"{}\"",
        link_expanded, link_expanded, target_expanded, link_expanded
    );

    let mut ssh = create_ssh_command(conn);
    ssh.arg(&command);

    let output = ssh
        .output()
        .map_err(|e| format!("Failed to create remote symlink: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Remote symlink failed: {}", stderr.trim()))
    }
}

/// Remove a file or directory on remote
pub fn remove_remote_path(conn: &SSHConnection, path: &str) -> Result<(), String> {
    let remote_path = path.replace("~", "$HOME");
    let command = format!("rm -rf \"{}\"", remote_path);

    let mut ssh = create_ssh_command(conn);
    ssh.arg(&command);

    let output = ssh
        .output()
        .map_err(|e| format!("Failed to remove remote path: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Remote remove failed: {}", stderr.trim()))
    }
}

/// List subdirectories in a remote directory
pub fn list_remote_dir(conn: &SSHConnection, path: &str) -> Result<Vec<String>, String> {
    let remote_path = path.replace("~", "$HOME");
    let command = format!(
        "if [ -d \"{}\" ]; then ls -1 \"{}\"; fi",
        remote_path, remote_path
    );

    let mut ssh = create_ssh_command(conn);
    ssh.arg(&command);

    let output = ssh
        .output()
        .map_err(|e| format!("Failed to list remote dir: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Check if a remote symlink exists and points to the expected target
pub fn check_remote_symlink_exists(
    conn: &SSHConnection,
    link_path: &str,
    expected_target: &str,
) -> bool {
    let link_expanded = link_path.replace("~", "$HOME");
    let target_expanded = expected_target.replace("~", "$HOME");
    let command = format!(
        "[ -L \"{}\" ] && [ \"$(readlink \"{}\")\" = \"{}\" ] && echo yes || echo no",
        link_expanded, link_expanded, target_expanded
    );

    let mut ssh = create_ssh_command(conn);
    ssh.arg(&command);

    if let Ok(output) = ssh.output() {
        String::from_utf8_lossy(&output.stdout).trim() == "yes"
    } else {
        false
    }
}
