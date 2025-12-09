# API Surface Inventory

This document tracks the public items exported by the `atomic_risk_envelope` crate so that
we can reason about stability and versioning commitments.

## Modules

- `flag`
  - `Flags` newtype with bitmask operations (`bits`, `contains`, `intersects`, etc.).
  - Flag constants: `PAUSED`, `EMERGENCY_FLAT`, `NEWS_LOCKOUT`, `LIQUIDATION_ONLY`,
    `VENUE_HALT`, `CUSTOM`, composites (`HARD_FLAT`, `HALT`, `NEWS_LOCK`, `ALL`).
  - `MASK` guard constant.
- Root module
  - Structs: `Fields`, `RiskEnvelope`, `AtomicRiskEnvelope`, `OrderCheck`.
  - Enums: `FieldError`, `GateOutcome`, `DenyReason`.
  - Associated functions:
    - `RiskEnvelope::{from_bits, try_from_bits, try_from_fields, to_fields, bits, sequence, version, flags, eod_flat_min_ct, forbid_after_min_ct, max_open_ms, max_contracts, max_per_trade_cents, rem_daily_loss_cents}`.
    - Mutators: `RiskEnvelope::{with_sequence, with_rem_daily_loss_cents, with_flags, update_flags, debit_daily_loss, saturating_debit_daily_loss}`.
    - `AtomicRiskEnvelope::{new, load, load_validated, store, swap, compare_exchange, fetch_update, debit_daily_loss}`.
    - `OrderCheck::new` constructor.
  - Free helpers (currently private): `validate_fields`, `pack_fields`, `check_range`.

## Planned Stability (draft)

- **Guaranteed in 0.1.x**: `Fields`, `RiskEnvelope`, `AtomicRiskEnvelope`, `OrderCheck`,
  `FieldError`, `GateOutcome`, `DenyReason`, primary flag constants, `flag::Flags` operations.
- **Provisional**: `RiskEnvelope::update_flags`, `RiskEnvelope::saturating_debit_daily_loss`,
  `AtomicRiskEnvelope::fetch_update`, serde feature interactions.
- **Out of scope**: Examples, tests, and private helpers.

We will snapshot this inventory before each tagged release and record API additions or
breaking changes in `CHANGELOG.md`.
