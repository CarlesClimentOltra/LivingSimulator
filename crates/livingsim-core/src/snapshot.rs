//! Snapshot format, versioning and migrations.
//!
//! ```text
//! [ "LSIM" | format_version u16 | sim_version [u8;32] | kind u8 | region u16 | tick u32
//!   | state_hash [u8;32] ] + zstd(bincode-fixint(payload))
//! ```
//! * `format_version` changes when the serialized *layout* changes; old layouts are
//!   migrated by [`migrate`].
//! * `sim_version` identifies the engine + content build (see `catalog::sim_version`).
//! * `state_hash` is blake3 of the uncompressed payload, so it does not depend on the
//!   compressor.

use crate::messages::MessageLog;
use crate::region::Region;
use crate::world::World;
use bincode::Options;
use serde::{de::DeserializeOwned, Serialize};

pub const MAGIC: &[u8; 4] = b"LSIM";
pub const FORMAT_VERSION: u16 = 3;
pub const HEADER_LEN: usize = 4 + 2 + 32 + 1 + 2 + 4 + 32;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    World = 1,
    Region = 2,
    MessageLog = 3,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateHash(pub [u8; 32]);

impl StateHash {
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl std::fmt::Debug for StateHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.to_hex()[..16])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub format_version: u16,
    pub sim_version: [u8; 32],
    pub kind: u8,
    pub region: u16,
    pub tick: u32,
    pub hash: [u8; 32],
}

#[derive(Debug)]
pub enum SnapshotError {
    BadMagic,
    Truncated,
    WrongKind { expected: u8, found: u8 },
    UnsupportedFormat(u16),
    Decompress(String),
    Decode(String),
    HashMismatch,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SnapshotError {}

/// The bincode configuration used everywhere: fixed-width little-endian integers, so the
/// encoding never depends on the platform (`usize` is never serialized).
pub fn bincode_opts() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .allow_trailing_bytes()
}

pub fn to_bytes<T: Serialize>(v: &T) -> Vec<u8> {
    bincode_opts().serialize(v).expect("serialize")
}

pub fn from_bytes<T: DeserializeOwned>(b: &[u8]) -> Result<T, SnapshotError> {
    bincode_opts()
        .deserialize(b)
        .map_err(|e| SnapshotError::Decode(e.to_string()))
}

pub fn hash_bytes(b: &[u8]) -> StateHash {
    StateHash(*blake3::hash(b).as_bytes())
}

pub fn hash_of<T: Serialize>(v: &T) -> StateHash {
    hash_bytes(&to_bytes(v))
}

pub fn compress(b: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(b, ruzstd::encoding::CompressionLevel::Fastest)
}

pub fn decompress(b: &[u8]) -> Result<Vec<u8>, SnapshotError> {
    use std::io::Read;
    let mut dec = ruzstd::decoding::StreamingDecoder::new(b)
        .map_err(|e| SnapshotError::Decompress(e.to_string()))?;
    let mut out = Vec::new();
    dec.read_to_end(&mut out)
        .map_err(|e| SnapshotError::Decompress(e.to_string()))?;
    Ok(out)
}

fn encode(kind: Kind, region: u16, tick: u32, payload: &[u8]) -> Vec<u8> {
    let hash = hash_bytes(payload);
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len() / 4);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&crate::catalog::sim_version_bytes());
    out.push(kind as u8);
    out.extend_from_slice(&region.to_le_bytes());
    out.extend_from_slice(&tick.to_le_bytes());
    out.extend_from_slice(&hash.0);
    out.extend_from_slice(&compress(payload));
    out
}

