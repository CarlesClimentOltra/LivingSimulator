//! Phase 6: founding new towns, and new highways between regions.
//!
//! Every morning each town adds up "pressure": people who had to leave for lack of work or
//! a home, and households waiting for a home beyond the free ones; much more once the town
//! has no room left to grow. The world (see `World::maybe_found`) founds a new town next to
//! the town under most pressure. The new town announces itself with a `RoadBuilt` message,
//! and its parent builds the highway exit on its side (so every region still changes only
//! through its own messages, and browsers replay it exactly).

use super::DailyContext;
use crate::catalog::content;
use crate::gen::{place_gateway, GatewaySpec, Side};
use crate::map::{TileKind, MARGIN, PITCH};
use crate::region::{NewsItem, Region};
use crate::rng::{keyed, Rng};
use crate::roads::{RoadClass, NONE};
use crate::time::Tick;
use livingsim_content::{LotSize, NewsKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExpansionState {
    /// Accumulated pressure towards founding a new town (decays 1% a day).
    pub pressure: u32,
    /// Free small lots in developed blocks (yesterday).
    pub free_lots: u32,
    /// Empty blocks inside the growth frame that could still be opened.
    pub open_blocks: u32,
    /// When this town was founded (0 for the towns of the initial world).
    pub founded_at: Tick,
    /// The town whose settlers founded this one.
    pub parent: Option<u16>,
    /// Towns founded from here.
    pub children: Vec<u16>,
}

impl ExpansionState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // Expansion needs the whole region; see `Region::expansion_daily`.
    }
    /// True when the town has no room left to grow.
    pub fn land_full(&self) -> bool {
        self.open_blocks == 0 && self.free_lots < content().world.expansion.full_land_free_lots
    }
}

impl Region {
    /// Side of the map a gateway is on.
    pub fn gateway_side(&self, gi: usize) -> Side {
        let n = self.gateways[gi].node as usize;
        let (x, y) = (self.roads.node_x[n], self.roads.node_y[n]);
        if x == 0 {
            Side::West
        } else if x + 1 >= self.map.width {
            Side::East
        } else if y == 0 {
            Side::North
        } else {
            Side::South
        }
    }

    /// Where the highway exit on `side` is (or would be), in map coordinates.
    pub fn gateway_point(&self, side: Side) -> (u16, u16) {
        let nb = self.systems.construction.nb.max(1) as i32;
        let c = (MARGIN as i32 + (nb / 2) * PITCH as i32) as u16;
        let (w, h) = (self.map.width, self.map.height);
        match side {
            Side::East => (w - 1, c),
            Side::West => (0, c),
            Side::North => (c, 0),
            Side::South => (c, h - 1),
        }
    }

    pub(crate) fn expansion_daily(&mut self, _t: Tick) {
        let e = &content().world.expansion;
        // Room to grow.
        let nb = self.systems.construction.nb as i32;
        let (mut free_lots, mut open_blocks) = (0u32, 0u32);
        for by in 0..nb {
            for bx in 0..nb {
                if self.block_developed(bx, by) {
                    free_lots += self.free_small_lots(bx, by);
                } else if self.block_is_empty(bx, by) {
                    open_blocks += 1;
                }
            }
        }
        // Pressure.
        let d = &self.systems.demography.yesterday;
        let left = d.emigrants + d.moved_out;
        let gap = self.housing_gap(self.tick);
        let seekers = self.long_term_jobseekers(self.tick).len() as u32;
        let mut pts = left * e.points_per_emigrant
            + gap * e.points_per_waiting_household
            + seekers * e.points_per_jobseeker_permille / 1000;
        let x = &mut self.systems.expansion;
        x.free_lots = free_lots;
        x.open_blocks = open_blocks;
        if x.land_full() {
            pts = pts * e.full_land_multiplier_permille / 1000;
        }
        x.pressure = x.pressure - x.pressure / 100 + pts;
    }

    /// A new town down a new highway (message `RoadBuilt` from the new town).
    pub(crate) fn receive_road_built(
        &mut self,
        src: u16,
        side: u8,
        to_name: &str,
        highway_ticks: Tick,
    ) {
        if self.gateways.iter().any(|g| g.to_region == src) {
            return;
        }
        let side = Side::from_code(side);
        self.add_gateway(side, src, to_name, highway_ticks);
        let x = &mut self.systems.expansion;
        x.pressure = 0;
        x.children.push(src);
        let t = self.tick;
        self.push_news(NewsItem {
            tick: t,
            kind: NewsKind::TownFounded as u16,
            subject: src as u32,
            n: 0,
        });
        // Settlers: families of long-term job seekers try their luck in the new town.
        let mut moved = 0;
        let mut seen: Vec<u32> = Vec::new();
        for a in self.long_term_jobseekers(t) {
            if moved >= content().world.expansion.settler_households {
                break;
            }
            let hh = self.agents.household[a as usize];
            if hh == NONE || seen.contains(&hh) || !self.resident(a) {
                continue;
            }
            seen.push(hh);
            let members = self.household_members(hh);
            // Only if nobody else in the household has a job keeping them here.
            if members
                .iter()
                .any(|m| *m != a && self.agents.profession[*m as usize] != u16::MAX)
            {
                continue;
            }
            self.send_movers(&members, src, t);
            moved += 1;
        }
    }

