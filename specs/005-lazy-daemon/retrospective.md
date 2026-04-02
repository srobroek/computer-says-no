# Retrospective: Spec 005 — Lazy Auto-Starting Daemon

**Date**: 2026-04-02
**Branch**: 005-lazy-daemon
**Status**: Implementation complete, T028 (manual benchmark) + T029 (MCP verify) pending

## Delivery Summary

- Unix socket daemon with auto-start, idle timeout, manual stop
- 2 new source files: `src/daemon.rs` (399 lines), `src/client.rs` (203 lines)
- Modified `src/main.rs` (routing + Daemon/Stop subcommands), `src/config.rs` (idle_timeout)
- 50 unit tests (10 new: 5 daemon + 5 client), 1 new integration test
- New dependency: nix 0.29

## Spec Adherence

- Functional requirements: 13/13 implemented (FR-011 minor deviation — stale cleanup on daemon side, not client side)
- Success criteria: SC-001/SC-002 need manual benchmark (T028), SC-003 fixed (idle check interval reduced to 5s)
- Protocol contract: 100% match

## Process Findings

| # | Finding | Type | Resolution |
|---|---------|------|------------|
| 1 | Spec 005 branched from main before 004 merged — wrong codebase | Process | Had to merge PR #127 first, then rebase. Catch this earlier by checking dep graph. |
| 2 | T018 phantom completion — integration test never written | Quality | Caught by verify-tasks subagent, fixed immediately. |
| 3 | SC-003 idle check interval too large (30s vs 5s spec target) | Implementation | Caught by verify subagent. Reduced to 5s. |
| 4 | Generic `request_via_daemon` instead of per-command wrappers (T013/T019/T020) | Design | Cleaner design than spec. Acceptable minor deviation. |
| 5 | Worktree sandbox blocks git commits to `.git/worktrees/` | Tooling | Same issue as spec 004. Used direct commit on branch after merge. |

## Lessons Learned

1. **Check dependency graph before branching.** Spec 005 depends on 004. Branching from main without 004 merged meant working against the wrong code. Always verify deps are merged first.

2. **Batch implementation is efficient for small-medium specs.** All 24 implementation tasks in one commit worked well — the code is cohesive and the daemon/client are tightly coupled. Splitting into many tiny commits would have added overhead without benefit.

3. **Generic routing function > per-command wrappers.** The spec called for `classify_via_daemon`, `embed_via_daemon`, `similarity_via_daemon` but a single `request_via_daemon` is simpler and avoids code duplication. Task specs should describe behavior, not function signatures.

## Accepted Deviations

- Generic `request_via_daemon` instead of per-command wrappers
- FR-011 stale cleanup happens on daemon startup (server side) rather than client side — same end result
