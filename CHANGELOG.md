# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/srobroek/computer-says-no/releases/tag/v0.1.0) - 2026-04-02

### Bug Fixes

- *(ci)* use dtolnay/rust-toolchain@1.92.0 tag directly
- unmark T025 — accuracy validation requires manual run outside sandbox
- add p99 to table output and latency to comparison report
- resolve CI failures — commit Cargo.lock, fix clippy useless_vec

### Documentation

- comprehensive README, release-plz for binary publishing ([#162](https://github.com/srobroek/computer-says-no/pull/162))
- add project README with setup instructions and hook guide ([#160](https://github.com/srobroek/computer-says-no/pull/160))
- *(retrospective)* spec 002 adherence report — 93% adherence, 96% completion
- align spec FR-001 CLI command with implementation
- mark T012, T015-T020 complete — scaffold, filters, comparison all implemented
- tasks.md for spec 002 — 26 tasks across 7 phases
- complete spec 002 plan — research, data model, contracts
- 500 prompts/dataset, 6 difficulty tiers, 30min SC-001
- clarify dataset generation (LLM) and iteration count (20)
- add cold startup latency to spec 002 benchmark
- spec 002 — model benchmark harness

### Features

- MLP multi-category classification (spec 008) ([#161](https://github.com/srobroek/computer-says-no/pull/161))
- character n-gram features for typo robustness (spec 007) ([#159](https://github.com/srobroek/computer-says-no/pull/159))
- lazy auto-starting daemon (spec 005) ([#157](https://github.com/srobroek/computer-says-no/pull/157))
- MCP stdio server (spec 004) ([#127](https://github.com/srobroek/computer-says-no/pull/127))
- MLP pushback classifier (spec 003) ([#100](https://github.com/srobroek/computer-says-no/pull/100))
- pushback detection — 3-lens reference set, strategy comparison, MLP prototype
- add 3 new reference sets and 500-prompt datasets
- T023 — benchmark integration tests, all 26 tasks complete
- T013-T014 — generate 500-prompt labeled datasets
- wave 3 polish — clippy clean, justfile bench targets, task updates
- wave 2 — benchmark orchestration, table output, JSON, comparison
- wave 1 foundational — CLI subcommands, scaffold generation
- wave 0 setup — benchmark and dataset types, deps, config
- spec 001 — core binary with CLI and REST daemon ([#36](https://github.com/srobroek/computer-says-no/pull/36))
- initial scaffold with core embedding engine

### Miscellaneous

- tune frustration hook — add mild phrases, configurable threshold ([#158](https://github.com/srobroek/computer-says-no/pull/158))
- pin Rust 1.92, update docs, add spec dependency graph
- add Rust formatting and linting pre-commit hooks ([#33](https://github.com/srobroek/computer-says-no/pull/33))
- initialize speckit with constitution and community extensions
- exclude .claude, .specify, specs from pre-commit hooks

### Refactoring

- replace unwrap on locks with error responses, extract measure_dataset

### Style

- rustfmt
