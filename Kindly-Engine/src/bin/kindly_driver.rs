//! Demonstration driver: fixed-step sim tick + optional coarse strategic cadence,
//! wiring shard contexts with terrain/ballistics/fire profiles, artillery target IDs,
//! kgpu overlays, and optional io_uring persistence when compiled with `--features io-uring`.

use kindly_engine::ballistics::{
    apply_fire_control_for_ids, BallisticsCapsule, FireControlProfileCapsule,
};
use kindly_engine::courier::{CourierCapsule, Doctrine};
use kindly_engine::fire_doctrine::{FireDoctrineCapsule, FireDoctrineMode};
use kindly_engine::formation::FormationCapsule;
use kindly_engine::command::{commanders_to_generals, CommanderCapsule, CommandHierarchyCapsule};
use kindly_engine::general::{snapshot_generals, GeneralCapsule};
use kindly_engine::grenade::GrenadeCapsule;
use kindly_engine::kgpu_bridge::{
    make_aperture_overlay_from_render, make_doctrine_overlay_from_render,
    make_supply_heatmap_from_render, try_submit_with_kgpu_driver, KgpuTerminalCapsule,
    RenderOverlayCapsule, SupplyHeatmapLegend, SupplyHeatmapSink, TerminalDoctrineSink,
    TerminalHeatmapSink,
};
use kindly_engine::kgpu_ingest::{KgpuIngestCapsule, KgpuRenderSinkCapsule};
use kindly_engine::math::Q16_16;
use kindly_engine::order::{
    pack_charge_meta, pack_fire_doctrine_payload, pack_fire_meta_extended, pack_fire_payload,
    pack_grenade_meta, pack_grenade_payload, pack_move_payload, pack_posture_payload,
    unpack_fire_payload, CommandDelayBufferCapsule, OrderKind, OrderQueueCapsule,
};
use kindly_engine::pathing::PathingCapsule;
use kindly_engine::physics::PhysicsPreset;
use kindly_engine::strategic_map::{ProvinceCapsule, StrategicMapCapsule, WeatherKeyframe};
use kindly_engine::supply::{SupplyRoad, SupplySnapshot};
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
    grenade_fuse_ms: u16,
    grenade_fragments: u16,
    replay_summary: bool,
    replay_summary_out: Option<PathBuf>,
}

