//! The world: a set of regions plus the highways between them.

use crate::catalog::content;
use crate::gen::{generate_region, region_name, region_size, GatewaySpec, Side};
use crate::messages::{Message, WINDOW};
use crate::region::Region;
use crate::roads::decis_per_tile;
use crate::time::Tick;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Highway {
    pub a: u16,
    pub b: u16,
    pub length_tiles: u32,
}

/// Highways between regions' gateways: the top level of hierarchical routing (inside a
/// region, the road graph; between regions, this graph of highways).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IntercityGraph {
    pub highways: Vec<Highway>,
}

impl IntercityGraph {
    /// Regions along the shortest highway route from `a` to `b` (both included), and its
    /// length in tiles; `None` if they are not connected.
    pub fn route(&self, a: u16, b: u16) -> Option<(Vec<u16>, u32)> {
        let n = self
            .highways
            .iter()
            .map(|h| h.a.max(h.b) as usize + 1)
            .max()
            .unwrap_or(0)
            .max(a.max(b) as usize + 1);
        let mut dist = vec![u32::MAX; n];
        let mut prev = vec![u16::MAX; n];
        let mut done = vec![false; n];
        dist[a as usize] = 0;
        // Few regions: a linear scan is enough (ties: lowest id).
        while let Some(u) = (0..n)
            .filter(|i| !done[*i] && dist[*i] != u32::MAX)
            .min_by_key(|i| (dist[*i], *i))
        {
            done[u] = true;
            for h in &self.highways {
                let v = if h.a as usize == u {
                    h.b
                } else if h.b as usize == u {
                    h.a
                } else {
                    continue;
                };
                let d = dist[u] + h.length_tiles;
                if d < dist[v as usize] {
                    dist[v as usize] = d;
                    prev[v as usize] = u as u16;
                }
            }
        }
        if dist[b as usize] == u32::MAX {
            return None;
        }
        let mut path = vec![b];
        let mut cur = b;
        while cur != a {
            cur = prev[cur as usize];
            path.push(cur);
        }
        path.reverse();
        Some((path, dist[b as usize]))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub seed: u64,
    pub tick: Tick,
    pub regions: Vec<Region>,
    pub intercity: IntercityGraph,
}

/// Delivered messages by destination region, for one exchange.
pub type Deliveries = Vec<(u16, Vec<Message>)>;

impl World {
    /// Generates the initial world described in `data/world.ron` at tick 0.
    pub fn new(seed: u64) -> World {
        World::generate(seed, 0)
    }

    /// Generates the initial world with its clock set to `start` (used to restart a world
    /// without changing the epoch). `start` must be a multiple of the window size.
    pub fn generate(seed: u64, start: Tick) -> World {
        let c = content();
        let specs: Vec<(u16, u16)> = c.world.initial_regions.clone();
        World::generate_with(seed, start, &specs, true)
    }

    /// Generates `n` copies of the given template, unconnected (for benchmarks).
    pub fn synthetic(seed: u64, template: &str, n: u16) -> World {
        let c = content();
        let t = c
            .world
            .templates
            .iter()
            .position(|x| x.id == template)
            .expect("template") as u16;
        let specs: Vec<(u16, u16)> = (0..n)
            .map(|i| (t, i % c.names.styles.len() as u16))
            .collect();
        World::generate_with(seed, 0, &specs, false)
    }

    fn generate_with(seed: u64, start: Tick, specs: &[(u16, u16)], connect: bool) -> World {
        assert!(
            start.is_multiple_of(WINDOW),
            "worlds start on a window boundary"
        );
        let c = content();
        let names: Vec<String> = specs
            .iter()
            .enumerate()
            .map(|(i, (_, s))| region_name(seed, i as u16, *s))
            .collect();
        let hw = c.world.highway_tiles;
        let hw_ticks = (hw
            * decis_per_tile(crate::roads::Mode::Car, crate::roads::RoadClass::Highway))
        .div_ceil(600);
        let mut regions = Vec::new();
        let mut highways = Vec::new();
        let mut x = 0i32;
        let n = specs.len();
        for (i, (tpl, style)) in specs.iter().enumerate() {
            let mut gws = Vec::new();
            if connect && i > 0 {
                gws.push(GatewaySpec {
                    side: Side::West,
                    to_region: i as u16 - 1,
                    to_name: names[i - 1].clone(),
                    highway_ticks: hw_ticks,
                });
            }
            if connect && i + 1 < n {
                gws.push(GatewaySpec {
                    side: Side::East,
                    to_region: i as u16 + 1,
                    to_name: names[i + 1].clone(),
                    highway_ticks: hw_ticks,
                });
                highways.push(Highway {
                    a: i as u16,
                    b: i as u16 + 1,
                    length_tiles: hw,
                });
            }
            let mut r = generate_region(i as u16, *tpl, *style, seed, start, &gws);
            let size = region_size(*tpl) as i32;
            // Regions are laid out west to east, vertically centred on the first one.
            let first = region_size(specs[0].0) as i32;
            // The growth ring around the town extends the map on every side.
            let grow = (r.map.width as i32 - size) / 2;
            r.origin = (x - grow, (first - size) / 2 - grow);
            x += size + if connect { hw as i32 / 4 } else { 64 };
            regions.push(r);
        }
        World {
            seed,
            tick: start,
            regions,
            intercity: IntercityGraph { highways },
        }
    }

