//! Construction and growth (Phase 4).

use livingsim_core::catalog::content;
use livingsim_core::map::TileKind;
use livingsim_core::snapshot::{decode_region, read_header};
use livingsim_core::systems::construction::{Phase, Work};
use livingsim_core::time::TICKS_PER_DAY;
use livingsim_core::World;

const V4_VILLAGE: &[u8] = include_bytes!("fixtures/region_v4_village.bin.zst");

/// A Phase 3 (format 4) region loads, grows its ring of empty land on the first morning
/// and keeps every building where it was in the world.
#[test]
fn format_4_region_migrates_and_expands() {
    assert_eq!(read_header(V4_VILLAGE).unwrap().format_version, 4);
    let mut r = decode_region(V4_VILLAGE).expect("v4 snapshot decodes");
    let (w0, origin0) = (r.map.width, r.origin);
    let world_pos: Vec<(i32, i32)> = (0..r.buildings.len())
        .map(|b| {
            (
                r.origin.0 + r.buildings.x[b] as i32,
                r.origin.1 + r.buildings.y[b] as i32,
            )
        })
        .collect();
    let money = r.money_total();
    let pop = r.population();
    assert!(!r.systems.construction.expanded);

    let next_morning = (r.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY + 6 * 60 + 1;
    while r.tick < next_morning {
        r.step();
        r.take_outbox();
    }
    let c = &r.systems.construction;
    assert!(c.expanded);
    assert!(r.map.width > w0, "the map grew");
    let dx = (r.map.width - w0) as i32 / 2;
    assert_eq!(r.origin, (origin0.0 - dx, origin0.1 - dx));
    for (b, p) in world_pos.iter().enumerate() {
        let is_gateway = r.gateways.iter().any(|g| g.building as usize == b);
        if !is_gateway {
            let now = (
                r.origin.0 + r.buildings.x[b] as i32,
                r.origin.1 + r.buildings.y[b] as i32,
            );
            assert_eq!(now, *p, "building {b} stays in place");
        }
    }
    // Highway exits sit on the new edge of the map.
    for g in &r.gateways {
        let x = r.roads.node_x[g.node as usize];
        assert!(x == 0 || x == r.map.width - 1);
    }
    assert!(r.roads.is_connected());
    assert_eq!(r.money_total(), money);
    assert!(r.population() + 5 >= pop);
}

/// Three months in a fresh world: sites are planned, paid phase by phase, built by crews
/// and opened as real buildings; money is conserved every day.
#[test]
fn towns_build_and_money_is_conserved() {
    let mut w = World::new(3);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    let site = content().construction.site;
    for _ in 0..90 {
        for _ in 0..24 {
            w.step_window();
        }
        for (r, s) in w.regions.iter().zip(&start) {
            assert_eq!(r.money_total(), *s, "money not conserved in {}", r.name());
            for p in &r.systems.construction.projects {
                if let Work::Building { site: b, def } = p.work {
                    let now = r.buildings.def[b as usize];
                    if p.phase == Phase::Finished {
                        assert_eq!(now, def, "finished sites become the building");
                    } else {
                        assert_eq!(now, site, "unfinished sites stay sites");
                    }
                }
            }
        }
    }
    let mut finished = 0;
    for r in &w.regions {
        let c = &r.systems.construction;
        finished += c.total.finished + c.today.finished;
        assert!(r.roads.is_connected(), "{} stays connected", r.name());
        // Finished streets are painted and in the road graph.
        for p in c.projects.iter().filter(|p| p.phase == Phase::Finished) {
            if let Work::Road { a, b } = p.work {
                assert!(r.roads.find_edge(a, b).is_some());
                let (x, y) = (r.roads.node_x[a as usize], r.roads.node_y[a as usize]);
                assert_ne!(r.map.get(x, y), TileKind::RoadWorks);
            }
        }
    }
    assert!(finished >= 3, "something got built ({finished})");
}
