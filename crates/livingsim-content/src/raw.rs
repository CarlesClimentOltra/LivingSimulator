//! Human-friendly schema mirroring the RON files in `/data`.
//!
//! Every struct uses `deny_unknown_fields`, so typos in data files are reported instead of
//! silently ignored.

use crate::{LotSize, RoofKind, Zone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Need {
    pub need: String,
    pub name: String,
    pub decay_per_hour: i32,
    pub critical: u16,
    pub weight: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Place {
    Home,
    Workplace,
    Tag(String),
    OtherRegion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub id: String,
    pub name: String,
    pub kind: crate::ActivityKind,
    pub place: Place,
    /// need id -> change per hour (added to the need's base decay).
    #[serde(default)]
    pub effects: BTreeMap<String, i32>,
    /// (min, max) duration in minutes.
    pub minutes: (u16, u16),
    #[serde(default)]
    pub cost_cents: i64,
    #[serde(default)]
    pub bias: i32,
    /// (min, max) age in years, inclusive.
    #[serde(default = "default_ages")]
    pub ages: (u8, u8),
    pub doing: String,
}

fn default_ages() -> (u8, u8) {
    (0, 255)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleBlock {
    /// "Mon-Fri", "Sat-Sun", "daily", "Mon,Wed,Fri"…
    pub days: String,
    /// "09:00-17:00" (may cross midnight: "22:00-06:00").
    pub time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<ScheduleBlock>,
    pub sleep: String,
    pub sleep_free_day: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profession {
    pub id: String,
    pub name: String,
    pub wage_per_shift_cents: i64,
    pub schedules: Vec<String>,
    #[serde(default = "default_min_age")]
    pub min_age: u8,
}

fn default_min_age() -> u8 {
    18
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Building {
    pub id: String,
    pub name: String,
    pub lot: LotSize,
    /// (width, depth) in tiles.
    pub size: (u8, u8),
    #[serde(default = "one")]
    pub floors: u8,
    #[serde(default)]
    pub dwellings: u16,
    /// profession id -> number of jobs.
    #[serde(default)]
    pub jobs: BTreeMap<String, u16>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub zones: BTreeMap<Zone, u16>,
    /// "08:00-22:00"; omitted = always open.
    #[serde(default)]
    pub open: Option<String>,
    #[serde(default = "daily")]
    pub open_days: String,
    #[serde(default)]
    pub students: u16,
    #[serde(default)]
    pub stock: u32,
    /// Maximum visitors at once (0 = unlimited).
    #[serde(default)]
    pub capacity: u16,
    #[serde(default)]
    pub name_patterns: Vec<String>,
    /// "#rrggbb"
    pub color: String,
    #[serde(default = "default_roof_color")]
    pub roof_color: String,
    #[serde(default = "default_roof")]
    pub roof: RoofKind,
}

fn one() -> u8 {
    1
}
fn daily() -> String {
    "daily".into()
}
fn default_roof_color() -> String {
    "#8a8f98".into()
}
fn default_roof() -> RoofKind {
    RoofKind::Flat
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyllableSet {
    pub first: Vec<String>,
    #[serde(default)]
    pub middle: Vec<String>,
    pub last: Vec<String>,
    /// (min, max) number of syllables.
    pub syllables: (u8, u8),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameStyle {
    pub id: String,
    pub female: SyllableSet,
    pub male: SyllableSet,
    pub family: SyllableSet,
    pub places: SyllableSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamesCommon {
    pub blacklist: Vec<String>,
    pub street_suffixes: Vec<String>,
    pub avenue_suffixes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub journal: BTreeMap<String, String>,
    pub news: BTreeMap<String, (String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Economy {
    pub pension_per_day_cents: i64,
    pub benefit_per_day_cents: i64,
    pub meal_price_cents: i64,
    pub meals_per_person_bought: u16,
    pub farm_food_per_day: u32,
    pub starting_money_cents: (i64, i64),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionTemplate {
    pub id: String,
    pub blocks: u16,
    pub target_population: u32,
    pub rings: Vec<(u16, Zone)>,
    #[serde(default)]
    pub park_permille: u16,
    #[serde(default)]
    pub has_river: bool,
    /// building id -> count
    #[serde(default)]
    pub required: BTreeMap<String, u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct World {
    /// (template id, name style id)
    pub initial_regions: Vec<(String, String)>,
    pub highway_tiles: u32,
    pub templates: Vec<RegionTemplate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Good {
    pub id: String,
    pub name: String,
    pub base_price_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crime {
    pub id: String,
    pub name: String,
    pub severity: u8,
    pub sentence_days: (u32, u32),
    pub base_permille: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Technology {
    pub id: String,
    pub name: String,
    pub cost_points: u64,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub unlocks: Vec<String>,
}

/// Every file of `/data`, parsed but not yet validated.
#[derive(Clone, Debug, Default)]
pub struct RawContent {
    pub needs: Vec<Need>,
    pub activities: Vec<Activity>,
    pub schedules: Vec<Schedule>,
    pub professions: Vec<Profession>,
    pub buildings: Vec<Building>,
    pub name_styles: Vec<NameStyle>,
    pub names_common: Option<NamesCommon>,
    pub text: Option<Text>,
    pub economy: Option<Economy>,
    pub world: Option<World>,
    pub goods: Vec<Good>,
    pub crimes: Vec<Crime>,
    pub technologies: Vec<Technology>,
}
