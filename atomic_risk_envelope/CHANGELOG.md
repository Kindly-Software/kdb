# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- Initial API surface inventory for `atomic_risk_envelope` (ARE) with typed flags, envelope packing, and order gate helpers.
- Atomic helpers for debiting daily loss and fetch-update workflows; serde support for envelopes and flags.
- Examples showcasing single-account and multi-account integration paths.
- Integration tests covering Topstep rule scenarios and multi-threaded debit stress.
- Offline gateway CLI with configurable threads/resets and JSON bootstrap (trade-secret only).
- CI workflow running `fmt`, `clippy`, unit tests, and example builds on push/pull requests.
- Structured error helpers (`FieldErrorKind`, `DenyReason::code`) for richer telemetry.

### Stability
- Public API is undergoing validation; expect potential breaking changes until `v1.0.0` is tagged.
- Upcoming milestone: freeze modules under `atomic_risk_envelope::{flag, Fields, RiskEnvelope, AtomicRiskEnvelope}` and document semver guarantees.
