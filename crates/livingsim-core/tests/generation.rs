//! Invariants of the generated city.

use livingsim_core::agents::Agents;
use livingsim_core::catalog::catalog;
use livingsim_core::roads::NONE;
use livingsim_core::time::age_years;
use livingsim_core::World;

#[test]
fn initial_city_is_coherent() {
    let w = World::new(7);
    let c = catalog();
    let city = &w.regions[0];
    let pop = city.population();
    let target = c.content.world.templates[city.template as usize].target_population;
    assert!(pop >= target, "population {pop} < target {target}");
    assert!(city.roads.is_connected(), "road network must be connected");
    let mut employed = 0;
    for a in 0..city.agents.len() as u32 {
        let i = a as usize;
        assert!(city.agents.alive(a));
        assert_ne!(city.agents.home[i], NONE, "everyone has a home");
        let age = age_years(city.agents.birth[i], 0);
        if (6..=17).contains(&age) {
            assert_ne!(city.agents.work[i], NONE, "every child has a school");
        }
        if city.agents.profession[i] != u16::MAX {
            employed += 1;
            assert!(age >= 16);
            assert_ne!(city.agents.work[i], NONE);
        }
    }
    assert!(
        employed * 100 / pop >= 45,
        "about half the city works ({employed}/{pop})"
    );
    // Every building is reachable from the road network.
    for b in 0..city.buildings.len() {
        assert_ne!(
            city.buildings.access_node[b], NONE,
            "building {b} has road access"
        );
    }
    // Two regions connected by a highway.
    assert_eq!(w.regions.len(), 2);
    assert_eq!(w.regions[0].gateways[0].to_region, 1);
    assert_eq!(w.regions[1].gateways[0].to_region, 0);
}

#[test]
fn memory_budget_per_agent() {
    // See docs/ARCHITECTURE.md §3.4: ≤ 384 bytes per agent including the journal.
    let bytes = Agents::bytes_per_agent();
    println!("bytes per agent: {bytes}");
    assert!(bytes <= 384, "agent columns use {bytes} B > 384 B budget");
}
