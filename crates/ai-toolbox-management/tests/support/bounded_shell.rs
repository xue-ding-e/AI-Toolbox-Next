// Shared Linux-only synthetic shell runner. Never used by product code.
use std::fs::File;
use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const RUN_LIMIT: Duration = Duration::from_secs(8);
const CLEANUP_LIMIT: Duration = Duration::from_secs(2);
const LOG_LIMIT: u64 = 16 * 1024;

#[derive(Clone, Copy)]
struct ProcStat {
    state: u8,
    group: i32,
    start_time: u64,
}

impl ProcStat {
    fn is_live(self) -> bool {
        !matches!(self.state, b'Z' | b'X' | b'x')
    }
}

fn invalid_stat() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid /proc process stat")
}

fn parse_proc_stat(bytes: &[u8]) -> std::io::Result<ProcStat> {
    // comm can contain spaces and ')', and need not be UTF-8. Only the fields
    // after its final ')' are interpreted. Their indexes start at field 3.
    let end = bytes.iter().rposition(|byte| *byte == b')').ok_or_else(invalid_stat)?;
    let tail = bytes.get(end + 1..).ok_or_else(invalid_stat)?;
    let fields: Vec<_> = std::str::from_utf8(tail).map_err(|_| invalid_stat())?
        .split_whitespace().collect();
    let state = fields.first().ok_or_else(invalid_stat)?.as_bytes();
    if state.len() != 1 { return Err(invalid_stat()); }
    let _parent: i32 = fields.get(1).ok_or_else(invalid_stat)?
        .parse().map_err(|_| invalid_stat())?;
    let group = fields.get(2).ok_or_else(invalid_stat)?
        .parse().map_err(|_| invalid_stat())?;
    let start_time = fields.get(19).ok_or_else(invalid_stat)?
        .parse().map_err(|_| invalid_stat())?;
    Ok(ProcStat { state: state[0], group, start_time })
}

fn read_proc_stat(pid: i32) -> std::io::Result<ProcStat> {
    let mut bytes = Vec::new();
    File::open(format!("/proc/{pid}/stat"))?.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 { return Err(invalid_stat()); }
    parse_proc_stat(&bytes)
}

fn live_group_members(group: i32, deadline: Instant) -> std::io::Result<usize> {
    let mut live = 0;
    for entry in std::fs::read_dir("/proc")? {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "process-group scan deadline exceeded"));
        }
        let entry = entry?;
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<i32>().ok()) else { continue; };
        let stat = match read_proc_stat(pid) {
            Ok(stat) => stat,
            // A process can disappear during enumeration. All other errors,
            // including unreadable or malformed records, remain fail-closed.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if stat.group == group && stat.is_live() { live += 1; }
    }
    Ok(live)
}

fn leader_exited_without_reaping(pid: i32) -> std::io::Result<bool> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let result = unsafe {
        libc::waitid(libc::P_PID, pid as libc::id_t, &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT)
    };
    if result != 0 { return Err(std::io::Error::last_os_error()); }
    match unsafe { info.si_pid() } {
        0 => Ok(false),
        observed if observed == pid => Ok(true),
        _ => Err(invalid_stat()),
    }
}

fn log_limit_reached(paths: &[&std::path::Path]) -> std::io::Result<bool> {
    for path in paths {
        if std::fs::metadata(path)?.len() >= LOG_LIMIT { return Ok(true); }
    }
    Ok(false)
}

