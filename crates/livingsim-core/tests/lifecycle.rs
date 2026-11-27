//! Life cycle invariants (Phase 3).

use livingsim_core::agents::flags;
use livingsim_core::roads::NONE;
use livingsim_core::snapshot::{decode_region, region_hash};
use livingsim_core::time::{age_years, TICKS_PER_DAY};
use livingsim_core::World;

/// Forty days: people are born and die, couples form, money stays conserved, and
/// biology is respected (only a woman with a male partner gets pregnant).
#[test]
fn life_goes_on_and_money_is_conserved() {
    let mut w = World::new(11);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    let (mut births, mut deaths, mut couples) = (0, 0, 0);
    for _ in 0..40 {
        for _ in 0..24 {
            w.step_window();
        }
        for (r, s) in w.regions.iter().zip(&start) {
            assert_eq!(r.money_total(), *s, "money not conserved in {}", r.name());
            let d = &r.systems.demography;
            births += d.yesterday.births;
            deaths += d.yesterday.deaths;
            couples += d.yesterday.couples;
            for &mother in d.pregnancies.keys() {
                let i = mother as usize;
                assert!(
                    r.agents.flags[i] & flags::FEMALE != 0,
                    "only women get pregnant"
                );
                let p = r.agents.partner[i];
                assert!(
                    p != NONE && r.agents.flags[p as usize] & flags::FEMALE == 0,
                    "pregnant with a male partner"
                );
            }
        }
    }
    assert!(births > 0, "babies are born");
    assert!(deaths > 0, "people die of old age or illness");
    assert!(couples > 0, "couples form");
    // Family links never point to a free slot or to oneself.
    for r in &w.regions {
        for a in 0..r.agents.len() as u32 {
            if !r.resident(a) {
                continue;
            }
            let i = a as usize;
            for p in [
                r.agents.partner[i],
                r.agents.parent_a[i],
                r.agents.parent_b[i],
            ] {
                assert!(
                    p == NONE || (p != a && r.resident(p)),
                    "dangling family link"
                );
            }
            let p = r.agents.partner[i];
            if p != NONE {
                assert_eq!(
                    r.agents.partner[p as usize], a,
                    "partners point at each other"
                );
            }
            // Ages are sensible.
            assert!(age_years(r.agents.birth[i], w.tick) < 130);
        }
    }
}

/// A Phase 2 (format v2) snapshot loads: people are kept and gain education.
#[test]
fn migrates_v2_snapshot() {
    let bytes = include_bytes!("fixtures/region_v2_village.bin.zst");
    assert_eq!(
        livingsim_core::snapshot::read_header(bytes)
            .unwrap()
            .format_version,
        2
    );
    let r = decode_region(bytes).expect("v2 migrates");
    assert_eq!(r.population(), 501);
    assert!(r.systems.demography.bootstrapped);
    assert!(r.systems.economy.bootstrapped);
    let educated = (0..r.agents.len() as u32)
        .filter(|a| r.systems.demography.education_of(*a) > 0)
        .count();
    assert!(educated > 200);
    assert_eq!(
        region_hash(&r),
        region_hash(&decode_region(bytes).unwrap()),
        "deterministic migration"
    );
}

#[test]
fn villagers_commute_to_the_city() {
    let mut w = World::new(12);
    w.step_to(10 * TICKS_PER_DAY);
    let out = w.regions[1].systems.demography.remote_jobs.len();
    let inn = w.regions[0].systems.demography.remote_staff.len();
    assert!(out > 0, "some villagers took jobs in the city");
    assert_eq!(out, inn, "both towns agree on who commutes");
}
