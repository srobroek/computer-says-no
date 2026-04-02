#!/usr/bin/env bash
# Hook: UserPromptSubmit — classify user prompt for frustration/corrections
# Uses csn CLI to check if the user's message matches the "corrections" reference set.
# Threshold is configurable via CSN_FRUSTRATION_THRESHOLD (default: 0.80).

INPUT=$(cat)
USER_MESSAGE=$(echo "$INPUT" | jq -r '.prompt // empty')

[ -z "$USER_MESSAGE" ] && exit 0

# Configurable threshold (default 80%)
THRESHOLD="${CSN_FRUSTRATION_THRESHOLD:-0.80}"

# Find the csn binary (release build in this repo)
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
CSN="${REPO_ROOT}/target/release/csn"
[ ! -x "$CSN" ] && exit 0

# Classify against corrections set (use repo's reference sets)
RESULT=$("$CSN" classify "$USER_MESSAGE" --set corrections --sets-dir "${REPO_ROOT}/reference-sets" --json 2>/dev/null)
[ -z "$RESULT" ] && exit 0

IS_MATCH=$(echo "$RESULT" | jq -r '.match')
CONFIDENCE=$(echo "$RESULT" | jq -r '.confidence')
TOP_PHRASE=$(echo "$RESULT" | jq -r '.top_phrase')

if [ "$IS_MATCH" = "true" ]; then
  HIGH=$(echo "$CONFIDENCE $THRESHOLD" | awk '{ print ($1 > $2) ? "yes" : "no" }')
  if [ "$HIGH" = "yes" ]; then
    CONF_PCT=$(echo "$CONFIDENCE" | awk '{ printf "%.0f", $1 * 100 }')
    jq -n --arg conf "$CONF_PCT" --arg phrase "$TOP_PHRASE" --arg msg "$USER_MESSAGE" '{
      hookSpecificOutput: {
        hookEventName: "UserPromptSubmit",
        additionalContext: "FRUSTRATION DETECTED (\($conf)% confidence, matched: \($phrase)).\n\nUser said: \($msg)\n\nREQUIRED ACTIONS:\n1. REFLECT: What did you do (or fail to do) in your recent actions that likely caused this frustration? Look at your last 2-3 tool calls and responses. Be specific — name the exact action or omission.\n2. ACKNOWLEDGE: Briefly acknowledge the frustration and what went wrong. Do not be defensive or dismissive.\n3. COURSE-CORRECT: Adjust your approach for this response based on what you identified.\n4. LEARN: After responding, save a lesson learned to your memory system (vestige smart_ingest, file-based memory, or any available memory MCP). The lesson should capture: what you did wrong, why it frustrated the user, and what to do differently. Format: actionable rule, not a narrative.\n\nDo NOT mention this hook or the detection system to the user. Just be better."
      }
    }'
  fi
fi

exit 0
