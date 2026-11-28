//! Runs a fresh world and reports town-founding pressure and new towns:
//! `cargo run --release --example expansion -- [days]`.
use livingsim_core::World;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let mut w = World::new(1);
    let money0: i128 = w.regions.iter().map(|r| r.money_total()).sum();
    for d in 1..=days {
        w.step_to(d * 1440);
        if d % 10 == 0 {
            for r in &w.regions {
                let x = &r.systems.expansion;
                let dm = &r.systems.demography.yesterday;
                println!(
                    "day {d:4} r{} {:12} pop {:5} pressure {:5} free_lots {:4} open_blocks {:3} left_yday {} | gateways {} origin {:?} size {}",
                    r.id, r.name(), r.population(), x.pressure, x.free_lots, x.open_blocks,
                    dm.emigrants + dm.moved_out, r.gateways.len(), r.origin, r.map.width
                );
            }
        }
    }
    let money: i128 = w.regions.iter().map(|r| r.money_total()).sum();
    println!(
        "regions {} highways {:?}",
        w.regions.len(),
        w.intercity
            .highways
            .iter()
            .map(|h| (h.a, h.b))
            .collect::<Vec<_>>()
    );
    println!(
        "money (all regions, new towns add their own) {} -> {}",
        money0, money
    );
}
