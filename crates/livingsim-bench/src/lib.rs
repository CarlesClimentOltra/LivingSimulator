//! Synthetic worlds for benchmarks: N copies of the city template (10k inhabitants each),
//! with different seeds and no connections, warmed up so they are not all asleep.

use livingsim_core::time::TICKS_PER_DAY;
use livingsim_core::{World, WINDOW};

/// A world with about `agents` inhabitants (rounded to whole cities of ~10k).
pub fn synthetic(agents: u32) -> World {
    let cities = agents.div_ceil(10_000).max(1) as u16;
    let mut w = World::synthetic(7, "city", cities);
    // Warm up half a day so the benchmark starts in a busy morning.
    while w.tick < TICKS_PER_DAY / 4 {
        w.step_window();
    }
    w
}

/// Simulates one full day (24 exchange windows, regions in parallel).
pub fn one_day(w: &mut World) {
    for _ in 0..TICKS_PER_DAY / WINDOW {
        w.step_window();
    }
}
