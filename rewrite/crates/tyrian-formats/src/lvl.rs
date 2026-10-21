//! `tyrianN.lvl` levels (`lvllib.c:JE_analyzeLevel`, `level_load.c:JE_loadMap`).
//!
//! Offset container (spec § 0.4) whose entries come in pairs:
//! - entry `2k`   : header of level k+1, random enemies, events;
//! - entry `2k+1` : `mapSh` (big-endian) + 3 map layers (22,368 bytes);
//! - optional final entry (odd `lvlNum`): empty (ep. 1–3) or item
//!   block of the episode (ep. 4–5, see [`crate::hdt::ItemTables`]).

use crate::container::OffsetContainer;
use crate::hdt::ItemTables;
use crate::io::{FormatError, Reader, Result, Writer};
use serde::Serialize;

/// C cap of `levelEnemyMax` (`levelEnemy[40]`, tyrian2.c).
pub const LEVEL_ENEMY_MAX: usize = 40;
/// C cap of `maxEvent` (`lvlmast.h:EVENT_MAXIMUM`).
pub const EVENT_MAXIMUM: usize = 2500;
/// Sentinel added in memory after the last event (not stored).
pub const EVENT_SENTINEL_TIME: u16 = 65500;

pub const LAYER1_WIDTH: usize = 14;
pub const LAYER1_HEIGHT: usize = 300;
pub const LAYER2_WIDTH: usize = 14;
pub const LAYER2_HEIGHT: usize = 600;
pub const LAYER3_WIDTH: usize = 15;
pub const LAYER3_HEIGHT: usize = 600;
/// Size of the map entry (768 + 4,200 + 8,400 + 9,000).
pub const MAP_ENTRY_SIZE: usize = 3 * 128 * 2 + 14 * 300 + 14 * 600 + 15 * 600;

/// Dimensions (width, height) of the three layers.
pub const LAYER_DIMS: [(usize, usize); 3] =
    [(LAYER1_WIDTH, LAYER1_HEIGHT), (LAYER2_WIDTH, LAYER2_HEIGHT), (LAYER3_WIDTH, LAYER3_HEIGHT)];

/// `varz.h:JE_EventRecType`. File order: time, type, dat, dat2, dat3, dat5, dat6, dat4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Event {
    pub eventtime: u16,
    pub eventtype: u8,
    pub eventdat: i16,
    pub eventdat2: i16,
    pub eventdat3: i8,
    pub eventdat5: i8,
    pub eventdat6: i8,
    pub eventdat4: u8,
}

impl Event {
    pub const SIZE: usize = 11;

    pub fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            eventtime: r.u16()?,
            eventtype: r.u8()?,
            eventdat: r.i16()?,
            eventdat2: r.i16()?,
            eventdat3: r.i8()?,
            eventdat5: r.i8()?,
            eventdat6: r.i8()?,
            eventdat4: r.u8()?,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u16(self.eventtime);
        w.u8(self.eventtype);
        w.i16(self.eventdat);
        w.i16(self.eventdat2);
        w.i8(self.eventdat3);
        w.i8(self.eventdat5);
        w.i8(self.eventdat6);
        w.u8(self.eventdat4);
    }
}

/// Level header (start of the even entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelHeader {
    /// `char_mapFile` (read, unused by the C code).
    pub map_file: u8,
    /// `char_shapeFile`: letter of the `shapes%c.dat` file (lowercase applied at load time).
    pub shape_file: u8,
    pub map_x: u16,
    pub map_x2: u16,
    pub map_x3: u16,
}

impl LevelHeader {
    /// Name of the tile file used (`shapes%c.dat` with `tolower`).
    pub fn shapes_file_name(&self) -> String {
        format!("shapes{}.dat", (self.shape_file as char).to_ascii_lowercase())
    }
}

/// Even entry: header + random enemies + events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelEvents {
    pub header: LevelHeader,
    /// `levelEnemy[]`: all stored identifiers (the C code reads 40 at most).
    pub enemies: Vec<u16>,
    /// All stored events (the C code caps at 2,500).
    pub events: Vec<Event>,
    /// Possible bytes between the last event and the next entry.
    #[serde(skip)]
    pub tail: Vec<u8>,
}

impl LevelEvents {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let header =
            LevelHeader { map_file: r.u8()?, shape_file: r.u8()?, map_x: r.u16()?, map_x2: r.u16()?, map_x3: r.u16()? };
        let n_enemies = r.u16()? as usize;
        let enemies = (0..n_enemies).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
        let n_events = r.u16()? as usize;
        let events = (0..n_events).map(|_| Event::read(&mut r)).collect::<Result<Vec<_>>>()?;
        let tail = r.rest();
        Ok(Self { header, enemies, events, tail })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u8(self.header.map_file);
        w.u8(self.header.shape_file);
        w.u16(self.header.map_x);
        w.u16(self.header.map_x2);
        w.u16(self.header.map_x3);
        w.u16(self.enemies.len() as u16);
        w.u16_slice(&self.enemies);
        w.u16(self.events.len() as u16);
        for e in &self.events {
            e.write(&mut w);
        }
        w.bytes(&self.tail);
        w.into_inner()
    }
}

