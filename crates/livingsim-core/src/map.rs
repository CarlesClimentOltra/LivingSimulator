//! Tile map of a region. 1 tile ≈ 8 m. Chunks of [`CHUNK`]×[`CHUNK`] tiles are the unit of
//! rendering and (later) of incremental snapshots.

use serde::{Deserialize, Serialize};

pub const CHUNK: u16 = 64;
/// Distance between parallel streets, in tiles (12-tile blocks + 1 road tile).
pub const PITCH: u16 = 13;
/// Empty margin around the street grid.
pub const MARGIN: u16 = 8;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileKind {
    Grass = 0,
    Street = 1,
    Avenue = 2,
    Highway = 3,
    Water = 4,
    Bridge = 5,
    Lot = 6,
    Park = 7,
    Field = 8,
    Plaza = 9,
    /// A street being built (dirt).
    RoadWorks = 10,
    /// A fenced construction site.
    Site = 11,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileMap {
    pub width: u16,
    pub height: u16,
    /// `TileKind as u8`, row-major.
    pub kind: Vec<u8>,
    /// Building occupying the tile, or `u32::MAX`.
    pub building: Vec<u32>,
}

impl TileMap {
    pub fn new(width: u16, height: u16) -> TileMap {
        let n = width as usize * height as usize;
        TileMap {
            width,
            height,
            kind: vec![TileKind::Grass as u8; n],
            building: vec![u32::MAX; n],
        }
    }
    #[inline]
    pub fn idx(&self, x: u16, y: u16) -> usize {
        y as usize * self.width as usize + x as usize
    }
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }
    pub fn get(&self, x: u16, y: u16) -> TileKind {
        tile_kind(self.kind[self.idx(x, y)])
    }
    pub fn set(&mut self, x: u16, y: u16, k: TileKind) {
        let i = self.idx(x, y);
        self.kind[i] = k as u8;
    }
    pub fn chunks_x(&self) -> u16 {
        self.width.div_ceil(CHUNK)
    }
    pub fn chunks_y(&self) -> u16 {
        self.height.div_ceil(CHUNK)
    }
}

pub fn tile_kind(v: u8) -> TileKind {
    match v {
        1 => TileKind::Street,
        2 => TileKind::Avenue,
        3 => TileKind::Highway,
        4 => TileKind::Water,
        5 => TileKind::Bridge,
        6 => TileKind::Lot,
        7 => TileKind::Park,
        8 => TileKind::Field,
        9 => TileKind::Plaza,
        10 => TileKind::RoadWorks,
        11 => TileKind::Site,
        _ => TileKind::Grass,
    }
}
