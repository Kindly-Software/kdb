//! Demonstration driver: fixed-step sim tick + optional coarse strategic cadence,
//! wiring shard contexts with terrain/ballistics/fire profiles, artillery target IDs,
//! kgpu overlays, and optional io_uring persistence when compiled with `--features io-uring`.

use kindly_engine::ballistics::{
    apply_fire_control_for_ids, BallisticsCapsule, FireControlProfileCapsule,
};
use kindly_engine::courier::{CourierCapsule, Doctrine};
use kindly_engine::fire_doctrine::{FireDoctrineCapsule, FireDoctrineMode};
use kindly_engine::formation::FormationCapsule;
use kindly_engine::kgpu_bridge::{
    make_doctrine_overlay_from_render, make_supply_heatmap_from_render, try_submit_with_kgpu_driver,
    KgpuTerminalCapsule, RenderOverlayCapsule, SupplyHeatmapLegend, SupplyHeatmapSink,
    TerminalDoctrineSink, TerminalHeatmapSink,
};
use kindly_engine::kgpu_ingest::{KgpuIngestCapsule, KgpuRenderSinkCapsule};
use kindly_engine::math::Q16_16;
use kindly_engine::order::{
    pack_charge_meta, pack_fire_doctrine_payload, pack_fire_meta_extended, pack_fire_payload,
    pack_move_payload, pack_posture_payload, unpack_fire_payload, OrderKind, OrderQueueCapsule,
};
use kindly_engine::pathing::PathingCapsule;
use kindly_engine::physics::PhysicsPreset;
use kindly_engine::supply::{SupplyCapsule, SupplyRoad, SupplySnapshot};
use kindly_engine::telemetry::TelemetryCapsule;
use kindly_engine::terrain::{TerrainGridCapsule, TerrainSnapshot};
use kindly_engine::tick::{
    collect_world_render_slab, make_shard_context, tick_world, RenderSoaSlabCapsule,
};
use kindly_engine::weather::WeatherCapsule;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Coarse clock for strategic layers (runs every `interval` sim ticks).
struct StrategicClock {
    interval: u64,
    next_tick: u64,
}

impl StrategicClock {
    fn new(interval: u64) -> Self {
        Self {
            interval,
            next_tick: interval,
        }
    }

    fn should_fire(&self, sim_tick: u64) -> bool {
        sim_tick >= self.next_tick
    }

    fn advance(&mut self) {
        self.next_tick = self.next_tick.saturating_add(self.interval);
    }
}

fn nearest_target_id(
    formations: &[FormationCapsule],
    target_q16: (u32, u32),
    shooter_id: usize,
) -> Option<usize> {
    let tx = target_q16.0 as i64;
    let tz = target_q16.1 as i64;
    let mut best: Option<(usize, i128)> = None;
    for (idx, formation) in formations.iter().enumerate() {
        if idx == shooter_id {
            continue;
        }
        let snap = formation.snapshot();
        let dx = snap.position_x_q16 as i64 - tx;
        let dz = snap.position_z_q16 as i64 - tz;
        let dist2 = (dx as i128) * (dx as i128) + (dz as i128) * (dz as i128);
        if let Some((_, best_dist)) = best {
            if dist2 < best_dist {
                best = Some((idx, dist2));
            }
        } else {
            best = Some((idx, dist2));
        }
    }
    best.map(|(idx, _)| idx)
}

#[derive(Debug, Clone)]
struct DriverArgs {
    render_out: PathBuf,
    replay_out: PathBuf,
    replay_index: Option<PathBuf>,
    snapshot_out: PathBuf,
    strat_interval: u64,
    strat_seed: u64,
    use_kgpu_driver: bool,
    kgpu_device_index: usize,
    heatmap_enabled: bool,
    supply_graph: Option<PathBuf>,
    weather_script: Option<PathBuf>,
    doctrine_defaults: Option<String>,
}

