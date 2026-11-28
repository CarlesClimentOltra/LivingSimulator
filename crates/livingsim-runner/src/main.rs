//! `livingsim-runner`: advances the shared world and writes the static files that GitHub
//! Pages serves (see docs/ARCHITECTURE.md §5.3 and §11).
//!
//! The runner simulates *ahead* of real time (`--lead-hours`), so a late or failed cron
//! run is invisible to visitors.

#![allow(clippy::disallowed_types)]

mod manifest;
mod store;

use clap::{Parser, Subcommand};
use livingsim_core::messages::MessageLog;
use livingsim_core::snapshot::{self, decode_region, encode_log, encode_region, region_hash};
use livingsim_core::time::REAL_MS_PER_TICK;
use livingsim_core::world::{Highway, IntercityGraph};
use livingsim_core::{catalog, Region, World, WINDOW};
use manifest::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use store::Source;

#[derive(Parser)]
#[command(
    name = "livingsim-runner",
    version,
    about = "Advances the Living Simulator world"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Advance the published world to now + lead and write the new files.
    Tick {
        /// Where the previous files live: the public Pages URL of `world/`, or a directory.
        #[arg(long)]
        base: Option<String>,
        /// Output directory (becomes `<site>/world`).
        #[arg(long)]
        out: PathBuf,
        /// How far ahead of real time to simulate, in real hours.
        #[arg(long, default_value_t = 6)]
        lead_hours: u32,
        /// Seed for a brand new world (only used when nothing was published yet).
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Epoch for a brand new world (Unix ms). Defaults to now.
        #[arg(long)]
        epoch_ms: Option<u64>,
        /// Pretend the current time is this (Unix ms). For tests.
        #[arg(long)]
        now_ms: Option<u64>,
        /// World snapshot (file or URL) to restore from if the published state is missing.
        #[arg(long)]
        restore: Option<String>,
        /// Also write the frontier as a full world snapshot here (for Release backups).
        #[arg(long)]
        backup: Option<PathBuf>,
        /// Safety limit on simulated days per run.
        #[arg(long, default_value_t = 400)]
        max_days: u32,
    },
    /// Print the state hash of a fresh world after N ticks (determinism checks).
    Hash {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 1440)]
        ticks: u32,
    },
    /// Re-simulate every region of a published directory alone from its checkpoints and
    /// message logs, and compare with the published hashes.
    Verify {
        #[arg(long)]
        dir: PathBuf,
    },
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn main() {
    let cli = Cli::parse();
    let res = match cli.cmd {
        Cmd::Tick {
            base,
            out,
            lead_hours,
            seed,
            epoch_ms,
            now_ms: now,
            restore,
            backup,
            max_days,
        } => tick(TickArgs {
            base,
            out,
            lead_hours,
            seed,
            epoch_ms,
            now: now.unwrap_or_else(now_ms),
            restore,
            backup,
            max_days,
        }),
        Cmd::Hash { seed, ticks } => {
            let mut w = World::new(seed);
            w.step_to(ticks);
            println!("{}", w.state_hash().to_hex());
            Ok(())
        }
        Cmd::Verify { dir } => verify(&dir),
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

struct TickArgs {
    base: Option<String>,
    out: PathBuf,
    lead_hours: u32,
    seed: u64,
    epoch_ms: Option<u64>,
    now: u64,
    restore: Option<String>,
    backup: Option<PathBuf>,
    max_days: u32,
}

fn write(out: &Path, rel: &str, bytes: &[u8]) -> Result<(), String> {
    let p = out.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))
}

fn tick_at(now: u64, epoch: u64) -> u32 {
    (now.saturating_sub(epoch) / REAL_MS_PER_TICK) as u32
}

fn floor_cp(t: u32) -> u32 {
    t / CHECKPOINT_TICKS * CHECKPOINT_TICKS
}

fn load_regions(src: &Source, ver: &str, cp: &Checkpoint) -> Result<Vec<Region>, String> {
    let mut regions = Vec::new();
    for r in &cp.regions {
        let bytes = src
            .get(&cp_path(ver, cp.tick, r.id))?
            .ok_or_else(|| format!("missing checkpoint {} r{}", cp.tick, r.id))?;
        regions.push(
            decode_region(&bytes).map_err(|e| format!("checkpoint {} r{}: {e}", cp.tick, r.id))?,
        );
    }
    Ok(regions)
}

