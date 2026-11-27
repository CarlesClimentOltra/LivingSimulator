//! Read-only views for the UI (person card, stats, newspaper). Text is produced here from
//! the compact state and the templates in `data/journal.ron`. Nothing here mutates state.

use crate::agents::{flags, AgentId, ACT_AWAY, ACT_FREE_SLOT, ACT_TRAVEL};
use crate::buildings::BuildingId;
use crate::catalog::{catalog, content};
use crate::region::Region;
use crate::roads::NONE;
use crate::time::{age_years, date, Tick, MONTH_NAMES, WEEKDAY_NAMES};
use livingsim_content::{JournalKind, Need, NewsKind};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Relative {
    pub id: u32,
    pub name: String,
    pub relation: String,
    pub age: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct JournalLine {
    pub tick: Tick,
    pub when: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentCard {
    pub id: u32,
    pub region: u16,
    pub name: String,
    pub age: u32,
    pub female: bool,
    pub visitor: bool,
    pub home: String,
    pub occupation: String,
    pub workplace: String,
    pub schedule: String,
    pub doing: String,
    pub place: String,
    pub needs: Vec<(String, u16)>,
    pub money_cents: i64,
    pub family: Vec<Relative>,
    pub journal: Vec<JournalLine>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RegionStats {
    pub id: u16,
    pub name: String,
    pub tick: Tick,
    pub population: u32,
    pub households: u32,
    pub buildings: u32,
    pub employed: u32,
    pub unemployed: u32,
    pub students: u32,
    pub children: u32,
    pub retired: u32,
    pub visitors: u32,
    pub away: u32,
    pub travelling: u32,
    pub by_activity: Vec<(String, u32)>,
    pub avg_needs: Vec<(String, u32)>,
    pub avg_money_cents: i64,
    pub trips_yesterday: u32,
    pub meals_out_yesterday: u32,
    pub groceries_yesterday: u32,
    pub shelves_empty_yesterday: u32,
    pub wages_yesterday_cents: i64,
    pub events_total: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NewsLine {
    pub tick: Tick,
    pub when: String,
    pub headline: String,
    pub body: String,
}

pub fn format_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.unsigned_abs();
    format!("{sign}{}.{:02} ₡", c / 100, c % 100)
}

pub fn format_when(t: Tick) -> String {
    let d = date(t);
    format!(
        "{} {} {}, {:02}:{:02}",
        &WEEKDAY_NAMES[d.weekday as usize][..3],
        MONTH_NAMES[d.month as usize - 1],
        d.day,
        d.hour,
        d.minute
    )
}

impl Region {
    pub fn full_name(&self, a: AgentId) -> String {
        let i = a as usize;
        format!(
            "{} {}",
            self.names.get(self.agents.given[i]),
            self.names.get(self.agents.family[i])
        )
    }

    pub fn building_name(&self, b: BuildingId) -> String {
        if b == NONE || b as usize >= self.buildings.len() {
            return "somewhere".into();
        }
        let i = b as usize;
        let n = self.buildings.name[i];
        if n != crate::names::NO_NAME {
            return self.names.get(n).to_string();
        }
        let def = &content().buildings[self.buildings.def[i] as usize];
        let street = self
            .streets
            .get(self.buildings.street[i] as usize)
            .map(|s| self.names.get(*s))
            .unwrap_or("");
        format!("{} on {}", def.name.to_lowercase(), street)
    }

    fn region_label(&self, id: u16) -> String {
        if id == self.id {
            return self.name().to_string();
        }
        self.gateways
            .iter()
            .find(|g| g.to_region == id)
            .map(|g| g.to_name.clone())
            .unwrap_or_else(|| format!("region {id}"))
    }

    fn doing(&self, a: AgentId) -> (String, String) {
        let i = a as usize;
        let c = content();
        match self.agents.act[i] {
            ACT_TRAVEL => {
                let by = if self.agents.flags[i] & flags::BY_CAR != 0 {
                    "driving"
                } else {
                    "walking"
                };
                (
                    format!("{by} to {}", self.building_name(self.agents.trip_to[i])),
                    "on the road".into(),
                )
            }
            ACT_AWAY => {
                let r = self
                    .away
                    .get(&a)
                    .map(|x| self.region_label(x.region))
                    .unwrap_or_else(|| "another town".into());
                (format!("visiting {r}"), r)
            }
            ACT_FREE_SLOT => ("—".into(), "—".into()),
            act => (
                c.activities[act as usize].doing.clone(),
                self.building_name(self.agents.place[i]),
            ),
        }
    }

    fn journal_text(&self, kind: JournalKind, arg: u16, subject: u32) -> String {
        let c = content();
        let tpl = &c.text.journal[kind as usize];
        let place = match kind {
            JournalKind::LeftForRegion | JournalKind::BackFromRegion => self.region_label(arg),
            _ => self.building_name(subject),
        };
        let amount = match kind {
            JournalKind::Shopped => format_money(arg as i64 * 100),
            _ => format_money(subject as i64),
        };
        let activity = c
            .activities
            .get(arg as usize)
            .map(|a| a.doing.clone())
            .unwrap_or_default();
        tpl.replace("{place}", &place)
            .replace("{amount}", &amount)
            .replace("{activity}", &activity)
            .replace("{n}", &arg.to_string())
            .replace("{city}", self.name())
    }

    pub fn agent_card(&self, a: AgentId) -> Option<AgentCard> {
        let i = a as usize;
        if i >= self.agents.len() || !self.agents.alive(a) {
            return None;
        }
        let c = content();
        let cat = catalog();
        let t = self.tick;
        let visitor = self.agents.is_visitor(a);
        let sched = self.agents.schedule[i];
        let prof = self.agents.profession[i];
        let work = self.agents.work[i];
        let occupation = if visitor {
            let from = self
                .visitors
                .get(&a)
                .map(|v| self.region_label(v.origin_region))
                .unwrap_or_default();
            format!("Visitor from {from}")
        } else if prof != u16::MAX {
            c.professions[prof as usize].name.clone()
        } else if sched == cat.schedule_child {
            if work != NONE {
                "Pupil".into()
            } else {
                "Child".into()
            }
        } else if sched == cat.schedule_university {
            "University student".into()
        } else if sched == cat.schedule_toddler {
            "Toddler".into()
        } else if sched == cat.schedule_retired {
            "Retired".into()
        } else {
            "Looking for a job".into()
        };
        let mut family = Vec::new();
        let mut rel = |id: u32, relation: &str| {
            if id != NONE
                && (id as usize) < self.agents.len()
                && self.agents.alive(id)
                && !family.iter().any(|r: &Relative| r.id == id)
            {
                family.push(Relative {
                    id,
                    name: self.full_name(id),
                    relation: relation.into(),
                    age: age_years(self.agents.birth[id as usize], t),
                });
            }
        };
        rel(self.agents.partner[i], "partner");
        rel(self.agents.parent_a[i], "parent");
        rel(self.agents.parent_b[i], "parent");
        if !visitor {
            for k in 0..self.agents.len() as u32 {
                if self.agents.parent_a[k as usize] == a || self.agents.parent_b[k as usize] == a {
                    rel(k, "child");
                }
            }
            let hh = self.agents.household[i];
            if hh != NONE {
                for k in 0..self.agents.len() as u32 {
                    if k != a && self.agents.household[k as usize] == hh && self.agents.alive(k) {
                        let p = self.agents.parent_a[k as usize];
                        let sibling = p != NONE
                            && (p == self.agents.parent_a[i] || p == self.agents.parent_b[i]);
                        rel(k, if sibling { "sibling" } else { "household" });
                    }
                }
            }
        }
        let (doing, place) = self.doing(a);
        let now = self.needs_at(a, t);
        let needs = c
            .needs
            .iter()
            .filter(|n| n.need != Need::Safety)
            .map(|n| (n.name.clone(), now[n.need as usize]))
            .collect();
        let journal = self
            .agents
            .journal
            .read(a)
            .into_iter()
            .filter_map(|e| JournalKind::from_u16(e.kind).map(|k| (e, k)))
            .map(|(e, k)| JournalLine {
                tick: e.tick,
                when: format_when(e.tick),
                text: self.journal_text(k, e.arg, e.subject),
            })
            .collect();
        Some(AgentCard {
            id: a,
            region: self.id,
            name: self.full_name(a),
            age: age_years(self.agents.birth[i], t),
            female: self.agents.flags[i] & flags::FEMALE != 0,
            visitor,
            home: if visitor {
                "—".into()
            } else {
                self.building_name(self.agents.home[i])
            },
            occupation,
            workplace: if work != NONE {
                self.building_name(work)
            } else {
                "—".into()
            },
            schedule: c.schedules[sched as usize].name.clone(),
            doing,
            place,
            needs,
            money_cents: self.agents.money[i],
            family,
            journal,
        })
    }

    pub fn stats(&self) -> RegionStats {
        let c = content();
        let cat = catalog();
        let mut s = RegionStats {
            id: self.id,
            name: self.name().to_string(),
            tick: self.tick,
            population: 0,
            households: self.households.len() as u32,
            buildings: self.buildings.len() as u32,
            employed: 0,
            unemployed: 0,
            students: 0,
            children: 0,
            retired: 0,
            visitors: self.visitors.len() as u32,
            away: 0,
            travelling: 0,
            by_activity: Vec::new(),
            avg_needs: Vec::new(),
            avg_money_cents: 0,
            trips_yesterday: self.yesterday.trips,
            meals_out_yesterday: self.yesterday.meals_out,
            groceries_yesterday: self.yesterday.groceries,
            shelves_empty_yesterday: self.yesterday.shelves_empty,
            wages_yesterday_cents: self.yesterday.wages_paid_cents,
            events_total: self.events,
        };
        let mut by_act = vec![0u32; c.activities.len()];
        let mut needs = [0u64; livingsim_content::NEED_COUNT];
        let mut money = 0i64;
        for a in 0..self.agents.len() as u32 {
            let i = a as usize;
            if !self.agents.alive(a) || self.agents.is_visitor(a) {
                continue;
            }
            s.population += 1;
            money += self.agents.money[i];
            let now = self.needs_at(a, self.tick);
            for (n, v) in needs.iter_mut().enumerate() {
                *v += now[n] as u64;
            }
            let sched = self.agents.schedule[i];
            if self.agents.profession[i] != u16::MAX {
                s.employed += 1;
            } else if sched == cat.schedule_unemployed {
                s.unemployed += 1;
            } else if sched == cat.schedule_university {
                s.students += 1;
            } else if sched == cat.schedule_retired {
                s.retired += 1;
            } else {
                s.children += 1;
            }
            match self.agents.act[i] {
                ACT_TRAVEL => s.travelling += 1,
                ACT_AWAY => s.away += 1,
                ACT_FREE_SLOT => {}
                x => by_act[x as usize] += 1,
            }
        }
        let mut acts: Vec<(String, u32)> = c
            .activities
            .iter()
            .zip(by_act)
            .filter(|(_, n)| *n > 0)
            .map(|(a, n)| (a.doing.clone(), n))
            .collect();
        acts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        s.by_activity = acts;
        let p = s.population.max(1) as u64;
        s.avg_needs = c
            .needs
            .iter()
            .filter(|n| n.need != Need::Safety)
            .map(|n| (n.name.clone(), (needs[n.need as usize] / p) as u32))
            .collect();
        s.avg_money_cents = money / p as i64;
        s
    }

    pub fn newspaper(&self) -> Vec<NewsLine> {
        let c = content();
        self.news
            .iter()
            .rev()
            .filter_map(|n| {
                let kind = NewsKind::from_u16(n.kind)?;
                let (h, b) = &c.text.news[kind as usize];
                let place = match kind {
                    NewsKind::Visitors | NewsKind::FoodDelivery => {
                        self.region_label(n.subject as u16)
                    }
                    NewsKind::BusiestPlace | NewsKind::ShelvesEmpty => {
                        self.building_name(n.subject)
                    }
                    _ => self.name().to_string(),
                };
                let f = |s: &str| {
                    s.replace("{place}", &place)
                        .replace("{n}", &n.n.to_string())
                        .replace("{city}", self.name())
                };
                Some(NewsLine {
                    tick: n.tick,
                    when: format_when(n.tick),
                    headline: f(h),
                    body: f(b),
                })
            })
            .collect()
    }

    /// Agents currently inside a building (for the building panel and interiors).
    pub fn occupants(&self, b: BuildingId) -> Vec<AgentId> {
        (0..self.agents.len() as u32)
            .filter(|a| self.agents.alive(*a) && self.agents.place[*a as usize] == b)
            .collect()
    }
}
