//! Phase 3: the life cycle. Friends, couples, births and adoption, schooling and
//! retirement, illness and death (always non-graphic), inheritance, people leaving and
//! arriving, and villagers commuting to jobs in another town.
//!
//! Everything runs once per simulated day (06:00) and is driven by rates in
//! `data/demography.ron`. Randomness comes from the keyed hash, so results never depend on
//! processing order. Money moves only through `Region::transfer`, so inheritance and
//! migration keep money conserved.

// Loops index several parallel vectors while also mutating `self`, so ranges are clearer.
#![allow(clippy::needless_range_loop)]

use super::economy::{Account, Tenure, TenureKind};
use super::DailyContext;
use crate::agents::{flags, AgentId, ACT_AWAY, ACT_FREE_SLOT, ACT_TRAVEL};
use crate::buildings::BuildingId;
use crate::catalog::{catalog, content};
use crate::messages::Body;
use crate::names::{generate, generate_for_baby, NameRef};
use crate::region::{AwayInfo, NewsItem, Region};
use crate::rng::{in_range, keyed};
use crate::roads::NONE;
use crate::time::{age_years, date, weekday, Tick, TICKS_PER_DAY, TICKS_PER_YEAR};
use livingsim_content::{FirmKind, JournalKind, NewsKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_FRIENDS: usize = 4;

/// Agents grouped by workplace/school and by home building.
type Groups = (
    BTreeMap<BuildingId, Vec<AgentId>>,
    BTreeMap<BuildingId, Vec<AgentId>>,
);
pub const HISTORY_DAYS: usize = 60;

/// Per-agent flags (in `DemographyState::dflags`).
pub mod dflags {
    pub const MARRIED: u8 = 1;
    pub const ADOPTED: u8 = 2;
    pub const ORPHAN: u8 = 4;
}

/// Education levels.
pub const EDU_NONE: u8 = 0;
pub const EDU_PRIMARY: u8 = 1;
pub const EDU_SECONDARY: u8 = 2;
pub const EDU_UNIVERSITY: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Opposite,
    Same,
    Both,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Illness {
    pub until: Tick,
    pub severe: bool,
}

/// A local person working for a firm in another region.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteJob {
    pub region: u16,
    pub firm: u32,
    pub place: String,
    pub profession: u16,
}

/// A person from another region working for a local firm.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteStaff {
    pub region: u16,
    pub agent: u32,
    pub firm: u32,
    pub profession: u16,
    pub schedule: u16,
}

/// A person travelling with a family that moves to another town.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mover {
    pub given: String,
    pub family: String,
    pub birth: i64,
    pub female: bool,
    pub traits: u32,
    pub money: i64,
    pub education: u8,
    pub married: bool,
    /// Index of the partner and parents inside the moving group (or `u8::MAX`).
    pub partner: u8,
    pub parent_a: u8,
    pub parent_b: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DemoTotals {
    pub births: u32,
    pub deaths: u32,
    pub marriages: u32,
    pub divorces: u32,
    pub couples: u32,
    pub adoptions: u32,
    pub immigrants: u32,
    pub emigrants: u32,
    pub moved_in: u32,
    pub moved_out: u32,
    pub left_home: u32,
    pub illnesses: u32,
    pub retirements: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DemoSample {
    pub tick: Tick,
    pub population: u32,
    pub births: u32,
    pub deaths: u32,
    pub marriages: u32,
    pub immigrants: u32,
    pub emigrants: u32,
    pub ill: u32,
    pub commuters_out: u32,
    pub commuters_in: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DemographyState {
    pub bootstrapped: bool,
    // ---- per agent slot (kept as long as `agents`)
    pub education: Vec<u8>,
    pub friends: Vec<[u32; MAX_FRIENDS]>,
    pub dflags: Vec<u8>,
    // ---- sparse state
    /// Couples dating but not living together: lower id → since.
    pub dating_since: BTreeMap<AgentId, Tick>,
    /// Mother → due tick.
    pub pregnancies: BTreeMap<AgentId, Tick>,
    pub leave_until: BTreeMap<AgentId, Tick>,
    pub illness: BTreeMap<AgentId, Illness>,
    pub unemployed_since: BTreeMap<AgentId, Tick>,
    pub remote_jobs: BTreeMap<AgentId, RemoteJob>,
    pub remote_staff: Vec<RemoteStaff>,
    pub today: DemoTotals,
    pub yesterday: DemoTotals,
    pub week: DemoTotals,
    pub history: Vec<DemoSample>,
}

impl DemographyState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // The life cycle needs the whole region; see `Region::demography_daily`.
    }
    pub fn education_of(&self, a: AgentId) -> u8 {
        self.education.get(a as usize).copied().unwrap_or(0)
    }
    pub fn married(&self, a: AgentId) -> bool {
        self.dflags
            .get(a as usize)
            .is_some_and(|f| f & dflags::MARRIED != 0)
    }
    pub fn adopted(&self, a: AgentId) -> bool {
        self.dflags
            .get(a as usize)
            .is_some_and(|f| f & dflags::ADOPTED != 0)
    }
}

// Keyed-hash purposes for the daily rolls.
const P_DEATH: u64 = 0xD1;
const P_ILL: u64 = 0xD2;
const P_SEVERE: u64 = 0xD3;
const P_ILL_DAYS: u64 = 0xD4;
const P_DATE: u64 = 0xD5;
const P_MARRY: u64 = 0xD6;
const P_DIVORCE: u64 = 0xD7;
const P_PREG: u64 = 0xD8;
const P_ADOPT: u64 = 0xD9;
const P_FRIEND: u64 = 0xDA;
const P_LEAVE: u64 = 0xDB;
const P_EMIG: u64 = 0xDC;
const P_IMMIG: u64 = 0xDD;
const P_UNI: u64 = 0xDE;
const P_ORIENT: u64 = 0xDF;
const P_MISC: u64 = 0xE0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Leaving {
    Death,
    Emigration,
    MovedTown,
}

fn by_decade(v: &[u32], age: u32) -> u32 {
    v[((age / 10) as usize).min(v.len() - 1)]
}

impl Region {
    // ------------------------------------------------------------------ helpers

    /// Daily roll for an annual ‰ probability.
    fn roll(&self, a: u64, day: u64, purpose: u64, annual_permille: u32) -> bool {
        keyed(self.seed, a, day, purpose) % 365_000 < annual_permille as u64
    }

    pub fn orientation(&self, a: AgentId) -> Orientation {
        let d = &content().demography;
        let r = (keyed(self.seed, a as u64, 0, P_ORIENT) % 1000) as u32;
        if r < d.same_sex_permille {
            Orientation::Same
        } else if r < d.same_sex_permille + d.bisexual_permille {
            Orientation::Both
        } else {
            Orientation::Opposite
        }
    }

    fn female(&self, a: AgentId) -> bool {
        self.agents.flags[a as usize] & flags::FEMALE != 0
    }

    fn attracted(&self, a: AgentId, b: AgentId) -> bool {
        let same = self.female(a) == self.female(b);
        let ok = |x: AgentId| match self.orientation(x) {
            Orientation::Opposite => !same,
            Orientation::Same => same,
            Orientation::Both => true,
        };
        ok(a) && ok(b)
    }

    /// A local, living resident (not a visitor, not a free slot).
    pub fn resident(&self, a: AgentId) -> bool {
        (a as usize) < self.agents.len()
            && self.agents.alive(a)
            && !self.agents.is_visitor(a)
            && self.agents.act[a as usize] != ACT_FREE_SLOT
    }

    fn age(&self, a: AgentId, t: Tick) -> u32 {
        age_years(self.agents.birth[a as usize], t)
    }

    fn ensure_demo_len(&mut self) {
        let n = self.agents.len();
        let d = &mut self.systems.demography;
        d.education.resize(n, 0);
        d.friends.resize(n, [NONE; MAX_FRIENDS]);
        d.dflags.resize(n, 0);
    }

    /// Resets per-agent demographic state for a freshly allocated slot.
    pub(crate) fn demo_reset(&mut self, a: AgentId) {
        self.ensure_demo_len();
        let d = &mut self.systems.demography;
        d.education[a as usize] = 0;
        d.friends[a as usize] = [NONE; MAX_FRIENDS];
        d.dflags[a as usize] = 0;
    }

    pub fn is_ill(&self, a: AgentId) -> Option<&Illness> {
        self.systems.demography.illness.get(&a)
    }

    pub fn on_leave(&self, a: AgentId, t: Tick) -> bool {
        self.systems
            .demography
            .leave_until
            .get(&a)
            .is_some_and(|u| *u > t)
    }

