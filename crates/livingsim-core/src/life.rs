//! Daily life: needs, agendas and utility-based decisions (classic AI, no LLM).
//!
//! An agent is only touched at its *decision events*: when an activity ends or a trip
//! arrives. Needs are lazy (`v(t) = v0 + rate·(t − t0)`), so a sleeping or working agent
//! costs nothing until its next event.

use crate::agents::{flags, traits, AgentId, ACT_AWAY, ACT_DEPART, ACT_TRAVEL};
use crate::buildings::BuildingId;
use crate::catalog::{catalog, content};
use crate::messages::{Body, Traveller, WINDOW};
use crate::region::{AwayInfo, Region};
use crate::rng::{in_range, keyed, Purpose};
use crate::roads::{Mode, NONE};
use crate::time::{
    age_years, day_number, minute_of_day, next_minute_of_day, start_of_day, weekday, Tick,
};
use livingsim_content::{ActivityKind, DaySpan, JournalKind, Need, Place, ScheduleDef};

const ACTS_MAX: u16 = 0xFFF0;

/// Is the building open at `t`, and until when?
pub fn open_until(def: u16, t: Tick) -> Option<Tick> {
    let b = &content().buildings[def as usize];
    match b.open {
        None => Some(t + 24 * 60),
        Some(span) => {
            let m = minute_of_day(t);
            let inside = if span.crosses_midnight() {
                m >= span.start as u32 || m < span.end as u32
            } else {
                m >= span.start as u32 && m < span.end as u32
            };
            // Day mask refers to the day the opening started.
            let opened_day_offset = if span.crosses_midnight() && m < span.end as u32 {
                6
            } else {
                0
            };
            let wd = (weekday(t) + opened_day_offset) % 7;
            if inside && b.open_days.contains(wd) {
                Some(next_minute_of_day(t, span.end as u32))
            } else {
                None
            }
        }
    }
}

fn spans_on(sched: &ScheduleDef, t: Tick) -> impl Iterator<Item = (Tick, Tick)> + '_ {
    let day0 = start_of_day(t);
    let wd0 = weekday(t) as i64;
    (-1i64..=7).flat_map(move |d| {
        sched.blocks.iter().filter_map(move |b| {
            let wd = (wd0 + d).rem_euclid(7) as u8;
            if !b.days.contains(wd) {
                return None;
            }
            let s = day0 + d * 1440 + b.span.start as i64;
            let e = s + b.span.len() as i64;
            if e <= t as i64 || s < 0 {
                None
            } else {
                Some((s as Tick, e as Tick))
            }
        })
    })
}

/// The obligation block containing `t`, or the next one (within a week).
pub fn next_block(sched: &ScheduleDef, t: Tick) -> Option<(Tick, Tick)> {
    spans_on(sched, t).min()
}

fn has_block_on(sched: &ScheduleDef, wd: u8) -> bool {
    sched.blocks.iter().any(|b| b.days.contains(wd))
}

impl Region {
    // ------------------------------------------------------------------ needs

    /// Needs evaluated at `t` without modifying state (for the UI).
    pub fn needs_at(&self, a: AgentId, t: Tick) -> [u16; livingsim_content::NEED_COUNT] {
        let i = a as usize;
        let c = catalog();
        let mut out = self.agents.needs[i];
        let dt = t.saturating_sub(self.agents.needs_t[i]) as i64;
        let act = self.agents.act[i];
        let rates = if act < ACTS_MAX {
            c.rates[act as usize]
        } else {
            c.decay.map(|d| -d)
        };
        for (v, r) in out.iter_mut().zip(rates).take(5) {
            *v = (*v as i64 + r as i64 * dt / 60).clamp(0, 10_000) as u16;
        }
        out
    }

