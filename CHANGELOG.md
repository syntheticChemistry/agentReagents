# Changelog

All notable changes to agentReagents are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] — 2026-04-05

### Added
- Root `CHANGELOG.md` (canonical; `docs/HISTORY.md` retains Dec 2025 bootstrap notes)
- `deny.toml`

### Changed (Deep Debt Resolution Sprint)
- Coverage: 7.1% → 60.2% (89 tests)
- Hardcoded Songbird registration → capability-based RegistrationSettings
- All `#[allow(` → `#[expect(` with reasons
- README: license aligned to -or-later, build requirements documented, security note added
- Archive paths scrubbed of machine-specific paths
- `tarpaulin.toml` with `fail-under=60.0`
- C dependencies documented in `deny.toml`