/// Odd entry: shape table + 3 layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelMap {
    /// `mapSh[layer][i]`: 1-based tile number in `shapesX.dat`, 0 = none.
    /// Only indices 0..=71 are consulted by the C code; 72..127 kept.
    #[serde(serialize_with = "serialize_map_sh")]
    pub map_sh: [[u16; 128]; 3],
    /// Layer 1: 300 rows × 14, row-major; byte = index into `map_sh[0]`.
    pub layer1: Vec<u8>,
    /// Layer 2: 600 × 14 → `map_sh[1]`.
    pub layer2: Vec<u8>,
    /// Layer 3: 600 × 15 → `map_sh[2]`.
    pub layer3: Vec<u8>,
    #[serde(skip)]
    pub tail: Vec<u8>,
}

fn serialize_map_sh<S: serde::Serializer>(v: &[[u16; 128]; 3], s: S) -> std::result::Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(3))?;
    for layer in v {
        seq.serialize_element(&layer[..])?;
    }
    seq.end()
}

impl LevelMap {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut map_sh = [[0u16; 128]; 3];
        for layer in map_sh.iter_mut() {
            for v in layer.iter_mut() {
                *v = r.u16_be()?;
            }
        }
        let layer1 = r.bytes(LAYER1_WIDTH * LAYER1_HEIGHT)?.to_vec();
        let layer2 = r.bytes(LAYER2_WIDTH * LAYER2_HEIGHT)?.to_vec();
        let layer3 = r.bytes(LAYER3_WIDTH * LAYER3_HEIGHT)?.to_vec();
        let tail = r.rest();
        Ok(Self { map_sh, layer1, layer2, layer3, tail })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        for layer in &self.map_sh {
            for &v in layer {
                w.u16_be(v);
            }
        }
        w.bytes(&self.layer1);
        w.bytes(&self.layer2);
        w.bytes(&self.layer3);
        w.bytes(&self.tail);
        w.into_inner()
    }

    /// Raw layer `n` (0..3).
    pub fn layer(&self, n: usize) -> &[u8] {
        match n {
            0 => &self.layer1,
            1 => &self.layer2,
            _ => &self.layer3,
        }
    }

    /// 1-based tile index (`shapesX.dat`) of a cell, after applying
    /// the rules of `JE_loadMap`: `None` if the cell is empty (mapSh = 0,
    /// x > 71, x = 71 on layer 2, x ≥ 70 on layer 3, or empty tile on
    /// layer 2/3 — `is_blank(tile)` tells whether the record is empty).
    pub fn resolved_tile(&self, layer: usize, cell: u8, is_blank: impl Fn(u16) -> bool) -> Option<u16> {
        let x = cell as usize;
        if x > 71 {
            return None;
        }
        let sh = self.map_sh[layer][x];
        if sh == 0 || sh > 600 {
            return None;
        }
        match layer {
            0 => Some(sh),
            1 if x == 71 || is_blank(sh) => None,
            2 if x >= 70 || is_blank(sh) => None,
            _ => Some(sh),
        }
    }
}

/// A complete level (pair of entries).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Level {
    pub events: LevelEvents,
    pub map: LevelMap,
}

/// `tyrianN.lvl` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvlFile {
    /// `lvlNum` read from the file.
    pub lvl_num: u16,
    pub levels: Vec<Level>,
    /// Entries remaining after the pairs (at most 1: empty or item block).
    pub extra: Vec<Vec<u8>>,
}

impl LvlFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let c = OffsetContainer::parse(data)?;
        let n_levels = c.entries.len() / 2;
        let mut levels = Vec::with_capacity(n_levels);
        for k in 0..n_levels {
            let events = LevelEvents::parse(&c.entries[2 * k])
                .map_err(|e| FormatError::new(format!("level {} (entry {}): {e}", k + 1, 2 * k)))?;
            let map = LevelMap::parse(&c.entries[2 * k + 1])
                .map_err(|e| FormatError::new(format!("map {} (entry {}): {e}", k + 1, 2 * k + 1)))?;
            levels.push(Level { events, map });
        }
        let extra = c.entries[2 * n_levels..].to_vec();
        Ok(Self { lvl_num: c.count, levels, extra })
    }

    pub fn to_container(&self) -> OffsetContainer {
        let mut entries = Vec::with_capacity(self.levels.len() * 2 + self.extra.len());
        for l in &self.levels {
            entries.push(l.events.write());
            entries.push(l.map.write());
        }
        entries.extend(self.extra.iter().cloned());
        OffsetContainer { count: self.lvl_num, entries }
    }

    pub fn write(&self) -> Vec<u8> {
        self.to_container().write()
    }

    /// Level by `lvlFileNum` (1-based, like `]L[`).
    pub fn level(&self, lvl_file_num: usize) -> Option<&Level> {
        lvl_file_num.checked_sub(1).and_then(|i| self.levels.get(i))
    }

    /// Item block of the episode (last non-empty entry, ep. 4–5).
    pub fn item_tables(&self) -> Option<Result<ItemTables>> {
        self.extra.last().filter(|e| !e.is_empty()).map(|e| ItemTables::parse(e))
    }
}
