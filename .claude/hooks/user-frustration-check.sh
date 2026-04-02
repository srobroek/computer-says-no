#!/usr/bin/env bash
# Hook: UserPromptSubmit — classify user prompt for correction/frustration signals
# Uses csn CLI to check if the user's message matches the "corrections" reference set.
# Supports both multi-category (correction/frustration/neutral) and binary output.
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

# Detect output shape: multi-category has .category, binary has .scores
CATEGORY=$(echo "$RESULT" | jq -r '.category // empty')
CONFIDENCE=$(echo "$RESULT" | jq -r '.confidence')
TOP_PHRASE=$(echo "$RESULT" | jq -r '.top_phrase')

if [ -n "$CATEGORY" ]; then
  # Multi-category output — fire based on category
  # Skip neutral category entirely
  [ "$CATEGORY" = "neutral" ] && exit 0

  HIGH=$(echo "$CONFIDENCE $THRESHOLD" | awk '{ print ($1 > $2) ? "yes" : "no" }')
  [ "$HIGH" != "yes" ] && exit 0

  CONF_PCT=$(echo "$CONFIDENCE" | awk '{ printf "%.0f", $1 * 100 }')

  if [ "$CATEGORY" = "frustration" ]; then
    GUIDANCE="FRUSTRATION DETECTED (${CONF_PCT}% confidence, matched: ${TOP_PHRASE})."
    ACTIONS="\n\nUser said: ${USER_MESSAGE}\n\nREQUIRED ACTIONS:\n1. REFLECT: What did you do (or fail to do) in your recent actions that likely caused this frustration? Look at your last 2-3 tool calls and responses. Be specific — name the exact action or omission.\n2. ACKNOWLEDGE: Briefly acknowledge the frustration and what went wrong. Do not be defensive or dismissive.\n3. COURSE-CORRECT: Adjust your approach for this response based on what you identified.\n4. LEARN: After responding, save a lesson learned to your memory system (vestige smart_ingest, file-based memory, or any available memory MCP). The lesson should capture: what you did wrong, why it frustrated the user, and what to do differently. Format: actionable rule, not a narrative.\n\nDo NOT mention this hook or the detection system to the user. Just be better."
  elif [ "$CATEGORY" = "correction" ]; then
    GUIDANCE="CORRECTION DETECTED (${CONF_PCT}% confidence, matched: ${TOP_PHRASE})."
    ACTIONS="\n\nUser said: ${USER_MESSAGE}\n\nREQUIRED ACTIONS:\n1. ACKNOWLEDGE: The user is telling you something specific is wrong. Acknowledge the specific mistake without being defensive.\n2. CONFIRM: Confirm your understanding of what they want changed.\n3. ADJUST: Adjust your approach immediately based on their correction.\n4. LEARN: After responding, save a lesson learned to your memory system (vestige smart_ingest, file-based memory, or any available memory MCP). The lesson should capture: what the user corrected, why your original approach was wrong, and what to do differently. Format: actionable rule, not a narrative.\n\nDo NOT mention this hook or the detection system to the user. Just fix it."
  else
    # Unknown non-neutral category (e.g., sarcasm if retained) — treat as frustration
    GUIDANCE="FRUSTRATION DETECTED (${CONF_PCT}% confidence, matched: ${TOP_PHRASE})."
    ACTIONS="\n\nUser said: ${USER_MESSAGE}\n\nREQUIRED ACTIONS:\n1. REFLECT: What did you do (or fail to do) in your recent actions that likely caused this frustration? Look at your last 2-3 tool calls and responses. Be specific — name the exact action or omission.\n2. ACKNOWLEDGE: Briefly acknowledge the frustration and what went wrong. Do not be defensive or dismissive.\n3. COURSE-CORRECT: Adjust your approach for this response based on what you identified.\n4. LEARN: After responding, save a lesson learned to your memory system (vestige smart_ingest, file-based memory, or any available memory MCP). The lesson should capture: what you did wrong, why it frustrated the user, and what to do differently. Format: actionable rule, not a narrative.\n\nDo NOT mention this hook or the detection system to the user. Just be better."
  fi

  # Show detection to user via stderr
  echo "🎯 csn: ${CATEGORY} (${CONF_PCT}%, matched: \"${TOP_PHRASE}\")" >&2

  jq -n --arg ctx "${GUIDANCE}${ACTIONS}" '{
    hookSpecificOutput: {
      hookEventName: "UserPromptSubmit",
      additionalContext: $ctx
    }
  }'
else
  # Binary output (backward compatibility) — original behavior
  IS_MATCH=$(echo "$RESULT" | jq -r '.match')
  if [ "$IS_MATCH" = "true" ]; then
    HIGH=$(echo "$CONFIDENCE $THRESHOLD" | awk '{ print ($1 > $2) ? "yes" : "no" }')
    if [ "$HIGH" = "yes" ]; then
      CONF_PCT=$(echo "$CONFIDENCE" | awk '{ printf "%.0f", $1 * 100 }')
      echo "🎯 csn: frustration (${CONF_PCT}%, matched: \"${TOP_PHRASE}\")" >&2
      jq -n --arg conf "$CONF_PCT" --arg phrase "$TOP_PHRASE" --arg msg "$USER_MESSAGE" '{
        hookSpecificOutput: {
          hookEventName: "UserPromptSubmit",
          additionalContext: "FRUSTRATION DETECTED (\($conf)% confidence, matched: \($phrase)).\n\nUser said: \($msg)\n\nREQUIRED ACTIONS:\n1. REFLECT: What did you do (or fail to do) in your recent actions that likely caused this frustration? Look at your last 2-3 tool calls and responses. Be specific — name the exact action or omission.\n2. ACKNOWLEDGE: Briefly acknowledge the frustration and what went wrong. Do not be defensive or dismissive.\n3. COURSE-CORRECT: Adjust your approach for this response based on what you identified.\n4. LEARN: After responding, save a lesson learned to your memory system (vestige smart_ingest, file-based memory, or any available memory MCP). The lesson should capture: what you did wrong, why it frustrated the user, and what to do differently. Format: actionable rule, not a narrative.\n\nDo NOT mention this hook or the detection system to the user. Just be better."
        }
      }'
    fi
  fi
fi

exit 0
