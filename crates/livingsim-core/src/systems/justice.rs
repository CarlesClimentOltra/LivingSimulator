//! Phase 5: patrols, investigations, arrests, trials, sentences and jail.

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Case {
    pub incident: u32,
    pub evidence: u16,
    pub suspect: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inmate {
    pub agent: u32,
    pub release: crate::time::Tick,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JusticeState {
    pub open_cases: Vec<Case>,
    pub inmates: Vec<Inmate>,
}

impl JusticeState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 5): investigations progress, trials, releases.
    }
}