fn intercity_of(m: &Manifest) -> IntercityGraph {
    IntercityGraph {
        highways: m
            .highways
            .iter()
            .map(|h| Highway {
                a: h.a,
                b: h.b,
                length_tiles: h.length_tiles,
            })
            .collect(),
    }
}

fn tick(args: TickArgs) -> Result<(), String> {
    let started = Instant::now();
    let src = Source::parse(args.base.as_deref());
    let sim_version = livingsim_core::sim_version().to_string();
    let ver = ver_of(&sim_version);
    let lead_ticks = (args.lead_hours as u64 * 3_600_000 / REAL_MS_PER_TICK) as u32;

    let old: Option<Manifest> = match src.get("manifest.json")? {
        Some(b) => Some(serde_json::from_slice(&b).map_err(|e| format!("manifest.json: {e}"))?),
        None => None,
    };

    // ---- Load the world to continue from.
    let mut rebased = false;
    let mut keep_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut checkpoints: Vec<Checkpoint> = Vec::new();
    let mut logs: Vec<LogInfo> = Vec::new();
    let (mut world, epoch, seed) = match &old {
        None => {
            let epoch = args.epoch_ms.unwrap_or(args.now);
            let start = floor_cp(tick_at(args.now, epoch));
            eprintln!(
                "no published world: generating a new one (seed {}, start tick {start})",
                args.seed
            );
            (World::generate(args.seed, start), epoch, args.seed)
        }
        Some(m) => {
            let now_tick = tick_at(args.now, m.epoch_ms);
            let same = m.sim_version == sim_version && m.format_version == snapshot::FORMAT_VERSION;
            let loaded = if same {
                // Continue from the frontier and keep the still-useful published files.
                let frontier = m
                    .checkpoints
                    .iter()
                    .find(|c| c.tick == m.frontier_tick)
                    .ok_or("frontier checkpoint missing from manifest")?;
                let keep_from = floor_cp(now_tick).saturating_sub(CHECKPOINT_TICKS);
                let regions = load_regions(&src, &m.ver, frontier);
                if regions.is_ok() {
                    for cp in m.checkpoints.iter().filter(|c| c.tick >= keep_from) {
                        for r in &cp.regions {
                            let p = cp_path(&m.ver, cp.tick, r.id);
                            if let Some(b) = src.get(&p)? {
                                keep_files.push((p, b));
                            }
                        }
                        checkpoints.push(cp.clone());
                    }
                    for l in m.logs.iter().filter(|l| l.to > keep_from) {
                        let p = log_path(&m.ver, l.region, l.from);
                        if let Some(b) = src.get(&p)? {
                            keep_files.push((p, b));
                            logs.push(l.clone());
                        }
                    }
                }
                regions
            } else {
                // New engine or content: rebase on the latest checkpoint <= now.
                rebased = true;
                let cp = m
                    .checkpoints
                    .iter()
                    .filter(|c| c.tick <= now_tick.max(m.checkpoints[0].tick))
                    .max_by_key(|c| c.tick)
                    .ok_or("no checkpoint to rebase on")?;
                eprintln!(
                    "sim version changed ({} -> {}): rebasing at tick {}",
                    m.ver, ver, cp.tick
                );
                match load_regions(&src, &m.ver, cp) {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        eprintln!(
                            "old checkpoint unusable ({e}); restarting the world at tick {}",
                            cp.tick
                        );
                        Ok(World::generate(m.seed, cp.tick).regions)
                    }
                }
            };
            match loaded {
                Ok(regions) => (
                    World::from_regions(m.seed, intercity_of(m), regions),
                    m.epoch_ms,
                    m.seed,
                ),
                Err(e) => {
                    eprintln!("published state unavailable: {e}");
                    let r = args
                        .restore
                        .as_deref()
                        .ok_or("no --restore snapshot given")?;
                    let bytes = store::read_any(r)?.ok_or("restore snapshot not found")?;
                    let w = World::deserialize(&bytes).map_err(|e| format!("restore: {e}"))?;
                    eprintln!("restored world at tick {}", w.tick);
                    checkpoints.clear();
                    logs.clear();
                    keep_files.clear();
                    (w, m.epoch_ms, m.seed)
                }
            }
        }
    };

    let now_tick = tick_at(args.now, epoch);
    let target = (now_tick + lead_ticks).div_ceil(CHECKPOINT_TICKS) * CHECKPOINT_TICKS;
    let mut new_files: Vec<(String, Vec<u8>)> = Vec::new();

    // The starting point is itself a checkpoint (needed after genesis/rebase/restore).
    if !checkpoints.iter().any(|c| c.tick == world.tick) {
        let cp = write_checkpoint(&world, &ver, &mut new_files);
        checkpoints.push(cp);
    }

    // ---- Simulate day by day.
    let start_tick = world.tick;
    let mut days = 0;
    while world.tick < target && days < args.max_days {
        let from = world.tick;
        let to = from + CHECKPOINT_TICKS;
        let mut day_logs: BTreeMap<u16, MessageLog> = world
            .regions
            .iter()
            .map(|r| {
                (
                    r.id,
                    MessageLog {
                        region: r.id,
                        from,
                        to,
                        deliveries: vec![],
                    },
                )
            })
            .collect();
        while world.tick < to {
            let t = world.tick;
            for (dest, msgs) in world.step_window() {
                // A town founded today has a log from today on.
                day_logs
                    .entry(dest)
                    .or_insert_with(|| MessageLog {
                        region: dest,
                        from,
                        to,
                        deliveries: vec![],
                    })
                    .deliveries
                    .push((t, msgs));
            }
            for r in &world.regions {
                day_logs.entry(r.id).or_insert_with(|| MessageLog {
                    region: r.id,
                    from,
                    to,
                    deliveries: vec![],
                });
            }
            debug_assert_eq!(world.tick, t + WINDOW);
        }
        for l in day_logs.values() {
            let bytes = encode_log(l);
            logs.push(LogInfo {
                region: l.region,
                from,
                to,
                bytes: bytes.len() as u64,
            });
            new_files.push((log_path(&ver, l.region, from), bytes));
        }
        let cp = write_checkpoint(&world, &ver, &mut new_files);
        checkpoints.push(cp);
        days += 1;
    }
    let simulated = world.tick - start_tick;
    eprintln!("simulated {simulated} ticks ({days} days) in {:?}; frontier {} (now {now_tick}, +{} ticks ahead)", started.elapsed(), world.tick, world.tick as i64 - now_tick as i64);

    // ---- Retention: keep one checkpoint before "now" so late visitors still find one.
    let keep_from = floor_cp(now_tick)
        .saturating_sub(CHECKPOINT_TICKS)
        .min(world.tick);
    checkpoints.retain(|c| c.tick >= keep_from);
    checkpoints.sort_by_key(|c| c.tick);
    logs.retain(|l| l.to > keep_from);
    logs.sort_by_key(|l| (l.region, l.from));
    logs.dedup_by_key(|l| (l.region, l.from));

    for (p, b) in keep_files.iter().chain(new_files.iter()) {
        let keep = checkpoints
            .iter()
            .any(|c| c.regions.iter().any(|r| cp_path(&ver, c.tick, r.id) == *p))
            || logs.iter().any(|l| log_path(&ver, l.region, l.from) == *p);
        if keep {
            write(&args.out, p, b)?;
        }
    }

    // Static data of every region at the frontier (for towns a browser does not simulate).
    for r in &world.regions {
        write(
            &args.out,
            &static_path(&ver, world.tick, r.id),
            static_json(r).as_bytes(),
        )?;
    }

    let c = catalog::content();
    let manifest = Manifest {
        manifest_version: MANIFEST_VERSION,
        format_version: snapshot::FORMAT_VERSION,
        sim_version: sim_version.clone(),
        ver: ver.clone(),
        seed,
        epoch_ms: epoch,
        ms_per_tick: REAL_MS_PER_TICK,
        window_ticks: WINDOW,
        checkpoint_ticks: CHECKPOINT_TICKS,
        frontier_tick: world.tick,
        checkpoints,
        logs,
        regions: world
            .regions
            .iter()
            .map(|r| RegionInfo {
                id: r.id,
                name: r.name().to_string(),
                template: c.world.templates[r.template as usize].id.clone(),
                population: r.population(),
                origin: [r.origin.0, r.origin.1],
                size: r.map.width,
                summary: summary_of(r),
                static_file: static_path(&ver, world.tick, r.id),
            })
            .collect(),
        highways: world
            .intercity
            .highways
            .iter()
            .map(|h| HighwayInfo {
                a: h.a,
                b: h.b,
                length_tiles: h.length_tiles,
            })
            .collect(),
        runner: RunnerInfo {
            finished_at_ms: now_ms(),
            duration_ms: started.elapsed().as_millis() as u64,
            simulated_ticks: simulated,
            rebased,
        },
    };
    write(
        &args.out,
        "manifest.json",
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    )?;
    if let Some(b) = &args.backup {
        std::fs::write(b, world.serialize()).map_err(|e| format!("{}: {e}", b.display()))?;
        eprintln!("backup written to {}", b.display());
    }
    eprintln!("done in {:?}", started.elapsed());
    Ok(())
}

