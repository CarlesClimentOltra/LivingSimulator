//! Generates the world and simulates a couple of days, printing statistics.
//! `cargo run --release -p livingsim-core --example inspect -- [days]`
#![allow(clippy::disallowed_types)]
use livingsim_core::{time::TICKS_PER_DAY, World};
use std::time::Instant;

fn main() {
    let days: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let t0 = Instant::now();
    let mut w = World::new(42);
    println!("generated in {:?}", t0.elapsed());
    for r in &w.regions {
        let s = r.stats();
        println!(
            "region {} '{}': pop {} hh {} bld {} employed {} unemployed {} students {} children {} retired {} nodes {} size {}",
            r.id, s.name, s.population, s.households, s.buildings, s.employed, s.unemployed, s.students, s.children, s.retired,
            r.roads.node_count(), r.map.width
        );
    }
    for d in 0..days {
        let t0 = Instant::now();
        let e0 = w.events();
        for _ in 0..TICKS_PER_DAY / 60 {
            w.step_window();
        }
        let el = t0.elapsed();
        println!(
            "day {d}: {:?}, events {}, hash {:?}",
            el,
            w.events() - e0,
            w.state_hash()
        );
        for r in &w.regions {
            let s = r.stats();
            println!(
                "  {} trips {} meals_out {} groceries {} empty {} visitors {} away {} needs {:?}",
                s.name,
                s.trips_yesterday,
                s.meals_out_yesterday,
                s.groceries_yesterday,
                s.shelves_empty_yesterday,
                s.visitors,
                s.away,
                s.avg_needs
            );
            println!(
                "  acts now: {:?}",
                &s.by_activity[..s.by_activity.len().min(8)]
            );
        }
    }
    let r = &w.regions[0];
    for n in r.newspaper().iter().take(6) {
        println!("NEWS {} — {}: {}", n.when, n.headline, n.body);
    }
    let card = r.agent_card(17).unwrap();
    println!(
        "{} ({}), {} — {}",
        card.name, card.age, card.occupation, card.doing
    );
    for j in card.journal.iter().take(16) {
        println!("   {} {}", j.when, j.text);
    }
    let bytes = w.serialize();
    println!(
        "world snapshot {} KB, bytes/agent {}",
        bytes.len() / 1024,
        livingsim_core::agents::Agents::bytes_per_agent()
    );
    for r in &w.regions {
        println!(
            "region {} snapshot {} KB",
            r.id,
            livingsim_core::snapshot::encode_region(r).len() / 1024
        );
    }
}
