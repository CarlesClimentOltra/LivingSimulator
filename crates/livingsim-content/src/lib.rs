//! Content definitions for Living Simulator.
//!
//! Everything that describes *what exists* in the world (needs, activities, schedules,
//! professions, buildings, names, text templates, …) lives in RON files under `/data`.
//! This crate has two layers:
//!
//! * [`raw`]: the human-friendly schema that maps 1:1 to the RON files (string ids,
//!   `"09:00"` times, …).
//! * [`Content`]: the *compiled* form used by the simulation, with every reference
//!   resolved to a small integer index and every value validated.
//!
//! The compiled form only contains integers and strings (never floats), so it can be
//! hashed and is identical on every platform.

#![forbid(unsafe_code)]

pub mod raw;

mod compile;
pub use compile::{compile, ContentError};

#[cfg(feature = "loader")]
mod loader;
#[cfg(feature = "loader")]
pub use loader::{data_files, load_dir};

use serde::{Deserialize, Serialize};

/// Number of needs simulated by the engine. The order is fixed (see [`Need`]).
pub const NEED_COUNT: usize = 7;

/// The needs every inhabitant has. Their tuning lives in `data/needs.ron`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Need {
    Hunger = 0,
    Energy = 1,
    Hygiene = 2,
    Fun = 3,
    Social = 4,
    Money = 5,
    Safety = 6,
}

impl Need {
    pub const ALL: [Need; NEED_COUNT] = [
        Need::Hunger,
        Need::Energy,
        Need::Hygiene,
        Need::Fun,
        Need::Social,
        Need::Money,
        Need::Safety,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Need::Hunger => "hunger",
            Need::Energy => "energy",
            Need::Hygiene => "hygiene",
            Need::Fun => "fun",
            Need::Social => "social",
            Need::Money => "money",
            Need::Safety => "safety",
        }
    }
}

/// Zoning of a city block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Zone {
    Downtown,
    Mixed,
    Residential,
    Industrial,
    Rural,
    Civic,
    Park,
}

/// Size of a lot inside a block (blocks are 12×12 tiles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LotSize {
    /// 4×6 tiles, 6 per block.
    Small,
    /// 6×6 tiles, 4 per block.
    Medium,
    /// 12×12 tiles, the whole block.
    Large,
}

/// What an activity *means* to the engine. Content can add any number of activities of
/// each kind (e.g. several leisure activities at different building tags).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActivityKind {
    Sleep,
    Work,
    School,
    EatHome,
    EatOut,
    Shop,
    Hygiene,
    Leisure,
    Relax,
    /// Visit another region (city/town) for a few hours.
    VisitRegion,
}

/// Where an activity takes place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    Home,
    Workplace,
    /// Any building carrying this tag (resolved to a tag index when compiled).
    Tag(u16),
    /// Another region (only for [`ActivityKind::VisitRegion`]).
    OtherRegion,
}

/// Days of the week as a bit mask (bit 0 = Monday … bit 6 = Sunday).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DayMask(pub u8);

impl DayMask {
    pub fn contains(self, weekday: u8) -> bool {
        self.0 & (1 << weekday) != 0
    }
}

/// A time span inside a day, in minutes since midnight. `end` may be smaller than
/// `start` for spans that cross midnight (night shifts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DaySpan {
    pub start: u16,
    pub end: u16,
}

