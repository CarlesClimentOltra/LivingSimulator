//! Simulates N days and prints the economy of each region every day.
//! `cargo run --release -p livingsim-core --example economy -- [days]`
use livingsim_content::FirmKind;
use livingsim_core::time::TICKS_PER_DAY;
use livingsim_core::World;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let mut w = World::new(1);
    let totals: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    for _ in 0..days {
        for _ in 0..24 {
            w.step_window();
        }
        for (ri, r) in w.regions.iter().enumerate() {
            let e = &r.systems.economy;
            let s = e.history.last().cloned().unwrap_or_default();
            let y = &e.yesterday;
            let red = e
                .firms
                .iter()
                .filter(|f| f.is_private() && f.cash < 0)
                .count();
            println!(
                "{} day {:>2} | food ${:.2} unemp {:>3}‰ tax {}‰ treasury ${:>9} savings ${:>7} rent ${:>5} | open {} closed {} red {} | hires {} layoffs {} bankr {} reopen {} | imp ${} exp ${} food_imp {} | Δmoney {}",
                r.name(), w.tick / TICKS_PER_DAY, s.food_price as f64 / 100.0, s.unemployment_permille, e.income_tax_permille,
                e.treasury / 100, s.avg_savings / 100, s.avg_rent / 100, s.firms_open, s.firms_closed, red,
                y.hires, y.layoffs, y.bankruptcies, y.reopenings, y.imports / 100, y.exports / 100, y.food_imported,
                r.money_total() - totals[ri]
            );
        }
    }
    let r = &w.regions[0];
    let mut kinds: std::collections::BTreeMap<String, (i64, i64, u32)> = Default::default();
    for f in &r.systems.economy.firms {
        let k = kinds.entry(format!("{:?}", f.kind)).or_default();
        k.0 += f.cash;
        k.1 += 1;
        if !f.open {
            k.2 += 1
        }
    }
    for (k, (cash, n, closed)) in kinds {
        println!(
            "  {k:<12} n={n:<4} closed={closed:<3} avg cash ${}",
            cash / n / 100
        );
    }
    let _ = FirmKind::Shop;
    for n in r.newspaper().iter().take(8) {
        println!("NEWS {} — {}: {}", n.when, n.headline, n.body);
    }
}
