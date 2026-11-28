//! Snapshot format 5 (Phase 4), kept to migrate old snapshots.
//!
//! [`RegionV5`] lists the serialized fields of a region in order, generic over the parts
//! that changed between formats, so every old layout is described by the same struct with
//! frozen types plugged in:
//! * format 4: `RegionV5<ConstructionV4, CrimeV5, JusticeV5, ContentIdsV5, ExpansionV6>`
//! * format 5: `RegionV5<ConstructionState, CrimeV5, JusticeV5, ContentIdsV5, ExpansionV6>`
//! * format 6: `RegionV5<ConstructionState, CrimeState, JusticeState, ContentIds, ExpansionV6>`
//! * format 7: `RegionV5<…, ContentIds, ExpansionState>` (= `Region`)
//!
//! Fields not listed as generic are decoded with today's types; if one of them changes,
//! freeze it here before changing it.

use crate::agents::{AgentId, Agents};
use crate::buildings::{Buildings, Households};
use crate::content_ids::ContentIds;
use crate::map::TileMap;
use crate::messages::Message;
use crate::names::{NamePool, NameRef};
use crate::region::{AwayInfo, DailyCounters, Gateway, NewsItem, VisitorInfo};
use crate::roads::RoadGraph;
use crate::snapshot::{from_bytes, to_bytes, Kind, SnapshotError};
use crate::systems::{construction, crime, demography, economy, justice, technology};
use crate::time::Tick;
use crate::world::IntercityGraph;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---- frozen Phase 1–4 stubs

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IncidentV5 {
    pub crime: u16,
    pub tick: Tick,
    pub building: u32,
    pub offender: u32,
    pub victim: u32,
    pub solved: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CrimeV5 {
    pub incidents: Vec<IncidentV5>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseV5 {
    pub incident: u32,
    pub evidence: u16,
    pub suspect: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InmateV5 {
    pub agent: u32,
    pub release: Tick,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JusticeV5 {
    pub open_cases: Vec<CaseV5>,
    pub inmates: Vec<InmateV5>,
}

/// Phases 1–5 expansion stub (frozen in format 7).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExpansionV6 {
    pub free_lots: u32,
    pub pressure_permille: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentIdsV5 {
    pub buildings: Vec<String>,
    pub professions: Vec<String>,
    pub schedules: Vec<String>,
    pub activities: Vec<String>,
    pub goods: Vec<String>,
}

// ---- generic layout

#[derive(Serialize, Deserialize)]
pub struct SystemsV5<Co, Cr, Ju, Ex> {
    pub economy: economy::EconomyState,
    pub demography: demography::DemographyState,
    pub construction: Co,
    pub crime: Cr,
    pub justice: Ju,
    pub expansion: Ex,
    pub technology: technology::TechnologyState,
}

#[derive(Serialize, Deserialize)]
pub struct RegionV5<Co, Cr, Ju, Ids, Ex> {
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
    pub systems: SystemsV5<Co, Cr, Ju, Ex>,
    pub events: u64,
    pub content_ids: Ids,
}

#[derive(Serialize, Deserialize)]
pub struct WorldV5<R> {
    pub seed: u64,
    pub tick: Tick,
    pub regions: Vec<R>,
    pub intercity: IntercityGraph,
}

impl<Co, Cr, Ju, Ids, Ex> RegionV5<Co, Cr, Ju, Ids, Ex> {
    /// Rebuilds the region with new construction, crime, justice, content-id and expansion
    /// parts.
    #[allow(clippy::too_many_arguments)]
    pub fn map_parts<Co2, Cr2, Ju2, Ids2, Ex2>(
        self,
        co: impl FnOnce(Co) -> Co2,
        cr: impl FnOnce(Cr) -> Cr2,
        ju: impl FnOnce(Ju) -> Ju2,
        ids: impl FnOnce(Ids) -> Ids2,
        ex: impl FnOnce(Ex) -> Ex2,
    ) -> RegionV5<Co2, Cr2, Ju2, Ids2, Ex2> {
        let s = self.systems;
        RegionV5 {
            id: self.id,
            seed: self.seed,
            template: self.template,
            style: self.style,
            name: self.name,
            origin: self.origin,
            tick: self.tick,
            map: self.map,
            roads: self.roads,
            buildings: self.buildings,
            households: self.households,
            agents: self.agents,
            names: self.names,
            streets: self.streets,
            gateways: self.gateways,
            visitors: self.visitors,
            away: self.away,
            today: self.today,
            yesterday: self.yesterday,
            news: self.news,
            outbox: self.outbox,
            out_seq: self.out_seq,
            systems: SystemsV5 {
                economy: s.economy,
                demography: s.demography,
                construction: co(s.construction),
                crime: cr(s.crime),
                justice: ju(s.justice),
                expansion: ex(s.expansion),
                technology: s.technology,
            },
            events: self.events,
            content_ids: ids(self.content_ids),
        }
    }
}

type V5 = RegionV5<construction::ConstructionState, CrimeV5, JusticeV5, ContentIdsV5, ExpansionV6>;
/// Format 6 (Phase 5).
pub type V6 = RegionV5<
    construction::ConstructionState,
    crime::CrimeState,
    justice::JusticeState,
    ContentIds,
    ExpansionV6,
>;

fn upgrade(r: V5) -> V6 {
    // The crime and justice stubs never held data. Crime ids did not exist yet; no state
    // referred to them.
    r.map_parts(
        |c| c,
        |_| crime::CrimeState::default(),
        |_| justice::JusticeState::default(),
        |ids| ContentIds {
            buildings: ids.buildings,
            professions: ids.professions,
            schedules: ids.schedules,
            activities: ids.activities,
            goods: ids.goods,
            crimes: ContentIds::current().crimes,
        },
        |x| x,
    )
}

/// Format 5 → 6.
pub fn to_next(kind: Kind, payload: Vec<u8>) -> Result<Vec<u8>, SnapshotError> {
    match kind {
        Kind::Region => {
            let r: V5 = from_bytes(&payload)?;
            Ok(to_bytes(&upgrade(r)))
        }
        Kind::World => {
            let w: WorldV5<V5> = from_bytes(&payload)?;
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