fn summary_of(r: &Region) -> RegionSummary {
    let s = r.stats();
    RegionSummary {
        households: s.households,
        buildings: s.buildings,
        unemployment_permille: s.economy.unemployment_permille,
        treasury_cents: s.economy.treasury_cents,
        under_construction: s.construction.open.len() as u32,
        incidents_30d: s.safety.by_crime_30d.iter().map(|(_, n)| n).sum(),
        inmates: s.safety.inmates + s.safety.inmates_from_elsewhere,
        founded_at: r.systems.expansion.founded_at,
        parent: r.systems.expansion.parent,
        technologies: r.systems.technology.researched.len() as u32,
    }
}

fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The same static data the browser gets from a simulated region (see `livingsim-wasm`).
fn static_json(r: &Region) -> String {
    use livingsim_core::render;
    let gateways: Vec<_> = r
        .gateways
        .iter()
        .map(|g| {
            serde_json::json!({
                "building": g.building, "to_region": g.to_region, "to_name": g.to_name, "node": g.node,
                "x": r.roads.node_x[g.node as usize], "y": r.roads.node_y[g.node as usize],
            })
        })
        .collect();
    serde_json::json!({
        "info": {
            "id": r.id, "name": r.name(), "width": r.map.width, "height": r.map.height,
            "origin": [r.origin.0, r.origin.1], "gateways": gateways, "population": r.population(),
            "techs": r.systems.technology.researched,
        },
        "tiles": base64(&r.map.kind),
        "buildings": render::buildings_packed(r),
        "roads": render::roads_packed(r),
    })
    .to_string()
}

