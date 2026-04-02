#!/usr/bin/env bash
# Test: verify hook jq parsing logic for multi-category and binary output shapes.
# Run: bash tests/hook_parsing_test.sh

set -euo pipefail

PASS=0
FAIL=0

assert_contains() {
    local label="$1" output="$2" expected="$3"
    if echo "$output" | grep -q "$expected"; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label — expected '$expected' in output"
        FAIL=$((FAIL + 1))
    fi
}

assert_empty() {
    local label="$1" output="$2"
    if [ -z "$output" ]; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label — expected empty output, got: $output"
        FAIL=$((FAIL + 1))
    fi
}

# --- Multi-category: frustration ---
echo "Test 1: Multi-category frustration output"
MULTI_FRUSTRATION='{"match":true,"category":"frustration","confidence":0.92,"top_phrase":"wtf","all_scores":[{"category":"frustration","score":0.92,"top_phrase":"wtf"},{"category":"correction","score":0.05,"top_phrase":"wrong"},{"category":"neutral","score":0.03,"top_phrase":"ok"}]}'
CATEGORY=$(echo "$MULTI_FRUSTRATION" | jq -r '.category // empty')
assert_contains "category is frustration" "$CATEGORY" "frustration"

# --- Multi-category: correction ---
echo "Test 2: Multi-category correction output"
MULTI_CORRECTION='{"match":true,"category":"correction","confidence":0.85,"top_phrase":"wrong file","all_scores":[{"category":"correction","score":0.85,"top_phrase":"wrong file"},{"category":"frustration","score":0.10,"top_phrase":"ugh"},{"category":"neutral","score":0.05,"top_phrase":"ok"}]}'
CATEGORY=$(echo "$MULTI_CORRECTION" | jq -r '.category // empty')
assert_contains "category is correction" "$CATEGORY" "correction"

# --- Multi-category: neutral (should not fire) ---
echo "Test 3: Multi-category neutral output (should skip)"
MULTI_NEUTRAL='{"match":true,"category":"neutral","confidence":0.88,"top_phrase":"sounds good","all_scores":[{"category":"neutral","score":0.88,"top_phrase":"sounds good"},{"category":"correction","score":0.08,"top_phrase":"wrong"},{"category":"frustration","score":0.04,"top_phrase":"ugh"}]}'
CATEGORY=$(echo "$MULTI_NEUTRAL" | jq -r '.category // empty')
assert_contains "category is neutral" "$CATEGORY" "neutral"
# Hook should exit 0 without output for neutral — we just verify the field is "neutral"

# --- Binary output (backward compat) ---
echo "Test 4: Binary output shape"
BINARY='{"match":true,"confidence":0.91,"top_phrase":"wrong file","scores":{"positive":0.91,"negative":0.3}}'
CATEGORY=$(echo "$BINARY" | jq -r '.category // empty')
IS_MATCH=$(echo "$BINARY" | jq -r '.match')
assert_empty "no category field in binary" "$CATEGORY"
assert_contains "binary match is true" "$IS_MATCH" "true"

# --- Summary ---
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