impl DriverArgs {
    fn parse() -> Self {
        let mut args = env::args().skip(1);
        let mut cfg = Self {
            render_out: PathBuf::from("data/kindly-engine/render_stream.bin"),
            replay_out: PathBuf::from("data/kindly-engine/replay.bin"),
            replay_index: Some(PathBuf::from("data/kindly-engine/replay.idx")),
            snapshot_out: PathBuf::from("data/kindly-engine/snapshot.bin"),
            strat_interval: 5,
            strat_seed: 0xC0DA_C0DA,
            use_kgpu_driver: false,
            kgpu_device_index: 0,
            heatmap_enabled: true,
            supply_graph: None,
            weather_script: None,
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--render-out" => {
                    cfg.render_out = PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--render-out requires a path");
                        std::process::exit(2);
                    }));
                }
                "--replay-out" => {
                    cfg.replay_out = PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--replay-out requires a path");
                        std::process::exit(2);
                    }));
                }
                "--snapshot-out" => {
                    cfg.snapshot_out = PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--snapshot-out requires a path");
                        std::process::exit(2);
                    }));
                }
                "--replay-index" => {
                    cfg.replay_index = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--replay-index requires a path");
                        std::process::exit(2);
                    })));
                }
                "--no-index" => cfg.replay_index = None,
                "--strat-interval" => {
                    let raw = args.next().unwrap_or_else(|| {
                        eprintln!("--strat-interval requires a number");
                        std::process::exit(2);
                    });
                    cfg.strat_interval = raw.parse().unwrap_or_else(|_| {
                        eprintln!("invalid --strat-interval value");
                        std::process::exit(2);
                    });
                }
                "--strat-seed" => {
                    let raw = args.next().unwrap_or_else(|| {
                        eprintln!("--strat-seed requires a number");
                        std::process::exit(2);
                    });
                    cfg.strat_seed = u64::from_str_radix(raw.trim_start_matches("0x"), 16)
                        .unwrap_or_else(|_| {
                            raw.parse().unwrap_or_else(|_| {
                                eprintln!("invalid --strat-seed value");
                                std::process::exit(2);
                            })
                        });
                }
                "--kgpu" => cfg.use_kgpu_driver = true,
                "--kgpu-device" => {
                    let raw = args.next().unwrap_or_else(|| {
                        eprintln!("--kgpu-device requires an index");
                        std::process::exit(2);
                    });
                    cfg.kgpu_device_index = raw.parse().unwrap_or_else(|_| {
                        eprintln!("invalid --kgpu-device value");
                        std::process::exit(2);
                    });
                }
                "--no-heatmap" => cfg.heatmap_enabled = false,
                "--heatmap" => cfg.heatmap_enabled = true,
                "--supply-graph" => {
                    cfg.supply_graph = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--supply-graph requires a path");
                        std::process::exit(2);
                    })));
                }
                "--weather-script" => {
                    cfg.weather_script = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--weather-script requires a path");
                        std::process::exit(2);
                    })));
                }
                "--doctrine-defaults" => {
                    cfg.doctrine_defaults = Some(args.next().unwrap_or_else(|| {
                        eprintln!("--doctrine-defaults requires a comma list (e.g., volley:3,byrank:2,rolling:4,advance:3)");
                        std::process::exit(2);
                    }));
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => {
                    eprintln!("Unknown arg: {other}");
                    print_help();
                    std::process::exit(2);
                }
            }
        }
        cfg
    }

    fn ensure_dirs(&self) {
        let mut paths = vec![
            self.render_out.clone(),
            self.replay_out.clone(),
            self.snapshot_out.clone(),
        ];
        if let Some(idx) = &self.replay_index {
            paths.push(idx.clone());
        }
        for p in paths {
            if let Some(parent) = p.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Failed to create {:?}: {err}", parent);
                    std::process::exit(2);
                }
            }
        }
    }
}

