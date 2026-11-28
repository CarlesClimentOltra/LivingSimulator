//! `manifest.json`: the index of everything published for the current world.

use serde::{Deserialize, Serialize};

pub const MANIFEST_VERSION: u32 = 1;
/// Checkpoint spacing: one simulated day = one real hour.
pub const CHECKPOINT_TICKS: u32 = 1440;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionInfo {
    pub id: u16,
    pub name: String,
    pub template: String,
    pub population: u32,
    /// World tile coordinates of the region's (0, 0) tile.
    pub origin: [i32; 2],
    pub size: u16,
    /// Phase 6: a summary for the world map, and the region's static data (map, buildings,
    /// roads) so browsers can draw towns they do not simulate.
    #[serde(default)]
    pub summary: RegionSummary,
    #[serde(default)]
    pub static_file: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RegionSummary {
    pub households: u32,
    pub buildings: u32,
    pub unemployment_permille: u32,
    pub treasury_cents: i64,
    pub under_construction: u32,
    pub incidents_30d: u32,
    pub inmates: u32,
    pub founded_at: u32,
    pub parent: Option<u16>,
    #[serde(default)]
    pub technologies: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointRegion {
    pub id: u16,
    pub hash: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub tick: u32,
    pub regions: Vec<CheckpointRegion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogInfo {
    pub region: u16,
    pub from: u32,
    pub to: u32,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HighwayInfo {
    pub a: u16,
    pub b: u16,
    pub length_tiles: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunnerInfo {
    pub finished_at_ms: u64,
    pub duration_ms: u64,
    pub simulated_ticks: u32,
    pub rebased: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: u32,
    pub format_version: u16,
    pub sim_version: String,
    /// Short version used in file paths.
    pub ver: String,
    pub seed: u64,
    pub epoch_ms: u64,
    pub ms_per_tick: u64,
    pub window_ticks: u32,
    pub checkpoint_ticks: u32,
    pub frontier_tick: u32,
    pub checkpoints: Vec<Checkpoint>,
    pub logs: Vec<LogInfo>,
    pub regions: Vec<RegionInfo>,
    pub highways: Vec<HighwayInfo>,
    pub runner: RunnerInfo,
}

pub fn ver_of(sim_version: &str) -> String {
    sim_version.chars().take(16).collect()
}

pub fn static_path(ver: &str, tick: u32, region: u16) -> String {
    format!("{ver}/static/{tick}/r{region}.json")
}

pub fn cp_path(ver: &str, tick: u32, region: u16) -> String {
    format!("{ver}/cp/{tick}/r{region}.bin.zst")
}

pub fn log_path(ver: &str, region: u16, from: u32) -> String {
    format!("{ver}/msg/r{region}/{from}.bin.zst")
}
