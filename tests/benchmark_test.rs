use std::path::Path;

/// Verify dataset files are valid and have expected structure.
#[test]
fn datasets_are_valid_json() {
    let datasets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("datasets");

    for name in ["corrections"] {
        let path = datasets_dir.join(format!("{name}.json"));
        assert!(path.exists(), "dataset {name}.json should exist");

        let content = std::fs::read_to_string(&path).unwrap();
        let ds: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("{name}.json is invalid JSON: {e}"));

        // Check required fields
        assert_eq!(ds["name"].as_str().unwrap(), name);
        assert!(ds["reference_set"].is_string());
        assert!(ds["mode"].is_string());

        // Check prompt count (at least 50 for meaningful benchmarks)
        let prompts = ds["prompts"].as_array().unwrap();
        assert!(
            prompts.len() >= 50,
            "{name}: expected >= 50 prompts, got {}",
            prompts.len()
        );

        // Check each prompt has required fields
        for (i, p) in prompts.iter().enumerate() {
            assert!(p["text"].is_string(), "{name} prompt {i}: missing text");
            assert!(
                p["expected_label"].is_string(),
                "{name} prompt {i}: missing expected_label"
            );
            assert!(p["tier"].is_string(), "{name} prompt {i}: missing tier");
            assert!(
                p["polarity"].is_string(),
                "{name} prompt {i}: missing polarity"
            );
        }
    }
}

/// Verify corrections dataset has correct multi-category labels.
#[test]
fn corrections_dataset_labels() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("datasets/corrections.json");
    let content = std::fs::read_to_string(&path).unwrap();
    let ds: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(ds["mode"].as_str().unwrap(), "multi-category");

    let valid_labels = ["correction", "frustration", "neutral"];
    let prompts = ds["prompts"].as_array().unwrap();

    for p in prompts {
        let label = p["expected_label"].as_str().unwrap();
        assert!(
            valid_labels.contains(&label),
            "unexpected label '{label}', expected one of {valid_labels:?}"
        );
    }
}
