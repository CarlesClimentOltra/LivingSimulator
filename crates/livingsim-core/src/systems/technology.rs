//! Phase 7: technology. Professors and researchers earn research points every shift; the
//! town works on the first technology of `data/technologies.ron` it can (requirements
//! known). A finished technology changes the town (faster trips, cheaper buildings, more
//! production, new building types) and spreads to neighbouring towns with `TechDiffusion`
//! messages; they adopt it some days later, then pass it on.
//!
//! Technologies are stored by id (not by content index), so `/data` can grow freely.

use super::DailyContext;
use crate::catalog::content;
use crate::messages::Body;
use crate::region::{NewsItem, Region};
use crate::time::{Tick, TICKS_PER_DAY};
use livingsim_content::{FirmKind, NewsKind, TechEffect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HISTORY_DAYS: usize = 60;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnologyState {
    /// Points put into the current research.
    pub points: u64,
    /// Known technologies (ids), in the order they arrived, and when.
    pub researched: Vec<String>,
    pub researched_at: Vec<Tick>,
    /// What the town's researchers work on now.
    pub current: Option<String>,
    /// Technologies a neighbouring town has: id → when this town adopts it.
    pub adopting: BTreeMap<String, Tick>,
    pub points_today: u64,
    /// Research points per day (last 60 days).
    pub history: Vec<u64>,
}

impl TechnologyState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // Technology needs the whole region; see `Region::technology_daily`.
    }
    pub fn knows(&self, id: &str) -> bool {
        self.researched.iter().any(|x| x == id)
    }
}

/// The combined effect of the known technologies (derived, rebuilt when one arrives).
#[derive(Clone, Debug)]
pub struct TechMods {
    pub walk: u32,
    pub car: u32,
    pub highway: u32,
    pub maintenance: u32,
    pub output: Vec<(FirmKind, u32)>,
}

impl Default for TechMods {
    fn default() -> Self {
        TechMods {
            walk: 1000,
            car: 1000,
            highway: 1000,
            maintenance: 1000,
            output: Vec::new(),
        }
    }
}

impl TechMods {
    pub fn output_of(&self, kind: FirmKind) -> u32 {
        self.output
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| *v)
            .unwrap_or(1000)
    }
}

impl Region {
    /// Recomputes `tech` from the known technologies.
    pub(crate) fn refresh_tech(&mut self) {
        let mut m = TechMods::default();
        let c = content();
        for id in &self.systems.technology.researched {
            let Some(t) = c.technologies.iter().find(|t| &t.id == id) else {
                continue;
            };
            for e in &t.effects {
                match *e {
                    TechEffect::WalkTime(p) => m.walk = m.walk * p as u32 / 1000,
                    TechEffect::CarTime(p) => m.car = m.car * p as u32 / 1000,
                    TechEffect::HighwayTime(p) => m.highway = m.highway * p as u32 / 1000,
                    TechEffect::Maintenance(p) => m.maintenance = m.maintenance * p as u32 / 1000,
                    TechEffect::Output(k, p) => match m.output.iter_mut().find(|(x, _)| *x == k) {
                        Some((_, v)) => *v = *v * p as u32 / 1000,
                        None => m.output.push((k, p as u32)),
                    },
                }
            }
        }
        self.tech = m;
    }

    /// Driving time to the town at the end of gateway `gi`.
    pub(crate) fn highway_ticks(&self, gi: usize) -> Tick {
        (self.gateways[gi].highway_ticks * self.tech.highway / 1000).max(1)
    }

    /// False for building types that need a technology this town does not know yet.
    pub fn building_unlocked(&self, def: u16) -> bool {
        content()
            .technologies
            .iter()
            .filter(|t| t.unlocks.contains(&def))
            .all(|t| self.systems.technology.knows(&t.id))
    }

    /// Research points for a shift of agent `a` (professors, researchers).
    pub(crate) fn research_shift(&mut self, profession: u16) {
        let pts = content()
            .research
            .points_per_shift
            .iter()
            .find(|(p, _)| *p == profession)
            .map(|(_, n)| *n as u64)
            .unwrap_or(0);
        if pts > 0 {
            let s = &mut self.systems.technology;
            s.points += pts;
            s.points_today += pts;
        }
    }

    /// The first technology (in data order) whose requirements are known, not yet known
    /// and not about to be adopted from a neighbour.
    fn next_research(&self) -> Option<String> {
        let c = content();
        let s = &self.systems.technology;
        c.technologies
            .iter()
            .find(|t| {
                !s.knows(&t.id)
                    && !s.adopting.contains_key(&t.id)
                    && t.requires
                        .iter()
                        .all(|r| s.knows(&c.technologies[*r as usize].id))
            })
            .map(|t| t.id.clone())
    }

    fn learn(&mut self, id: &str, t: Tick, adopted: bool) {
        let c = content();
        let Some(idx) = c.technologies.iter().position(|x| x.id == id) else {
            return;
        };
        let s = &mut self.systems.technology;
        if s.knows(id) {
            return;
        }
        s.researched.push(id.to_string());
        s.researched_at.push(t);
        s.adopting.remove(id);
        if s.current.as_deref() == Some(id) {
            s.current = None;
            s.points = 0;
        }
        self.refresh_tech();
        self.systems.construction.static_version += 1;
        self.push_news(NewsItem {
            tick: t,
            kind: if adopted {
                NewsKind::TechAdopted
            } else {
                NewsKind::Breakthrough
            } as u16,
            subject: idx as u32,
            n: 0,
        });
        // Neighbours hear about it.
        let to: Vec<u16> = self.gateways.iter().map(|g| g.to_region).collect();
        for r in to {
            self.send(
                r,
                Body::TechDiffusion {
                    tech: id.to_string(),
                },
            );
        }
    }

    pub(crate) fn receive_tech(&mut self, tech: &str) {
        let days = content().research.adoption_days as Tick;
        let t = self.tick;
        let s = &mut self.systems.technology;
        if s.knows(tech) || s.adopting.contains_key(tech) {
            return;
        }
        if !content().technologies.iter().any(|x| x.id == tech) {
            return;
        }
        s.adopting
            .insert(tech.to_string(), t + days * TICKS_PER_DAY);
        // No point researching what is on its way.
        if s.current.as_deref() == Some(tech) {
            s.current = None;
        }
    }

    pub(crate) fn technology_daily(&mut self, t: Tick) {
        // Adoptions due (requirements first: they arrive in order anyway).
        let due: Vec<String> = self
            .systems
            .technology
            .adopting
            .iter()
            .filter(|(_, at)| **at <= t)
            .map(|(k, _)| k.clone())
            .collect();
        for id in due {
            self.learn(&id, t, true);
        }
        // Research.
        if self.systems.technology.current.is_none() {
            self.systems.technology.current = self.next_research();
        }
        if let Some(cur) = self.systems.technology.current.clone() {
            let cost = content()
                .technologies
                .iter()
                .find(|x| x.id == cur)
                .map(|x| x.cost_points)
                .unwrap_or(u64::MAX);
            if self.systems.technology.points >= cost {
                let extra = self.systems.technology.points - cost;
                self.learn(&cur, t, false);
                self.systems.technology.points = extra;
                self.systems.technology.current = self.next_research();
            }
        }
        let s = &mut self.systems.technology;
        s.history.push(std::mem::take(&mut s.points_today));
        if s.history.len() > HISTORY_DAYS {
            s.history.remove(0);
        }
    }
}