fn write_checkpoint(world: &World, ver: &str, files: &mut Vec<(String, Vec<u8>)>) -> Checkpoint {
    let mut regions = Vec::new();
    for r in &world.regions {
        let bytes = encode_region(r);
        regions.push(CheckpointRegion {
            id: r.id,
            hash: region_hash(r).to_hex(),
            bytes: bytes.len() as u64,
        });
        files.push((cp_path(ver, world.tick, r.id), bytes));
    }
    Checkpoint {
        tick: world.tick,
        regions,
    }
}

fn verify(dir: &Path) -> Result<(), String> {
    let src = Source::Dir(dir.to_path_buf());
    let m: Manifest = serde_json::from_slice(&src.get("manifest.json")?.ok_or("no manifest.json")?)
        .map_err(|e| e.to_string())?;
    let first = m.checkpoints.first().ok_or("no checkpoints")?;
    let last = m.checkpoints.last().unwrap();
    let mut failures = 0;
    for r in &first.regions {
        let cp = decode_region(
            &src.get(&cp_path(&m.ver, first.tick, r.id))?
                .ok_or("missing checkpoint")?,
        )
        .map_err(|e| e.to_string())?;
        let mut replay = livingsim_core::RegionReplay::new(cp);
        for l in m.logs.iter().filter(|l| l.region == r.id) {
            let log = snapshot::decode_log(
                &src.get(&log_path(&m.ver, l.region, l.from))?
                    .ok_or("missing log")?,
            )
            .map_err(|e| e.to_string())?;
            replay.add_log(log);
        }
        let t0 = Instant::now();
        replay.step_to(last.tick).map_err(|e| format!("{e:?}"))?;
        let expected = &last.regions.iter().find(|x| x.id == r.id).unwrap().hash;
        let got = region_hash(&replay.region).to_hex();
        let ok = &got == expected;
        failures += (!ok) as u32;
        println!(
            "region {} replayed {} ticks in {:?}: {}",
            r.id,
            last.tick - first.tick,
            t0.elapsed(),
            if ok { "OK" } else { "MISMATCH" }
        );
    }
    if failures > 0 {
        return Err(format!("{failures} region(s) diverged"));
    }
    Ok(())
}
