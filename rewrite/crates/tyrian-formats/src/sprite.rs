//! Sprite sheets (`sprite.c`).
//!
//! - "Simple" table (`load_sprites`, `blit_sprite`): first 7 tables of
//!   `tyrian.shp`, `estsc.shp`, `estpa.shp`.
//! - Compressed "Sprite2" sheet (`JE_loadCompShapes(B)`, `blit_sprite2`):
//!   `newsh?.shp` and the last 6 tables of `tyrian.shp`. Kept raw
//!   (shared offsets, several possible RLE encodings), decoded on demand.
//! - `tyrian.shp` / `tyrianc.shp` (`JE_loadMainShapeTables`): container of 13 tables.
//! - `newsh$.shp`: Sprite2 followed by 154 encrypted `extraShips` bytes (`editship.c`).

use crate::container::OffsetContainer;
use crate::io::{FormatError, Reader, Result, Writer};

/// Decoded indexed image: palette pixels + opacity mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub opaque: Vec<bool>,
}

impl IndexedImage {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![0; width * height], opaque: vec![false; width * height] }
    }

    pub fn set(&mut self, x: usize, y: usize, v: u8) {
        if x < self.width && y < self.height {
            let i = y * self.width + x;
            self.pixels[i] = v;
            self.opaque[i] = true;
        }
    }

    /// RGBA8 with the given palette (unpainted pixels transparent).
    pub fn to_rgba(&self, palette: &crate::palette::Palette) -> Vec<u8> {
        crate::palette::masked_to_rgba(&self.pixels, &self.opaque, palette)
    }
}

// ---------------------------------------------------------------------------
// Simple table
// ---------------------------------------------------------------------------

/// Sprite of a simple table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleSprite {
    pub width: u16,
    pub height: u16,
    /// Encoded stream (`size` = `data.len()`).
    pub data: Vec<u8>,
}

/// Screen width; height rounded to 256 because tyrian(c).shp contains a 191×201 sprite.
pub const SIMPLE_MAX_WIDTH: u16 = 320;
pub const SIMPLE_MAX_HEIGHT: u16 = 256;

impl SimpleSprite {
    /// Also validates directly constructed sprites, before any allocation.
    pub fn validate_dimensions(&self) -> Result<()> {
        validate_simple_dimensions(self.width, self.height)
    }
    /// Decoding per `blit_sprite`: 255 n = skip, 254 = end of line,
    /// 253 = 1 transparent, other = pixel; line wrap at `width`.
    pub fn decode(&self) -> IndexedImage {
        // Preserves the historical signature: an invalid object gives an empty image.
        if self.validate_dimensions().is_err() {
            return IndexedImage::new(0, 0);
        }
        let w = self.width as usize;
        let mut img = IndexedImage::new(w, self.height as usize);
        if w == 0 {
            return img;
        }
        let (mut x, mut y) = (0usize, 0usize);
        let mut i = 0;
        while i < self.data.len() {
            match self.data[i] {
                255 => {
                    i += 1;
                    x += *self.data.get(i).unwrap_or(&0) as usize;
                }
                254 => x = w,
                253 => x += 1,
                v => {
                    img.set(x, y, v);
                    x += 1;
                }
            }
            // The C code advances the pointer by `pitch - x_offset`: overflows
            // past `width` are lost, the next line restarts at 0.
            if x >= w {
                y += 1;
                x = 0;
            }
            i += 1;
        }
        img
    }
}

fn validate_simple_dimensions(width: u16, height: u16) -> Result<()> {
    if width > SIMPLE_MAX_WIDTH || height > SIMPLE_MAX_HEIGHT {
        return Err(FormatError::new(format!(
            "simple sprite: dimensions {width}×{height}, maximum {SIMPLE_MAX_WIDTH}×{SIMPLE_MAX_HEIGHT}"
        )));
    }
    Ok(())
}

/// Simple table entry: raw `populated` byte + optional sprite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleEntry {
    /// Byte read by `fread_bool_die` (kept as is).
    pub populated: u8,
    pub sprite: Option<SimpleSprite>,
}