    /// Adults looking for work for a long time.
    fn long_term_jobseekers(&self, t: Tick) -> Vec<crate::agents::AgentId> {
        let days = content().world.expansion.long_term_jobseeker_days as Tick;
        self.systems
            .demography
            .unemployed_since
            .iter()
            .filter(|(a, since)| {
                self.resident(**a)
                    && **since + days * crate::time::TICKS_PER_DAY <= t
                    && self.agents.profession[**a as usize] == u16::MAX
            })
            .map(|(a, _)| *a)
            .collect()
    }

    /// Builds a highway exit on `side`: a highway from the map edge along the town's
    /// central street line to the first junction already connected to the town.
    pub(crate) fn add_gateway(&mut self, side: Side, to: u16, to_name: &str, highway_ticks: Tick) {
        let nb = self.systems.construction.nb as i32;
        let cb = nb / 2;
        let line = |i: i32| (MARGIN as i32 + i * PITCH as i32) as u16;
        let edge = self.gateway_point(side);
        // Points from the edge inwards: the edge, then every grid junction on the way.
        let mut pts: Vec<(u16, u16)> = vec![edge];
        let steps: Vec<i32> = match side {
            Side::North | Side::West => (0..=nb).collect(),
            Side::South | Side::East => (0..=nb).rev().collect(),
        };
        for j in steps {
            pts.push(match side {
                Side::North | Side::South => (line(cb), line(j)),
                Side::East | Side::West => (line(j), line(cb)),
            });
        }
        let node_at = |r: &Region, p: (u16, u16)| {
            (0..r.roads.node_count() as u32)
                .find(|n| r.roads.node_x[*n as usize] == p.0 && r.roads.node_y[*n as usize] == p.1)
        };
        let gateway = self.roads.add_node(edge.0, edge.1);
        let mut prev = gateway;
        self.roads.finalize();
        for &p in &pts[1..] {
            let n = match node_at(self, p) {
                Some(n) => n,
                None => self.roads.add_node(p.0, p.1),
            };
            let (px, py) = (
                self.roads.node_x[prev as usize],
                self.roads.node_y[prev as usize],
            );
            let len =
                (px as i32 - p.0 as i32).unsigned_abs() + (py as i32 - p.1 as i32).unsigned_abs();
            self.roads.finalize();
            let connected = !self.roads.neighbours(n).is_empty();
            if self.roads.find_edge(prev, n).is_none() {
                self.roads.add_edge(prev, n, len as u16, RoadClass::Highway);
            }
            for y in py.min(p.1)..=py.max(p.1) {
                for x in px.min(p.0)..=px.max(p.0) {
                    let k = self.map.get(x, y);
                    let new = match k {
                        TileKind::Water | TileKind::Bridge => TileKind::Bridge,
                        TileKind::Street | TileKind::Avenue => k,
                        _ => TileKind::Highway,
                    };
                    self.map.set(x, y, new);
                }
            }
            self.roads.finalize();
            if connected {
                break;
            }
            prev = n;
        }
        self.roads.version += 1;
        self.roads.finalize();
        let spec = GatewaySpec {
            side,
            to_region: to,
            to_name: to_name.to_string(),
            highway_ticks,
        };
        let mut rng = Rng::new(self.seed, keyed(to as u64, self.tick as u64, 0x6A7E, 0));
        let style = self.style;
        let before = self.buildings.len();
        place_gateway(self, &mut rng, &spec, gateway, cb as u16, style);
        for _ in before..self.buildings.len() {
            self.buildings.present.push(0);
            self.systems.economy.firm_of.push(NONE);
        }
        self.buildings.rebuild_index(content().tags.len());
        self.systems.construction.static_version += 1;
    }

    fn block_is_empty(&self, bx: i32, by: i32) -> bool {
        let line = |i: i32| (MARGIN as i32 + i * PITCH as i32) as u16;
        let (x0, y0) = (line(bx) + 1, line(by) + 1);
        (0..PITCH - 1).all(|dy| {
            (0..PITCH - 1).all(|dx| {
                let (x, y) = (x0 + dx, y0 + dy);
                x < self.map.width && y < self.map.height && self.map.get(x, y) == TileKind::Grass
            })
        })
    }

    /// Free small lots in a developed block (whatever their frontage).
    fn free_small_lots(&self, bx: i32, by: i32) -> u32 {
        let line = |i: i32| (MARGIN as i32 + i * PITCH as i32) as u16;
        let (x0, y0) = (line(bx) + 1, line(by) + 1);
        let _ = LotSize::Small;
        (0..6u16)
            .filter(|k| {
                let (lx, ly) = (x0 + (k % 3) * 4, y0 + (k / 3) * 6);
                (ly..ly + 6).all(|y| {
                    (lx..lx + 4).all(|x| {
                        x < self.map.width
                            && y < self.map.height
                            && self.map.get(x, y) == TileKind::Grass
                    })
                })
            })
            .count() as u32
    }
}
