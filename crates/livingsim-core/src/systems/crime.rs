//! Phase 5: crime. Offences arise from personality (honesty, temper), money troubles, a
//! criminal record and recent conflicts. They are always represented non-graphically:
//! icons, police tape, the newspaper and people's diaries.
//!
//! The day's offences are planned every morning (who, what and at which hour) and carried
//! out on the hour. The investigation, arrests, trials and jail live in `justice.rs`.

use super::demography::{Illness, Leaving};
use super::economy::{Account, TenureKind};
use super::DailyContext;
use crate::agents::{traits, AgentId};
use crate::buildings::BuildingId;
use crate::catalog::content;
use crate::map::{MARGIN, PITCH};
use crate::region::{NewsItem, Region};
use crate::rng::keyed;
use crate::roads::NONE;
use crate::time::{age_years, date, Tick, TICKS_PER_DAY};
use livingsim_content::{CrimeKind, FirmKind, JournalKind, NewsKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Days of incidents kept after their case is closed (for the UI and the map).
pub const INCIDENTS_KEPT_DAYS: Tick = 30;
pub const HISTORY_DAYS: usize = 60;

// Keys for `keyed` (purpose tags).
const K_ROLL: u64 = 0xC1;
const K_HOUR: u64 = 0xC2;
const K_TARGET: u64 = 0xC3;
const K_LOSS: u64 = 0xC4;
const K_WITNESS: u64 = 0xC5;
const K_HURT: u64 = 0xC6;
const K_MOVE: u64 = 0xC7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Under investigation.
    Open,
    /// Enough evidence: the suspect will be arrested.
    Solved,
    /// Suspect arrested, awaiting trial.
    Arrested,
    Convicted,
    Acquitted,
    /// Punished with a fine (minor offences).
    Fined,
    /// Unsolved and closed.
    Cold,
}