/// Simple sprite table (`load_sprites`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteTable {
    pub entries: Vec<SimpleEntry>,
    /// Bytes not read after the table.
    pub tail: Vec<u8>,
}

impl SpriteTable {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let count = r.u16()?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let populated = r.u8()?;
            let sprite = if populated != 0 {
                let width = r.u16()?;
                let height = r.u16()?;
                validate_simple_dimensions(width, height)?;
                let size = r.u16()? as usize;
                Some(SimpleSprite { width, height, data: r.bytes(size)?.to_vec() })
            } else {
                None
            };
            entries.push(SimpleEntry { populated, sprite });
        }
        Ok(Self { entries, tail: r.rest() })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u16(self.entries.len() as u16);
        for e in &self.entries {
            w.u8(e.populated);
            if let Some(s) = &e.sprite {
                w.u16(s.width);
                w.u16(s.height);
                w.u16(s.data.len() as u16);
                w.bytes(&s.data);
            }
        }
        w.bytes(&self.tail);
        w.into_inner()
    }
}

// ---------------------------------------------------------------------------
// Sprite2
// ---------------------------------------------------------------------------

/// Nominal width of a Sprite2 line (`pitch - 12` in `blit_sprite2`).
pub const SPRITE2_ROW_WIDTH: i32 = 12;

/// Sprite2 decoded in its bounding box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite2Image {
    /// Position of the box's top-left corner relative to the drawing point (x, y).
    pub offset_x: i32,
    pub offset_y: i32,
    pub image: IndexedImage,
}

/// Raw Sprite2 sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite2Sheet {
    pub data: Vec<u8>,
}

/// Pixel of a decoded sprite: (x, y, color).
pub type SpritePixel = (i32, i32, u8);

impl Sprite2Sheet {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 2 {
            return Err(FormatError::new("Sprite2: fewer than 2 bytes"));
        }
        Ok(Self { data: data.to_vec() })
    }

    pub fn write(&self) -> Vec<u8> {
        self.data.clone()
    }

    /// Number of sprites (`sprite2_count`: `offset[0] / 2`).
    pub fn count(&self) -> usize {
        if self.data.len() < 2 {
            return 0;
        }
        let first = u16::from_le_bytes([self.data[0], self.data[1]]) as usize;
        if first == 0 || first > self.data.len() {
            0
        } else {
            first / 2
        }
    }

    /// Offset of the stream of sprite `index` (1-based).
    pub fn offset(&self, index: usize) -> Option<usize> {
        if index == 0 || index > self.count() {
            return None;
        }
        let p = (index - 1) * 2;
        Some(u16::from_le_bytes([self.data[p], self.data[p + 1]]) as usize)
    }

    /// Commands of sprite `index` in coordinates relative to the
    /// drawing point, emulating the linear pointer of `blit_sprite2` with a
    /// wide virtual pitch (line overflows stay consistent).
    /// Returns the (x, y, color) list and whether the stream ends with 0x0F.
    pub fn sprite_pixels(&self, index: usize) -> Option<(Vec<SpritePixel>, bool)> {
        const PITCH: i64 = 4096;
        const ORIGIN: i64 = 2048;
        let start = self.offset(index)?;
        let mut pos: i64 = ORIGIN;
        let mut out = Vec::new();
        let mut i = start;
        let mut terminated = false;
        while i < self.data.len() {
            let b = self.data[i];
            if b == 0x0F {
                terminated = true;
                break;
            }
            pos += (b & 0x0F) as i64;
            let count = (b >> 4) as usize;
            if count == 0 {
                pos += PITCH - SPRITE2_ROW_WIDTH as i64;
            } else {
                for _ in 0..count {
                    i += 1;
                    let Some(&v) = self.data.get(i) else { break };
                    let y = pos.div_euclid(PITCH);
                    let x = pos.rem_euclid(PITCH) - ORIGIN;
                    out.push((x as i32, y as i32, v));
                    pos += 1;
                }
            }
            i += 1;
        }
        Some((out, terminated))
    }

    /// Sprite `index` (1-based) decoded in its bounding box; `None` if
    /// the index is outside the table or the sprite is empty.
    pub fn decode(&self, index: usize) -> Option<Sprite2Image> {
        let (px, _) = self.sprite_pixels(index)?;
        if px.is_empty() {
            return None;
        }
        let min_x = px.iter().map(|p| p.0).min()?;
        let max_x = px.iter().map(|p| p.0).max()?;
        let min_y = px.iter().map(|p| p.1).min()?;
        let max_y = px.iter().map(|p| p.1).max()?;
        let mut image = IndexedImage::new((max_x - min_x + 1) as usize, (max_y - min_y + 1) as usize);
        for (x, y, v) in px {
            image.set((x - min_x) as usize, (y - min_y) as usize, v);
        }
        Some(Sprite2Image { offset_x: min_x, offset_y: min_y, image })
    }
}

