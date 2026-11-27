//! Phase 6: founding new towns when land runs out, and new highways between regions.

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExpansionState {
    /// Free buildable lots left (updated daily from Phase 4 on).
    pub free_lots: u32,
    /// Pressure towards founding a new town (‰).
    pub pressure_permille: u16,
}

impl ExpansionState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 6): emit a founding request to the world when pressure is high.
    }
}
