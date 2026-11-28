//! Phase 4: construction and growth.
//!
//! Every Monday the town looks at what it lacks (homes, daycare, school places, a
//! cemetery, shops, jobs) and plans projects on free lots — or lays out new streets into
//! the farmland when there are none. A funder (town hall, real estate agency or investor)
//! pays each phase to the construction company (labour) and to the outside (materials).
//! Builders go to the site every workday; each shift is one unit of work. Projects go
//! through foundation → structure → roof; when done, the site becomes the real building,
//! with its business, owner and name.
//!
//! Sites are real buildings with the `construction_site` definition, so nothing else in
//! the simulation (shops, homes, schools) can use them until they are finished.

// Loops index several parallel vectors while also mutating `self`, so ranges are clearer.
#![allow(clippy::needless_range_loop)]

use super::economy::{Account, Firm, NO_GOOD};
use super::DailyContext;
use crate::agents::AgentId;
use crate::buildings::BuildingId;
use crate::catalog::{catalog, content};
use crate::map::{TileKind, TileMap, MARGIN, PITCH};
use crate::region::{NewsItem, Region};
use crate::rng::{keyed, Rng};
use crate::roads::{RoadClass, NONE};
use crate::time::{age_years, date, Tick, TICKS_PER_DAY};
use livingsim_content::{FirmKind, JournalKind, LotSize, NewsKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Phase {
    /// Waiting for money to start the next phase.
    Planned,
    Foundation,
    Structure,
    Roof,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Funder {
    Treasury,
    Firm(u32),
    Agent(AgentId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purpose {
    Housing,
    Service,
    Shop,
    Jobs,
    Street,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Work {
    /// A building on a lot (the site building holds the `construction_site` def).
    Building { site: BuildingId, def: u16 },
    /// A street between two road nodes.
    Road { a: u32, b: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: u32,
    pub work: Work,
    pub purpose: Purpose,
    pub funder: Funder,
    /// Construction firm doing the work, or `NONE` for outside contractors.
    pub builder: u32,
    pub phase: Phase,
    /// The phase that will start once paid (when `phase == Planned`).
    pub next: Phase,
    /// Units needed for foundation, structure and roof.
    pub units: [u32; 3],
    /// Units done in the current phase.
    pub done: u32,
    pub started: Tick,
    /// When it was finished (0 while under construction).
    pub finished: Tick,
}

impl Project {
    fn phase_index(p: Phase) -> usize {
        match p {
            Phase::Foundation => 0,
            Phase::Structure => 1,
            _ => 2,
        }
    }
    pub fn active(&self) -> bool {
        matches!(
            self.phase,
            Phase::Foundation | Phase::Structure | Phase::Roof
        )
    }
    /// Overall progress in ‰.
    pub fn progress(&self) -> u32 {
        let total: u32 = self.units.iter().sum::<u32>().max(1);
        let before: u32 = match (self.phase, self.next) {
            (Phase::Planned, p) => self.units[..Project::phase_index(p)].iter().sum(),
            (Phase::Finished, _) => total,
            (p, _) => self.units[..Project::phase_index(p)].iter().sum::<u32>() + self.done,
        };
        (before * 1000 / total).min(1000)
    }
}

/// Who owns the homes of a building built in Phase 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeOwner {
    Municipal,
    Agency(u32),
    Landlord(AgentId),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildTotals {
    pub started: u32,
    pub finished: u32,
    pub streets: u32,
    pub units: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConstructionState {
    /// The map has its growth ring and the block frame below is valid.
    pub expanded: bool,
    /// Blocks per side of the frame (street line `i` is at `MARGIN + i * PITCH`).
    pub nb: u16,
    /// Growth ring added around the original town, in blocks.
    pub extra: u16,
    pub projects: Vec<Project>,
    pub next_id: u32,
    pub owners: BTreeMap<BuildingId, HomeOwner>,
    /// Bumped whenever the map or the buildings change (the renderer reloads them).
    pub static_version: u32,
    pub today: BuildTotals,
    pub yesterday: BuildTotals,
    pub total: BuildTotals,
}

impl ConstructionState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // Construction needs the whole region; see `Region::construction_daily`.
    }
}

/// A free lot found for a project.
#[derive(Clone, Copy, Debug)]
struct Lot {
    rect: [u16; 4],
    north: bool,
    block: (i32, i32),
}

impl Region {
    fn line(&self, i: i32) -> u16 {
        (MARGIN as i32 + i * PITCH as i32) as u16
    }

    // ------------------------------------------------------------------ room to grow

    /// Adds `extra` blocks of undeveloped land around the town, moving every coordinate so
    /// the town keeps its position in the world.
    pub fn expand_canvas(&mut self, extra: u16) {
        let dx = extra * PITCH;
        let (ow, oh) = (self.map.width, self.map.height);
        let (nw, nh) = (ow + 2 * dx, oh + 2 * dx);
        let mut map = TileMap::new(nw, nh);
        for y in 0..oh {
            for x in 0..ow {
                let (o, n) = (self.map.idx(x, y), map.idx(x + dx, y + dx));
                map.kind[n] = self.map.kind[o];
                map.building[n] = self.map.building[o];
            }
        }
        // The river keeps flowing through the new land.
        for x in 0..ow {
            for (edge_y, range) in [(0u16, 0..dx), (oh - 1, oh + dx..nh)] {
                if self.map.get(x, edge_y) == TileKind::Water {
                    for y in range {
                        map.set(x + dx, y, TileKind::Water);
                    }
                }
            }
        }
        self.map = map;
        let b = &mut self.buildings;
        for i in 0..b.len() {
            b.x[i] += dx;
            b.y[i] += dx;
            b.lot[i][0] += dx;
            b.lot[i][1] += dx;
            b.door_x[i] += dx;
            b.door_y[i] += dx;
        }
        for i in 0..self.roads.node_count() {
            self.roads.node_x[i] += dx;
            self.roads.node_y[i] += dx;
        }
        self.origin = (self.origin.0 - dx as i32, self.origin.1 - dx as i32);
        // Highway exits move to the new edge of the map; the highway is extended.
        for gi in 0..self.gateways.len() {
            let node = self.gateways[gi].node as usize;
            let (gx, gy) = (self.roads.node_x[node], self.roads.node_y[node]);
            // Which edge the exit was on (in shifted coordinates).
            let horizontal = gx == dx || gx == ow - 1 + dx;
            let (nx, ny) = if horizontal {
                (if gx <= dx { 0 } else { nw - 1 }, gy)
            } else {
                (gx, if gy <= dx { 0 } else { nh - 1 })
            };
            self.roads.node_x[node] = nx;
            self.roads.node_y[node] = ny;
            for e in 0..self.roads.edge_count() {
                if self.roads.edge_a[e] as usize == node || self.roads.edge_b[e] as usize == node {
                    self.roads.edge_len[e] += dx;
                }
            }
            for y in gy.min(ny)..=gy.max(ny) {
                for x in gx.min(nx)..=gx.max(nx) {
                    self.map.set(x, y, TileKind::Highway);
                }
            }
            // Move the exit building next to the new node.
            let bld = self.gateways[gi].building as usize;
            let (sx, sy) = (nx as i32 - gx as i32, ny as i32 - gy as i32);
            for y in
                self.buildings.lot[bld][1]..self.buildings.lot[bld][1] + self.buildings.lot[bld][3]
            {
                for x in self.buildings.lot[bld][0]
                    ..self.buildings.lot[bld][0] + self.buildings.lot[bld][2]
                {
                    let i = self.map.idx(x, y);
                    self.map.kind[i] = TileKind::Grass as u8;
                    self.map.building[i] = NONE;
                }
            }
            let mv = |v: u16, s: i32| (v as i32 + s) as u16;
            let bb = &mut self.buildings;
            bb.x[bld] = mv(bb.x[bld], sx);
            bb.y[bld] = mv(bb.y[bld], sy);
            bb.lot[bld][0] = mv(bb.lot[bld][0], sx);
            bb.lot[bld][1] = mv(bb.lot[bld][1], sy);
            bb.door_x[bld] = mv(bb.door_x[bld], sx);
            bb.door_y[bld] = mv(bb.door_y[bld], sy);
            for y in bb.lot[bld][1]..bb.lot[bld][1] + bb.lot[bld][3] {
                for x in bb.lot[bld][0]..bb.lot[bld][0] + bb.lot[bld][2] {
                    let i = self.map.idx(x, y);
                    self.map.kind[i] = TileKind::Lot as u8;
                    self.map.building[i] = bld as u32;
                }
            }
        }
        // Names for the new street lines: horizontals first, then verticals (as in `gen`).
        let nb_old = self.systems.construction.nb as usize;
        let e = extra as usize;
        if self.streets.len() == 2 * (nb_old + 1) {
            let style = &content().names.styles[self.style as usize];
            let fresh = |r: &mut Region, k: u64| {
                let base = crate::names::generate(
                    &content().names,
                    &style.places,
                    keyed(r.seed, k, extra as u64, 0x57),
                );
                let suffixes = &content().names.street_suffixes;
                let suffix =
                    &suffixes[(keyed(r.seed, k, 1, 0x57) % suffixes.len() as u64) as usize];
                r.names.intern(&format!("{base} {suffix}"))
            };
            let old = std::mem::take(&mut self.streets);
            let mut out = Vec::with_capacity(old.len() + 4 * e);
            let mut k = (nb_old as u64) << 8;
            for half in old.chunks(nb_old + 1) {
                for _ in 0..e {
                    k += 1;
                    out.push(fresh(self, k));
                }
                out.extend_from_slice(half);
                for _ in 0..e {
                    k += 1;
                    out.push(fresh(self, k));
                }
            }
            self.streets = out;
            for s in self.buildings.street.iter_mut() {
                *s += extra;
            }
        }
        self.roads.version += 1;
        self.roads.finalize();
        let c = &mut self.systems.construction;
        c.nb += 2 * extra;
        c.extra += extra;
        c.static_version += 1;
    }

    /// Gives the region its growth ring (new worlds and migrated snapshots).
    pub fn bootstrap_construction(&mut self) {
        if self.systems.construction.expanded {
            return;
        }
        let tpl = &content().world.templates[self.template as usize];
        let extra = (tpl.max_blocks.saturating_sub(tpl.blocks)) / 2;
        self.systems.construction.nb = tpl.blocks;
        self.systems.construction.extra = 0;
        if extra > 0 {
            self.expand_canvas(extra);
        }
        self.systems.construction.expanded = true;
    }

    // ------------------------------------------------------------------ queries

    pub fn site_project(&self, b: BuildingId) -> Option<&Project> {
        self.systems
            .construction
            .projects
            .iter()
            .find(|p| matches!(p.work, Work::Building { site, .. } if site == b))
    }

    fn node_at(&self, x: u16, y: u16) -> Option<u32> {
        (0..self.roads.node_count())
            .find(|i| self.roads.node_x[*i] == x && self.roads.node_y[*i] == y)
            .map(|i| i as u32)
    }

    fn edge_exists(&self, a: (u16, u16), b: (u16, u16)) -> bool {
        match (self.node_at(a.0, a.1), self.node_at(b.0, b.1)) {
            (Some(na), Some(nb)) => self.roads.find_edge(na, nb).is_some(),
            _ => false,
        }
    }

    /// Corners of block (bx, by) and its four edges: north, south, west, east.
    fn block_edges(&self, bx: i32, by: i32) -> [((u16, u16), (u16, u16)); 4] {
        let (x0, x1, y0, y1) = (
            self.line(bx),
            self.line(bx + 1),
            self.line(by),
            self.line(by + 1),
        );
        [
            ((x0, y0), (x1, y0)),
            ((x0, y1), (x1, y1)),
            ((x0, y0), (x0, y1)),
            ((x1, y0), (x1, y1)),
        ]
    }

    pub(crate) fn block_developed(&self, bx: i32, by: i32) -> bool {
        self.block_edges(bx, by)
            .iter()
            .filter(|(a, b)| self.edge_exists(*a, *b))
            .count()
            >= 3
    }

    fn rect_free(&self, r: [u16; 4]) -> bool {
        for y in r[1]..r[1] + r[3] {
            for x in r[0]..r[0] + r[2] {
                if x >= self.map.width
                    || y >= self.map.height
                    || self.map.get(x, y) != TileKind::Grass
                {
                    return false;
                }
            }
        }
        true
    }

    fn lots_of(&self, bx: i32, by: i32, size: LotSize) -> Vec<Lot> {
        let (bx0, by0) = (self.line(bx) + 1, self.line(by) + 1);
        let n = match size {
            LotSize::Small => 6,
            LotSize::Medium => 4,
            LotSize::Large => 1,
        };
        (0..n)
            .map(|k: u16| {
                let (rect, north) = match size {
                    LotSize::Small => ([bx0 + (k % 3) * 4, by0 + (k / 3) * 6, 4, 6], k < 3),
                    LotSize::Medium => ([bx0 + (k % 2) * 6, by0 + (k / 2) * 6, 6, 6], k < 2),
                    LotSize::Large => ([bx0, by0, 12, 12], true),
                };
                Lot {
                    rect,
                    north,
                    block: (bx, by),
                }
            })
            .collect()
    }

    /// The best free lot of a size: in a developed block whose frontage street exists,
    /// closest to the centre (with a little deterministic variety).
    fn find_lot(&self, size: LotSize, t: Tick) -> Option<Lot> {
        let nb = self.systems.construction.nb as i32;
        let cb = nb / 2;
        let mut best: Option<(i64, Lot)> = None;
        for by in 0..nb {
            for bx in 0..nb {
                if !self.block_developed(bx, by) {
                    continue;
                }
                let edges = self.block_edges(bx, by);
                for lot in self.lots_of(bx, by, size) {
                    if !self.rect_free(lot.rect) {
                        continue;
                    }
                    let (a, b) = if lot.north { edges[0] } else { edges[1] };
                    if !self.edge_exists(a, b) {
                        continue;
                    }
                    let d = (bx - cb).abs().max((by - cb).abs()) as i64;
                    let noise = (keyed(
                        self.seed,
                        lot.rect[0] as u64,
                        lot.rect[1] as u64,
                        t as u64 / TICKS_PER_DAY as u64,
                    ) % 3) as i64;
                    let score = d * 4 + noise;
                    if best.as_ref().is_none_or(|(s, _)| score < *s) {
                        best = Some((score, lot));
                    }
                }
            }
        }
        best.map(|(_, l)| l)
    }

    // ------------------------------------------------------------------ planning

    fn builders(&self) -> Vec<u32> {
        self.systems
            .economy
            .firms
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == FirmKind::Construction && f.open)
            .map(|(i, _)| i as u32)
            .collect()
    }

    fn active_projects(&self) -> usize {
        self.systems
            .construction
            .projects
            .iter()
            .filter(|p| p.phase != Phase::Finished)
            .count()
    }

    fn project_cost(&self, units: [u32; 3]) -> i64 {
        let k = &content().construction;
        units.iter().sum::<u32>() as i64 * (k.labour_per_unit_cents + k.materials_per_unit_cents)
    }

    fn building_units(def: u16) -> [u32; 3] {
        let c = content();
        let d = &c.buildings[def as usize];
        let k = &c.construction;
        let tiles = d.width as u32 * d.depth as u32;
        if d.roof == livingsim_content::RoofKind::None {
            // Parks, plazas and cemeteries: landscaping only.
            return [tiles * k.foundation_per_tile, 0, 0];
        }
        [
            tiles * k.foundation_per_tile,
            tiles * d.floors as u32 * k.structure_per_tile_floor,
            tiles * k.roof_per_tile,
        ]
    }

    /// Weekly: decide what to build.
    fn plan(&mut self, t: Tick) {
        let c = content();
        let k = &c.construction;
        let cat = catalog();
        let capacity = (self.builders().len().max(1) * k.projects_per_firm as usize).max(2);
        let mut budget = k.new_projects_per_week as usize;
        let active_of = |r: &Region, purpose: Purpose| {
            r.systems
                .construction
                .projects
                .iter()
                .any(|p| p.phase != Phase::Finished && p.purpose == purpose)
        };
        let mut wants: Vec<(Purpose, u16)> = Vec::new();

        // Services.
        let count_tag = |r: &Region, tag: &str| {
            c.tag(tag)
                .map(|tg| r.buildings.with_tag(tg).len())
                .unwrap_or(0)
        };
        let mut toddlers_waiting = 0u32;
        let mut kids_without_school = 0u32;
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) {
                continue;
            }
            let i = a as usize;
            let age = age_years(self.agents.birth[i], t);
            if (1..6).contains(&age) && self.agents.schedule[i] == cat.schedule_toddler {
                toddlers_waiting += 1;
            }
            if (6..18).contains(&age) && self.agents.work[i] == NONE {
                kids_without_school += 1;
            }
        }
        if !active_of(self, Purpose::Service) {
            if count_tag(self, "cemetery") == 0 {
                wants.push((Purpose::Service, k.cemetery));
            } else if kids_without_school >= k.school_waiting_threshold {
                wants.push((Purpose::Service, k.school));
            } else if toddlers_waiting >= k.daycare_waiting_threshold {
                wants.push((Purpose::Service, k.daycare));
            }
        }
        // Housing.
        let waiting = self.households_waiting(t);
        let free: u32 = self.free_homes_count();
        let planned: u32 = self
            .systems
            .construction
            .projects
            .iter()
            .filter(|p| p.phase != Phase::Finished && p.purpose == Purpose::Housing)
            .map(|p| match p.work {
                Work::Building { def, .. } => c.buildings[def as usize].dwellings as u32,
                _ => 0,
            })
            .sum();
        let gap = waiting.saturating_sub(free + planned);
        if gap >= k.housing_waiting_threshold {
            let list = if gap >= 10 && !k.medium_housing.is_empty() {
                &k.medium_housing
            } else {
                &k.small_housing
            };
            let def = list[(keyed(self.seed, t as u64, 1, 0xC0) % list.len() as u64) as usize];
            wants.push((Purpose::Housing, def));
            if gap >= 25 {
                let def = k.small_housing
                    [(keyed(self.seed, t as u64, 2, 0xC0) % k.small_housing.len() as u64) as usize];
                wants.push((Purpose::Housing, def));
            }
        }
        // Shops and jobs.
        if self.yesterday.shelves_empty >= k.shop_shortage_threshold
            && !active_of(self, Purpose::Shop)
            && !k.shops.is_empty()
        {
            wants.push((Purpose::Shop, k.shops[0]));
        }
        let unemployment = self
            .systems
            .economy
            .history
            .last()
            .map(|h| h.unemployment_permille)
            .unwrap_or(0);
        if unemployment >= k.unemployment_threshold
            && !active_of(self, Purpose::Jobs)
            && !k.workplaces.is_empty()
        {
            wants.push((Purpose::Jobs, k.workplaces[0]));
        }

        for (purpose, def) in wants {
            if budget == 0 || self.active_projects() >= capacity {
                break;
            }
            let size = c.buildings[def as usize].lot;
            match self.find_lot(size, t) {
                Some(lot) => {
                    if self.start_building(def, lot, purpose, t) {
                        budget -= 1;
                    }
                }
                None => {
                    // No room: open up a new block (once its streets exist, lots are free).
                    let streets_underway = self
                        .systems
                        .construction
                        .projects
                        .iter()
                        .any(|p| p.phase != Phase::Finished && p.purpose == Purpose::Street);
                    // Only when someone could pay for the building afterwards.
                    let cost = self.project_cost(Region::building_units(def));
                    if !streets_underway && self.pick_funder(purpose, cost, t).is_some() {
                        self.plan_expansion(t);
                    }
                    break;
                }
            }
        }
    }

    /// Households that would like a home of their own.
    fn households_waiting(&self, t: Tick) -> u32 {
        let mut couples_apart = 0;
        let mut young_at_home = 0;
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) {
                continue;
            }
            let i = a as usize;
            let p = self.agents.partner[i];
            if p != NONE
                && p > a
                && self.resident(p)
                && self.agents.household[p as usize] != self.agents.household[i]
            {
                couples_apart += 1;
            }
            let age = age_years(self.agents.birth[i], t);
            if (24..=40).contains(&age) && p == NONE && self.agents.profession[i] != u16::MAX {
                let hh = self.agents.household[i];
                let with_parents = [self.agents.parent_a[i], self.agents.parent_b[i]]
                    .iter()
                    .any(|q| {
                        *q != NONE && self.resident(*q) && self.agents.household[*q as usize] == hh
                    });
                if with_parents {
                    young_at_home += 1;
                }
            }
        }
        couples_apart + young_at_home / 2
    }

    /// Households waiting for a home beyond the free ones.
    pub(crate) fn housing_gap(&self, t: Tick) -> u32 {
        self.households_waiting(t)
            .saturating_sub(self.free_homes_count())
    }

    fn free_homes_count(&self) -> u32 {
        let mut used = vec![0u16; self.buildings.len()];
        for h in 0..self.households.len() {
            if self.households.members[h] > 0 {
                used[self.households.home[h] as usize] += 1;
            }
        }
        let c = content();
        (0..self.buildings.len())
            .map(|b| {
                c.buildings[self.buildings.def[b] as usize]
                    .dwellings
                    .saturating_sub(used[b]) as u32
            })
            .sum()
    }

    /// Picks who pays: the town for public works, otherwise agencies, firms or investors.
    fn pick_funder(&self, purpose: Purpose, cost: i64, t: Tick) -> Option<Funder> {
        let e = &self.systems.economy;
        let monthly_spending: i64 = e.spending_30d.iter().sum::<i64>().max(1);
        let town_can = e.treasury > cost + monthly_spending;
        match purpose {
            Purpose::Service | Purpose::Street => return town_can.then_some(Funder::Treasury),
            Purpose::Housing
                if town_can && keyed(self.seed, t as u64, 3, 0xC0).is_multiple_of(3) =>
            {
                return Some(Funder::Treasury)
            }
            _ => {}
        }
        if purpose == Purpose::Housing {
            let agency = e
                .firms
                .iter()
                .enumerate()
                .filter(|(_, f)| f.kind == FirmKind::RealEstate && f.open && f.cash > cost * 3 / 2)
                .max_by_key(|(i, f)| (f.cash, std::cmp::Reverse(*i)))
                .map(|(i, _)| i as u32);
            if let Some(f) = agency {
                return Some(Funder::Firm(f));
            }
        }
        // A wealthy resident.
        let mut best: Option<(i64, AgentId)> = None;
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a)
                || !(25..=75).contains(&age_years(self.agents.birth[a as usize], t))
            {
                continue;
            }
            let m = self.agents.money[a as usize];
            if m > cost * 11 / 10
                && best
                    .is_none_or(|(bm, ba)| (m, std::cmp::Reverse(a)) > (bm, std::cmp::Reverse(ba)))
            {
                best = Some((m, a));
            }
        }
        if let Some((_, a)) = best {
            return Some(Funder::Agent(a));
        }
        // A profitable company investing its cash.
        let firm = e
            .firms
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_private() && f.open && f.owner != NONE && f.cash > cost * 2)
            .max_by_key(|(i, f)| (f.cash, std::cmp::Reverse(*i)))
            .map(|(i, _)| i as u32);
        if let Some(f) = firm {
            return Some(Funder::Firm(f));
        }
        (purpose == Purpose::Housing && town_can).then_some(Funder::Treasury)
    }

    fn funder_account(f: Funder) -> Account {
        match f {
            Funder::Treasury => Account::Treasury,
            Funder::Firm(x) => Account::Firm(x),
            Funder::Agent(a) => Account::Agent(a),
        }
    }

    fn funder_money(&self, f: Funder) -> i64 {
        match f {
            Funder::Treasury => self.systems.economy.treasury,
            Funder::Firm(x) => self.systems.economy.firms[x as usize].cash,
            Funder::Agent(a) => {
                if self.resident(a) {
                    self.agents.money[a as usize]
                } else {
                    0
                }
            }
        }
    }

    fn pick_builder(&self) -> u32 {
        let builders = self.builders();
        builders
            .into_iter()
            .min_by_key(|f| {
                (
                    self.systems
                        .construction
                        .projects
                        .iter()
                        .filter(|p| p.phase != Phase::Finished && p.builder == *f)
                        .count(),
                    *f,
                )
            })
            .unwrap_or(NONE)
    }

    fn start_building(&mut self, def: u16, lot: Lot, purpose: Purpose, t: Tick) -> bool {
        let c = content();
        let units = Region::building_units(def);
        let cost = self.project_cost(units);
        let Some(funder) = self.pick_funder(purpose, cost, t) else {
            return false;
        };
        let d = &c.buildings[def as usize];
        let [lx, ly, lw, lh] = lot.rect;
        let (fw, fd) = (d.width as u16, d.depth as u16);
        let fx = lx + (lw - fw) / 2;
        let fy = if lot.north {
            ly + if lh > 6 { 1 } else { 0 }
        } else {
            ly + lh - fd
        };
        let road_y = if lot.north {
            self.line(lot.block.1)
        } else {
            self.line(lot.block.1 + 1)
        };
        let door = (fx + fw / 2, road_y);
        let site_def = c.construction.site;
        let b = self
            .buildings
            .push(site_def, fx, fy, d.width, d.depth, lot.rect, door);
        // Access: nearest end of the frontage street.
        let (x0, x1) = (self.line(lot.block.0), self.line(lot.block.0 + 1));
        let na = self.node_at(x0, road_y).unwrap_or(0);
        let nbn = self.node_at(x1, road_y).unwrap_or(na);
        let (da, db) = (
            (door.0 as i32 - x0 as i32).unsigned_abs(),
            (door.0 as i32 - x1 as i32).unsigned_abs(),
        );
        let (node, tiles) = if da <= db { (na, da) } else { (nbn, db) };
        self.buildings.access_node[b as usize] = node;
        self.buildings.access_tiles[b as usize] = tiles as u16;
        self.buildings.street[b as usize] = if lot.north {
            lot.block.1 as u16
        } else {
            lot.block.1 as u16 + 1
        };
        self.buildings.present.push(0);
        self.systems.economy.firm_of.push(NONE);
        for y in ly..ly + lh {
            for x in lx..lx + lw {
                let i = self.map.idx(x, y);
                self.map.kind[i] = TileKind::Site as u8;
                self.map.building[i] = b;
            }
        }
        self.buildings.rebuild_index(c.tags.len());
        let builder = self.pick_builder();
        let id = self.systems.construction.next_id;
        self.systems.construction.next_id += 1;
        self.systems.construction.projects.push(Project {
            id,
            work: Work::Building { site: b, def },
            purpose,
            funder,
            builder,
            phase: Phase::Planned,
            next: Phase::Foundation,
            units,
            done: 0,
            started: t,
            finished: 0,
        });
        if let Funder::Agent(a) = funder {
            self.journal(
                a,
                t,
                JournalKind::Invested,
                def,
                cost.clamp(0, u32::MAX as i64) as u32,
            );
        }
        self.systems.construction.today.started += 1;
        self.systems.construction.static_version += 1;
        self.push_news(NewsItem {
            tick: t,
            kind: NewsKind::WorksStart as u16,
            subject: def as u32,
            n: 0,
        });
        true
    }

    /// Lays out streets around a new block next to the town.
    fn plan_expansion(&mut self, t: Tick) -> bool {
        let nb = self.systems.construction.nb as i32;
        let cb = nb / 2;
        let mut best: Option<(i64, (i32, i32))> = None;
        for by in 1..nb - 1 {
            for bx in 1..nb - 1 {
                if self.block_developed(bx, by) {
                    continue;
                }
                let neighbours = [(bx - 1, by), (bx + 1, by), (bx, by - 1), (bx, by + 1)]
                    .iter()
                    .filter(|(x, y)| self.block_developed(*x, *y))
                    .count();
                if neighbours == 0 {
                    continue;
                }
                // Only open blocks of empty land (no water, fields or farms).
                let (x0, y0) = (self.line(bx) + 1, self.line(by) + 1);
                if !self.rect_free([x0, y0, PITCH - 1, PITCH - 1]) {
                    continue;
                }
                let dist = (bx - cb).abs().max((by - cb).abs()) as i64;
                let noise = (keyed(
                    self.seed,
                    bx as u64,
                    by as u64,
                    t as u64 / TICKS_PER_DAY as u64,
                ) % 3) as i64;
                let score = dist * 10 - neighbours as i64 * 4 + noise;
                if best.is_none_or(|(s, _)| score < s) {
                    best = Some((score, (bx, by)));
                }
            }
        }
        let Some((_, (bx, by))) = best else {
            return false;
        };
        let edges = self.block_edges(bx, by);
        let k = &content().construction;
        let mut planned = false;
        for (a, b) in edges {
            if self.edge_exists(a, b) {
                continue;
            }
            let units_total = PITCH as u32 * k.road_per_tile;
            let cost =
                self.project_cost([units_total * 6 / 10, units_total - units_total * 6 / 10, 0]);
            let Some(funder) = self.pick_funder(Purpose::Street, cost, t) else {
                return planned;
            };
            let na = match self.node_at(a.0, a.1) {
                Some(n) => n,
                None => self.roads.add_node(a.0, a.1),
            };
            let nbn = match self.node_at(b.0, b.1) {
                Some(n) => n,
                None => self.roads.add_node(b.0, b.1),
            };
            self.roads.finalize();
            self.paint_road(a, b, TileKind::RoadWorks);
            let builder = self.pick_builder();
            let id = self.systems.construction.next_id;
            self.systems.construction.next_id += 1;
            self.systems.construction.projects.push(Project {
                id,
                work: Work::Road { a: na, b: nbn },
                purpose: Purpose::Street,
                funder,
                builder,
                phase: Phase::Planned,
                next: Phase::Foundation,
                units: [units_total * 6 / 10, units_total - units_total * 6 / 10, 0],
                done: 0,
                started: t,
                finished: 0,
            });
            self.systems.construction.today.started += 1;
            planned = true;
        }
        if planned {
            self.systems.construction.static_version += 1;
        }
        planned
    }

    fn paint_road(&mut self, a: (u16, u16), b: (u16, u16), kind: TileKind) {
        for y in a.1.min(b.1)..=a.1.max(b.1) {
            for x in a.0.min(b.0)..=a.0.max(b.0) {
                let cur = self.map.get(x, y);
                if matches!(
                    cur,
                    TileKind::Grass | TileKind::RoadWorks | TileKind::Field | TileKind::Park
                ) {
                    self.map.set(x, y, kind);
                } else if cur == TileKind::Water {
                    self.map.set(x, y, TileKind::Bridge);
                }
            }
        }
    }

    // ------------------------------------------------------------------ progress

    /// Construction firm `firm`'s projects that its builders can work on now.
    fn workable(&self, firm: u32) -> Vec<usize> {
        self.systems
            .construction
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.builder == firm && p.active())
            .map(|(i, _)| i)
            .collect()
    }

    /// The project builder `a` of firm `f` works on: crews of `crew_size` builders, by
    /// seniority among the company's builders (lowest agent id first).
    fn crew_project(&self, a: AgentId, f: u32) -> Option<usize> {
        let i = a as usize;
        if self.agents.profession[i] != catalog().profession_builder {
            return None;
        }
        let list = self.workable(f);
        if list.is_empty() {
            return None;
        }
        let work = self.agents.work[i];
        let rank = (0..i)
            .filter(|&j| {
                self.agents.work[j] == work
                    && self.agents.profession[j] == self.agents.profession[i]
            })
            .count();
        list.get(rank / content().construction.crew_size as usize)
            .copied()
    }

    /// The site a builder should go to today (building projects only), if any.
    pub(crate) fn builder_site(&self, a: AgentId) -> Option<BuildingId> {
        let work = self.agents.work[a as usize];
        let f = self.firm_at(work)?;
        if self.systems.economy.firms[f as usize].kind != FirmKind::Construction {
            return None;
        }
        let p = &self.systems.construction.projects[self.crew_project(a, f)?];
        match p.work {
            Work::Building { site, .. } => Some(site),
            Work::Road { .. } => None,
        }
    }

    /// A builder finished a shift: one unit of work on their assigned project.
    pub(crate) fn construction_shift_done(&mut self, a: AgentId, firm: u32, t: Tick) {
        let Some(idx) = self.crew_project(a, firm) else {
            return;
        };
        if let Work::Building { site, .. } = self.systems.construction.projects[idx].work {
            if self.agents.place[a as usize] == site || self.agents.place[a as usize] == NONE {
                self.journal(a, t, JournalKind::WorkedOnSite, 0, site);
            }
        }
        self.add_work(idx, 1, t);
    }

    fn add_work(&mut self, idx: usize, units: u32, t: Tick) {
        self.systems.construction.today.units += units;
        let p = &mut self.systems.construction.projects[idx];
        if !p.active() {
            return;
        }
        p.done += units;
        let need = p.units[Project::phase_index(p.phase)];
        if p.done < need {
            return;
        }
        // Phases without work (parks have no structure or roof) are skipped.
        let next = match p.phase {
            Phase::Foundation if p.units[1] > 0 => Phase::Structure,
            Phase::Foundation | Phase::Structure if p.units[2] > 0 => Phase::Roof,
            _ => Phase::Finished,
        };
        p.done = 0;
        if next == Phase::Finished {
            self.finish_project(idx, t);
        } else {
            // Wait for the funder to pay the next phase (checked daily).
            p.phase = Phase::Planned;
            p.next = next;
        }
    }

    /// Pays and starts phases that are waiting for money.
    fn fund_phases(&mut self, _t: Tick) {
        let k = &content().construction;
        for idx in 0..self.systems.construction.projects.len() {
            let p = self.systems.construction.projects[idx].clone();
            if p.phase != Phase::Planned {
                continue;
            }
            let units = p.units[Project::phase_index(p.next)];
            let labour = units as i64 * k.labour_per_unit_cents;
            let materials = units as i64 * k.materials_per_unit_cents;
            if self.funder_money(p.funder) < labour + materials {
                continue;
            }
            let from = Region::funder_account(p.funder);
            let builder_open = p.builder != NONE
                && (p.builder as usize) < self.systems.economy.firms.len()
                && self.systems.economy.firms[p.builder as usize].open;
            if builder_open {
                self.transfer(from, Account::Firm(p.builder), labour);
                self.systems.economy.firms[p.builder as usize].revenue_today += labour;
            } else {
                self.transfer(from, Account::World, labour);
            }
            self.transfer(from, Account::World, materials);
            self.systems.economy.today.imports += materials;
            let q = &mut self.systems.construction.projects[idx];
            q.phase = q.next;
            if !builder_open {
                q.builder = NONE;
            }
        }
    }

    fn finish_project(&mut self, idx: usize, t: Tick) {
        let c = content();
        let p = self.systems.construction.projects[idx].clone();
        self.systems.construction.projects[idx].phase = Phase::Finished;
        self.systems.construction.projects[idx].finished = t;
        self.systems.construction.today.finished += 1;
        self.systems.construction.static_version += 1;
        match p.work {
            Work::Road { a, b } => {
                let (ax, ay, bx, by) = (
                    self.roads.node_x[a as usize],
                    self.roads.node_y[a as usize],
                    self.roads.node_x[b as usize],
                    self.roads.node_y[b as usize],
                );
                let len =
                    (ax as i32 - bx as i32).unsigned_abs() + (ay as i32 - by as i32).unsigned_abs();
                if self.roads.find_edge(a, b).is_none() {
                    self.roads.add_edge(a, b, len as u16, RoadClass::Street);
                    self.roads.finalize();
                }
                self.paint_road((ax, ay), (bx, by), TileKind::Street);
                self.systems.construction.today.streets += 1;
                if self.news.last().is_none_or(|n| {
                    n.kind != NewsKind::NewStreet as u16 || n.tick + TICKS_PER_DAY <= t
                }) {
                    let id = self.id as u32;
                    self.push_news(NewsItem {
                        tick: t,
                        kind: NewsKind::NewStreet as u16,
                        subject: id,
                        n: 0,
                    });
                }
            }
            Work::Building { site, def } => {
                let d = &c.buildings[def as usize];
                self.buildings.def[site as usize] = def;
                let [lx, ly, lw, lh] = self.buildings.lot[site as usize];
                let (fx, fy, fw, fd) = (
                    self.buildings.x[site as usize],
                    self.buildings.y[site as usize],
                    d.width as u16,
                    d.depth as u16,
                );
                let park = d.roof == livingsim_content::RoofKind::None;
                let farm = catalog().tags.farm.is_some_and(|tg| d.tags.contains(&tg));
                for y in ly..ly + lh {
                    for x in lx..lx + lw {
                        let i = self.map.idx(x, y);
                        let in_fp = x >= fx && x < fx + fw && y >= fy && y < fy + fd;
                        let kind = if park {
                            if farm {
                                TileKind::Field
                            } else {
                                TileKind::Park
                            }
                        } else if farm && !in_fp {
                            TileKind::Field
                        } else {
                            TileKind::Lot
                        };
                        self.map.kind[i] = kind as u8;
                        self.map.building[i] = if in_fp || park { site } else { NONE };
                    }
                }
                let mut rng = Rng::new(self.seed, keyed(site as u64, t as u64, 0xB1, 0));
                let style = self.style;
                crate::gen::name_building(self, &mut rng, site, style);
                self.buildings.rebuild_index(c.tags.len());
                // Owner of the new homes.
                if d.dwellings > 0 {
                    let owner = match p.funder {
                        Funder::Treasury => HomeOwner::Municipal,
                        Funder::Firm(f)
                            if self.systems.economy.firms[f as usize].kind
                                == FirmKind::RealEstate =>
                        {
                            HomeOwner::Agency(f)
                        }
                        Funder::Firm(f) => {
                            let o = self.systems.economy.firms[f as usize].owner;
                            if o != NONE {
                                HomeOwner::Landlord(o)
                            } else {
                                HomeOwner::Municipal
                            }
                        }
                        Funder::Agent(a) => HomeOwner::Landlord(a),
                    };
                    self.systems.construction.owners.insert(site, owner);
                }
                // A business or public service starts operating.
                if let Some(spec) = d.firm.clone() {
                    let owner = match p.funder {
                        Funder::Agent(a) => a,
                        Funder::Firm(f) => self.systems.economy.firms[f as usize].owner,
                        Funder::Treasury => NONE,
                    };
                    let econ = &c.economy;
                    let price = match spec.kind {
                        FirmKind::Shop => econ.meal_price_cents,
                        FirmKind::Farm => econ.farm_price_cents,
                        _ => super::economy::BASE_FACTOR,
                    };
                    let positions: u16 = d.jobs.iter().map(|j| j.count).sum();
                    let f = self.systems.economy.firms.len() as u32;
                    self.systems.economy.firms.push(Firm {
                        building: site,
                        kind: spec.kind,
                        good: spec.good.unwrap_or(NO_GOOD),
                        cash: 0,
                        stock: d.stock / 2,
                        price,
                        owner,
                        open: true,
                        closed_since: 0,
                        days_in_red: 0,
                        generation: 0,
                        revenue_today: 0,
                        costs_today: 0,
                        customers_today: 0,
                        revenue_yesterday: 0,
                        costs_yesterday: 0,
                        customers_yesterday: 0,
                        unit_cost: econ.farm_price_cents,
                        positions,
                        sold_today: 0,
                        sold_yesterday: 0,
                        profit_avg: 0,
                    });
                    self.systems.economy.firm_of[site as usize] = f;
                    // Working capital from the investor (about two weeks of wages).
                    let payroll: i64 = d
                        .jobs
                        .iter()
                        .map(|j| {
                            j.count as i64
                                * c.professions[j.profession as usize].wage_per_shift_cents
                        })
                        .sum();
                    let capital = (payroll * 14).min(self.funder_money(p.funder).max(0));
                    if spec.kind != FirmKind::Public && capital > 0 {
                        self.transfer(Region::funder_account(p.funder), Account::Firm(f), capital);
                    }
                } else if !d.jobs.is_empty() {
                    let f = self.systems.economy.firms.len() as u32;
                    self.systems.economy.firms.push(Firm {
                        building: site,
                        kind: FirmKind::Public,
                        good: NO_GOOD,
                        cash: 0,
                        stock: 0,
                        price: super::economy::BASE_FACTOR,
                        owner: NONE,
                        open: true,
                        closed_since: 0,
                        days_in_red: 0,
                        generation: 0,
                        revenue_today: 0,
                        costs_today: 0,
                        customers_today: 0,
                        revenue_yesterday: 0,
                        costs_yesterday: 0,
                        customers_yesterday: 0,
                        unit_cost: 0,
                        positions: d.jobs.iter().map(|j| j.count).sum(),
                        sold_today: 0,
                        sold_yesterday: 0,
                        profit_avg: 0,
                    });
                    self.systems.economy.firm_of[site as usize] = f;
                }
                let teaches = ["daycare", "school"]
                    .iter()
                    .any(|tag| c.tag(tag).is_some_and(|tg| d.tags.contains(&tg)));
                if teaches {
                    self.enrol_waiting_children(t);
                }
                let days = (t.saturating_sub(p.started) / TICKS_PER_DAY).max(1);
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Inauguration as u16,
                    subject: site,
                    n: days,
                });
            }
        }
    }

    // ------------------------------------------------------------------ daily

    pub(crate) fn construction_daily(&mut self, t: Tick) {
        self.bootstrap_construction();
        {
            let c = &mut self.systems.construction;
            c.yesterday = std::mem::take(&mut c.today);
            let y = c.yesterday.clone();
            c.total.started += y.started;
            c.total.finished += y.finished;
            c.total.streets += y.streets;
            c.total.units += y.units;
            // Forget finished projects after a while (keep the list short).
            c.projects
                .retain(|p| p.phase != Phase::Finished || p.finished + 60 * TICKS_PER_DAY > t);
        }
        // Outside contractors work where there is no construction company.
        let k = &content().construction;
        for idx in 0..self.systems.construction.projects.len() {
            let p = &self.systems.construction.projects[idx];
            if p.active() && p.builder == NONE {
                self.add_work(idx, k.contractor_units_per_day, t);
            }
        }
        // Projects of a construction company that closed go to outside contractors.
        let builders = self.builders();
        for p in self.systems.construction.projects.iter_mut() {
            if p.builder != NONE && !builders.contains(&p.builder) && p.phase != Phase::Finished {
                p.builder = NONE;
            }
        }
        self.fund_phases(t);
        if date(t).weekday == 0 && t >= TICKS_PER_DAY {
            self.plan(t);
            self.fund_phases(t);
        }
    }

    /// Who owns the homes of building `b`, if it was built in Phase 4.
    pub fn home_owner(&self, b: BuildingId) -> Option<HomeOwner> {
        self.systems.construction.owners.get(&b).copied()
    }

    /// Projects not finished yet, for the UI.
    pub fn open_projects(&self) -> Vec<&Project> {
        self.systems
            .construction
            .projects
            .iter()
            .filter(|p| p.phase != Phase::Finished)
            .collect()
    }

    /// Agents' work units are counted per builder firm (for the UI).
    pub fn project_label(&self, p: &Project) -> String {
        match p.work {
            Work::Building { def, .. } => content().buildings[def as usize].name.clone(),
            Work::Road { .. } => "New street".into(),
        }
    }
}

/// Helper used by tests: total work units of a building definition.
pub fn units_for(def: u16) -> [u32; 3] {
    Region::building_units(def)
}