    pub fn remote_job(&self, a: AgentId) -> Option<&RemoteJob> {
        self.systems.demography.remote_jobs.get(&a)
    }

    /// Number of people from other regions working for firm `f`.
    pub fn remote_staff_of(&self, f: u32) -> usize {
        self.systems
            .demography
            .remote_staff
            .iter()
            .filter(|r| r.firm == f)
            .count()
    }

    fn building_with_tag(
        &self,
        tag: &str,
        near: BuildingId,
        capacity_left: bool,
    ) -> Option<BuildingId> {
        let t = content().tag(tag)?;
        let mut best: Option<(u32, BuildingId)> = None;
        for &b in self.buildings.with_tag(t) {
            if capacity_left {
                let cap =
                    content().buildings[self.buildings.def[b as usize] as usize].students as usize;
                let used = (0..self.agents.len())
                    .filter(|a| self.agents.work[*a] == b && self.agents.profession[*a] == u16::MAX)
                    .count();
                if cap > 0 && used >= cap {
                    continue;
                }
            }
            let d = if near == NONE {
                0
            } else {
                self.manhattan(near, b)
            };
            if best.is_none_or(|(bd, bb)| (d, b) < (bd, bb)) {
                best = Some((d, b));
            }
        }
        best.map(|(_, b)| b)
    }

    /// Homes with room for one more household: building → free dwellings.
    fn free_homes(&self) -> Vec<(BuildingId, u16)> {
        let mut used = vec![0u16; self.buildings.len()];
        for h in 0..self.households.len() {
            if self.households.members[h] > 0 {
                used[self.households.home[h] as usize] += 1;
            }
        }
        let c = content();
        (0..self.buildings.len())
            .filter_map(|b| {
                let cap = c.buildings[self.buildings.def[b] as usize].dwellings;
                (cap > used[b]).then_some((b as BuildingId, cap - used[b]))
            })
            .collect()
    }

    fn take_free_home(free: &mut Vec<(BuildingId, u16)>, pick: u64) -> Option<BuildingId> {
        if free.is_empty() {
            return None;
        }
        let i = (pick % free.len() as u64) as usize;
        let b = free[i].0;
        free[i].1 -= 1;
        if free[i].1 == 0 {
            free.swap_remove(i);
        }
        Some(b)
    }

