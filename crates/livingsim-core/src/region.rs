//! A region (one town or city): the unit of simulation, parallelism, persistence and
//! download. A region never reads another region's state; it only exchanges messages.

use crate::agents::{flags, AgentId, Agents, ACT_AWAY, ACT_FREE_SLOT, ACT_TRAVEL};
use crate::buildings::{BuildingId, Buildings, Households};
use crate::catalog::{catalog, content};
use crate::map::TileMap;
use crate::messages::{Body, Message, WINDOW};
use crate::names::{NamePool, NameRef};
use crate::queue::EventQueue;
use crate::roads::{Mode, RoadGraph, RouteCache, NONE};
use crate::systems::{DailyContext, Systems};
use crate::time::{Tick, TICKS_PER_DAY};
use livingsim_content::{JournalKind, NewsKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A highway exit linking this region to a neighbour.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gateway {
    pub building: BuildingId,
    pub node: u32,
    pub to_region: u16,
    pub to_name: String,
    /// Driving time along the highway to the other region's exit.
    pub highway_ticks: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisitorInfo {
    pub origin_region: u16,
    pub origin_agent: u32,
    pub arrival: Tick,
    pub leave_by: Tick,
    pub start_money: i64,
    pub places: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AwayInfo {
    pub region: u16,
    pub gateway: u16,
    pub return_at: Tick,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub tick: Tick,
    pub kind: u16,
    /// Building or region, depending on `kind`.
    pub subject: u32,
    pub n: u32,
}

pub const NEWS_KEPT: usize = 40;

/// Counters reset every morning.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DailyCounters {
    pub trips: u32,
    pub car_trips: u32,
    pub meals_out: u32,
    pub groceries: u32,
    pub shelves_empty: u32,
    pub wages_paid_cents: i64,
    pub visitors_in: BTreeMap<u16, u32>,
    pub food_in: BTreeMap<u16, u32>,
    pub decisions: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Region {
    pub id: u16,
    pub seed: u64,
    pub template: u16,
    pub style: u16,
    pub name: NameRef,
    /// Position of tile (0,0) in world tile coordinates.
    pub origin: (i32, i32),
    /// The next tick to simulate.
    pub tick: Tick,
    pub map: TileMap,
    pub roads: RoadGraph,
    pub buildings: Buildings,
    pub households: Households,
    pub agents: Agents,
    pub names: NamePool,
    /// Street names: horizontal lines then vertical lines.
    pub streets: Vec<NameRef>,
    pub gateways: Vec<Gateway>,
    pub visitors: BTreeMap<AgentId, VisitorInfo>,
    pub away: BTreeMap<AgentId, AwayInfo>,
    pub today: DailyCounters,
    pub yesterday: DailyCounters,
    pub news: Vec<NewsItem>,
    pub outbox: Vec<Message>,
    pub out_seq: u32,
    pub systems: Systems,
    /// Total events processed (statistics only).
    pub events: u64,
    /// Names behind the content indices stored in this region (see `content_ids`).
    pub content_ids: crate::content_ids::ContentIds,
    // ---- derived, not serialized
    #[serde(skip)]
    pub(crate) queue: EventQueue,
    #[serde(skip)]
    pub routes: RouteCache,
    #[serde(skip)]
    due: Vec<u32>,
    #[serde(skip)]
    ready: bool,
}

impl Region {
    pub(crate) fn empty(
        id: u16,
        seed: u64,
        template: u16,
        style: u16,
        map: TileMap,
        start: Tick,
    ) -> Region {
        Region {
            id,
            seed,
            template,
            style,
            name: crate::names::NO_NAME,
            origin: (0, 0),
            tick: start,
            map,
            roads: RoadGraph::default(),
            buildings: Buildings::default(),
            households: Households::default(),
            agents: Agents::default(),
            names: NamePool::default(),
            streets: Vec::new(),
            gateways: Vec::new(),
            visitors: BTreeMap::new(),
            away: BTreeMap::new(),
            today: DailyCounters::default(),
            yesterday: DailyCounters::default(),
            news: Vec::new(),
            outbox: Vec::new(),
            out_seq: 0,
            systems: Systems::default(),
            events: 0,
            content_ids: crate::content_ids::ContentIds::current(),
            queue: EventQueue::new(start),
            routes: RouteCache::default(),
            due: Vec::new(),
            ready: false,
        }
    }

    pub fn name(&self) -> &str {
        self.names.get(self.name)
    }

    /// Rebuilds derived state (indices, event queue). Called automatically.
    pub fn prepare(&mut self) {
        if self.ready {
            return;
        }
        self.roads.finalize();
        self.buildings.rebuild_index(content().tags.len());
        self.buildings.present = vec![0; self.buildings.len()];
        for a in 0..self.agents.len() {
            let p = self.agents.place[a];
            if p != NONE && self.agents.alive(a as u32) {
                self.buildings.present[p as usize] =
                    self.buildings.present[p as usize].saturating_add(1);
            }
        }
        self.queue = EventQueue::new(self.tick);
        for a in 0..self.agents.len() {
            let t = self.agents.next_event[a];
            if t != Tick::MAX && self.agents.alive(a as u32) {
                self.queue.schedule(a as u32, t);
            }
        }
        self.ready = true;
    }

    pub fn population(&self) -> u32 {
        self.agents.count_alive() as u32
    }

    /// Moves an agent into a building (or `NONE`), keeping occupancy counts in sync.
    #[inline]
    pub(crate) fn set_place(&mut self, a: AgentId, p: u32) {
        let old = self.agents.place[a as usize];
        if old != NONE {
            let c = &mut self.buildings.present[old as usize];
            *c = c.saturating_sub(1);
        }
        if p != NONE {
            let c = &mut self.buildings.present[p as usize];
            *c = c.saturating_add(1);
        }
        self.agents.place[a as usize] = p;
    }

    #[inline]
    pub(crate) fn set_event(&mut self, a: AgentId, t: Tick) {
        self.agents.next_event[a as usize] = t;
        self.queue.schedule(a, t);
    }

    #[inline]
    pub(crate) fn journal(
        &mut self,
        a: AgentId,
        t: Tick,
        kind: JournalKind,
        arg: u16,
        subject: u32,
    ) {
        self.agents.journal.write(a, t, kind, arg, subject);
    }

    pub(crate) fn send(&mut self, dest: u16, body: Body) {
        let seq = self.out_seq;
        self.out_seq = self.out_seq.wrapping_add(1);
        self.outbox.push(Message {
            src: self.id,
            dest,
            seq,
            sent: self.tick,
            body,
        });
    }

    /// Takes the messages emitted since the last exchange.
    pub fn take_outbox(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.outbox)
    }

    /// Delivers inbound messages at the start of a window (`self.tick % WINDOW == 0`).
    /// Messages must be sorted by (src, seq); the world and the message log guarantee it.
    pub fn receive(&mut self, msgs: &[Message]) {
        self.prepare();
        debug_assert!(self.tick.is_multiple_of(WINDOW));
        for m in msgs {
            debug_assert_eq!(m.dest, self.id);
            match &m.body {
                Body::Trade { good: _, qty } => self.receive_food(m.src, *qty),
                Body::Traveller(t) => self.spawn_visitor(m.src, t),
                Body::VisitReport {
                    agent,
                    spent_cents,
                    places_visited: _,
                } => {
                    let a = *agent;
                    if (a as usize) < self.agents.len() && self.agents.alive(a) {
                        self.transfer(
                            crate::systems::economy::Account::Agent(a),
                            crate::systems::economy::Account::OtherRegions,
                            *spent_cents,
                        );
                        let t = self.tick;
                        self.journal(
                            a,
                            t,
                            JournalKind::BackFromRegion,
                            m.src,
                            (*spent_cents).clamp(0, u32::MAX as i64) as u32,
                        );
                    }
                }
                Body::Sale {
                    good: _,
                    qty,
                    unit_price,
                    seller_firm,
                } => self.receive_sale(m.src, *seller_firm, *qty, *unit_price),
                Body::Payment { firm, amount } => self.receive_payment(*firm, *amount),
                Body::Movers(movers) => self.receive_movers(m.src, movers),
                Body::JobOffers(offers) => self.receive_job_offers(m.src, offers),
                Body::JobAccept {
                    firm,
                    agent,
                    profession,
                    schedule,
                } => self.receive_job_accept(m.src, *firm, *agent, *profession, *schedule),
                Body::JobEnded { firm, agent } => self.receive_job_ended(m.src, *firm, *agent),
                Body::WorkerLeft { firm, agent } => self.receive_worker_left(m.src, *firm, *agent),
                Body::RoadBuilt {
                    side,
                    name,
                    highway_ticks,
                } => self.receive_road_built(m.src, *side, name, *highway_ticks),
                Body::Prisoner {
                    agent,
                    crime,
                    release,
                } => self.receive_prisoner(m.src, *agent, *crime, *release),
                Body::RemoteWage { agent, net } => self.receive_remote_wage(*agent, *net),
                // Later phases.
                Body::Migration { .. }
                | Body::MaterialShipment { .. }
                | Body::TechDiffusion { .. }
                | Body::NewsItem { .. } => {}
            }
        }
    }

    /// Food received for free (legacy `Trade` messages).
    fn receive_food(&mut self, src: u16, qty: u32) {
        use livingsim_content::FirmKind;
        *self.today.food_in.entry(src).or_insert(0) += qty;
        let firms = &mut self.systems.economy.firms;
        let target = firms
            .iter()
            .position(|f| f.open && f.kind == FirmKind::Warehouse)
            .or_else(|| {
                firms
                    .iter()
                    .position(|f| f.open && f.kind == FirmKind::Shop)
            });
        if let Some(f) = target {
            firms[f].stock = firms[f].stock.saturating_add(qty);
        }
    }

    /// Simulates one tick (one minute).
    pub fn step(&mut self) {
        self.prepare();
        let t = self.tick;
        if t.is_multiple_of(TICKS_PER_DAY) {
            self.morning(t);
        }
        if t.is_multiple_of(60) {
            self.crime_hourly(t);
        }
        let mut due = std::mem::take(&mut self.due);
        self.queue.take_due(t, &mut due);
        for &a in &due {
            if self.agents.next_event[a as usize] == t {
                self.process(a, t);
            }
        }
        self.due = due;
        self.tick = t + 1;
    }

    fn process(&mut self, a: AgentId, t: Tick) {
        self.events += 1;
        self.agents.next_event[a as usize] = Tick::MAX;
        match self.agents.act[a as usize] {
            ACT_FREE_SLOT => {}
            ACT_TRAVEL => self.arrive(a, t),
            ACT_AWAY => self.come_back(a, t),
            _ => {
                self.finish_activity(a, t);
                self.decide(a, t);
            }
        }
    }

    /// Daily update at 06:00: birthdays, news, the economy and later-phase systems.
    fn morning(&mut self, t: Tick) {
        let mut employed = 0;
        for a in 0..self.agents.len() as u32 {
            let i = a as usize;
            if !self.agents.alive(a)
                || self.agents.is_visitor(a)
                || self.agents.act[i] == ACT_FREE_SLOT
            {
                continue;
            }
            if self.agents.profession[i] != u16::MAX {
                employed += 1;
            }
            let age_now = crate::time::age_years(self.agents.birth[i], t);
            if t >= TICKS_PER_DAY
                && age_now != crate::time::age_years(self.agents.birth[i], t - TICKS_PER_DAY)
            {
                self.journal(a, t, JournalKind::BirthdayAge, age_now as u16, 0);
            }
        }
        self.write_news(t);
        self.demography_daily(t);
        self.economy_daily(t);
        self.commuting_daily(t);
        self.construction_daily(t);
        self.crime_daily(t);
        self.expansion_daily(t);
        let mut ctx = DailyContext {
            tick: t,
            population: self.population(),
            employed,
            housing_free: 0,
        };
        self.systems.daily(&mut ctx);
        self.yesterday = std::mem::take(&mut self.today);
        for v in self.buildings.visits_today.iter_mut() {
            *v = 0;
        }
        for v in self.buildings.empty_today.iter_mut() {
            *v = 0;
        }
    }

    pub(crate) fn push_news(&mut self, item: NewsItem) {
        self.news.push(item);
        if self.news.len() > NEWS_KEPT {
            self.news.remove(0);
        }
    }

    fn write_news(&mut self, t: Tick) {
        // Nothing happened yet (the region was just generated).
        if self.today.decisions == 0 {
            return;
        }
        let d = self.today.clone();
        self.push_news(NewsItem {
            tick: t,
            kind: NewsKind::DailySummary as u16,
            subject: self.id as u32,
            n: d.trips,
        });
        // Busiest place (ties: lowest id).
        let mut best: Option<(u16, u32)> = None;
        for (b, v) in self.buildings.visits_today.iter().enumerate() {
            if *v > 0 && best.is_none_or(|(bv, _)| *v > bv) {
                best = Some((*v, b as u32));
            }
        }
        if let Some((v, b)) = best {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::BusiestPlace as u16,
                subject: b,
                n: v as u32,
            });
        }
        let mut worst: Option<(u16, u32)> = None;
        for (b, v) in self.buildings.empty_today.iter().enumerate() {
            if *v > 0 && worst.is_none_or(|(bv, _)| *v > bv) {
                worst = Some((*v, b as u32));
            }
        }
        if let Some((v, b)) = worst {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::ShelvesEmpty as u16,
                subject: b,
                n: v as u32,
            });
        }
        for (r, n) in &d.visitors_in {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::Visitors as u16,
                subject: *r as u32,
                n: *n,
            });
        }
        for (r, n) in &d.food_in {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::FoodDelivery as u16,
                subject: *r as u32,
                n: *n,
            });
        }
    }

    // ------------------------------------------------------------------ trips

    /// Travel time between two buildings, in ticks (at least 1).
    pub fn trip_ticks(&mut self, from: BuildingId, to: BuildingId, mode: Mode) -> Tick {
        if from == to || from == NONE || to == NONE {
            return 1;
        }
        let b = &self.buildings;
        let (fa, ta) = (b.access_node[from as usize], b.access_node[to as usize]);
        let access = (b.access_tiles[from as usize] as u32 + b.access_tiles[to as usize] as u32)
            * crate::roads::decis_per_tile(mode, crate::roads::RoadClass::Street);
        let road = self.routes.travel(&self.roads, mode, fa, ta);
        let overhead = if mode == Mode::Car { 1200 } else { 0 };
        (access + road + overhead).div_ceil(600).max(1)
    }

    pub fn manhattan(&self, a: BuildingId, b: BuildingId) -> u32 {
        let (ax, ay) = (
            self.buildings.door_x[a as usize] as i32,
            self.buildings.door_y[a as usize] as i32,
        );
        let (bx, by) = (
            self.buildings.door_x[b as usize] as i32,
            self.buildings.door_y[b as usize] as i32,
        );
        ((ax - bx).abs() + (ay - by).abs()) as u32
    }

    pub(crate) fn mode_for(&self, a: AgentId, from: BuildingId, to: BuildingId, t: Tick) -> Mode {
        let i = a as usize;
        if self.agents.is_visitor(a) {
            return Mode::Car;
        }
        let hh = self.agents.household[i];
        let car = hh != NONE && self.households.has_car[hh as usize];
        let adult = crate::time::age_years(self.agents.birth[i], t) >= 18;
        if car && adult && from != NONE && to != NONE && self.manhattan(from, to) > 110 {
            Mode::Car
        } else {
            Mode::Walk
        }
    }

    // ------------------------------------------------------------------ visitors

    fn spawn_visitor(&mut self, src: u16, t: &crate::messages::Traveller) {
        let Some(gw) = self
            .gateways
            .iter()
            .find(|g| g.to_region == src)
            .map(|g| g.building)
        else {
            return;
        };
        let a = self.agents.alloc();
        let i = a as usize;
        let given = self.names.intern(&t.given);
        let family = self.names.intern(&t.family);
        let ag = &mut self.agents;
        ag.flags[i] = flags::ALIVE | flags::VISITOR | if t.female { flags::FEMALE } else { 0 };
        ag.given[i] = given;
        ag.family[i] = family;
        ag.birth[i] = t.birth;
        ag.traits[i] = t.traits;
        ag.needs[i] = t.needs;
        ag.needs_t[i] = self.tick;
        ag.home[i] = gw;
        ag.schedule[i] = catalog().schedule_unemployed;
        ag.act[i] = ACT_AWAY;
        ag.until[i] = t.arrival;
        ag.place[i] = NONE;
        self.visitors.insert(
            a,
            VisitorInfo {
                origin_region: src,
                origin_agent: t.agent,
                arrival: t.arrival,
                leave_by: t.leave_by,
                start_money: t.money,
                places: 0,
            },
        );
        self.visitor_money_in(a, t.money);
        *self.today.visitors_in.entry(src).or_insert(0) += 1;
        let at = t.arrival.max(self.tick);
        self.set_event(a, at);
    }

    /// End of `ACT_AWAY`: a visitor arriving here, or a local coming back from a trip.
    fn come_back(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        if self.agents.is_visitor(a) {
            let home = self.agents.home[i];
            self.set_place(a, home);
            self.agents.act[i] = catalog().relax_activity;
            self.agents.needs_t[i] = t;
            self.journal(a, t, JournalKind::ArrivedFromRegion, 0, 0);
            self.decide(a, t);
            return;
        }
        let info = self.away.remove(&a);
        let gw = info
            .and_then(|i| self.gateways.get(i.gateway as usize))
            .map(|g| g.building)
            .unwrap_or(self.agents.home[i]);
        self.set_place(a, gw);
        self.agents.act[i] = catalog().relax_activity;
        self.update_needs_after_away(a, t);
        self.decide(a, t);
    }

    fn update_needs_after_away(&mut self, a: AgentId, t: Tick) {
        // While away, needs evolved under the visit activity in the other region; we only
        // know its rough effect: fun and social improved, others decayed.
        let i = a as usize;
        let c = catalog();
        let dt = t.saturating_sub(self.agents.needs_t[i]) as i64;
        let visit = c
            .content
            .activities
            .iter()
            .position(|x| x.kind == livingsim_content::ActivityKind::VisitRegion)
            .unwrap_or(0);
        let rates = c.rates[visit];
        for (n, v) in self.agents.needs[i].iter_mut().enumerate().take(5) {
            *v = (*v as i64 + rates[n] as i64 * dt / 60).clamp(0, 10_000) as u16;
        }
        self.agents.needs_t[i] = t;
    }

    pub(crate) fn depart_visitor(&mut self, a: AgentId, t: Tick) {
        let Some(info) = self.visitors.remove(&a) else {
            self.visitor_money_out(a);
            self.set_place(a, NONE);
            self.agents.release(a);
            return;
        };
        let spent = info.start_money - self.agents.money[a as usize];
        self.send(
            info.origin_region,
            Body::VisitReport {
                agent: info.origin_agent,
                spent_cents: spent,
                places_visited: info.places,
            },
        );
        let _ = t;
        self.visitor_money_out(a);
        self.set_place(a, NONE);
        self.agents.release(a);
    }

    /// Heap bytes used by agent columns (for the memory budget test).
    pub fn agent_bytes(&self) -> usize {
        self.agents.len() * Agents::bytes_per_agent()
    }
}
