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
        assert!(validate_sync_shell_output("AI_TOOLBOX_POSIX_SYNC_NO_HOME").unwrap_err().contains("HOME"));
        for output in ["", "Windows", "Linux", "AI_TOOLBOX_POSIX_SYNC_V1 extra", "echo AI_TOOLBOX_POSIX_SYNC_V1"] {
            assert_eq!(validate_sync_shell_output(output), Err(UNSUPPORTED_SYNC_SHELL.to_string()));
        }
    }
}

/// The write future is not created/polled until the read-only probe succeeds.
/// Used by each production sync entry point, with a bounded probe wait.
pub(super) async fn guard_sync_operation<P, F, W, T>(
    probe: P,
    timeout: std::time::Duration,
    write: F,
) -> Result<T, String>
where
    P: std::future::Future<Output = Result<String, String>>,
    F: FnOnce() -> W,
    W: std::future::Future<Output = T>,
{
    let output = tokio::time::timeout(timeout, probe)
        .await
        .map_err(|_| "SSH 同步 shell 预检超时；已停止后续写入。".to_string())?
        .map_err(|error| format!("SSH 同步 shell 预检命令失败；已停止后续写入。当前同步仅支持 POSIX 兼容 shell。{}", error))?;
    validate_sync_shell_output(&output)?;
    Ok(write().await)
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use std::cell::Cell;
    use std::time::Duration;

    #[tokio::test]
    async fn rejected_probe_never_invokes_write_closure() {
        for response in [Ok(String::new()), Ok("AI_TOOLBOX_POSIX_SYNC_NO_HOME".into()), Ok("unexpected".into()), Err("synthetic exit 1".into())] {
            let writes = Cell::new(0);
            let result = guard_sync_operation(async { response }, Duration::from_secs(1), || {
                writes.set(writes.get() + 1);
                async { () }
            }).await;
            assert!(result.is_err());
            assert_eq!(writes.get(), 0);
        }
    }

    #[tokio::test]
    async fn timeout_never_invokes_write_closure() {
        let writes = Cell::new(0);
        let result = guard_sync_operation(std::future::pending::<Result<String, String>>(), Duration::from_millis(1), || {
            writes.set(writes.get() + 1);
            async { () }
        }).await;
        assert!(result.unwrap_err().contains("超时"));
        assert_eq!(writes.get(), 0);
    }

    #[tokio::test]
    async fn accepted_probe_invokes_write_once_and_preserves_result() {
        let writes = Cell::new(0);
        let result = guard_sync_operation(async { Ok("AI_TOOLBOX_POSIX_SYNC_V1\n".into()) }, Duration::from_secs(1), || {
            writes.set(writes.get() + 1);
            async { "phase-result" }
        }).await;
        assert_eq!(result.unwrap(), "phase-result");
        assert_eq!(writes.get(), 1);
    }
}
