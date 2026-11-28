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
    /// "Owns their home" / "Rents from … for $X a month".
    pub housing: String,
    /// Monthly rent paid by the household (0 for owners).
    pub rent_cents: i64,
    /// Months of unpaid rent.
    pub rent_arrears: u8,
    /// Businesses owned by this person.
    pub businesses: Vec<String>,
    pub education: String,
    pub relationship: String,
    pub health: String,
    /// Extra life situation: pregnancy, parental leave, commuting, adoption…
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FirmInfo {
    pub kind: String,
    pub open: bool,
    pub owner: String,
    pub cash_cents: i64,
    /// Human-readable price ("$4.50 a meal", "112% of the usual price").
    pub price: String,
    pub stock: u32,
    pub staff: u32,
    pub positions: u32,
    pub revenue_yesterday_cents: i64,
    pub costs_yesterday_cents: i64,
    pub customers_yesterday: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildingCard {
    pub id: u32,
    pub name: String,
    pub kind: String,
    pub floors: u8,
    pub dwellings: u16,
    pub present: u16,
    pub occupants: Vec<(u32, String)>,
    pub firm: Option<FirmInfo>,
    /// Building site: what is being built and how far it got.
    pub site: Option<SiteInfo>,
    /// Who owns the homes (buildings built since Phase 4).
    pub landlord: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SiteInfo {
    pub building: String,
    pub phase: String,
    pub progress_permille: u32,
    pub phase_permille: u32,
    pub funder: String,
    pub builder: String,
    pub days: u32,
    pub cost_cents: i64,
    pub workers_here: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectLine {
    pub site: u32,
    pub what: String,
    pub phase: String,
    pub progress_permille: u32,
    pub funder: String,
    pub days: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct IncidentLine {
    pub building: u32,
    pub what: String,
    pub place: String,
    pub when: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SafetyStats {
    /// (crime name, incidents in the last 30 days).
    pub by_crime_30d: Vec<(String, u32)>,
    pub solved_30d: u32,
    pub convictions_30d: u32,
    pub inmates: u32,
    pub inmates_from_elsewhere: u32,
    pub officers: u32,
    /// Average block safety (‰).
    pub safety: u32,
    pub recent: Vec<IncidentLine>,
    pub history: Vec<crate::systems::crime::CrimeSample>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConstructionStats {
    pub open: Vec<ProjectLine>,
    pub started: u32,
    pub finished: u32,
    pub streets: u32,
    pub units_yesterday: u32,
    pub map_tiles: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct EconomyStats {
    pub treasury_cents: i64,
    pub income_tax_permille: u16,
    pub sales_tax_permille: u16,
    pub unemployment_permille: u32,
    pub food_price_cents: i64,
    pub avg_rent_cents: i64,
    pub firms_open: u32,
    pub firms_closed: u32,
    pub owners: u32,
    pub renters: u32,
    pub output_cents: i64,
    pub exports_cents: i64,
    pub imports_cents: i64,
    pub rent_paid_cents: i64,
    pub hires: u32,
    pub layoffs: u32,
    /// Daily history (oldest first) for charts.
    pub history: Vec<crate::systems::economy::DaySample>,
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
    pub economy: EconomyStats,
    pub demography: DemographyStats,
    pub construction: ConstructionStats,
    pub safety: SafetyStats,
}

#[derive(Clone, Debug, Serialize)]
pub struct DemographyStats {
    /// Population by 5-year band (0–4 … 90+): (men, women).
    pub pyramid: Vec<(u32, u32)>,
    pub average_age: u32,
    pub births_30d: u32,
    pub deaths_30d: u32,
    pub marriages_30d: u32,
    pub arrivals_30d: u32,
    pub departures_30d: u32,
    pub ill: u32,
    pub pregnant: u32,
    pub couples: u32,
    pub commuters_out: u32,
    pub commuters_in: u32,
    pub university_graduates: u32,
    pub history: Vec<crate::systems::demography::DemoSample>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NewsLine {
    pub tick: Tick,
    pub when: String,
    pub headline: String,
    pub body: String,
}

/// "$1,234.56" (US dollars, money is stored in cents).
pub fn format_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.unsigned_abs();
    let dollars = (c / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in dollars.chars().enumerate() {
        if i > 0 && (dollars.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}${grouped}.{:02}", c % 100)
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
        // A building site is named after what is being built there.
        let def = match self
            .site_project(b)
            .filter(|p| p.phase != crate::systems::construction::Phase::Finished)
            .map(|p| &p.work)
        {
            Some(crate::systems::construction::Work::Building { def, .. }) => {
                return format!(
                    "Future {} on {}",
                    content().buildings[*def as usize].name.to_lowercase(),
                    self.streets
                        .get(self.buildings.street[i] as usize)
                        .map(|s| self.names.get(*s))
                        .unwrap_or("")
                );
            }
            _ => &content().buildings[self.buildings.def[i] as usize],
        };
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
            JournalKind::CommuteJob => "a firm".into(),
            JournalKind::FamilyDeath => {
                let cem = content()
                    .tag("cemetery")
                    .and_then(|t| self.buildings.with_tag(t).first().copied());
                cem.map(|b| self.building_name(b))
                    .unwrap_or_else(|| "the regional cemetery".into())
            }
            _ => self.building_name(subject),
        };
        let city = match kind {
            JournalKind::CommuteJob | JournalKind::WorkedAway | JournalKind::MovedTown => {
                self.region_label(arg)
            }
            _ => self.name().to_string(),
        };
        let person = if self.resident(subject) {
            self.full_name(subject)
        } else {
            "someone".into()
        };
        let first = self.names.get(subject).to_string();
        let amount = match kind {
            JournalKind::Shopped => format_money(arg as i64 * 100),
            _ => format_money(subject as i64),
        };
        let activity = match kind {
            JournalKind::Invested => c
                .buildings
                .get(arg as usize)
                .map(|d| d.name.to_lowercase())
                .unwrap_or_default(),
            _ => c
                .activities
                .get(arg as usize)
                .map(|a| a.doing.clone())
                .unwrap_or_default(),
        };
        tpl.replace("{place}", &place)
            .replace("{amount}", &amount)
            .replace("{activity}", &activity)
            .replace("{n}", &arg.to_string())
            .replace("{city}", &city)
            .replace("{person}", &person)
            .replace("{first}", &first)
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
        let occupation = if let Some(j) = self.remote_job(a) {
            let p = self.agents.profession[i];
            let name = if p != u16::MAX {
                c.professions[p as usize].name.clone()
            } else {
                "Worker".into()
            };
            format!("{name} (in {})", self.region_label(j.region))
        } else if visitor {
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
        let female = |x: u32| self.agents.flags[x as usize] & flags::FEMALE != 0;
        let married = self.systems.demography.married(a);
        let p = self.agents.partner[i];
        if p != NONE && (p as usize) < self.agents.len() {
            let r = if !married {
                "partner"
            } else if female(p) {
                "wife"
            } else {
                "husband"
            };
            rel(p, r);
        }
        let parents = [self.agents.parent_a[i], self.agents.parent_b[i]];
        for &q in &parents {
            if q != NONE && (q as usize) < self.agents.len() {
                rel(q, if female(q) { "mother" } else { "father" });
            }
        }
        if !visitor {
            let child_of = |k: u32, x: u32| {
                self.agents.parent_a[k as usize] == x || self.agents.parent_b[k as usize] == x
            };
            let kids: Vec<u32> = (0..self.agents.len() as u32)
                .filter(|k| self.agents.alive(*k) && child_of(*k, a))
                .collect();
            for &k in &kids {
                let adopted = self.systems.demography.adopted(k);
                let r = match (female(k), adopted) {
                    (true, false) => "daughter",
                    (false, false) => "son",
                    (true, true) => "adopted daughter",
                    (false, true) => "adopted son",
                };
                rel(k, r);
            }
            for k in 0..self.agents.len() as u32 {
                if k == a || !self.agents.alive(k) {
                    continue;
                }
                if parents.iter().any(|q| *q != NONE && child_of(k, *q)) {
                    rel(k, if female(k) { "sister" } else { "brother" });
                }
                if kids.iter().any(|c| child_of(k, *c)) {
                    rel(
                        k,
                        if female(k) {
                            "granddaughter"
                        } else {
                            "grandson"
                        },
                    );
                }
            }
            for &q in &parents {
                if q == NONE || (q as usize) >= self.agents.len() {
                    continue;
                }
                for g in [
                    self.agents.parent_a[q as usize],
                    self.agents.parent_b[q as usize],
                ] {
                    if g != NONE && (g as usize) < self.agents.len() {
                        rel(
                            g,
                            if female(g) {
                                "grandmother"
                            } else {
                                "grandfather"
                            },
                        );
                    }
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
        let (housing, rent_cents, rent_arrears) = self.housing_text(a);
        let businesses = self
            .systems
            .economy
            .firms
            .iter()
            .filter(|f| f.owner == a && f.open)
            .map(|f| self.building_name(f.building))
            .collect();
        let d = &self.systems.demography;
        let education = match d.education_of(a) {
            0 => "No formal education yet",
            1 => "Primary school",
            2 => "Secondary school",
            _ => "University degree",
        }
        .to_string();
        let relationship = {
            let p = self.agents.partner[i];
            if p != NONE && self.resident(p) {
                let together = self.agents.household[p as usize] == self.agents.household[i];
                if married {
                    format!("Married to {}", self.full_name(p))
                } else if together {
                    format!("Lives with {}", self.full_name(p))
                } else {
                    format!("Going out with {}", self.full_name(p))
                }
            } else if age_years(self.agents.birth[i], t) >= 18 {
                "Single".into()
            } else {
                "—".into()
            }
        };
        let health = match self.is_ill(a) {
            Some(x) if x.severe => "In hospital".to_string(),
            Some(_) => "Ill, resting at home".to_string(),
            None => "Healthy".to_string(),
        };
        let mut notes = Vec::new();
        if d.pregnancies.contains_key(&a) {
            notes.push("Expecting a baby".to_string());
        }
        if self.on_leave(a, t) {
            notes.push("On parental leave".to_string());
        }
        if let Some(j) = self.remote_job(a) {
            notes.push(format!(
                "Commutes to {} in {}",
                j.place,
                self.region_label(j.region)
            ));
        }
        if d.adopted(a) {
            notes.push("Adopted".to_string());
        }
        if let Some(m) = self.systems.justice.inmates.get(&a) {
            notes.push(if m.custody {
                format!("In custody, awaiting trial ({})", format_when(m.release))
            } else {
                format!("In jail until {}", format_when(m.release))
            });
        }
        if let Some(n) = self.systems.crime.records.get(&a) {
            notes.push(format!(
                "Criminal record: {n} conviction{}",
                if *n == 1 { "" } else { "s" }
            ));
        }
        if d.dflags
            .get(i)
            .is_some_and(|f| f & crate::systems::demography::dflags::ORPHAN != 0)
        {
            notes.push("Waiting for a family to adopt them".to_string());
        }
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
            workplace: if let Some(j) = self.remote_job(a) {
                format!("{} ({})", j.place, self.region_label(j.region))
            } else if work != NONE {
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
            housing,
            rent_cents,
            rent_arrears,
            businesses,
            education,
            relationship,
            health,
            notes,
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
            economy: self.economy_stats(),
            demography: self.demography_stats(),
            construction: self.construction_stats(),
            safety: self.safety_stats(),
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
                    NewsKind::BusiestPlace
                    | NewsKind::ShelvesEmpty
                    | NewsKind::Bankruptcy
                    | NewsKind::Reopened => self.building_name(n.subject),
                    NewsKind::Obituary => {
                        format!("{} {}", self.names.get(n.subject), self.names.get(n.n >> 8))
                    }
                    NewsKind::Centenarian => {
                        format!("{} {}", self.names.get(n.subject), self.names.get(n.n))
                    }
                    NewsKind::Wedding => {
                        format!("{} and {}", self.names.get(n.subject), self.names.get(n.n))
                    }
                    NewsKind::Migration => n.subject.to_string(),
                    NewsKind::WorksStart => c
                        .buildings
                        .get(n.subject as usize)
                        .map(|d| d.name.to_lowercase())
                        .unwrap_or_default(),
                    NewsKind::Inauguration => self.building_name(n.subject),
                    NewsKind::CrimeReport
                    | NewsKind::Arrest
                    | NewsKind::Verdict
                    | NewsKind::Acquittal => self.building_name(n.subject),
                    NewsKind::Homicide => {
                        format!("{} {}", self.names.get(n.subject), self.names.get(n.n))
                    }
                    NewsKind::Commuters => self.region_label(n.subject as u16),
                    _ => self.name().to_string(),
                };
                let crime_idx = match kind {
                    NewsKind::CrimeReport
                    | NewsKind::Arrest
                    | NewsKind::Verdict
                    | NewsKind::Acquittal => Some((n.n & 0xFF) as usize),
                    _ => None,
                };
                let crime = crime_idx
                    .and_then(|k| c.crimes.get(k))
                    .map(|x| x.name.clone())
                    .unwrap_or_default();
                let shown_n = match kind {
                    NewsKind::Obituary => n.n & 0xFF,
                    NewsKind::Verdict => n.n >> 8,
                    _ => n.n,
                };
                let amount = match kind {
                    NewsKind::Budget => format_money(n.n as i64 * 100),
                    _ => format_money(n.n as i64),
                };
                let f = |s: &str| {
                    s.replace("{place}", &place)
                        .replace("{amount}", &amount)
                        .replace("{n}", &shown_n.to_string())
                        .replace("{city}", self.name())
                        .replace("{crime_lower}", &crime.to_lowercase())
                        .replace("{crime}", &crime)
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

    fn housing_text(&self, a: AgentId) -> (String, i64, u8) {
        use crate::systems::economy::TenureKind;
        let hh = self.agents.household[a as usize];
        let Some(t) = (hh != NONE)
            .then(|| self.systems.economy.tenure.get(hh as usize))
            .flatten()
        else {
            return ("—".into(), 0, 0);
        };
        let text = match t.kind {
            TenureKind::Owner => "Owns their home".to_string(),
            TenureKind::Municipal => format!(
                "Rents from the municipal housing company for {} a month",
                format_money(t.rent)
            ),
            TenureKind::Agency => {
                let f = &self.systems.economy.firms[t.landlord as usize];
                format!(
                    "Rents from {} for {} a month",
                    self.building_name(f.building),
                    format_money(t.rent)
                )
            }
            TenureKind::PrivateLandlord => {
                format!(
                    "Rents from {} for {} a month",
                    self.full_name(t.landlord),
                    format_money(t.rent)
                )
            }
        };
        (text, t.rent, t.arrears)
    }

    pub fn building_card(&self, b: BuildingId) -> Option<BuildingCard> {
        if b as usize >= self.buildings.len() {
            return None;
        }
        let def = &content().buildings[self.buildings.def[b as usize] as usize];
        let occupants = self
            .occupants(b)
            .into_iter()
            .take(40)
            .map(|a| (a, self.full_name(a)))
            .collect();
        let firm = self.firm_at(b).map(|f| {
            use livingsim_content::FirmKind;
            let fm = &self.systems.economy.firms[f as usize];
            let staff = (0..self.agents.len() as u32)
                .filter(|a| {
                    self.agents.work[*a as usize] == b
                        && self.agents.profession[*a as usize] != u16::MAX
                })
                .count() as u32;
            let price = match fm.kind {
                FirmKind::Shop => format!("{} a meal", format_money(fm.price)),
                FirmKind::Farm | FirmKind::Warehouse => {
                    format!("{} a unit (wholesale)", format_money(fm.price))
                }
                FirmKind::FoodService | FirmKind::Leisure => {
                    format!("{}% of the usual price", fm.price / 10)
                }
                _ => "—".into(),
            };
            FirmInfo {
                kind: format!("{:?}", fm.kind),
                open: fm.open,
                owner: if fm.owner == NONE {
                    if fm.kind == FirmKind::Public {
                        "the town hall".into()
                    } else {
                        "nobody".into()
                    }
                } else {
                    self.full_name(fm.owner)
                },
                cash_cents: fm.cash,
                price,
                stock: fm.stock,
                staff,
                positions: fm.positions as u32,
                revenue_yesterday_cents: fm.revenue_yesterday,
                costs_yesterday_cents: fm.costs_yesterday,
                customers_yesterday: fm.customers_yesterday,
            }
        });
        Some(BuildingCard {
            id: b,
            name: self.building_name(b),
            kind: def.name.clone(),
            floors: def.floors,
            dwellings: def.dwellings,
            present: self.buildings.present.get(b as usize).copied().unwrap_or(0),
            occupants,
            firm,
            site: self.site_info(b),
            landlord: self.home_owner(b).map(|o| self.owner_label(o)),
        })
    }

    fn owner_label(&self, o: crate::systems::construction::HomeOwner) -> String {
        use crate::systems::construction::HomeOwner;
        match o {
            HomeOwner::Municipal => "the municipal housing company".into(),
            HomeOwner::Agency(f) => {
                self.building_name(self.systems.economy.firms[f as usize].building)
            }
            HomeOwner::Landlord(a) => self.full_name(a),
        }
    }

    fn funder_label(&self, f: crate::systems::construction::Funder) -> String {
        use crate::systems::construction::Funder;
        match f {
            Funder::Treasury => "the town hall".into(),
            Funder::Firm(x) => self.building_name(self.systems.economy.firms[x as usize].building),
            Funder::Agent(a) => self.full_name(a),
        }
    }

    fn phase_label(p: &crate::systems::construction::Project) -> String {
        use crate::systems::construction::{Phase, Work};
        let road = matches!(p.work, Work::Road { .. });
        let name = |ph: Phase| match (ph, road) {
            (Phase::Foundation, false) => "foundations",
            (Phase::Structure, false) => "structure",
            (Phase::Roof, false) => "roof and finishing",
            (Phase::Foundation, true) => "earthworks",
            (Phase::Structure, true) => "paving",
            (Phase::Finished, _) => "finished",
            _ => "works",
        };
        match p.phase {
            Phase::Planned => format!("waiting for money ({})", name(p.next)),
            ph => name(ph).to_string(),
        }
    }

    fn site_info(&self, b: BuildingId) -> Option<SiteInfo> {
        use crate::systems::construction::{Phase, Work};
        let p = self.site_project(b)?;
        if p.phase == Phase::Finished {
            return None;
        }
        let Work::Building { def, .. } = p.work else {
            return None;
        };
        let k = &content().construction;
        let units: u32 = p.units.iter().sum();
        let ph = match p.phase {
            Phase::Planned => p.next,
            x => x,
        };
        let idx = match ph {
            Phase::Foundation => 0,
            Phase::Structure => 1,
            _ => 2,
        };
        let need = p.units[idx].max(1);
        let firm = p.builder;
        Some(SiteInfo {
            building: content().buildings[def as usize].name.clone(),
            phase: Region::phase_label(p),
            progress_permille: p.progress(),
            phase_permille: if p.phase == Phase::Planned {
                0
            } else {
                (p.done * 1000 / need).min(1000)
            },
            funder: self.funder_label(p.funder),
            builder: if firm == NONE {
                "outside contractors".into()
            } else {
                self.building_name(self.systems.economy.firms[firm as usize].building)
            },
            days: (self.tick.saturating_sub(p.started) / crate::time::TICKS_PER_DAY),
            cost_cents: units as i64 * (k.labour_per_unit_cents + k.materials_per_unit_cents),
            workers_here: self.buildings.present.get(b as usize).copied().unwrap_or(0) as u32,
        })
    }

    pub fn safety_stats(&self) -> SafetyStats {
        use crate::systems::crime::Status;
        let c = content();
        let cr = &self.systems.crime;
        let since = self.tick.saturating_sub(30 * crate::time::TICKS_PER_DAY);
        let mut by = vec![0u32; c.crimes.len()];
        let (mut solved, mut convicted) = (0, 0);
        for x in cr.incidents.iter().filter(|x| x.tick >= since) {
            if let Some(v) = by.get_mut(x.crime as usize) {
                *v += 1;
            }
            if !matches!(x.status, Status::Open | Status::Cold) {
                solved += 1;
            }
            if x.status == Status::Convicted {
                convicted += 1;
            }
        }
        let police = c.tag("police");
        let officers = (0..self.agents.len())
            .filter(|&a| {
                let w = self.agents.work[a];
                w != NONE
                    && self.agents.profession[a] != u16::MAX
                    && police.is_some_and(|p| {
                        c.buildings[self.buildings.def[w as usize] as usize]
                            .tags
                            .contains(&p)
                    })
            })
            .count() as u32;
        let recent = cr
            .incidents
            .iter()
            .rev()
            .take(12)
            .map(|x| IncidentLine {
                building: x.building,
                what: c.crimes[x.crime as usize].name.clone(),
                place: self.building_name(x.building),
                when: format_when(x.tick),
                status: Self::status_label(x.status).into(),
            })
            .collect();
        SafetyStats {
            by_crime_30d: c.crimes.iter().map(|x| x.name.clone()).zip(by).collect(),
            solved_30d: solved,
            convictions_30d: convicted,
            inmates: self.systems.justice.inmates.len() as u32,
            inmates_from_elsewhere: self.systems.justice.guests.len() as u32,
            officers,
            safety: cr.history.last().map(|h| h.safety).unwrap_or(1000),
            recent,
            history: cr.history.clone(),
        }
    }

    fn status_label(s: crate::systems::crime::Status) -> &'static str {
        use crate::systems::crime::Status;
        match s {
            Status::Open => "under investigation",
            Status::Solved => "suspect identified",
            Status::Arrested => "suspect arrested, awaiting trial",
            Status::Convicted => "convicted",
            Status::Acquitted => "acquitted",
            Status::Fined => "fined",
            Status::Cold => "unsolved",
        }
    }

    pub fn construction_stats(&self) -> ConstructionStats {
        use crate::systems::construction::Work;
        let c = &self.systems.construction;
        let open = self
            .open_projects()
            .into_iter()
            .map(|p| ProjectLine {
                site: match p.work {
                    Work::Building { site, .. } => site,
                    Work::Road { .. } => NONE,
                },
                what: self.project_label(p),
                phase: Region::phase_label(p),
                progress_permille: p.progress(),
                funder: self.funder_label(p.funder),
                days: (self.tick.saturating_sub(p.started) / crate::time::TICKS_PER_DAY),
            })
            .collect();
        ConstructionStats {
            open,
            started: c.total.started + c.today.started,
            finished: c.total.finished + c.today.finished,
            streets: c.total.streets + c.today.streets,
            units_yesterday: c.yesterday.units,
            map_tiles: self.map.width as u32 * self.map.height as u32,
        }
    }

    pub fn economy_stats(&self) -> EconomyStats {
        use crate::systems::economy::TenureKind;
        let e = &self.systems.economy;
        let last = e.history.last().cloned().unwrap_or_default();
        EconomyStats {
            treasury_cents: e.treasury,
            income_tax_permille: e.income_tax_permille,
            sales_tax_permille: e.sales_tax_permille,
            unemployment_permille: last.unemployment_permille,
            food_price_cents: last.food_price,
            avg_rent_cents: last.avg_rent,
            firms_open: last.firms_open,
            firms_closed: last.firms_closed,
            owners: e
                .tenure
                .iter()
                .filter(|t| t.kind == TenureKind::Owner)
                .count() as u32,
            renters: e
                .tenure
                .iter()
                .filter(|t| t.kind != TenureKind::Owner)
                .count() as u32,
            output_cents: last.output,
            exports_cents: e.yesterday.exports,
            imports_cents: e.yesterday.imports,
            rent_paid_cents: e.yesterday.rent_paid,
            hires: e.yesterday.hires,
            layoffs: e.yesterday.layoffs,
            history: e.history.clone(),
        }
    }

    pub fn demography_stats(&self) -> DemographyStats {
        let d = &self.systems.demography;
        let t = self.tick;
        let mut pyramid = vec![(0u32, 0u32); 19];
        let (mut ages, mut n, mut couples, mut grads) = (0u64, 0u64, 0u32, 0u32);
        for a in 0..self.agents.len() as u32 {
            if !self.resident(a) {
                continue;
            }
            let age = age_years(self.agents.birth[a as usize], t);
            let band = ((age / 5) as usize).min(18);
            if self.agents.flags[a as usize] & flags::FEMALE != 0 {
                pyramid[band].1 += 1;
            } else {
                pyramid[band].0 += 1;
            }
            ages += age as u64;
            n += 1;
            if self.agents.partner[a as usize] != NONE && self.agents.partner[a as usize] > a {
                couples += 1;
            }
            if d.education_of(a) >= 3 {
                grads += 1;
            }
        }
        let last = |f: fn(&crate::systems::demography::DemoSample) -> u32| {
            d.history.iter().rev().take(30).map(f).sum()
        };
        DemographyStats {
            pyramid,
            average_age: (ages / n.max(1)) as u32,
            births_30d: last(|s| s.births),
            deaths_30d: last(|s| s.deaths),
            marriages_30d: last(|s| s.marriages),
            arrivals_30d: last(|s| s.immigrants),
            departures_30d: last(|s| s.emigrants),
            ill: d.illness.len() as u32,
            pregnant: d.pregnancies.len() as u32,
            couples,
            commuters_out: d.remote_jobs.len() as u32,
            commuters_in: d.remote_staff.len() as u32,
            university_graduates: grads,
            history: d.history.clone(),
        }
    }
}