fn read_log(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(LOG_LIMIT).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn run(
    shell: &str,
    script: &str,
    fixture: tempfile::TempDir,
) -> (tempfile::TempDir, Output) {
    let logs = tempfile::Builder::new().prefix("coord-shell-").tempdir().unwrap();
    let stdout_path = logs.path().join("stdout");
    let stderr_path = logs.path().join("stderr");
    let stdout = File::create(&stdout_path).unwrap();
    let stderr = File::create(&stderr_path).unwrap();
    let mut command = Command::new(shell);
    command.args(["-c", script]).process_group(0)
        .current_dir(fixture.path()).env_clear()
        .env("PATH", "/usr/bin:/bin").env("LC_ALL", "C")
        .env("HOME", fixture.path()).env("TMPDIR", logs.path())
        .stdin(Stdio::null()).stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr));
    // This child-only limit bounds temporary logs on disk, not just reporting.
    // The synthetic source files are also deliberately smaller than this limit.
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: LOG_LIMIT as libc::rlim_t,
                rlim_max: LOG_LIMIT as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().expect("spawn isolated shell fixture");
    let group = i32::try_from(child.id()).expect("Linux PID fits pid_t");
    let deadline = Instant::now() + RUN_LIMIT;
    let mut failures = Vec::new();
    let identity = match read_proc_stat(group) {
        Ok(stat) if stat.group == group => Some(stat),
        Ok(_) => { failures.push("shell fixture did not retain its isolated process group".to_string()); None }
        Err(error) => { failures.push(format!("shell fixture identity inspection failed: {error}")); None }
    };
    let mut exited = false;
    while failures.is_empty() {
        match log_limit_reached(&[&stdout_path, &stderr_path]) {
            Ok(false) => {}
            Ok(true) => { failures.push("shell fixture log limit reached".to_string()); break; }
            Err(error) => { failures.push(format!("shell fixture log inspection failed: {error}")); break; }
        }
        match leader_exited_without_reaping(group) {
            Ok(true) => { exited = true; break; }
            Ok(false) => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => { failures.push(format!("shell fixture status failed: {error}")); break; }
        }
        if Instant::now() >= deadline {
            failures.push("shell fixture execution deadline exceeded".to_string()); break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // WNOWAIT keeps the leader PID allocated until all group signals finish.
    // This helper is the sole waiter. Never reap and then signal a numeric PGID.
    // Only known fixtures without setsid/setpgid escapes are supported; this is
    // not supervision of an arbitrary or adversarial descendant process tree.
    let cleanup_deadline = Instant::now() + CLEANUP_LIMIT;
    let mut cleanup_confirmed = false;
    let mut signalled = false;
    loop {
        if Instant::now() >= cleanup_deadline { break; }
        match leader_exited_without_reaping(group) {
            Ok(observed) => exited |= observed,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => { failures.push(format!("shell fixture identity is no longer waitable: {error}")); break; }
        }
        let Some(identity) = identity else { break; };
        match read_proc_stat(group) {
            Ok(stat) if stat.group == group && stat.start_time == identity.start_time => {}
            Ok(_) => { failures.push("shell fixture leader identity or group changed".to_string()); break; }
            Err(error) => { failures.push(format!("shell fixture leader verification failed: {error}")); break; }
        }
        let live = match live_group_members(group, cleanup_deadline) {
            Ok(live) => live,
            Err(error) => {
                failures.push(format!("shell fixture group inspection failed: {error}"));
                // This iteration already verified the still-unreaped leader.
                // Best-effort stopping is safe even when a complete /proc scan
                // is unavailable, but it cannot establish group cleanup.
                let killed = unsafe { libc::kill(-group, libc::SIGKILL) };
                if killed != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                    failures.push("failed to stop shell fixture process group".to_string());
                }
                while !exited && Instant::now() < cleanup_deadline {
                    match leader_exited_without_reaping(group) {
                        Ok(observed) => exited = observed,
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(error) => { failures.push(format!("shell fixture exit observation failed: {error}")); break; }
                    }
                    if !exited { std::thread::sleep(Duration::from_millis(10)); }
                }
                break;
            }
        };
        if exited && live == 0 {
            cleanup_confirmed = true;
            break;
        }
        if exited && live != 0 && !signalled {
            failures.push("shell fixture leader exited with live process-group members".to_string());
        }
        if live != 0 {
            // The unreaped, verified leader pins this group identity. Repeated
            // cleanup signals are allowed only before the single final reap.
            let killed = unsafe { libc::kill(-group, libc::SIGKILL) };
            if killed != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                failures.push("failed to stop shell fixture process group".to_string()); break;
            }
            signalled = true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !cleanup_confirmed {
        failures.push("shell fixture exit/process-group cleanup was not established".to_string());
    }
    // Exactly one nonblocking reap, after all signalling. Even if it fails or
    // returns None, no later operation may signal this now-unpinned group ID.
    let status = match child.try_wait() {
        Ok(Some(status)) => Some(status),
        Ok(None) => { failures.push("shell fixture leader exit was not confirmed".to_string()); None }
        Err(error) => { failures.push(format!("shell fixture leader reap failed: {error}")); None }
    };
    // Catch a short-lived child reaching the limit between supervisor polls.
    match log_limit_reached(&[&stdout_path, &stderr_path]) {
        Ok(false) => {}
        Ok(true) => failures.push("shell fixture log limit reached".to_string()),
        Err(error) => failures.push(format!("shell fixture log inspection failed: {error}")),
    }
    if status.as_ref().and_then(ExitStatusExt::signal) == Some(libc::SIGXFSZ) {
        failures.push("shell fixture file-size limit exceeded".to_string());
    }
    let stdout = read_log(&stdout_path).unwrap_or_else(|error| {
        failures.push(format!("shell fixture stdout read failed: {error}"));
        Vec::new()
    });
    let stderr = read_log(&stderr_path).unwrap_or_else(|error| {
        failures.push(format!("shell fixture stderr read failed: {error}"));
        Vec::new()
    });
    if !failures.is_empty() {
        let fixture_path = fixture.keep();
        let logs_path = logs.keep();
        panic!("{}; retained fixture={}, logs={}; stderr={}", failures.join("; "),
            fixture_path.display(), logs_path.display(), String::from_utf8_lossy(&stderr));
    }
    (fixture, Output { status: status.expect("confirmed child exit"), stdout, stderr })
}

#[test]
fn proc_stat_parser_distinguishes_live_members_from_zombies() {
    for (state, expected_live) in [("S", true), ("Z", false), ("X", false)] {
        let mut fields = vec!["0"; 20];
        fields[0] = state;
        fields[1] = "1";
        fields[2] = "123";
        fields[19] = "456";
        let stat = format!("123 (fixture ) name) {}", fields.join(" "));
        let parsed = parse_proc_stat(stat.as_bytes()).unwrap();
        assert_eq!(parsed.group, 123);
        assert_eq!(parsed.start_time, 456);
        assert_eq!(parsed.is_live(), expected_live);
    }
    assert!(parse_proc_stat(b"123 (truncated) Z 1 123").is_err());
}