fn print_help() {
    println!(
        "Usage: kindly_driver [options]\n\
         --render-out <path>      Render stream output file for io_uring (default data/kindly-engine/render_stream.bin)\n\
         --replay-out <path>      Replay mmap output (default data/kindly-engine/replay.bin)\n\
         --snapshot-out <path>    Snapshot mmap output (default data/kindly-engine/snapshot.bin)\n\
         --replay-index <path>    Enable replay indexing at path (default data/kindly-engine/replay.idx)\n\
         --no-index               Disable replay index writes\n\
         --strat-interval <n>     Strategic (logistics/diplomacy) tick interval in sim ticks (default 5)\n\
         --strat-seed <hex|dec>   Deterministic strategic RNG seed (default 0xC0DAC0DA)\n\
         --kgpu                   Enable kgpu-driver handshake (Linux only, feature-gated)\n\
        --kgpu-device <idx>      GPU index for kgpu-driver (default 0)\n\
        --no-heatmap             Disable supply/fatigue heatmap submission\n\
        --supply-graph <path>    Optional supply graph file (lines: from to capacity loss distance)\n\
        --weather-script <path>  Optional weather script (lines: tick precip wind gust)\n\
        --doctrine-defaults <list> Comma list (volley:<ticks>,byrank:<ticks>,rolling:<ticks>,advance:<ticks>)"
    );
}

/// Deterministic strategic hooks (logistics/diplomacy) feeding orders on coarse ticks.
struct StrategicHooks {
    lcg: u64,
}

impl StrategicHooks {
    fn new(seed: u64) -> Self {
        Self { lcg: seed }
    }

    fn next(&mut self) -> u64 {
        self.lcg = self.lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.lcg
    }

    fn apply(
        &mut self,
        sim_tick: u64,
        orders: &OrderQueueCapsule,
        formations: &[FormationCapsule],
        supply: &SupplySnapshot,
        courier_eta_hint: Option<u32>,
    ) {
        if formations.is_empty() {
            return;
        }
        // Logistics: rotate stances to simulate supply/formation tightening.
        let posture_seed = self.next();
        let formation_idx = (posture_seed as usize) % formations.len();
        let stance = ((posture_seed >> 8) & 0x3) as u8;
        let payload = pack_posture_payload((posture_seed & 0x3) as u8, stance);
        let _ = orders.push_order(
            OrderKind::ChangePosture,
            formations[formation_idx].snapshot().formation_id,
            payload,
            0,
        );

        // Doctrine preset: occasionally rotate firing doctrine per formation.
        if sim_tick % 9 == 0 {
            let mode = match (self.next() & 0x3) as u8 {
                0 => FireDoctrineMode::Volley,
                1 => FireDoctrineMode::ByRank,
                2 => FireDoctrineMode::Rolling,
                _ => FireDoctrineMode::AdvanceAndFire,
            };
            let cadence = (2 + (self.next() % 5)) as u16;
            let doctrine_payload = pack_fire_doctrine_payload(mode, cadence);
            let _ = orders.push_order(
                OrderKind::SetFireDoctrine,
                formations[formation_idx].snapshot().formation_id,
                doctrine_payload,
                0,
            );
        }

        // Diplomacy/operational AI stub: deterministic nudge to reposition artillery/charge focus.
        let target_seed = self.next();
        let target_idx = (target_seed as usize) % formations.len();
        let target = formations[target_idx].snapshot();
        let dx = ((sim_tick as i64 % 3) * 1_000) as i32;
        let dz = (((target_seed >> 4) as i64 & 0x3) * 1_000) as i32;
        let offset = |v: u32, delta: i32| -> u32 {
            if delta >= 0 {
                v.saturating_add(delta as u32)
            } else {
                v.saturating_sub(delta.unsigned_abs())
            }
        };
        let target_x = offset(target.position_x_q16, dx);
        let target_z = offset(target.position_z_q16, dz);
        let payload_a = pack_move_payload(target_x, target_z);
        let payload_b = pack_charge_meta(1, false);

        // Logistics-aware tweak: if supply is low or courier ETA is high, bias to retreat.
        let low_supply = supply.avg_pressure_q16 < 20_000 || supply.baggage_captured;
        let slow_courier = courier_eta_hint.map(|eta| eta > 24).unwrap_or(false);
        if low_supply || slow_courier {
            // Pull back slightly instead of charging forward.
            let back = |v: u32| v.saturating_sub(2_000);
            let fallback =
                pack_move_payload(back(target.position_x_q16), back(target.position_z_q16));
            let _ = orders.push_order(OrderKind::FallBack, target.formation_id, fallback, 0);
        } else {
            let _ = orders.push_order(OrderKind::Charge, target.formation_id, payload_a, payload_b);
        }
    }
}

