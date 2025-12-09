# ASF-256 — Atomic Slip/Fee Surface

`atomic_slip_fee_surface` implements the ASF-256 capsule: a two-word, cache-line
aligned atomic surface that exposes routed fee schedules and a compact maker / taker
slippage model from a single snapshot read. The writer publishes both 128-bit
lanes and flips the commit bit with one release store; readers evaluate fees plus
expected slippage in O(1) without chasing pointers or taking locks.

## Field Layout

The capsule is split across two 128-bit words that pack the following fields:

```
W0: commit | stale | ver_even_u8 | seq_head_u16 | size_scale_q8.8 | maker_fee_bp_q8.8
  | taker_fee_bp_q8.8 | misc_fee_bp_q8.8 | b1_vol_q4.8 | b2_spread_q2.8
  | age_ms_bucket_u8 | flags_u8

W1: a0_m_q6.8 | a1_m_q4.8 | a2_m_q0.8 | c_m_lat_q4.8 | a0_t_q6.8 | a1_t_q4.8
  | a2_t_q0.8 | c_t_lat_q4.8 | slip_cap_m_bp_u10 | slip_cap_t_bp_u10
  | ver_tail_u8 | reserved_u8
```

* Q8.8 fields store unsigned basis points or scale factors with a precision of
  1/256.
* Q6.8 fields encode signed intercepts with saturation in the range
  −64.0..=+63.996 bp.
* Q4.8 and Q0.8 lanes cover sensitivity to size, volatility, spread, and latency.
* Caps provide explicit guard rails for the maker / taker lanes.

`ver_tail` is written with the odd working version, while `ver_even` carries the
committed even version. Readers verify the pair matches to reject torn reads.

## Writer Path

```rust
use atomic_slip_fee_surface::{flag, Asf256, AsfSnapshotBuilder, Flags, LanePublish};

let slot = Asf256::new();
let publish = AsfSnapshotBuilder::builder()
    .with_size_scale(0.25)
    .with_maker_fee_bp(0.42)
    .with_taker_fee_bp(0.42)
    .with_misc_fee_bp(0.10)
    .with_shared_vol_coeff(0.50)
    .with_shared_spread_coeff(0.60)
    .with_maker_lane(|_| LanePublish {
        intercept_bp: 0.40,
        size_linear_bp: 0.15,
        size_quadratic_bp: 0.05,
        latency_coeff_bp: 0.20,
        slip_cap_bp: 6.0,
    })
    .with_taker_lane(|_| LanePublish {
        intercept_bp: 0.25,
        size_linear_bp: 0.08,
        size_quadratic_bp: 0.02,
        latency_coeff_bp: 0.10,
        slip_cap_bp: 10.0,
    })
    .with_flags(flag::HAS_DATA_M | flag::HAS_DATA_T)
    .build();

slot.publish(&publish);
```

The writer hydrates the snapshot from estimator coefficients, fills in the odd
version for the tail word, and commits the even version with a single
`store(Ordering::Release)` on `W0`.

## Reader Path

```rust
if let Some(surface) = slot.load_relaxed() {
    if surface.is_fresh() {
        let maker_slip = surface.estimate_maker_slip(
            contracts,
            vol_bp,
            spread_ticks,
            rtt_ms,
            jitter_ms,
            0.5,
        );
        let fees = surface.maker_total_fee_bp();
        // feed into ACT-128 or pricing logic
    }
}
```

Readers perform two relaxed loads, cross-check the versions, and immediately
consume quantized coefficients. Latency weighting factors (λ) are policy-driven
and applied on the reader side.

## Tests

The crate ships with unit and property tests that validate packing ranges,
quantizer saturation, and atomic publish semantics. Run the suite with:

```
cargo test
```

## License

Licensed under either of

* Apache License, Version 2.0
* MIT license

at your option.
