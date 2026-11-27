//! Phase 2: firms, wages, prices, rents, shop stock, farms, municipal budget.
//!
//! Phase 1 already pays wages per shift, pensions and benefits, sells groceries and meals,
//! restocks shops and ships farm produce between regions (see `life.rs` / `region.rs`).
//! This module will own the full economy.

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EconomyState {
    /// Municipal budget.
    pub treasury_cents: i64,
    /// Income tax rate (‰). Not applied yet.
    pub tax_permille: u16,
    /// Firms per building (Phase 2).
    pub firms: Vec<Firm>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Firm {
    pub building: u32,
    pub cash_cents: i64,
    pub revenue_today_cents: i64,
    pub payroll_today_cents: i64,
}

impl EconomyState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 2): prices from stock/demand, rents, taxes, public spending.
    }
}