    pub(crate) fn update_needs(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        let c = catalog();
        let dt = t.saturating_sub(self.agents.needs_t[i]) as i64;
        if dt > 0 {
            let act = self.agents.act[i];
            let rates = if act < ACTS_MAX {
                c.rates[act as usize]
            } else {
                c.decay.map(|d| -d)
            };
            for (v, r) in self.agents.needs[i].iter_mut().zip(rates).take(5) {
                *v = (*v as i64 + r as i64 * dt / 60).clamp(0, 10_000) as u16;
            }
        }
        // Derived needs (Phase 1): money from savings, safety constant until Phase 5.
        let money = self.agents.money[i].clamp(0, 300_000);
        self.agents.needs[i][Need::Money as usize] = (money * 10_000 / 300_000) as u16;
        self.agents.needs[i][Need::Safety as usize] = 8_000;
        self.agents.needs_t[i] = t;
    }

    // ------------------------------------------------------------------ activities

    /// Effects at the end of an activity.
    pub(crate) fn finish_activity(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        let act = self.agents.act[i];
        let c = catalog();
        self.update_needs(a, t);
        if act == c.work_activity {
            let p = self.agents.profession[i];
            if p != u16::MAX {
                let wage = c.content.professions[p as usize].wage_per_shift_cents;
                self.agents.money[i] += wage;
                self.today.wages_paid_cents += wage;
                self.journal(
                    a,
                    t,
                    JournalKind::GotPaid,
                    0,
                    wage.clamp(0, u32::MAX as i64) as u32,
                );
            }
        } else if act == c.sleep_activity {
            self.journal(a, t, JournalKind::WokeUp, 0, 0);
        }
    }

    fn start(&mut self, a: AgentId, t: Tick, act: u16, until: Tick) {
        let i = a as usize;
        let c = catalog();
        let def = &c.content.activities[act as usize];
        let place = self.agents.place[i];
        let until = until.max(t + 1);
        self.agents.act[i] = act;
        self.agents.until[i] = until;
        self.agents.flags[i] &= !flags::BY_CAR;
        match def.kind {
            ActivityKind::Sleep => self.journal(a, t, JournalKind::WentToSleep, act, place),
            ActivityKind::EatHome => {
                let hh = self.agents.household[i];
                if hh != NONE && self.households.food[hh as usize] > 0 {
                    self.households.food[hh as usize] -= 1;
                    self.journal(a, t, JournalKind::Started, act, place);
                } else {
                    self.journal(a, t, JournalKind::NoFoodAtHome, act, place);
                    // Nothing to eat: just a short pause, then decide again.
                    self.agents.act[i] = c.relax_activity;
                    self.agents.until[i] = t + 10;
                }
            }
            ActivityKind::Shop => self.buy_groceries(a, t, place),
            _ => {
                if def.cost_cents > 0 {
                    self.agents.money[i] -= def.cost_cents;
                }
                if matches!(def.kind, ActivityKind::EatOut) {
                    self.today.meals_out += 1;
                }
                self.journal(a, t, JournalKind::Started, act, place);
            }
        }
        if place != NONE
            && place != self.agents.home[i]
            && def.kind != ActivityKind::Work
            && def.kind != ActivityKind::School
        {
            let v = &mut self.buildings.visits_today[place as usize];
            *v = v.saturating_add(1);
            if let Some(info) = self.visitors.get_mut(&a) {
                info.places += 1;
            }
        }
        let until = self.agents.until[i];
        self.set_event(a, until);
    }

    fn buy_groceries(&mut self, a: AgentId, t: Tick, shop: BuildingId) {
        let i = a as usize;
        let c = catalog();
        let econ = &c.content.economy;
        let hh = self.agents.household[i];
        let members = if hh == NONE {
            1
        } else {
            self.households.members[hh as usize].max(1) as u32
        };
        let want = members * econ.meals_per_person_bought as u32;
        let stock = &mut self.buildings.stock[shop as usize];
        let got = want.min(*stock);
        *stock -= got;
        if got == 0 {
            self.buildings.empty_today[shop as usize] =
                self.buildings.empty_today[shop as usize].saturating_add(1);
            self.today.shelves_empty += 1;
            self.journal(a, t, JournalKind::ShelvesEmpty, 0, shop);
            return;
        }
        let cost = got as i64 * econ.meal_price_cents;
        self.agents.money[i] -= cost;
        if hh != NONE {
            let f = &mut self.households.food[hh as usize];
            *f = (*f as u32 + got).min(u16::MAX as u32) as u16;
        }
        self.today.groceries += 1;
        // Subject: shop; amount goes in `arg` as euros (cents / 100), capped.
        self.journal(
            a,
            t,
            JournalKind::Shopped,
            (cost / 100).clamp(0, u16::MAX as i64) as u16,
            shop,
        );
    }