// ---------------------------------------------------------------------------
// newsh$.shp: Sprite2 + encrypted editor ships
// ---------------------------------------------------------------------------

/// Size of `extraShips` (`JE_ShipsType`): 150 bytes + 4 check bytes.
pub const EXTRA_SHIPS_SIZE: usize = 154;
/// `SAS` in `editship.c`.
pub const EXTRA_SHIPS_PLAIN: usize = EXTRA_SHIPS_SIZE - 4;
/// `editship.c:extraCryptKey`.
pub const EXTRA_CRYPT_KEY: [u8; 10] = [58, 23, 16, 192, 254, 82, 113, 147, 62, 99];

/// Decrypts `extraShips` (`JE_decryptShips`); returns the plaintext and the
/// result of the 4 checks (sum, difference, product+1, xor).
pub fn decrypt_extra_ships(cipher: &[u8; EXTRA_SHIPS_SIZE]) -> (Vec<u8>, bool) {
    let mut s2 = vec![0u8; EXTRA_SHIPS_PLAIN];
    for x in (0..EXTRA_SHIPS_PLAIN).rev() {
        s2[x] = cipher[x] ^ EXTRA_CRYPT_KEY[(x + 1) % 10];
        if x > 0 {
            s2[x] ^= cipher[x - 1];
        }
    }
    let sum = s2.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    let diff = s2.iter().fold(0u8, |a, &b| a.wrapping_sub(b));
    let prod = s2.iter().fold(1u8, |a, &b| a.wrapping_mul(b).wrapping_add(1));
    let xor = s2.iter().fold(0u8, |a, &b| a ^ b);
    let ok = cipher[EXTRA_SHIPS_PLAIN] == sum
        && cipher[EXTRA_SHIPS_PLAIN + 1] == diff
        && cipher[EXTRA_SHIPS_PLAIN + 2] == prod
        && cipher[EXTRA_SHIPS_PLAIN + 3] == xor;
    (s2, ok)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraShipsFile {
    pub sheet: Sprite2Sheet,
    pub extra_ships: [u8; EXTRA_SHIPS_SIZE],
}

impl ExtraShipsFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < EXTRA_SHIPS_SIZE + 2 {
            return Err(FormatError::new("newsh$.shp too short"));
        }
        let split = data.len() - EXTRA_SHIPS_SIZE;
        let mut extra_ships = [0u8; EXTRA_SHIPS_SIZE];
        extra_ships.copy_from_slice(&data[split..]);
        Ok(Self { sheet: Sprite2Sheet::parse(&data[..split])?, extra_ships })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut out = self.sheet.write();
        out.extend_from_slice(&self.extra_ships);
        out
    }
}

// ---------------------------------------------------------------------------
// tyrian.shp
// ---------------------------------------------------------------------------

