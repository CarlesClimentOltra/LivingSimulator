//! Determinism and persistence tests. These protect the core promise of the project:
//! every visitor, the runner and the browser compute exactly the same world.

use livingsim_core::messages::MessageLog;
use livingsim_core::snapshot::{decode_log, decode_region, encode_log, encode_region, region_hash};
use livingsim_core::time::TICKS_PER_DAY;
use livingsim_core::{RegionReplay, World, WINDOW};

const SEED: u64 = 42;

#[test]
fn same_seed_same_world() {
    let mut a = World::new(SEED);
    let mut b = World::new(SEED);
    assert_eq!(a.state_hash(), b.state_hash());
    a.step_to(600);
    for _ in 0..10 {
        b.step_window();
    }
    assert_eq!(a.tick, b.tick);
    assert_eq!(
        a.state_hash(),
        b.state_hash(),
        "step() and step_window() must agree"
    );
    let c = World::new(SEED + 1);
    assert_ne!(World::new(SEED).state_hash(), c.state_hash());
}

#[test]
fn serialize_roundtrip_continues_identically() {
    let mut w = World::new(SEED);
    w.step_to(TICKS_PER_DAY / 2 + 7); // mid-window on purpose
    let bytes = w.serialize();
    let mut w2 = World::deserialize(&bytes).expect("decode");
    assert_eq!(w.state_hash(), w2.state_hash());
    w.step_to(TICKS_PER_DAY + 30);
    w2.step_to(TICKS_PER_DAY + 30);
    assert_eq!(w.state_hash(), w2.state_hash());
}

#[test]
fn single_region_replay_matches_full_world() {
    // Run the full world for three days, recording what each region receives (farms start
    // selling their surplus to the other town after a couple of days).
    let mut w = World::new(SEED);
    w.step_to(TICKS_PER_DAY); // checkpoint at a window boundary
    let checkpoints: Vec<Vec<u8>> = w.regions.iter().map(encode_region).collect();
    let start = w.tick;
    let end = start + 3 * TICKS_PER_DAY + 120;
    let mut logs: Vec<MessageLog> = (0..w.regions.len() as u16)
        .map(|r| MessageLog {
            region: r,
            from: start,
            to: end,
            deliveries: vec![],
        })
        .collect();
    while w.tick < end {
        let t = w.tick;
        if let Some(d) = w.step() {
            for (dest, msgs) in d {
                logs[dest as usize].deliveries.push((t, msgs));
            }
        }
    }
    let delivered: usize = logs.iter().map(|l| l.deliveries.len()).sum();
    assert!(delivered > 0, "regions should exchange messages");
    for (i, cp) in checkpoints.iter().enumerate() {
        let region = decode_region(cp).unwrap();
        let log = decode_log(&encode_log(&logs[i])).unwrap();
        let mut replay = RegionReplay::new(region);
        replay.add_log(log);
        replay.step_to(end).expect("log covers the span");
        assert_eq!(
            region_hash(&replay.region),
            region_hash(&w.regions[i]),
            "region {i} diverged in replay"
        );
    }
}

#[test]
fn replay_stops_without_messages() {
    let w = World::new(SEED);
    let mut replay = RegionReplay::new(w.regions[0].clone());
    assert!(replay.step_to(WINDOW * 2).is_err());
}

#[test]
fn world_from_region_checkpoints() {
    let mut w = World::new(SEED);
    w.step_to(TICKS_PER_DAY);
    let regions = w
        .regions
        .iter()
        .map(|r| decode_region(&encode_region(r)).unwrap())
        .collect();
    let mut w2 = World::from_regions(w.seed, w.intercity.clone(), regions);
    assert_eq!(w.state_hash(), w2.state_hash());
    w.step_to(TICKS_PER_DAY * 2);
    w2.step_to(TICKS_PER_DAY * 2);
    assert_eq!(w.state_hash(), w2.state_hash());
}

/// The golden hash pins the exact behaviour of this version. If you change the simulation
/// on purpose, run `scripts/update-golden.sh` and commit the new value.
#[test]
fn golden_hash() {
    let golden = include_str!("golden.txt").trim();
    let mut w = World::new(SEED);
    w.step_to(livingsim_core::time::TICKS_PER_DAY / 2);
    let h = w.state_hash().to_hex();
    if std::env::var("LIVINGSIM_PRINT_GOLDEN").is_ok() {
        println!("GOLDEN={h}");
        return;
    }
    assert_eq!(
        h, golden,
        "simulation output changed: if intended, run scripts/update-golden.sh"
    );
}