#[derive(Debug, Clone)]
struct WeatherKeyframe {
    tick: u64,
    precipitation_q16: u32,
    wind_speed_q16: u32,
    gust_q16: u32,
}

fn load_supply_graph(path: &Path) -> io::Result<Vec<SupplyRoad>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut roads = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let raw = line?;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: expected 5 fields", idx + 1),
            ));
        }
        let parse_u32 = |s: &str| -> io::Result<u32> {
            s.parse::<u32>().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: invalid number '{}'", idx + 1, s),
                )
            })
        };
        let from = parse_u32(parts[0])?;
        let to = parse_u32(parts[1])?;
        let capacity_q16 = parse_u32(parts[2])?;
        let loss_q16 = parse_u32(parts[3])?;
        let distance_tiles = parse_u32(parts[4])?;
        roads.push(SupplyRoad {
            from,
            to,
            capacity_q16,
            loss_q16,
            distance_tiles,
        });
    }
    Ok(roads)
}

fn load_weather_script(path: &Path) -> io::Result<Vec<WeatherKeyframe>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut frames = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let raw = line?;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "line {}: expected 4 fields (tick precip wind gust)",
                    idx + 1
                ),
            ));
        }
        let parse_u64 = |s: &str| -> io::Result<u64> {
            s.parse::<u64>().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: invalid number '{}'", idx + 1, s),
                )
            })
        };
        let parse_u32 = |s: &str| -> io::Result<u32> {
            s.parse::<u32>().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: invalid number '{}'", idx + 1, s),
                )
            })
        };
        frames.push(WeatherKeyframe {
            tick: parse_u64(parts[0])?,
            precipitation_q16: parse_u32(parts[1])?,
            wind_speed_q16: parse_u32(parts[2])?,
            gust_q16: parse_u32(parts[3])?,
        });
    }
    frames.sort_by_key(|f| f.tick);
    Ok(frames)
}

/// Campaign feeds: deterministic supply + weather derived from formation physics (replaces placeholders).
struct CampaignFeeds {
    supply: SupplyCapsule,
    weather: WeatherCapsule,
    weather_script: Option<Vec<WeatherKeyframe>>,
    weather_cursor: usize,
}

impl CampaignFeeds {
    fn new(
        formations: &[FormationCapsule],
        roads: Option<&[SupplyRoad]>,
        weather_script: Option<Vec<WeatherKeyframe>>,
    ) -> Self {
        let mut supply = SupplyCapsule::new(formations.len());
        if let Some(edges) = roads {
            for edge in edges {
                supply.add_road(
                    edge.from,
                    edge.to,
                    edge.capacity_q16,
                    edge.loss_q16,
                    edge.distance_tiles,
                );
            }
        } else {
            // Bidirectional roads between adjacent formations to simulate a simple front-line mesh.
            for idx in 0..formations.len() {
                let next = (idx + 1) % formations.len();
                if idx != next {
                    supply.add_road(idx as u32, next as u32, 60_000, 1_200, 4);
                    supply.add_road(next as u32, idx as u32, 60_000, 1_200, 4);
                }
            }
        }
        for idx in 0..formations.len() {
            let snap = formations[idx].snapshot();
            supply.set_ammo(idx as u32, snap.ammo);
        }
        let weather = WeatherCapsule::new();
        weather.set_precipitation(18_000);
        weather.set_wind(12 << 16, 10_000);
        Self {
            supply,
            weather,
            weather_script,
            weather_cursor: 0,
        }
    }

