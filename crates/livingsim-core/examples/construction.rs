//! Runs a fresh world and reports construction activity: `cargo run --release --example construction -- [days]`.
use livingsim_core::catalog::content;
use livingsim_core::systems::construction::{Phase, Work};
use livingsim_core::World;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let mut w = World::new(1);
    let money0: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    for d in 1..=days {
        w.step_to(d * 1440);
        if d % 10 == 0 {
            for r in &w.regions {
                let c = &r.systems.construction;
                let open: Vec<String> = r
                    .open_projects()
                    .iter()
                    .map(|p| {
                        let what = match p.work {
                            Work::Building { def, .. } => {
                                content().buildings[def as usize].id.clone()
                            }
                            Work::Road { .. } => "road".into(),
                        };
                        format!("{what}:{:?}/{:?}:{}‰", p.phase, p.next, p.progress())
                    })
                    .collect();
                println!(
                    "day {d:4} {:12} pop {:5} map {}x{} started {} finished {} streets {} units {} | {}",
                    r.name(),
                    r.population(),
                    r.map.width,
                    r.map.height,
                    c.total.started,
                    c.total.finished,
                    c.total.streets,
                    c.total.units,
                    open.join(" ")
                );
            }
        }
    }
    for (r, m0) in w.regions.iter().zip(money0) {
        println!("{}: money drift {}", r.name(), r.money_total() - m0);
        for p in r
            .systems
            .construction
            .projects
            .iter()
            .filter(|p| p.phase == Phase::Finished)
        {
            let what = match p.work {
                Work::Building { def, .. } => content().buildings[def as usize].id.clone(),
                Work::Road { .. } => "road".into(),
            };
            println!(
                "  {what}: {} units in {} days",
                p.units.iter().sum::<u32>(),
                (p.finished - p.started) / 1440
            );
        }
    }
}
