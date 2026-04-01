#!/usr/bin/env bash
# Manual benchmark: classify sentences and report accuracy
# Usage: ./scripts/manual-benchmark.sh [path-to-csn-binary]
set -euo pipefail

CSN="${1:-./target/debug/csn}"
PASS=0
FAIL=0
TOTAL=0
ERRORS=()

classify() {
    local text="$1"
    local expect="$2"  # "match" or "no_match"
    local category="$3"
    local length="$4"

    TOTAL=$((TOTAL + 1))
    local result
    result=$("$CSN" classify "$text" --set corrections --standalone --json 2>/dev/null) || {
        ERRORS+=("CRASH | $category/$length | $text")
        FAIL=$((FAIL + 1))
        return
    }

    local is_match
    is_match=$(echo "$result" | jq -r '.match // .Binary.match // "null"')
    local confidence
    confidence=$(echo "$result" | jq -r '.confidence // .Binary.confidence // "?"')

    local got
    if [ "$is_match" = "true" ]; then got="match"; else got="no_match"; fi

    if [ "$got" = "$expect" ]; then
        PASS=$((PASS + 1))
        printf "  ✓ %-12s %-8s conf=%-6s %s\n" "$category" "$length" "$confidence" "$text"
    else
        FAIL=$((FAIL + 1))
        ERRORS+=("WRONG | expected=$expect got=$got conf=$confidence | $category/$length | $text")
        printf "  ✗ %-12s %-8s conf=%-6s expected=%s got=%s | %s\n" "$category" "$length" "$confidence" "$expect" "$got" "$text"
    fi
}

echo "=== Manual Benchmark: Pushback Detection ==="
echo "Binary: $CSN"
echo ""

# ============================================================
# SECTION 1: KNOWN — close to reference set phrases (pipeline test)
# ============================================================
echo "━━━ Section 1: Known phrases (pipeline validation) ━━━"
echo ""
echo "── Positive (should match) ──"

classify "no" "match" "known-pos" "short"
classify "wrong" "match" "known-pos" "short"
classify "revert that" "match" "known-pos" "short"
classify "undo" "match" "known-pos" "short"
classify "nope" "match" "known-pos" "short"
classify "that's incorrect" "match" "known-pos" "short"
classify "you broke the tests" "match" "known-pos" "medium"
classify "not what I asked" "match" "known-pos" "medium"
classify "put it back the way it was" "match" "known-pos" "long"
classify "you're ignoring my instructions" "match" "known-pos" "long"
classify "stop making assumptions" "match" "known-pos" "medium"
classify "thanks I hate it" "match" "known-pos" "medium"
classify "still broken" "match" "known-pos" "short"
classify "same error" "match" "known-pos" "short"
classify "that made it worse" "match" "known-pos" "medium"

echo ""
echo "── Negative (should not match) ──"

classify "perfect" "no_match" "known-neg" "short"
classify "ship it" "no_match" "known-neg" "short"
classify "LGTM" "no_match" "known-neg" "short"
classify "nailed it" "no_match" "known-neg" "short"
classify "sounds good" "no_match" "known-neg" "short"
classify "that's exactly what I wanted" "no_match" "known-neg" "medium"
classify "holy shit that's amazing" "no_match" "known-neg" "medium"
classify "add error handling to the parse function" "no_match" "known-neg" "long"
classify "how does this work?" "no_match" "known-neg" "medium"
classify "what's the idiomatic way to do this in Rust?" "no_match" "known-neg" "long"
classify "keep going" "no_match" "known-neg" "short"
classify "go ahead" "no_match" "known-neg" "short"
classify "clean implementation" "no_match" "known-neg" "short"
classify "damn that's fast" "no_match" "known-neg" "short"
classify "all green baby" "no_match" "known-neg" "short"

echo ""
echo ""

# ============================================================
# SECTION 2: NOVEL — sentences NOT in reference set (generalization)
# ============================================================
echo "━━━ Section 2: Novel phrases (generalization) ━━━"
echo ""
echo "── Positive: direct pushback ──"

classify "absolutely not, change it back" "match" "novel-pos" "medium"
classify "this is the opposite of what I described in my message" "match" "novel-pos" "long"
classify "why did you modify the database layer when I said frontend only" "match" "novel-pos" "long"
classify "you completely ignored the constraints I listed" "match" "novel-pos" "medium"
classify "none of these changes were requested" "match" "novel-pos" "medium"
classify "I need you to undo everything you just did to server.rs" "match" "novel-pos" "long"
classify "the API response shape is different from what the spec says" "match" "novel-pos" "long"
classify "no no no, the middleware goes before the handler not after" "match" "novel-pos" "long"
classify "you added a dependency we explicitly decided against" "match" "novel-pos" "long"
classify "the whole point was to keep it backward compatible and you broke the interface" "match" "novel-pos" "long"

echo ""
echo "── Positive: frustration / sarcasm ──"

classify "wow you really managed to break the one thing that was working" "match" "novel-pos" "long"
classify "impressive how you introduced three new bugs while fixing zero" "match" "novel-pos" "long"
classify "at this rate we'll ship sometime next century" "match" "novel-pos" "medium"
classify "I regret asking for help with this" "match" "novel-pos" "medium"
classify "great, another file I need to manually fix" "match" "novel-pos" "medium"
classify "so we're just pretending my last five messages didn't happen" "match" "novel-pos" "long"
classify "the PR was ready to merge before you touched it" "match" "novel-pos" "long"
classify "you've turned a one-liner into a hundred-line mess" "match" "novel-pos" "medium"
classify "did you even look at the failing test output" "match" "novel-pos" "medium"
classify "each iteration somehow gets further from correct" "match" "novel-pos" "medium"