impl DriverArgs {
    fn parse() -> Self {
        let mut args = env::args().skip(1);
        let mut cfg = Self {
            render_out: PathBuf::from("data/kindly-engine/render_stream.bin"),
            replay_out: PathBuf::from("data/kindly-engine/replay.bin"),
            replay_summary: false,
            replay_summary_out: None,
            replay_index: Some(PathBuf::from("data/kindly-engine/replay.idx")),
            snapshot_out: PathBuf::from("data/kindly-engine/snapshot.bin"),
            strat_interval: 5,
            strat_seed: 0xC0DA_C0DA,
            use_kgpu_driver: false,
            kgpu_device_index: 0,
            heatmap_enabled: true,
            supply_graph: None,
            weather_script: None,
            doctrine_defaults: None,
            grenade_fuse_ms: 1200,
            grenade_fragments: 48,
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
                "--replay-summary" => {
                    cfg.replay_summary = true;
                }
                "--replay-summary-out" => {
                    cfg.replay_summary_out = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                        eprintln!("--replay-summary-out requires a path");
                        std::process::exit(2);
                    })));
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
                "--grenade-fuse" => {
                    cfg.grenade_fuse_ms = args
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(cfg.grenade_fuse_ms);
                }
                "--grenade-fragments" => {
                    cfg.grenade_fragments = args
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(cfg.grenade_fragments);
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
        if let Some(summary_out) = &self.replay_summary_out {
            paths.push(summary_out.clone());
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
         --replay-summary         Decode replay after run (StratOps summary)\n\
         --replay-summary-out <path> Write StratOps summary JSON to path\n\
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
        --doctrine-defaults <list> Comma list (volley:<ticks>,byrank:<ticks>,rolling:<ticks>,advance:<ticks>)\n\
        --grenade-fuse <ms>      Default grenade fuse (ms, default 1200)\n\
        --grenade-fragments <n>  Default grenade fragment count (default 48)"
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
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "line {}: expected at least 4 fields (tick precip wind gust [wind_dir])",
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
        let wind_dir = if parts.len() > 4 {
            parse_u32(parts[4])?
        } else {
            0
        };
        frames.push(WeatherKeyframe {
            tick: parse_u64(parts[0])?,
            precipitation_q16: parse_u32(parts[1])?,
            wind_speed_q16: parse_u32(parts[2])?,
            gust_q16: parse_u32(parts[3])?,
            wind_dir_deg_q16: wind_dir,
        });
    }
    frames.sort_by_key(|f| f.tick);
    Ok(frames)
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
    } else {
        println!("supply graph not provided: using deterministic fallback mesh between formations");
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
    } else {
        println!(
            "weather script not provided: using deterministic fallback precipitation/wind phases"
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
    let grenades = GrenadeCapsule::new(
        30 << 16,
        cfg.grenade_fuse_ms as u32,
        cfg.grenade_fragments as u32,
        50_000,
        2 << 16,
        cfg.strat_seed,
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
        FormationCapsule::spawn_line(0, 0, 0, 40_000, 8_000, 50_000, 120, 0, 0, 0),
        FormationCapsule::spawn_guard(
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
        ),
        FormationCapsule::spawn_grenadier(
            2,
            0,
            0,
            42_000,
            7_000,
            50_000,
            96,
            Q16_16::from_f64(-25.0).to_raw() as u32,
            10 << 16,
            0,
        ),
    ];
    let pathings = vec![
        PathingCapsule::new(16, 0, 8),
        PathingCapsule::new(16, 0, 8),
        PathingCapsule::new(16, 0, 8),
    ];
    // Commanders (single army commander following formation 0 for demo).
    let mut commanders = vec![CommanderCapsule::new(
        formations[0].snapshot().position_x_q16,
        formations[0].snapshot().position_z_q16,
        20_000,
        2_000,
        1_000,
        1_000,
        2,
        true,
    )];
    seed_fire_doctrine_presets(
        &fire_doctrine,
        &formations,
        cfg.doctrine_defaults.as_deref(),
    );
    let command_delays = CommandDelayBufferCapsule::new();
    let command_hierarchy = CommandHierarchyCapsule::new(formations.len());
    for (idx, _f) in formations.iter().enumerate() {
        command_hierarchy.assign_commander(idx, 0);
    }
    let mut provinces: Vec<ProvinceCapsule> = formations
        .iter()
        .map(|f| {
            let snap = f.snapshot();
            let p = ProvinceCapsule::new(0, 100_000, 40_000);
            p.set_depot_pressure(snap.density_q16.min(60_000));
            p
        })
        .collect();
    let mut strategic_map =
        StrategicMapCapsule::new(provinces, supply_roads.as_deref(), weather_script.clone());
    for (idx, formation) in formations.iter().enumerate() {
        strategic_map.set_ammo(idx as u32, formation.snapshot().ammo);
    }

    // Overlay ingestion (no GPU side-effects; just demonstrates overlay publication).
    let overlay_capsule = RenderOverlayCapsule::new();
    let kgpu = KgpuTerminalCapsule::new();
    let supply_heatmap_sink = TerminalHeatmapSink::new();
    let doctrine_sink = TerminalDoctrineSink::new();
    let heatmap_legend = SupplyHeatmapLegend::default();
    let mut heatmap_legend_printed = false;
    let mut aperture_legend_printed = false;
    #[allow(unused_mut)]
    let mut kgpu_sink: Option<KgpuRenderSinkCapsule> = None;
    let mut kgpu_ingest = KgpuIngestCapsule::new(2_048);
    let command_delays = CommandDelayBufferCapsule::new();
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
        let strat_snap = strategic_map.step(sim_tick);
        let supply_snap = &strat_snap.supply;

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
                    None,
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
                supply_snap,
                courier_eta_hint,
            );
            orders
                .push_order(OrderKind::ArtilleryFire, 0, payload_a, payload_b)
                .expect("queue push");
            if formations.len() > 1 {
                let grenadier_idx = formations.len() - 1;
                let shooter = formations[grenadier_idx].snapshot();
                let target = select_grenade_target(&formations, grenadier_idx, &terrain)
                    .unwrap_or_else(|| formations[1].snapshot());
                let grenade_payload =
                    pack_grenade_payload(target.position_x_q16, target.position_z_q16);
                let grenade_meta = pack_grenade_meta(cfg.grenade_fuse_ms, cfg.grenade_fragments);
                orders
                    .push_order(
                        OrderKind::Grenade,
                        shooter.formation_id,
                        grenade_payload,
                        grenade_meta,
                    )
                    .expect("queue push");
            }
            strat_clock.advance();
        }

        if let (Some(cmd), Some(first)) = (commanders.first(), formations.first()) {
            let pos = first.snapshot();
            cmd.set_position(pos.position_x_q16, pos.position_z_q16);
        }
        let commander_snaps: Vec<_> = commanders.iter().map(CommanderCapsule::snapshot).collect();
        let general_snaps = commanders_to_generals(&commander_snaps);

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
            Some(&grenades),
            None,
            None,
            Some(supply_snap),
            Some(&courier),
            Some(&fire_doctrine),
            None,
            None,
            Some(&general_snaps),
            Some(&command_hierarchy),
            Some(&commander_snaps),
            Some(&strat_snap),
            Some(&command_delays),
        );
        let stats = tick_world::<16>(sim_tick, &[shard]);

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
            &mut aperture_legend_printed,
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
            &grenades,
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
    grenades: &GrenadeCapsule,
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
    aperture_legend_printed: &mut bool,
    kgpu_ingest: &mut KgpuIngestCapsule,
    doctrine_sink: &TerminalDoctrineSink,
) {
    use kindly_engine::driver::DriverCapsule;
    use kindly_engine::grenade::GrenadeCapsule;
    use kindly_engine::io_bridge::RenderUringSinkCapsule;
    use kindly_engine::pathing::PathingCapsule as _;
    use kindly_engine::replay::{
        build_stratops_lane, decode_replay_payload, ReplayFlushCapsule, ReplayIndexCapsule,
        ReplayLogCapsule, ReplayMmapCapsule,
    };
    use kindly_engine::snapshot::{CampaignSnapshotCapsule, SnapshotMmapCapsule};
    use kindly_engine::structure::StructureCapsule;
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
    let mut replay_mmap = ReplayMmapCapsule::new(&tmp_replay, 1_048_576, 1).expect("replay mmap");
    let replay_index = tmp_index
        .as_ref()
        .and_then(|path| ReplayIndexCapsule::new(path, 1_048_576, 1).ok());

    let snapshot_capsule = CampaignSnapshotCapsule::new();
    let mut snapshot_mmap =
        SnapshotMmapCapsule::new(&tmp_snapshot, 1_048_576, 1).expect("snapshot mmap");
    let provinces: Vec<ProvinceCapsule> = formations
        .iter()
        .map(|f| {
            let snap = f.snapshot();
            let p = ProvinceCapsule::new(0, 100_000, 40_000);
            p.set_depot_pressure(snap.density_q16.min(60_000));
            p
        })
        .collect();
    let mut campaign = StrategicMapCapsule::new(provinces, supply_roads, weather_script);
    for (idx, formation) in formations.iter().enumerate() {
        campaign.set_ammo(idx as u32, formation.snapshot().ammo);
    }
    let strat_snap = campaign.step(0);
    let supply_snap = &strat_snap.supply;

    let structures: Vec<StructureCapsule> = Vec::new();

    let mut driver = DriverCapsule::new(
        runtime,
        render_sink,
        &overlay_capsule,
        &kgpu,
        &replay_log,
        &replay_flush,
        &mut replay_mmap,
        replay_index.as_ref(),
        &snapshot_capsule,
        &mut snapshot_mmap,
        formations,
        &structures,
        None,
        orders,
        telemetry,
        Some(ballistics),
        Some(fire_profile),
        Some(terrain),
        Some(grenades),
        None,
        Some(&courier),
        Some(&fire_doctrine),
    );

    let shard = driver.make_shard_context(
        0,
        formations,
        pathings,
        None,
        Some(supply_snap),
        Some(&strat_snap),
        None,
        None,
        None,
        Some(&command_delays),
    );
    let frame = driver
        .step(
            &[shard],
            Some(&strat_snap),
            None,
            None,
            Some(&command_delays),
            kgpu_sink.as_deref_mut(),
        )
        .expect("driver step");
    publish_render_snapshot(
        &frame.render,
        None,
        heatmap_enabled,
        heatmap_sink,
        heatmap_legend,
        heatmap_legend_printed,
        &mut false, // aperture legend only printed in main driver loop
        kgpu_ingest,
        &doctrine_sink,
    );
    println!(
        "io_uring stream tick {} overlay_version {} snapshot_chain {} render_frame_id {}",
        frame.tick, frame.overlay.version, frame.snapshot_chain, frame.render.frame_id
    );

    if cfg.replay_summary || cfg.replay_summary_out.is_some() {
        match std::fs::read(&cfg.replay_out) {
            Ok(bytes) => {
                let mut decoded = Vec::new();
                for chunk in bytes.chunks(16) {
                    if chunk.len() < 16 {
                        break;
                    }
                    let tick = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
                    let payload = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
                    decoded.push((tick, decode_replay_payload(payload)));
                }
                let lane = build_stratops_lane(&decoded);
                if cfg.replay_summary {
                    if lane.is_empty() {
                        println!("replay summary: no StratOps records found");
                    } else {
                        let mut strat = 0;
                        let mut delay_applied = 0;
                        let mut delay_hist = 0;
                        let mut eta_hist = 0;
                        for rec in &lane {
                            match rec {
                                kindly_engine::replay::StratOpsRecord::Strategic { .. } => strat += 1,
                                kindly_engine::replay::StratOpsRecord::CommandDelayApplied { .. } => {
                                    delay_applied += 1
                                }
                                kindly_engine::replay::StratOpsRecord::CommandDelayHist { .. } => {
                                    delay_hist += 1
                                }
                                kindly_engine::replay::StratOpsRecord::CourierEtaHist { .. } => {
                                    eta_hist += 1
                                }
                            }
                        }
                        println!("=== StratOps replay summary ===");
                        println!("strategic events        : {strat}");
                        println!("cmd delay applied       : {delay_applied}");
                        println!("cmd delay hist chunks   : {delay_hist}");
                        println!("courier ETA hist chunks : {eta_hist}");
                        println!("recent (up to 10):");
                        for rec in lane.iter().rev().take(10).rev() {
                            println!("  {rec:?}");
                        }
                    }
                }
                if let Some(path) = &cfg.replay_summary_out {
                    if let Err(err) = write_stratops_json(&lane, path) {
                        eprintln!("failed to write replay summary json: {err}");
                    } else {
                        println!("wrote StratOps summary json to {:?}", path);
                    }
                }
            }
            Err(err) => eprintln!("failed to read replay for summary: {err}"),
        }
    }
}

