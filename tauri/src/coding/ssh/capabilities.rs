//! Read-only grammar probe for the existing POSIX-shell sync implementation.
//! This is not OS detection or a promise that every remote utility is available.

// Only shell builtins, parameter expansion and stdout are used. In particular,
// never probe support by creating, replacing or removing a remote file.
pub(super) const POSIX_SYNC_PROBE: &str =
    "if [ -n \"$HOME\" ]; then : && printf '%s\\n' 'AI_TOOLBOX_POSIX_SYNC_V1'; else : && printf '%s\\n' 'AI_TOOLBOX_POSIX_SYNC_NO_HOME'; fi";

pub(super) const UNSUPPORTED_SYNC_SHELL: &str =
    "SSH 同步要求 POSIX 兼容 shell；远端 POSIX shell 语法检查未通过，已停止后续写入（Windows PowerShell/cmd 暂不支持）。";

pub(super) fn validate_sync_shell_output(output: &str) -> Result<(), String> {
    if output.trim() == "AI_TOOLBOX_POSIX_SYNC_V1" {
        Ok(())
    } else if output.trim() == "AI_TOOLBOX_POSIX_SYNC_NO_HOME" {
        Err("SSH 同步要求非空的远端 HOME；已停止后续写入。".to_string())
    } else {
        Err(UNSUPPORTED_SYNC_SHELL.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_expected_probe_response() {
        assert!(validate_sync_shell_output("AI_TOOLBOX_POSIX_SYNC_V1\n").is_ok());
        assert!(validate_sync_shell_output("AI_TOOLBOX_POSIX_SYNC_V1\r\n").is_ok());
        for output in ["", "Windows", "Linux", "AI_TOOLBOX_POSIX_SYNC_V1 extra", "echo AI_TOOLBOX_POSIX_SYNC_V1"] {
            assert_eq!(validate_sync_shell_output(output), Err(UNSUPPORTED_SYNC_SHELL.to_string()));
        }
    }
}
