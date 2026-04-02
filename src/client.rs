use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use nix::sys::signal;
use nix::unistd::Pid;

use crate::config::AppConfig;
use crate::daemon::{DaemonRequest, DaemonResponse};

/// Try to send a request to the daemon. Returns None if daemon is unreachable.
pub fn try_daemon_request(config: &AppConfig, request: &DaemonRequest) -> Option<DaemonResponse> {
    let socket_path = config.socket_path();

    // Try to connect
    let stream = match UnixStream::connect(&socket_path) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // Set timeouts to avoid hanging
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    send_request(stream, request).ok()
}

fn send_request(mut stream: UnixStream, request: &DaemonRequest) -> Result<DaemonResponse> {
    let json = serde_json::to_string(request)?;
    stream.write_all(json.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let response: DaemonResponse = serde_json::from_str(line.trim())?;
    Ok(response)
}

/// Check if the daemon process is alive by reading the PID file and signaling.
pub fn is_daemon_alive(pid_path: &Path) -> bool {
    let content = match std::fs::read_to_string(pid_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let pid: i32 = match content.trim().parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // kill(pid, None) = signal 0 = check if process exists
    signal::kill(Pid::from_raw(pid), None).is_ok()
}

/// Clean up stale socket and PID files.
fn cleanup_stale_files(config: &AppConfig) {
    let _ = std::fs::remove_file(config.socket_path());
    let _ = std::fs::remove_file(config.pid_path());
}

/// Spawn the daemon as a detached background process.
fn spawn_daemon(config: &AppConfig) -> Result<()> {
    let exe = std::env::current_exe().context("cannot determine current executable path")?;

    // Ensure cache dir exists for the lock file
    std::fs::create_dir_all(&config.cache_dir)?;

    // Acquire lock file to prevent races
    let lock_path = config.lock_path();
    let lock_file = std::fs::File::create(&lock_path).context("creating daemon lock file")?;

    use std::os::unix::io::AsRawFd;
    let fd = lock_file.as_raw_fd();
    let got_lock = unsafe { nix::libc::flock(fd, nix::libc::LOCK_EX | nix::libc::LOCK_NB) == 0 };

    if !got_lock {
        // Another process is spawning — wait for socket instead
        drop(lock_file);
        return wait_for_socket(config);
    }

    // We hold the lock — spawn the daemon
    let mut cmd = Command::new(&exe);
    cmd.arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    cmd.spawn().context("spawning daemon process")?;

    // Wait for socket to appear, then release lock
    let result = wait_for_socket(config);
    drop(lock_file);
    let _ = std::fs::remove_file(&lock_path);
    result
}

/// Wait for the daemon socket to appear with exponential backoff.
fn wait_for_socket(config: &AppConfig) -> Result<()> {
    let socket_path = config.socket_path();
    let delays = [10, 20, 40, 80, 160, 190]; // total: 500ms
    for delay_ms in delays {
        if socket_path.exists() {
            // Give the daemon a moment to start accepting
            std::thread::sleep(Duration::from_millis(10));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(delay_ms));
    }
    anyhow::bail!(
        "daemon socket did not appear at {} within 500ms",
        socket_path.display()
    )
}

/// Try to route a request through the daemon, spawning it if needed.
/// Returns None if the daemon is completely unavailable (caller should fall back).
pub fn request_via_daemon(config: &AppConfig, request: &DaemonRequest) -> Option<DaemonResponse> {
    // Fast path: try existing daemon
    if let Some(resp) = try_daemon_request(config, request) {
        return Some(resp);
    }

    // Check if daemon is alive but socket failed
    let pid_path = config.pid_path();
    if pid_path.exists() && !is_daemon_alive(&pid_path) {
        // Stale — clean up
        cleanup_stale_files(config);
    }

    // Try to spawn daemon — if it doesn't start in time (e.g., first run
    // downloading models), silently fall back to in-process classification.
    if spawn_daemon(config).is_err() {
        return None;
    }

    // Retry after spawn
    try_daemon_request(config, request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_daemon_alive_returns_false_for_nonexistent_pid_file() {
        assert!(!is_daemon_alive(&PathBuf::from(
            "/tmp/nonexistent-csn-test.pid"
        )));
    }

    #[test]
    fn is_daemon_alive_returns_false_for_invalid_pid_content() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("test.pid");
        std::fs::write(&pid_path, "not-a-number").unwrap();
        assert!(!is_daemon_alive(&pid_path));
    }

    #[test]
    fn is_daemon_alive_returns_false_for_dead_process() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("test.pid");
        // PID 99999999 is very unlikely to be alive
        std::fs::write(&pid_path, "99999999").unwrap();
        assert!(!is_daemon_alive(&pid_path));
    }

    #[test]
    fn is_daemon_alive_returns_true_for_current_process() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("test.pid");
        std::fs::write(&pid_path, std::process::id().to_string()).unwrap();
        assert!(is_daemon_alive(&pid_path));
    }

    #[test]
    fn try_daemon_request_returns_none_when_no_daemon() {
        let config = AppConfig::load(crate::config::CliOverrides {
            cache_dir: Some(PathBuf::from("/tmp/csn-test-no-daemon")),
            ..Default::default()
        })
        .unwrap();
        let req = DaemonRequest {
            command: "classify".to_string(),
            args: serde_json::json!({"text": "test", "set": "corrections"}),
        };
        assert!(try_daemon_request(&config, &req).is_none());
    }
}
