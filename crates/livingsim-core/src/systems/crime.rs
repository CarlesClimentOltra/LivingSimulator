//! Phase 5: offences arising from traits, economic desperation and conflicts. Always
//! represented non-graphically (icons, police tape, newspaper).

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Incident {
    pub crime: u16,
    pub tick: crate::time::Tick,
    pub building: u32,
    pub offender: u32,
    pub victim: u32,
    pub solved: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CrimeState {
    pub incidents: Vec<Incident>,
}

impl CrimeState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 5): motive scores, offences from data/crimes.ron.
    }
}