    fn step(&mut self, sim_tick: u64, formations: &[FormationCapsule]) -> SupplySnapshot {
        if let Some(script) = self.weather_script.as_ref() {
            if let Some(last) = script.last() {
                while self.weather_cursor + 1 < script.len()
                    && script[self.weather_cursor + 1].tick <= sim_tick
                {
                    self.weather_cursor += 1;
                }
                let key = &script[self.weather_cursor.min(script.len() - 1)];
                self.weather
                    .set_precipitation(key.precipitation_q16.min(65_536));
                self.weather
                    .set_wind(key.wind_speed_q16.min(65_536), key.gust_q16.min(65_536));
                if sim_tick > last.tick {
                    self.weather_cursor = script.len() - 1;
                }
            }
        } else {
            // Weather script: alternating dry/cloudburst/overcast phases tied to the campaign clock.
            let phase = (sim_tick % 12) as u32;
            let precip = match phase {
                0..=3 => 16_000,
                4..=7 => 36_000,
                _ => 22_000,
            };
            let wind = (12_000 + phase.saturating_mul(1_200)).min(45_000);
            self.weather.set_precipitation(precip);
            self.weather.set_wind(wind, wind / 2);
        }
        let wx = self.weather.step();
        self.supply.set_weather(wx.mud_q16, wx.wind_speed_q16);
        let decay_penalty = 65_536u32.saturating_sub(self.weather.supply_decay_scale_q16());
        let decay_q16 = 2_048u32.saturating_add(decay_penalty / 4).min(30_000);
        self.supply.set_decay_q16(decay_q16);

        // Demand/pressure derived from formation physics (density/mass) and current fatigue.
        for (idx, formation) in formations.iter().enumerate() {
            let snap: kindly_engine::formation::FormationSnapshot = formation.snapshot();
            let density_bias = snap.density_q16 / 6;
            let mass_bias = snap.mass_q16 / 8;
            let fatigue_drag = snap.fatigue_q16 / 12;
            let base = 32_000u32
                .saturating_add(density_bias)
                .saturating_add(mass_bias)
                .saturating_sub(fatigue_drag);
            self.supply.inject_pressure(idx as u32, base.min(65_536));
        }

        self.supply.step()
    }
}

