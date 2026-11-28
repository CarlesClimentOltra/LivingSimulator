//! Phase 5: investigations, arrests, trials, sentences and jail.
//!
//! Every police shift adds evidence to the open cases (most serious first); witnesses add
//! evidence at the scene. With enough evidence the suspect is arrested: minor offences end
//! with a fine, the rest with custody until the trial. Towns without a police station are
//! covered by officers from the city, and their convicts serve time in the city's jail.

use super::crime::Status;
use super::economy::Account;
use super::DailyContext;
use crate::agents::{AgentId, ACT_AWAY};
use crate::catalog::{catalog, content};
use crate::messages::Body;
use crate::region::{AwayInfo, NewsItem, Region};
use crate::rng::keyed;
use crate::roads::NONE;
use crate::time::{Tick, TICKS_PER_DAY};
use livingsim_content::{JournalKind, NewsKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const K_VERDICT: u64 = 0xD1;
const K_SENTENCE: u64 = 0xD2;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inmate {
    pub incident: u32,
    pub crime: u16,
    pub release: Tick,
    /// Held until the trial (not yet convicted).
    pub custody: bool,
}

/// Someone from another town serving time here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guest {
    pub crime: u16,
    pub release: Tick,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JusticeState {
    pub inmates: BTreeMap<AgentId, Inmate>,
    /// (region, agent) → inmates from other towns held in this town's jail.
    pub guests: BTreeMap<(u16, u32), Guest>,
}

impl JusticeState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // Justice needs the whole region; see `Region::justice_daily`.
    }
}

impl Region {
    fn has_tag(&self, tag: &str) -> bool {
        content()
            .tag(tag)
            .is_some_and(|t| !self.buildings.with_tag(t).is_empty())
    }

    pub(crate) fn justice_daily(&mut self, t: Tick) {
        // Towns without a police station: officers from the city investigate.
        if !self.has_tag("police") {
            let pts = content().justice.outside_police_points_per_day;
            self.add_evidence(pts, t);
        }
    }

    /// A police officer finished a shift.
    pub(crate) fn police_shift(&mut self, t: Tick) {
        self.add_evidence(content().justice.points_per_officer_shift, t);
    }

    /// Spreads evidence points over open cases, most serious (then oldest) first.
    fn add_evidence(&mut self, mut points: u32, _t: Tick) {
        let c = content();
        while points > 0 {
            let best = self
                .systems
                .crime
                .incidents
                .iter()
                .enumerate()
                .filter(|(_, x)| x.status == Status::Open && x.lead)
                .max_by_key(|(_, x)| (c.crimes[x.crime as usize].severity, std::cmp::Reverse(x.id)))
                .map(|(i, _)| i);
            let Some(i) = best else { return };
            let x = &mut self.systems.crime.incidents[i];
            let need = c.crimes[x.crime as usize].evidence;
            let add = points.min(need.saturating_sub(x.evidence).max(1));
            x.evidence += add;
            points -= add;
            if x.evidence >= need {
                x.status = Status::Solved;
                self.systems.crime.today.solved += 1;
            }
        }
    }

    /// When an inmate is due out (or due in court).
    pub fn inmate_release(&self, a: AgentId) -> Option<Tick> {
        self.systems.justice.inmates.get(&a).map(|m| m.release)
    }

    fn jail_building(&self, near: u32) -> Option<u32> {
        self.building_with_tag("jail", near, false)
    }

    /// Puts an inmate in the jail building (or sends them to the city's jail).
    fn lock_up(&mut self, a: AgentId, t: Tick) {
        let Some(release) = self.inmate_release(a) else {
            return;
        };
        let i = a as usize;
        if let Some(jail) = self.jail_building(self.agents.home[i]) {
            self.serve(a, t, jail, release);
            return;
        }
        let Some(g) = self.gateways.first().cloned() else {
            // Nowhere to hold them: house arrest at home.
            return;
        };
        let crime = self.systems.justice.inmates[&a].crime;
        self.set_place(a, NONE);
        self.agents.act[i] = ACT_AWAY;
        self.agents.until[i] = release;
        self.away.insert(
            a,
            AwayInfo {
                region: g.to_region,
                gateway: 0,
                return_at: release,
            },
        );
        self.set_event(a, release);
        self.send(
            g.to_region,
            Body::Prisoner {
                agent: a,
                crime,
                release,
            },
        );
    }

    /// One stay in the jail building, until the release or for a day.
    fn serve(&mut self, a: AgentId, t: Tick, jail: u32, release: Tick) {
        let i = a as usize;
        self.set_place(a, jail);
        self.agents.act[i] = catalog().serve_activity;
        let until = release.min(t + TICKS_PER_DAY).max(t + 1);
        self.agents.until[i] = until;
        self.agents.needs_t[i] = t;
        self.set_event(a, until);
    }

    /// Called by `decide`: inmates stay in jail. Returns true when handled.
    pub(crate) fn decide_inmate(&mut self, a: AgentId, t: Tick) -> bool {
        let Some(release) = self.inmate_release(a) else {
            return false;
        };
        match self.jail_building(self.agents.home[a as usize]) {
            Some(jail) => {
                self.serve(a, t, jail, release);
                true
            }
            None => false,
        }
    }

    pub(crate) fn receive_prisoner(&mut self, src: u16, agent: u32, crime: u16, release: Tick) {
        if release <= self.tick {
            self.systems.justice.guests.remove(&(src, agent));
        } else {
            self.systems
                .justice
                .guests
                .insert((src, agent), Guest { crime, release });
        }
    }

