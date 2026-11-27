//! Frozen layouts of snapshot formats v1 (Phase 1) and v2 (Phase 2), kept forever so old snapshots can be
//! migrated. Only the types that changed in v2 are copied here; everything else reuses
//! the current types, which have the same layout.
//!
//! v1 → v2: `systems.economy` changed from a stub to the real economy. Migration decodes
//! the v1 region, carries every field over and runs `bootstrap_economy` (deterministic in
//! the region seed), so the people, homes and jobs of a running world are kept.

use crate::agents::{AgentId, Agents};
use crate::buildings::{Buildings, Households};
use crate::map::TileMap;
use crate::messages::Message;
use crate::names::{NamePool, NameRef};
use crate::region::{AwayInfo, DailyCounters, Gateway, NewsItem, Region, VisitorInfo};
use crate::roads::RoadGraph;
use crate::systems::{construction, crime, economy, expansion, justice, technology, Systems};
use crate::time::Tick;
use crate::world::{IntercityGraph, World};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Firm {
    pub building: u32,
    pub cash_cents: i64,
    pub revenue_today_cents: i64,
    pub payroll_today_cents: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EconomyState {
    pub treasury_cents: i64,
    pub tax_permille: u16,
    pub firms: Vec<Firm>,
}

/// The Phase 1–2 demography stub (frozen).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DemographyStateV1 {
    pub births: u32,
    pub deaths: u32,
    pub marriages: u32,
    pub pregnancies: Vec<(u32, Tick)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemsV1 {
    pub economy: EconomyState,
    pub demography: DemographyStateV1,
    pub construction: construction::ConstructionState,
    pub crime: crime::CrimeState,
    pub justice: justice::JusticeState,
    pub expansion: expansion::ExpansionState,
    pub technology: technology::TechnologyState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionV1 {
    pub id: u16,
    pub seed: u64,
    pub template: u16,
    pub style: u16,
    pub name: NameRef,
    pub origin: (i32, i32),
    pub tick: Tick,
    pub map: TileMap,
    pub roads: RoadGraph,
    pub buildings: Buildings,
    pub households: Households,
    pub agents: Agents,
    pub names: NamePool,
    pub streets: Vec<NameRef>,
    pub gateways: Vec<Gateway>,
    pub visitors: BTreeMap<AgentId, VisitorInfo>,
    pub away: BTreeMap<AgentId, AwayInfo>,
    pub today: DailyCounters,
    pub yesterday: DailyCounters,
    pub news: Vec<NewsItem>,
    pub outbox: Vec<Message>,
    pub out_seq: u32,
    pub systems: SystemsV1,
    pub events: u64,
}

/// Format v2 (Phase 2): the real economy, and still the demography stub.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemsV2 {
    pub economy: economy::EconomyState,
    pub demography: DemographyStateV1,
    pub construction: construction::ConstructionState,
    pub crime: crime::CrimeState,
    pub justice: justice::JusticeState,
    pub expansion: expansion::ExpansionState,
    pub technology: technology::TechnologyState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionV2 {
    pub id: u16,
    pub seed: u64,
    pub template: u16,
    pub style: u16,
    pub name: NameRef,
    pub origin: (i32, i32),
    pub tick: Tick,
    pub map: TileMap,
    pub roads: RoadGraph,
    pub buildings: Buildings,
    pub households: Households,
    pub agents: Agents,
    pub names: NamePool,
    pub streets: Vec<NameRef>,
    pub gateways: Vec<Gateway>,
    pub visitors: BTreeMap<AgentId, VisitorInfo>,
    pub away: BTreeMap<AgentId, AwayInfo>,
    pub today: DailyCounters,
    pub yesterday: DailyCounters,
    pub news: Vec<NewsItem>,
    pub outbox: Vec<Message>,
    pub out_seq: u32,
    pub systems: SystemsV2,
    pub events: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldV2 {
    pub seed: u64,
    pub tick: Tick,
    pub regions: Vec<RegionV2>,
    pub intercity: IntercityGraph,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldV1 {
    pub seed: u64,
    pub tick: Tick,
    pub regions: Vec<RegionV1>,
    pub intercity: IntercityGraph,
}

impl RegionV1 {
    pub fn upgrade(self) -> Region {
        let mut r = Region::empty(
            self.id,
            self.seed,
            self.template,
            self.style,
            self.map,
            self.tick,
        );
        r.name = self.name;
        r.origin = self.origin;
        r.roads = self.roads;
        r.buildings = self.buildings;
        r.households = self.households;
        r.agents = self.agents;
        r.names = self.names;
        r.streets = self.streets;
        r.gateways = self.gateways;
        r.visitors = self.visitors;
        r.away = self.away;
        r.today = self.today;
        r.yesterday = self.yesterday;
        r.news = self.news;
        r.outbox = self.outbox;
        r.out_seq = self.out_seq;
        r.events = self.events;
        let s = self.systems;
        r.systems = Systems {
            economy: Default::default(),
            demography: Default::default(),
            construction: s.construction,
            crime: s.crime,
            justice: s.justice,
            expansion: s.expansion,
            technology: s.technology,
        };
        r.prepare();
        r.bootstrap_economy();
        r.bootstrap_demography();
        r
    }
}

impl RegionV2 {
    pub fn upgrade(self) -> Region {
        let mut r = Region::empty(
            self.id,
            self.seed,
            self.template,
            self.style,
            self.map,
            self.tick,
        );
        r.name = self.name;
        r.origin = self.origin;
        r.roads = self.roads;
        r.buildings = self.buildings;
        r.households = self.households;
        r.agents = self.agents;
        r.names = self.names;
        r.streets = self.streets;
        r.gateways = self.gateways;
        r.visitors = self.visitors;
        r.away = self.away;
        r.today = self.today;
        r.yesterday = self.yesterday;
        r.news = self.news;
        r.outbox = self.outbox;
        r.out_seq = self.out_seq;
        r.events = self.events;
        let s = self.systems;
        r.systems = Systems {
            economy: s.economy,
            demography: Default::default(),
            construction: s.construction,
            crime: s.crime,
            justice: s.justice,
            expansion: s.expansion,
            technology: s.technology,
        };
        r.prepare();
        r.bootstrap_demography();
        r
    }
}

impl WorldV2 {
    pub fn upgrade(self) -> World {
        World {
            seed: self.seed,
            tick: self.tick,
            regions: self.regions.into_iter().map(RegionV2::upgrade).collect(),
            intercity: self.intercity,
        }
    }
}

impl WorldV1 {
    pub fn upgrade(self) -> World {
        World {
            seed: self.seed,
            tick: self.tick,
            regions: self.regions.into_iter().map(RegionV1::upgrade).collect(),
            intercity: self.intercity,
        }
    }
}
