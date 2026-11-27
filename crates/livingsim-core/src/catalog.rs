//! The embedded content (`/data`, compiled at build time) plus lookups derived from it.
//!
//! The catalog is immutable and identical on every platform, so a global is safe here.

use livingsim_content::{ActivityKind, Content, Place};
use std::sync::OnceLock;

static CONTENT_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/content.bin"));
static SIM_VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/sim_version.txt"));

/// Hex hash of engine sources + content. Snapshots record it.
pub fn sim_version() -> &'static str {
    SIM_VERSION
}

pub fn sim_version_bytes() -> [u8; 32] {
    let mut out = [0u8; 32];
    let hex = SIM_VERSION.as_bytes();
    for (i, o) in out.iter_mut().enumerate() {
        let n = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
        *o = n(hex[i * 2]) << 4 | n(hex[i * 2 + 1]);
    }
    out
}

/// Well-known tags, resolved once.
#[derive(Debug)]
pub struct Tags {
    pub grocery: Option<u16>,
    pub school: Option<u16>,
    pub university: Option<u16>,
    pub farm: Option<u16>,
    pub gateway: Option<u16>,
    pub park: Option<u16>,
}

#[derive(Debug)]
pub struct Catalog {
    pub content: Content,
    pub tags: Tags,
    /// Activities usable in free time (not sleep/work/school).
    pub free_activities: Vec<u16>,
    pub sleep_activity: u16,
    pub work_activity: u16,
    pub school_activity: u16,
    /// Fallback when nothing else scores: first `Relax` activity.
    pub relax_activity: u16,
    pub schedule_toddler: u16,
    pub schedule_child: u16,
    pub schedule_university: u16,
    pub schedule_retired: u16,
    pub schedule_unemployed: u16,
    pub schedule_daycare: u16,
    /// Activity of toddlers at daycare.
    pub daycare_activity: u16,
    /// Resting while ill at home / in hospital.
    pub recover_home: u16,
    pub recover_hospital: Option<u16>,
    pub gateway_building: Option<u16>,
    /// Base decay per hour for each need.
    pub decay: [i32; livingsim_content::NEED_COUNT],
    /// Net rate per hour for each (activity, need) = effect - decay.
    pub rates: Vec<[i32; livingsim_content::NEED_COUNT]>,
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

pub fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| {
        let content: Content =
            bincode::deserialize(CONTENT_BLOB).expect("embedded content is valid");
        build(content)
    })
}

pub fn content() -> &'static Content {
    &catalog().content
}

fn build(content: Content) -> Catalog {
    let first = |k: ActivityKind| {
        content
            .activities
            .iter()
            .position(|a| a.kind == k)
            .expect("validated") as u16
    };
    let free_activities = content
        .activities
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            !matches!(
                a.kind,
                ActivityKind::Sleep
                    | ActivityKind::Work
                    | ActivityKind::School
                    | ActivityKind::Recover
            )
        })
        .map(|(i, _)| i as u16)
        .collect();
    let mut decay = [0; livingsim_content::NEED_COUNT];
    for n in &content.needs {
        decay[n.need as usize] = n.decay_per_hour;
    }
    let rates = content
        .activities
        .iter()
        .map(|a| {
            let mut r = [0; livingsim_content::NEED_COUNT];
            for i in 0..r.len() {
                r[i] = a.effects[i] - decay[i];
            }
            r
        })
        .collect();
    let s = |id: &str| content.schedule(id).expect("validated");
    let tags = Tags {
        grocery: content.tag("grocery"),
        school: content.tag("school"),
        university: content.tag("university"),
        farm: content.tag("farm"),
        gateway: content.tag("gateway"),
        park: content.tag("park"),
    };
    // Sanity: activities at OtherRegion are only VisitRegion.
    debug_assert!(content
        .activities
        .iter()
        .all(|a| (a.place == Place::OtherRegion) == (a.kind == ActivityKind::VisitRegion)));
    Catalog {
        sleep_activity: first(ActivityKind::Sleep),
        work_activity: first(ActivityKind::Work),
        school_activity: first(ActivityKind::School),
        relax_activity: first(ActivityKind::Relax),
        schedule_toddler: s("toddler"),
        schedule_child: s("child_school"),
        schedule_university: s("student_university"),
        schedule_retired: s("retired"),
        schedule_unemployed: s("unemployed"),
        schedule_daycare: content.schedule("daycare_child").unwrap_or(s("toddler")),
        daycare_activity: content
            .activities
            .iter()
            .position(|a| a.kind == ActivityKind::School && a.max_age < 6)
            .map(|i| i as u16)
            .unwrap_or(first(ActivityKind::School)),
        recover_home: content
            .activities
            .iter()
            .position(|a| a.kind == ActivityKind::Recover && a.place == Place::Home)
            .map(|i| i as u16)
            .unwrap_or(first(ActivityKind::Sleep)),
        recover_hospital: content
            .activities
            .iter()
            .position(|a| a.kind == ActivityKind::Recover && matches!(a.place, Place::Tag(_)))
            .map(|i| i as u16),
        gateway_building: content.building("highway_exit"),
        free_activities,
        decay,
        rates,
        tags,
        content,
    }
}
