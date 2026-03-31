use std::path::Path;

/// Verify dataset files are valid and have expected structure.
#[test]
fn datasets_are_valid_json() {
    let datasets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("datasets");

    for name in ["corrections", "commit-types"] {
        let path = datasets_dir.join(format!("{name}.json"));
        assert!(path.exists(), "dataset {name}.json should exist");

        let content = std::fs::read_to_string(&path).unwrap();
        let ds: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("{name}.json is invalid JSON: {e}"));

        // Check required fields
        assert_eq!(ds["name"].as_str().unwrap(), name);
        assert!(ds["reference_set"].is_string());
        assert!(ds["mode"].is_string());

        // Check prompt count
        let prompts = ds["prompts"].as_array().unwrap();
        assert!(
            prompts.len() >= 490,
            "{name}: expected ~500 prompts, got {}",
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

        // Check tier distribution (each bucket should have at least 70)
        let tiers = ["clear", "moderate", "edge"];
        let polarities = ["positive", "negative"];
        for tier in tiers {
            for pol in polarities {
                let count = prompts
                    .iter()
                    .filter(|p| {
                        p["tier"].as_str() == Some(tier) && p["polarity"].as_str() == Some(pol)
                    })
                    .count();
                assert!(
                    count >= 70,
                    "{name}: {tier}/{pol} has only {count} prompts (expected ~83)"
                );
            }
        }
    }
}

/// Verify corrections dataset has correct labels.
#[test]
fn corrections_dataset_labels() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("datasets/corrections.json");
    let content = std::fs::read_to_string(&path).unwrap();
    let ds: serde_json::Value = serde_json::from_str(&content).unwrap();
    let prompts = ds["prompts"].as_array().unwrap();

    for p in prompts {
        let label = p["expected_label"].as_str().unwrap();
        let polarity = p["polarity"].as_str().unwrap();
        match polarity {
            "positive" => assert_eq!(label, "match", "positive prompt should have label 'match'"),
            "negative" => assert_eq!(
                label, "no_match",
                "negative prompt should have label 'no_match'"
            ),
            _ => panic!("unexpected polarity: {polarity}"),
        }
    }
}

/// Verify commit-types dataset labels match valid categories.
#[test]
fn commit_types_dataset_labels() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("datasets/commit-types.json");
    let content = std::fs::read_to_string(&path).unwrap();
    let ds: serde_json::Value = serde_json::from_str(&content).unwrap();
    let prompts = ds["prompts"].as_array().unwrap();

    let valid_categories = [
        "feat", "fix", "refactor", "docs", "chore", "test", "no_match",
    ];

    for (i, p) in prompts.iter().enumerate() {
        let label = p["expected_label"].as_str().unwrap();
        assert!(
            valid_categories.contains(&label),
            "prompt {i}: unexpected label '{label}'"
        );
    }
}
