//! Content can be added, removed or reordered in `/data` without corrupting running worlds:
//! snapshots carry the names behind their indices and are translated on load.

use livingsim_core::content_ids::ContentIds;
use livingsim_core::snapshot::{decode_region, encode_region, region_hash};
use livingsim_core::World;

fn pos(list: &[String], id: &str) -> u16 {
    list.iter().position(|x| x == id).unwrap() as u16
}

#[test]
fn reordered_content_is_remapped_by_name() {
    let mut w = World::new(21);
    w.step_to(600);
    let original = w.regions[0].clone();
    let now = ContentIds::current();

    // Pretend the snapshot was written by a build whose lists were in reverse order.
    let mut old = now.clone();
    old.buildings.reverse();
    old.professions.reverse();
    old.schedules.reverse();
    old.activities.reverse();
    old.goods.reverse();
    let mut r = original.clone();
    for d in r.buildings.def.iter_mut() {
        *d = pos(&old.buildings, &now.buildings[*d as usize]);
    }
    for i in 0..r.agents.len() {
        r.agents.schedule[i] = pos(
            &old.schedules,
            &now.schedules[r.agents.schedule[i] as usize],
        );
        if r.agents.profession[i] != u16::MAX {
            r.agents.profession[i] = pos(
                &old.professions,
                &now.professions[r.agents.profession[i] as usize],
            );
        }
        for v in [&mut r.agents.act[i], &mut r.agents.next_act[i]] {
            if (*v as usize) < now.activities.len() {
                *v = pos(&old.activities, &now.activities[*v as usize]);
            }
        }
    }
    for f in r.systems.economy.firms.iter_mut() {
        if (f.good as usize) < now.goods.len() {
            f.good = pos(&old.goods, &now.goods[f.good as usize]);
        }
    }
    r.content_ids = old;
    // The journal stores activity indices too: translate them the same way.
    r.agents.journal.remap_activities(|v| {
        if (v as usize) < now.activities.len() {
            pos(&r_old_acts(), &now.activities[v as usize])
        } else {
            v
        }
    });

    let restored = decode_region(&encode_region(&r)).unwrap();
    assert_eq!(restored.content_ids, now);
    assert_eq!(restored.buildings.def, original.buildings.def);
    assert_eq!(restored.agents.schedule, original.agents.schedule);
    assert_eq!(restored.agents.profession, original.agents.profession);
    assert_eq!(region_hash(&restored), region_hash(&original));
}

fn r_old_acts() -> Vec<String> {
    let mut a = ContentIds::current().activities;
    a.reverse();
    a
}

#[test]
fn snapshots_record_their_content() {
    let w = World::new(22);
    let r = decode_region(&encode_region(&w.regions[0])).unwrap();
    assert_eq!(r.content_ids, ContentIds::current());
}