    fn travel(&mut self, a: AgentId, t: Tick, dest: BuildingId, next: u16, plan_until: Tick) {
        let i = a as usize;
        let from = self.agents.place[i];
        let mode = self.mode_for(a, from, dest, t);
        let ticks = self.trip_ticks(from, dest, mode);
        self.set_place(a, NONE);
        let ag = &mut self.agents;
        ag.act[i] = ACT_TRAVEL;
        ag.trip_from[i] = from;
        ag.trip_to[i] = dest;
        ag.trip_depart[i] = t;
        ag.next_act[i] = next;
        ag.plan_until[i] = plan_until;
        ag.until[i] = t + ticks;
        if mode == Mode::Car {
            ag.flags[i] |= flags::BY_CAR;
            self.today.car_trips += 1;
        } else {
            ag.flags[i] &= !flags::BY_CAR;
        }
        self.today.trips += 1;
        self.journal(a, t, JournalKind::WentTo, 0, dest);
        self.set_event(a, t + ticks);
    }

    fn go(&mut self, a: AgentId, t: Tick, dest: BuildingId, act: u16, until: Tick) {
        if self.agents.place[a as usize] == dest {
            self.start(a, t, act, until);
        } else {
            self.travel(a, t, dest, act, until);
        }
    }

    pub(crate) fn arrive(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        self.update_needs(a, t);
        let dest = self.agents.trip_to[i];
        self.set_place(a, dest);
        self.agents.flags[i] &= !flags::BY_CAR;
        let next = self.agents.next_act[i];
        if next == ACT_DEPART {
            self.depart_visitor(a, t);
            return;
        }
        let c = catalog();
        let def = &c.content.activities[next as usize];
        if def.kind == ActivityKind::VisitRegion {
            let plan = self.agents.plan_until[i];
            self.leave_for_region(a, t, plan.saturating_sub(t));
            return;
        }
        let plan = self.agents.plan_until[i];
        let min_left = if matches!(
            def.kind,
            ActivityKind::Work | ActivityKind::School | ActivityKind::Sleep
        ) {
            1
        } else {
            def.min_minutes as Tick / 2
        };
        if plan > t + min_left {
            self.start(a, t, next, plan);
        } else {
            self.agents.act[i] = c.relax_activity;
            self.decide(a, t);
        }
    }

    // ------------------------------------------------------------------ regions

    fn leave_for_region(&mut self, a: AgentId, t: Tick, visit_minutes: Tick) {
        let i = a as usize;
        let place = self.agents.place[i];
        let Some(gi) = self.gateways.iter().position(|g| g.building == place) else {
            self.agents.act[i] = catalog().relax_activity;
            self.decide(a, t);
            return;
        };
        let g = self.gateways[gi].clone();
        let next_window = (t / WINDOW + 1) * WINDOW;
        let arrival = (t + g.highway_ticks).max(next_window);
        let leave_by = arrival + visit_minutes.max(60);
        let return_at = leave_by + g.highway_ticks;
        let female = self.agents.flags[i] & flags::FEMALE != 0;
        let traveller = Traveller {
            agent: a,
            given: self.names.get(self.agents.given[i]).to_string(),
            family: self.names.get(self.agents.family[i]).to_string(),
            birth: self.agents.birth[i],
            female,
            traits: self.agents.traits[i],
            needs: self.agents.needs[i],
            money: self.agents.money[i],
            arrival,
            leave_by,
        };
        self.send(g.to_region, Body::Traveller(traveller));
        self.journal(a, t, JournalKind::LeftForRegion, g.to_region, 0);
        self.set_place(a, NONE);
        let ag = &mut self.agents;
        ag.act[i] = ACT_AWAY;
        ag.until[i] = return_at;
        self.away.insert(
            a,
            AwayInfo {
                region: g.to_region,
                gateway: gi as u16,
                return_at,
            },
        );
        self.set_event(a, return_at);
    }

