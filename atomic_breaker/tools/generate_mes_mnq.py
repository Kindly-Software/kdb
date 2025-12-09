#!/usr/bin/env python3
"""Generate synthetic MES/MNQ scalping telemetry datasets.

The script writes CSV files under ``data/mes_mnq/`` with a 10 Hz cadence. Each dataset
covers a specific microstructure regime (normal liquidity, liquidity vacuum, etc.). The
columns align with `data/mes_mnq/README.md`.
"""

from __future__ import annotations

import csv
import random
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional

LATENCY_BUDGET_MS = 6.0
VOL_BUDGET = 1.2  # ticks/s

CAUSE = {
    "THERM": 1,
    "NET": 1 << 1,
    "IO": 1 << 2,
    "MEM": 1 << 3,
    "CPU": 1 << 4,
    "LAT": 1 << 5,
    "JIT": 1 << 6,
    "TIMEOUT": 1 << 7,
}


@dataclass
class CauseProfile:
    bit: str
    prob: float


@dataclass
class ScenarioProfile:
    spread_mean: float
    spread_std: float
    latency_mean: float
    latency_std: float
    fill_mean: float
    fill_std: float
    imbalance_mean: float
    imbalance_std: float
    vol_mean: float
    vol_std: float
    pnl_mean: float
    pnl_std: float
    err_mean: float
    err_std: float
    causes: List[CauseProfile]
    default_cause: Optional[str]
    recovery_model: str
    spread_widen: float = 0.0
    spread_cycle: int = 30
    vol_to_mu_bias: float = 0.0
    mid_drift_per_sample: float = 0.0
    mid_jitter: float = 0.2
    gateway_bias: float = 0.0


def clamp(value: float, low: float, high: float) -> float:
    return max(low, min(high, value))


def generate_row(idx: int, mid: float, profile: ScenarioProfile) -> Dict[str, float]:
    timestamp = idx * 100  # 10 Hz cadence
    mid += profile.mid_drift_per_sample + random.gauss(0.0, profile.mid_jitter)
    spread = clamp(random.gauss(profile.spread_mean, profile.spread_std), 0.5, 8.0)
    if idx % profile.spread_cycle == 0:
        spread += profile.spread_widen
    latency = clamp(random.gauss(profile.latency_mean, profile.latency_std), 1.5, 60.0)
    fill_ratio = clamp(random.gauss(profile.fill_mean, profile.fill_std), 0.0, 1.0)
    imbalance = clamp(random.gauss(profile.imbalance_mean, profile.imbalance_std), -1.0, 1.0)
    micro_vol = clamp(random.gauss(profile.vol_mean, profile.vol_std), 0.0, 6.0)
    pnl_ticks = random.gauss(profile.pnl_mean, profile.pnl_std)
    err_inc = max(0, int(random.gauss(profile.err_mean, profile.err_std)))

    cause_mask = 0
    for cause in profile.causes:
        if random.random() < cause.prob:
            cause_mask |= CAUSE[cause.bit]
    if cause_mask == 0 and profile.default_cause:
        cause_mask = CAUSE[profile.default_cause]

    mu_norm = clamp(latency / LATENCY_BUDGET_MS, 0.0, 64.0)
    sg_norm = clamp(micro_vol / VOL_BUDGET, 0.0, 64.0)
    mu_norm += profile.vol_to_mu_bias * abs(imbalance)

    if profile.recovery_model == "good":
        success_window = int(clamp(random.gauss(45, 12), 18, 95))
        recovered = True
    elif profile.recovery_model == "mixed":
        success_window = int(clamp(random.gauss(125, 65), 40, 260))
        recovered = random.random() < 0.6
    else:  # poor
        success_window = int(clamp(random.gauss(280, 90), 120, 540))
        recovered = random.random() < 0.25

    primary_latency = clamp(latency * (1.0 + profile.gateway_bias) + random.gauss(0.0, 1.2), 2.0, 80.0)
    secondary_latency = clamp(latency * (0.8 - profile.gateway_bias) + random.gauss(0.0, 1.8), 2.5, 100.0)
    primary_queue = clamp(random.gauss(18.0 - 8.0 * profile.gateway_bias, 4.0), 2.0, 40.0)
    secondary_queue = clamp(random.gauss(12.0 + 6.0 * profile.gateway_bias, 5.0), 1.0, 50.0)
    active_primary = 1 if primary_latency <= secondary_latency else 0

    return {
        "timestamp_ms": timestamp,
        "mid_price": round(mid, 2),
        "spread_ticks": round(spread, 2),
        "order_latency_ms": round(latency, 2),
        "fill_ratio": round(fill_ratio, 3),
        "imbalance": round(imbalance, 3),
        "micro_vol": round(micro_vol, 3),
        "pnl_ticks": round(pnl_ticks, 3),
        "err_inc": err_inc,
        "cause": cause_mask,
        "mu_norm": round(mu_norm, 3),
        "sg_norm": round(sg_norm, 3),
        "success_window_ms": success_window,
        "recovered_within_target": 1 if recovered else 0,
        "gateway_primary_latency_ms": round(primary_latency, 2),
        "gateway_secondary_latency_ms": round(secondary_latency, 2),
        "queue_depth_primary": round(primary_queue, 2),
        "queue_depth_secondary": round(secondary_queue, 2),
        "active_primary": active_primary,
    }