fn write_stratops_json(
    lane: &[kindly_engine::replay::StratOpsRecord],
    path: &Path,
) -> std::io::Result<()> {
    use std::io::Write;
    let mut strat = 0u64;
    let mut delay_applied = 0u64;
    let mut delay_hist = 0u64;
    let mut eta_hist = 0u64;
    for rec in lane {
        match rec {
            kindly_engine::replay::StratOpsRecord::Strategic { .. } => strat += 1,
            kindly_engine::replay::StratOpsRecord::CommandDelayApplied { .. } => delay_applied += 1,
            kindly_engine::replay::StratOpsRecord::CommandDelayHist { .. } => delay_hist += 1,
            kindly_engine::replay::StratOpsRecord::CourierEtaHist { .. } => eta_hist += 1,
        }
    }
    let mut recent = String::new();
    recent.push('[');
    for (idx, rec) in lane.iter().rev().take(10).rev().enumerate() {
        if idx > 0 {
            recent.push(',');
        }
        let entry = match rec {
            kindly_engine::replay::StratOpsRecord::Strategic {
                tick,
                kind,
                province_id,
                primary,
                secondary,
            } => format!(
                "{{\"type\":\"strategic\",\"tick\":{},\"kind\":\"{:?}\",\"province_id\":{},\"primary\":{},\"secondary\":{}}}",
                tick, kind, province_id, primary, secondary
            ),
            kindly_engine::replay::StratOpsRecord::CommandDelayApplied {
                tick,
                count,
                avg_delay_ticks,
            } => format!(
                "{{\"type\":\"cmd_delay_applied\",\"tick\":{},\"count\":{},\"avg_delay_ticks\":{}}}",
                tick, count, avg_delay_ticks
            ),
            kindly_engine::replay::StratOpsRecord::CommandDelayHist {
                tick,
                chunk,
                buckets,
            } => format!(
                "{{\"type\":\"cmd_delay_hist\",\"tick\":{},\"chunk\":{},\"buckets\":[{},{},{},{}]}}",
                tick, chunk, buckets[0], buckets[1], buckets[2], buckets[3]
            ),
            kindly_engine::replay::StratOpsRecord::CourierEtaHist {
                tick,
                chunk,
                buckets,
            } => format!(
                "{{\"type\":\"courier_eta_hist\",\"tick\":{},\"chunk\":{},\"buckets\":[{},{},{},{}]}}",
                tick, chunk, buckets[0], buckets[1], buckets[2], buckets[3]
            ),
        };
        recent.push_str(&entry);
    }
    recent.push(']');

    let json = format!(
        "{{\"strategic\":{},\"cmd_delay_applied\":{},\"cmd_delay_hist\":{},\"courier_eta_hist\":{},\"recent\":{}}}",
        strat, delay_applied, delay_hist, eta_hist, recent
    );
    let mut file = std::fs::File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
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

/// Pick a grenade target: prefer the densest enemy formation with lowest cover toward the grenadier.
fn select_grenade_target(
    formations: &[FormationCapsule],
    grenadier_idx: usize,
    terrain: &TerrainGridCapsule,
) -> Option<FormationSnapshot> {
    let shooter = formations.get(grenadier_idx)?.snapshot();
    let shooter_tile = (shooter.position_x_q16 >> 16, shooter.position_z_q16 >> 16);
    formations
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != grenadier_idx)
        .map(|(_, f)| f.snapshot())
        .map(|snap| {
            let target_tile = (snap.position_x_q16 >> 16, snap.position_z_q16 >> 16);
            let cover_q16 = terrain.los_clear(shooter_tile, target_tile).1;
            (snap, cover_q16)
        })
        .max_by(|(a_snap, a_cover), (b_snap, b_cover)| {
            a_snap
                .density_q16
                .cmp(&b_snap.density_q16)
                .then_with(|| b_cover.cmp(a_cover))
        })
        .map(|(snap, _)| snap)
}

fn publish_render_snapshot(
    snapshot: &kindly_engine::kgpu_bridge::RenderSnapshot<'_>,
    kgpu_sink: Option<&mut KgpuRenderSinkCapsule>,
    heatmap_enabled: bool,
    heatmap_sink: &TerminalHeatmapSink,
    legend: &SupplyHeatmapLegend,
    legend_printed: &mut bool,
    aperture_legend_printed: &mut bool,
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
    if !*aperture_legend_printed {
        println!("aperture overlay: R=min aperture, G=max, B=avg (normalized Q16.16)");
        *aperture_legend_printed = true;
    }
    // Build aperture overlay frame for renderer/debug consumers (no-op terminal submit).
    let _aperture_frame = make_aperture_overlay_from_render(snapshot);
    let _ = try_submit_with_kgpu_driver(snapshot);
}
