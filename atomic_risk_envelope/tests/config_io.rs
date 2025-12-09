#![cfg(feature = "serde")]

use atomic_risk_envelope::{flag, Fields, RiskEnvelope};

#[test]
fn parse_fields_from_json() {
    let json = r#"[
        {
            "rem_daily_loss_cents": 50000,
            "max_per_trade_cents": 20000,
            "max_contracts": 8,
            "max_open_ms": 60000,
            "forbid_after_min_ct": 900,
            "eod_flat_min_ct": 930,
            "flags": 0,
            "version": 1,
            "sequence": 0
        },
        {
            "rem_daily_loss_cents": 80000,
            "max_per_trade_cents": 25000,
            "max_contracts": 10,
            "max_open_ms": 90000,
            "forbid_after_min_ct": 880,
            "eod_flat_min_ct": 915,
            "flags": 1,
            "version": 2,
            "sequence": 4
        }
    ]"#;

    let fields: Vec<Fields> = serde_json::from_str(json).expect("json parse");
    assert_eq!(fields.len(), 2);
    assert!(fields[1].flags.contains(flag::PAUSED));

    let envelopes: Vec<_> = fields
        .into_iter()
        .map(|f| RiskEnvelope::try_from_fields(f).expect("valid env"))
        .collect();
    assert_eq!(envelopes[0].rem_daily_loss_cents(), 50_000);
    assert_eq!(envelopes[1].forbid_after_min_ct(), 880);
}
