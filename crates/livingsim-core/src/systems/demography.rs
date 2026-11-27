//! Phase 3: couples, births (named with `names::generate_for_baby`), ageing, deaths,
//! retirement, education.

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DemographyState {
    pub births: u32,
    pub deaths: u32,
    pub marriages: u32,
    /// Pregnancies in progress: (mother, due tick).
    pub pregnancies: Vec<(u32, crate::time::Tick)>,
}

impl DemographyState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 3): partnering, pregnancies, births, natural deaths, retirement.
    }
}
