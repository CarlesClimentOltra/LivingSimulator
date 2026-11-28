//! Procedural generation of a region: terrain, roads, zoning, buildings and population.
//! Everything derives from the seed; the same seed always gives the same town.

use crate::agents::{flags, ACT_FREE_SLOT};
use crate::buildings::BuildingId;
use crate::catalog::{catalog, content};
use crate::map::{TileKind, TileMap, MARGIN, PITCH};
use crate::names::generate;
use crate::region::{Gateway, NewsItem, Region};
use crate::rng::{keyed, Rng};
use crate::roads::{RoadClass, NONE};
use crate::time::{Tick, TICKS_PER_YEAR};
use livingsim_content::{LotSize, NewsKind, RoofKind, Zone};

/// Which side of the map a highway leaves from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    East,
    West,
    North,
    South,
}

impl Side {
    pub fn code(self) -> u8 {
        match self {
            Side::East => 0,
            Side::West => 1,
            Side::North => 2,
            Side::South => 3,
        }
    }
    pub fn from_code(c: u8) -> Side {
        match c {
            0 => Side::East,
            1 => Side::West,
            2 => Side::North,
            _ => Side::South,
        }
    }
    pub fn opposite(self) -> Side {
        match self {
            Side::East => Side::West,
            Side::West => Side::East,
            Side::North => Side::South,
            Side::South => Side::North,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GatewaySpec {
    pub side: Side,
    pub to_region: u16,
    pub to_name: String,
    pub highway_ticks: Tick,
}

/// The name a region will get (computed before generation so neighbours can know it).
pub fn region_name(seed: u64, id: u16, style: u16) -> String {
    let c = content();
    generate(
        &c.names,
        &c.names.styles[style as usize].places,
        keyed(seed, id as u64, 0x5E61, 0),
    )
}

pub fn region_size(template: u16) -> u16 {
    let b = content().world.templates[template as usize].blocks;
    2 * MARGIN + b * PITCH + 1
}

#[derive(Clone)]
struct BlockPlan {
    zone: Option<Zone>,
    lot: Option<LotSize>,
    lots: Vec<Option<u16>>,
}

fn lots_in(size: LotSize) -> usize {
    match size {
        LotSize::Small => 6,
        LotSize::Medium => 4,
        LotSize::Large => 1,
    }
}

pub fn generate_region(
    id: u16,
    template: u16,
    style: u16,
    seed: u64,
    start: Tick,
    gateways: &[GatewaySpec],
) -> Region {
    let c = content();
    let tpl = &c.world.templates[template as usize];
    let nb = tpl.blocks as i32;
    let size = region_size(template);
    let mut rng = Rng::new(seed, 0x1000 + id as u64);
    let mut r = Region::empty(
        id,
        seed ^ (id as u64).wrapping_mul(0x9E37_79B9),
        template,
        style,
        TileMap::new(size, size),
        start,
    );
    let line = |i: i32| (MARGIN as i32 + i * PITCH as i32) as u16;
    let cb = nb / 2;
    let ring = |bx: i32, by: i32| (bx - cb).abs().max((by - cb).abs()) as u16;
    let is_avenue = |i: i32| (i - cb).rem_euclid(5) == 0;

    // ---- names
    let style_def = &c.names.styles[style as usize];
    let rname = region_name(seed, id, style);
    r.name = r.names.intern(&rname);
    for i in 0..=nb {
        let base = generate(&c.names, &style_def.places, rng.next_u64());
        let suffix = if is_avenue(i) {
            rng.pick(&c.names.avenue_suffixes)
        } else {
            rng.pick(&c.names.street_suffixes)
        };
        let s = format!("{base} {suffix}");
        r.streets.push(r.names.intern(&s));
    }
    for i in 0..=nb {
        let base = generate(&c.names, &style_def.places, rng.next_u64());
        let suffix = if is_avenue(i) {
            rng.pick(&c.names.avenue_suffixes)
        } else {
            rng.pick(&c.names.street_suffixes)
        };
        let s = format!("{base} {suffix}");
        r.streets.push(r.names.intern(&s));
    }

    // ---- zoning
    let river = if tpl.has_river && nb >= 8 {
        Some(if rng.permille(500) { cb + 3 } else { cb - 4 })
    } else {
        None
    };
    let industrial_quadrant = (
        if rng.permille(500) { 1 } else { -1 },
        if rng.permille(500) { 1 } else { -1 },
    );
    let mut plans: Vec<BlockPlan> = Vec::new();
    for by in 0..nb {
        for bx in 0..nb {
            let d = ring(bx, by);
            let mut zone = tpl
                .rings
                .iter()
                .find(|(max, _)| d <= *max)
                .map(|(_, z)| *z)
                .unwrap_or(Zone::Rural);
            if Some(bx) == river {
                plans.push(BlockPlan {
                    zone: None,
                    lot: None,
                    lots: vec![],
                });
                continue;
            }
            if nb >= 12
                && (4..=8).contains(&d)
                && (bx - cb) * industrial_quadrant.0 > 1
                && (by - cb) * industrial_quadrant.1 > 1
            {
                zone = Zone::Industrial;
            }
            if d > 0
                && zone != Zone::Industrial
                && zone != Zone::Rural
                && rng.permille(tpl.park_permille as u32)
            {
                zone = Zone::Park;
            }
            plans.push(BlockPlan {
                zone: Some(zone),
                lot: None,
                lots: vec![],
            });
        }
    }
    let bidx = |bx: i32, by: i32| (by * nb + bx) as usize;

    // ---- terrain: river
    if let Some(rx) = river {
        for y in 0..size {
            for x in line(rx) + 1..line(rx + 1) {
                r.map.set(x, y, TileKind::Water);
            }
        }
    }

    // ---- road graph
    let nid = |i: i32, j: i32| (j * (nb + 1) + i) as u32;
    for j in 0..=nb {
        for i in 0..=nb {
            r.roads.add_node(line(i), line(j));
        }
    }
    let mut removable: Vec<(u32, u32, u32)> = Vec::new(); // (a, b, permille)
    for j in 0..=nb {
        for i in 0..=nb {
            if i < nb {
                // horizontal edge along line j, spanning block column i
                let over_river = Some(i) == river;
                if !over_river || is_avenue(j) {
                    let class = if is_avenue(j) {
                        RoadClass::Avenue
                    } else {
                        RoadClass::Street
                    };
                    r.roads.add_edge(nid(i, j), nid(i + 1, j), PITCH, class);
                    if !over_river && !is_avenue(j) {
                        let z = |bx: i32, by: i32| {
                            if bx < 0 || by < 0 || bx >= nb || by >= nb {
                                Some(Zone::Rural)
                            } else {
                                plans[bidx(bx, by)].zone
                            }
                        };
                        let p = match (z(i, j - 1), z(i, j)) {
                            (Some(Zone::Rural), Some(Zone::Rural)) => 300,
                            (Some(Zone::Residential), Some(Zone::Residential)) => 60,
                            _ => 0,
                        };
                        if p > 0 {
                            removable.push((nid(i, j), nid(i + 1, j), p));
                        }
                    }
                }
            }
            if j < nb {
                let class = if is_avenue(i) {
                    RoadClass::Avenue
                } else {
                    RoadClass::Street
                };
                r.roads.add_edge(nid(i, j), nid(i, j + 1), PITCH, class);
                if !is_avenue(i) {
                    let z = |bx: i32, by: i32| {
                        if bx < 0 || by < 0 || bx >= nb || by >= nb {
                            Some(Zone::Rural)
                        } else {
                            plans[bidx(bx, by)].zone
                        }
                    };
                    let p = match (z(i - 1, j), z(i, j)) {
                        (Some(Zone::Rural), Some(Zone::Rural)) => 300,
                        (Some(Zone::Residential), Some(Zone::Residential)) => 60,
                        _ => 0,
                    };
                    if p > 0 && Some(i) != river && Some(i - 1) != river {
                        removable.push((nid(i, j), nid(i, j + 1), p));
                    }
                }
            }
        }
    }
    r.roads.finalize();
    for (a, b, p) in removable {
        if !rng.permille(p) {
            continue;
        }
        let Some(e) = r.roads.find_edge(a, b) else {
            continue;
        };
        let class = r.roads.edge_class[e as usize];
        let len = r.roads.edge_len[e as usize];
        r.roads.remove_edge(e);
        if !r.roads.is_connected() {
            r.roads.add_edge(a, b, len, class);
            r.roads.finalize();
        }
    }
    // Highways towards neighbours.
    let mut gateway_nodes = Vec::new();
    for g in gateways {
        let (from, gx, gy) = match g.side {
            Side::East => (nid(nb, cb), size - 1, line(cb)),
            Side::West => (nid(0, cb), 0, line(cb)),
            Side::North => (nid(cb, 0), line(cb), 0),
            Side::South => (nid(cb, nb), line(cb), size - 1),
        };
        let n = r.roads.add_node(gx, gy);
        let len = (gx as i32 - r.roads.node_x[from as usize] as i32).unsigned_abs() as u16
            + (gy as i32 - r.roads.node_y[from as usize] as i32).unsigned_abs() as u16;
        r.roads.add_edge(from, n, len, RoadClass::Highway);
        gateway_nodes.push(n);
    }
    r.roads.finalize();

    // ---- paint roads
    for e in 0..r.roads.edge_count() {
        let (a, b) = (r.roads.edge_a[e] as usize, r.roads.edge_b[e] as usize);
        let (x0, y0, x1, y1) = (
            r.roads.node_x[a],
            r.roads.node_y[a],
            r.roads.node_x[b],
            r.roads.node_y[b],
        );
        let kind = match r.roads.edge_class[e] {
            RoadClass::Street => TileKind::Street,
            RoadClass::Avenue => TileKind::Avenue,
            RoadClass::Highway => TileKind::Highway,
        };
        for y in y0.min(y1)..=y0.max(y1) {
            for x in x0.min(x1)..=x0.max(x1) {
                let k = if r.map.get(x, y) == TileKind::Water {
                    TileKind::Bridge
                } else {
                    kind
                };
                let cur = r.map.get(x, y);
                // Avenues win over streets at crossings.
                if cur == TileKind::Avenue && k == TileKind::Street {
                    continue;
                }
                r.map.set(x, y, k);
            }
        }
    }

    // ---- required buildings
    let max_urban_ring = tpl
        .rings
        .iter()
        .filter(|(_, z)| *z != Zone::Rural)
        .map(|(d, _)| *d)
        .max()
        .unwrap_or(nb as u16);
    let mut placed: Vec<(u16, i32, i32)> = Vec::new();
    for &(def, count) in &tpl.required {
        let bdef = &c.buildings[def as usize];
        let tag_is = |t: &str| c.tag(t).is_some_and(|x| bdef.tags.contains(&x));
        for _ in 0..count {
            let mut best: Option<(i64, usize)> = None;
            for by in 0..nb {
                for bx in 0..nb {
                    let p = &plans[bidx(bx, by)];
                    let Some(zone) = p.zone else { continue };
                    let d = ring(bx, by) as i64;
                    let free_lot = match p.lot {
                        None => true,
                        Some(sz) => {
                            sz == bdef.lot
                                && p.lots.iter().any(|l| l.is_none())
                                && !placed
                                    .iter()
                                    .any(|(pd, x, y)| *pd == def && *x == bx && *y == by)
                        }
                    };
                    if !free_lot {
                        continue;
                    }
                    let ok_zone = if !bdef.zones.is_empty() {
                        bdef.zones.iter().any(|(z, w)| *z == zone && *w > 0)
                    } else if tag_is("park") {
                        zone != Zone::Industrial
                    } else {
                        zone != Zone::Park
                            && (zone != Zone::Rural || max_urban_ring as i32 >= nb)
                            && (zone != Zone::Rural || d as u16 <= max_urban_ring + 1)
                    };
                    let ok_zone = ok_zone
                        || (bdef.zones.is_empty()
                            && zone == Zone::Rural
                            && d as u16 <= max_urban_ring + 1);
                    if !ok_zone {
                        continue;
                    }
                    let spread = placed
                        .iter()
                        .filter(|(pd, _, _)| *pd == def)
                        .map(|(_, x, y)| ((x - bx).abs().max((y - by).abs())) as i64)
                        .min()
                        .unwrap_or(99);
                    let noise = (rng.next_u64() % 3) as i64;
                    let score = if tag_is("town_hall") || tag_is("cinema") || tag_is("office") {
                        -d * 10 + noise
                    } else if tag_is("jail") {
                        (if zone == Zone::Industrial { 500 } else { 0 }) + d * 10 + noise
                    } else if tag_is("police")
                        || tag_is("fire_station")
                        || tag_is("hospital")
                        || tag_is("school")
                        || tag_is("park")
                    {
                        spread.min(12) * 20 - (d - 4).abs() * 6 + noise
                    } else {
                        spread.min(8) * 10 + noise
                    };
                    if best.is_none_or(|(s, _)| score > s) {
                        best = Some((score, bidx(bx, by)));
                    }
                }
            }
            let Some((_, bi)) = best else { continue };
            let p = &mut plans[bi];
            if p.lot.is_none() {
                p.lot = Some(bdef.lot);
                p.lots = vec![None; lots_in(bdef.lot)];
            }
            let slot = p.lots.iter().position(|l| l.is_none()).unwrap();
            p.lots[slot] = Some(def);
            placed.push((def, bi as i32 % nb, bi as i32 / nb));
        }
    }

    // ---- fill the remaining lots by zone weights
    let gateway_def = catalog().gateway_building;
    let zone_defs = |zone: Zone, lot: Option<LotSize>| -> (Vec<u16>, Vec<u32>) {
        let mut ids = Vec::new();
        let mut ws = Vec::new();
        for (i, b) in c.buildings.iter().enumerate() {
            if Some(i as u16) == gateway_def || lot.is_some_and(|l| l != b.lot) {
                continue;
            }
            if let Some((_, w)) = b.zones.iter().find(|(z, _)| *z == zone) {
                ids.push(i as u16);
                ws.push(*w as u32);
            }
        }
        (ids, ws)
    };
    for p in plans.iter_mut() {
        let Some(zone) = p.zone else { continue };
        if p.lot.is_none() {
            let (ids, ws) = zone_defs(zone, None);
            let Some(k) = rng.weighted(&ws) else { continue };
            let def = ids[k];
            p.lot = Some(c.buildings[def as usize].lot);
            p.lots = vec![None; lots_in(c.buildings[def as usize].lot)];
            p.lots[0] = Some(def);
        }
        let lot = p.lot.unwrap();
        let (ids, ws) = zone_defs(zone, Some(lot));
        for l in p.lots.iter_mut() {
            if l.is_none() {
                if zone == Zone::Rural && rng.permille(450) {
                    continue;
                }
                if let Some(k) = rng.weighted(&ws) {
                    *l = Some(ids[k]);
                }
            }
        }
    }

    // ---- place buildings
    for by in 0..nb {
        for bx in 0..nb {
            let p = plans[bidx(bx, by)].clone();
            let Some(lot) = p.lot else { continue };
            for (k, def) in p.lots.iter().enumerate() {
                let Some(def) = *def else { continue };
                let (bx0, by0) = (line(bx) + 1, line(by) + 1);
                let (lx, ly, lw, lh, north) = match lot {
                    LotSize::Small => (
                        bx0 + (k as u16 % 3) * 4,
                        by0 + (k as u16 / 3) * 6,
                        4,
                        6,
                        k < 3,
                    ),
                    LotSize::Medium => (
                        bx0 + (k as u16 % 2) * 6,
                        by0 + (k as u16 / 2) * 6,
                        6,
                        6,
                        k < 2,
                    ),
                    LotSize::Large => (bx0, by0, 12, 12, true),
                };
                place_building(
                    &mut r,
                    &mut rng,
                    def,
                    [lx, ly, lw, lh],
                    north,
                    (bx, by),
                    nb,
                    &line,
                    style,
                );
            }
        }
    }

    // ---- gateways
    let _ = gateway_def.expect("data defines highway_exit");
    for (g, &node) in gateways.iter().zip(&gateway_nodes) {
        place_gateway(&mut r, &mut rng, g, node, cb as u16, style);
    }
    r.buildings.rebuild_index(c.tags.len());

    // Shops start with full stock.
    for b in 0..r.buildings.len() {
        r.buildings.stock[b] = c.buildings[r.buildings.def[b] as usize].stock;
    }

    populate(&mut r, &mut rng, start);
    r.bootstrap_economy();
    r.bootstrap_demography();
    r.bootstrap_construction();
    let pop = r.population();
    r.news.push(NewsItem {
        tick: start,
        kind: NewsKind::CityFounded as u16,
        subject: id as u32,
        n: pop,
    });
    r.prepare();
    r
}

#[allow(clippy::too_many_arguments)]
fn place_building(
    r: &mut Region,
    rng: &mut Rng,
    def: u16,
    lot: [u16; 4],
    north: bool,
    (bx, by): (i32, i32),
    nb: i32,
    line: &dyn Fn(i32) -> u16,
    style: u16,
) {
    let c = content();
    let d = &c.buildings[def as usize];
    let [lx, ly, lw, lh] = lot;
    let (fw, fd) = (d.width as u16, d.depth as u16);
    let fx = lx + (lw - fw) / 2;
    let fy = if north {
        ly + if lh > 6 { 1 } else { 0 }
    } else {
        ly + lh - fd
    };
    let road_y = if north { line(by) } else { line(by + 1) };
    let door = (fx + fw / 2, road_y);
    let b = r.buildings.push(def, fx, fy, d.width, d.depth, lot, door);
    r.buildings.street[b as usize] = if north { by as u16 } else { by as u16 + 1 };
    // Tiles.
    let is_park = d.roof == RoofKind::None;
    let farm = catalog().tags.farm.is_some_and(|t| d.tags.contains(&t));
    for y in ly..ly + lh {
        for x in lx..lx + lw {
            let i = r.map.idx(x, y);
            let in_fp = x >= fx && x < fx + fw && y >= fy && y < fy + fd;
            let k = if is_park {
                if catalog().tags.park.is_some_and(|t| d.tags.contains(&t)) && d.id == "plaza" {
                    TileKind::Plaza
                } else if farm {
                    TileKind::Field
                } else {
                    TileKind::Park
                }
            } else if farm && !in_fp {
                TileKind::Field
            } else {
                TileKind::Lot
            };
            r.map.kind[i] = k as u8;
            if in_fp || is_park {
                r.map.building[i] = b;
            }
        }
    }
    // Access: nearest end of the frontage street if it exists, else nearest connected corner.
    let nid = |i: i32, j: i32| (j * (nb + 1) + i) as u32;
    let j = if north { by } else { by + 1 };
    let (na, nbn) = (nid(bx, j), nid(bx + 1, j));
    let (access, tiles) = if r.roads.find_edge(na, nbn).is_some() {
        let da = (door.0 as i32 - r.roads.node_x[na as usize] as i32).unsigned_abs();
        let db = (door.0 as i32 - r.roads.node_x[nbn as usize] as i32).unsigned_abs();
        if da <= db {
            (na, da)
        } else {
            (nbn, db)
        }
    } else {
        let mut best = (NONE, u32::MAX);
        for (i, jj) in [(bx, by), (bx + 1, by), (bx, by + 1), (bx + 1, by + 1)] {
            let n = nid(i, jj);
            if r.roads.neighbours(n).is_empty() {
                continue;
            }
            let dd = (door.0 as i32 - r.roads.node_x[n as usize] as i32).unsigned_abs()
                + (door.1 as i32 - r.roads.node_y[n as usize] as i32).unsigned_abs();
            if dd < best.1 {
                best = (n, dd);
            }
        }
        best
    };
    r.buildings.access_node[b as usize] = access;
    r.buildings.access_tiles[b as usize] = tiles as u16;
    name_building(r, rng, b, style);
}

/// The highway exit building next to gateway node `node`, and the gateway record.
pub(crate) fn place_gateway(
    r: &mut Region,
    rng: &mut Rng,
    g: &GatewaySpec,
    node: u32,
    street: u16,
    style: u16,
) {
    let c = content();
    let def = c
        .building("highway_exit")
        .expect("data defines highway_exit");
    let gx = r.roads.node_x[node as usize];
    let gy = r.roads.node_y[node as usize];
    // A 2x2 lot beside the highway, a little inside the map.
    let (fx, fy, door) = match g.side {
        Side::East => (gx - 3, gy + 1, (gx - 2, gy)),
        Side::West => (gx + 2, gy + 1, (gx + 3, gy)),
        Side::North => (gx + 1, gy + 2, (gx, gy + 3)),
        Side::South => (gx + 1, gy - 3, (gx, gy - 2)),
    };
    let b = r.buildings.push(def, fx, fy, 2, 2, [fx, fy, 2, 2], door);
    r.buildings.access_node[b as usize] = node;
    r.buildings.access_tiles[b as usize] = 2;
    r.buildings.street[b as usize] = street;
    name_building(r, rng, b, style);
    for y in fy..fy + 2 {
        for x in fx..fx + 2 {
            let i = r.map.idx(x, y);
            r.map.kind[i] = TileKind::Lot as u8;
            r.map.building[i] = b;
        }
    }
    r.gateways.push(Gateway {
        building: b,
        node,
        to_region: g.to_region,
        to_name: g.to_name.clone(),
        highway_ticks: g.highway_ticks,
    });
}

pub(crate) fn name_building(r: &mut Region, rng: &mut Rng, b: BuildingId, style: u16) {
    let c = content();
    let d = &c.buildings[r.buildings.def[b as usize] as usize];
    if d.name_patterns.is_empty() {
        return;
    }
    let st = &c.names.styles[style as usize];
    let pattern = rng.pick(&d.name_patterns).clone();
    let mut s = pattern;
    if s.contains("{family}") {
        s = s.replace("{family}", &generate(&c.names, &st.family, rng.next_u64()));
    }
    if s.contains("{given}") {
        let set = if rng.permille(500) {
            &st.female
        } else {
            &st.male
        };
        s = s.replace("{given}", &generate(&c.names, set, rng.next_u64()));
    }
    if s.contains("{place}") {
        s = s.replace("{place}", &generate(&c.names, &st.places, rng.next_u64()));
    }
    r.buildings.name[b as usize] = r.names.intern(&s);
}

// ---------------------------------------------------------------------- population

fn adult_age(rng: &mut Rng, lo: i64, hi: i64) -> i64 {
    (rng.range(lo, hi) + rng.range(lo, hi)) / 2
}

fn new_agent(
    r: &mut Region,
    rng: &mut Rng,
    hh: u32,
    female: bool,
    age: i64,
    family: crate::names::NameRef,
    start: Tick,
) -> u32 {
    let c = content();
    let st = &c.names.styles[r.style as usize];
    let a = r.agents.alloc();
    let i = a as usize;
    let given = generate(
        &c.names,
        if female { &st.female } else { &st.male },
        rng.next_u64(),
    );
    let given = r.names.intern(&given);
    let econ = &c.economy;
    let ag = &mut r.agents;
    ag.flags[i] = flags::ALIVE | if female { flags::FEMALE } else { 0 };
    ag.given[i] = given;
    ag.family[i] = family;
    ag.birth[i] =
        start as i64 - age * TICKS_PER_YEAR as i64 - rng.range(0, TICKS_PER_YEAR as i64 - 1);
    ag.traits[i] = rng.next_u64() as u32;
    ag.household[i] = hh;
    ag.home[i] = r.households.home[hh as usize];
    ag.place[i] = ag.home[i];
    ag.money[i] = if age >= 18 {
        rng.range(econ.starting_money_min_cents, econ.starting_money_max_cents)
    } else {
        rng.range(0, 3000)
    };
    ag.needs[i] = [
        rng.range(5000, 8000) as u16,
        rng.range(7000, 9500) as u16,
        rng.range(5000, 8500) as u16,
        rng.range(5000, 9000) as u16,
        rng.range(5000, 9000) as u16,
        5000,
        8000,
    ];
    ag.needs_t[i] = start;
    r.households.members[hh as usize] += 1;
    a
}

fn populate(r: &mut Region, rng: &mut Rng, start: Tick) {
    let c = content();
    let cat = catalog();
    let tpl = &c.world.templates[r.template as usize];
    let st = &c.names.styles[r.style as usize];
    let mut dwellings: Vec<BuildingId> = Vec::new();
    for b in 0..r.buildings.len() {
        for _ in 0..c.buildings[r.buildings.def[b] as usize].dwellings {
            dwellings.push(b as BuildingId);
        }
    }
    rng.shuffle(&mut dwellings);
    let car_permille = if tpl.blocks >= 10 { 550 } else { 850 };
    let target = tpl.target_population;
    while r.population() < target {
        let Some(home) = dwellings.pop() else { break };
        let hh = r.households.push(home, rng.permille(car_permille));
        r.households.food[hh as usize] = rng.range(4, 18) as u16;
        let family = generate(&c.names, &st.family, rng.next_u64());
        let family = r.names.intern(&family);
        // single 27, couple 24, family 30, single parent 7, retired couple 8, retired single 4
        match rng.weighted(&[27, 24, 30, 7, 8, 4]).unwrap() {
            0 => {
                // Singles are mostly young adults.
                let age = if rng.permille(650) {
                    rng.range(19, 38)
                } else {
                    rng.range(39, 64)
                };
                let f = rng.permille(500);
                new_agent(r, rng, hh, f, age, family, start);
            }
            k @ (1 | 4) => {
                let age = if k == 4 {
                    adult_age(rng, 66, 90)
                } else {
                    rng.range(22, 65)
                };
                let a = new_agent(r, rng, hh, true, age, family, start);
                let pa = (age + rng.range(-4, 5)).clamp(19, 95);
                let b = new_agent(r, rng, hh, false, pa, family, start);
                r.agents.partner[a as usize] = b;
                r.agents.partner[b as usize] = a;
            }
            k @ (2 | 3) => {
                let age = rng.range(24, 56);
                let a = new_agent(r, rng, hh, true, age, family, start);
                let mut b = NONE;
                if k == 2 {
                    let pa = (age + rng.range(-4, 5)).clamp(20, 70);
                    b = new_agent(r, rng, hh, false, pa, family, start);
                    r.agents.partner[a as usize] = b;
                    r.agents.partner[b as usize] = a;
                }
                let kids = rng.range(1, 3);
                for _ in 0..kids {
                    // Some grown-up children (18–25) still live at home.
                    let kid_age = rng.range(0, (age - 20).clamp(0, 25));
                    let f = rng.permille(500);
                    let kid = new_agent(r, rng, hh, f, kid_age, family, start);
                    r.agents.parent_a[kid as usize] = a;
                    r.agents.parent_b[kid as usize] = b;
                }
            }
            _ => {
                let age = rng.range(66, 92);
                let f = rng.permille(580);
                new_agent(r, rng, hh, f, age, family, start);
            }
        }
    }
    // ---- jobs, schools, schedules
    let mut slots: Vec<(BuildingId, u16, u16)> = Vec::new();
    for b in 0..r.buildings.len() {
        for js in &c.buildings[r.buildings.def[b] as usize].jobs {
            let n = c.professions[js.profession as usize].schedules.len() as u16;
            for k in 0..js.count {
                slots.push((b as BuildingId, js.profession, k % n));
            }
        }
    }
    rng.shuffle(&mut slots);
    let mut school_cap: Vec<(BuildingId, u32)> = Vec::new();
    let mut uni_cap: Vec<(BuildingId, u32)> = Vec::new();
    for b in 0..r.buildings.len() {
        let d = &c.buildings[r.buildings.def[b] as usize];
        if cat.tags.school.is_some_and(|t| d.tags.contains(&t)) {
            school_cap.push((b as BuildingId, d.students as u32));
        }
        if cat.tags.university.is_some_and(|t| d.tags.contains(&t)) {
            uni_cap.push((b as BuildingId, d.students as u32));
        }
    }
    for a in 0..r.agents.len() as u32 {
        let i = a as usize;
        let age = crate::time::age_years(r.agents.birth[i], start) as i64;
        let home = r.agents.home[i];
        let sched;
        let mut work = NONE;
        let mut prof = u16::MAX;
        if age <= 5 {
            sched = cat.schedule_toddler;
        } else if age <= 17 {
            sched = cat.schedule_child;
            let best = school_cap
                .iter_mut()
                .filter(|(_, cap)| *cap > 0)
                .min_by_key(|(b, _)| (r_manhattan(r, *b, home), *b));
            if let Some((b, cap)) = best {
                *cap -= 1;
                work = *b;
            }
        } else if age >= 66 {
            sched = cat.schedule_retired;
        } else if age <= 23 && !uni_cap.is_empty() && uni_cap[0].1 > 0 && rng.permille(450) {
            sched = cat.schedule_university;
            uni_cap[0].1 -= 1;
            work = uni_cap[0].0;
        } else if rng.permille(930) && !slots.is_empty() {
            // Pick the nearest of a few random open jobs the agent qualifies for.
            let mut best: Option<(u32, usize)> = None;
            for _ in 0..12 {
                let k = rng.below(slots.len());
                let (b, p, _) = slots[k];
                if (c.professions[p as usize].min_age as i64) > age {
                    continue;
                }
                let d = r_manhattan(r, b, home);
                if best.is_none_or(|(bd, bk)| (d, k) < (bd, bk)) {
                    best = Some((d, k));
                }
            }
            if let Some((_, k)) = best {
                let (b, p, variant) = slots.swap_remove(k);
                work = b;
                prof = p;
                sched = c.professions[p as usize].schedules[variant as usize];
            } else {
                sched = cat.schedule_unemployed;
            }
        } else {
            sched = cat.schedule_unemployed;
        }
        r.agents.schedule[i] = sched;
        r.agents.work[i] = work;
        r.agents.profession[i] = prof;
        // Everybody starts at home, asleep, waking up over the next hour and a half.
        r.agents.act[i] = cat.sleep_activity;
        let wake = start + 1 + (keyed(r.seed, a as u64, 0x5747, 0) % 90) as Tick;
        r.agents.until[i] = wake;
        r.agents.next_event[i] = wake;
    }
    debug_assert!(r.agents.act.iter().all(|a| *a != ACT_FREE_SLOT));
}

fn r_manhattan(r: &Region, a: BuildingId, b: BuildingId) -> u32 {
    r.manhattan(a, b)
}
