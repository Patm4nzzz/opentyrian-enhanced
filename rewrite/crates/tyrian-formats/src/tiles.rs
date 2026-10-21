//! Tile sets `shapesX.dat` (`level_load.c:JE_loadMap`): 600
//! records `u8 blank` (+ 672 bytes 24×28 if `blank == 0`), then
//! an unread tail (520 bytes in the shipped files).

use crate::io::{Reader, Result, Writer};

pub const TILE_WIDTH: usize = 24;
pub const TILE_HEIGHT: usize = 28;
pub const TILE_SIZE: usize = TILE_WIDTH * TILE_HEIGHT;
pub const TILE_COUNT: usize = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// Raw `blank` byte (≠ 0 → empty tile, nothing follows).
    pub blank: u8,
    /// 672 pixels if not empty.
    pub pixels: Option<Vec<u8>>,
}

impl Tile {
    pub fn is_blank(&self) -> bool {
        self.blank != 0
    }

    /// `fill` of layers 2/3: true if the first half contains no 0.
    pub fn fill(&self) -> bool {
        match &self.pixels {
            Some(p) => p[..TILE_SIZE / 2].iter().all(|&v| v != 0),
            None => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileSet {
    pub tiles: Vec<Tile>,
    pub tail: Vec<u8>,
}

impl TileSet {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut tiles = Vec::with_capacity(TILE_COUNT);
        for _ in 0..TILE_COUNT {
            let blank = r.u8()?;
            let pixels = if blank == 0 { Some(r.bytes(TILE_SIZE)?.to_vec()) } else { None };
            tiles.push(Tile { blank, pixels });
        }
        Ok(Self { tiles, tail: r.rest() })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        for t in &self.tiles {
            w.u8(t.blank);
            if let Some(p) = &t.pixels {
                w.bytes(p);
            }
        }
        w.bytes(&self.tail);
        w.into_inner()
    }

    /// Tile by 1-based number (`mapSh`), `None` for 0 or outside the table.
    pub fn tile(&self, number: u16) -> Option<&Tile> {
        (number as usize).checked_sub(1).and_then(|i| self.tiles.get(i))
    }

    /// Indexed atlas: `columns` tiles per row, tile `i` (0-based) at
    /// `(i % columns, i / columns)`. Empty tiles stay at 0.
    pub fn atlas(&self, columns: usize) -> (usize, usize, Vec<u8>) {
        let rows = self.tiles.len().div_ceil(columns);
        let w = columns * TILE_WIDTH;
        let h = rows * TILE_HEIGHT;
        let mut out = vec![0u8; w * h];
        for (i, t) in self.tiles.iter().enumerate() {
            let Some(p) = &t.pixels else { continue };
            let (cx, cy) = ((i % columns) * TILE_WIDTH, (i / columns) * TILE_HEIGHT);
            for y in 0..TILE_HEIGHT {
                let dst = (cy + y) * w + cx;
                out[dst..dst + TILE_WIDTH].copy_from_slice(&p[y * TILE_WIDTH..(y + 1) * TILE_WIDTH]);
            }
        }
        (w, h, out)
    }
}
