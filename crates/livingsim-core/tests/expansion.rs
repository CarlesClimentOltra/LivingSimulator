//! New towns and the world map (Phase 6).

use livingsim_core::snapshot::{decode_region, read_header};
use livingsim_core::World;

const V6_VILLAGE: &[u8] = include_bytes!("fixtures/region_v6_village.bin.zst");

#[test]
fn format_6_region_migrates() {
    assert_eq!(read_header(V6_VILLAGE).unwrap().format_version, 6);
    let mut r = decode_region(V6_VILLAGE).expect("v6 snapshot decodes");
    assert_eq!(r.systems.expansion.founded_at, 0);
    assert!(r.systems.expansion.parent.is_none());
    let money = r.money_total();
    let end = r.tick + 2 * 1440;
    while r.tick < end {
        r.step();
        r.take_outbox();
    }
    assert_eq!(r.money_total(), money);
}

fn overlap(a: &livingsim_core::region::Region, b: &livingsim_core::region::Region) -> bool {
    let (ax0, ay0, ax1, ay1) = (
        a.origin.0,
        a.origin.1,
        a.origin.0 + a.map.width as i32,
        a.origin.1 + a.map.height as i32,
    );
    let (bx0, by0, bx1, by1) = (
        b.origin.0,
        b.origin.1,
        b.origin.0 + b.map.width as i32,
        b.origin.1 + b.map.height as i32,
    );
    ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
}

/// A new town is founded next to the city: both get a highway exit facing each other, the
/// maps do not overlap, the road networks stay connected, people travel between them and
/// every town conserves its money.
#[test]
fn founding_a_town() {
    let mut w = World::new(9);
    w.step_to(2 * 1440);
    let id = w.found_town(0).expect("a free side");
    assert_eq!(id, 2);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    for _ in 0..(3 * 24) {
        w.step_window();
        for (r, s) in w.regions.iter().zip(&start) {
            assert_eq!(r.money_total(), *s, "money not conserved in {}", r.name());
        }
    }
    let (city, town) = (&w.regions[0], &w.regions[2]);
    assert!(
        city.gateways.iter().any(|g| g.to_region == 2),
        "the city builds its exit"
    );
    assert!(town.gateways.iter().any(|g| g.to_region == 0));
    assert_eq!(town.systems.expansion.parent, Some(0));
    assert!(city.systems.expansion.children.contains(&2));
    for r in &w.regions {
        assert!(r.roads.is_connected(), "{} stays connected", r.name());
    }
    for i in 0..w.regions.len() {
        for j in i + 1..w.regions.len() {
            assert!(
                !overlap(&w.regions[i], &w.regions[j]),
                "regions {i} and {j} overlap"
            );
        }
    }
    // The two exits face each other across the highway.
    let gp = |r: &livingsim_core::region::Region, to: u16| {
        let g = r.gateways.iter().find(|g| g.to_region == to).unwrap();
        (
            r.origin.0 + r.roads.node_x[g.node as usize] as i32,
            r.origin.1 + r.roads.node_y[g.node as usize] as i32,
        )
    };
    let (a, b) = (gp(city, 2), gp(town, 0));
    assert!(a.0 == b.0 || a.1 == b.1, "aligned exits {a:?} {b:?}");
    let route = w.intercity.route(2, 1).expect("connected");
    assert_eq!(route.0, vec![2, 0, 1]);
    assert!(town.population() > 100);
}
