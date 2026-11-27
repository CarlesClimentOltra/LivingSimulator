//! Inhabitants, stored as a structure of arrays. `AgentId` is a dense `u32` index inside
//! its region. Free slots are reused lowest-first (deterministic).

use crate::journal::Journal;
use crate::names::NameRef;
use crate::roads::NONE;
use crate::time::Tick;
use livingsim_content::NEED_COUNT;
use serde::{Deserialize, Serialize};

pub type AgentId = u32;

/// Special values of `act` (otherwise an index into `content.activities`).
pub const ACT_TRAVEL: u16 = 0xFFFE;
pub const ACT_AWAY: u16 = 0xFFFD;
pub const ACT_FREE_SLOT: u16 = 0xFFFF;
/// "Pseudo activity" used as `next_act` for a visitor heading home.
pub const ACT_DEPART: u16 = 0xFFFC;

pub mod flags {
    pub const ALIVE: u8 = 1;
    pub const VISITOR: u8 = 2;
    pub const FEMALE: u8 = 4;
    /// The current trip is by car.
    pub const BY_CAR: u8 = 8;
}

/// Personality traits, 4 bits each (0..=15).
pub mod traits {
    pub const SOCIABLE: u32 = 0;
    pub const ACTIVE: u32 = 1;
    pub const HOMEBODY: u32 = 2;
    pub const THRIFTY: u32 = 3;
    pub const NIGHT_OWL: u32 = 4;
    pub const PUNCTUAL: u32 = 5;
    /// Used from Phase 5 (crime).
    pub const TEMPER: u32 = 6;
    pub const HONESTY: u32 = 7;

    #[inline]
    pub fn get(packed: u32, t: u32) -> u32 {
        (packed >> (t * 4)) & 0xF
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Agents {
    // ---- hot: touched at every decision
    pub act: Vec<u16>,
    pub until: Vec<Tick>,
    /// Building the agent is in, or `NONE` while travelling / away.
    pub place: Vec<u32>,
    pub next_event: Vec<Tick>,
    pub needs: Vec<[u16; NEED_COUNT]>,
    pub needs_t: Vec<Tick>,
    // ---- current trip (valid while `act == ACT_TRAVEL`)
    pub trip_from: Vec<u32>,
    pub trip_to: Vec<u32>,
    pub trip_depart: Vec<Tick>,
    pub next_act: Vec<u16>,
    pub plan_until: Vec<Tick>,
    // ---- warm
    pub home: Vec<u32>,
    /// Workplace or school building.
    pub work: Vec<u32>,
    pub schedule: Vec<u16>,
    /// Index into `content.professions`, or `u16::MAX`.
    pub profession: Vec<u16>,
    pub household: Vec<u32>,
    pub money: Vec<i64>,
    pub traits: Vec<u32>,
    pub flags: Vec<u8>,
    // ---- cold
    /// Birth tick (negative = before the world started).
    pub birth: Vec<i64>,
    pub given: Vec<NameRef>,
    pub family: Vec<NameRef>,
    pub partner: Vec<u32>,
    pub parent_a: Vec<u32>,
    pub parent_b: Vec<u32>,
    pub journal: Journal,
    /// Free slots, sorted descending so `pop()` yields the lowest id.
    pub free: Vec<u32>,
}

impl Agents {
    pub fn len(&self) -> usize {
        self.act.len()
    }
    pub fn is_empty(&self) -> bool {
        self.act.is_empty()
    }
    pub fn alive(&self, a: AgentId) -> bool {
        self.flags[a as usize] & flags::ALIVE != 0
    }
    pub fn is_visitor(&self, a: AgentId) -> bool {
        self.flags[a as usize] & flags::VISITOR != 0
    }
    pub fn count_alive(&self) -> usize {
        self.flags
            .iter()
            .filter(|f| **f & flags::ALIVE != 0 && **f & flags::VISITOR == 0)
            .count()
    }

    /// Allocates a slot (reusing the lowest free one) with neutral values.
    pub fn alloc(&mut self) -> AgentId {
        if let Some(a) = self.free.pop() {
            self.reset(a);
            return a;
        }
        self.act.push(ACT_FREE_SLOT);
        self.until.push(0);
        self.place.push(NONE);
        self.next_event.push(Tick::MAX);
        self.needs.push([8000; NEED_COUNT]);
        self.needs_t.push(0);
        self.trip_from.push(NONE);
        self.trip_to.push(NONE);
        self.trip_depart.push(0);
        self.next_act.push(ACT_FREE_SLOT);
        self.plan_until.push(0);
        self.home.push(NONE);
        self.work.push(NONE);
        self.schedule.push(0);
        self.profession.push(u16::MAX);
        self.household.push(NONE);
        self.money.push(0);
        self.traits.push(0);
        self.flags.push(0);
        self.birth.push(0);
        self.given.push(crate::names::NO_NAME);
        self.family.push(crate::names::NO_NAME);
        self.partner.push(NONE);
        self.parent_a.push(NONE);
        self.parent_b.push(NONE);
        self.journal.push_agent();
        (self.act.len() - 1) as AgentId
    }

    fn reset(&mut self, a: AgentId) {
        let i = a as usize;
        self.act[i] = ACT_FREE_SLOT;
        self.until[i] = 0;
        self.place[i] = NONE;
        self.next_event[i] = Tick::MAX;
        self.needs[i] = [8000; NEED_COUNT];
        self.needs_t[i] = 0;
        self.trip_from[i] = NONE;
        self.trip_to[i] = NONE;
        self.trip_depart[i] = 0;
        self.next_act[i] = ACT_FREE_SLOT;
        self.plan_until[i] = 0;
        self.home[i] = NONE;
        self.work[i] = NONE;
        self.schedule[i] = 0;
        self.profession[i] = u16::MAX;
        self.household[i] = NONE;
        self.money[i] = 0;
        self.traits[i] = 0;
        self.flags[i] = 0;
        self.birth[i] = 0;
        self.given[i] = crate::names::NO_NAME;
        self.family[i] = crate::names::NO_NAME;
        self.partner[i] = NONE;
        self.parent_a[i] = NONE;
        self.parent_b[i] = NONE;
        self.journal.clear(a);
    }

    /// Frees a slot (the agent left the region or died).
    pub fn release(&mut self, a: AgentId) {
        self.reset(a);
        // keep `free` sorted descending
        let pos = self.free.partition_point(|x| *x > a);
        self.free.insert(pos, a);
    }

    /// Bytes per agent slot of every column (measured, not estimated).
    pub fn bytes_per_agent() -> usize {
        use std::mem::size_of;
        size_of::<u16>() * 3          // act, next_act, schedule
            + size_of::<u16>()        // profession
            + size_of::<Tick>() * 5   // until, next_event, needs_t, trip_depart, plan_until
            + size_of::<u32>() * 10   // place, trip_from, trip_to, home, work, household, traits, partner, parent_a, parent_b
            + size_of::<[u16; NEED_COUNT]>()
            + size_of::<i64>() * 2    // money, birth
            + size_of::<NameRef>() * 2
            + size_of::<u8>()         // flags
            + Journal::bytes_per_agent()
    }
}