    /// Exchanges messages between regions (start of a window). Returns what was delivered.
    pub fn exchange(&mut self) -> Deliveries {
        if self.tick.is_multiple_of(crate::time::TICKS_PER_DAY) {
            self.maybe_found();
        }
        let mut all: Vec<Message> = Vec::new();
        for r in self.regions.iter_mut() {
            all.extend(r.take_outbox());
        }
        all.sort_by_key(|m| (m.dest, m.src, m.seq));
        let mut out: Deliveries = Vec::new();
        for m in all {
            if (m.dest as usize) >= self.regions.len() {
                continue;
            }
            match out.last_mut() {
                Some((d, v)) if *d == m.dest => v.push(m),
                _ => out.push((m.dest, vec![m])),
            }
        }
        for (d, msgs) in &out {
            self.regions[*d as usize].receive(msgs);
        }
        out
    }

    /// Phase 6: founds a new town next to the town under most pressure, when the world is
    /// old enough, no town was founded recently and there is room on the map. Runs at
    /// 06:00, before the exchange, so the new town's `RoadBuilt` message reaches its parent
    /// in the same window (and in the parent's message log).
    fn maybe_found(&mut self) {
        let c = content();
        let e = &c.world.expansion;
        let day = crate::time::TICKS_PER_DAY;
        if self.regions.len() >= e.max_regions as usize
            || self.tick < e.min_world_age_days as Tick * day
        {
            return;
        }
        let last = self
            .regions
            .iter()
            .map(|r| r.systems.expansion.founded_at)
            .max()
            .unwrap_or(0);
        if last > 0 && self.tick < last + e.cooldown_days as Tick * day {
            return;
        }
        // Candidates: towns over the threshold, most pressure first.
        let mut cands: Vec<(u32, u16)> = self
            .regions
            .iter()
            .filter(|r| r.systems.expansion.pressure >= e.threshold)
            .map(|r| (r.systems.expansion.pressure, r.id))
            .collect();
        cands.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
        for (_, p) in cands {
            if let Some((side, origin)) = self.free_side(p) {
                self.found(p, side, origin);
                return;
            }
        }
    }

    /// A side of region `p` without a highway exit where a new town fits; returns the side
    /// and the new town's origin.
    fn free_side(&self, p: u16) -> Option<(Side, (i32, i32))> {
        let c = content();
        let tpl = c.world.expansion.template;
        let t = &c.world.templates[tpl as usize];
        let size =
            (region_size(tpl) + (t.max_blocks - t.blocks) / 2 * 2 * crate::map::PITCH) as i32;
        // The new town's gateway sits on its central line, like every town's.
        let centre =
            (crate::map::MARGIN as i32) + (t.max_blocks as i32 / 2) * crate::map::PITCH as i32;
        let gap = (c.world.highway_tiles / 4) as i32;
        let parent = &self.regions[p as usize];
        let used: Vec<Side> = (0..parent.gateways.len())
            .map(|g| parent.gateway_side(g))
            .collect();
        for side in [Side::North, Side::South, Side::East, Side::West] {
            if used.contains(&side) {
                continue;
            }
            let (gx, gy) = parent.gateway_point(side);
            let (wx, wy) = (parent.origin.0 + gx as i32, parent.origin.1 + gy as i32);
            let origin = match side {
                Side::North => (wx - centre, wy - gap - (size - 1)),
                Side::South => (wx - centre, wy + gap),
                Side::East => (wx + gap, wy - centre),
                Side::West => (wx - gap - (size - 1), wy - centre),
            };
            let margin = 40;
            let overlaps = self.regions.iter().any(|r| {
                let (ax0, ay0) = (origin.0 - margin, origin.1 - margin);
                let (ax1, ay1) = (origin.0 + size + margin, origin.1 + size + margin);
                let (bx0, by0) = r.origin;
                let (bx1, by1) = (bx0 + r.map.width as i32, by0 + r.map.height as i32);
                ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
            });
            if !overlaps {
                return Some((side, origin));
            }
        }
        None
    }

