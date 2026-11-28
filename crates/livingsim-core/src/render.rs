//! Compact arrays for the renderer. These are views of the state; the renderer turns
//! them into instanced meshes and interpolates trips analytically (see ARCHITECTURE §6.3).

use crate::agents::{flags, ACT_AWAY, ACT_FREE_SLOT, ACT_TRAVEL};
use crate::buildings::BuildingId;
use crate::catalog::catalog;
use crate::region::Region;
use crate::roads::{Mode, NONE};
use crate::time::age_years;

/// u32 values per building in [`buildings_packed`].
pub const BUILDING_STRIDE: usize = 9;
/// u32 values per agent in [`agents_packed`].
pub const AGENT_STRIDE: usize = 8;

/// Activity codes for the renderer beyond content activity indices.
pub const R_TRAVEL: u32 = 0xFFFE;
pub const R_AWAY: u32 = 0xFFFD;
pub const R_NONE: u32 = 0xFFFF;

/// Per building: def, x, y, w, d, lot_x, lot_y, lot_w, lot_h.
pub fn buildings_packed(r: &Region) -> Vec<u32> {
    let b = &r.buildings;
    let mut out = Vec::with_capacity(b.len() * BUILDING_STRIDE);
    for i in 0..b.len() {
        let l = b.lot[i];
        out.extend_from_slice(&[
            b.def[i] as u32,
            b.x[i] as u32,
            b.y[i] as u32,
            b.w[i] as u32,
            b.d[i] as u32,
            l[0] as u32,
            l[1] as u32,
            l[2] as u32,
            l[3] as u32,
        ]);
    }
    out
}

/// u32 values per building site in [`sites_packed`].
pub const SITE_STRIDE: usize = 5;

/// Building sites: building, target def, phase (1 foundation, 2 structure, 3 roof),
/// progress of that phase (‰), and 1 while the phase is under way (0 waiting for money).
pub fn sites_packed(r: &Region) -> Vec<u32> {
    use crate::systems::construction::{Phase, Work};
    let mut out = Vec::new();
    for p in &r.systems.construction.projects {
        let Work::Building { site, def } = p.work else {
            continue;
        };
        if p.phase == Phase::Finished {
            continue;
        }
        let ph = if p.phase == Phase::Planned {
            p.next
        } else {
            p.phase
        };
        let (code, idx) = match ph {
            Phase::Foundation => (1, 0),
            Phase::Structure => (2, 1),
            _ => (3, 2),
        };
        let local = if p.phase == Phase::Planned {
            0
        } else {
            (p.done * 1000 / p.units[idx].max(1)).min(1000)
        };
        out.extend_from_slice(&[
            site,
            def as u32,
            code,
            local,
            (p.phase != Phase::Planned) as u32,
        ]);
    }
    out
}

/// Per agent slot: act, place, trip_from, trip_to, depart, arrive, info where
/// info = flags | age << 8 | (shirt colour seed & 0xff) << 16, household.
pub fn agents_packed(r: &Region) -> Vec<u32> {
    let a = &r.agents;
    let t = r.tick;
    let mut out = Vec::with_capacity(a.len() * AGENT_STRIDE);
    for i in 0..a.len() {
        let act = match a.act[i] {
            ACT_TRAVEL => R_TRAVEL,
            ACT_AWAY => R_AWAY,
            ACT_FREE_SLOT => R_NONE,
            x => x as u32,
        };
        let alive = a.flags[i] & flags::ALIVE != 0;
        let act = if alive { act } else { R_NONE };
        let age = age_years(a.birth[i], t).min(255);
        let info = a.flags[i] as u32 | age << 8 | (a.traits[i] & 0xff) << 16;
        out.extend_from_slice(&[
            act,
            a.place[i],
            a.trip_from[i],
            a.trip_to[i],
            a.trip_depart[i],
            a.until[i],
            info,
            a.household[i],
        ]);
    }
    out
}

/// Per building: (occupants, awake occupants), packed as `occupants | awake << 16`.
pub fn occupancy(r: &Region) -> Vec<u32> {
    let mut occ = vec![0u32; r.buildings.len()];
    let sleep = catalog().sleep_activity;
    for i in 0..r.agents.len() {
        let p = r.agents.place[i];
        if p == NONE || r.agents.flags[i] & flags::ALIVE == 0 {
            continue;
        }
        let awake = r.agents.act[i] != sleep;
        occ[p as usize] += 1 + if awake { 1 << 16 } else { 0 };
    }
    occ
}

/// Polyline (tile coordinates ×2, i.e. half-tile precision) of the trip between two
/// buildings: door → road nodes → door.
pub fn route_points(r: &mut Region, from: BuildingId, to: BuildingId, car: bool) -> Vec<i32> {
    let mut pts = Vec::new();
    if from == NONE
        || to == NONE
        || from as usize >= r.buildings.len()
        || to as usize >= r.buildings.len()
    {
        return pts;
    }
    let mode = if car { Mode::Car } else { Mode::Walk };
    let (fa, ta) = (
        r.buildings.access_node[from as usize],
        r.buildings.access_node[to as usize],
    );
    let (fc, tc) = (r.buildings.centre2(from), r.buildings.centre2(to));
    pts.extend_from_slice(&[fc.0, fc.1]);
    pts.extend_from_slice(&[
        r.buildings.door_x[from as usize] as i32 * 2 + 1,
        r.buildings.door_y[from as usize] as i32 * 2 + 1,
    ]);
    if fa != NONE && ta != NONE {
        let path = r.routes.path(&r.roads, mode, fa, ta);
        for n in path {
            pts.extend_from_slice(&[
                r.roads.node_x[n as usize] as i32 * 2 + 1,
                r.roads.node_y[n as usize] as i32 * 2 + 1,
            ]);
        }
    }
    pts.extend_from_slice(&[
        r.buildings.door_x[to as usize] as i32 * 2 + 1,
        r.buildings.door_y[to as usize] as i32 * 2 + 1,
    ]);
    pts.extend_from_slice(&[tc.0, tc.1]);
    pts
}

/// Per road edge: x0, y0, x1, y1, class.
pub fn roads_packed(r: &Region) -> Vec<u32> {
    let g = &r.roads;
    let mut out = Vec::with_capacity(g.edge_count() * 5);
    for e in 0..g.edge_count() {
        let (a, b) = (g.edge_a[e] as usize, g.edge_b[e] as usize);
        out.extend_from_slice(&[
            g.node_x[a] as u32,
            g.node_y[a] as u32,
            g.node_x[b] as u32,
            g.node_y[b] as u32,
            g.edge_class[e] as u32,
        ]);
    }
    out
}