/// Number of sub-tables of `tyrian.shp` (`SHP_NUM`).
pub const MAIN_SHAPE_TABLES: usize = 13;
/// Number of simple tables at the start.
pub const MAIN_SIMPLE_TABLES: usize = 7;
/// Names of the 13 tables (sprite.h / spec § 4.3).
pub const MAIN_TABLE_NAMES: [&str; MAIN_SHAPE_TABLES] = [
    "font",
    "small_font",
    "tiny_font",
    "planets",
    "faces",
    "options",
    "weapons",
    "player_shots",
    "player_ships",
    "powerups",
    "coins_datacubes",
    "player_shots2",
    "ships_t2000",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainShapes {
    pub count: u16,
    /// Tables 0..6.
    pub simple: Vec<SpriteTable>,
    /// Tables 7..12.
    pub sheets: Vec<Sprite2Sheet>,
}

impl MainShapes {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let c = OffsetContainer::parse(data)?;
        if c.entries.len() != MAIN_SHAPE_TABLES {
            return Err(FormatError::new(format!("tyrian.shp: {} tables instead of 13", c.entries.len())));
        }
        let simple = c.entries[..MAIN_SIMPLE_TABLES].iter().map(|e| SpriteTable::parse(e)).collect::<Result<_>>()?;
        let sheets = c.entries[MAIN_SIMPLE_TABLES..].iter().map(|e| Sprite2Sheet::parse(e)).collect::<Result<_>>()?;
        Ok(Self { count: c.count, simple, sheets })
    }

    /// Intentional deviation from the original (divergences.md): in the small
    /// font (table 1) and the tiny font (table 2), the glyph "s"
    /// (index 52) is a capital S at capital height. It is
    /// replaced by a lowercase s at the height of a, e, o, same colors;
    /// the C code does the same replacement (sprite.c:font_use_lowercase_s).
    /// Does not touch the file: `parse` stays faithful to the data.
    pub fn with_lowercase_s(mut self) -> Self {
        const SMALL_S: [u8; 55] = [
            254, 254, 253, 29, 31, 30, 29, 26, 254, 27, 29, 29, 29, 29, 29, 26, 29, 29, 25, 254, 26, 29, 29, 29, 29,
            26, 254, 253, 26, 29, 29, 29, 29, 26, 253, 253, 253, 253, 25, 29, 29, 26, 29, 29, 29, 29, 29, 27, 253, 27,
            29, 29, 29, 27, 254,
        ];
        const TINY_S: [u8; 17] = [254, 254, 3, 7, 7, 7, 7, 3, 254, 253, 253, 3, 7, 7, 7, 7, 3];
        for (table, w, h, data) in [(1usize, 7u16, 10u16, &SMALL_S[..]), (2, 4, 6, &TINY_S[..])] {
            if let Some(e) = self.simple.get_mut(table).and_then(|t| t.entries.get_mut(52)) {
                if e.sprite.is_some() {
                    e.sprite = Some(SimpleSprite { width: w, height: h, data: data.to_vec() });
                }
            }
        }
        self
    }

    /// Intentional deviation from the original (divergences.md): in the tiny
    /// font (table 2), the glyph "x" (index 57) has its center at index 4,
    /// much darker than the extremities at index 7; on screen only
    /// four dots at the corners remain and it reads as "%" ("1.5x" → "1.5%").
    /// It is replaced by a solid x, same dimensions (4×6, rows 2 to 5, at
    /// the height of a, e, o), all at index 7. The small font (table 1)
    /// already has a readable x: unchanged. Does not touch the file.
    pub fn with_redrawn_x(mut self) -> Self {
        const TINY_X: [u8; 18] = [254, 254, 7, 255, 2, 7, 253, 7, 7, 254, 253, 7, 7, 254, 7, 255, 2, 7];
        if let Some(e) = self.simple.get_mut(2).and_then(|t| t.entries.get_mut(57)) {
            if e.sprite.is_some() {
                e.sprite = Some(SimpleSprite { width: 4, height: 6, data: TINY_X.to_vec() });
            }
        }
        self
    }

    pub fn write(&self) -> Vec<u8> {
        let mut entries: Vec<Vec<u8>> = self.simple.iter().map(|t| t.write()).collect();
        entries.extend(self.sheets.iter().map(|s| s.write()));
        OffsetContainer { count: self.count, entries }.write()
    }
}
