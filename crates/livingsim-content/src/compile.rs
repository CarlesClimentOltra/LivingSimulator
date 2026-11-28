//! Validation and compilation of [`RawContent`] into [`Content`].

use crate::raw::{self, RawContent};
use crate::*;
use std::fmt;

/// All problems found in the content, with a human-readable location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError {
    pub problems: Vec<String>,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} problem(s) found in /data:", self.problems.len())?;
        for p in &self.problems {
            writeln!(f, "  - {p}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ContentError {}

struct Ctx {
    problems: Vec<String>,
}

impl Ctx {
    fn err(&mut self, msg: impl Into<String>) {
        self.problems.push(msg.into());
    }
}

/// Parses "HH:MM" into minutes since midnight.
pub fn parse_time(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    if h > 24 || m > 59 || (h == 24 && m != 0) {
        return None;
    }
    Some((h * 60 + m) % 1440)
}

/// Parses "HH:MM-HH:MM".
pub fn parse_span(s: &str) -> Option<DaySpan> {
    let (a, b) = s.split_once('-')?;
    Some(DaySpan {
        start: parse_time(a)?,
        end: parse_time(b)?,
    })
}

const DAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// Parses "Mon-Fri", "Sat-Sun", "daily", "weekdays", "weekend", "Mon,Wed,Fri".
pub fn parse_days(s: &str) -> Option<DayMask> {
    let s = s.trim().to_ascii_lowercase();
    match s.as_str() {
        "daily" => return Some(DayMask(0x7f)),
        "weekdays" => return Some(DayMask(0x1f)),
        "weekend" => return Some(DayMask(0x60)),
        _ => {}
    }
    let mut mask = 0u8;
    for part in s.split(',') {
        let part = part.trim();
        let idx = |d: &str| DAY_NAMES.iter().position(|n| *n == d.trim());
        if let Some((a, b)) = part.split_once('-') {
            let (a, b) = (idx(a)?, idx(b)?);
            if a > b {
                return None;
            }
            for d in a..=b {
                mask |= 1 << d;
            }
        } else {
            mask |= 1 << idx(part)?;
        }
    }
    Some(DayMask(mask))
}

fn parse_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

fn check_unique<'a>(ctx: &mut Ctx, what: &str, ids: impl Iterator<Item = &'a str>) {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            ctx.err(format!("duplicate {what} id `{id}`"));
        }
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            ctx.err(format!("{what} id `{id}` must be snake_case ascii"));
        }
    }
}

fn find(ids: &[&str], id: &str) -> Option<u16> {
    ids.iter().position(|x| *x == id).map(|i| i as u16)
}

fn syllables(ctx: &mut Ctx, where_: &str, s: &raw::SyllableSet) -> SyllableSet {
    if s.first.is_empty() || s.last.is_empty() {
        ctx.err(format!(
            "{where_}: `first` and `last` syllables must not be empty"
        ));
    }
    let (min, max) = s.syllables;
    if min < 2 || max < min {
        ctx.err(format!("{where_}: syllables must be (min>=2, max>=min)"));
    }
    if max > 2 && s.middle.is_empty() {
        ctx.err(format!(
            "{where_}: more than 2 syllables needs `middle` syllables"
        ));
    }
    SyllableSet {
        first: s.first.clone(),
        middle: s.middle.clone(),
        last: s.last.clone(),
        min_syllables: min,
        max_syllables: max,
    }
}