    // ------------------------------------------------------------------ sleep

    /// Returns (wake time if the agent should be asleep now, start of the next sleep).
    fn sleep_info(&self, a: AgentId, t: Tick, sched: &ScheduleDef) -> (Option<Tick>, Tick) {
        let i = a as usize;
        let owl = traits::get(self.agents.traits[i], traits::NIGHT_OWL) as i64 - 7;
        let mut asleep = None;
        let mut next = Tick::MAX;
        let day0 = start_of_day(t);
        for d in -1i64..=1 {
            let base = day0 + d * 1440;
            let pick = |span: DaySpan| {
                let s = base + span.start as i64;
                (s, s + span.len() as i64)
            };
            let (s1, e1) = pick(sched.sleep);
            let wake_wd = ((weekday(t) as i64 + d + (e1 - base) / 1440).rem_euclid(7)) as u8;
            let (s, e) = if has_block_on(sched, wake_wd) {
                (s1, e1)
            } else {
                pick(sched.sleep_free_day)
            };
            let jitter = owl * 6
                + in_range(
                    keyed(
                        self.seed,
                        a as u64,
                        day_number((base.max(0)) as Tick) as u64 + d as u64,
                        Purpose::SleepJitter as u64,
                    ),
                    -15,
                    15,
                );
            let (s, e) = (s + jitter, e + jitter);
            if s <= t as i64 && (t as i64) < e {
                asleep = Some(e.max(t as i64 + 1) as Tick);
            }
            if s > t as i64 {
                next = next.min(s as Tick);
            }
        }
        (asleep, next)
    }

    // ------------------------------------------------------------------ decisions

    pub(crate) fn decide(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        self.update_needs(a, t);
        self.today.decisions += 1;
        let c = catalog();
        if self.agents.is_visitor(a) {
            self.decide_visitor(a, t);
            return;
        }
        let sched = &c.content.schedules[self.agents.schedule[i] as usize];
        let place = self.agents.place[i];
        let home = self.agents.home[i];
        let work = self.agents.work[i];
        let mut free_until = t + 1440;

        // 1. Obligations (work, school).
        if work != NONE {
            if let Some((s, e)) = next_block(sched, t) {
                let act = if self.agents.profession[i] == u16::MAX {
                    c.school_activity
                } else {
                    c.work_activity
                };
                let mode = self.mode_for(a, place, work, t);
                let travel = if place == work {
                    0
                } else {
                    self.trip_ticks(place, work, mode)
                };
                let punctual = traits::get(self.agents.traits[i], traits::PUNCTUAL) as Tick;
                let leave = s.saturating_sub(travel + 2 + punctual);
                if t >= leave {
                    self.go(a, t, work, act, e);
                    return;
                }
                free_until = free_until.min(leave);
            }
        }

        // 2. Sleep.
        let (wake, next_sleep) = self.sleep_info(a, t, sched);
        let energy = self.agents.needs[i][Need::Energy as usize];
        let exhausted = energy < 800;
        if let Some(w) = wake.or(if exhausted { Some(t + 7 * 60) } else { None }) {
            let w = w.min(free_until.saturating_sub(45)).max(t + 20);
            self.go(a, t, home, c.sleep_activity, w);
            return;
        }
        free_until = free_until.min(next_sleep);

        if free_until <= t + 15 {
            // Almost time to leave: stay put.
            let cur = self.agents.act[i];
            let act = if place == home || cur >= ACTS_MAX {
                c.relax_activity
            } else {
                cur
            };
            self.agents.act[i] = act;
            self.agents.until[i] = free_until.max(t + 1);
            self.set_event(a, free_until.max(t + 1));
            return;
        }

        // 3. Free time: utility AI.
        match self.choose_free(a, t, free_until, false) {
            Some((act, dest, end)) => self.go(a, t, dest, act, end),
            None => {
                let end = free_until.min(t + 60);
                self.go(a, t, home, c.relax_activity, end);
            }
        }
    }

