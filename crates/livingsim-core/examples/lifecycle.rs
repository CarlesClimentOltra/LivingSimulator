//! Simulates N days and prints the life cycle of each region.
//! `cargo run --release -p livingsim-core --example lifecycle -- [days]`
use livingsim_core::time::TICKS_PER_DAY;
use livingsim_core::World;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let mut w = World::new(1);
    let totals: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    let mut sum = vec![livingsim_core::systems::demography::DemoTotals::default(); w.regions.len()];
    for d in 1..=days {
        for _ in 0..24 {
            w.step_window();
        }
        for (ri, r) in w.regions.iter().enumerate() {
            let y = &r.systems.demography.yesterday;
            let s = &mut sum[ri];
            s.births += y.births;
            s.deaths += y.deaths;
            s.couples += y.couples;
            s.marriages += y.marriages;
            s.divorces += y.divorces;
            s.adoptions += y.adoptions;
            s.immigrants += y.immigrants;
            s.emigrants += y.emigrants;
            s.moved_in += y.moved_in;
            s.moved_out += y.moved_out;
            s.left_home += y.left_home;
            s.illnesses += y.illnesses;
            s.retirements += y.retirements;
        }
        if d % 15 == 0 {
            for (ri, r) in w.regions.iter().enumerate() {
                let s = &sum[ri];
                let dm = &r.systems.demography;
                let e = r
                    .systems
                    .economy
                    .history
                    .last()
                    .cloned()
                    .unwrap_or_default();
                println!("{:<10} day {:>3} pop {:>5} | births {:>3} deaths {:>3} couples {:>3} weddings {:>3} divorces {:>2} adoptions {:>2} | imm {:>3} emig {:>3} in {:>2} out {:>2} leftHome {:>3} | ill now {:>3} pregnant {:>3} | commute out {:>3} in {:>3} | unemp {}‰ | Δmoney {}",
                    r.name(), d, r.population(), s.births, s.deaths, s.couples, s.marriages, s.divorces, s.adoptions, s.immigrants, s.emigrants, s.moved_in, s.moved_out, s.left_home,
                    dm.illness.len(), dm.pregnancies.len(), dm.remote_jobs.len(), dm.remote_staff.len(), e.unemployment_permille, r.money_total() - totals[ri]);
            }
        }
    }
    let r = &w.regions[0];
    for n in r.newspaper().iter().take(10) {
        println!("NEWS {} — {}: {}", n.when, n.headline, n.body);
    }
    let _ = TICKS_PER_DAY;
}
