use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Spawn `csn mcp`, send a JSON-RPC request via stdin, read the response from stdout.
///
/// Requires model download — marked `#[ignore]` for CI.
/// Run manually: `cargo test --test integration_test -- --ignored`
#[test]
#[ignore]
fn mcp_initialize_and_list_tools() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bin = format!("{}/target/debug/csn", manifest_dir);
    let sets_dir = format!("{}/reference-sets", manifest_dir);

    let mut child = Command::new(&bin)
        .arg("mcp")
        .env("CSN_SETS_DIR", &sets_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start csn mcp — run `cargo build` first");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "integration-test",
                "version": "0.1.0"
            }
        }
    });

    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Read initialize response
    let init_response = read_jsonrpc_response(&mut reader, Duration::from_secs(120));
    assert_eq!(init_response["id"], 1);
    assert!(
        init_response["result"]["serverInfo"]["name"]
            .as_str()
            .unwrap()
            .contains("csn"),
        "server name should contain 'csn'"
    );

    // Send initialized notification
    let initialized = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    writeln!(stdin, "{}", serde_json::to_string(&initialized).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Send tools/list request
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    writeln!(stdin, "{}", serde_json::to_string(&list_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Read tools/list response
    let tools_response = read_jsonrpc_response(&mut reader, Duration::from_secs(10));
    assert_eq!(tools_response["id"], 2);

    let tools = tools_response["result"]["tools"]
        .as_array()
        .expect("tools should be an array");
    assert_eq!(tools.len(), 4, "should have 4 tools");

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(tool_names.contains(&"classify"));
    assert!(tool_names.contains(&"list_sets"));
    assert!(tool_names.contains(&"embed"));
    assert!(tool_names.contains(&"similarity"));

    // Clean up
    drop(stdin);
    child.kill().ok();
    child.wait().ok();
}

/// Spawn `csn mcp`, call the classify tool, verify the response format.
///
/// Requires model download — marked `#[ignore]` for CI.
/// Run manually: `cargo test --test integration_test -- --ignored`
#[test]
#[ignore]
fn mcp_classify_tool_returns_valid_result() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bin = format!("{}/target/debug/csn", manifest_dir);
    let sets_dir = format!("{}/reference-sets", manifest_dir);

    let mut child = Command::new(&bin)
        .arg("mcp")
        .env("CSN_SETS_DIR", &sets_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start csn mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize handshake
    mcp_handshake(&mut stdin, &mut reader);

    // Call classify tool
    let call_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "classify",
            "arguments": {
                "text": "no, use the other approach instead",
                "reference_set": "corrections"
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    let response = read_jsonrpc_response(&mut reader, Duration::from_secs(30));
    assert_eq!(response["id"], 3);

    let content = &response["result"]["content"];
    assert!(content.is_array(), "content should be an array");
    let text = content[0]["text"]
        .as_str()
        .expect("should have text content");
    let result: serde_json::Value = serde_json::from_str(text).expect("text should be valid JSON");

    assert!(result.get("match").is_some(), "should have 'match' field");
    assert!(
        result.get("confidence").is_some(),
        "should have 'confidence' field"
    );
    assert!(
        result.get("top_phrase").is_some(),
        "should have 'top_phrase' field"
    );
    assert!(result.get("scores").is_some(), "should have 'scores' field");

    // Clean up
    drop(stdin);
    child.kill().ok();
    child.wait().ok();
}

fn mcp_handshake(stdin: &mut impl Write, reader: &mut impl BufRead) {
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "integration-test",
                "version": "0.1.0"
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_request).unwrap()).unwrap();
    stdin.flush().unwrap();

    let _init_response = read_jsonrpc_response(reader, Duration::from_secs(120));

    let initialized = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    writeln!(stdin, "{}", serde_json::to_string(&initialized).unwrap()).unwrap();
    stdin.flush().unwrap();
}

/// Spawn `csn daemon` with a short idle timeout, verify it self-exits.
///
/// Requires model download — marked `#[ignore]` for CI.
/// Run manually: `cargo test --test integration_test -- --ignored`
#[test]
#[ignore]
#[cfg(unix)]
fn daemon_self_exits_on_idle_timeout() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bin = format!("{}/target/debug/csn", manifest_dir);
    let sets_dir = format!("{}/reference-sets", manifest_dir);
    let cache_dir = format!("{}/target/test-daemon-cache", manifest_dir);

    // Ensure clean state
    let socket_path = format!("{cache_dir}/csn.sock");
    let pid_path = format!("{cache_dir}/csn.pid");
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::create_dir_all(&cache_dir);

    // Start daemon with 2-second idle timeout
    let mut child = Command::new(&bin)
        .arg("daemon")
        .env("CSN_SETS_DIR", &sets_dir)
        .env("CSN_CACHE_DIR", &cache_dir)
        .env("CSN_IDLE_TIMEOUT", "2")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start csn daemon — run `cargo build` first");

    // Wait for socket to appear
    let start = std::time::Instant::now();
    while !std::path::Path::new(&socket_path).exists() {
        if start.elapsed() > Duration::from_secs(120) {
            child.kill().ok();
            panic!("daemon socket did not appear within 120s");
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Send one request to verify daemon works
    let stream = std::os::unix::net::UnixStream::connect(&socket_path)
        .expect("failed to connect to daemon socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    {
        use std::io::Write as _;
        let req = serde_json::json!({"command": "classify", "args": {"text": "test", "set": "corrections"}});
        let mut msg = serde_json::to_string(&req).unwrap();
        msg.push('\n');
        (&stream).write_all(msg.as_bytes()).unwrap();
        (&stream).flush().unwrap();

        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).unwrap();
        let resp: serde_json::Value = serde_json::from_str(response_line.trim()).unwrap();
        assert_eq!(resp["ok"], true, "daemon classify should succeed");
    }
    drop(stream);

    // Wait for idle timeout (2s) + check interval (30s max, but daemon checks every 30s)
    // With 2s timeout and 30s check interval, daemon should exit within ~32s
    let timeout_start = std::time::Instant::now();
    loop {
        if !std::path::Path::new(&socket_path).exists() {
            break; // Socket cleaned up — daemon exited
        }
        if timeout_start.elapsed() > Duration::from_secs(40) {
            child.kill().ok();
            child.wait().ok();
            panic!("daemon did not self-exit within 40s after idle timeout");
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Verify PID file also cleaned up
    assert!(
        !std::path::Path::new(&pid_path).exists(),
        "PID file should be cleaned up after daemon exit"
    );

    // Verify process actually exited
    let status = child.wait().expect("failed to wait on daemon");
    assert!(
        status.success(),
        "daemon should exit cleanly, got: {status}"
    );
}

fn read_jsonrpc_response(reader: &mut impl BufRead, timeout: Duration) -> serde_json::Value {
    let start = std::time::Instant::now();
    let mut line = String::new();

    loop {
        if start.elapsed() > timeout {
            panic!(
                "timed out waiting for JSON-RPC response after {:?}",
                timeout
            );
        }

        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => panic!("EOF while waiting for JSON-RPC response"),
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    // Skip notifications (no "id" field)
                    if val.get("id").is_some() {
                        return val;
                    }
                }
            }
            Err(e) => panic!("error reading from stdout: {e}"),
        }
    }
}