def generate_dataset(name: str, length: int, base_mid: float, profile: ScenarioProfile, out_dir: Path) -> None:
    path = out_dir / f"{name}.csv"
    with path.open("w", newline="") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=[
                "timestamp_ms",
                "mid_price",
                "spread_ticks",
                "order_latency_ms",
                "fill_ratio",
                "imbalance",
                "micro_vol",
                "pnl_ticks",
                "err_inc",
                "cause",
                "mu_norm",
                "sg_norm",
                "success_window_ms",
                "recovered_within_target",
                "gateway_primary_latency_ms",
                "gateway_secondary_latency_ms",
                "queue_depth_primary",
                "queue_depth_secondary",
                "active_primary",
            ],
        )
        writer.writeheader()
        mid = base_mid
        for idx in range(length):
            row = generate_row(idx, mid, profile)
            mid = row["mid_price"]
            writer.writerow(row)


def main() -> None:
    random.seed(1337)
    out_dir = Path("data/mes_mnq")
    out_dir.mkdir(parents=True, exist_ok=True)

    scenarios: Dict[str, ScenarioProfile] = {
        "normal_liquidity": ScenarioProfile(
            spread_mean=1.2,
            spread_std=0.18,
            latency_mean=3.8,
            latency_std=0.5,
            fill_mean=0.93,
            fill_std=0.04,
            imbalance_mean=0.05,
            imbalance_std=0.18,
            vol_mean=0.55,
            vol_std=0.18,
            pnl_mean=0.16,
            pnl_std=0.07,
            err_mean=0.05,
            err_std=0.15,
            causes=[CauseProfile("LAT", 0.08)],
            default_cause=None,
            recovery_model="good",
            gateway_bias=-0.1,
        ),
        "liquidity_vacuum": ScenarioProfile(
            spread_mean=3.4,
            spread_std=0.8,
            spread_widen=1.2,
            spread_cycle=12,
            latency_mean=12.0,
            latency_std=2.6,
            fill_mean=0.26,
            fill_std=0.11,
            imbalance_mean=-0.55,
            imbalance_std=0.28,
            vol_mean=2.9,
            vol_std=0.75,
            pnl_mean=-0.62,
            pnl_std=0.42,
            err_mean=1.9,
            err_std=0.55,
            causes=[
                CauseProfile("NET", 0.75),
                CauseProfile("LAT", 0.9),
                CauseProfile("TIMEOUT", 0.35),
            ],
            default_cause="LAT",
            recovery_model="poor",
            vol_to_mu_bias=0.5,
            gateway_bias=0.25,
        ),
        "queue_loss": ScenarioProfile(
            spread_mean=1.35,
            spread_std=0.22,
            latency_mean=5.5,
            latency_std=1.0,
            fill_mean=0.23,
            fill_std=0.14,
            imbalance_mean=-0.32,
            imbalance_std=0.34,
            vol_mean=1.55,
            vol_std=0.4,
            pnl_mean=-0.27,
            pnl_std=0.23,
            err_mean=1.05,
            err_std=0.45,
            causes=[
                CauseProfile("JIT", 0.45),
                CauseProfile("LAT", 0.6),
            ],
            default_cause="LAT",
            recovery_model="mixed",
            gateway_bias=0.1,
        ),
        "infrastructure_impairment": ScenarioProfile(
            spread_mean=2.1,
            spread_std=0.45,
            latency_mean=18.2,
            latency_std=3.4,
            fill_mean=0.42,
            fill_std=0.17,
            imbalance_mean=0.0,
            imbalance_std=0.38,
            vol_mean=1.15,
            vol_std=0.42,
            pnl_mean=-0.33,
            pnl_std=0.28,
            err_mean=2.4,
            err_std=0.65,
            causes=[
                CauseProfile("NET", 0.82),
                CauseProfile("IO", 0.58),
                CauseProfile("CPU", 0.5),
                CauseProfile("TIMEOUT", 0.45),
            ],
            default_cause="NET",
            recovery_model="poor",
            gateway_bias=0.35,
        ),
        "over_trading": ScenarioProfile(
            spread_mean=1.05,
            spread_std=0.2,
            latency_mean=4.0,
            latency_std=0.9,
            fill_mean=0.72,
            fill_std=0.11,
            imbalance_mean=0.12,
            imbalance_std=0.24,
            vol_mean=0.95,
            vol_std=0.33,
            pnl_mean=-0.2,
            pnl_std=0.38,
            err_mean=0.85,
            err_std=0.4,
            causes=[
                CauseProfile("LAT", 0.58),
                CauseProfile("JIT", 0.48),
            ],
            default_cause="LAT",
            recovery_model="mixed",
            mid_drift_per_sample=-0.035,
            mid_jitter=0.32,
            gateway_bias=-0.2,
        ),
    }

    for name, profile in scenarios.items():
        base_mid = 4050.0 if name != "over_trading" else 13300.0
        generate_dataset(name, length=360, base_mid=base_mid, profile=profile, out_dir=out_dir)
    print(f"Generated datasets in {out_dir}")


if __name__ == "__main__":
    main()