    /// Founds a new town next to region `p` now, if a side is free (tests and tools; the
    /// simulation itself decides in `maybe_found`). Returns the new region id.
    pub fn found_town(&mut self, p: u16) -> Option<u16> {
        let (side, origin) = self.free_side(p)?;
        self.found(p, side, origin);
        Some(self.regions.len() as u16 - 1)
    }

    fn found(&mut self, p: u16, side: Side, origin: (i32, i32)) {
        let c = content();
        let tpl = c.world.expansion.template;
        let id = self.regions.len() as u16;
        let style = id % c.names.styles.len() as u16;
        let name = region_name(self.seed, id, style);
        let hw = c.world.highway_tiles;
        let hw_ticks = (hw
            * decis_per_tile(crate::roads::Mode::Car, crate::roads::RoadClass::Highway))
        .div_ceil(600);
        let parent_name = self.regions[p as usize].name().to_string();
        let spec = GatewaySpec {
            side: side.opposite(),
            to_region: p,
            to_name: parent_name,
            highway_ticks: hw_ticks,
        };
        let mut r = generate_region(id, tpl, style, self.seed, self.tick, &[spec]);
        r.origin = origin;
        r.systems.expansion.founded_at = self.tick;
        r.systems.expansion.parent = Some(p);
        // Settlers bring their town's technology with them.
        let known = self.regions[p as usize]
            .systems
            .technology
            .researched
            .clone();
        r.systems.technology.researched_at = vec![self.tick; known.len()];
        r.systems.technology.researched = known;
        r.refresh_tech();
        r.send(
            p,
            crate::messages::Body::RoadBuilt {
                side: side.code(),
                name: name.clone(),
                highway_ticks: hw_ticks,
            },
        );
        self.regions.push(r);
        self.intercity.highways.push(Highway {
            a: p,
            b: id,
            length_tiles: hw,
        });
    }

    /// Advances one tick.
    pub fn step(&mut self) -> Option<Deliveries> {
        let d = if self.tick.is_multiple_of(WINDOW) {
            Some(self.exchange())
        } else {
            None
        };
        for r in self.regions.iter_mut() {
            debug_assert_eq!(r.tick, self.tick);
            r.step();
        }
        self.tick += 1;
        d
    }

    /// Advances to `target` (exclusive of events at `target`).
    pub fn step_to(&mut self, target: Tick) {
        while self.tick < target {
            self.step();
        }
    }

    /// Advances one full window (`tick` must be on a window boundary), stepping regions in
    /// parallel with rayon when the `parallel` feature is on. Identical results either way.
    pub fn step_window(&mut self) -> Deliveries {
        assert!(self.tick.is_multiple_of(WINDOW));
        let d = self.exchange();
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            self.regions.par_iter_mut().for_each(|r| {
                for _ in 0..WINDOW {
                    r.step();
                }
            });
        }
        #[cfg(not(feature = "parallel"))]
        for r in self.regions.iter_mut() {
            for _ in 0..WINDOW {
                r.step();
            }
        }
        self.tick += WINDOW;
        d
    }

    pub fn population(&self) -> u32 {
        self.regions.iter().map(|r| r.population()).sum()
    }

    pub fn events(&self) -> u64 {
        self.regions.iter().map(|r| r.events).sum()
    }

    pub fn serialize(&self) -> Vec<u8> {
        crate::snapshot::encode_world(self)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<World, crate::snapshot::SnapshotError> {
        crate::snapshot::decode_world(bytes)
    }

    pub fn state_hash(&self) -> crate::snapshot::StateHash {
        crate::snapshot::hash_of(self)
    }

    /// Rebuilds a world from region checkpoints taken at the same tick (window boundary).
    /// Pending messages travel inside the regions' outboxes, so nothing else is needed.
    pub fn from_regions(seed: u64, intercity: IntercityGraph, mut regions: Vec<Region>) -> World {
        regions.sort_by_key(|r| r.id);
        let tick = regions.first().map(|r| r.tick).unwrap_or(0);
        assert!(
            regions.iter().all(|r| r.tick == tick),
            "regions must share a tick"
        );
        World {
            seed,
            tick,
            regions,
            intercity,
        }
    }
}
