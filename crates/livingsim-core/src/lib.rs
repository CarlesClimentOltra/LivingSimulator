//! # livingsim-core
//!
//! The deterministic simulation of Living Simulator.
//!
//! * No I/O, no system clock, no threads (except optional rayon *across* regions), no
//!   floats in simulation logic, no hash-ordered iteration. See `CONTRIBUTING.md`.
//! * 1 tick = 1 simulated minute. [`World::step`] advances one tick; cost is proportional
//!   to the number of scheduled events, not to the number of agents.
//! * The world is split into [`Region`]s that only talk through [`messages`], so one region
//!   can be simulated alone ([`replay::RegionReplay`]).

#![forbid(unsafe_code)]

pub mod agents;
pub mod buildings;
pub mod catalog;
pub mod content_ids;
pub mod gen;
pub mod journal;
mod life;
pub mod map;
pub mod messages;
pub mod names;
pub mod query;
pub mod queue;
pub mod region;
pub mod render;
pub mod replay;
pub mod rng;
pub mod roads;
pub mod snapshot;
pub mod snapshot_v4;
pub mod snapshot_v5;
pub mod snapshot_v6;
pub mod systems;
pub mod time;
pub mod world;

pub use catalog::{catalog, content, sim_version};
pub use life::{next_block, open_until};
pub use messages::{Message, MessageLog, WINDOW};
pub use region::Region;
pub use replay::RegionReplay;
pub use snapshot::StateHash;
pub use time::Tick;
pub use world::World;