pub fn read_header(b: &[u8]) -> Result<Header, SnapshotError> {
    if b.len() < HEADER_LEN {
        return Err(SnapshotError::Truncated);
    }
    if &b[0..4] != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    let mut sim_version = [0u8; 32];
    sim_version.copy_from_slice(&b[6..38]);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&b[45..77]);
    Ok(Header {
        format_version: u16::from_le_bytes([b[4], b[5]]),
        sim_version,
        kind: b[38],
        region: u16::from_le_bytes([b[39], b[40]]),
        tick: u32::from_le_bytes([b[41], b[42], b[43], b[44]]),
        hash,
    })
}

/// Validates, decompresses and migrates a snapshot. Returns the header and the payload in
/// the current format.
fn decode(b: &[u8], kind: Kind) -> Result<(Header, Vec<u8>), SnapshotError> {
    let h = read_header(b)?;
    if h.kind != kind as u8 {
        return Err(SnapshotError::WrongKind {
            expected: kind as u8,
            found: h.kind,
        });
    }
    let payload = decompress(&b[HEADER_LEN..])?;
    if hash_bytes(&payload).0 != h.hash {
        return Err(SnapshotError::HashMismatch);
    }
    let payload = migrate(h.format_version, kind, payload)?;
    Ok((h, payload))
}

/// Upgrades a payload from an older `format_version` to [`FORMAT_VERSION`].
///
/// When the layout changes: bump `FORMAT_VERSION`, copy the old types into a module
/// `snapshot::vN` (frozen forever), and add a step `N => vN::to_next(payload)` here. Add a
/// fixture of the old format under `tests/fixtures/` so CI proves the migration works.
pub fn migrate(from: u16, kind: Kind, payload: Vec<u8>) -> Result<Vec<u8>, SnapshotError> {
    let mut v = from;
    let mut p = payload;
    if v == 0 || v > FORMAT_VERSION {
        return Err(SnapshotError::UnsupportedFormat(from));
    }
    // Each frozen layout upgrades straight to the current one.
    if v < FORMAT_VERSION {
        p = match (v, kind) {
            (1, Kind::Region) => {
                to_bytes(&from_bytes::<crate::snapshot_v1::RegionV1>(&p)?.upgrade())
            }
            (1, Kind::World) => to_bytes(&from_bytes::<crate::snapshot_v1::WorldV1>(&p)?.upgrade()),
            (2, Kind::Region) => {
                to_bytes(&from_bytes::<crate::snapshot_v1::RegionV2>(&p)?.upgrade())
            }
            (2, Kind::World) => to_bytes(&from_bytes::<crate::snapshot_v1::WorldV2>(&p)?.upgrade()),
            // Message logs only gained new message kinds (appended).
            (_, Kind::MessageLog) => p,
            _ => return Err(SnapshotError::UnsupportedFormat(from)),
        };
        v = FORMAT_VERSION;
    }
    let _ = v;
    Ok(p)
}

pub fn encode_world(w: &World) -> Vec<u8> {
    encode(Kind::World, u16::MAX, w.tick, &to_bytes(w))
}

pub fn decode_world(b: &[u8]) -> Result<World, SnapshotError> {
    let (_, p) = decode(b, Kind::World)?;
    let mut w: World = from_bytes(&p)?;
    for r in w.regions.iter_mut() {
        r.prepare();
    }
    Ok(w)
}

pub fn encode_region(r: &Region) -> Vec<u8> {
    encode(Kind::Region, r.id, r.tick, &to_bytes(r))
}

pub fn decode_region(b: &[u8]) -> Result<Region, SnapshotError> {
    let (_, p) = decode(b, Kind::Region)?;
    let mut r: Region = from_bytes(&p)?;
    r.prepare();
    Ok(r)
}

pub fn region_hash(r: &Region) -> StateHash {
    hash_of(r)
}

pub fn encode_log(l: &MessageLog) -> Vec<u8> {
    encode(Kind::MessageLog, l.region, l.from, &to_bytes(l))
}

pub fn decode_log(b: &[u8]) -> Result<MessageLog, SnapshotError> {
    let (_, p) = decode(b, Kind::MessageLog)?;
    from_bytes(&p)
}
