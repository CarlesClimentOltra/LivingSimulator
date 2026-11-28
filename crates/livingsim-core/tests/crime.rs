//! Crime and justice (Phase 5).

use livingsim_core::catalog::{catalog, content};
use livingsim_core::snapshot::{decode_region, read_header};
use livingsim_core::systems::crime::Status;
use livingsim_core::World;

const V5_VILLAGE: &[u8] = include_bytes!("fixtures/region_v5_village.bin.zst");

/// A Phase 4 (format 5) region loads with empty crime and justice state and keeps living.
#[test]
fn format_5_region_migrates() {
    assert_eq!(read_header(V5_VILLAGE).unwrap().format_version, 5);
    let mut r = decode_region(V5_VILLAGE).expect("v5 snapshot decodes");
    assert!(r.systems.crime.incidents.is_empty());
    assert!(r.systems.justice.inmates.is_empty());
    assert_eq!(r.content_ids.crimes.len(), content().crimes.len());
    assert!(r.systems.construction.expanded, "Phase 4 state is kept");
    let money = r.money_total();
    let end = r.tick + 3 * 1440;
    while r.tick < end {
        r.step();
        r.take_outbox();
    }
    assert_eq!(r.money_total(), money);
}

/// Two months: offences happen, some are solved, convicts are in jail and nobody in jail
/// commits a new offence; money is conserved every day.
#[test]
fn crime_and_justice_invariants() {
    let mut w = World::new(5);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    let jail_tag = content().tag("jail").unwrap();
    let mut incidents = 0;
    let mut convicted = 0;
    for _ in 0..60 {
        for _ in 0..24 {
            w.step_window();
        }
        for (r, s) in w.regions.iter().zip(&start) {
            assert_eq!(r.money_total(), *s, "money not conserved in {}", r.name());
            let j = &r.systems.justice;
            for (a, m) in &j.inmates {
                // Nobody commits an offence while held.
                assert!(r.systems.crime.incidents.iter().all(|x| x.offender != *a
                    || x.tick <= m.release.saturating_sub(1)
                    || x.status != Status::Open
                    || x.id == m.incident));
                let place = r.agents.place[*a as usize];
                let act = r.agents.act[*a as usize];
                let in_jail = place != u32::MAX && r.buildings.with_tag(jail_tag).contains(&place);
                let away = r.away.contains_key(a);
                let serving = act == catalog().serve_activity;
                assert!(
                    in_jail || away || !serving || place == u32::MAX,
                    "inmate {a} misplaced"
                );
            }
            for x in &r.systems.crime.incidents {
                if x.status == Status::Convicted {
                    assert!(
                        r.systems.crime.records.contains_key(&x.offender)
                            || !r.resident(x.offender)
                    );
                }
            }
        }
    }
    for r in &w.regions {
        let c = &r.systems.crime;
        incidents += c.total.by_crime.iter().sum::<u32>();
        convicted += c.total.convictions;
    }
    assert!(incidents > 20, "offences happen ({incidents})");
    assert!(convicted > 0, "some offenders are convicted");
}
