//! `palette.dat` (`palette.c:JE_loadPals`): 24 palettes × 256 × 6-bit VGA RGB.

use crate::io::{FormatError, Result};
use serde::Serialize;

pub const PALETTE_COUNT: usize = 24;
pub const PALETTE_FILE_SIZE: usize = PALETTE_COUNT * 256 * 3;

/// Palette of each image of `tyrian.pic` (`pcxmast.c:pcxpal`, 0-based index).
pub const PCX_PALETTE: [u8; 14] = [0, 7, 5, 8, 10, 5, 18, 19, 19, 20, 21, 22, 5, 23];
/// Palette of the datacube faces (`pcxmast.c:facepal`, 0-based face index).
pub const FACE_PALETTE: [u8; 12] = [1, 2, 3, 4, 6, 9, 11, 12, 16, 13, 14, 15];
/// In-game palette: the level starts after `JE_loadPic(3 or 6)` (tyrian2.c),
/// hence `palettes[pcxpal[2]] = palettes[5]` (identical for image 6).
pub const GAMEPLAY_PALETTE: usize = PCX_PALETTE[2] as usize;

/// One 8-bit color per component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// 6-bit VGA component → 8 bits, exactly like `palette.c`.
pub fn vga6_to_8(c: u8) -> u8 {
    (c << 2) | (c >> 4)
}

/// Raw palette: 256 6-bit triplets (file bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub raw: [[u8; 3]; 256],
}

impl Palette {
    /// 8-bit color of index `i`.
    pub fn rgb(&self, i: u8) -> Rgb {
        let [r, g, b] = self.raw[i as usize];
        Rgb { r: vga6_to_8(r), g: vga6_to_8(g), b: vga6_to_8(b) }
    }

    /// The 256 colors in 8 bits.
    pub fn to_rgb8(&self) -> Vec<Rgb> {
        (0..=255u8).map(|i| self.rgb(i)).collect()
    }

    /// RGBA for an index; index 0 is transparent if `transparent_zero`.
    pub fn rgba(&self, i: u8, transparent_zero: bool) -> [u8; 4] {
        let c = self.rgb(i);
        let a = if transparent_zero && i == 0 { 0 } else { 255 };
        [c.r, c.g, c.b, a]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteFile {
    pub palettes: Vec<Palette>,
}

impl PaletteFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if !data.len().is_multiple_of(256 * 3) || data.is_empty() {
            return Err(FormatError::new(format!("palette.dat: size {} not a multiple of 768", data.len())));
        }
        let palettes = data
            .as_chunks::<{ 256 * 3 }>()
            .0
            .iter()
            .map(|chunk| {
                let mut raw = [[0u8; 3]; 256];
                for (i, c) in chunk.as_chunks::<3>().0.iter().enumerate() {
                    raw[i] = [c[0], c[1], c[2]];
                }
                Palette { raw }
            })
            .collect();
        Ok(Self { palettes })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.palettes.len() * 768);
        for p in &self.palettes {
            for c in &p.raw {
                out.extend_from_slice(c);
            }
        }
        out
    }
}

/// Converts an index buffer to RGBA8 (index 0 transparent if requested).
pub fn indexed_to_rgba(pixels: &[u8], palette: &Palette, transparent_zero: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for &p in pixels {
        out.extend_from_slice(&palette.rgba(p, transparent_zero));
    }
    out
}

/// Variant with an explicit opacity mask (unpainted pixels = transparent).
pub fn masked_to_rgba(pixels: &[u8], mask: &[bool], palette: &Palette) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for (&p, &m) in pixels.iter().zip(mask) {
        if m {
            out.extend_from_slice(&palette.rgba(p, false));
        } else {
            out.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    out
}