fn main() {
    let cfg = DriverArgs::parse();
    cfg.ensure_dirs();
    let supply_roads = match cfg.supply_graph.as_ref() {
        Some(path) => Some(load_supply_graph(path).unwrap_or_else(|err| {
            eprintln!("failed to parse supply graph {:?}: {err}", path);
            std::process::exit(2);
        })),
        None => None,
    };
    if let (Some(path), Some(edges)) = (&cfg.supply_graph, supply_roads.as_ref()) {
        let edge_count: usize = edges.len();
        println!("loaded supply graph {:?} ({} edges)", path, edge_count);
    }
    let weather_script = match cfg.weather_script.as_ref() {
        Some(path) => Some(load_weather_script(path).unwrap_or_else(|err| {
            eprintln!("failed to parse weather script {:?}: {err}", path);
            std::process::exit(2);
        })),
        None => None,
    };
    if let (Some(path), Some(script)) = (&cfg.weather_script, weather_script.as_ref()) {
        println!(
            "loaded weather script {:?} ({} keyframes)",
            path,
            script.len()
        );
    }

    // Core capsules
    let telemetry = TelemetryCapsule::new();
    let orders = OrderQueueCapsule::new();
    let courier = CourierCapsule::new(Doctrine::defensive(), 6);
    let fire_doctrine = FireDoctrineCapsule::new();
    let ballistics = BallisticsCapsule::new(
        (400.0 * 65_536.0) as u32,
        (6.0 * 65_536.0) as u32,
        0,
        0,
        1,
        0,
    );
    let fire_profile = FireControlProfileCapsule::default();
    let terrain = TerrainGridCapsule::new(
        8,
        8,
        TerrainSnapshot {
            height_mm: 0,
            slope_q16: 0,
            cover_q16: 2_000,
            mud_q16: 1_000,
            material: 0,
        },
    );

    // Formations/pathing
    let formations = vec![
        FormationCapsule::new_with_preset(
            0,
            0,
            0,
            40_000,
            8_000,
            50_000,
            120,
            0,
            0,
            0,
            PhysicsPreset::LineInfantry,
        ),
        FormationCapsule::new_with_preset(
            1,
            0,
            0,
            45_000,
            7_000,
            52_000,
            120,
            Q16_16::from_f64(50.0).to_raw() as u32,
            0,
            0,
            PhysicsPreset::OldGuard,
        ),
    ];
    let pathings = vec![PathingCapsule::new(16, 0, 8), PathingCapsule::new(16, 0, 8)];
    seed_fire_doctrine_presets(
        &fire_doctrine,
        &formations,
        cfg.doctrine_defaults.as_deref(),
    );
    let mut campaign =
        CampaignFeeds::new(&formations, supply_roads.as_deref(), weather_script.clone());

    // Overlay ingestion (no GPU side-effects; just demonstrates overlay publication).
    let overlay_capsule = RenderOverlayCapsule::new();
    let kgpu = KgpuTerminalCapsule::new();
    let supply_heatmap_sink = TerminalHeatmapSink::new();
    let doctrine_sink = TerminalDoctrineSink::new();
    let heatmap_legend = SupplyHeatmapLegend::default();
    let mut heatmap_legend_printed = false;
    #[allow(unused_mut)]
    let mut kgpu_sink: Option<KgpuRenderSinkCapsule> = None;
    let mut kgpu_ingest = KgpuIngestCapsule::new(2_048);
    #[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
    {
        if cfg.use_kgpu_driver {
            kgpu_sink = match KgpuRenderSinkCapsule::new(
                cfg.kgpu_device_index,
                heatmap_legend,
                cfg.heatmap_enabled,
                8 * 1024,
            ) {
                Ok(sink) => Some(sink),
                Err(err) => {
                    eprintln!("kgpu-driver init failed: {err:?}");
                    None
                }
            };
        }
    }

    // Artillery payload
    let payload_a = pack_fire_payload(Q16_16::from_f64(50.0).to_raw() as u32, 0);
    let payload_b = pack_fire_meta_extended(12, 0, 0, false);

    // Strategic clock on user-configured interval
    let mut strat_clock = StrategicClock::new(cfg.strat_interval);
    let mut strategic = StrategicHooks::new(cfg.strat_seed);
    let mut slab = RenderSoaSlabCapsule::new(1, 2);

    for sim_tick in 0..10 {
        // Campaign-driven supply/weather feeds (no placeholders).
        let supply_snap = campaign.step(sim_tick, &formations);

        // If target ID is known, resolve deterministic fire-control with shooter/target snapshots.
        if let Some(order) = orders.pop_order() {
            let shooter_idx = order.formation_id as usize;
            if shooter_idx < formations.len() {
                let target_id = match order.kind {
                    OrderKind::ArtilleryFire => {
                        let target_q16 = unpack_fire_payload(order.payload_a);
                        nearest_target_id(&formations, target_q16, shooter_idx)
                    }
                    _ => None,
                };
                let _ = apply_fire_control_for_ids(
                    &order,
                    &terrain,
                    &ballistics,
                    &telemetry,
                    &fire_profile,
                    &formations,
                    shooter_idx,
                    target_id,
                );
            }

            // Requeue for the formation tick path.
            orders
                .push_order(
                    order.kind,
                    order.formation_id,
                    order.payload_a,
                    order.payload_b,
                )
                .expect("queue push");
        }

        if strat_clock.should_fire(sim_tick) {
            let courier_eta_hint = Some(courier.debug_snapshot().base_eta_ticks);
            strategic.apply(
                sim_tick,
                &orders,
                &formations,
                &supply_snap,
                courier_eta_hint,
            );
            orders
                .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
                .expect("queue push");
            strat_clock.advance();
        }

        let shard = make_shard_context(
            0,
            &orders,
            &formations,
            &pathings,
            &telemetry,
            None,
            Some(&ballistics),
            Some(&fire_profile),
            Some(&terrain),
            Some(&supply_snap),
            Some(&courier),
            None,
        );
        let stats = tick_world::<16>(&[shard]);

        // Produce a render view and publish overlays for kgpu.
        let shard_views = [&formations[..]];
        let view = collect_world_render_slab(&shard_views, &mut slab).expect("render slab");
        let overlay_snap = kgpu.ingest_overlays(&overlay_capsule, &view, sim_tick);
        let render_snap = kgpu.ingest_with_clock(&view, sim_tick);
        publish_render_snapshot(
            &render_snap,
            kgpu_sink.as_mut(),
            cfg.heatmap_enabled,
            &supply_heatmap_sink,
            &heatmap_legend,
            &mut heatmap_legend_printed,
            &mut kgpu_ingest,
            &doctrine_sink,
        );
        println!(
            "sim_tick {sim_tick}: orders {}, overlay_version {}",
            stats[0].processed_orders, overlay_snap.version
        );
    }

    // Optional: run one io_uring + persistence + kgpu overlay step when compiled with io-uring.
    #[cfg(feature = "io-uring")]
    {
        run_stream_with_io_uring(
            &formations,
            &pathings,
            &orders,
            &telemetry,
            &ballistics,
            &fire_profile,
            &terrain,
            &cfg,
            &overlay_capsule,
            &kgpu,
            kgpu_sink.as_mut(),
            supply_roads.as_deref(),
            weather_script.clone(),
            cfg.heatmap_enabled,
            &supply_heatmap_sink,
            &heatmap_legend,
            &mut heatmap_legend_printed,
            &mut kgpu_ingest,
            &doctrine_sink,
        );
    }
}

