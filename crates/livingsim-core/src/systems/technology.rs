//! Phase 7: research points from the university and tech firms; tech tree from
//! data/technologies.ron unlocks buildings and improvements.

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnologyState {
    pub points: u64,
    pub researched: Vec<u16>,
    pub current: Option<u16>,
}

impl TechnologyState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 7): accumulate research, unlock technologies, diffuse to neighbours.
    }
}