    fn decide_visitor(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        let Some(info) = self.visitors.get(&a).cloned() else {
            self.set_place(a, NONE);
            self.agents.release(a);
            return;
        };
        let place = self.agents.place[i];
        let exit = self.agents.home[i];
        let back = if place == exit {
            0
        } else {
            self.trip_ticks(place, exit, Mode::Car)
        };
        if t + back + 10 >= info.leave_by {
            if place == exit {
                self.depart_visitor(a, t);
            } else {
                self.travel(a, t, exit, ACT_DEPART, info.leave_by);
            }
            return;
        }
        let free_until = info.leave_by.saturating_sub(back + 10);
        match self.choose_free(a, t, free_until, true) {
            Some((act, dest, end)) => self.go(a, t, dest, act, end),
            None => self.travel(a, t, exit, ACT_DEPART, info.leave_by),
        }
    }

    /// Picks a building with `tag` for agent `a`: close to where they are, with a personal,
    /// day-varying preference, and open now.
    fn pick_place(&self, a: AgentId, t: Tick, tag: u16) -> Option<BuildingId> {
        let i = a as usize;
        let from = if self.agents.place[i] != NONE {
            self.agents.place[i]
        } else {
            self.agents.home[i]
        };
        let day = day_number(t) as u64;
        let mut best: Option<(u32, BuildingId)> = None;
        for &b in self.buildings.with_tag(tag) {
            let def = self.buildings.def[b as usize];
            if open_until(def, t).is_none() {
                continue;
            }
            let cap = content().buildings[def as usize].capacity;
            if cap > 0 && self.buildings.present[b as usize] >= cap {
                continue;
            }
            let noise = (keyed(
                self.seed,
                a as u64,
                b as u64 ^ (day << 32),
                Purpose::PlaceChoice as u64,
            ) % 60) as u32;
            let score = self.manhattan(from, b) + noise;
            if best.is_none_or(|(s, bb)| (score, b) < (s, bb)) {
                best = Some((score, b));
            }
        }
        best.map(|(_, b)| b)
    }