    fn update_away_release(&mut self, a: AgentId, release: Tick) {
        if let Some(info) = self.away.get_mut(&a) {
            info.return_at = release;
            let to = info.region;
            self.agents.until[a as usize] = release;
            self.set_event(a, release);
            let crime = self
                .systems
                .justice
                .inmates
                .get(&a)
                .map(|m| m.crime)
                .unwrap_or(0);
            self.send(
                to,
                Body::Prisoner {
                    agent: a,
                    crime,
                    release,
                },
            );
        }
    }

    pub(crate) fn justice_hourly(&mut self, t: Tick) {
        let c = content();
        let j = &c.justice;
        // ---- arrests
        let solved: Vec<usize> = self
            .systems
            .crime
            .incidents
            .iter()
            .enumerate()
            .filter(|(_, x)| x.status == Status::Solved)
            .map(|(i, _)| i)
            .collect();
        for i in solved {
            let x = self.systems.crime.incidents[i].clone();
            let a = x.offender;
            if !self.resident(a) {
                self.systems.crime.incidents[i].status = Status::Cold;
                continue;
            }
            if self.away.contains_key(&a) || self.systems.justice.inmates.contains_key(&a) {
                continue; // out of town or already held: later
            }
            let cd = &c.crimes[x.crime as usize];
            self.journal(a, t, JournalKind::Arrested, 0, 0);
            self.systems.crime.today.arrests += 1;
            if cd.max_sentence_days == 0 {
                let fine = (j.fine_per_severity_cents * cd.severity as i64)
                    .min(self.agents.money[a as usize].max(0));
                if fine > 0 {
                    self.transfer(Account::Agent(a), Account::Treasury, fine);
                }
                self.journal(
                    a,
                    t,
                    JournalKind::Fined,
                    0,
                    fine.clamp(0, u32::MAX as i64) as u32,
                );
                self.systems.crime.incidents[i].status = Status::Fined;
                self.systems.crime.today.fines += 1;
                continue;
            }
            let trial_at = t + j.trial_after_days as Tick * TICKS_PER_DAY;
            {
                let inc = &mut self.systems.crime.incidents[i];
                inc.status = Status::Arrested;
                inc.trial_at = trial_at;
            }
            self.systems.justice.inmates.insert(
                a,
                Inmate {
                    incident: x.id,
                    crime: x.crime,
                    // A little after the trial, so the verdict comes first.
                    release: trial_at + 60,
                    custody: true,
                },
            );
            self.lock_up(a, t);
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::Arrest as u16,
                subject: x.building,
                n: x.crime as u32,
            });
        }

        // ---- trials
        let due: Vec<usize> = self
            .systems
            .crime
            .incidents
            .iter()
            .enumerate()
            .filter(|(_, x)| x.status == Status::Arrested && x.trial_at <= t)
            .map(|(i, _)| i)
            .collect();
        for i in due {
            let x = self.systems.crime.incidents[i].clone();
            let a = x.offender;
            let cd = &c.crimes[x.crime as usize];
            if !self.resident(a) {
                self.systems.crime.incidents[i].status = Status::Cold;
                self.systems.justice.inmates.remove(&a);
                continue;
            }
            let need = cd.evidence.max(1) as i64;
            let extra = (x.evidence as i64 - need) * 1000 / need;
            let chance = (j.conviction_base_permille as i64 + extra / 2).clamp(300, 980) as u64;
            let guilty = keyed(self.seed, x.id as u64, t as u64, K_VERDICT) % 1000 < chance;
            if guilty {
                let span = (cd.max_sentence_days - cd.min_sentence_days) as u64 + 1;
                let days = cd.min_sentence_days
                    + (keyed(self.seed, x.id as u64, 1, K_SENTENCE) % span) as u32;
                let served = t.saturating_sub(x.tick) / TICKS_PER_DAY;
                let left = days.saturating_sub(served).max(1);
                let release = t + left as Tick * TICKS_PER_DAY;
                {
                    let inc = &mut self.systems.crime.incidents[i];
                    inc.status = Status::Convicted;
                    inc.sentence_days = days;
                }
                if let Some(m) = self.systems.justice.inmates.get_mut(&a) {
                    m.release = release;
                    m.custody = false;
                }
                *self.systems.crime.records.entry(a).or_insert(0) += 1;
                self.quit_job(a);
                self.journal(
                    a,
                    t,
                    JournalKind::Sentenced,
                    days.min(u16::MAX as u32) as u16,
                    0,
                );
                self.systems.crime.today.convictions += 1;
                self.update_away_release(a, release);
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Verdict as u16,
                    subject: x.building,
                    n: (days << 8) | x.crime as u32,
                });
            } else {
                self.systems.crime.incidents[i].status = Status::Acquitted;
                self.systems.crime.today.acquittals += 1;
                self.journal(a, t, JournalKind::Acquitted, 0, 0);
                self.release(a, t);
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Acquittal as u16,
                    subject: x.building,
                    n: x.crime as u32,
                });
            }
        }

        // ---- releases
        let out: Vec<AgentId> = self
            .systems
            .justice
            .inmates
            .iter()
            .filter(|(_, m)| !m.custody && m.release <= t)
            .map(|(a, _)| *a)
            .collect();
        for a in out {
            self.journal(a, t, JournalKind::Released, 0, 0);
            self.release(a, t);
        }
        self.systems.justice.guests.retain(|_, g| g.release > t);
    }

    fn release(&mut self, a: AgentId, t: Tick) {
        self.systems.justice.inmates.remove(&a);
        if self.away.contains_key(&a) {
            // Held in another town: they come back now.
            self.update_away_release(a, t + 1);
        } else if self.resident(a) {
            self.set_event(a, t);
        }
    }
}
