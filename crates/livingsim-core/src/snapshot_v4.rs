//! Snapshot format 4 (Phases 1–3 with content ids), kept to migrate old snapshots.
//!
//! The construction state was a stub; it is frozen here. The rest of the layout is the
//! generic [`RegionV5`](crate::snapshot_v5::RegionV5) with the frozen format-5 parts.

use crate::snapshot::{from_bytes, to_bytes, Kind, SnapshotError};
use crate::snapshot_v5::{ContentIdsV5, CrimeV5, ExpansionV6, JusticeV5, RegionV5, WorldV5};
use crate::systems::construction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PhaseV4 {
    Planned,
    Foundation,
    Structure,
    Roof,
    Finished,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectV4 {
    pub building_def: u16,
    pub x: u16,
    pub y: u16,
    pub phase: PhaseV4,
    pub work_left: u32,
    pub firm_building: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConstructionV4 {
    pub projects: Vec<ProjectV4>,
}

type V4 = RegionV5<ConstructionV4, CrimeV5, JusticeV5, ContentIdsV5, ExpansionV6>;
type V5 = RegionV5<construction::ConstructionState, CrimeV5, JusticeV5, ContentIdsV5, ExpansionV6>;

fn upgrade(r: V4) -> V5 {
    // The stub never held projects. The growth ring is added on the first morning
    // (`bootstrap_construction`), deterministically for the runner and the browser.
    r.map_parts(
        |_| construction::ConstructionState::default(),
        |c| c,
        |j| j,
        |i| i,
        |x| x,
    )
}

/// Format 4 → 5.
pub fn to_next(kind: Kind, payload: Vec<u8>) -> Result<Vec<u8>, SnapshotError> {
    match kind {
        Kind::Region => {
            let r: V4 = from_bytes(&payload)?;
            Ok(to_bytes(&upgrade(r)))
        }
        Kind::World => {
            let w: WorldV5<V4> = from_bytes(&payload)?;
            Ok(to_bytes(&WorldV5 {
                seed: w.seed,
                tick: w.tick,
                regions: w.regions.into_iter().map(upgrade).collect(),
                intercity: w.intercity,
            }))
        }
        Kind::MessageLog => Ok(payload),
    }
}
