//! Buildings (structure of arrays).

use crate::names::NameRef;
use crate::roads::NONE;
use serde::{Deserialize, Serialize};

pub type BuildingId = u32;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Buildings {
    /// Index into `content.buildings`.
    pub def: Vec<u16>,
    /// Footprint (tiles).
    pub x: Vec<u16>,
    pub y: Vec<u16>,
    pub w: Vec<u8>,
    pub d: Vec<u8>,
    /// Lot rectangle (tiles), used for gardens, parks and fields.
    pub lot: Vec<[u16; 4]>,
    /// The road tile in front of the entrance.
    pub door_x: Vec<u16>,
    pub door_y: Vec<u16>,
    /// Road node the entrance connects to, and the distance to it along the road.
    pub access_node: Vec<u32>,
    pub access_tiles: Vec<u16>,
    pub name: Vec<NameRef>,
    pub street: Vec<u16>,
    /// Goods in stock (shops), in meals.
    pub stock: Vec<u32>,
    /// Daily counters for news and stats (reset every morning).
    pub visits_today: Vec<u16>,
    pub empty_today: Vec<u16>,
    /// Buildings per tag, in id order. Derived; rebuilt after loading.
    #[serde(skip)]
    pub by_tag: Vec<Vec<BuildingId>>,
    /// Agents currently inside each building. Derived; rebuilt after loading.
    #[serde(skip)]
    pub present: Vec<u16>,
}

impl Buildings {
    pub fn len(&self) -> usize {
        self.def.len()
    }
    pub fn is_empty(&self) -> bool {
        self.def.is_empty()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        def: u16,
        x: u16,
        y: u16,
        w: u8,
        d: u8,
        lot: [u16; 4],
        door: (u16, u16),
    ) -> BuildingId {
        self.def.push(def);
        self.x.push(x);
        self.y.push(y);
        self.w.push(w);
        self.d.push(d);
        self.lot.push(lot);
        self.door_x.push(door.0);
        self.door_y.push(door.1);
        self.access_node.push(NONE);
        self.access_tiles.push(0);
        self.name.push(crate::names::NO_NAME);
        self.street.push(0);
        self.stock.push(0);
        self.visits_today.push(0);
        self.empty_today.push(0);
        (self.def.len() - 1) as BuildingId
    }
    pub fn rebuild_index(&mut self, tag_count: usize) {
        let content = crate::catalog::content();
        self.by_tag = vec![Vec::new(); tag_count];
        for (i, def) in self.def.iter().enumerate() {
            for t in &content.buildings[*def as usize].tags {
                self.by_tag[*t as usize].push(i as BuildingId);
            }
        }
    }
    pub fn with_tag(&self, tag: u16) -> &[BuildingId] {
        self.by_tag
            .get(tag as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    /// Centre of the footprint in tiles ×2 (integer half-tile precision).
    pub fn centre2(&self, b: BuildingId) -> (i32, i32) {
        let b = b as usize;
        (
            self.x[b] as i32 * 2 + self.w[b] as i32,
            self.y[b] as i32 * 2 + self.d[b] as i32,
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Households {
    pub home: Vec<BuildingId>,
    pub members: Vec<u8>,
    /// Meals in the fridge.
    pub food: Vec<u16>,
    pub has_car: Vec<bool>,
}

impl Households {
    pub fn len(&self) -> usize {
        self.home.len()
    }
    pub fn is_empty(&self) -> bool {
        self.home.is_empty()
    }
    pub fn push(&mut self, home: BuildingId, has_car: bool) -> u32 {
        self.home.push(home);
        self.members.push(0);
        self.food.push(0);
        self.has_car.push(has_car);
        (self.home.len() - 1) as u32
    }
}
