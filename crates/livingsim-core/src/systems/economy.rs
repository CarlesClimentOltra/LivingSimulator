//! Phase 2: the economy. Money is US dollar cents (`i64`) and circulates in a closed loop:
//!
//! ```text
//! households ──purchases / rent──▶ firms ──wages──▶ households
//!     │                              │
//!     └── income tax, sales tax ─▶ town hall ◀── taxes
//!                                    │
//!                 public wages, pensions, benefits, maintenance
//! ```
//!
//! Every movement of money goes through [`Region::transfer`], so the sum of all accounts
//! (agents, firms, treasury and the two *external* accounts: the rest of the world and
//! other regions) never changes. A test checks this invariant.
//!
//! Cost model: the economy runs once per simulated day per firm (06:00) and at events that
//! already exist (end of a shift, a purchase). Nothing runs per agent per tick.

// Loops index several parallel vectors while also mutating `self`, so ranges are clearer.
#![allow(clippy::needless_range_loop)]

use super::DailyContext;
use crate::agents::{AgentId, ACT_FREE_SLOT};
use crate::buildings::BuildingId;
use crate::catalog::{catalog, content};
use crate::messages::Body;
use crate::names::generate;
use crate::region::{NewsItem, Region};
use crate::rng::{keyed, Rng};
use crate::roads::NONE;
use crate::time::{age_years, date, Tick, TICKS_PER_DAY};
use livingsim_content::{FirmKind, JournalKind, NewsKind};
use serde::{Deserialize, Serialize};

pub const NO_GOOD: u16 = u16::MAX;
/// Days of history kept for the charts.
pub const HISTORY_DAYS: usize = 60;
/// Price of services is a factor in ‰ of the activity's base cost.
pub const BASE_FACTOR: i64 = 1000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Firm {
    pub building: BuildingId,
    pub kind: FirmKind,
    pub good: u16,
    pub cash: i64,
    /// Units of `good` (or food, for food services) in stock.
    pub stock: u32,
    /// Cents per unit for goods; ‰ of the base cost for services.
    pub price: i64,
    /// Agent who owns the business (private firms), or `NONE`.
    pub owner: u32,
    pub open: bool,
    pub closed_since: Tick,
    pub days_in_red: u16,
    /// Times the business has been reopened after a bankruptcy.
    pub generation: u16,
    pub revenue_today: i64,
    pub costs_today: i64,
    pub customers_today: u32,
    pub revenue_yesterday: i64,
    pub costs_yesterday: i64,
    pub customers_yesterday: u32,
    /// Average cost of the units bought yesterday (warehouses), for cost-plus pricing.
    pub unit_cost: i64,
    /// Jobs this firm offers (≤ the building's slots; calibrated to the local workforce).
    pub positions: u16,
    pub sold_today: u32,
    pub sold_yesterday: u32,
    /// Smoothed daily profit (about a week), used for hiring and layoffs.
    pub profit_avg: i64,
}

impl Firm {
    pub fn is_private(&self) -> bool {
        self.kind != FirmKind::Public
    }
}

/// How a household holds its home.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenureKind {
    Owner = 0,
    Municipal = 1,
    Agency = 2,
    PrivateLandlord = 3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tenure {
    pub kind: TenureKind,
    /// Firm (agency) or agent (private landlord), else `NONE`.
    pub landlord: u32,
    /// Monthly rent in cents (0 for owners).
    pub rent: i64,
    /// Months of unpaid rent in a row.
    pub arrears: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EconTotals {
    pub retail_sales: i64,
    pub service_sales: i64,
    pub exports: i64,
    pub imports: i64,
    pub wages_private: i64,
    pub wages_public: i64,
    pub income_tax: i64,
    pub sales_tax: i64,
    pub rent_paid: i64,
    pub pensions_benefits: i64,
    pub maintenance: i64,
    pub hires: u32,
    pub layoffs: u32,
    pub bankruptcies: u32,
    pub reopenings: u32,
    pub food_imported: u32,
    pub food_exported: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaySample {
    pub tick: Tick,
    /// Average shop price of a meal, cents.
    pub food_price: i64,
    /// Job seekers per 1000 people of working age who want a job.
    pub unemployment_permille: u32,
    pub treasury: i64,
    pub avg_savings: i64,
    /// Firms' revenue + public wages, yesterday (a GDP-like indicator).
    pub output: i64,
    pub avg_rent: i64,
    pub firms_open: u32,
    pub firms_closed: u32,
    pub income_tax_permille: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EconomyState {
    pub bootstrapped: bool,
    pub treasury: i64,
    /// Money held by the rest of the world because of this region (imports minus exports…).
    pub world: i64,
    /// Money held by other regions because of this region.
    pub other_regions: i64,
    pub income_tax_permille: u16,
    pub sales_tax_permille: u16,
    pub firms: Vec<Firm>,
    /// Building → firm index, or `NONE`.
    pub firm_of: Vec<u32>,
    /// Household → tenure.
    pub tenure: Vec<Tenure>,
    pub today: EconTotals,
    pub yesterday: EconTotals,
    pub history: Vec<DaySample>,
    /// Public spending of the last 30 days (for the tax rule).
    pub spending_30d: Vec<i64>,
}

impl EconomyState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // The economy needs the whole region; see `Region::economy_daily`.
    }
}

/// A money account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Account {
    Agent(AgentId),
    Firm(u32),
    Treasury,
    World,
    OtherRegions,
}

impl Region {
    // ------------------------------------------------------------------ accounts

    /// Moves money between accounts. The only way money changes hands.
    pub fn transfer(&mut self, from: Account, to: Account, amount: i64) {
        if amount == 0 || from == to {
            return;
        }
        self.adjust(from, -amount);
        self.adjust(to, amount);
    }

    fn adjust(&mut self, acc: Account, delta: i64) {
        let e = &mut self.systems.economy;
        match acc {
            Account::Agent(a) => self.agents.money[a as usize] += delta,
            Account::Firm(f) => e.firms[f as usize].cash += delta,
            Account::Treasury => e.treasury += delta,
            Account::World => e.world += delta,
            Account::OtherRegions => e.other_regions += delta,
        }
    }

    /// Sum of every account (must never change).
    pub fn money_total(&self) -> i128 {
        let e = &self.systems.economy;
        let agents: i128 = self.agents.money.iter().map(|m| *m as i128).sum();
        let firms: i128 = e.firms.iter().map(|f| f.cash as i128).sum();
        agents + firms + e.treasury as i128 + e.world as i128 + e.other_regions as i128
    }

    pub fn firm_at(&self, b: BuildingId) -> Option<u32> {
        let f = *self.systems.economy.firm_of.get(b as usize)?;
        (f != NONE).then_some(f)
    }

    /// True if the building hosts a business that is currently closed (bankrupt).
    pub fn firm_closed(&self, b: BuildingId) -> bool {
        self.firm_at(b)
            .is_some_and(|f| !self.systems.economy.firms[f as usize].open)
    }

    fn wage_of(&self, a: AgentId) -> i64 {
        let p = self.agents.profession[a as usize];
        if p == u16::MAX {
            0
        } else {
            content().professions[p as usize].wage_per_shift_cents
        }
    }

