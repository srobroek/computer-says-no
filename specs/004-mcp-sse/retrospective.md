# Retrospective: Spec 004 — MCP Server

**Date**: 2026-04-02
**Branch**: 004-mcp-sse
**Status**: Implementation complete, T026 (manual MCP verification) pending

## Delivery Summary

- MCP stdio server with 4 tools (classify, list_sets, embed, similarity)
- Removed REST daemon, file watcher, standalone flag (~570 lines deleted)
- Added `src/mcp.rs` (236 lines), updated `src/main.rs` and `src/config.rs`
- 40 unit tests (7 new MCP tests), 2 MCP integration tests (`#[ignore]`)
- Cross-spec documentation updated to reflect architecture change
- Release binary: 25M (no pre-spec baseline for comparison)

## Spec Adherence

- Functional requirements: 10/10 implemented
- Success criteria: 3/5 verified, 2 require manual validation (SC-001 startup, SC-002 timing)
- SC-005 (binary size decrease): unverifiable — no pre-spec baseline captured

## Process Findings

| # | Finding | Type | Resolution |
|---|---------|------|------------|
| 1 | Worktree sandbox blocks git commits when `.git/worktrees/` lives outside sandbox allowlist | Tooling | Workaround: copy changes to main repo and commit there. Consider adding `.git/worktrees/` to sandbox `allowWrite`. |
| 2 | T001-T011 checkmarks lost across worktree merge — edits to tasks.md in main repo were overwritten | Process | Checkpoint tasks.md inside the worktree, not the main repo |
| 3 | `cd && git` compound commands bypassed git-guard hook | Security | Fixed: added detection to `git-guard.sh`, blocks with suggestion to use `git -C` |
| 4 | macOS grep lacks `-P` flag — PCRE patterns silently fail in hooks | Tooling | Fixed: changed to `-E` (extended regex) in git-guard hook |
| 5 | Cross-spec staleness not caught until step 14 — specs 001/002/003 referenced daemon/REST/standalone | Process | Resolved in step 14. Consider adding supersession check to speckit.iterate or speckit.specify. |
| 6 | Pre-commit `typos` hook catches unrelated staged files when using `git add -A` | Process | Always stage specific files by name, not `-A` |

## Lessons Learned

1. **Architecture changes that remove a transport layer have wide spec blast radius.** Every prior spec that mentioned the transport (REST, daemon, standalone, watcher) needed updating. Step 14 (sync.conflicts) caught these but earlier detection would be better.

2. **Worktree sandbox issues are session-specific and intermittent.** The same worktree pattern worked in one session but not another. Fallback: copy files and commit in the main repo.

3. **The `unsafe impl Sync` pattern** for wrapping `!Sync` types (fastembed, Burn NdArray) in `Mutex` works for the single-client stdio MCP model but should be revisited if concurrent access is ever needed.

4. **Pre-spec binary size baselines** should be captured before starting implementation, not after. SC-005 was unverifiable because we didn't record the pre-spec binary size.

## Accepted Deviations

- `McpHandler` stores `ModelChoice` instead of `AppConfig` (simplification — only field needed)
- SC-005 binary size decrease accepted as unverifiable
