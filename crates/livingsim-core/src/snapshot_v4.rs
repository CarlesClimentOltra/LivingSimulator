//! Snapshot format 4 (Phases 1–3 with content ids), kept to migrate old snapshots.
//!
//! Only the part that changed in format 5 is frozen here: the construction state was a
//! stub. Every other field is decoded with today's types and re-encoded unchanged; if one
//! of them changes later, freeze it here too before changing it.

use crate::agents::{AgentId, Agents};
use crate::buildings::{Buildings, Households};
use crate::map::TileMap;
use crate::messages::Message;
use crate::names::{NamePool, NameRef};
use crate::region::{AwayInfo, DailyCounters, Gateway, NewsItem, VisitorInfo};
use crate::roads::RoadGraph;
use crate::snapshot::{from_bytes, to_bytes, Kind, SnapshotError};
use crate::systems::{construction, crime, demography, economy, expansion, justice, technology};
use crate::time::Tick;
use crate::world::IntercityGraph;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Serialize, Deserialize)]
pub struct SystemsV4<C> {
    pub economy: economy::EconomyState,
    pub demography: demography::DemographyState,
    pub construction: C,
    pub crime: crime::CrimeState,
    pub justice: justice::JusticeState,
    pub expansion: expansion::ExpansionState,
    pub technology: technology::TechnologyState,
}

/// The serialized fields of a region, generic over the construction state.
#[derive(Serialize, Deserialize)]
pub struct RegionV4<C> {
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
    pub systems: SystemsV4<C>,
    pub events: u64,
    pub content_ids: crate::content_ids::ContentIds,
}

#[derive(Serialize, Deserialize)]
pub struct WorldV4<C> {
    pub seed: u64,
    pub tick: Tick,
    pub regions: Vec<RegionV4<C>>,
    pub intercity: IntercityGraph,
}

fn upgrade(r: RegionV4<ConstructionV4>) -> RegionV4<construction::ConstructionState> {
    // The stub never held projects. The growth ring is added on the first morning
    // (`bootstrap_construction`), deterministically for the runner and the browser.
    let s = r.systems;
    RegionV4 {
        id: r.id,
        seed: r.seed,
        template: r.template,
        style: r.style,
        name: r.name,
        origin: r.origin,
        tick: r.tick,
        map: r.map,
        roads: r.roads,
        buildings: r.buildings,
        households: r.households,
        agents: r.agents,
        names: r.names,
        streets: r.streets,
        gateways: r.gateways,
        visitors: r.visitors,
        away: r.away,
        today: r.today,
        yesterday: r.yesterday,
        news: r.news,
        outbox: r.outbox,
        out_seq: r.out_seq,
        systems: SystemsV4 {
            economy: s.economy,
            demography: s.demography,
            construction: construction::ConstructionState::default(),
            crime: s.crime,
            justice: s.justice,
            expansion: s.expansion,
            technology: s.technology,
        },
        events: r.events,
        content_ids: r.content_ids,
    }
}

/// Format 4 → 5.
pub fn to_next(kind: Kind, payload: Vec<u8>) -> Result<Vec<u8>, SnapshotError> {
    match kind {
        Kind::Region => {
            let r: RegionV4<ConstructionV4> = from_bytes(&payload)?;
            Ok(to_bytes(&upgrade(r)))
        }
        Kind::World => {
            let w: WorldV4<ConstructionV4> = from_bytes(&payload)?;
            Ok(to_bytes(&WorldV4 {
                seed: w.seed,
                tick: w.tick,
                regions: w.regions.into_iter().map(upgrade).collect(),
                intercity: w.intercity,
            }))
        }
        Kind::MessageLog => Ok(payload),
    }
}