    /// Creates a household in `home` with a rental tenure (or ownership if wealthy).
    fn new_household(&mut self, home: BuildingId, wealth: i64, key: u64) -> u32 {
        let c = content();
        let has_car = keyed(self.seed, key, 1, P_MISC) % 1000 < 600;
        let hh = self.households.push(home, has_car);
        let def = &c.buildings[self.buildings.def[home as usize] as usize];
        let agencies: Vec<u32> = self
            .systems
            .economy
            .firms
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == FirmKind::RealEstate && f.open)
            .map(|(i, _)| i as u32)
            .collect();
        let roll = keyed(self.seed, key, 2, P_MISC) % 100;
        let price = def.rent_cents * 150; // ~12 years of rent buys the home
        let kind = if def.dwellings <= 3 && wealth > price && roll < 50 {
            TenureKind::Owner
        } else if !agencies.is_empty() && roll.is_multiple_of(2) {
            TenureKind::Agency
        } else {
            TenureKind::Municipal
        };
        let mut landlord = if kind == TenureKind::Agency {
            agencies[(roll as usize) % agencies.len()]
        } else {
            NONE
        };
        // Homes built in Phase 4 are rented out by whoever paid for them.
        use super::construction::HomeOwner;
        let kind = match self.home_owner(home) {
            Some(HomeOwner::Municipal) => TenureKind::Municipal,
            Some(HomeOwner::Agency(f)) if self.systems.economy.firms[f as usize].open => {
                landlord = f;
                TenureKind::Agency
            }
            Some(HomeOwner::Landlord(o)) if self.resident(o) => {
                landlord = o;
                TenureKind::PrivateLandlord
            }
            Some(_) => {
                landlord = NONE;
                TenureKind::Municipal
            }
            None => kind,
        };
        let (cx, cy) = (self.map.width as i64 / 2, self.map.height as i64 / 2);
        let half = (self.map.width as i64 / 2).max(1);
        let dist = ((self.buildings.door_x[home as usize] as i64 - cx).abs()
            + (self.buildings.door_y[home as usize] as i64 - cy).abs())
        .min(half * 2);
        let location = 1300 - 600 * dist / (half * 2);
        let rent = if kind == TenureKind::Owner {
            0
        } else {
            def.rent_cents * location / 1000 / 100 * 100
        };
        self.systems.economy.tenure.push(Tenure {
            kind,
            landlord,
            rent,
            arrears: 0,
        });
        hh
    }

    fn move_to_household(&mut self, a: AgentId, hh: u32) {
        let i = a as usize;
        let old = self.agents.household[i];
        if old != NONE {
            let m = &mut self.households.members[old as usize];
            *m = m.saturating_sub(1);
        }
        self.agents.household[i] = hh;
        self.agents.home[i] = self.households.home[hh as usize];
        self.households.members[hh as usize] += 1;
    }

    /// Living, local members of a household.
    fn household_members(&self, hh: u32) -> Vec<AgentId> {
        (0..self.agents.len() as u32)
            .filter(|a| self.agents.household[*a as usize] == hh && self.resident(*a))
            .collect()
    }

    fn children_of(&self, a: AgentId) -> Vec<AgentId> {
        (0..self.agents.len() as u32)
            .filter(|k| {
                self.resident(*k)
                    && (self.agents.parent_a[*k as usize] == a
                        || self.agents.parent_b[*k as usize] == a)
            })
            .collect()
    }

    /// Number of living children of every agent (one pass).
    fn children_counts(&self) -> Vec<u16> {
        let mut n = vec![0u16; self.agents.len()];
        for k in 0..self.agents.len() as u32 {
            if !self.resident(k) {
                continue;
            }
            for p in [
                self.agents.parent_a[k as usize],
                self.agents.parent_b[k as usize],
            ] {
                if p != NONE && (p as usize) < n.len() {
                    n[p as usize] += 1;
                }
            }
        }
        n
    }

    /// Creates a person. Returns the new agent id.
    #[allow(clippy::too_many_arguments)]
    fn create_person(
        &mut self,
        hh: u32,
        female: bool,
        birth: i64,
        given: &str,
        family: NameRef,
        money: i64,
        education: u8,
        t: Tick,
    ) -> AgentId {
        let a = self.agents.alloc();
        self.demo_reset(a);
        let i = a as usize;
        let given = self.names.intern(given);
        let cat = catalog();
        let ag = &mut self.agents;
        ag.flags[i] = flags::ALIVE | if female { flags::FEMALE } else { 0 };
        ag.given[i] = given;
        ag.family[i] = family;
        ag.birth[i] = birth;
        ag.traits[i] = keyed(self.seed, a as u64, t as u64, P_MISC) as u32;
        ag.money[i] = 0;
        ag.needs[i] = [8000, 8000, 8000, 8000, 8000, 5000, 8000];
        ag.needs_t[i] = t;
        ag.schedule[i] = if age_years(birth, t) >= 18 {
            cat.schedule_unemployed
        } else {
            cat.schedule_toddler
        };
        ag.act[i] = cat.relax_activity;
        ag.until[i] = t + 30;
        self.systems.demography.education[i] = education;
        self.move_to_household(a, hh);
        let home = self.agents.home[i];
        self.set_place(a, home);
        if money != 0 {
            self.transfer(Account::World, Account::Agent(a), money);
        }
        self.set_event(a, t + 30);
        a
    }

    // ------------------------------------------------------------------ bootstrap

    /// Assigns education, daycare places and first friendships to an existing population
    /// (new worlds and migrated snapshots). Deterministic in the region seed.
    pub fn bootstrap_demography(&mut self) {
        if self.systems.demography.bootstrapped {
            return;
        }
        self.ensure_demo_len();
        let c = content();
        let cat = catalog();
        let t = self.tick;
        let (p1, p2, _) = c.demography.initial_education_permille;
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) {
                continue;
            }
            let i = a as usize;
            let age = self.age(a, t);
            let roll = (keyed(self.seed, a as u64, 0, P_UNI) % 1000) as u32;
            let mut edu = match age {
                0..=11 => EDU_NONE,
                12..=17 => EDU_PRIMARY,
                _ => {
                    if roll < p1 {
                        EDU_PRIMARY
                    } else if roll < p1 + p2 {
                        EDU_SECONDARY
                    } else {
                        EDU_UNIVERSITY
                    }
                }
            };
            // Consistent with their job.
            let p = self.agents.profession[i];
            if p != u16::MAX {
                edu = edu.max(c.professions[p as usize].education);
            }
            if self.agents.schedule[i] == cat.schedule_university {
                edu = EDU_SECONDARY;
            }
            self.systems.demography.education[i] = edu;
            // Toddlers go to daycare where there is one.
            if (1..=5).contains(&age) && self.agents.schedule[i] == cat.schedule_toddler {
                if let Some(d) = self.building_with_tag("daycare", self.agents.home[i], true) {
                    self.agents.work[i] = d;
                    self.agents.schedule[i] = cat.schedule_daycare;
                }
            }
            // Married couples from the generator.
            if self.agents.partner[i] != NONE {
                self.systems.demography.dflags[i] |= dflags::MARRIED;
            }
        }
        // Pregnancies already under way when the world starts (or is migrated).
        let kids_of = self.children_counts();
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) || !self.female(a) {
                continue;
            }
            let p = self.agents.partner[a as usize];
            if p == NONE
                || !self.resident(p)
                || self.female(p)
                || self.agents.household[p as usize] != self.agents.household[a as usize]
            {
                continue;
            }
            if kids_of[a as usize] >= c.demography.max_children as u16 {
                continue;
            }
            let age = self.age(a, t) as u8;
            let Some((_, rate)) = c
                .demography
                .fertility_by_age
                .iter()
                .find(|(max, _)| age <= *max)
            else {
                continue;
            };
            // Chance of being pregnant right now ≈ yearly rate × 9 months.
            if keyed(self.seed, a as u64, 1, P_PREG) % 1000 < (*rate as u64 * 3 / 4) {
                let left = in_range(
                    keyed(self.seed, a as u64, 2, P_PREG),
                    1,
                    c.demography.pregnancy_days as i64,
                ) as Tick;
                self.systems
                    .demography
                    .pregnancies
                    .insert(a, t + left * TICKS_PER_DAY);
            }
        }
        // First friendships: coworkers, classmates and neighbours.
        let groups = self.groups();
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) || self.age(a, t) < 6 {
                continue;
            }
            for k in 0..2u64 {
                self.try_befriend(
                    a,
                    &groups,
                    keyed(self.seed, a as u64, k, P_FRIEND),
                    t,
                    false,
                );
            }
        }
        self.systems.demography.bootstrapped = true;
    }

    /// Agents grouped by workplace/school and by home building (ids ascending).
    fn groups(&self) -> Groups {
        let mut work: BTreeMap<BuildingId, Vec<AgentId>> = BTreeMap::new();
        let mut home: BTreeMap<BuildingId, Vec<AgentId>> = BTreeMap::new();
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) {
                continue;
            }
            let w = self.agents.work[a as usize];
            if w != NONE {
                work.entry(w).or_default().push(a);
            }
            home.entry(self.agents.home[a as usize])
                .or_default()
                .push(a);
        }
        (work, home)
    }

    fn try_befriend(&mut self, a: AgentId, groups: &Groups, h: u64, t: Tick, journal: bool) {
        let i = a as usize;
        let d = &self.systems.demography;
        let Some(slot) = d.friends[i].iter().position(|f| *f == NONE) else {
            return;
        };
        let use_work = !h.is_multiple_of(3) && self.agents.work[i] != NONE;
        let list = if use_work {
            groups.0.get(&self.agents.work[i])
        } else {
            groups.1.get(&self.agents.home[i])
        };
        let Some(list) = list else { return };
        if list.len() < 2 {
            return;
        }
        let b = list[((h >> 8) % list.len() as u64) as usize];
        let bi = b as usize;
        if b == a
            || d.friends[i].contains(&b)
            || self.agents.household[bi] == self.agents.household[i]
        {
            return;
        }
        let (aa, ab) = (self.age(a, t) as i64, self.age(b, t) as i64);
        if (aa - ab).abs() > 15 && aa.min(ab) < 25 {
            return;
        }
        let Some(slot_b) = d.friends[bi].iter().position(|f| *f == NONE) else {
            return;
        };
        self.systems.demography.friends[i][slot] = b;
        self.systems.demography.friends[bi][slot_b] = a;
        if journal {
            self.journal(a, t, JournalKind::NewFriend, 0, b);
            self.journal(b, t, JournalKind::NewFriend, 0, a);
        }
    }

    /// A friend's home to visit (deterministic pick), if any.
    pub(crate) fn friend_home(&self, a: AgentId, t: Tick) -> Option<BuildingId> {
        let fr = self.systems.demography.friends.get(a as usize)?;
        let alive: Vec<AgentId> = fr
            .iter()
            .copied()
            .filter(|f| *f != NONE && self.resident(*f))
            .collect();
        if alive.is_empty() {
            return None;
        }
        let f = alive[(keyed(
            self.seed,
            a as u64,
            t as u64 / TICKS_PER_DAY as u64,
            P_FRIEND,
        ) % alive.len() as u64) as usize];
        let home = self.agents.home[f as usize];
        (home != self.agents.home[a as usize]).then_some(home)
    }

    // ------------------------------------------------------------------ daily

    /// Runs every morning at 06:00, before the economy.
    pub(crate) fn demography_daily(&mut self, t: Tick) {
        self.bootstrap_demography();
        self.ensure_demo_len();
        {
            let d = &mut self.systems.demography;
            d.yesterday = std::mem::take(&mut d.today);
        }
        let day = (t / TICKS_PER_DAY) as u64;
        let people: Vec<AgentId> = (0..self.agents.len() as u32)
            .filter(|a| self.resident(*a))
            .collect();

        self.health(t, day, &people);
        self.deaths(t, day, &people);
        self.births(t);
        let people: Vec<AgentId> = (0..self.agents.len() as u32)
            .filter(|a| self.resident(*a))
            .collect();
        self.stages(t, &people);
        let groups = self.groups();
        self.relationships(t, day, &people, &groups);
        self.moving_out(t, day, &people);
        self.immigration(t, day);
        self.track_unemployment(t);
        self.parental_benefits(t);
        self.demography_news(t);
        self.sample_demography(t);
    }

    fn health(&mut self, t: Tick, day: u64, people: &[AgentId]) {
        let dm = &content().demography;
        // Recoveries.
        let done: Vec<AgentId> = self
            .systems
            .demography
            .illness
            .iter()
            .filter(|(_, i)| i.until <= t)
            .map(|(a, _)| *a)
            .collect();
        for a in done {
            self.systems.demography.illness.remove(&a);
            if self.resident(a) {
                self.journal(a, t, JournalKind::Recovered, 0, 0);
            }
        }
        // New illnesses.
        for &a in people {
            if self.systems.demography.illness.contains_key(&a) {
                continue;
            }
            let age = self.age(a, t);
            if !self.roll(a as u64, day, P_ILL, by_decade(&dm.illness_by_decade, age)) {
                continue;
            }
            let severe = keyed(self.seed, a as u64, day, P_SEVERE) % 1000
                < by_decade(&dm.severe_by_decade, age) as u64;
            let days = in_range(
                keyed(self.seed, a as u64, day, P_ILL_DAYS),
                dm.illness_days.0 as i64,
                dm.illness_days.1 as i64,
            ) as Tick;
            self.systems.demography.illness.insert(
                a,
                Illness {
                    until: t + days * TICKS_PER_DAY,
                    severe,
                },
            );
            self.systems.demography.today.illnesses += 1;
            if severe {
                let hospital = self
                    .building_with_tag("hospital", self.agents.home[a as usize], false)
                    .unwrap_or(NONE);
                self.journal(a, t, JournalKind::Hospitalized, 0, hospital);
            } else {
                self.journal(a, t, JournalKind::GotSick, 0, 0);
            }
            // Re-plan their day now (they skip work or school).
            if self.agents.act[a as usize] != ACT_AWAY && self.agents.act[a as usize] != ACT_TRAVEL
            {
                self.set_event(a, t + 1);
            }
        }
    }

    fn deaths(&mut self, t: Tick, day: u64, people: &[AgentId]) {
        let dm = &content().demography;
        let mut dying = Vec::new();
        for &a in people {
            let age = self.age(a, t);
            let mut rate = by_decade(&dm.mortality_by_decade, age);
            if self
                .systems
                .demography
                .illness
                .get(&a)
                .is_some_and(|i| i.severe)
            {
                rate = rate.saturating_mul(dm.severe_mortality_factor);
            }
            if self.roll(a as u64, day, P_DEATH, rate) {
                dying.push(a);
            }
        }
        for a in dying {
            if self.resident(a) {
                self.remove_person(a, t, Leaving::Death);
            }
        }
    }

    fn births(&mut self, t: Tick) {
        let due: Vec<(AgentId, Tick)> = self
            .systems
            .demography
            .pregnancies
            .iter()
            .filter(|(_, d)| **d <= t)
            .map(|(a, d)| (*a, *d))
            .collect();
        for (mother, _) in due {
            self.systems.demography.pregnancies.remove(&mother);
            if !self.resident(mother) {
                continue;
            }
            let father = self.agents.partner[mother as usize];
            let father = if father != NONE && self.resident(father) {
                father
            } else {
                NONE
            };
            self.give_birth(mother, father, t);
        }
    }

    fn give_birth(&mut self, mother: AgentId, father: AgentId, t: Tick) {
        let c = content();
        let mi = mother as usize;
        let hh = self.agents.household[mi];
        let female = keyed(self.seed, mother as u64, t as u64, P_MISC).is_multiple_of(2);
        let style = &c.names.styles[self.style as usize];
        let set = if female { &style.female } else { &style.male };
        // Names in the family influence the choice.
        let mut relatives: Vec<String> = vec![self.names.get(self.agents.given[mi]).to_string()];
        for p in [self.agents.parent_a[mi], self.agents.parent_b[mi], father] {
            if p != NONE && self.resident(p) {
                relatives.push(self.names.get(self.agents.given[p as usize]).to_string());
            }
        }
        let refs: Vec<&str> = relatives.iter().map(|s| s.as_str()).collect();
        let given = generate_for_baby(
            &c.names,
            set,
            &refs,
            keyed(self.seed, mother as u64, t as u64, 0xBABE),
        );
        let family = if father != NONE
            && keyed(self.seed, father as u64, t as u64, P_MISC).is_multiple_of(2)
        {
            self.agents.family[father as usize]
        } else {
            self.agents.family[mi]
        };
        let baby = self.create_person(hh, female, t as i64, &given, family, 0, EDU_NONE, t);
        self.agents.parent_a[baby as usize] = mother;
        self.agents.parent_b[baby as usize] = father;
        let hospital = self
            .building_with_tag("hospital", self.agents.home[mi], false)
            .unwrap_or(self.agents.home[mi]);
        self.journal(baby, t, JournalKind::WasBorn, 0, hospital);
        let leave = t + content().demography.parental_leave_days as Tick * TICKS_PER_DAY;
        for p in [mother, father] {
            if p != NONE {
                self.journal(p, t, JournalKind::BabyBorn, 0, baby);
                self.systems.demography.leave_until.insert(p, leave);
            }
        }
        self.systems.demography.today.births += 1;
    }

    /// Birthdays that change someone's stage of life.
    fn stages(&mut self, t: Tick, people: &[AgentId]) {
        if t < TICKS_PER_DAY {
            return;
        }
        let c = content();
        let cat = catalog();
        let dm = &c.demography;
        for &a in people {
            let i = a as usize;
            let birth = self.agents.birth[i];
            let age = age_years(birth, t);
            if age == age_years(birth, t - TICKS_PER_DAY) {
                continue;
            }
            let home = self.agents.home[i];
            let sched = self.agents.schedule[i];
            if age == 100 {
                let family = self.agents.family[i];
                let given = self.agents.given[i];
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Centenarian as u16,
                    subject: given,
                    n: family,
                });
            }
            match age {
                1 if sched == cat.schedule_toddler => {
                    if let Some(d) = self.building_with_tag("daycare", home, true) {
                        self.agents.work[i] = d;
                        self.agents.schedule[i] = cat.schedule_daycare;
                        self.journal(a, t, JournalKind::StartedSchool, 0, d);
                    }
                }
                6 => {
                    if let Some(s) = self.building_with_tag("school", home, true) {
                        self.agents.work[i] = s;
                        self.agents.schedule[i] = cat.schedule_child;
                        self.journal(a, t, JournalKind::StartedSchool, 0, s);
                    } else {
                        self.agents.work[i] = NONE;
                        self.agents.schedule[i] = cat.schedule_child;
                    }
                }
                12 => {
                    self.systems.demography.education[i] =
                        self.systems.demography.education[i].max(EDU_PRIMARY)
                }
                18 if sched == cat.schedule_child || sched == cat.schedule_toddler => {
                    let school = self.agents.work[i];
                    self.systems.demography.education[i] =
                        self.systems.demography.education[i].max(EDU_SECONDARY);
                    self.journal(a, t, JournalKind::Graduated, 0, school);
                    let uni = self.building_with_tag("university", home, true);
                    let goes = keyed(self.seed, a as u64, 18, P_UNI) % 1000
                        < dm.university_permille as u64;
                    if let (Some(u), true) = (uni, goes) {
                        self.agents.work[i] = u;
                        self.agents.schedule[i] = cat.schedule_university;
                        self.journal(a, t, JournalKind::StartedUniversity, 0, u);
                    } else {
                        self.agents.work[i] = NONE;
                        self.agents.schedule[i] = cat.schedule_unemployed;
                    }
                }
                _ => {}
            }
            // University degree.
            if sched == cat.schedule_university && age >= 18 + dm.university_years as u32 {
                let uni = self.agents.work[i];
                self.systems.demography.education[i] = EDU_UNIVERSITY;
                self.agents.work[i] = NONE;
                self.agents.schedule[i] = cat.schedule_unemployed;
                self.journal(a, t, JournalKind::Graduated, 0, uni);
            }
            // Retirement.
            if age >= dm.retirement_age as u32
                && sched != cat.schedule_retired
                && sched != cat.schedule_toddler
                && sched != cat.schedule_child
            {
                self.quit_job(a);
                self.agents.schedule[i] = cat.schedule_retired;
                self.systems.demography.unemployed_since.remove(&a);
                self.systems.demography.today.retirements += 1;
                self.journal(a, t, JournalKind::Retired, 0, 0);
            }
        }
    }

    /// Leaves the current job (local or in another town) without a layoff entry.
    fn quit_job(&mut self, a: AgentId) {
        let i = a as usize;
        if let Some(job) = self.systems.demography.remote_jobs.remove(&a) {
            self.send(
                job.region,
                Body::WorkerLeft {
                    firm: job.firm,
                    agent: a,
                },
            );
        }
        if self.agents.profession[i] != u16::MAX
            || self.agents.schedule[i] == catalog().schedule_university
        {
            self.agents.work[i] = NONE;
            self.agents.profession[i] = u16::MAX;
        }
    }

    fn relationships(&mut self, t: Tick, day: u64, people: &[AgentId], groups: &Groups) {
        let c = content();
        let dm = &c.demography;
        let mut free = self.free_homes();
        let kids_of = self.children_counts();
        for &a in people {
            if !self.resident(a) {
                continue;
            }
            let i = a as usize;
            let age = self.age(a, t);
            // Friendships.
            if age >= 6 && self.roll(a as u64, day, P_FRIEND, dm.friendship_permille) {
                self.try_befriend(
                    a,
                    groups,
                    keyed(self.seed, a as u64, day, P_FRIEND + 1),
                    t,
                    true,
                );
            }
            if age < 18 {
                continue;
            }
            let p = self.agents.partner[i];
            if p == NONE {
                // Single: maybe start dating a friend or someone met at work.
                if (20..=65).contains(&age) && self.roll(a as u64, day, P_DATE, dm.dating_permille)
                {
                    self.find_partner(a, t, day, groups);
                }
                continue;
            }
            if p < a || !self.resident(p) {
                continue; // handle each couple once (from the lower id)
            }
            let pi = p as usize;
            let together = self.agents.household[i] == self.agents.household[pi];
            if !together {
                let since = self
                    .systems
                    .demography
                    .dating_since
                    .get(&a)
                    .copied()
                    .unwrap_or(t);
                if t >= since + dm.move_in_after_days as Tick * TICKS_PER_DAY {
                    self.move_in_together(a, p, t, &mut free);
                }
                continue;
            }
            let married = self.systems.demography.married(a);
            if !married && self.roll(a as u64, day, P_MARRY, dm.marriage_permille) {
                self.systems.demography.dflags[i] |= dflags::MARRIED;
                self.systems.demography.dflags[pi] |= dflags::MARRIED;
                self.journal(a, t, JournalKind::Married, 0, p);
                self.journal(p, t, JournalKind::Married, 0, a);
                self.systems.demography.today.marriages += 1;
                let (ga, gb) = (self.agents.given[i], self.agents.given[pi]);
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Wedding as u16,
                    subject: ga,
                    n: gb,
                });
                continue;
            }
            if self.roll(
                a as u64,
                day,
                P_DIVORCE,
                dm.divorce_permille * if married { 1 } else { 3 },
            ) {
                self.separate(a, p, t, &mut free);
                continue;
            }
            // Children: a fertile woman with a male partner can conceive; any couple can adopt.
            let (fa, fp) = (self.female(a), self.female(p));
            let mother = if fa && !fp {
                Some(a)
            } else if fp && !fa {
                Some(p)
            } else {
                None
            };
            let kids = (kids_of[i] as usize).max(kids_of[pi] as usize);
            if kids >= dm.max_children as usize {
                continue;
            }
            if let Some(m) = mother {
                let mage = self.age(m, t) as u8;
                if !self.systems.demography.pregnancies.contains_key(&m) {
                    if let Some((_, rate)) =
                        dm.fertility_by_age.iter().find(|(max, _)| mage <= *max)
                    {
                        if self.roll(m as u64, day, P_PREG, *rate) {
                            let due = t + dm.pregnancy_days as Tick * TICKS_PER_DAY;
                            self.systems.demography.pregnancies.insert(m, due);
                        }
                    }
                }
            }
            let both_ok = (25..=50).contains(&age) && (25..=50).contains(&self.age(p, t));
            let adopt_rate = if mother.is_none() {
                dm.adoption_permille * 3
            } else if kids == 0 && age >= 38 {
                dm.adoption_permille
            } else {
                0
            };
            if both_ok
                && kids < 2
                && adopt_rate > 0
                && self.roll(a as u64, day, P_ADOPT, adopt_rate)
            {
                self.adopt(a, p, t);
            }
        }
    }

    fn find_partner(&mut self, a: AgentId, t: Tick, day: u64, groups: &Groups) {
        let i = a as usize;
        let age = self.age(a, t) as i64;
        let mut candidates: Vec<AgentId> = self.systems.demography.friends[i]
            .iter()
            .copied()
            .filter(|f| *f != NONE)
            .collect();
        if let Some(w) = groups.0.get(&self.agents.work[i]) {
            for k in 0..6u64 {
                candidates.push(
                    w[(keyed(self.seed, a as u64, day * 8 + k, P_DATE) % w.len() as u64) as usize],
                );
            }
        }
        // Someone met around town.
        let n = self.agents.len() as u64;
        for k in 0..10u64 {
            candidates.push((keyed(self.seed, a as u64, day * 16 + k, P_DATE + 1) % n) as AgentId);
        }
        for b in candidates {
            let bi = b as usize;
            if b == a
                || !self.resident(b)
                || self.agents.partner[bi] != NONE
                || self.agents.household[bi] == self.agents.household[i]
            {
                continue;
            }
            let bage = self.age(b, t) as i64;
            if bage < 20 || (bage - age).abs() > 8 || !self.attracted(a, b) {
                continue;
            }
            // Not close family.
            let pa = |x: AgentId| {
                [
                    self.agents.parent_a[x as usize],
                    self.agents.parent_b[x as usize],
                ]
            };
            if pa(a).iter().any(|p| *p != NONE && pa(b).contains(p))
                || pa(a).contains(&b)
                || pa(b).contains(&a)
            {
                continue;
            }
            self.agents.partner[i] = b;
            self.agents.partner[bi] = a;
            self.systems.demography.dating_since.insert(a.min(b), t);
            self.systems.demography.today.couples += 1;
            self.journal(a, t, JournalKind::StartedDating, 0, b);
            self.journal(b, t, JournalKind::StartedDating, 0, a);
            return;
        }
    }

    fn move_in_together(
        &mut self,
        a: AgentId,
        b: AgentId,
        t: Tick,
        free: &mut Vec<(BuildingId, u16)>,
    ) {
        let (ha, hb) = (
            self.agents.household[a as usize],
            self.agents.household[b as usize],
        );
        let (ma, mb) = (
            self.households.members[ha as usize],
            self.households.members[hb as usize],
        );
        // Prefer the home of whoever lives alone; otherwise a free home.
        let target = if ma == 1 {
            Some(ha)
        } else if mb == 1 {
            Some(hb)
        } else {
            let wealth = self.agents.money[a as usize] + self.agents.money[b as usize];
            Self::take_free_home(free, keyed(self.seed, a as u64, t as u64, P_MISC))
                .map(|home| self.new_household(home, wealth, a as u64 ^ (t as u64) << 20))
        };
        let Some(hh) = target else { return }; // no home yet: keep dating, living apart
        for x in [a, b] {
            if self.agents.household[x as usize] != hh {
                // Their young children come along.
                let old = self.agents.household[x as usize];
                let kids: Vec<AgentId> = self
                    .children_of(x)
                    .into_iter()
                    .filter(|k| self.agents.household[*k as usize] == old && self.age(*k, t) < 18)
                    .collect();
                self.move_to_household(x, hh);
                for k in kids {
                    self.move_to_household(k, hh);
                }
            }
        }
        self.systems.demography.dating_since.remove(&a.min(b));
        self.journal(a, t, JournalKind::MovedInWith, 0, b);
        self.journal(b, t, JournalKind::MovedInWith, 0, a);
    }

    fn separate(&mut self, a: AgentId, b: AgentId, t: Tick, free: &mut Vec<(BuildingId, u16)>) {
        let (ai, bi) = (a as usize, b as usize);
        self.agents.partner[ai] = NONE;
        self.agents.partner[bi] = NONE;
        let married = self.systems.demography.married(a);
        self.systems.demography.dflags[ai] &= !dflags::MARRIED;
        self.systems.demography.dflags[bi] &= !dflags::MARRIED;
        self.systems.demography.dating_since.remove(&a.min(b));
        if married {
            self.systems.demography.today.divorces += 1;
        }
        self.journal(a, t, JournalKind::Divorced, 0, b);
        self.journal(b, t, JournalKind::Divorced, 0, a);
        // The one who moves out: the higher id. Children stay.
        let leaver = a.max(b);
        let wealth = self.agents.money[leaver as usize];
        if let Some(home) =
            Self::take_free_home(free, keyed(self.seed, leaver as u64, t as u64, P_MISC))
        {
            let hh = self.new_household(home, wealth, leaver as u64 ^ (t as u64) << 20);
            self.move_to_household(leaver, hh);
        } else {
            let parent_home = [
                self.agents.parent_a[leaver as usize],
                self.agents.parent_b[leaver as usize],
            ]
            .into_iter()
            .find(|p| *p != NONE && self.resident(*p))
            .map(|p| self.agents.household[p as usize]);
            if let Some(hh) = parent_home {
                self.move_to_household(leaver, hh);
            }
            // Otherwise they keep living in the same home for now.
        }
    }

    fn adopt(&mut self, a: AgentId, b: AgentId, t: Tick) {
        let hh = self.agents.household[a as usize];
        // An orphan from the town first; otherwise a child from outside the region.
        let orphan = (0..self.agents.len() as u32).find(|k| {
            self.resident(*k)
                && self.systems.demography.dflags[*k as usize] & dflags::ORPHAN != 0
                && self.age(*k, t) < 16
        });
        let child = match orphan {
            Some(k) => {
                self.move_to_household(k, hh);
                self.systems.demography.dflags[k as usize] &= !dflags::ORPHAN;
                k
            }
            None => {
                let c = content();
                let female = keyed(self.seed, a as u64, t as u64, P_ADOPT + 1).is_multiple_of(2);
                let st = &c.names.styles[self.style as usize];
                let given = generate(
                    &c.names,
                    if female { &st.female } else { &st.male },
                    keyed(self.seed, a as u64, t as u64, P_ADOPT + 2),
                );
                let age = in_range(keyed(self.seed, a as u64, t as u64, P_ADOPT + 3), 0, 9);
                let birth = t as i64
                    - age * TICKS_PER_YEAR as i64
                    - in_range(
                        keyed(self.seed, a as u64, t as u64, P_ADOPT + 4),
                        0,
                        TICKS_PER_YEAR as i64 - 1,
                    );
                let family = self.agents.family[a as usize];
                let k = self.create_person(
                    hh,
                    female,
                    birth,
                    &given,
                    family,
                    0,
                    if age >= 12 { EDU_PRIMARY } else { EDU_NONE },
                    t,
                );
                self.enrol_child(k, t);
                k
            }
        };
        self.agents.parent_a[child as usize] = a;
        self.agents.parent_b[child as usize] = b;
        self.systems.demography.dflags[child as usize] |= dflags::ADOPTED;
        self.journal(a, t, JournalKind::Adopted, 0, child);
        self.journal(b, t, JournalKind::Adopted, 0, child);
        self.journal(child, t, JournalKind::WasAdopted, 0, 0);
        self.systems.demography.today.adoptions += 1;
    }

    /// Children waiting for a daycare or school place get one (after a new one opens).
    pub(crate) fn enrol_waiting_children(&mut self, t: Tick) {
        let cat = catalog();
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) {
                continue;
            }
            let i = a as usize;
            let age = self.age(a, t);
            let waiting = ((1..6).contains(&age)
                && self.agents.schedule[i] == cat.schedule_toddler)
                || ((6..18).contains(&age) && self.agents.work[i] == NONE);
            if waiting {
                self.enrol_child(a, t);
                let w = self.agents.work[i];
                if w != NONE {
                    self.journal(a, t, JournalKind::StartedSchool, 0, w);
                }
            }
        }
    }

    /// Puts a child into daycare or school according to age.
    fn enrol_child(&mut self, k: AgentId, t: Tick) {
        let cat = catalog();
        let i = k as usize;
        let age = self.age(k, t);
        let home = self.agents.home[i];
        if (6..18).contains(&age) {
            self.agents.schedule[i] = cat.schedule_child;
            self.agents.work[i] = self.building_with_tag("school", home, true).unwrap_or(NONE);
        } else if (1..6).contains(&age) {
            if let Some(d) = self.building_with_tag("daycare", home, true) {
                self.agents.schedule[i] = cat.schedule_daycare;
                self.agents.work[i] = d;
            }
        }
    }

    /// Adult children leaving home, and long-term job seekers leaving the region.
    fn moving_out(&mut self, t: Tick, day: u64, people: &[AgentId]) {
        let dm = &content().demography;
        let cat = catalog();
        let mut free = self.free_homes();
        for &a in people {
            if !self.resident(a) {
                continue;
            }
            let i = a as usize;
            let age = self.age(a, t);
            let hh = self.agents.household[i];
            // Leaving the family home.
            if (22..=40).contains(&age)
                && self.agents.partner[i] == NONE
                && self.agents.profession[i] != u16::MAX
            {
                let with_parents = [self.agents.parent_a[i], self.agents.parent_b[i]]
                    .iter()
                    .any(|p| {
                        *p != NONE && self.resident(*p) && self.agents.household[*p as usize] == hh
                    });
                if with_parents && self.roll(a as u64, day, P_LEAVE, dm.leave_home_permille) {
                    if let Some(home) =
                        Self::take_free_home(&mut free, keyed(self.seed, a as u64, day, P_LEAVE))
                    {
                        let nh = self.new_household(
                            home,
                            self.agents.money[i],
                            a as u64 ^ (t as u64) << 20,
                        );
                        self.move_to_household(a, nh);
                        self.systems.demography.today.left_home += 1;
                        self.journal(a, t, JournalKind::LeftHome, 0, home);
                    }
                }
            }
            // Long-term job seekers may leave the region with their household.
            if self.agents.schedule[i] != cat.schedule_unemployed {
                continue;
            }
            let since = self
                .systems
                .demography
                .unemployed_since
                .get(&a)
                .copied()
                .unwrap_or(t);
            if t < since + dm.emigration_after_days as Tick * TICKS_PER_DAY
                || !self.roll(a as u64, day, P_EMIG, dm.emigration_permille)
            {
                continue;
            }
            let members = self.household_members(hh);
            // Only if nobody else in the household has a job keeping them here.
            if members
                .iter()
                .any(|m| *m != a && self.agents.profession[*m as usize] != u16::MAX)
            {
                continue;
            }
            let to_town = !self.gateways.is_empty()
                && keyed(self.seed, a as u64, day, P_EMIG + 1).is_multiple_of(2);
            if to_town {
                let dest = self.gateways[(keyed(self.seed, a as u64, day, P_EMIG + 2)
                    % self.gateways.len() as u64)
                    as usize]
                    .to_region;
                self.send_movers(&members, dest, t);
            } else {
                for m in members {
                    if self.resident(m) {
                        self.remove_person(m, t, Leaving::Emigration);
                    }
                }
            }
        }
    }

    fn send_movers(&mut self, members: &[AgentId], dest: u16, t: Tick) {
        let idx = |x: u32| {
            members
                .iter()
                .position(|m| *m == x)
                .map(|p| p as u8)
                .unwrap_or(u8::MAX)
        };
        let movers: Vec<Mover> = members
            .iter()
            .map(|&m| {
                let i = m as usize;
                Mover {
                    given: self.names.get(self.agents.given[i]).to_string(),
                    family: self.names.get(self.agents.family[i]).to_string(),
                    birth: self.agents.birth[i],
                    female: self.female(m),
                    traits: self.agents.traits[i],
                    money: self.agents.money[i],
                    education: self.systems.demography.education_of(m),
                    married: self.systems.demography.married(m),
                    partner: idx(self.agents.partner[i]),
                    parent_a: idx(self.agents.parent_a[i]),
                    parent_b: idx(self.agents.parent_b[i]),
                }
            })
            .collect();
        for &m in members {
            if self.resident(m) {
                self.remove_person(m, t, Leaving::MovedTown);
            }
        }
        self.send(dest, Body::Movers(movers));
    }

    /// A family arriving from another town (message `Movers`).
    pub(crate) fn receive_movers(&mut self, src: u16, movers: &[Mover]) {
        let t = self.tick;
        let mut free = self.free_homes();
        let wealth: i64 = movers.iter().map(|m| m.money).sum();
        let Some(home) =
            Self::take_free_home(&mut free, keyed(self.seed, src as u64, t as u64, P_IMMIG))
        else {
            // No home here: they move on, out of the region, taking their money with them.
            return;
        };
        let hh = self.new_household(home, wealth, (src as u64) << 32 ^ t as u64);
        let mut ids = Vec::new();
        for m in movers {
            let family = self.names.intern(&m.family);
            let a = self.create_person(hh, m.female, m.birth, &m.given, family, 0, m.education, t);
            self.agents.traits[a as usize] = m.traits;
            // The money arrives from the other town.
            self.transfer(Account::OtherRegions, Account::Agent(a), m.money);
            if m.married {
                self.systems.demography.dflags[a as usize] |= dflags::MARRIED;
            }
            ids.push(a);
        }
        for (k, m) in movers.iter().enumerate() {
            let a = ids[k] as usize;
            let get = |x: u8| ids.get(x as usize).copied().unwrap_or(NONE);
            self.agents.partner[a] = get(m.partner);
            self.agents.parent_a[a] = get(m.parent_a);
            self.agents.parent_b[a] = get(m.parent_b);
            self.enrol_child(ids[k], t);
            self.journal(ids[k], t, JournalKind::MovedTown, self.id, 0);
        }
        self.systems.demography.today.moved_in += movers.len() as u32;
    }

    fn immigration(&mut self, t: Tick, day: u64) {
        let dm = &content().demography;
        let c = content();
        let mut free = self.free_homes();
        let homes: u32 = free.iter().map(|(_, n)| *n as u32).sum();
        for k in 0..homes.min(50) {
            if !self.roll(k as u64, day, P_IMMIG, dm.immigration_permille) {
                continue;
            }
            let key = keyed(self.seed, day, k as u64, P_IMMIG + 1);
            let Some(home) = Self::take_free_home(&mut free, key) else {
                break;
            };
            let st = &c.names.styles[self.style as usize];
            let family_name = generate(&c.names, &st.family, key);
            let family = self.names.intern(&family_name);
            let size = 1 + (key % 4) as usize;
            let hh = self.new_household(home, 0, key);
            let mut adults: Vec<AgentId> = Vec::new();
            for m in 0..size {
                let mk = keyed(key, m as u64, 0, 0);
                let adult = m < 2.min(size);
                let female = if m == 0 {
                    mk.is_multiple_of(2)
                } else if m == 1 {
                    !self.female(adults[0])
                } else {
                    mk.is_multiple_of(2)
                };
                let age = if adult {
                    in_range(mk, 24, 45)
                } else {
                    in_range(mk, 0, 14)
                };
                let birth = t as i64
                    - age * TICKS_PER_YEAR as i64
                    - in_range(mk >> 7, 0, TICKS_PER_YEAR as i64 - 1);
                let given = generate(
                    &c.names,
                    if female { &st.female } else { &st.male },
                    mk >> 3,
                );
                let edu = if adult { 1 + (mk % 3) as u8 } else { 0 };
                let money = if adult {
                    in_range(mk >> 11, 50_000, 800_000)
                } else {
                    0
                };
                let a = self.create_person(hh, female, birth, &given, family, money, edu, t);
                if adult {
                    adults.push(a);
                    self.journal(a, t, JournalKind::Immigrated, 0, 0);
                } else {
                    self.agents.parent_a[a as usize] = adults[0];
                    self.agents.parent_b[a as usize] = adults.get(1).copied().unwrap_or(NONE);
                    self.enrol_child(a, t);
                }
            }
            if adults.len() == 2 {
                let (a, b) = (adults[0], adults[1]);
                self.agents.partner[a as usize] = b;
                self.agents.partner[b as usize] = a;
                self.systems.demography.dflags[a as usize] |= dflags::MARRIED;
                self.systems.demography.dflags[b as usize] |= dflags::MARRIED;
            }
            self.systems.demography.today.immigrants += size as u32;
        }
    }

    fn track_unemployment(&mut self, t: Tick) {
        let cat = catalog();
        for a in 0..self.agents.len() as u32 {
            let unemployed =
                self.resident(a) && self.agents.schedule[a as usize] == cat.schedule_unemployed;
            let d = &mut self.systems.demography.unemployed_since;
            if unemployed {
                d.entry(a).or_insert(t);
            } else {
                d.remove(&a);
            }
        }
    }

    fn parental_benefits(&mut self, t: Tick) {
        let amount = content().economy.pension_per_day_cents;
        let on_leave: Vec<AgentId> = self
            .systems
            .demography
            .leave_until
            .iter()
            .filter(|(_, u)| **u > t)
            .map(|(a, _)| *a)
            .collect();
        self.systems.demography.leave_until.retain(|_, u| *u > t);
        for a in on_leave {
            if self.resident(a) {
                self.transfer(Account::Treasury, Account::Agent(a), amount);
            }
        }
    }

    // ------------------------------------------------------------------ leaving

    /// Removes a person from the region: death, emigration or a move to another town.
    fn remove_person(&mut self, a: AgentId, t: Tick, why: Leaving) {
        let i = a as usize;
        let age = self.age(a, t);
        let given = self.agents.given[i];
        let family_name = self.agents.family[i];
        let partner = self.agents.partner[i];
        let partner = if partner != NONE && self.resident(partner) {
            partner
        } else {
            NONE
        };
        let children: Vec<AgentId> = self.children_of(a);
        let parents: Vec<AgentId> = [self.agents.parent_a[i], self.agents.parent_b[i]]
            .into_iter()
            .filter(|p| *p != NONE && self.resident(*p))
            .collect();

        // Money.
        let money = self.agents.money[i];
        match why {
            Leaving::Death => {
                let heirs: Vec<AgentId> = if partner != NONE {
                    vec![partner]
                } else if !children.is_empty() {
                    children.clone()
                } else {
                    parents.clone()
                };
                if money > 0 && !heirs.is_empty() {
                    let share = money / heirs.len() as i64;
                    for (k, h) in heirs.iter().enumerate() {
                        let amount = if k == 0 {
                            money - share * (heirs.len() as i64 - 1)
                        } else {
                            share
                        };
                        self.transfer(Account::Agent(a), Account::Agent(*h), amount);
                        self.journal(
                            *h,
                            t,
                            JournalKind::Inherited,
                            0,
                            amount.clamp(0, u32::MAX as i64) as u32,
                        );
                    }
                } else if money > 0 {
                    self.transfer(Account::Agent(a), Account::Treasury, money);
                } else if money < 0 {
                    // Debts die with them: creditors outside the region absorb them.
                    self.transfer(Account::World, Account::Agent(a), -money);
                }
                // Businesses and rented-out homes pass to the heir (or the town).
                let heir = heirs.first().copied().unwrap_or(NONE);
                for f in self.systems.economy.firms.iter_mut() {
                    if f.owner == a {
                        f.owner = heir;
                    }
                }
                for tn in self.systems.economy.tenure.iter_mut() {
                    if tn.kind == TenureKind::PrivateLandlord && tn.landlord == a {
                        if heir == NONE {
                            tn.kind = TenureKind::Municipal;
                            tn.landlord = NONE;
                        } else {
                            tn.landlord = heir;
                        }
                    }
                }
                self.systems.demography.today.deaths += 1;
                let cemetery = self
                    .building_with_tag("cemetery", self.agents.home[i], false)
                    .unwrap_or(NONE);
                let mut mourners: Vec<AgentId> = Vec::new();
                mourners.extend(partner.ne(&NONE).then_some(partner));
                mourners.extend(children.iter().copied());
                mourners.extend(parents.iter().copied());
                for m in mourners {
                    self.journal(m, t, JournalKind::FamilyDeath, 0, given);
                    let _ = cemetery;
                    // Grief: social and fun drop for a while.
                    let n = &mut self.agents.needs[m as usize];
                    n[3] = n[3].saturating_sub(3000);
                    n[4] = n[4].saturating_sub(2000);
                }
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Obituary as u16,
                    subject: given,
                    n: (family_name << 8) | age.min(255),
                });
            }
            Leaving::Emigration => {
                self.transfer(Account::Agent(a), Account::World, money);
                self.systems.demography.today.emigrants += 1;
                for r in children.iter().chain(parents.iter()) {
                    if self.agents.household[*r as usize] != self.agents.household[i] {
                        self.journal(*r, t, JournalKind::FamilyLeft, 0, given);
                    }
                }
            }
            Leaving::MovedTown => {
                self.transfer(Account::Agent(a), Account::OtherRegions, money);
                self.systems.demography.today.moved_out += 1;
            }
        }

        // Job, studies and pending state.
        if let Some(job) = self.systems.demography.remote_jobs.remove(&a) {
            self.send(
                job.region,
                Body::WorkerLeft {
                    firm: job.firm,
                    agent: a,
                },
            );
        }
        let d = &mut self.systems.demography;
        d.illness.remove(&a);
        d.pregnancies.remove(&a);
        d.leave_until.remove(&a);
        d.unemployed_since.remove(&a);
        d.dating_since.remove(&a);
        if partner != NONE {
            d.dating_since.remove(&a.min(partner));
        }
        self.away.remove(&a);

        // Family links.
        if partner != NONE {
            self.agents.partner[partner as usize] = NONE;
            self.systems.demography.dflags[partner as usize] &= !dflags::MARRIED;
        }
        for k in 0..self.agents.len() {
            if self.agents.parent_a[k] == a {
                self.agents.parent_a[k] = NONE;
            }
            if self.agents.parent_b[k] == a {
                self.agents.parent_b[k] = NONE;
            }
            for f in self.systems.demography.friends[k].iter_mut() {
                if *f == a {
                    *f = NONE;
                }
            }
        }
        // Children left without a parent at home become orphans (until adopted).
        if why == Leaving::Death {
            for c in &children {
                let ci = *c as usize;
                let has_parent = [self.agents.parent_a[ci], self.agents.parent_b[ci]]
                    .iter()
                    .any(|p| *p != NONE && self.resident(*p));
                if !has_parent && self.age(*c, t) < 18 {
                    self.systems.demography.dflags[ci] |= dflags::ORPHAN;
                }
            }
        }

        // Free the slot.
        let hh = self.agents.household[i];
        if hh != NONE {
            let m = &mut self.households.members[hh as usize];
            *m = m.saturating_sub(1);
        }
        self.set_place(a, NONE);
        self.agents.release(a);
        self.demo_reset(a);
    }

    // ------------------------------------------------------------------ commuting

    /// After the labour market: pay commuters working here, and offer the remaining
    /// vacancies to the neighbouring towns.
    pub(crate) fn commuting_daily(&mut self, t: Tick) {
        let c = content();
        // Wages for yesterday's shifts of people from other towns.
        if t >= TICKS_PER_DAY {
            let yesterday_wd = weekday(t - TICKS_PER_DAY);
            let staff = self.systems.demography.remote_staff.clone();
            for s in staff {
                let worked = c.schedules[s.schedule as usize]
                    .blocks
                    .iter()
                    .any(|b| b.days.contains(yesterday_wd));
                if !worked || (s.firm as usize) >= self.systems.economy.firms.len() {
                    continue;
                }
                let wage = c.professions[s.profession as usize].wage_per_shift_cents;
                let tax = wage * self.systems.economy.income_tax_permille as i64 / 1000;
                let public = self.systems.economy.firms[s.firm as usize].kind == FirmKind::Public;
                let payer = if public {
                    Account::Treasury
                } else {
                    Account::Firm(s.firm)
                };
                if !public {
                    self.transfer(payer, Account::Treasury, tax);
                    self.systems.economy.firms[s.firm as usize].costs_today += wage;
                }
                self.transfer(payer, Account::OtherRegions, wage - tax);
                self.send(
                    s.region,
                    Body::RemoteWage {
                        agent: s.agent,
                        net: wage - tax,
                    },
                );
            }
        }
        // Vacancies nobody local took: offer them to neighbours.
        if self.gateways.is_empty() {
            return;
        }
        let mut offers = Vec::new();
        let mut staff_at = vec![0u32; self.buildings.len()];
        for a in 0..self.agents.len() {
            let w = self.agents.work[a];
            if w != NONE && self.agents.profession[a] != u16::MAX {
                staff_at[w as usize] += 1;
            }
        }
        for f in 0..self.systems.economy.firms.len() {
            let fm = &self.systems.economy.firms[f];
            if !fm.open {
                continue;
            }
            // Same rule as local hiring: profitable firms fill their positions, others only
            // what their demand justifies.
            let limit = if !fm.is_private() || fm.profit_avg >= 0 {
                fm.positions as usize
            } else {
                self.desired_staff(f as u32).min(fm.positions as u32) as usize
            };
            let b = fm.building;
            let def = &c.buildings[self.buildings.def[b as usize] as usize];
            let staff = staff_at[b as usize] as usize + self.remote_staff_of(f as u32);
            let free = limit.saturating_sub(staff);
            if free == 0 {
                continue;
            }
            if let Some(js) = def.jobs.first() {
                offers.push((
                    f as u32,
                    js.profession,
                    free.min(5) as u16,
                    self.building_name(b),
                ));
            }
            if offers.len() >= 20 {
                break;
            }
        }
        if offers.is_empty() {
            return;
        }
        let regions: Vec<u16> = self.gateways.iter().map(|g| g.to_region).collect();
        for r in regions {
            self.send(r, Body::JobOffers(offers.clone()));
        }
    }

    /// Job offers from another town: local job seekers who qualify accept them.
    pub(crate) fn receive_job_offers(&mut self, src: u16, offers: &[(u32, u16, u16, String)]) {
        let c = content();
        let cat = catalog();
        let t = self.tick;
        let Some(gw) = self
            .gateways
            .iter()
            .find(|g| g.to_region == src)
            .map(|g| g.building)
        else {
            return;
        };
        let town = self
            .gateways
            .iter()
            .find(|g| g.to_region == src)
            .map(|g| g.to_name.clone())
            .unwrap_or_default();
        let mut seekers: Vec<AgentId> = (0..self.agents.len() as u32)
            .filter(|a| {
                self.resident(*a)
                    && self.agents.schedule[*a as usize] == cat.schedule_unemployed
                    && (18..=65).contains(&self.age(*a, t))
            })
            .collect();
        for (firm, prof, count, place) in offers {
            let pd = &c.professions[*prof as usize];
            for _ in 0..*count {
                let Some(k) = seekers.iter().position(|a| {
                    self.age(*a, t) >= pd.min_age as u32
                        && self.systems.demography.education_of(*a) >= pd.education
                }) else {
                    break;
                };
                let a = seekers.remove(k);
                let i = a as usize;
                self.agents.work[i] = gw;
                self.agents.profession[i] = *prof;
                self.agents.schedule[i] = pd.schedules[0];
                self.systems.demography.remote_jobs.insert(
                    a,
                    RemoteJob {
                        region: src,
                        firm: *firm,
                        place: place.clone(),
                        profession: *prof,
                    },
                );
                self.journal(a, t, JournalKind::CommuteJob, src, 0);
                let _ = &town;
                self.send(
                    src,
                    Body::JobAccept {
                        firm: *firm,
                        agent: a,
                        profession: *prof,
                        schedule: pd.schedules[0],
                    },
                );
            }
        }
    }

    pub(crate) fn receive_job_accept(
        &mut self,
        src: u16,
        firm: u32,
        agent: u32,
        profession: u16,
        schedule: u16,
    ) {
        let ok = (firm as usize) < self.systems.economy.firms.len()
            && self.systems.economy.firms[firm as usize].open;
        if ok {
            self.systems.demography.remote_staff.push(RemoteStaff {
                region: src,
                agent,
                firm,
                profession,
                schedule,
            });
        } else {
            self.send(src, Body::JobEnded { firm, agent });
        }
    }

    /// A worker from another town quit one of our jobs.
    pub(crate) fn receive_worker_left(&mut self, src: u16, firm: u32, agent: u32) {
        self.systems
            .demography
            .remote_staff
            .retain(|s| !(s.region == src && s.agent == agent && s.firm == firm));
    }

    /// The town where our commuter works ended their job (only that town sends this).
    pub(crate) fn receive_job_ended(&mut self, src: u16, firm: u32, agent: u32) {
        if self
            .systems
            .demography
            .remote_jobs
            .get(&agent)
            .is_some_and(|j| j.region == src && j.firm == firm)
        {
            self.systems.demography.remote_jobs.remove(&agent);
            let i = agent as usize;
            if self.resident(agent) {
                self.agents.work[i] = NONE;
                self.agents.profession[i] = u16::MAX;
                self.agents.schedule[i] = catalog().schedule_unemployed;
                let t = self.tick;
                self.journal(agent, t, JournalKind::LaidOff, 0, NONE);
            }
        }
    }

    pub(crate) fn receive_remote_wage(&mut self, agent: u32, net: i64) {
        if self.resident(agent) && self.systems.demography.remote_jobs.contains_key(&agent) {
            self.transfer(Account::OtherRegions, Account::Agent(agent), net);
            let t = self.tick;
            self.journal(
                agent,
                t,
                JournalKind::GotPaid,
                0,
                net.clamp(0, u32::MAX as i64) as u32,
            );
        } else {
            // They left: the money goes to the town.
            self.transfer(Account::OtherRegions, Account::Treasury, net);
        }
    }

    /// Firm `f` let go of everyone (bankruptcy): tell the commuters' towns.
    pub(crate) fn end_remote_staff(&mut self, f: u32) {
        let gone: Vec<RemoteStaff> = self
            .systems
            .demography
            .remote_staff
            .iter()
            .filter(|s| s.firm == f)
            .cloned()
            .collect();
        self.systems.demography.remote_staff.retain(|s| s.firm != f);
        for s in gone {
            self.send(
                s.region,
                Body::JobEnded {
                    firm: s.firm,
                    agent: s.agent,
                },
            );
        }
    }

    /// A commuter arriving at the highway exit on the way to work: they are away until
    /// the end of the shift plus the drive back.
    pub(crate) fn commute_away(&mut self, a: AgentId, t: Tick, shift_end: Tick) -> bool {
        let Some(job) = self.systems.demography.remote_jobs.get(&a).cloned() else {
            return false;
        };
        let place = self.agents.place[a as usize];
        let Some(gi) = self
            .gateways
            .iter()
            .position(|g| g.building == place && g.to_region == job.region)
        else {
            return false;
        };
        let hw = self.gateways[gi].highway_ticks;
        let return_at = shift_end.max(t + 60) + hw;
        self.journal(a, t, JournalKind::WorkedAway, job.region, 0);
        self.set_place(a, NONE);
        self.agents.act[a as usize] = ACT_AWAY;
        self.agents.until[a as usize] = return_at;
        self.away.insert(
            a,
            AwayInfo {
                region: job.region,
                gateway: gi as u16,
                return_at,
            },
        );
        self.set_event(a, return_at);
        true
    }

    // ------------------------------------------------------------------ news & history

    fn demography_news(&mut self, t: Tick) {
        let id = self.id as u32;
        let y = self.systems.demography.yesterday.clone();
        if y.births > 0 {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::Births as u16,
                subject: id,
                n: y.births,
            });
        }
        {
            let w = &mut self.systems.demography.week;
            w.immigrants += y.immigrants + y.moved_in;
            w.emigrants += y.emigrants + y.moved_out;
        }
        if date(t).day == 1 && t >= TICKS_PER_DAY {
            let pop = self.population();
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::Census as u16,
                subject: id,
                n: pop,
            });
        }
        if date(t).weekday == 0 && t >= TICKS_PER_DAY {
            let w = std::mem::take(&mut self.systems.demography.week);
            if w.immigrants + w.emigrants > 0 {
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Migration as u16,
                    subject: w.emigrants,
                    n: w.immigrants,
                });
            }
            let commuters = self.systems.demography.remote_staff.len() as u32;
            if commuters > 0 {
                let from = self.systems.demography.remote_staff[0].region as u32;
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Commuters as u16,
                    subject: from,
                    n: commuters,
                });
            }
        }
    }

    fn sample_demography(&mut self, t: Tick) {
        let d = &self.systems.demography;
        let s = DemoSample {
            tick: t,
            population: self.population(),
            births: d.yesterday.births,
            deaths: d.yesterday.deaths,
            marriages: d.yesterday.marriages,
            immigrants: d.yesterday.immigrants + d.yesterday.moved_in,
            emigrants: d.yesterday.emigrants + d.yesterday.moved_out,
            ill: d.illness.len() as u32,
            commuters_out: d.remote_jobs.len() as u32,
            commuters_in: d.remote_staff.len() as u32,
        };
        let h = &mut self.systems.demography.history;
        h.push(s);
        if h.len() > HISTORY_DAYS {
            h.remove(0);
        }
    }
}
