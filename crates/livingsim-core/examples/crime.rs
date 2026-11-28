//! Runs a fresh world and reports crime and justice: `cargo run --release --example crime -- [days]`.
use livingsim_core::catalog::content;
use livingsim_core::World;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let mut w = World::new(1);
    let money0: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    let names: Vec<String> = content().crimes.iter().map(|c| c.id.clone()).collect();
    for d in 1..=days {
        w.step_to(d * 1440);
        if d % 10 == 0 {
            for r in &w.regions {
                let c = &r.systems.crime;
                let mut tot = c.total.clone();
                for (k, n) in c.today.by_crime.iter().enumerate() {
                    if tot.by_crime.len() <= k {
                        tot.by_crime.resize(k + 1, 0);
                    }
                    tot.by_crime[k] += n;
                }
                let by: Vec<String> = tot
                    .by_crime
                    .iter()
                    .enumerate()
                    .map(|(k, n)| format!("{} {}", names[k], n))
                    .collect();
                println!(
                    "day {d:4} {:10} pop {:5} | {} | solved {} arrests {} convicted {} acquitted {} fined {} cold {} | inmates {} guests {} | safety {:?}",
                    r.name(), r.population(), by.join(", "),
                    tot.solved + c.today.solved, tot.arrests + c.today.arrests, tot.convictions + c.today.convictions,
                    tot.acquittals + c.today.acquittals, tot.fines + c.today.fines, tot.cold + c.today.cold,
                    r.systems.justice.inmates.len(), r.systems.justice.guests.len(),
                    c.history.last().map(|h| h.safety)
                );
            }
        }
    }
    for (r, m0) in w.regions.iter().zip(money0) {
        println!("{}: money drift {}", r.name(), r.money_total() - m0);
    }
}