    /// Utility scoring of free-time activities. Returns (activity, building, end tick).
    fn choose_free(
        &mut self,
        a: AgentId,
        t: Tick,
        free_until: Tick,
        visitor: bool,
    ) -> Option<(u16, BuildingId, Tick)> {
        let i = a as usize;
        let c = catalog();
        let content = &c.content;
        let age = age_years(self.agents.birth[i], t);
        let needs = self.agents.needs[i];
        let money = self.agents.money[i];
        let tr = self.agents.traits[i];
        let place = self.agents.place[i];
        let home = self.agents.home[i];
        let hh = self.agents.household[i];
        let m = minute_of_day(t);
        let weekend = weekday(t) >= 5;
        let mut best: Option<(i64, u16, BuildingId, Tick)> = None;

        for &ai in &c.free_activities {
            let def = &content.activities[ai as usize];
            if age < def.min_age as u32 || age > def.max_age as u32 {
                continue;
            }
            let dest = match def.place {
                Place::Home => {
                    if visitor {
                        continue;
                    }
                    home
                }
                Place::Workplace => continue,
                Place::Tag(tag) => match self.pick_place(a, t, tag) {
                    Some(b) => b,
                    None => continue,
                },
                Place::OtherRegion => {
                    if visitor
                        || self.gateways.is_empty()
                        || !weekend
                        || !(9 * 60..=13 * 60).contains(&m)
                    {
                        continue;
                    }
                    // Only occasionally (keyed, so it is the same on every machine).
                    if keyed(self.seed, a as u64, t as u64, Purpose::Visit as u64) % 1000 >= 250 {
                        continue;
                    }
                    self.gateways[0].building
                }
            };
            if dest == NONE {
                continue;
            }
            if def.cost_cents > 0 && money < def.cost_cents {
                continue;
            }
            let mode = if visitor {
                Mode::Car
            } else {
                self.mode_for(a, place, dest, t)
            };
            let travel = if dest == place {
                0
            } else {
                self.trip_ticks(place, dest, mode)
            };
            let arrive = t + travel;
            let dur = in_range(
                keyed(
                    self.seed,
                    a as u64,
                    t as u64 ^ ((ai as u64) << 40),
                    Purpose::Duration as u64,
                ),
                def.min_minutes as i64,
                def.max_minutes as i64,
            ) as Tick;
            let back = if dest == home || visitor { 0 } else { travel };
            let mut end = (arrive + dur).min(free_until.saturating_sub(back));
            if let Place::Tag(_) = def.place {
                match open_until(self.buildings.def[dest as usize], arrive) {
                    Some(close) => end = end.min(close),
                    None => continue,
                }
            }
            if def.kind == ActivityKind::VisitRegion {
                let hw = self.gateways[0].highway_ticks;
                let needed = travel * 2 + dur + hw * 2 + WINDOW + 60;
                if t + needed > free_until {
                    continue;
                }
                end = t + travel + dur;
            }
            if end < arrive + def.min_minutes as Tick {
                continue;
            }
            let effective = (end - arrive) as i64;

            let mut score: i64 = 0;
            for (n, need) in content.needs.iter().enumerate().take(5) {
                let gain = c.rates[ai as usize][n] as i64 * effective / 60;
                if gain <= 0 {
                    continue;
                }
                let v = needs[n] as i64;
                let deficit = 10_000 - v;
                let urgency = deficit * deficit / 10_000;
                score += need.weight as i64 * urgency * gain.min(deficit) / 10_000 / 100;
            }
            match def.kind {
                ActivityKind::EatHome => {
                    if hh == NONE || self.households.food[hh as usize] == 0 {
                        continue;
                    }
                }
                ActivityKind::Shop => {
                    if hh == NONE {
                        continue;
                    }
                    let members = self.households.members[hh as usize].max(1) as i64;
                    let food = self.households.food[hh as usize] as i64;
                    if food >= members * 3 {
                        continue;
                    }
                    score += 2500 + (members * 3 - food) * 350;
                }
                _ => {}
            }
            // Travel and money costs, personality.
            score -= travel as i64 * 22;
            let thrifty = traits::get(tr, traits::THRIFTY) as i64;
            score -= def.cost_cents * (thrifty + 4) / 40;
            if money < 5_000 {
                score -= def.cost_cents / 2;
            }
            let social_gain = c.rates[ai as usize][Need::Social as usize].max(0) as i64;
            score += social_gain * (traits::get(tr, traits::SOCIABLE) as i64 - 7) / 60;
            if def.place == Place::Home {
                score += (traits::get(tr, traits::HOMEBODY) as i64 - 5) * 40;
            }
            if def.kind == ActivityKind::Leisure {
                score += (traits::get(tr, traits::ACTIVE) as i64 - 7) * 30;
            }
            score += def.bias as i64;
            score += (keyed(
                self.seed,
                a as u64,
                t as u64 ^ ((ai as u64) << 32),
                Purpose::Utility as u64,
            ) % 500) as i64;
            if best.is_none_or(|(s, bai, _, _)| score > s || (score == s && ai < bai)) {
                best = Some((score, ai, dest, end));
            }
        }
        best.filter(|(s, _, _, _)| *s > 0)
            .map(|(_, ai, dest, end)| (ai, dest, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_blocks() {
        let c = content();
        let office = &c.schedules[c.schedule("office_day").unwrap() as usize];
        // Tick 0 = Monday 06:00. Next block: Monday 09:00-17:00.
        assert_eq!(next_block(office, 0), Some((180, 660)));
        // During the block.
        assert_eq!(next_block(office, 200), Some((180, 660)));
        // Friday 18:00 -> next Monday 09:00.
        let fri_18 = 4 * 1440 + 12 * 60;
        assert_eq!(
            next_block(office, fri_18),
            Some((7 * 1440 + 180, 7 * 1440 + 660))
        );
        let night = &c.schedules[c.schedule("night_shift").unwrap() as usize];
        // Monday 06:00 is the end of Sunday night's shift? Sunday is not in Mon-Fri, so no.
        assert_eq!(next_block(night, 0), Some((16 * 60, 16 * 60 + 480)));
    }
}