impl DaySpan {
    pub fn crosses_midnight(self) -> bool {
        self.end <= self.start
    }
    /// Length in minutes.
    pub fn len(self) -> u16 {
        if self.crosses_midnight() {
            1440 - self.start + self.end
        } else {
            self.end - self.start
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedDef {
    pub need: Need,
    pub name: String,
    /// Loss per simulated hour when no activity affects this need (0..=10_000 scale).
    pub decay_per_hour: i32,
    /// Below this value the need is "critical" and forces a decision.
    pub critical: u16,
    /// Relative importance in utility scoring (100 = normal).
    pub weight: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityDef {
    pub id: String,
    pub name: String,
    pub kind: ActivityKind,
    pub place: Place,
    /// Change per hour for each need while doing the activity, *added* to the decay.
    pub effects: [i32; NEED_COUNT],
    pub min_minutes: u16,
    pub max_minutes: u16,
    pub cost_cents: i64,
    /// Constant added to the utility score.
    pub bias: i32,
    /// Ages allowed (inclusive).
    pub min_age: u8,
    pub max_age: u8,
    /// Verb used in journals and UI, e.g. "watching TV".
    pub doing: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleBlock {
    pub days: DayMask,
    pub span: DaySpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDef {
    pub id: String,
    pub name: String,
    /// Obligations (work, school). Empty for retirees, toddlers, unemployed.
    pub blocks: Vec<ScheduleBlock>,
    /// Preferred sleeping time.
    pub sleep: DaySpan,
    /// Preferred sleeping time on days without blocks.
    pub sleep_free_day: DaySpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfessionDef {
    pub id: String,
    pub name: String,
    pub wage_per_shift_cents: i64,
    /// Schedules used by jobs of this profession; jobs are distributed among them.
    pub schedules: Vec<u16>,
    pub min_age: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSlots {
    pub profession: u16,
    pub count: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    pub lot: LotSize,
    /// Footprint inside the lot (tiles).
    pub width: u8,
    pub depth: u8,
    pub floors: u8,
    /// Households that can live here.
    pub dwellings: u16,
    pub jobs: Vec<JobSlots>,
    /// Tags (indices into [`Content::tags`]) used by activities and services.
    pub tags: Vec<u16>,
    /// (zone, weight) pairs used by the city generator.
    pub zones: Vec<(Zone, u16)>,
    /// Opening hours for visitors (activities at tags). `None` = always open.
    pub open: Option<DaySpan>,
    pub open_days: DayMask,
    /// Students that can attend (schools, university).
    pub students: u16,
    /// Goods stock capacity (shops), in meals.
    pub stock: u32,
    /// Maximum visitors at once (0 = unlimited).
    pub capacity: u16,
    /// Business run here, if any.
    pub firm: Option<FirmSpec>,
    /// Monthly rent per dwelling before location adjustments, in cents.
    pub rent_cents: i64,
    /// Patterns for firm names, e.g. "{family} Market". Empty = generic name.
    pub name_patterns: Vec<String>,
    /// Render hints.
    pub color: u32,
    pub roof_color: u32,
    pub roof: RoofKind,
}

/// What kind of business (or public service) runs in a building.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FirmKind {
    /// Paid by the town hall (schools, hospital, police, parks…).
    Public,
    /// Sells goods (food) to households.
    Shop,
    /// Serves meals and drinks; uses food from its own stock.
    FoodService,
    /// Sells an experience (bar, cinema, gym); no stock.
    Leisure,
    /// Produces food per worked shift.
    Farm,
    /// Produces goods per worked shift and exports them.
    Factory,
    /// Buys from producers or imports, supplies shops and food services.
    Warehouse,
    /// Sells services to the rest of the world.
    Office,
    /// Owns and rents out homes.
    RealEstate,
    /// Paid by the town hall for maintenance (and builds in Phase 4).
    Construction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmSpec {
    pub kind: FirmKind,
    /// Good produced/sold (index into `Content::goods`), if any.
    pub good: Option<u16>,
    /// Units produced per worked shift (farms, factories).
    pub per_shift: u32,
    /// Revenue from the rest of the world per worked shift (offices), in cents.
    pub export_per_shift_cents: i64,
    /// Food used per customer served (food services).
    pub food_per_customer: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoofKind {
    Flat,
    Gabled,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyllableSet {
    pub first: Vec<String>,
    pub middle: Vec<String>,
    pub last: Vec<String>,
    pub min_syllables: u8,
    pub max_syllables: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameStyle {
    pub id: String,
    pub female: SyllableSet,
    pub male: SyllableSet,
    pub family: SyllableSet,
    pub places: SyllableSet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamesDef {
    pub styles: Vec<NameStyle>,
    /// Lower-case names that must never be produced.
    pub blacklist: Vec<String>,
    pub street_suffixes: Vec<String>,
    pub avenue_suffixes: Vec<String>,
}

/// Engine-defined journal entry kinds. Templates for each live in `data/journal.ron`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u16)]
pub enum JournalKind {
    WokeUp = 0,
    WentTo = 1,
    Started = 2,
    WentToSleep = 3,
    Shopped = 4,
    ShelvesEmpty = 5,
    GotPaid = 6,
    LeftForRegion = 7,
    BackFromRegion = 8,
    ArrivedFromRegion = 9,
    NoFoodAtHome = 10,
    MovedIn = 11,
    BirthdayAge = 12,
    PaidRent = 13,
    MissedRent = 14,
    Hired = 15,
    LaidOff = 16,
    OpenedBusiness = 17,
    ReceivedRent = 18,
}

impl JournalKind {
    pub const ALL: [JournalKind; 19] = [
        JournalKind::WokeUp,
        JournalKind::WentTo,
        JournalKind::Started,
        JournalKind::WentToSleep,
        JournalKind::Shopped,
        JournalKind::ShelvesEmpty,
        JournalKind::GotPaid,
        JournalKind::LeftForRegion,
        JournalKind::BackFromRegion,
        JournalKind::ArrivedFromRegion,
        JournalKind::NoFoodAtHome,
        JournalKind::MovedIn,
        JournalKind::BirthdayAge,
        JournalKind::PaidRent,
        JournalKind::MissedRent,
        JournalKind::Hired,
        JournalKind::LaidOff,
        JournalKind::OpenedBusiness,
        JournalKind::ReceivedRent,
    ];
    pub fn id(self) -> &'static str {
        match self {
            JournalKind::WokeUp => "woke_up",
            JournalKind::WentTo => "went_to",
            JournalKind::Started => "started",
            JournalKind::WentToSleep => "went_to_sleep",
            JournalKind::Shopped => "shopped",
            JournalKind::ShelvesEmpty => "shelves_empty",
            JournalKind::GotPaid => "got_paid",
            JournalKind::LeftForRegion => "left_for_region",
            JournalKind::BackFromRegion => "back_from_region",
            JournalKind::ArrivedFromRegion => "arrived_from_region",
            JournalKind::NoFoodAtHome => "no_food_at_home",
            JournalKind::MovedIn => "moved_in",
            JournalKind::BirthdayAge => "birthday",
            JournalKind::PaidRent => "paid_rent",
            JournalKind::MissedRent => "missed_rent",
            JournalKind::Hired => "hired",
            JournalKind::LaidOff => "laid_off",
            JournalKind::OpenedBusiness => "opened_business",
            JournalKind::ReceivedRent => "received_rent",
        }
    }
    pub fn from_u16(v: u16) -> Option<JournalKind> {
        JournalKind::ALL.get(v as usize).copied()
    }
}

/// Engine-defined newspaper item kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u16)]
pub enum NewsKind {
    DailySummary = 0,
    BusiestPlace = 1,
    ShelvesEmpty = 2,
    Visitors = 3,
    FoodDelivery = 4,
    CityFounded = 5,
    PriceUp = 6,
    PriceDown = 7,
    Bankruptcy = 8,
    Reopened = 9,
    Unemployment = 10,
    Budget = 11,
    TaxChange = 12,
    FoodImports = 13,
}

impl NewsKind {
    pub const ALL: [NewsKind; 14] = [
        NewsKind::DailySummary,
        NewsKind::BusiestPlace,
        NewsKind::ShelvesEmpty,
        NewsKind::Visitors,
        NewsKind::FoodDelivery,
        NewsKind::CityFounded,
        NewsKind::PriceUp,
        NewsKind::PriceDown,
        NewsKind::Bankruptcy,
        NewsKind::Reopened,
        NewsKind::Unemployment,
        NewsKind::Budget,
        NewsKind::TaxChange,
        NewsKind::FoodImports,
    ];
    pub fn id(self) -> &'static str {
        match self {
            NewsKind::DailySummary => "daily_summary",
            NewsKind::BusiestPlace => "busiest_place",
            NewsKind::ShelvesEmpty => "shelves_empty",
            NewsKind::Visitors => "visitors",
            NewsKind::FoodDelivery => "food_delivery",
            NewsKind::CityFounded => "city_founded",
            NewsKind::PriceUp => "price_up",
            NewsKind::PriceDown => "price_down",
            NewsKind::Bankruptcy => "bankruptcy",
            NewsKind::Reopened => "reopened",
            NewsKind::Unemployment => "unemployment",
            NewsKind::Budget => "budget",
            NewsKind::TaxChange => "tax_change",
            NewsKind::FoodImports => "food_imports",
        }
    }
    pub fn from_u16(v: u16) -> Option<NewsKind> {
        NewsKind::ALL.get(v as usize).copied()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextDef {
    /// Indexed by `JournalKind as usize`.
    pub journal: Vec<String>,
    /// Indexed by `NewsKind as usize`: (headline, body).
    pub news: Vec<(String, String)>,
}

/// Simple economy parameters for Phase 1 (fully expanded in Phase 2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomyDef {
    pub pension_per_day_cents: i64,
    pub benefit_per_day_cents: i64,
    pub meal_price_cents: i64,
    pub meals_per_person_bought: u16,
    pub farm_food_per_day: u32,
    pub starting_money_min_cents: i64,
    pub starting_money_max_cents: i64,
    pub income_tax_permille: u16,
    pub sales_tax_permille: u16,
    pub min_income_tax_permille: u16,
    pub max_income_tax_permille: u16,
    /// Monthly tax paid by owner-occupiers.
    pub property_tax_cents: i64,
    /// Price a farm asks per unit of food, at start.
    pub farm_price_cents: i64,
    /// Warehouse margin over its purchase price (‰).
    pub wholesale_markup_permille: u16,
    /// Price of imported food relative to the farm price (‰).
    pub import_price_permille: u16,
    /// Price of exported surplus relative to the farm price (‰).
    pub export_price_permille: u16,
    /// Food a warehouse can import per day.
    pub import_cap_per_day: u32,
    /// Daily maintenance paid by the town per public building.
    pub maintenance_per_building_cents: i64,
    /// Initial cash of a firm, in days of full payroll.
    pub firm_start_cash_days: u16,
    /// Initial treasury, in days of public spending.
    pub treasury_start_days: u16,
    /// Days with negative cash before the first layoff.
    pub layoff_after_days: u16,
    /// Days with negative cash before bankruptcy.
    pub bankruptcy_after_days: u16,
    /// Days a bankrupt business stays closed before someone may reopen it.
    pub reopen_after_days: u16,
    /// Maximum hires per firm per day.
    pub hires_per_day: u16,
    /// Price change per daily adjustment (‰).
    pub price_step_permille: u16,
    /// Price bounds relative to the base price (‰).
    pub min_price_permille: u16,
    pub max_price_permille: u16,
}

/// Region templates used by the world generator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTemplate {
    pub id: String,
    /// Blocks along each axis.
    pub blocks: u16,
    pub target_population: u32,
    /// Zone by ring distance from the centre (in blocks): (max_distance, zone).
    pub rings: Vec<(u16, Zone)>,
    /// Permille of blocks that become parks.
    pub park_permille: u16,
    pub has_river: bool,
    /// Buildings that must exist: (building index, count).
    pub required: Vec<(u16, u16)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldDef {
    /// Regions generated at tick 0: (template index, name style index).
    pub initial_regions: Vec<(u16, u16)>,
    /// Highway length between neighbouring initial regions (tiles).
    pub highway_tiles: u32,
    pub templates: Vec<RegionTemplate>,
}

// ---- Content for later phases: schemas exist now so contributors can start adding data.

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodDef {
    pub id: String,
    pub name: String,
    pub base_price_cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrimeDef {
    pub id: String,
    pub name: String,
    /// Severity 1..=10 (non-graphic representation only).
    pub severity: u8,
    pub min_sentence_days: u32,
    pub max_sentence_days: u32,
    /// Permille base chance once motive threshold is reached.
    pub base_permille: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechDef {
    pub id: String,
    pub name: String,
    pub cost_points: u64,
    pub requires: Vec<u16>,
    /// Buildings unlocked (indices).
    pub unlocks: Vec<u16>,
}

/// The compiled, validated content of `/data`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    pub needs: Vec<NeedDef>,
    pub activities: Vec<ActivityDef>,
    pub schedules: Vec<ScheduleDef>,
    pub professions: Vec<ProfessionDef>,
    pub buildings: Vec<BuildingDef>,
    pub tags: Vec<String>,
    pub names: NamesDef,
    pub text: TextDef,
    pub economy: EconomyDef,
    pub world: WorldDef,
    pub goods: Vec<GoodDef>,
    pub crimes: Vec<CrimeDef>,
    pub technologies: Vec<TechDef>,
}

impl Content {
    pub fn tag(&self, name: &str) -> Option<u16> {
        self.tags.iter().position(|t| t == name).map(|i| i as u16)
    }
    pub fn building(&self, id: &str) -> Option<u16> {
        self.buildings
            .iter()
            .position(|b| b.id == id)
            .map(|i| i as u16)
    }
    pub fn schedule(&self, id: &str) -> Option<u16> {
        self.schedules
            .iter()
            .position(|s| s.id == id)
            .map(|i| i as u16)
    }
    pub fn activity(&self, id: &str) -> Option<u16> {
        self.activities
            .iter()
            .position(|a| a.id == id)
            .map(|i| i as u16)
    }
    pub fn good(&self, id: &str) -> Option<u16> {
        self.goods.iter().position(|g| g.id == id).map(|i| i as u16)
    }
    pub fn profession(&self, id: &str) -> Option<u16> {
        self.professions
            .iter()
            .position(|p| p.id == id)
            .map(|i| i as u16)
    }
}
