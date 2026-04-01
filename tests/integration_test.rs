use std::net::TcpListener;
use std::time::Duration;

fn available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn wait_for_server(port: u16, timeout: Duration) -> reqwest::blocking::Client {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();

    let start = std::time::Instant::now();
    let url = format!("http://127.0.0.1:{}/health", port);

    while start.elapsed() < timeout {
        if client.get(&url).send().is_ok() {
            return client;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    panic!("server did not start within {:?}", timeout);
}

#[test]
fn daemon_rest_api_contract() {
    let port = available_port();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let sets_dir = format!("{}/reference-sets", manifest_dir);
    let bin = format!("{}/target/debug/csn", manifest_dir);

    // Start daemon as subprocess with smallest model
    let mut child = std::process::Command::new(&bin)
        .args([
            "serve",
            "--port",
            &port.to_string(),
            "--sets-dir",
            &sets_dir,
            "--model",
            "bge-small-en-v1.5-Q",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start daemon — run `cargo build` first");

    let client = wait_for_server(port, Duration::from_secs(60));
    let base = format!("http://127.0.0.1:{}", port);

    // GET /health → 200 with {status, model, sets, uptime}
    let resp = client.get(format!("{base}/health")).send().unwrap();
    assert_eq!(resp.status(), 200);
    let health: serde_json::Value = resp.json().unwrap();
    assert_eq!(health["status"], "ok");
    assert!(health["model"].is_string());
    assert!(health["sets"].as_u64().unwrap() >= 1);
    assert!(health["uptime"].is_string());

    // GET /sets → 200 with [{name, phrases, mode}]
    let resp = client.get(format!("{base}/sets")).send().unwrap();
    assert_eq!(resp.status(), 200);
    let sets: Vec<serde_json::Value> = resp.json().unwrap();
    assert!(!sets.is_empty());
    let first = &sets[0];
    assert!(first["name"].is_string());
    assert!(first["phrases"].is_number());
    assert!(first["mode"].is_string());

    // POST /classify with valid set → 200
    let resp = client
        .post(format!("{base}/classify"))
        .json(&serde_json::json!({
            "text": "no, use the other approach instead",
            "reference_set": "corrections"
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let result: serde_json::Value = resp.json().unwrap();
    // Binary result has "match", "confidence", "top_phrase", "scores"
    assert!(result.get("match").is_some());
    assert!(result.get("confidence").is_some());
    assert!(result.get("top_phrase").is_some());

    // POST /classify with missing set → 404 with {error}
    let resp = client
        .post(format!("{base}/classify"))
        .json(&serde_json::json!({
            "text": "test",
            "reference_set": "nonexistent"
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 404);
    let err: serde_json::Value = resp.json().unwrap();
    assert!(err["error"].as_str().unwrap().contains("not found"));

    // POST /embed → 200 with {embedding, dimensions, model}
    let resp = client
        .post(format!("{base}/embed"))
        .json(&serde_json::json!({"text": "embedding test"}))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let embed: serde_json::Value = resp.json().unwrap();
    assert!(embed["dimensions"].as_u64().unwrap() > 0);
    assert!(!embed["embedding"].as_array().unwrap().is_empty());
    assert!(embed["model"].is_string());

    // POST /similarity → 200 with {similarity}
    let resp = client
        .post(format!("{base}/similarity"))
        .json(&serde_json::json!({"a": "cat", "b": "dog"}))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let sim: serde_json::Value = resp.json().unwrap();
    let score = sim["similarity"].as_f64().unwrap();
    assert!((-1.0..=1.0).contains(&score));

    // Clean up
    child.kill().ok();
    child.wait().ok();
}

/// Integration test that verifies `/classify` returns a result when MLP is loaded.
///
/// Requires model download and MLP training at startup, so it is ignored in CI.
/// Run manually: `cargo test --test integration_test -- --ignored`
#[test]
#[ignore]
fn classify_returns_mlp_result() {
    let port = available_port();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let sets_dir = format!("{}/reference-sets", manifest_dir);
    let bin = format!("{}/target/debug/csn", manifest_dir);

    let mut child = std::process::Command::new(&bin)
        .args([
            "serve",
            "--port",
            &port.to_string(),
            "--sets-dir",
            &sets_dir,
            "--model",
            "bge-small-en-v1.5-Q",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start daemon — run `cargo build` first");

    // MLP training happens at startup, allow extra time
    let client = wait_for_server(port, Duration::from_secs(120));
    let base = format!("http://127.0.0.1:{}", port);

    let resp = client
        .post(format!("{base}/classify"))
        .json(&serde_json::json!({
            "text": "I don't think that's right",
            "reference_set": "corrections"
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);

    let result: serde_json::Value = resp.json().unwrap();

    // "match" field must be a boolean (serde renames is_match -> match)
    assert!(
        result.get("match").is_some(),
        "response must contain 'match' field"
    );
    assert!(
        result["match"].is_boolean(),
        "'match' must be a boolean, got: {}",
        result["match"]
    );

    // confidence must be between 0 and 1
    let confidence = result["confidence"]
        .as_f64()
        .expect("response must contain numeric 'confidence' field");
    assert!(
        (0.0..=1.0).contains(&confidence),
        "confidence must be in [0, 1], got: {confidence}"
    );

    // scores must exist
    assert!(
        result.get("scores").is_some(),
        "response must contain 'scores' field"
    );

    // Clean up
    child.kill().ok();
    child.wait().ok();
}