impl Status {
    pub fn closed(self) -> bool {
        matches!(
            self,
            Status::Convicted | Status::Acquitted | Status::Fined | Status::Cold
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Incident {
    pub id: u32,
    pub crime: u16,
    pub tick: Tick,
    pub building: BuildingId,
    pub offender: AgentId,
    /// Victim (agent), or `NONE`.
    pub victim: AgentId,
    /// Victim business (shoplifting, fraud), or `NONE`.
    pub victim_firm: u32,
    /// Money taken (cents) or goods taken (meals, for shoplifting).
    pub loss: i64,
    pub witnesses: u16,
    pub evidence: u32,
    /// The case has leads; without them it can never be solved.
    pub lead: bool,
    pub status: Status,
    /// Trial date while arrested; sentence in days once convicted.
    pub trial_at: Tick,
    pub sentence_days: u32,
}

/// An offence planned for later today.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Planned {
    pub tick: Tick,
    pub offender: AgentId,
    pub crime: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrimeTotals {
    /// Incidents by crime (content index).
    pub by_crime: Vec<u32>,
    pub solved: u32,
    pub arrests: u32,
    pub convictions: u32,
    pub acquittals: u32,
    pub fines: u32,
    pub cold: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrimeSample {
    pub tick: Tick,
    pub incidents: u32,
    pub solved: u32,
    pub inmates: u32,
    /// Average block safety (‰).
    pub safety: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CrimeState {
    pub incidents: Vec<Incident>,
    pub planned: Vec<Planned>,
    pub next_id: u32,
    /// Convictions per person (raises the chance of reoffending).
    pub records: BTreeMap<AgentId, u8>,
    /// Safety per street block (row-major over the construction frame), 0..=1000.
    pub safety: Vec<u16>,
    pub today: CrimeTotals,
    pub yesterday: CrimeTotals,
    pub total: CrimeTotals,
    pub history: Vec<CrimeSample>,
}

impl CrimeState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // Crime needs the whole region; see `Region::crime_daily`.
    }
    pub fn incident(&self, id: u32) -> Option<&Incident> {
        self.incidents.iter().find(|x| x.id == id)
    }
}

fn add(v: &mut Vec<u32>, i: usize) {
    if v.len() <= i {
        v.resize(i + 1, 0);
    }
    v[i] += 1;
}

fn in_range(h: u64, lo: i64, hi: i64) -> i64 {
    if hi <= lo {
        lo
    } else {
        lo + (h % (hi - lo + 1) as u64) as i64
    }
}

impl Region {
    // ------------------------------------------------------------------ safety

    /// Street block of a building (from its door), in the construction frame.
    fn block_of(&self, b: BuildingId) -> Option<usize> {
        let nb = self.systems.construction.nb as i32;
        if nb == 0 || b as usize >= self.buildings.len() {
            return None;
        }
        let bx = (self.buildings.x[b as usize] as i32 - MARGIN as i32) / PITCH as i32;
        let by = (self.buildings.y[b as usize] as i32 - MARGIN as i32) / PITCH as i32;
        ((0..nb).contains(&bx) && (0..nb).contains(&by)).then_some((by * nb + bx) as usize)
    }

    fn ensure_safety(&mut self) {
        let nb = self.systems.construction.nb as usize;
        let s = &mut self.systems.crime.safety;
        if s.len() != nb * nb {
            *s = vec![1000; nb * nb];
        }
    }

    /// How safe the block of a building feels (‰, 1000 = perfectly safe).
    pub fn safety_at(&self, b: BuildingId) -> u16 {
        self.block_of(b)
            .and_then(|k| self.systems.crime.safety.get(k).copied())
            .unwrap_or(1000)
    }

    /// Rent factor (‰) for a home in a block with this safety.
    pub(crate) fn safety_rent_factor(&self, b: BuildingId) -> i64 {
        let j = &content().justice;
        1000 - j.rent_penalty_max_permille as i64 * (1000 - self.safety_at(b) as i64) / 1000
    }

    // ------------------------------------------------------------------ motive

    /// Multiplier (‰) on the base rate of offence `crime` for agent `a`.
    fn motive(&self, a: AgentId, crime: u16, t: Tick) -> u64 {
        let c = content();
        let j = &c.justice;
        let kind = c.crimes[crime as usize].kind;
        let i = a as usize;
        let tr = self.agents.traits[i];
        // Honesty 0..15: 8 is average.
        let mut m: u64 = (16 - traits::get(tr, traits::HONESTY).min(15)) as u64 * 1000 / 8;
        let age = age_years(self.agents.birth[i], t);
        m = m * match age {
            16..=29 => 1500,
            30..=49 => 1000,
            50..=69 => 450,
            _ => 120,
        } / 1000;
        if self.systems.crime.records.contains_key(&a) {
            m = m * j.record_multiplier_permille as u64 / 1000;
        }
        match kind {
            CrimeKind::Shoplifting | CrimeKind::Burglary | CrimeKind::Fraud => {
                let money = self.agents.money[i];
                let jobless = self.agents.profession[i] == u16::MAX;
                if money < 0 || (jobless && money < 30_000) {
                    m = m * j.broke_multiplier_permille as u64 / 1000;
                }
                let hh = self.agents.household[i];
                let behind = self
                    .systems
                    .economy
                    .tenure
                    .get(hh as usize)
                    .is_some_and(|tn| tn.arrears > 0 && tn.kind != TenureKind::Owner);
                if behind {
                    m = m * j.arrears_multiplier_permille as u64 / 1000;
                }
            }
            CrimeKind::Assault | CrimeKind::Homicide => {
                // Temper 0..15: 7 is average.
                m = m * (traits::get(tr, traits::TEMPER) as u64 + 1) / 8;
                let laid_off = self.systems.demography.unemployed_since.contains_key(&a);
                let split = self.systems.demography.married(a) && self.agents.partner[i] == NONE;
                if laid_off || split {
                    m = m * j.conflict_multiplier_permille as u64 / 1000;
                }
            }
        }
        m
    }

    // ------------------------------------------------------------------ daily

    pub(crate) fn crime_daily(&mut self, t: Tick) {
        self.ensure_safety();
        let c = content();
        let j = &c.justice;
        let day = (t / TICKS_PER_DAY) as u64;
        {
            let cr = &mut self.systems.crime;
            cr.yesterday = std::mem::take(&mut cr.today);
            let y = cr.yesterday.clone();
            for (k, n) in y.by_crime.iter().enumerate() {
                for _ in 0..*n {
                    add(&mut cr.total.by_crime, k);
                }
            }
            cr.total.solved += y.solved;
            cr.total.arrests += y.arrests;
            cr.total.convictions += y.convictions;
            cr.total.acquittals += y.acquittals;
            cr.total.fines += y.fines;
            cr.total.cold += y.cold;
            for s in cr.safety.iter_mut() {
                *s = (*s + j.safety_recovery_per_day).min(1000);
            }
            // Cold cases.
            for x in cr.incidents.iter_mut() {
                let cold_after = c.crimes[x.crime as usize].cold_after_days as Tick;
                if x.status == Status::Open && x.tick + cold_after * TICKS_PER_DAY <= t {
                    x.status = Status::Cold;
                    cr.today.cold += 1;
                }
            }
            cr.incidents
                .retain(|x| !x.status.closed() || x.tick + INCIDENTS_KEPT_DAYS * TICKS_PER_DAY > t);
            cr.planned.clear();
        }
        self.justice_daily(t);

        // Plan today's offences: at most one per person.
        let mut planned = Vec::new();
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a)
                || age_years(self.agents.birth[a as usize], t) < 16
                || self.systems.justice.inmates.contains_key(&a)
            {
                continue;
            }
            for (k, cd) in c.crimes.iter().enumerate() {
                let rate = cd.rate_per_100m_days as u64 * self.motive(a, k as u16, t) / 1000;
                if keyed(self.seed, a as u64, (day << 8) | k as u64, K_ROLL) % 100_000_000 >= rate {
                    continue;
                }
                let (from, to) = (cd.hours.0 as u64, cd.hours.1 as u64);
                let span = (to + 24 - from) % 24;
                let hour = (from + keyed(self.seed, a as u64, day, K_HOUR) % span.max(1)) % 24;
                // Hours after 06:00 (the start of the simulated day).
                let offset = (hour + 24 - 6) % 24;
                planned.push(Planned {
                    tick: t + offset as Tick * 60,
                    offender: a,
                    crime: k as u16,
                });
                break;
            }
        }
        planned.sort_by_key(|p| (p.tick, p.offender));
        self.systems.crime.planned = planned;

        // Average safety of the blocks where people live.
        let safety = {
            let mut homes: Vec<usize> = (0..self.buildings.len() as u32)
                .filter(|b| c.buildings[self.buildings.def[*b as usize] as usize].dwellings > 0)
                .filter_map(|b| self.block_of(b))
                .collect();
            homes.sort_unstable();
            homes.dedup();
            let s = &self.systems.crime.safety;
            let v: Vec<u32> = homes
                .iter()
                .filter_map(|k| s.get(*k))
                .map(|x| *x as u32)
                .collect();
            if v.is_empty() {
                1000
            } else {
                v.iter().sum::<u32>() / v.len() as u32
            }
        };
        let sample = CrimeSample {
            tick: t,
            incidents: self.systems.crime.yesterday.by_crime.iter().sum(),
            solved: self.systems.crime.yesterday.solved,
            inmates: self.systems.justice.inmates.len() as u32
                + self.systems.justice.guests.len() as u32,
            safety,
        };
        let h = &mut self.systems.crime.history;
        h.push(sample);
        if h.len() > HISTORY_DAYS {
            h.remove(0);
        }
    }

    /// Every hour: offences due now, arrests, trials and releases.
    pub(crate) fn crime_hourly(&mut self, t: Tick) {
        let due: Vec<Planned> = {
            let p = &mut self.systems.crime.planned;
            let n = p.iter().take_while(|x| x.tick <= t).count();
            p.drain(..n).collect()
        };
        for p in due {
            if self.resident(p.offender)
                && !self.systems.justice.inmates.contains_key(&p.offender)
                && !self.away.contains_key(&p.offender)
            {
                self.commit(p.offender, p.crime, t);
            }
        }
        self.justice_hourly(t);
    }

    // ------------------------------------------------------------------ offences

    fn commit(&mut self, a: AgentId, crime: u16, t: Tick) {
        let c = content();
        let cd = &c.crimes[crime as usize];
        let key = ((t as u64) << 20) ^ a as u64;
        let mut inc = Incident {
            id: self.systems.crime.next_id,
            crime,
            tick: t,
            building: NONE,
            offender: a,
            victim: NONE,
            victim_firm: NONE,
            loss: 0,
            witnesses: 0,
            evidence: 0,
            lead: keyed(self.seed, key, 8, K_WITNESS) % 1000 < cd.clearance_permille as u64,
            status: Status::Open,
            trial_at: 0,
            sentence_days: 0,
        };
        let (lo, hi) = cd.loss_cents;
        let wanted = in_range(keyed(self.seed, key, 1, K_LOSS), lo, hi);
        match cd.kind {
            CrimeKind::Shoplifting => {
                let shops: Vec<u32> = self
                    .systems
                    .economy
                    .firms
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.kind == FirmKind::Shop && f.open && f.stock > 0)
                    .map(|(i, _)| i as u32)
                    .collect();
                if shops.is_empty() {
                    return;
                }
                let f = shops[(keyed(self.seed, key, 2, K_TARGET) % shops.len() as u64) as usize];
                let firm = &mut self.systems.economy.firms[f as usize];
                let price = firm.price.max(1);
                let meals = ((wanted / price).max(1) as u32).min(firm.stock);
                firm.stock -= meals;
                let b = firm.building;
                let hh = self.agents.household[a as usize];
                if let Some(food) = self.households.food.get_mut(hh as usize) {
                    *food = food.saturating_add(meals as u16);
                }
                inc.building = b;
                inc.victim_firm = f;
                inc.loss = meals as i64;
            }
            CrimeKind::Burglary => {
                let own = self.agents.home[a as usize];
                let n = self.households.len() as u64;
                if n == 0 {
                    return;
                }
                // A few tries to find an occupied home that is not the burglar's.
                let mut target = None;
                for k in 0..8 {
                    let h = (keyed(self.seed, key, k, K_TARGET) % n) as usize;
                    if self.households.members[h] > 0 && self.households.home[h] != own {
                        target = Some(h as u32);
                        break;
                    }
                }
                let Some(hh) = target else { return };
                let members: Vec<AgentId> = (0..self.agents.len() as u32)
                    .filter(|m| self.resident(*m) && self.agents.household[*m as usize] == hh)
                    .collect();
                let Some(&victim) = members
                    .iter()
                    .max_by_key(|m| (self.agents.money[**m as usize], **m))
                else {
                    return;
                };
                let money = self.agents.money[victim as usize];
                let taken = wanted.min(money / 4).max(0);
                if taken > 0 {
                    self.transfer(Account::Agent(victim), Account::Agent(a), taken);
                }
                inc.building = self.households.home[hh as usize];
                inc.victim = victim;
                inc.loss = taken;
                for m in members {
                    self.journal(
                        m,
                        t,
                        JournalKind::Burgled,
                        0,
                        taken.clamp(0, u32::MAX as i64) as u32,
                    );
                    self.shaken(m, 2500);
                }
                // Some families move away after a burglary.
                let roll = keyed(self.seed, key, 3, K_MOVE) % 1000;
                if roll < c.justice.move_after_burglary_permille as u64 {
                    self.move_household_after_crime(hh, t, key);
                }
            }
            CrimeKind::Assault | CrimeKind::Homicide => {
                let Some(victim) = self.acquaintance(a, key, t) else {
                    return;
                };
                let place = match self.agents.place[victim as usize] {
                    NONE => self.agents.home[victim as usize],
                    p => p,
                };
                inc.building = place;
                inc.victim = victim;
                if cd.kind == CrimeKind::Assault {
                    let days = 2 + keyed(self.seed, key, 4, K_HURT) % 9;
                    let severe = keyed(self.seed, key, 5, K_HURT) % 1000 < 200;
                    self.systems.demography.illness.insert(
                        victim,
                        Illness {
                            until: t + days as Tick * TICKS_PER_DAY,
                            severe,
                        },
                    );
                    self.journal(victim, t, JournalKind::Assaulted, 0, place);
                    self.shaken(victim, 3000);
                    let act = self.agents.act[victim as usize];
                    if act != crate::agents::ACT_AWAY && act != crate::agents::ACT_TRAVEL {
                        self.set_event(victim, t);
                    }
                } else {
                    let given = self.agents.given[victim as usize];
                    let family = self.agents.family[victim as usize];
                    self.remove_person(victim, t, Leaving::Death);
                    self.push_news(NewsItem {
                        tick: t,
                        kind: NewsKind::Homicide as u16,
                        subject: given,
                        n: family,
                    });
                }
            }
            CrimeKind::Fraud => {
                // A well-off resident or a private business.
                let n = self.agents.len() as u64;
                let mut best: Option<(i64, AgentId)> = None;
                for k in 0..12 {
                    let v = (keyed(self.seed, key, k, K_TARGET) % n.max(1)) as u32;
                    if v != a
                        && self.resident(v)
                        && age_years(self.agents.birth[v as usize], t) >= 18
                    {
                        let m = self.agents.money[v as usize];
                        if best.is_none_or(|(bm, _)| m > bm) {
                            best = Some((m, v));
                        }
                    }
                }
                let Some((money, victim)) = best else { return };
                let taken = wanted.min(money / 3).max(0);
                if taken == 0 {
                    return;
                }
                self.transfer(Account::Agent(victim), Account::Agent(a), taken);
                self.journal(
                    victim,
                    t,
                    JournalKind::Defrauded,
                    0,
                    taken.clamp(0, u32::MAX as i64) as u32,
                );
                inc.building = self.agents.home[victim as usize];
                inc.victim = victim;
                inc.loss = taken;
            }
        }
        if inc.building == NONE {
            return;
        }
        // Witnesses: people in the building at the time (fewer at night).
        let j = &c.justice;
        let chance = if cd.kind == CrimeKind::Burglary {
            100
        } else {
            350
        };
        let mut witnesses = 0u16;
        for w in self.occupants(inc.building) {
            if witnesses >= j.max_witnesses {
                break;
            }
            if w == a || w == inc.victim || !self.resident(w) {
                continue;
            }
            if keyed(self.seed, key ^ w as u64, 6, K_WITNESS) % 1000 < chance {
                witnesses += 1;
                self.journal(w, t, JournalKind::Witnessed, 0, inc.building);
                self.shaken(w, 1000);
            }
        }
        inc.witnesses = witnesses;
        inc.evidence = witnesses as u32 * j.points_per_witness + cd.evidence / 5;
        if inc.lead && inc.evidence >= cd.evidence {
            inc.status = Status::Solved;
            self.systems.crime.today.solved += 1;
        }
        // The neighbourhood feels less safe.
        if let Some(k) = self.block_of(inc.building) {
            self.ensure_safety();
            let s = &mut self.systems.crime.safety[k];
            *s = s.saturating_sub(j.safety_loss_per_severity * cd.severity as u16);
        }
        if cd.severity >= 4 && cd.kind != CrimeKind::Homicide {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::CrimeReport as u16,
                subject: inc.building,
                n: crime as u32,
            });
        }
        let cr = &mut self.systems.crime;
        add(&mut cr.today.by_crime, crime as usize);
        cr.next_id += 1;
        cr.incidents.push(inc);
    }

    /// Someone the offender knows: a friend, their partner, a coworker or a neighbour.
    fn acquaintance(&self, a: AgentId, key: u64, t: Tick) -> Option<AgentId> {
        let i = a as usize;
        let mut pool: Vec<AgentId> = Vec::new();
        if let Some(fr) = self.systems.demography.friends.get(i) {
            pool.extend(fr.iter().copied().filter(|f| *f != NONE));
        }
        let p = self.agents.partner[i];
        if p != NONE {
            pool.push(p);
        }
        let work = self.agents.work[i];
        let home = self.agents.home[i];
        for k in 0..16 {
            let v = (keyed(self.seed, key, k, K_TARGET) % self.agents.len().max(1) as u64) as u32;
            let vi = v as usize;
            if (work != NONE && self.agents.work[vi] == work) || self.agents.home[vi] == home {
                pool.push(v);
            }
        }
        pool.retain(|v| {
            *v != a && self.resident(*v) && age_years(self.agents.birth[*v as usize], t) >= 16
        });
        pool.retain(|v| {
            !self.systems.justice.inmates.contains_key(v) && !self.away.contains_key(v)
        });
        pool.sort_unstable();
        pool.dedup();
        if pool.is_empty() {
            return None;
        }
        Some(pool[(keyed(self.seed, key, 99, K_TARGET) % pool.len() as u64) as usize])
    }

    /// A frightening experience: the safety need drops.
    fn shaken(&mut self, a: AgentId, amount: u16) {
        let n = &mut self.agents.needs[a as usize][livingsim_content::Need::Safety as usize];
        *n = n.saturating_sub(amount);
    }

    fn move_household_after_crime(&mut self, hh: u32, t: Tick, key: u64) {
        let old = self.households.home[hh as usize];
        let mut free = self.free_homes();
        free.retain(|(b, _)| *b != old && self.safety_at(*b) > self.safety_at(old));
        let Some(home) = Region::take_free_home(&mut free, keyed(self.seed, key, 7, K_MOVE)) else {
            return;
        };
        let members: Vec<AgentId> = (0..self.agents.len() as u32)
            .filter(|m| self.resident(*m) && self.agents.household[*m as usize] == hh)
            .collect();
        let wealth: i64 = members.iter().map(|m| self.agents.money[*m as usize]).sum();
        let nh = self.new_household(home, wealth, key ^ 0x5AFE);
        let food = self.households.food[hh as usize];
        self.households.food[nh as usize] = food;
        self.households.food[hh as usize] = 0;
        for m in members {
            self.move_to_household(m, nh);
            self.journal(m, t, JournalKind::MovedAfterCrime, 0, home);
        }
    }

    /// Recent incidents for the map: building, crime, tick, status (as u32).
    pub fn recent_incidents(&self, since: Tick) -> Vec<&Incident> {
        self.systems
            .crime
            .incidents
            .iter()
            .filter(|x| x.tick >= since)
            .collect()
    }

    /// Hour of the day of a tick (helper for the UI).
    pub fn hour_of(t: Tick) -> u8 {
        date(t).hour
    }
}
