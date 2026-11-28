//! Loads published region checkpoints and projects them forward, printing a health report.
//! `cargo run --release -p livingsim-core --example health -- <r0.bin.zst> <r1.bin.zst> [days]`
use livingsim_core::snapshot::decode_region;
use livingsim_core::world::IntercityGraph;
use livingsim_core::World;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let days: u32 = args.last().and_then(|s| s.parse().ok()).unwrap_or(60);
    let regions = args[1..]
        .iter()
        .filter(|a| a.ends_with(".zst"))
        .map(|p| decode_region(&std::fs::read(p).unwrap()).unwrap())
        .collect();
    let mut w = World::from_regions(1, IntercityGraph::default(), regions);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    for d in 0..=days {
        if d % 15 == 0 {
            for (ri, r) in w.regions.iter().enumerate() {
                let s = r.stats();
                let e = &s.economy;
                let p = &s.demography;
                println!("{:<10} day+{:>3} pop {:>5} | unemp {:>3}‰ meal ${:.2} rent ${} treasury ${}k tax {}‰ open {} closed {} | births30 {} deaths30 {} couples {} ill {} preg {} | commute out {} in {} | savings ${} | Δmoney {}",
                    r.name(), d, s.population, e.unemployment_permille, e.food_price_cents as f64 / 100.0, e.avg_rent_cents / 100, e.treasury_cents / 100_000, e.income_tax_permille,
                    e.firms_open, e.firms_closed, p.births_30d, p.deaths_30d, p.couples, p.ill, p.pregnant, p.commuters_out, p.commuters_in, s.avg_money_cents / 100, r.money_total() - start[ri]);
                let k = &s.construction;
                println!(
                    "{:<10}          map {}x{} | building {} finished {} streets {} | {}",
                    r.name(),
                    r.map.width,
                    r.map.height,
                    k.open.len(),
                    k.finished,
                    k.streets,
                    k.open
                        .iter()
                        .map(|p| format!("{} {}‰ ({})", p.what, p.progress_permille, p.phase))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        if d < days {
            for _ in 0..24 {
                w.step_window();
            }
        }
    }
}
