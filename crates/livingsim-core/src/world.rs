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

/// Highways between regions' gateways (HPA*-style top level; tiny for now).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IntercityGraph {
    pub highways: Vec<Highway>,
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
            r.origin = (x, (first - size) / 2);
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
