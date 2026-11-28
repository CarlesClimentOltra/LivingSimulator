//! Runs a fresh world and reports research: `cargo run --release --example technology -- [days]`.
use livingsim_core::World;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let mut w = World::new(1);
    for d in 1..=days {
        w.step_to(d * 1440);
        if d % 20 == 0 {
            for r in &w.regions {
                let s = &r.systems.technology;
                let per_day = s.history.iter().rev().take(7).sum::<u64>() / 7;
                println!(
                    "day {d:4} {:10} {} pts/day, working on {:?} ({} pts) | known {:?} | adopting {:?}",
                    r.name(), per_day, s.current, s.points, s.researched, s.adopting.keys().collect::<Vec<_>>()
                );
            }
        }
    }
}
