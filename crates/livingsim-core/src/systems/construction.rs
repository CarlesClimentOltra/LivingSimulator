//! Phase 4: demand-driven construction. Construction firms employ builders who physically
//! build roads, houses and allotments in visible phases.

use super::DailyContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Planned,
    Foundation,
    Structure,
    Roof,
    Finished,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub building_def: u16,
    pub x: u16,
    pub y: u16,
    pub phase: Phase,
    /// Work (builder-shifts) left in the current phase.
    pub work_left: u32,
    pub firm_building: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConstructionState {
    pub projects: Vec<Project>,
}

impl ConstructionState {
    pub fn daily(&mut self, _ctx: &mut DailyContext) {
        // TODO(phase 4): evaluate demand (housing, jobs, shops), plan projects, assign crews.
    }
}
