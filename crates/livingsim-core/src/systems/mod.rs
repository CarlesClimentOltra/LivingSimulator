//! Simulation systems that arrive in later phases. Their state types exist from Phase 1 so
//! that snapshots, messages and content already have a place for them; their logic is
//! still a stub (see docs/ROADMAP.md).

pub mod construction;
pub mod crime;
pub mod demography;
pub mod economy;
pub mod expansion;
pub mod justice;
pub mod technology;

use serde::{Deserialize, Serialize};

/// State of every later-phase system, stored per region.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Systems {
    pub economy: economy::EconomyState,
    pub demography: demography::DemographyState,
    pub construction: construction::ConstructionState,
    pub crime: crime::CrimeState,
    pub justice: justice::JusticeState,
    pub expansion: expansion::ExpansionState,
    pub technology: technology::TechnologyState,
}

impl Systems {
    /// Called once per simulated day (06:00), in a fixed order.
    pub fn daily(&mut self, ctx: &mut DailyContext) {
        self.economy.daily(ctx);
        self.demography.daily(ctx);
        self.construction.daily(ctx);
        self.crime.daily(ctx);
        self.justice.daily(ctx);
        self.expansion.daily(ctx);
        self.technology.daily(ctx);
    }
}

/// What systems may read during their daily update (kept small on purpose).
pub struct DailyContext {
    pub tick: crate::time::Tick,
    pub population: u32,
    pub employed: u32,
    pub housing_free: u32,
}