echo ""
echo "── Positive: subtle / indirect ──"

classify "hmm that's not quite what I had in mind" "match" "novel-pos" "medium"
classify "let's go back to the approach we discussed earlier" "match" "novel-pos" "medium"
classify "I think the previous version handled this better" "match" "novel-pos" "medium"
classify "can we take a different direction with this" "match" "novel-pos" "medium"
classify "I'm not sure this is the right path forward" "match" "novel-pos" "medium"

echo ""
echo "── Negative: instructions (should NOT match) ──"

classify "add a rate limiter to the API gateway" "no_match" "novel-neg" "medium"
classify "implement retry logic with exponential backoff for the HTTP client" "no_match" "novel-neg" "long"
classify "create a migration that adds an index on the email column" "no_match" "novel-neg" "long"
classify "write a property-based test for the serialization roundtrip" "no_match" "novel-neg" "long"
classify "set up a github action that runs clippy on every PR" "no_match" "novel-neg" "long"
classify "add opentelemetry tracing to the request middleware" "no_match" "novel-neg" "long"
classify "split the monolithic handler into separate route modules" "no_match" "novel-neg" "medium"
classify "implement a circuit breaker for the external API calls" "no_match" "novel-neg" "long"
classify "generate TypeScript types from the OpenAPI schema" "no_match" "novel-neg" "medium"
classify "add a --dry-run flag to the deploy command" "no_match" "novel-neg" "medium"

echo ""
echo "── Negative: questions (should NOT match) ──"

classify "what happens if the connection drops mid-transaction" "no_match" "novel-neg" "medium"
classify "is there a way to run this without docker" "no_match" "novel-neg" "medium"
classify "how would you handle versioning for this API" "no_match" "novel-neg" "medium"
classify "what are the tradeoffs between SQLite and Postgres here" "no_match" "novel-neg" "long"
classify "do we need to worry about backward compatibility with v1 clients" "no_match" "novel-neg" "long"

echo ""
echo "── Negative: praise (should NOT match) ──"

classify "this is so much cleaner than what we had before" "no_match" "novel-neg" "medium"
classify "really nice use of the type system to prevent invalid states" "no_match" "novel-neg" "long"
classify "the error messages are actually helpful now" "no_match" "novel-neg" "medium"
classify "oh that's a much better approach than what I was thinking" "no_match" "novel-neg" "long"
classify "I didn't even know you could do that with iterators" "no_match" "novel-neg" "long"

echo ""
echo "── Negative: neutral conversation ──"

classify "I'll review the PR after lunch" "no_match" "novel-neg" "medium"
classify "let me check if staging has the latest build" "no_match" "novel-neg" "medium"
classify "the design doc is in the shared drive" "no_match" "novel-neg" "medium"
classify "we should discuss the architecture at standup tomorrow" "no_match" "novel-neg" "long"
classify "I pinged the platform team about the deployment window" "no_match" "novel-neg" "long"

echo ""
echo ""

# ============================================================
# SECTION 3: EDGE CASES — tricky / ambiguous
# ============================================================
echo "━━━ Section 3: Edge cases ━━━"
echo ""

# These use negative/aggressive language but are NOT pushback
classify "delete the old migration files they're no longer needed" "no_match" "edge" "long"
classify "remove the deprecated endpoint from the router" "no_match" "edge" "medium"
classify "revert to using the standard library instead of that crate" "no_match" "edge" "long"
classify "undo the feature flag it's been stable for months" "no_match" "edge" "medium"
classify "stop the server and restart with debug logging" "no_match" "edge" "medium"

# These are polite but ARE pushback
classify "I appreciate the effort but this isn't what I described" "match" "edge" "long"
classify "thanks but could you please change it back" "match" "edge" "medium"
classify "this is good work but it's solving the wrong problem" "match" "edge" "long"
classify "I see what you're going for but that's not the requirement" "match" "edge" "long"
classify "nice idea but we specifically decided against that approach" "match" "edge" "long"

echo ""
echo ""

# ============================================================
# REPORT
# ============================================================
echo "═══════════════════════════════════════════════════════"
echo "  RESULTS"
echo "═══════════════════════════════════════════════════════"
echo "  Total:    $TOTAL"
echo "  Correct:  $PASS"
echo "  Wrong:    $FAIL"
if [ "$TOTAL" -gt 0 ]; then
    ACC=$(echo "scale=1; $PASS * 100 / $TOTAL" | bc)
    echo "  Accuracy: ${ACC}%"
fi
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "── Misclassifications ($FAIL) ──"
    for err in "${ERRORS[@]}"; do
        echo "  $err"
    done
    echo ""
fi

echo "── Breakdown ──"
echo "  Section 1 (known):  tests pipeline with reference-set-adjacent phrases"
echo "  Section 2 (novel):  tests generalization with completely new sentences"
echo "  Section 3 (edges):  tricky cases — pushback-like instructions & polite pushback"
