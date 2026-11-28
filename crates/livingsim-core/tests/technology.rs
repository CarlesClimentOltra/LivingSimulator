//! Technology (Phase 7).

use livingsim_core::catalog::content;
use livingsim_core::snapshot::{decode_region, read_header};
use livingsim_core::World;

const V7_VILLAGE: &[u8] = include_bytes!("fixtures/region_v7_village.bin.zst");

#[test]
fn format_7_region_migrates() {
    assert_eq!(read_header(V7_VILLAGE).unwrap().format_version, 7);
    let mut r = decode_region(V7_VILLAGE).expect("v7 snapshot decodes");
    assert!(r.systems.technology.researched.is_empty());
    let money = r.money_total();
    let end = r.tick + 2 * 1440;
    while r.tick < end {
        r.step();
        r.take_outbox();
    }
    assert_eq!(r.money_total(), money);
}

/// The city researches its first technology within a few weeks, the village adopts it
/// from the city, trips get faster and money stays conserved.
#[test]
fn research_and_diffusion() {
    let mut w = World::new(4);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    let first = content().technologies[0].id.clone();
    let mut city_at = None;
    let mut village_at = None;
    for day in 1..=70u32 {
        for _ in 0..24 {
            w.step_window();
        }
        for (r, s) in w.regions.iter().zip(&start) {
            assert_eq!(r.money_total(), *s, "money not conserved in {}", r.name());
        }
        if city_at.is_none() && w.regions[0].systems.technology.knows(&first) {
            city_at = Some(day);
        }
        if village_at.is_none() && w.regions[1].systems.technology.knows(&first) {
            village_at = Some(day);
        }
    }
    let c = city_at.expect("the city researches something");
    let v = village_at.expect("the village adopts it");
    assert!(c < v, "the village adopts after the city ({c} vs {v})");
    assert!(
        w.regions[0].tech.walk < 1000,
        "buses make walking trips faster"
    );
    assert!(
        w.regions[1]
            .systems
            .technology
            .history
            .iter()
            .all(|p| *p == 0),
        "no research in the village"
    );
}
