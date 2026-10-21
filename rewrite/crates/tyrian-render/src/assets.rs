//! Graphics data read directly from `data/` (like the C code):
//! `tyrian.shp` (13 tables, `sprite.c:JE_loadMainShapeTables`),
//! `palette.dat` (`palette.c:JE_loadPals`), `tyrian.pic` (`picload.c`),
//! `newsh?.shp` sheets (`JE_loadCompShapes`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tyrian_formats::palette::{vga6_to_8, PaletteFile, PCX_PALETTE};
use tyrian_formats::pic::PicFile;
use tyrian_formats::sprite::MainShapes;

pub use tyrian_formats::sprite::SimpleSprite;

/// `sprite.h`: simple tables of `tyrian.shp`.
pub const FONT_SHAPES: usize = 0;
pub const SMALL_FONT_SHAPES: usize = 1;
pub const TINY_FONT: usize = 2;
pub const PLANET_SHAPES: usize = 3;
pub const FACE_SHAPES: usize = 4;
pub const OPTION_SHAPES: usize = 5;
pub const WEAPON_SHAPES: usize = 6;

/// 8-bit-per-component palette (the C `Palette` after `JE_loadPals`).
pub type Rgb8Palette = [[u8; 3]; 256];

pub struct Assets {
    pub data_dir: PathBuf,
    /// Simple tables 0..6: optional sprite by index (0-based).
    pub tables: Vec<Vec<Option<SimpleSprite>>>,
    /// Sprite2 sheets of `tyrian.shp`: 7 = shots (spriteSheet8), 8 =
    /// ships (spriteSheet9), 9 = objects (10), 10 = coins (11), 11 = shots 2
    /// (12), 12 = T2000.
    pub main_sheets: Vec<Vec<u8>>,
    pub palettes: Vec<Rgb8Palette>,
    pics: PicFile,
    extra_sheets: HashMap<String, Vec<u8>>,
}

impl Assets {
    pub fn load(data_dir: &Path, xmas: bool) -> Result<Self, String> {
        let read = |name: &str| {
            std::fs::read(data_dir.join(name)).map_err(|e| format!("{}: {e}", data_dir.join(name).display()))
        };
        let shp = MainShapes::parse(&read(if xmas { "tyrianc.shp" } else { "tyrian.shp" })?)
            .map_err(|e| e.to_string())?
            .with_lowercase_s()
            .with_redrawn_x();
        let tables = shp.simple.iter().map(|t| t.entries.iter().map(|e| e.sprite.clone()).collect()).collect();
        let main_sheets = shp.sheets.iter().map(|s| s.data.clone()).collect();
        let pal = PaletteFile::parse(&read("palette.dat")?).map_err(|e| e.to_string())?;
        let palettes = pal
            .palettes
            .iter()
            .map(|p| {
                let mut out = [[0u8; 3]; 256];
                for (i, c) in p.raw.iter().enumerate() {
                    out[i] = [vga6_to_8(c[0]), vga6_to_8(c[1]), vga6_to_8(c[2])];
                }
                out
            })
            .collect();
        let pics = PicFile::parse(&read("tyrian.pic")?).map_err(|e| e.to_string())?;
        Ok(Self { data_dir: data_dir.to_path_buf(), tables, main_sheets, palettes, pics, extra_sheets: HashMap::new() })
    }

    /// Sprite `index` of the simple table `table`, if it exists (`sprite_exists`).
    pub fn sprite(&self, table: usize, index: usize) -> Option<&SimpleSprite> {
        self.tables.get(table)?.get(index)?.as_ref()
    }

    /// `JE_loadPic`: pixels of image `number` (1-based) and its palette.
    pub fn pic(&self, number: usize) -> Option<(Vec<u8>, usize)> {
        let idx = number.checked_sub(1)?;
        let (px, _) = self.pics.decode(idx).ok()?;
        Some((px, PCX_PALETTE.get(idx).copied().unwrap_or(0) as usize))
    }

    /// Sprite2 sheet: "main:N" (N = 7..12, table of tyrian.shp) or a
    /// file name (`newsh6.shp`, `newsh^.shp`...), loaded on first call.
    pub fn sheet(&mut self, name: &str) -> Option<&[u8]> {
        if let Some(n) = name.strip_prefix("main:") {
            let i: usize = n.parse().ok()?;
            return self.main_sheets.get(i.checked_sub(7)?).map(|v| v.as_slice());
        }
        if !self.extra_sheets.contains_key(name) {
            let data = std::fs::read(self.data_dir.join(name)).ok()?;
            self.extra_sheets.insert(name.to_string(), data);
        }
        self.extra_sheets.get(name).map(|v| v.as_slice())
    }
}
