//! Snapshot format 7 (Phase 6), kept to migrate old snapshots: the technology state was a
//! stub (see [`TechnologyV7`](crate::snapshot_v5::TechnologyV7)).

use crate::snapshot::{from_bytes, to_bytes, Kind, SnapshotError};
use crate::snapshot_v5::{RegionV5, WorldV5};
use crate::snapshot_v6::V7;
use crate::systems::technology::TechnologyState;

type V8 = RegionV5<
    crate::systems::construction::ConstructionState,
    crate::systems::crime::CrimeState,
    crate::systems::justice::JusticeState,
    crate::content_ids::ContentIds,
    crate::systems::expansion::ExpansionState,
    TechnologyState,
>;

fn upgrade(r: V7) -> V8 {
    // Nothing was ever researched: every town starts with today's technology.
    r.map_parts(
        |c| c,
        |c| c,
        |j| j,
        |i| i,
        |x| x,
        |_| TechnologyState::default(),
    )
}

/// Format 7 → 8.
pub fn to_next(kind: Kind, payload: Vec<u8>) -> Result<Vec<u8>, SnapshotError> {
    match kind {
        Kind::Region => {
            let r: V7 = from_bytes(&payload)?;
            Ok(to_bytes(&upgrade(r)))
        }
        Kind::World => {
            let w: WorldV5<V7> = from_bytes(&payload)?;
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