#[cfg(feature = "io-uring")]
fn run_stream_with_io_uring(
    formations: &[FormationCapsule],
    pathings: &[PathingCapsule],
    orders: &OrderQueueCapsule,
    telemetry: &TelemetryCapsule,
    ballistics: &BallisticsCapsule,
    fire_profile: &FireControlProfileCapsule,
    terrain: &TerrainGridCapsule,
    cfg: &DriverArgs,
    overlay_capsule: &RenderOverlayCapsule,
    kgpu: &KgpuTerminalCapsule,
    kgpu_sink: Option<&mut KgpuRenderSinkCapsule>,
    supply_roads: Option<&[SupplyRoad]>,
    weather_script: Option<Vec<WeatherKeyframe>>,
    heatmap_enabled: bool,
    heatmap_sink: &TerminalHeatmapSink,
    heatmap_legend: &SupplyHeatmapLegend,
    heatmap_legend_printed: &mut bool,
    kgpu_ingest: &mut KgpuIngestCapsule,
    doctrine_sink: &TerminalDoctrineSink,
) {
    use kindly_engine::driver::DriverCapsule;
    use kindly_engine::io_bridge::RenderUringSinkCapsule;
    use kindly_engine::pathing::PathingCapsule as _;
    use kindly_engine::replay::{
        ReplayFlushCapsule, ReplayIndexCapsule, ReplayLogCapsule, ReplayMmapCapsule,
    };
    use kindly_engine::snapshot::{CampaignSnapshotCapsule, SnapshotMmapCapsule};
    use kindly_engine::tick::WorldLoopCapsule;
    use kindly_engine::WorldRuntimeCapsule;
    use std::fs::OpenOptions;
    use std::os::fd::AsRawFd;

    let loop_capsule = WorldLoopCapsule::new(0, 16_666_667, 0x1234_5678, 1);
    let runtime = WorldRuntimeCapsule::new(loop_capsule, 2, formations.len());

    // io_uring sink to real render target.
    let fd = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&cfg.render_out)
        .expect("open render_out");
    let render_sink =
        RenderUringSinkCapsule::new(8, 0, fd.as_raw_fd(), 1_048_576).expect("uring sink");

    // Replay/snapshot persistence to configured files.
    let tmp_replay = cfg.replay_out.clone();
    let tmp_snapshot = cfg.snapshot_out.clone();
    let tmp_index = cfg.replay_index.clone();

    let replay_log: ReplayLogCapsule<1024> = ReplayLogCapsule::new();
    let replay_flush = ReplayFlushCapsule::new();
    let replay_mmap = ReplayMmapCapsule::new(&tmp_replay, 1_048_576, 1).expect("replay mmap");
    let replay_index = tmp_index
        .as_ref()
        .and_then(|path| ReplayIndexCapsule::new(path, 1_048_576, 1).ok());

    let snapshot_capsule = CampaignSnapshotCapsule::new();
    let snapshot_mmap =
        SnapshotMmapCapsule::new(&tmp_snapshot, 1_048_576, 1).expect("snapshot mmap");
    let mut campaign = CampaignFeeds::new(formations, supply_roads, weather_script);
    let supply_snap = campaign.step(0, formations);

    let mut driver = DriverCapsule::new(
        runtime,
        render_sink,
        &overlay_capsule,
        &kgpu,
        &replay_log,
        &replay_flush,
        &replay_mmap,
        replay_index.as_ref(),
        &snapshot_capsule,
        &snapshot_mmap,
        formations,
        orders,
        telemetry,
        Some(ballistics),
        Some(fire_profile),
        Some(terrain),
        Some(&courier),
        Some(&fire_doctrine),
    );

    let shard = driver.make_shard_context(0, formations, pathings, None, Some(&supply_snap));
    let frame = driver.step(&[shard]).expect("driver step");
    publish_render_snapshot(
        &frame.render,
        kgpu_sink,
        heatmap_enabled,
        heatmap_sink,
        heatmap_legend,
        heatmap_legend_printed,
        kgpu_ingest,
        &doctrine_sink,
    );
    println!(
        "io_uring stream tick {} overlay_version {} snapshot_chain {} render_frame_id {}",
        frame.tick, frame.overlay.version, frame.snapshot_chain, frame.render.frame_id
    );
}