    // ------------------------------------------------------------------ bootstrap

    /// Creates firms, tenures and starting balances. Runs once per region (at generation,
    /// or when migrating a Phase 1 snapshot). Deterministic in the region seed.
    pub fn bootstrap_economy(&mut self) {
        if self.systems.economy.bootstrapped {
            return;
        }
        let c = content();
        let econ = &c.economy;
        let mut rng = Rng::new(self.seed, 0xEC0);
        let mut e = EconomyState {
            bootstrapped: true,
            income_tax_permille: econ.income_tax_permille,
            sales_tax_permille: econ.sales_tax_permille,
            firm_of: vec![NONE; self.buildings.len()],
            ..Default::default()
        };
        // Adults who may own businesses or rent out homes, richest first.
        let mut adults: Vec<AgentId> = (0..self.agents.len() as u32)
            .filter(|a| {
                self.agents.alive(*a)
                    && !self.agents.is_visitor(*a)
                    && (25..=75).contains(&age_years(self.agents.birth[*a as usize], self.tick))
            })
            .collect();
        adults.sort_by_key(|a| (std::cmp::Reverse(self.agents.money[*a as usize]), *a));
        let rich = &adults[..adults.len().div_ceil(2)];

        for b in 0..self.buildings.len() {
            let def = &c.buildings[self.buildings.def[b] as usize];
            let Some(spec) = &def.firm else { continue };
            let base_price = match spec.kind {
                FirmKind::Shop => econ.meal_price_cents,
                FirmKind::Farm => econ.farm_price_cents,
                FirmKind::Warehouse => {
                    econ.farm_price_cents * (1000 + econ.wholesale_markup_permille as i64) / 1000
                }
                FirmKind::Factory => spec
                    .good
                    .map(|g| c.goods[g as usize].base_price_cents)
                    .unwrap_or(0),
                _ => BASE_FACTOR,
            };
            let payroll: i64 = def
                .jobs
                .iter()
                .map(|j| j.count as i64 * c.professions[j.profession as usize].wage_per_shift_cents)
                .sum();
            let private = spec.kind != FirmKind::Public;
            let owner = if private && !rich.is_empty() {
                *rng.pick(rich)
            } else {
                NONE
            };
            let stock = match spec.kind {
                FirmKind::Shop => self.buildings.stock[b].max(def.stock / 2),
                FirmKind::FoodService => food_service_target(def.capacity),
                FirmKind::Warehouse => 2000,
                _ => 0,
            };
            e.firm_of[b] = e.firms.len() as u32;
            e.firms.push(Firm {
                building: b as BuildingId,
                kind: spec.kind,
                good: spec.good.unwrap_or(NO_GOOD),
                cash: if private {
                    payroll * econ.firm_start_cash_days as i64
                } else {
                    0
                },
                stock,
                price: base_price,
                owner,
                open: true,
                closed_since: 0,
                days_in_red: 0,
                generation: 0,
                revenue_today: 0,
                costs_today: 0,
                customers_today: 0,
                revenue_yesterday: 0,
                costs_yesterday: 0,
                customers_yesterday: 0,
                unit_cost: econ.farm_price_cents,
                positions: def.jobs.iter().map(|j| j.count).sum(),
                sold_today: 0,
                sold_yesterday: 0,
                profit_avg: 0,
            });
        }
        // Calibrate private positions so that about 96% of the workforce (demand-based staffing and job changes bring real unemployment to ~8%) can have a job
        // (the rest is natural unemployment).
        {
            let cat = catalog();
            let workforce = (0..self.agents.len() as u32)
                .filter(|a| {
                    let i = *a as usize;
                    self.agents.alive(*a)
                        && !self.agents.is_visitor(*a)
                        && self.agents.schedule[i] != cat.schedule_university
                        && (18..=65).contains(&age_years(self.agents.birth[i], self.tick))
                })
                .count() as i64;
            let public: i64 = e
                .firms
                .iter()
                .filter(|f| !f.is_private())
                .map(|f| f.positions as i64)
                .sum();
            let private: i64 = e
                .firms
                .iter()
                .filter(|f| f.is_private())
                .map(|f| f.positions as i64)
                .sum();
            let target = (workforce * 96 / 100 - public).max(0);
            if private > target && private > 0 {
                for f in e.firms.iter_mut().filter(|f| f.is_private()) {
                    f.positions =
                        ((f.positions as i64 * target + private / 2) / private).max(1) as u16;
                }
            }
        }
        // Workers beyond a firm's calibrated positions start out looking for a job.
        {
            let mut staff: Vec<u16> = vec![0; e.firms.len()];
            for a in 0..self.agents.len() {
                let w = self.agents.work[a];
                if w == NONE || self.agents.profession[a] == u16::MAX {
                    continue;
                }
                let Some(f) = e.firm_of.get(w as usize).copied().filter(|f| *f != NONE) else {
                    continue;
                };
                let fm = &e.firms[f as usize];
                if staff[f as usize] >= fm.positions && fm.is_private() {
                    self.agents.work[a] = NONE;
                    self.agents.profession[a] = u16::MAX;
                    self.agents.schedule[a] = catalog().schedule_unemployed;
                } else {
                    staff[f as usize] += 1;
                }
            }
        }
        // Initial job matching: job seekers fill the free positions (nearest of a sample).
        {
            let cat = catalog();
            let mut staff: Vec<Vec<u16>> = e
                .firms
                .iter()
                .map(|f| {
                    vec![
                        0u16;
                        c.buildings[self.buildings.def[f.building as usize] as usize]
                            .jobs
                            .len()
                    ]
                })
                .collect();
            let mut total: Vec<u16> = vec![0; e.firms.len()];
            for a in 0..self.agents.len() {
                let w = self.agents.work[a];
                let p = self.agents.profession[a];
                if w == NONE || p == u16::MAX {
                    continue;
                }
                let Some(f) = e.firm_of.get(w as usize).copied().filter(|f| *f != NONE) else {
                    continue;
                };
                let jobs = &c.buildings[self.buildings.def[w as usize] as usize].jobs;
                if let Some(j) = jobs.iter().position(|j| j.profession == p) {
                    staff[f as usize][j] += 1;
                }
                total[f as usize] += 1;
            }
            // Open vacancies: (firm, job index), one entry per free position.
            let mut vacancies: Vec<(u32, usize)> = Vec::new();
            for (f, fm) in e.firms.iter().enumerate() {
                let jobs = &c.buildings[self.buildings.def[fm.building as usize] as usize].jobs;
                let mut free = fm.positions.saturating_sub(total[f]);
                for (j, js) in jobs.iter().enumerate() {
                    let n = js.count.saturating_sub(staff[f][j]).min(free);
                    for _ in 0..n {
                        vacancies.push((f as u32, j));
                    }
                    free -= n;
                }
            }
            rng.shuffle(&mut vacancies);
            for a in 0..self.agents.len() as u32 {
                if vacancies.is_empty() {
                    break;
                }
                let i = a as usize;
                if !self.agents.alive(a)
                    || self.agents.is_visitor(a)
                    || self.agents.schedule[i] != cat.schedule_unemployed
                {
                    continue;
                }
                let age = age_years(self.agents.birth[i], self.tick);
                if !(18..=65).contains(&age) {
                    continue;
                }
                let mut best: Option<(u32, usize)> = None;
                for _ in 0..12 {
                    let k = rng.below(vacancies.len());
                    let (f, j) = vacancies[k];
                    let b = e.firms[f as usize].building;
                    let prof =
                        c.buildings[self.buildings.def[b as usize] as usize].jobs[j].profession;
                    if c.professions[prof as usize].min_age as u32 > age {
                        continue;
                    }
                    let d = self.manhattan(self.agents.home[i], b);
                    if best.is_none_or(|(bd, bk)| (d, k) < (bd, bk)) {
                        best = Some((d, k));
                    }
                }
                let Some((_, k)) = best else { continue };
                let (f, j) = vacancies.swap_remove(k);
                let b = e.firms[f as usize].building;
                let prof = c.buildings[self.buildings.def[b as usize] as usize].jobs[j].profession;
                let pd = &c.professions[prof as usize];
                self.agents.work[i] = b;
                self.agents.profession[i] = prof;
                self.agents.schedule[i] =
                    pd.schedules[(rng.below(pd.schedules.len())) % pd.schedules.len()];
            }
        }
        // Treasury: enough for `treasury_start_days` of public wages, pensions and benefits.
        let mut public_day: i64 = 0;
        for a in 0..self.agents.len() as u32 {
            let i = a as usize;
            if !self.agents.alive(a) || self.agents.act[i] == ACT_FREE_SLOT {
                continue;
            }
            let w = self.agents.work[i];
            if w != NONE && self.agents.profession[i] != u16::MAX {
                if let Some(f) = e.firm_of.get(w as usize).copied().filter(|f| *f != NONE) {
                    if e.firms[f as usize].kind == FirmKind::Public {
                        public_day += self.wage_of(a);
                    }
                }
            }
            let s = self.agents.schedule[i];
            if s == catalog().schedule_retired {
                public_day += econ.pension_per_day_cents;
            } else if s == catalog().schedule_unemployed {
                public_day += econ.benefit_per_day_cents;
            }
        }
        e.treasury = public_day * econ.treasury_start_days as i64;

        // Housing tenure.
        let agencies: Vec<u32> = e
            .firms
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == FirmKind::RealEstate)
            .map(|(i, _)| i as u32)
            .collect();
        let (cx, cy) = (self.map.width as i64 / 2, self.map.height as i64 / 2);
        let half = (self.map.width as i64 / 2).max(1);
        for h in 0..self.households.len() {
            let home = self.households.home[h] as usize;
            let def = &c.buildings[self.buildings.def[home] as usize];
            let roll = rng.below(100) as u32;
            let kind = match def.dwellings {
                0..=1 => pick_tenure(roll, [65, 0, 15, 20]),
                2..=4 => pick_tenure(roll, [50, 0, 25, 25]),
                _ => pick_tenure(roll, [10, 30, 45, 15]),
            };
            let kind = if kind == TenureKind::Agency && agencies.is_empty() {
                TenureKind::Municipal
            } else {
                kind
            };
            let landlord = match kind {
                TenureKind::Agency => agencies[rng.below(agencies.len())],
                TenureKind::PrivateLandlord => {
                    // A richer adult from another household.
                    let mut l = NONE;
                    for _ in 0..8 {
                        if rich.is_empty() {
                            break;
                        }
                        let cand = *rng.pick(rich);
                        if self.agents.household[cand as usize] != h as u32 {
                            l = cand;
                            break;
                        }
                    }
                    l
                }
                _ => NONE,
            };
            let kind = if kind == TenureKind::PrivateLandlord && landlord == NONE {
                TenureKind::Municipal
            } else {
                kind
            };
            let dist = ((self.buildings.door_x[home] as i64 - cx).abs()
                + (self.buildings.door_y[home] as i64 - cy).abs())
            .min(half * 2);
            let location = 1300 - 600 * dist / (half * 2);
            let jitter = 950 + rng.below(101) as i64;
            let rent = if kind == TenureKind::Owner {
                0
            } else {
                def.rent_cents * location / 1000 * jitter / 1000 / 100 * 100
            };
            e.tenure.push(Tenure {
                kind,
                landlord,
                rent,
                arrears: 0,
            });
        }
        self.systems.economy = e;
    }

    // ------------------------------------------------------------------ events

    /// End of a work shift: the employer pays the wage (minus income tax) and producers
    /// produce.
    pub(crate) fn econ_shift_done(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        let work = self.agents.work[i];
        let wage = self.wage_of(a);
        if wage == 0 || work == NONE {
            return;
        }
        let Some(f) = self.firm_at(work) else {
            // No firm (should not happen): the town pays.
            self.transfer(Account::Treasury, Account::Agent(a), wage);
            return;
        };
        let tax = wage * self.systems.economy.income_tax_permille as i64 / 1000;
        let firm = &self.systems.economy.firms[f as usize];
        let kind = firm.kind;
        if kind == FirmKind::Public {
            self.transfer(Account::Treasury, Account::Agent(a), wage - tax);
            self.systems.economy.today.wages_public += wage;
        } else {
            self.transfer(Account::Firm(f), Account::Agent(a), wage - tax);
            self.transfer(Account::Firm(f), Account::Treasury, tax);
            self.systems.economy.today.wages_private += wage;
            self.systems.economy.firms[f as usize].costs_today += wage;
        }
        self.systems.economy.today.income_tax += tax;
        self.today.wages_paid_cents += wage;
        if kind == FirmKind::Construction {
            self.construction_shift_done(a, f, t);
        }
        let police = content().tag("police");
        if police.is_some_and(|p| {
            content().buildings[self.buildings.def[work as usize] as usize]
                .tags
                .contains(&p)
        }) {
            self.police_shift(t);
        }
        self.journal(
            a,
            t,
            JournalKind::GotPaid,
            0,
            (wage - tax).clamp(0, u32::MAX as i64) as u32,
        );
        // Production.
        let c = content();
        let spec = c.buildings[self.buildings.def[work as usize] as usize]
            .firm
            .clone();
        if let Some(spec) = spec {
            match spec.kind {
                FirmKind::Farm => {
                    let fm = &mut self.systems.economy.firms[f as usize];
                    fm.stock = fm.stock.saturating_add(spec.per_shift);
                }
                FirmKind::Factory => {
                    let value = spec.per_shift as i64
                        * spec
                            .good
                            .map(|g| c.goods[g as usize].base_price_cents)
                            .unwrap_or(0);
                    self.transfer(Account::World, Account::Firm(f), value);
                    self.systems.economy.firms[f as usize].revenue_today += value;
                    self.systems.economy.today.exports += value;
                }
                FirmKind::Office => {
                    let value = spec.export_per_shift_cents;
                    self.transfer(Account::World, Account::Firm(f), value);
                    self.systems.economy.firms[f as usize].revenue_today += value;
                    self.systems.economy.today.exports += value;
                }
                _ => {}
            }
        }
    }

    /// Price of an activity at a building (services), or `None` if it cannot be served
    /// (closed, or no food left).
    pub fn service_price(&self, b: BuildingId, base_cost: i64) -> Option<i64> {
        let Some(f) = self.firm_at(b) else {
            return Some(base_cost);
        };
        let firm = &self.systems.economy.firms[f as usize];
        if !firm.open {
            return None;
        }
        let spec = content().buildings[self.buildings.def[b as usize] as usize]
            .firm
            .as_ref();
        let food = spec.map(|s| s.food_per_customer).unwrap_or(0);
        if firm.kind == FirmKind::FoodService && firm.stock < food {
            return None;
        }
        if firm.kind == FirmKind::Public {
            return Some(base_cost);
        }
        Some(base_cost * firm.price / BASE_FACTOR)
    }

    /// Charges a customer for a service. Returns false if it could not be served.
    pub(crate) fn sell_service(&mut self, a: AgentId, b: BuildingId, base_cost: i64) -> bool {
        let Some(price) = self.service_price(b, base_cost) else {
            return false;
        };
        if price <= 0 {
            return true;
        }
        if self.agents.money[a as usize] < price {
            return false;
        }
        match self.firm_at(b) {
            Some(f) if self.systems.economy.firms[f as usize].kind != FirmKind::Public => {
                let tax = self.sales_tax_of(price);
                self.transfer(Account::Agent(a), Account::Firm(f), price - tax);
                self.transfer(Account::Agent(a), Account::Treasury, tax);
                let spec = content().buildings[self.buildings.def[b as usize] as usize]
                    .firm
                    .clone();
                let fm = &mut self.systems.economy.firms[f as usize];
                fm.revenue_today += price - tax;
                fm.customers_today += 1;
                if let Some(s) = spec {
                    fm.stock = fm.stock.saturating_sub(s.food_per_customer);
                }
                self.systems.economy.today.service_sales += price;
                self.systems.economy.today.sales_tax += tax;
            }
            _ => self.transfer(Account::Agent(a), Account::Treasury, price),
        }
        true
    }

    fn sales_tax_of(&self, gross: i64) -> i64 {
        let st = self.systems.economy.sales_tax_permille as i64;
        gross * st / (1000 + st)
    }

    /// Buys groceries at a shop. Returns (units, total paid).
    pub(crate) fn sell_groceries(&mut self, a: AgentId, shop: BuildingId, want: u32) -> (u32, i64) {
        let Some(f) = self.firm_at(shop) else {
            return (0, 0);
        };
        let (open, stock, price) = {
            let fm = &self.systems.economy.firms[f as usize];
            (fm.open, fm.stock, fm.price.max(1))
        };
        if !open {
            return (0, 0);
        }
        let affordable = (self.agents.money[a as usize].max(0) / price).min(u32::MAX as i64) as u32;
        let units = want
            .min(stock)
            .min(affordable.max(if self.agents.money[a as usize] > 0 {
                1
            } else {
                0
            }));
        if units == 0 {
            return (0, 0);
        }
        let total = units as i64 * price;
        let tax = self.sales_tax_of(total);
        self.transfer(Account::Agent(a), Account::Firm(f), total - tax);
        self.transfer(Account::Agent(a), Account::Treasury, tax);
        let fm = &mut self.systems.economy.firms[f as usize];
        fm.stock -= units;
        fm.sold_today += units;
        fm.revenue_today += total - tax;
        fm.customers_today += 1;
        self.systems.economy.today.retail_sales += total;
        self.systems.economy.today.sales_tax += tax;
        (units, total)
    }

    /// Stock available at a shop (for the UI and decisions).
    pub fn shop_stock(&self, b: BuildingId) -> u32 {
        self.firm_at(b)
            .map(|f| self.systems.economy.firms[f as usize].stock)
            .unwrap_or(0)
    }

    // ------------------------------------------------------------------ inter-region trade

    /// Goods arriving from another region with an invoice: store them and pay.
    pub(crate) fn receive_sale(&mut self, src: u16, seller_firm: u32, qty: u32, unit_price: i64) {
        let buyer = self.food_buyer();
        let Some(buyer) = buyer else { return };
        let total = qty as i64 * unit_price;
        self.transfer(Account::Firm(buyer), Account::OtherRegions, total);
        let fm = &mut self.systems.economy.firms[buyer as usize];
        fm.stock = fm.stock.saturating_add(qty);
        fm.costs_today += total;
        self.systems.economy.today.imports += total;
        *self.today.food_in.entry(src).or_insert(0) += qty;
        self.send(
            src,
            Body::Payment {
                firm: seller_firm,
                amount: total,
            },
        );
    }

    pub(crate) fn receive_payment(&mut self, firm: u32, amount: i64) {
        if (firm as usize) < self.systems.economy.firms.len() {
            self.transfer(Account::OtherRegions, Account::Firm(firm), amount);
            self.systems.economy.firms[firm as usize].revenue_today += amount;
            self.systems.economy.today.exports += amount;
        } else {
            self.transfer(Account::OtherRegions, Account::Treasury, amount);
        }
    }

    /// Where incoming food goes: the first open warehouse, else the first open shop.
    fn food_buyer(&self) -> Option<u32> {
        let firms = &self.systems.economy.firms;
        let find = |k: FirmKind| {
            firms
                .iter()
                .position(|f| f.kind == k && f.open)
                .map(|i| i as u32)
        };
        find(FirmKind::Warehouse).or_else(|| find(FirmKind::Shop))
    }

    /// A visitor arrives with their money; a visitor leaves with what is left.
    pub(crate) fn visitor_money_in(&mut self, a: AgentId, money: i64) {
        self.agents.money[a as usize] = 0;
        self.transfer(Account::OtherRegions, Account::Agent(a), money);
    }

    pub(crate) fn visitor_money_out(&mut self, a: AgentId) {
        let m = self.agents.money[a as usize];
        self.transfer(Account::Agent(a), Account::OtherRegions, m);
    }

    // ------------------------------------------------------------------ daily

    /// Runs every morning at 06:00 (from `Region::morning`).
    pub(crate) fn economy_daily(&mut self, t: Tick) {
        self.bootstrap_economy();
        // Roll the daily books.
        {
            let e = &mut self.systems.economy;
            e.yesterday = std::mem::take(&mut e.today);
            for f in e.firms.iter_mut() {
                f.revenue_yesterday = std::mem::take(&mut f.revenue_today);
                f.costs_yesterday = std::mem::take(&mut f.costs_today);
                f.customers_yesterday = std::mem::take(&mut f.customers_today);
                f.sold_yesterday = std::mem::take(&mut f.sold_today);
                f.profit_avg += (f.revenue_yesterday - f.costs_yesterday - f.profit_avg) / 7;
            }
            let spent =
                e.yesterday.wages_public + e.yesterday.pensions_benefits + e.yesterday.maintenance;
            e.spending_30d.push(spent);
            if e.spending_30d.len() > 30 {
                e.spending_30d.remove(0);
            }
        }
        self.supply_chain(t);
        self.adjust_prices();
        self.public_spending();
        if date(t).day == 1 && t >= TICKS_PER_DAY {
            self.monthly(t);
        }
        self.labour_market(t);
        self.sample_history(t);
        self.economy_news(t);
    }

    fn supply_chain(&mut self, _t: Tick) {
        let c = content();
        let econ = &c.economy;
        let n = self.systems.economy.firms.len();
        let idx = |kind: FirmKind, e: &EconomyState| -> Vec<usize> {
            (0..n)
                .filter(|i| e.firms[*i].kind == kind && e.firms[*i].open)
                .collect()
        };
        let warehouses = idx(FirmKind::Warehouse, &self.systems.economy);
        let farms = idx(FirmKind::Farm, &self.systems.economy);
        let retail: Vec<usize> = (0..n)
            .filter(|i| {
                let f = &self.systems.economy.firms[*i];
                f.open && matches!(f.kind, FirmKind::Shop | FirmKind::FoodService)
            })
            .collect();
        let target_of = |r: &Region, i: usize| -> u32 {
            let f = &r.systems.economy.firms[i];
            let def = &c.buildings[r.buildings.def[f.building as usize] as usize];
            match f.kind {
                // Restock to what sold yesterday plus a margin, within limits.
                FirmKind::Shop => (f.sold_yesterday * 13 / 10).clamp(def.stock / 2, def.stock * 4),
                _ => food_service_target(def.capacity),
            }
        };
        let import_price = econ.farm_price_cents * econ.import_price_permille as i64 / 1000;
        let mut import_left = econ.import_cap_per_day * warehouses.len().max(1) as u32;

        // 1. Warehouses buy from local farms, up to the retail demand they serve.
        let retail_need: u32 = retail
            .iter()
            .map(|i| target_of(self, *i).saturating_sub(self.systems.economy.firms[*i].stock))
            .sum();
        if !warehouses.is_empty() {
            let per_wh = retail_need.div_ceil(warehouses.len() as u32) + 500;
            for &w in &warehouses {
                let mut bought_qty = 0u32;
                let mut bought_cost = 0i64;
                for &p in &farms {
                    let have = self.systems.economy.firms[w].stock;
                    if have >= per_wh {
                        break;
                    }
                    let farm_stock = self.systems.economy.firms[p].stock;
                    let q = farm_stock.min(per_wh - have);
                    if q == 0 {
                        continue;
                    }
                    let price = self.systems.economy.firms[p].price;
                    let cost = q as i64 * price;
                    self.transfer(Account::Firm(w as u32), Account::Firm(p as u32), cost);
                    let e = &mut self.systems.economy;
                    e.firms[p].stock -= q;
                    e.firms[p].revenue_today += cost;
                    e.firms[w].stock += q;
                    e.firms[w].costs_today += cost;
                    bought_qty += q;
                    bought_cost += cost;
                }
                // Shortfall: import from outside the region at a premium.
                let have = self.systems.economy.firms[w].stock;
                if have < per_wh && import_left > 0 {
                    let q = (per_wh - have).min(import_left);
                    import_left -= q;
                    let cost = q as i64 * import_price;
                    self.transfer(Account::Firm(w as u32), Account::World, cost);
                    let e = &mut self.systems.economy;
                    e.firms[w].stock += q;
                    e.firms[w].costs_today += cost;
                    e.today.imports += cost;
                    e.today.food_imported += q;
                    bought_qty += q;
                    bought_cost += cost;
                }
                if bought_qty > 0 {
                    let unit = bought_cost / bought_qty as i64;
                    let wh = &mut self.systems.economy.firms[w];
                    wh.unit_cost = unit;
                    wh.price = unit * (1000 + econ.wholesale_markup_permille as i64) / 1000;
                }
            }
        }

        // 2. Shops and food services restock from warehouses (or directly from farms, or
        //    imports, where there is no warehouse).
        for (k, &r) in retail.iter().enumerate() {
            let mut need = target_of(self, r).saturating_sub(self.systems.economy.firms[r].stock);
            if need == 0 {
                continue;
            }
            let sources: Vec<usize> = if warehouses.is_empty() {
                farms.clone()
            } else {
                (0..warehouses.len())
                    .map(|j| warehouses[(k + j) % warehouses.len()])
                    .collect()
            };
            for s in sources {
                if need == 0 {
                    break;
                }
                let q = need.min(self.systems.economy.firms[s].stock);
                if q == 0 {
                    continue;
                }
                let price = self.systems.economy.firms[s].price;
                let cost = q as i64 * price;
                self.transfer(Account::Firm(r as u32), Account::Firm(s as u32), cost);
                let e = &mut self.systems.economy;
                e.firms[s].stock -= q;
                e.firms[s].revenue_today += cost;
                e.firms[r].stock += q;
                e.firms[r].costs_today += cost;
                e.firms[r].unit_cost = price;
                need -= q;
            }
            if need > 0 && warehouses.is_empty() && import_left > 0 {
                let q = need.min(import_left);
                import_left -= q;
                let cost = q as i64 * import_price;
                self.transfer(Account::Firm(r as u32), Account::World, cost);
                let e = &mut self.systems.economy;
                e.firms[r].stock += q;
                e.firms[r].costs_today += cost;
                e.firms[r].unit_cost = import_price;
                e.today.imports += cost;
                e.today.food_imported += q;
            }
        }

        // 3. Farm surplus (more than two days of storage) is sold to the neighbouring town,
        //    or to the rest of the world.
        let export_price = econ.farm_price_cents * econ.export_price_permille as i64 / 1000;
        for &p in &farms {
            let b = self.systems.economy.firms[p].building;
            let spec = c.buildings[self.buildings.def[b as usize] as usize]
                .firm
                .as_ref()
                .map(|s| s.per_shift)
                .unwrap_or(0);
            let keep = spec * 12;
            let stock = self.systems.economy.firms[p].stock;
            if stock <= keep {
                continue;
            }
            let q = stock - keep;
            self.systems.economy.firms[p].stock = keep;
            self.systems.economy.today.food_exported += q;
            if let Some(g) = self.gateways.first() {
                let dest = g.to_region;
                self.send(
                    dest,
                    Body::Sale {
                        good: self.systems.economy.firms[p].good,
                        qty: q,
                        unit_price: export_price,
                        seller_firm: p as u32,
                    },
                );
            } else {
                let value = q as i64 * export_price;
                self.transfer(Account::World, Account::Firm(p as u32), value);
                self.systems.economy.firms[p].revenue_today += value;
                self.systems.economy.today.exports += value;
            }
        }
    }

    fn adjust_prices(&mut self) {
        let c = content();
        let econ = &c.economy;
        let step = econ.price_step_permille as i64;
        for i in 0..self.systems.economy.firms.len() {
            let b = self.systems.economy.firms[i].building;
            let def = &c.buildings[self.buildings.def[b as usize] as usize];
            let f = &mut self.systems.economy.firms[i];
            if !f.open {
                continue;
            }
            match f.kind {
                FirmKind::Shop => {
                    let base = econ.meal_price_cents;
                    let floor = f.unit_cost * 1150 / 1000;
                    let fill = f.stock as i64 * 1000
                        / (f.sold_yesterday * 13 / 10)
                            .clamp(def.stock / 2, def.stock * 4)
                            .max(1) as i64;
                    // Customers bought most of the stock → raise; lots left over → lower.
                    let sold = f.customers_yesterday as i64;
                    if fill < 300 || (sold > 0 && f.revenue_yesterday < f.costs_yesterday) {
                        f.price += f.price * step / 1000;
                    } else if fill > 900 {
                        f.price -= f.price * step / 1000;
                    }
                    f.price = f.price.max(floor).clamp(
                        base * econ.min_price_permille as i64 / 1000,
                        base * econ.max_price_permille as i64 / 1000,
                    );
                }
                FirmKind::FoodService | FirmKind::Leisure => {
                    let profit = f.revenue_yesterday - f.costs_yesterday;
                    let busy =
                        def.capacity > 0 && f.customers_yesterday as i64 > def.capacity as i64 * 3;
                    if profit < 0 || busy {
                        f.price += f.price * step / 1000;
                    } else if f.customers_yesterday < def.capacity as u32 / 2 {
                        f.price -= f.price * step / 1000;
                    }
                    f.price = f.price.clamp(
                        econ.min_price_permille as i64,
                        econ.max_price_permille as i64,
                    );
                }
                _ => {}
            }
        }
    }

    fn public_spending(&mut self) {
        let c = content();
        let econ = &c.economy;
        let cat = catalog();
        // Pensions and benefits.
        for a in 0..self.agents.len() as u32 {
            let i = a as usize;
            if !self.agents.alive(a)
                || self.agents.is_visitor(a)
                || self.agents.act[i] == ACT_FREE_SLOT
            {
                continue;
            }
            let s = self.agents.schedule[i];
            let amount = if s == cat.schedule_retired {
                econ.pension_per_day_cents
            } else if s == cat.schedule_unemployed {
                econ.benefit_per_day_cents
            } else {
                0
            };
            if amount > 0 {
                self.transfer(Account::Treasury, Account::Agent(a), amount);
                self.systems.economy.today.pensions_benefits += amount;
            }
        }
        // Maintenance of public buildings, paid to construction firms (or outside).
        let public = self
            .systems
            .economy
            .firms
            .iter()
            .filter(|f| f.kind == FirmKind::Public)
            .count() as i64;
        let total = public * econ.maintenance_per_building_cents;
        let builders: Vec<u32> = self
            .systems
            .economy
            .firms
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == FirmKind::Construction && f.open)
            .map(|(i, _)| i as u32)
            .collect();
        if total > 0 {
            if builders.is_empty() {
                self.transfer(Account::Treasury, Account::World, total);
            } else {
                let share = total / builders.len() as i64;
                for b in &builders {
                    self.transfer(Account::Treasury, Account::Firm(*b), share);
                    self.systems.economy.firms[*b as usize].revenue_today += share;
                }
            }
            self.systems.economy.today.maintenance += total;
        }
    }

    /// First day of the month: rent, property tax, dividends, tax policy.
    fn monthly(&mut self, t: Tick) {
        let c = content();
        let econ = &c.economy;
        // Who pays for each household: its adult with the most money.
        let mut payer = vec![NONE; self.households.len()];
        for a in 0..self.agents.len() as u32 {
            let i = a as usize;
            let h = self.agents.household[i];
            if h == NONE
                || !self.agents.alive(a)
                || self.agents.is_visitor(a)
                || age_years(self.agents.birth[i], t) < 18
            {
                continue;
            }
            let p = &mut payer[h as usize];
            if *p == NONE || self.agents.money[i] > self.agents.money[*p as usize] {
                *p = a;
            }
        }
        let mut received: std::collections::BTreeMap<u32, i64> = std::collections::BTreeMap::new();
        for h in 0..self.households.len().min(self.systems.economy.tenure.len()) {
            let p = payer[h];
            if p == NONE {
                continue;
            }
            let tn = self.systems.economy.tenure[h].clone();
            if tn.kind == TenureKind::Owner {
                let tax = econ
                    .property_tax_cents
                    .min(self.agents.money[p as usize].max(0));
                self.transfer(Account::Agent(p), Account::Treasury, tax);
                continue;
            }
            if self.agents.money[p as usize] < tn.rent {
                self.systems.economy.tenure[h].arrears = tn.arrears.saturating_add(1);
                self.journal(p, t, JournalKind::MissedRent, 0, 0);
                continue;
            }
            let to = match tn.kind {
                TenureKind::Agency => Account::Firm(tn.landlord),
                TenureKind::PrivateLandlord
                    if tn.landlord != NONE
                        && self.agents.alive(tn.landlord)
                        && !self.agents.is_visitor(tn.landlord) =>
                {
                    *received.entry(tn.landlord).or_insert(0) += tn.rent;
                    Account::Agent(tn.landlord)
                }
                _ => Account::Treasury,
            };
            self.transfer(Account::Agent(p), to, tn.rent);
            if let Account::Firm(f) = to {
                self.systems.economy.firms[f as usize].revenue_today += tn.rent;
            }
            self.systems.economy.tenure[h].arrears = 0;
            self.systems.economy.today.rent_paid += tn.rent;
            self.journal(
                p,
                t,
                JournalKind::PaidRent,
                0,
                tn.rent.clamp(0, u32::MAX as i64) as u32,
            );
        }
        for (l, amount) in received {
            self.journal(
                l,
                t,
                JournalKind::ReceivedRent,
                0,
                amount.clamp(0, u32::MAX as i64) as u32,
            );
        }
        // Dividends: firms with plenty of cash pay half the excess to their owner.
        let payroll = self.payroll_by_firm();
        for f in 0..self.systems.economy.firms.len() {
            let fm = &self.systems.economy.firms[f];
            if !fm.is_private() || !fm.open || fm.owner == NONE || !self.agents.alive(fm.owner) {
                continue;
            }
            let reserve = payroll[f] * 60 + 100_000;
            if fm.cash > reserve {
                let div = (fm.cash - reserve) / 2;
                let owner = fm.owner;
                self.transfer(Account::Firm(f as u32), Account::Agent(owner), div);
            }
        }
        // Tax policy: keep 1–4 months of spending in the treasury.
        let e = &self.systems.economy;
        let month: i64 = e.spending_30d.iter().sum::<i64>().max(1);
        let old = e.income_tax_permille;
        let mut tax = old;
        if e.treasury < month {
            tax = (tax + 10).min(econ.max_income_tax_permille);
        } else if e.treasury > month * 4 {
            tax = tax.saturating_sub(10).max(econ.min_income_tax_permille);
        }
        self.systems.economy.income_tax_permille = tax;
        let id = self.id as u32;
        let treasury_dollars =
            (self.systems.economy.treasury / 100).clamp(0, u32::MAX as i64) as u32;
        self.push_news(NewsItem {
            tick: t,
            kind: NewsKind::Budget as u16,
            subject: id,
            n: treasury_dollars,
        });
        if tax != old {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::TaxChange as u16,
                subject: id,
                n: tax as u32,
            });
        }
    }

    /// Daily payroll per firm at full attendance (one shift per employee).
    fn payroll_by_firm(&self) -> Vec<i64> {
        let mut p = vec![0i64; self.systems.economy.firms.len()];
        for a in 0..self.agents.len() as u32 {
            let w = self.agents.work[a as usize];
            if w == NONE || self.agents.profession[a as usize] == u16::MAX || !self.agents.alive(a)
            {
                continue;
            }
            if let Some(f) = self.firm_at(w) {
                p[f as usize] += self.wage_of(a);
            }
        }
        p
    }

    fn lay_off(&mut self, a: AgentId, t: Tick) {
        let i = a as usize;
        let place = self.agents.work[i];
        self.agents.work[i] = NONE;
        self.agents.profession[i] = u16::MAX;
        self.agents.schedule[i] = catalog().schedule_unemployed;
        self.systems.economy.today.layoffs += 1;
        self.journal(a, t, JournalKind::LaidOff, 0, place);
    }

    fn labour_market(&mut self, t: Tick) {
        let c = content();
        let econ = &c.economy;
        let cat = catalog();
        let nf = self.systems.economy.firms.len();
        // Employees per firm (ascending agent id).
        let mut staff: Vec<Vec<AgentId>> = vec![Vec::new(); nf];
        for a in 0..self.agents.len() as u32 {
            let w = self.agents.work[a as usize];
            if w == NONE || self.agents.profession[a as usize] == u16::MAX || !self.agents.alive(a)
            {
                continue;
            }
            if let Some(f) = self.firm_at(w) {
                staff[f as usize].push(a);
            }
        }
        let payroll = self.payroll_by_firm();

        // Layoffs and bankruptcies.
        for f in 0..nf {
            let fm = &self.systems.economy.firms[f];
            if !fm.is_private() || !fm.open {
                continue;
            }
            let in_red = fm.cash < 0;
            let days = if in_red { fm.days_in_red + 1 } else { 0 };
            self.systems.economy.firms[f].days_in_red = days;
            if days >= econ.bankruptcy_after_days {
                let n = staff[f].len() as u32;
                for a in std::mem::take(&mut staff[f]) {
                    self.lay_off(a, t);
                }
                self.end_remote_staff(f as u32);
                // Creditors outside the region absorb the debt.
                let debt = -self.systems.economy.firms[f].cash;
                self.transfer(Account::World, Account::Firm(f as u32), debt);
                let fm = &mut self.systems.economy.firms[f];
                fm.open = false;
                fm.closed_since = t;
                fm.days_in_red = 0;
                fm.owner = NONE;
                let b = fm.building;
                self.systems.economy.today.bankruptcies += 1;
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::Bankruptcy as u16,
                    subject: b,
                    n,
                });
            } else if days >= econ.layoff_after_days {
                if let Some(a) = staff[f].pop() {
                    self.lay_off(a, t);
                }
            } else {
                // Too many staff for the customers it has, and losing money: let one go.
                let fm = &self.systems.economy.firms[f];
                let losing = fm.profit_avg < 0;
                if losing && staff[f].len() as u32 > self.desired_staff(f as u32) + 1 {
                    if let Some(a) = staff[f].pop() {
                        self.lay_off(a, t);
                    }
                }
            }
        }

        // Reopenings: a local investor buys a closed business.
        for f in 0..nf {
            let fm = &self.systems.economy.firms[f];
            if fm.open || t < fm.closed_since + econ.reopen_after_days as Tick * TICKS_PER_DAY {
                continue;
            }
            let b = fm.building;
            let def = &c.buildings[self.buildings.def[b as usize] as usize];
            let full_payroll: i64 = def
                .jobs
                .iter()
                .map(|j| j.count as i64 * c.professions[j.profession as usize].wage_per_shift_cents)
                .sum();
            let capital = full_payroll * econ.firm_start_cash_days as i64 / 2;
            let mut investor = NONE;
            for a in 0..self.agents.len() as u32 {
                let i = a as usize;
                if !self.agents.alive(a)
                    || self.agents.is_visitor(a)
                    || self.agents.act[i] == ACT_FREE_SLOT
                {
                    continue;
                }
                if !(25..=75).contains(&age_years(self.agents.birth[i], t))
                    || self.agents.money[i] < capital * 2
                {
                    continue;
                }
                if investor == NONE || self.agents.money[i] > self.agents.money[investor as usize] {
                    investor = a;
                }
            }
            if investor == NONE {
                continue;
            }
            self.transfer(Account::Agent(investor), Account::Firm(f as u32), capital);
            let gen = self.systems.economy.firms[f].generation + 1;
            let fm = &mut self.systems.economy.firms[f];
            fm.open = true;
            fm.owner = investor;
            fm.generation = gen;
            fm.price = match fm.kind {
                FirmKind::Shop => econ.meal_price_cents,
                FirmKind::Farm => econ.farm_price_cents,
                FirmKind::Warehouse | FirmKind::Factory => fm.price,
                _ => BASE_FACTOR,
            };
            self.rename_business(b, gen);
            self.systems.economy.today.reopenings += 1;
            self.journal(investor, t, JournalKind::OpenedBusiness, 0, b);
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::Reopened as u16,
                subject: b,
                n: 0,
            });
        }

        // Hiring: open firms that can afford it fill vacancies with job seekers.
        let mut pool: Vec<AgentId> = (0..self.agents.len() as u32)
            .filter(|a| {
                let i = *a as usize;
                self.agents.alive(*a)
                    && !self.agents.is_visitor(*a)
                    && self.agents.schedule[i] == cat.schedule_unemployed
                    && self.agents.act[i] != ACT_FREE_SLOT
                    && (18..=65).contains(&age_years(self.agents.birth[i], t))
            })
            .collect();
        let day = t / TICKS_PER_DAY;
        for f in 0..nf {
            if pool.is_empty() {
                break;
            }
            let fm = &self.systems.economy.firms[f];
            if !fm.open {
                continue;
            }
            let can_hire = !fm.is_private() || fm.cash > payroll[f] * 7;
            if !can_hire {
                continue;
            }
            let b = fm.building;
            let def = &c.buildings[self.buildings.def[b as usize] as usize];
            // Profitable firms hire up to their positions; others only up to what their
            // demand justifies (so they do not rehire someone they just let go).
            let profitable = fm.profit_avg >= 0;
            let limit = if profitable || !fm.is_private() {
                fm.positions as u32
            } else {
                self.desired_staff(f as u32).min(fm.positions as u32)
            };
            let _ = def;
            let have_total = staff[f].len() as u32 + self.remote_staff_of(f as u32) as u32;
            if have_total >= limit {
                continue;
            }
            let mut room = limit - have_total;
            let mut hires = 0;
            for js in &def.jobs {
                let have = staff[f]
                    .iter()
                    .filter(|a| self.agents.profession[**a as usize] == js.profession)
                    .count() as u16;
                let mut open_slots = js.count.saturating_sub(have);
                let prof = &c.professions[js.profession as usize];
                while open_slots > 0 && room > 0 && hires < econ.hires_per_day && !pool.is_empty() {
                    // Nearest of a deterministic sample of candidates.
                    let start = (keyed(self.seed, day as u64, f as u64, js.profession as u64)
                        % pool.len() as u64) as usize;
                    let mut best: Option<(u32, usize)> = None;
                    for k in 0..pool.len().min(24) {
                        let j = (start + k) % pool.len();
                        let a = pool[j];
                        if age_years(self.agents.birth[a as usize], t) < prof.min_age as u32
                            || self.systems.demography.education_of(a) < prof.education
                        {
                            continue;
                        }
                        let d = self.manhattan(self.agents.home[a as usize], b);
                        if best.is_none_or(|(bd, bj)| (d, j) < (bd, bj)) {
                            best = Some((d, j));
                        }
                    }
                    let Some((_, j)) = best else { break };
                    let a = pool.remove(j);
                    let variant = (have as usize + hires as usize) % prof.schedules.len();
                    let i = a as usize;
                    self.agents.work[i] = b;
                    self.agents.profession[i] = js.profession;
                    self.agents.schedule[i] = prof.schedules[variant];
                    staff[f].push(a);
                    open_slots -= 1;
                    room -= 1;
                    hires += 1;
                    self.systems.economy.today.hires += 1;
                    self.journal(a, t, JournalKind::Hired, 0, b);
                }
            }
        }
    }

    /// Staff a firm wants: all its job slots, except businesses that serve customers,
    /// which staff according to yesterday's demand.
    pub fn desired_staff(&self, f: u32) -> u32 {
        let fm = &self.systems.economy.firms[f as usize];
        let def = &content().buildings[self.buildings.def[fm.building as usize] as usize];
        let _ = def;
        let slots = fm.positions as u32;
        let per_worker = match fm.kind {
            FirmKind::Shop => 40,
            FirmKind::FoodService => 20,
            FirmKind::Leisure => 30,
            _ => return slots,
        };
        if fm.customers_yesterday == 0 && fm.revenue_yesterday == 0 {
            return slots; // no data yet (first day, or just reopened)
        }
        fm.customers_yesterday
            .div_ceil(per_worker)
            .clamp(1, slots.max(1))
    }

    fn rename_business(&mut self, b: BuildingId, generation: u16) {
        let c = content();
        let d = &c.buildings[self.buildings.def[b as usize] as usize];
        if d.name_patterns.is_empty() {
            return;
        }
        let st = &c.names.styles[self.style as usize];
        let h = keyed(self.seed, b as u64, generation as u64, 0x4E41);
        let pattern = &d.name_patterns[(h % d.name_patterns.len() as u64) as usize];
        let mut s = pattern.clone();
        s = s.replace(
            "{family}",
            &generate(&c.names, &st.family, keyed(h, 1, 0, 0)),
        );
        s = s.replace("{given}", &generate(&c.names, &st.male, keyed(h, 2, 0, 0)));
        s = s.replace(
            "{place}",
            &generate(&c.names, &st.places, keyed(h, 3, 0, 0)),
        );
        self.buildings.name[b as usize] = self.names.intern(&s);
    }

    fn sample_history(&mut self, t: Tick) {
        let cat = catalog();
        let e = &self.systems.economy;
        let shops: Vec<&Firm> = e
            .firms
            .iter()
            .filter(|f| f.kind == FirmKind::Shop && f.open)
            .collect();
        let food_price = if shops.is_empty() {
            0
        } else {
            shops.iter().map(|f| f.price).sum::<i64>() / shops.len() as i64
        };
        let (mut workers, mut seekers, mut people, mut money) = (0u32, 0u32, 0i64, 0i64);
        for a in 0..self.agents.len() as u32 {
            let i = a as usize;
            if !self.agents.alive(a)
                || self.agents.is_visitor(a)
                || self.agents.act[i] == ACT_FREE_SLOT
            {
                continue;
            }
            people += 1;
            money += self.agents.money[i];
            if self.agents.profession[i] != u16::MAX {
                workers += 1;
            } else if self.agents.schedule[i] == cat.schedule_unemployed {
                seekers += 1;
            }
        }
        let rents: Vec<i64> = e
            .tenure
            .iter()
            .filter(|x| x.kind != TenureKind::Owner)
            .map(|x| x.rent)
            .collect();
        let sample = DaySample {
            tick: t,
            food_price,
            unemployment_permille: seekers * 1000 / (workers + seekers).max(1),
            treasury: e.treasury,
            avg_savings: money / people.max(1),
            output: e.firms.iter().map(|f| f.revenue_yesterday).sum::<i64>()
                + e.yesterday.wages_public,
            avg_rent: if rents.is_empty() {
                0
            } else {
                rents.iter().sum::<i64>() / rents.len() as i64
            },
            firms_open: e.firms.iter().filter(|f| f.is_private() && f.open).count() as u32,
            firms_closed: e.firms.iter().filter(|f| !f.open).count() as u32,
            income_tax_permille: e.income_tax_permille,
        };
        let h = &mut self.systems.economy.history;
        h.push(sample);
        if h.len() > HISTORY_DAYS {
            h.remove(0);
        }
    }

    fn economy_news(&mut self, t: Tick) {
        let id = self.id as u32;
        let h = &self.systems.economy.history;
        let Some(now) = h.last().cloned() else { return };
        // Weekly (Mondays): jobs report, price trend, imports.
        if date(t).weekday != 0 || h.len() < 2 {
            return;
        }
        let before = h[h.len().saturating_sub(8)].food_price;
        self.push_news(NewsItem {
            tick: t,
            kind: NewsKind::Unemployment as u16,
            subject: id,
            n: now.unemployment_permille,
        });
        if before > 0 {
            let change = (now.food_price - before) * 1000 / before;
            if change >= 50 {
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::PriceUp as u16,
                    subject: id,
                    n: now.food_price as u32,
                });
            } else if change <= -50 {
                self.push_news(NewsItem {
                    tick: t,
                    kind: NewsKind::PriceDown as u16,
                    subject: id,
                    n: now.food_price as u32,
                });
            }
        }
        let imported = self.systems.economy.yesterday.food_imported;
        if imported > 0 {
            self.push_news(NewsItem {
                tick: t,
                kind: NewsKind::FoodImports as u16,
                subject: id,
                n: imported,
            });
        }
    }
}

fn pick_tenure(roll: u32, w: [u32; 4]) -> TenureKind {
    let kinds = [
        TenureKind::Owner,
        TenureKind::Municipal,
        TenureKind::Agency,
        TenureKind::PrivateLandlord,
    ];
    let mut acc = 0;
    for (k, wk) in kinds.iter().zip(w) {
        acc += wk;
        if roll < acc {
            return *k;
        }
    }
    TenureKind::PrivateLandlord
}

/// Food a restaurant or café keeps in stock (about three days of customers).
fn food_service_target(capacity: u16) -> u32 {
    (capacity as u32 * 4).max(60)
}
