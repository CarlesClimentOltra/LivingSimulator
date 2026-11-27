//! Stable content references across versions.
//!
//! The state refers to content by index (`buildings.def`, `agents.schedule`, …) because it
//! is compact and fast. Indices are only meaningful for one version of `/data`, so every
//! region carries the *names* behind its indices. When a snapshot is loaded with a
//! different `/data` (entries added, removed or reordered), every index in the state is
//! translated by name. Contributors can therefore edit `/data` freely; the only rule is
//! never to rename an id that running worlds use (add a new one instead).

use crate::agents::{ACT_FREE_SLOT, ACT_TRAVEL};
use crate::catalog::content;
use crate::messages::Body;
use crate::region::Region;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentIds {
    pub buildings: Vec<String>,
    pub professions: Vec<String>,
    pub schedules: Vec<String>,
    pub activities: Vec<String>,
    pub goods: Vec<String>,
}

impl ContentIds {
    /// The ids of the content compiled into this build.
    pub fn current() -> ContentIds {
        let c = content();
        ContentIds {
            buildings: c.buildings.iter().map(|x| x.id.clone()).collect(),
            professions: c.professions.iter().map(|x| x.id.clone()).collect(),
            schedules: c.schedules.iter().map(|x| x.id.clone()).collect(),
            activities: c.activities.iter().map(|x| x.id.clone()).collect(),
            goods: c.goods.iter().map(|x| x.id.clone()).collect(),
        }
    }
}

/// Old index → new index for one kind of content. Ids that no longer exist map to
/// `fallback` (index 0 of the new list), so a removed id never crashes a world.
fn table(old: &[String], new: &[String]) -> Vec<u16> {
    old.iter()
        .map(|id| new.iter().position(|n| n == id).unwrap_or(0) as u16)
        .collect()
}

fn map(t: &[u16], v: u16) -> u16 {
    t.get(v as usize).copied().unwrap_or(v)
}

impl Region {
    /// Translates every content index in the state from `self.content_ids` to the content
    /// of this build, then records the current ids. Called after loading.
    pub fn remap_content(&mut self) {
        let now = ContentIds::current();
        if self.content_ids == now {
            return;
        }
        if self.content_ids == ContentIds::default() {
            // Created before ids were recorded: assume the current order.
            self.content_ids = now;
            return;
        }
        let old = std::mem::take(&mut self.content_ids);
        let b = table(&old.buildings, &now.buildings);
        let p = table(&old.professions, &now.professions);
        let s = table(&old.schedules, &now.schedules);
        let a = table(&old.activities, &now.activities);
        let g = table(&old.goods, &now.goods);

        for d in self.buildings.def.iter_mut() {
            *d = map(&b, *d);
        }
        let act = |v: u16| {
            if v >= ACT_FREE_SLOT - 0x10 || v == ACT_TRAVEL {
                v
            } else {
                map(&a, v)
            }
        };
        for i in 0..self.agents.len() {
            self.agents.schedule[i] = map(&s, self.agents.schedule[i]);
            if self.agents.profession[i] != u16::MAX {
                self.agents.profession[i] = map(&p, self.agents.profession[i]);
            }
            self.agents.act[i] = act(self.agents.act[i]);
            self.agents.next_act[i] = act(self.agents.next_act[i]);
        }
        self.agents.journal.remap_activities(|v| map(&a, v));
        for f in self.systems.economy.firms.iter_mut() {
            if f.good != crate::systems::economy::NO_GOOD {
                f.good = map(&g, f.good);
            }
        }
        for j in self.systems.demography.remote_jobs.values_mut() {
            j.profession = map(&p, j.profession);
        }
        for r in self.systems.demography.remote_staff.iter_mut() {
            r.profession = map(&p, r.profession);
            r.schedule = map(&s, r.schedule);
        }
        for m in self.outbox.iter_mut() {
            match &mut m.body {
                Body::JobOffers(offers) => {
                    for o in offers.iter_mut() {
                        o.1 = map(&p, o.1);
                    }
                }
                Body::JobAccept {
                    profession,
                    schedule,
                    ..
                } => {
                    *profession = map(&p, *profession);
                    *schedule = map(&s, *schedule);
                }
                Body::Sale { good, .. }
                | Body::Trade { good, .. }
                | Body::MaterialShipment { good, .. } => *good = map(&g, *good),
                _ => {}
            }
        }
        self.content_ids = now;
    }
}
