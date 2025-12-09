# DOS-1024 — Depth-of-Market Slice

`atomic_depth_of_market_slice` implements the DOS-1024 capsule: a single 1024-bit snapshot that
encodes the top five bid/ask levels for two instruments plus derived microstructure signals. The
capsule is designed for single-writer / many-reader (`SWeMR`) workloads so downstream systems can
consume the top-of-book for a pair of symbols with one cache-line read.

## Capsule layout

The capsule stores eight 128-bit words aligned to 64 bytes. Writers stage words `W1..W7`, compute
integrity metadata, then flip `W0` from an odd to an even version using a single `store(Release)`.
Readers perform relaxed loads, check the version/sequencer head and tail, and reject any snapshot
with an odd version or mismatched integrity.

### W0 — Commit header

```text
commit:1 | stale:1 | ver_even:8 | seq_head:16 | sym_a_id:16 | sym_b_id:16
| created_ms_coarse:24 | forbid_after_min_ct:11 | eod_flat_min_ct:11 | flags:14 | spare:10
```

> The published spec reserves 16 bits for `spare`; the 1024-bit budget only allows 10, so the crate
> trims the spare field accordingly while keeping all mandatory bits intact.

### W1..W3 — Instrument A payload

- `W1`: header, B1, A1, B2
- `W2`: A2, B3, A3, B4
- `W3`: A4, B5, A5, sums (bid/ask L1-3)

Depth levels are 32-bit chunks `(px_ticks_s16, qty_u16)` stored in strict
`B1,A1,B2,A2,…,B5,A5` order. The sums word packs `sum_bid_L1_3_u16 | sum_ask_L1_3_u16` in the upper
32 bits; no additional spare bits remain once the mandated data is encoded.

### W4..W6 — Instrument B payload

Identical packing to instrument A.

### W7 — Derived metrics and integrity

```text
A_spread_u8 | A_obi_q1_10_s12 | A_micro_off_s12 | A_sweep_u1 | A_trend_s11
| B_spread_u8 | B_obi_q1_10_s12 | B_micro_off_s12 | B_sweep_u1 | B_trend_s11
| checksum16_u16 | ver_tail_u8 | seq_tail_u16
```

- `spread` = best ask − best bid (ticks, ≥ 0)
- `obi_q1_10` = signed imbalance using top-three depth
- `micro_off` = microprice offset vs. mid (ticks)
- `sweep` flag decays over ≈200 ms when sweeps detected
- `trend` = 200 ms mid-price drift (ticks)
- `checksum16` = CRC16 over `W1..W6`
- `ver_tail` stores the odd staging version; `ver_even` in `W0` must equal `ver_tail + 1`

## Writer contracts

- Single writer per symbol pair, staging `W1..W6` followed by `W7`
- `stale` bit signals snapshots older than the permissible age budget
- Head/tail validation rules:
  - `ver_even` must be even and match `ver_tail + 1`
  - `seq_head` must equal `seq_tail`
  - `checksum16` must match a recompute of `W1..W6`

The provided `DosWriter` helper (enabled with the `std` feature) implements all packing details,
maintains the odd/even version handshake, detects sweeps, and computes spreads, imbalance, micro
offset, and 200 ms trend.

## Testing

- Unit tests cover packing and unpacking of headers, levels, derived metrics, and checksum checks.
- Property tests (with `proptest`) ensure clamping and integrity constraints stay within bounds.

## Feature flags

- `std`: enables the high-level writer, sweep detector, and benchmarking utilities; the core capsule
  remains `no_std` and depends only on `portable-atomic`.

## License

Dual-licensed under Apache-2.0 or MIT.
