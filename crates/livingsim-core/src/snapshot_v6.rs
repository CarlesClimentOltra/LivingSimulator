//! Snapshot format 6 (Phase 5), kept to migrate old snapshots: the expansion state was a
//! stub (see [`ExpansionV6`](crate::snapshot_v5::ExpansionV6)).

use crate::snapshot::{from_bytes, to_bytes, Kind, SnapshotError};
use crate::snapshot_v5::{WorldV5, V6};
use crate::systems::expansion::ExpansionState;

fn upgrade(
    r: V6,
) -> crate::snapshot_v5::RegionV5<
    crate::systems::construction::ConstructionState,
    crate::systems::crime::CrimeState,
    crate::systems::justice::JusticeState,
    crate::content_ids::ContentIds,
    ExpansionState,
> {
    // Towns of the initial world: founded at 0, no parent; pressure starts from zero.
    r.map_parts(|c| c, |c| c, |j| j, |i| i, |_| ExpansionState::default())
}

/// Format 6 → 7.
pub fn to_next(kind: Kind, payload: Vec<u8>) -> Result<Vec<u8>, SnapshotError> {
    match kind {
        Kind::Region => {
            let r: V6 = from_bytes(&payload)?;
            Ok(to_bytes(&upgrade(r)))
        }
        Kind::World => {
            let w: WorldV5<V6> = from_bytes(&payload)?;
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