/// Validates and compiles raw content. All problems are reported at once.
pub fn compile(raw: &RawContent) -> Result<Content, ContentError> {
    let mut ctx = Ctx { problems: vec![] };

    // ---- needs
    let mut needs = Vec::new();
    for need in Need::ALL {
        match raw.needs.iter().find(|n| n.need == need.id()) {
            Some(n) => needs.push(NeedDef {
                need,
                name: n.name.clone(),
                decay_per_hour: n.decay_per_hour,
                critical: n.critical.min(10_000),
                weight: n.weight,
            }),
            None => ctx.err(format!("needs.ron: missing need `{}`", need.id())),
        }
    }
    for n in &raw.needs {
        if !Need::ALL.iter().any(|x| x.id() == n.need) {
            ctx.err(format!("needs.ron: unknown need `{}` (the engine knows: hunger, energy, hygiene, fun, social, money, safety)", n.need));
        }
    }

    // ---- tags (collected from buildings; activities must reference existing ones)
    let mut tags: Vec<String> = Vec::new();
    for b in &raw.buildings {
        for t in &b.tags {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
    }
    tags.sort();

    // ---- schedules
    check_unique(
        &mut ctx,
        "schedule",
        raw.schedules.iter().map(|s| s.id.as_str()),
    );
    let schedule_ids: Vec<&str> = raw.schedules.iter().map(|s| s.id.as_str()).collect();
    let mut schedules = Vec::new();
    for s in &raw.schedules {
        let mut blocks = Vec::new();
        for b in &s.blocks {
            let days = parse_days(&b.days);
            let span = parse_span(&b.time);
            match (days, span) {
                (Some(days), Some(span)) if !span.is_empty() => {
                    blocks.push(ScheduleBlock { days, span })
                }
                _ => ctx.err(format!(
                    "schedules.ron `{}`: invalid block days=`{}` time=`{}`",
                    s.id, b.days, b.time
                )),
            }
        }
        let sleep = parse_span(&s.sleep).unwrap_or_else(|| {
            ctx.err(format!(
                "schedules.ron `{}`: invalid sleep `{}`",
                s.id, s.sleep
            ));
            DaySpan::default()
        });
        let sleep_free_day = parse_span(&s.sleep_free_day).unwrap_or_else(|| {
            ctx.err(format!(
                "schedules.ron `{}`: invalid sleep_free_day `{}`",
                s.id, s.sleep_free_day
            ));
            DaySpan::default()
        });
        schedules.push(ScheduleDef {
            id: s.id.clone(),
            name: s.name.clone(),
            blocks,
            sleep,
            sleep_free_day,
        });
    }
    for required in [
        "toddler",
        "child_school",
        "retired",
        "unemployed",
        "student_university",
    ] {
        if !schedule_ids.contains(&required) {
            ctx.err(format!(
                "schedules.ron: the engine requires a schedule `{required}`"
            ));
        }
    }

    // ---- professions
    check_unique(
        &mut ctx,
        "profession",
        raw.professions.iter().map(|p| p.id.as_str()),
    );
    let profession_ids: Vec<&str> = raw.professions.iter().map(|p| p.id.as_str()).collect();
    let mut professions = Vec::new();
    for p in &raw.professions {
        let mut sch = Vec::new();
        for s in &p.schedules {
            match find(&schedule_ids, s) {
                Some(i) => sch.push(i),
                None => ctx.err(format!(
                    "professions.ron `{}`: unknown schedule `{s}`",
                    p.id
                )),
            }
        }
        if sch.is_empty() {
            ctx.err(format!(
                "professions.ron `{}`: needs at least one schedule",
                p.id
            ));
        }
        professions.push(ProfessionDef {
            id: p.id.clone(),
            name: p.name.clone(),
            wage_per_shift_cents: p.wage_per_shift_cents,
            schedules: sch,
            min_age: p.min_age,
            education: p.education.min(3),
        });
    }

    // ---- buildings
    check_unique(
        &mut ctx,
        "building",
        raw.buildings.iter().map(|b| b.id.as_str()),
    );
    let building_ids: Vec<&str> = raw.buildings.iter().map(|b| b.id.as_str()).collect();
    let mut buildings = Vec::new();
    for b in &raw.buildings {
        let (lw, ld) = match b.lot {
            LotSize::Small => (4, 6),
            LotSize::Medium => (6, 6),
            LotSize::Large => (12, 12),
        };
        if b.size.0 == 0 || b.size.1 == 0 || b.size.0 > lw || b.size.1 > ld {
            ctx.err(format!(
                "buildings.ron `{}`: size {:?} does not fit a {:?} lot ({lw}x{ld})",
                b.id, b.size, b.lot
            ));
        }
        let mut jobs = Vec::new();
        for (prof, count) in &b.jobs {
            match find(&profession_ids, prof) {
                Some(i) => jobs.push(JobSlots {
                    profession: i,
                    count: *count,
                }),
                None => ctx.err(format!(
                    "buildings.ron `{}`: unknown profession `{prof}`",
                    b.id
                )),
            }
        }
        let open = match &b.open {
            None => None,
            Some(s) => match parse_span(s) {
                Some(sp) => Some(sp),
                None => {
                    ctx.err(format!("buildings.ron `{}`: invalid open `{s}`", b.id));
                    None
                }
            },
        };
        let open_days = parse_days(&b.open_days).unwrap_or_else(|| {
            ctx.err(format!(
                "buildings.ron `{}`: invalid open_days `{}`",
                b.id, b.open_days
            ));
            DayMask(0x7f)
        });
        let color = parse_color(&b.color).unwrap_or_else(|| {
            ctx.err(format!(
                "buildings.ron `{}`: invalid color `{}`",
                b.id, b.color
            ));
            0
        });
        let roof_color = parse_color(&b.roof_color).unwrap_or_else(|| {
            ctx.err(format!(
                "buildings.ron `{}`: invalid roof_color `{}`",
                b.id, b.roof_color
            ));
            0
        });
        let btags = b
            .tags
            .iter()
            .map(|t| tags.iter().position(|x| x == t).unwrap() as u16)
            .collect();
        let firm = match &b.firm {
            None => {
                if b.jobs.is_empty() {
                    None
                } else {
                    Some(FirmSpec {
                        kind: FirmKind::Public,
                        good: None,
                        per_shift: 0,
                        export_per_shift_cents: 0,
                        food_per_customer: 0,
                    })
                }
            }
            Some(f) => {
                let good = match &f.good {
                    None => None,
                    Some(g) => match raw.goods.iter().position(|x| &x.id == g) {
                        Some(i) => Some(i as u16),
                        None => {
                            ctx.err(format!("buildings.ron `{}`: unknown good `{g}`", b.id));
                            None
                        }
                    },
                };
                if matches!(
                    f.kind,
                    FirmKind::Shop | FirmKind::Farm | FirmKind::Factory | FirmKind::Warehouse
                ) && good.is_none()
                {
                    ctx.err(format!(
                        "buildings.ron `{}`: a {:?} firm needs a `good`",
                        b.id, f.kind
                    ));
                }
                if matches!(f.kind, FirmKind::Farm | FirmKind::Factory) && f.per_shift == 0 {
                    ctx.err(format!(
                        "buildings.ron `{}`: a {:?} firm needs `per_shift` > 0",
                        b.id, f.kind
                    ));
                }
                Some(FirmSpec {
                    kind: f.kind,
                    good,
                    per_shift: f.per_shift,
                    export_per_shift_cents: f.export_per_shift_cents,
                    food_per_customer: f.food_per_customer,
                })
            }
        };
        if b.dwellings > 0 && b.rent_cents <= 0 {
            ctx.err(format!(
                "buildings.ron `{}`: housing needs `rent_cents` > 0",
                b.id
            ));
        }
        buildings.push(BuildingDef {
            id: b.id.clone(),
            name: b.name.clone(),
            lot: b.lot,
            width: b.size.0,
            depth: b.size.1,
            floors: b.floors.max(1),
            dwellings: b.dwellings,
            jobs,
            tags: btags,
            zones: b.zones.iter().map(|(z, w)| (*z, *w)).collect(),
            open,
            open_days,
            students: b.students,
            stock: b.stock,
            capacity: b.capacity,
            firm,
            rent_cents: b.rent_cents,
            name_patterns: b.name_patterns.clone(),
            color,
            roof_color,
            roof: b.roof,
        });
    }

    // ---- activities
    check_unique(
        &mut ctx,
        "activity",
        raw.activities.iter().map(|a| a.id.as_str()),
    );
    let mut activities = Vec::new();
    for a in &raw.activities {
        let mut effects = [0i32; NEED_COUNT];
        for (need, v) in &a.effects {
            match Need::ALL.iter().position(|n| n.id() == need) {
                Some(i) => effects[i] = *v,
                None => ctx.err(format!("activities.ron `{}`: unknown need `{need}`", a.id)),
            }
        }
        let place = match &a.place {
            raw::Place::Home => Place::Home,
            raw::Place::Workplace => Place::Workplace,
            raw::Place::OtherRegion => Place::OtherRegion,
            raw::Place::FriendHome => Place::FriendHome,
            raw::Place::Tag(t) => match tags.iter().position(|x| x == t) {
                Some(i) => Place::Tag(i as u16),
                None => {
                    ctx.err(format!(
                        "activities.ron `{}`: no building has tag `{t}`",
                        a.id
                    ));
                    Place::Home
                }
            },
        };
        if a.minutes.0 == 0 || a.minutes.1 < a.minutes.0 {
            ctx.err(format!(
                "activities.ron `{}`: minutes must be (min>0, max>=min)",
                a.id
            ));
        }
        if a.ages.1 < a.ages.0 {
            ctx.err(format!(
                "activities.ron `{}`: ages must be (min, max>=min)",
                a.id
            ));
        }
        activities.push(ActivityDef {
            id: a.id.clone(),
            name: a.name.clone(),
            kind: a.kind,
            place,
            effects,
            min_minutes: a.minutes.0,
            max_minutes: a.minutes.1,
            cost_cents: a.cost_cents,
            bias: a.bias,
            min_age: a.ages.0,
            max_age: a.ages.1,
            doing: a.doing.clone(),
        });
    }
    for kind in [
        ActivityKind::Sleep,
        ActivityKind::Work,
        ActivityKind::School,
        ActivityKind::Relax,
    ] {
        if !activities.iter().any(|a| a.kind == kind) {
            ctx.err(format!(
                "activities.ron: the engine requires at least one activity of kind {kind:?}"
            ));
        }
    }

    // ---- names
    check_unique(
        &mut ctx,
        "name style",
        raw.name_styles.iter().map(|s| s.id.as_str()),
    );
    let mut styles = Vec::new();
    for s in &raw.name_styles {
        styles.push(NameStyle {
            id: s.id.clone(),
            female: syllables(&mut ctx, &format!("names `{}` female", s.id), &s.female),
            male: syllables(&mut ctx, &format!("names `{}` male", s.id), &s.male),
            family: syllables(&mut ctx, &format!("names `{}` family", s.id), &s.family),
            places: syllables(&mut ctx, &format!("names `{}` places", s.id), &s.places),
        });
    }
    if styles.is_empty() {
        ctx.err("names/: at least one name style is required");
    }
    let style_ids: Vec<&str> = raw.name_styles.iter().map(|s| s.id.as_str()).collect();
    let names = match &raw.names_common {
        Some(c) => NamesDef {
            styles,
            blacklist: c.blacklist.iter().map(|s| s.to_lowercase()).collect(),
            street_suffixes: c.street_suffixes.clone(),
            avenue_suffixes: c.avenue_suffixes.clone(),
        },
        None => {
            ctx.err("names/common.ron is missing");
            NamesDef {
                styles,
                blacklist: vec![],
                street_suffixes: vec![],
                avenue_suffixes: vec![],
            }
        }
    };
    if names.street_suffixes.is_empty() || names.avenue_suffixes.is_empty() {
        ctx.err("names/common.ron: street_suffixes and avenue_suffixes must not be empty");
    }

    // ---- text
    let text = match &raw.text {
        Some(t) => {
            let mut journal = Vec::new();
            for k in JournalKind::ALL {
                match t.journal.get(k.id()) {
                    Some(s) => journal.push(s.clone()),
                    None => {
                        ctx.err(format!(
                            "journal.ron: missing journal template `{}`",
                            k.id()
                        ));
                        journal.push(String::new());
                    }
                }
            }
            let mut news = Vec::new();
            for k in NewsKind::ALL {
                match t.news.get(k.id()) {
                    Some(s) => news.push(s.clone()),
                    None => {
                        ctx.err(format!("journal.ron: missing news template `{}`", k.id()));
                        news.push((String::new(), String::new()));
                    }
                }
            }
            for k in t.journal.keys() {
                if !JournalKind::ALL.iter().any(|j| j.id() == k) {
                    ctx.err(format!("journal.ron: unknown journal kind `{k}`"));
                }
            }
            TextDef { journal, news }
        }
        None => {
            ctx.err("journal.ron is missing");
            TextDef {
                journal: vec![],
                news: vec![],
            }
        }
    };

    // ---- economy
    let economy = match &raw.economy {
        Some(e) => EconomyDef {
            pension_per_day_cents: e.pension_per_day_cents,
            benefit_per_day_cents: e.benefit_per_day_cents,
            meal_price_cents: e.meal_price_cents,
            meals_per_person_bought: e.meals_per_person_bought,
            farm_food_per_day: e.farm_food_per_day,
            starting_money_min_cents: e.starting_money_cents.0,
            starting_money_max_cents: e.starting_money_cents.1.max(e.starting_money_cents.0),
            income_tax_permille: e.income_tax_permille,
            sales_tax_permille: e.sales_tax_permille,
            min_income_tax_permille: e.income_tax_bounds_permille.0,
            max_income_tax_permille: e
                .income_tax_bounds_permille
                .1
                .max(e.income_tax_bounds_permille.0),
            property_tax_cents: e.property_tax_cents,
            farm_price_cents: e.farm_price_cents.max(1),
            wholesale_markup_permille: e.wholesale_markup_permille,
            import_price_permille: e.import_price_permille,
            export_price_permille: e.export_price_permille,
            import_cap_per_day: e.import_cap_per_day,
            maintenance_per_building_cents: e.maintenance_per_building_cents,
            firm_start_cash_days: e.firm_start_cash_days,
            treasury_start_days: e.treasury_start_days,
            layoff_after_days: e.layoff_after_days.max(1),
            bankruptcy_after_days: e.bankruptcy_after_days.max(e.layoff_after_days + 1),
            reopen_after_days: e.reopen_after_days,
            hires_per_day: e.hires_per_day,
            price_step_permille: e.price_step_permille,
            min_price_permille: e.price_bounds_permille.0.max(1),
            max_price_permille: e.price_bounds_permille.1.max(e.price_bounds_permille.0),
        },
        None => {
            ctx.err("economy.ron is missing");
            return Err(ContentError {
                problems: ctx.problems,
            });
        }
    };

    // ---- world
    let world = match &raw.world {
        Some(w) => {
            check_unique(
                &mut ctx,
                "region template",
                w.templates.iter().map(|t| t.id.as_str()),
            );
            let mut templates = Vec::new();
            for t in &w.templates {
                let mut required = Vec::new();
                for (b, n) in &t.required {
                    match find(&building_ids, b) {
                        Some(i) => required.push((i, *n)),
                        None => ctx.err(format!(
                            "world.ron template `{}`: unknown building `{b}`",
                            t.id
                        )),
                    }
                }
                if t.blocks < 2 || t.blocks > 64 {
                    ctx.err(format!(
                        "world.ron template `{}`: blocks must be in 2..=64",
                        t.id
                    ));
                }
                if t.rings.is_empty() {
                    ctx.err(format!(
                        "world.ron template `{}`: rings must not be empty",
                        t.id
                    ));
                }
                templates.push(RegionTemplate {
                    id: t.id.clone(),
                    blocks: t.blocks,
                    target_population: t.target_population,
                    rings: t.rings.clone(),
                    park_permille: t.park_permille,
                    has_river: t.has_river,
                    required,
                    max_blocks: t.max_blocks.max(t.blocks),
                });
            }
            let template_ids: Vec<&str> = w.templates.iter().map(|t| t.id.as_str()).collect();
            let mut initial = Vec::new();
            for (t, s) in &w.initial_regions {
                match (find(&template_ids, t), find(&style_ids, s)) {
                    (Some(t), Some(s)) => initial.push((t, s)),
                    _ => ctx.err(format!(
                        "world.ron: unknown template/style in initial region ({t}, {s})"
                    )),
                }
            }
            if initial.is_empty() {
                ctx.err("world.ron: at least one initial region is required");
            }
            let e = &w.expansion;
            let template = match find(&template_ids, &e.template) {
                Some(t) => t,
                None => {
                    ctx.err(format!(
                        "world.ron expansion: unknown template `{}`",
                        e.template
                    ));
                    0
                }
            };
            WorldDef {
                initial_regions: initial,
                highway_tiles: w.highway_tiles,
                templates,
                expansion: ExpansionDef {
                    template,
                    max_regions: e.max_regions.max(1),
                    min_world_age_days: e.min_world_age_days,
                    cooldown_days: e.cooldown_days,
                    threshold: e.threshold.max(1),
                    points_per_emigrant: e.points_per_emigrant,
                    points_per_waiting_household: e.points_per_waiting_household,
                    points_per_jobseeker_permille: e.points_per_jobseeker_permille,
                    long_term_jobseeker_days: e.long_term_jobseeker_days,
                    settler_households: e.settler_households,
                    full_land_multiplier_permille: e.full_land_multiplier_permille,
                    full_land_free_lots: e.full_land_free_lots,
                },
            }
        }
        None => {
            ctx.err("world.ron is missing");
            WorldDef {
                initial_regions: vec![],
                highway_tiles: 0,
                templates: vec![],
                expansion: ExpansionDef::default(),
            }
        }
    };

    // ---- later phases
    check_unique(&mut ctx, "good", raw.goods.iter().map(|g| g.id.as_str()));
    let goods = raw
        .goods
        .iter()
        .map(|g| GoodDef {
            id: g.id.clone(),
            name: g.name.clone(),
            base_price_cents: g.base_price_cents,
        })
        .collect();
    check_unique(&mut ctx, "crime", raw.crimes.iter().map(|c| c.id.as_str()));
    let crimes = raw
        .crimes
        .iter()
        .map(|c| {
            if c.sentence_days.1 < c.sentence_days.0 {
                ctx.err(format!(
                    "crimes.ron `{}`: sentence_days must be (min, max>=min)",
                    c.id
                ));
            }
            if c.loss_cents.1 < c.loss_cents.0 || c.hours.0 > 23 || c.hours.1 > 24 {
                ctx.err(format!(
                    "crimes.ron `{}`: loss_cents must be (min, max>=min) and hours within 0..=24",
                    c.id
                ));
            }
            CrimeDef {
                id: c.id.clone(),
                name: c.name.clone(),
                kind: c.kind,
                severity: c.severity.clamp(1, 10),
                min_sentence_days: c.sentence_days.0,
                max_sentence_days: c.sentence_days.1,
                rate_per_100m_days: c.rate_per_100m_days,
                loss_cents: c.loss_cents,
                evidence: c.evidence.max(1),
                clearance_permille: c.clearance_permille.min(1000),
                cold_after_days: c.cold_after_days.max(1),
                hours: c.hours,
            }
        })
        .collect();
    check_unique(
        &mut ctx,
        "technology",
        raw.technologies.iter().map(|t| t.id.as_str()),
    );
    let tech_ids: Vec<&str> = raw.technologies.iter().map(|t| t.id.as_str()).collect();
    let technologies = raw
        .technologies
        .iter()
        .map(|t| TechDef {
            id: t.id.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            effects: t.effects.clone(),
            cost_points: t.cost_points.max(1),
            requires: t
                .requires
                .iter()
                .filter_map(|r| {
                    let i = find(&tech_ids, r);
                    if i.is_none() {
                        ctx.err(format!(
                            "technologies.ron `{}`: unknown requirement `{r}`",
                            t.id
                        ));
                    }
                    i
                })
                .collect(),
            unlocks: t
                .unlocks
                .iter()
                .filter_map(|b| {
                    let i = find(&building_ids, b);
                    if i.is_none() {
                        ctx.err(format!(
                            "technologies.ron `{}`: unknown building `{b}`",
                            t.id
                        ));
                    }
                    i
                })
                .collect(),
        })
        .collect();

    let demography = match &raw.demography {
        Some(d) => {
            if d.mortality_by_decade.is_empty()
                || d.illness_by_decade.is_empty()
                || d.severe_by_decade.is_empty()
            {
                ctx.err("demography.ron: rates by decade must not be empty");
            }
            if d.fertility_by_age.is_empty() {
                ctx.err("demography.ron: fertility_by_age must not be empty");
            }
            d.clone()
        }
        None => {
            ctx.err("demography.ron is missing");
            return Err(ContentError {
                problems: ctx.problems,
            });
        }
    };

    let construction = match &raw.construction {
        Some(k) => {
            let one = |ctx: &mut Ctx, id: &str| match find(&building_ids, id) {
                Some(i) => i,
                None => {
                    ctx.err(format!("construction.ron: unknown building `{id}`"));
                    0
                }
            };
            let many = |ctx: &mut Ctx, ids: &[String]| {
                ids.iter().map(|id| one(ctx, id)).collect::<Vec<u16>>()
            };
            ConstructionDef {
                labour_per_unit_cents: k.labour_per_unit_cents,
                materials_per_unit_cents: k.materials_per_unit_cents,
                foundation_per_tile: k.foundation_per_tile.max(1),
                structure_per_tile_floor: k.structure_per_tile_floor.max(1),
                roof_per_tile: k.roof_per_tile.max(1),
                road_per_tile: k.road_per_tile.max(1),
                contractor_units_per_day: k.contractor_units_per_day.max(1),
                crew_size: k.crew_size.max(1),
                projects_per_firm: k.projects_per_firm.max(1),
                new_projects_per_week: k.new_projects_per_week,
                housing_waiting_threshold: k.housing_waiting_threshold,
                shop_shortage_threshold: k.shop_shortage_threshold,
                unemployment_threshold: k.unemployment_threshold,
                daycare_waiting_threshold: k.daycare_waiting_threshold,
                school_waiting_threshold: k.school_waiting_threshold,
                small_housing: many(&mut ctx, &k.small_housing),
                medium_housing: many(&mut ctx, &k.medium_housing),
                shops: many(&mut ctx, &k.shops),
                workplaces: many(&mut ctx, &k.workplaces),
                daycare: one(&mut ctx, &k.daycare),
                school: one(&mut ctx, &k.school),
                cemetery: one(&mut ctx, &k.cemetery),
                site: one(&mut ctx, &k.site),
            }
        }
        None => {
            ctx.err("construction.ron is missing");
            return Err(ContentError {
                problems: ctx.problems,
            });
        }
    };

    let research = match &raw.research {
        Some(r) => ResearchDef {
            points_per_shift: r
                .points_per_shift
                .iter()
                .filter_map(|(p, n)| match find(&profession_ids, p) {
                    Some(i) => Some((i, *n)),
                    None => {
                        ctx.err(format!("research.ron: unknown profession `{p}`"));
                        None
                    }
                })
                .collect(),
            adoption_days: r.adoption_days,
        },
        None => {
            ctx.err("research.ron is missing");
            ResearchDef::default()
        }
    };
    let Some(justice) = raw.justice.clone() else {
        ctx.err("justice.ron is missing");
        return Err(ContentError {
            problems: ctx.problems,
        });
    };

    if ctx.problems.is_empty() {
        Ok(Content {
            needs,
            activities,
            schedules,
            professions,
            buildings,
            tags,
            names,
            text,
            economy,
            world,
            goods,
            crimes,
            technologies,
            demography,
            construction,
            justice,
            research,
        })
    } else {
        Err(ContentError {
            problems: ctx.problems,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_times_and_days() {
        assert_eq!(parse_time("09:30"), Some(570));
        assert_eq!(parse_time("24:00"), Some(0));
        assert_eq!(parse_time("25:00"), None);
        let s = parse_span("22:00-06:00").unwrap();
        assert!(s.crosses_midnight());
        assert_eq!(s.len(), 480);
        assert_eq!(parse_days("Mon-Fri"), Some(DayMask(0x1f)));
        assert_eq!(parse_days("Sat-Sun"), Some(DayMask(0x60)));
        assert_eq!(parse_days("mon,wed"), Some(DayMask(0b101)));
        assert_eq!(parse_days("Fri-Mon"), None);
    }
}
