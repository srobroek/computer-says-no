# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4](https://github.com/srobroek/computer-says-no/compare/v0.2.3...v0.2.4) - 2026-04-05

### Bug Fixes

- add ci-ok gate job and fix clippy typo ([#175](https://github.com/srobroek/computer-says-no/pull/175))
- gate release on CI, update hooks and docs ([#173](https://github.com/srobroek/computer-says-no/pull/173))

### Miscellaneous

- pin GitHub Actions to commit SHAs
- pin GitHub Actions to commit SHAs
- add CODEOWNERS for CI security

### Performance

- *(ci)* replace rust-cache with sccache ([#176](https://github.com/srobroek/computer-says-no/pull/176))

## [0.2.3](https://github.com/srobroek/computer-says-no/compare/v0.2.2...v0.2.3) - 2026-04-02

### Bug Fixes

- gate release workflow on CI success ([#170](https://github.com/srobroek/computer-says-no/pull/170))
- use conventional semver bumps instead of always-minor

## [0.2.2](https://github.com/srobroek/computer-says-no/compare/v0.2.1...v0.2.2) - 2026-04-02

### Bug Fixes

- release workflow — correct release-plz output parsing

## [0.2.1](https://github.com/srobroek/computer-says-no/compare/v0.2.0...v0.2.1) - 2026-04-02

### Documentation

- fix default model to nomic-embed-text-v1.5-Q, clean benchmark table
- simplify dataset section, general-purpose classifier note

## [0.2.0](https://github.com/srobroek/computer-says-no/compare/v0.1.0...v0.2.0) - 2026-04-02

### Bug Fixes

- clean up hook — remove stderr, generic memory instruction
- add LEARN step to correction hook path
- correct reference sets path in README for macOS

### Documentation

- cleaner flowchart for architecture diagram
- rewrite README — agent-agnostic, daemon explainer, dataset ideas ([#167](https://github.com/srobroek/computer-says-no/pull/167))
- add why section, update benchmark for multi-category ([#166](https://github.com/srobroek/computer-says-no/pull/166))
- install script, reference set location guide
- revert GitHub releases section to original
- fix GitHub releases section — binaries not yet available

### Features

- show hook detection to user via stderr

### Miscellaneous

- add release environment with OIDC for trusted publishing
