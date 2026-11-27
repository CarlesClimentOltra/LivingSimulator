//! Economy invariants (Phase 2).

use livingsim_content::FirmKind;
use livingsim_core::snapshot::{decode_region, encode_region, region_hash};
use livingsim_core::time::TICKS_PER_DAY;
use livingsim_core::{World, WINDOW};

/// Money is never created or destroyed: every region's accounts (agents, firms, treasury,
/// rest of the world, other regions) always add up to the same total.
#[test]
fn money_is_conserved() {
    let mut w = World::new(3);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    // Four days, checked every simulated hour, including a month change? Start is Jan 1;
    // rent day is covered by `rent_is_paid_monthly`.
    for _ in 0..(4 * TICKS_PER_DAY / WINDOW) {
        w.step_window();
        for (r, s) in w.regions.iter().zip(&start) {
            assert_eq!(
                r.money_total(),
                *s,
                "money created or destroyed in {} at tick {}",
                r.name(),
                w.tick
            );
        }
    }
    let r = &w.regions[0];
    let e = &r.systems.economy;
    assert!(
        e.yesterday.wages_private > 0 && e.yesterday.wages_public > 0,
        "wages are paid"
    );
    assert!(e.yesterday.retail_sales > 0, "shops sell");
    assert!(
        e.yesterday.income_tax > 0 && e.yesterday.sales_tax > 0,
        "taxes are collected"
    );
}

#[test]
fn rent_is_paid_monthly_and_money_still_conserved() {
    let mut w = World::new(4);
    let start: Vec<i128> = w.regions.iter().map(|r| r.money_total()).collect();
    // February 1st is day 31.
    w.step_to(31 * TICKS_PER_DAY + 60);
    for (r, s) in w.regions.iter().zip(&start) {
        assert_eq!(r.money_total(), *s);
    }
    let e = &w.regions[0].systems.economy;
    assert!(
        e.today.rent_paid > 0 || e.yesterday.rent_paid > 0,
        "rent was paid on the 1st"
    );
}

#[test]
fn firms_and_tenures_are_created() {
    let w = World::new(5);
    let r = &w.regions[0];
    let e = &r.systems.economy;
    assert!(e.bootstrapped);
    assert_eq!(e.tenure.len(), r.households.len());
    for kind in [
        FirmKind::Shop,
        FirmKind::Warehouse,
        FirmKind::Office,
        FirmKind::Public,
        FirmKind::RealEstate,
    ] {
        assert!(e.firms.iter().any(|f| f.kind == kind), "missing {kind:?}");
    }
    assert!(e.treasury > 0);
    // Every private firm has an owner.
    assert!(e
        .firms
        .iter()
        .filter(|f| f.is_private())
        .all(|f| f.owner != u32::MAX));
}

/// A Phase 1 (format v1) snapshot still loads: people are kept and the economy is created.
#[test]
fn migrates_v1_snapshot() {
    let bytes = include_bytes!("fixtures/region_v1_village.bin.zst");
    let h = livingsim_core::snapshot::read_header(bytes).unwrap();
    assert_eq!(h.format_version, 1);
    let mut r = decode_region(bytes).expect("v1 migrates");
    assert_eq!(r.population(), 501);
    assert!(r.systems.economy.bootstrapped);
    assert!(!r.systems.economy.firms.is_empty());
    // Deterministic migration.
    let again = decode_region(bytes).unwrap();
    assert_eq!(region_hash(&r), region_hash(&again));
    // And it keeps simulating (the village alone, without inbound messages).
    let total = r.money_total();
    for _ in 0..TICKS_PER_DAY {
        if r.tick.is_multiple_of(WINDOW) {
            r.take_outbox();
            r.receive(&[]);
        }
        r.step();
    }
    assert_eq!(r.money_total(), total);
    // Round trip in the new format.
    let back = decode_region(&encode_region(&r)).unwrap();
    assert_eq!(region_hash(&back), region_hash(&r));
}