fn seed_fire_doctrine_presets(
    doctrine: &FireDoctrineCapsule,
    formations: &[FormationCapsule],
    overrides: Option<&str>,
) {
    let mut volley = 3u16;
    let mut byrank = 2u16;
    let mut rolling = 3u16;
    let mut advance = 3u16;
    if let Some(raw) = overrides {
        for part in raw.split(',') {
            let mut kv = part.split(':');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                if let Ok(val) = v.parse::<u16>() {
                    match k.to_ascii_lowercase().as_str() {
                        "volley" => volley = val,
                        "byrank" => byrank = val,
                        "rolling" => rolling = val,
                        "advance" => advance = val,
                        _ => {}
                    }
                }
            }
        }
    }
    for (idx, formation) in formations.iter().enumerate() {
        let mode = match idx % 4 {
            0 => FireDoctrineMode::Volley,
            1 => FireDoctrineMode::ByRank,
            2 => FireDoctrineMode::Rolling,
            _ => FireDoctrineMode::AdvanceAndFire,
        };
        let cadence = match mode {
            FireDoctrineMode::Volley => volley,
            FireDoctrineMode::ByRank => byrank,
            FireDoctrineMode::Rolling => rolling,
            FireDoctrineMode::AdvanceAndFire => advance,
            FireDoctrineMode::Disabled => 0,
        };
        doctrine.set_doctrine(formation.snapshot().formation_id, mode, cadence);
    }
}

fn publish_render_snapshot(
    snapshot: &kindly_engine::kgpu_bridge::RenderSnapshot<'_>,
    kgpu_sink: Option<&mut KgpuRenderSinkCapsule>,
    heatmap_enabled: bool,
    heatmap_sink: &TerminalHeatmapSink,
    legend: &SupplyHeatmapLegend,
    legend_printed: &mut bool,
    kgpu_ingest: &mut KgpuIngestCapsule,
    doctrine_sink: &TerminalDoctrineSink,
) {
    // Hook for the real kgpu renderer: snapshot contains the zero-copy SoA view + overlays.
    let _ = snapshot.view.total_len;
    let _ = snapshot.frame_id;
    // Encode into staging buffer (deterministic, reused across ticks).
    let _encoded = kgpu_ingest.encode(snapshot);
    if let Some(sink) = kgpu_sink {
        // kgpu sink submits both render and heatmap (if enabled).
        let _ = sink.submit(snapshot);
        return;
    }

    // Supply heatmap publication (terminal path) + best-effort kgpu handshake.
    if heatmap_enabled {
        if !*legend_printed {
            println!(
                "heatmap legend: {} -> green, {} -> red (blue = morale/LOD)",
                legend.supply_label, legend.fatigue_label
            );
            *legend_printed = true;
        }
        let supply_frame = make_supply_heatmap_from_render(snapshot);
        heatmap_sink.submit(&supply_frame);
    }
    // Doctrine/rank-fire overlay publication (terminal path).
    let doctrine_frame = make_doctrine_overlay_from_render(snapshot);
    doctrine_sink.submit(&doctrine_frame);
    let _ = try_submit_with_kgpu_driver(snapshot);
}
