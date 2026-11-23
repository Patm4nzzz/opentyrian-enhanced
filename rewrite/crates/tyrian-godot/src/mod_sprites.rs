//! Mod images of the 8-bit pictures. First the play area: coins, enemies and
//! explosions drawn by the compositor (`playfield.rs`) in place of the original
//! sheet images; at the end of the file the screens: portraits, planets and
//! weapon pictures (`TyrianVga.blit_sprite`). For the play area,
//! like `sprite_override.c` (`coin_overrides`, `enemy_overrides`,
//! `explosion_overrides`, read by `enemy_render.c:blit_enemy` and
//! `explosion_update.c`).
//!
//! The PNGs are mapped to the palette of the level when the compositor first
//! needs them (the C maps them once to `palettes[0]`); pixels with an alpha
//! under 128 are transparent. Rendering only: nothing here is read by the
//! simulation.
//!
//! Where the rewrite goes further than the C, on purpose (docs/divergences.md,
//! entry 69):
//! - a coin PNG is the strip of its animation (frames of 12x14 or 24x28, like
//!   the templates of `data/reference/sprites/coins`): every frame is replaced,
//!   where the C always drew the first frame of the strip for the first image
//!   and the original images for the others;
//! - an image larger than one sheet cell stands for the whole 2x2 enemy: the
//!   three other cells are not drawn over it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use godot::classes::image::Format;
use godot::classes::Image;
use tyrian_mods::{SpriteAssignment, SpriteTable};
use tyrian_render::assets::Rgb8Palette;
use tyrian_render::Surface;
use tyrian_sim::world::SHEET_TYRIAN_SHP_11;

/// One sheet cell (`Sprite2` images of the enemy sheets).
const CELL_W: i32 = 12;
const CELL_H: i32 = 14;

/// A mod image in palette indices, 0 = transparent.
pub struct Indexed {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    /// Sheet byte (`World::enemy` sheet, 11 = coins) and image of the sheet.
    Enemy(u8, u32),
    /// `explosions[].sprite` (the names of data/sprite_manifest.json).
    Explosion(u32),
}

struct Rgba {
    key: Key,
    w: i32,
    h: i32,
    data: Vec<u8>,
}

#[derive(Default)]
pub struct ModSprites {
    /// Loaded, not yet mapped to the palette.
    pending: Vec<Rgba>,
    enemies: HashMap<(u8, u32), Indexed>,
    explosions: HashMap<u32, Indexed>,
}

thread_local! {
    pub static MOD_SPRITES: RefCell<ModSprites> = RefCell::new(ModSprites::default());
}

/// Sheet byte of an enemy image bank (`shapebank` 1..36), as the simulation
/// gives it for a drawn enemy (`tyrian-sim/src/enemies.rs`): banks 21 and 26
/// are sheets 11 and 10 of tyrian.shp, the others the lowercase letter of
/// their `newsh?.shp` file. 0 outside 1..36.
pub fn bank_sheet_byte(bank: i32) -> i32 {
    match bank {
        21 => 11,
        26 => 10,
        1..=36 => tyrian_sim::world::SHAPE_FILE[(bank - 1) as usize].to_ascii_lowercase() as i32,
        _ => 0,
    }
}

/// Palette of a level: that of its dashboard picture (`tyrian2.c:JE_main`,
/// `JE_loadPic(3)`, or 6 in a two-player game: same palette). The C maps the
/// mod images to `palettes[0]` and calls it an approximation; the level's own
/// palette gives the colors of the PNG.
pub fn level_palette(a: &tyrian_render::Assets) -> Rgb8Palette {
    a.palettes.get(tyrian_formats::palette::GAMEPLAY_PALETTE).copied().unwrap_or([[0; 3]; 256])
}

/// A mod image of a screen (portrait, planet, weapon picture), mapped to the
/// palette of its screen at the first blit.
enum ScreenImage {
    Rgba { w: i32, h: i32, data: Vec<u8> },
    Indexed(Indexed),
}

thread_local! {
    /// Mod images of the simple sprite tables of tyrian.shp, by (table, image):
    /// `PLANET_SHAPES`, `FACE_SHAPES` (`sprite_override.c:sprite_override_for_table`)
    /// and `WEAPON_SHAPES`. Kept apart from the play area images: they follow
    /// the enabled mods, not the level being played.
    static SCREEN_SPRITES: RefCell<HashMap<(usize, u32), ScreenImage>> = RefCell::new(HashMap::new());
}

/// Sprite table of tyrian.shp a mod image category replaces, if any.
fn screen_table(table: SpriteTable) -> Option<usize> {
    match table {
        SpriteTable::Planets => Some(tyrian_render::assets::PLANET_SHAPES),
        SpriteTable::Portraits => Some(tyrian_render::assets::FACE_SHAPES),
        SpriteTable::WeaponIcons => Some(tyrian_render::assets::WEAPON_SHAPES),
        _ => None,
    }
}

/// Loads the portrait, planet and weapon picture images of the enabled mods
/// (called when the mods are reloaded). Returns the number of images kept.
pub fn set_screen_sprites(sprites: &[SpriteAssignment]) -> usize {
    let mut out = HashMap::new();
    for s in sprites {
        let Some(table) = screen_table(s.table).filter(|_| s.index >= 0) else { continue };
        if let Some((w, h, data)) = load_rgba(&s.path, s.hue) {
            out.insert((table, s.index as u32), ScreenImage::Rgba { w, h, data });
        }
    }
    let n = out.len();
    SCREEN_SPRITES.with(|m| *m.borrow_mut() = out);
    n
}

/// Palette a screen image is mapped to and its last usable color, like
/// `sprite_override.c`: a portrait under the palette of its face
/// (`pcxmast.c:facepal`, the data cube screen reloads colors 1..207 from it),
/// a planet under palette 18 (navigation map, `newPal = 18`); the weapon
/// pictures of the shop's ship illustration under the shop palette (0).
fn screen_palette(a: &tyrian_render::Assets, table: usize, index: u32) -> (Rgb8Palette, usize) {
    use tyrian_formats::palette::FACE_PALETTE;
    let (palette, last) = if table == tyrian_render::assets::FACE_SHAPES {
        (FACE_PALETTE.get(index as usize).copied().unwrap_or(0) as usize, 207)
    } else if table == tyrian_render::assets::PLANET_SHAPES {
        (18, 255)
    } else {
        (0, 255)
    };
    (a.palettes.get(palette).copied().unwrap_or([[0; 3]; 256]), last)
}

/// `sprite.c:blit_sprite` with `sprite_override_for_table`: draws the image a
/// mod gives to sprite `index` of `table` at (`x`, `y`) and returns true, or
/// returns false when the original sprite applies. Like the C, an image larger
/// than the original sprite is not used: the callers place the sprite by its
/// original size (a wider title logo would run over the menu).
pub fn blit_screen_sprite(
    surf: &mut Surface,
    a: &tyrian_render::Assets,
    x: i32,
    y: i32,
    table: usize,
    index: usize,
) -> bool {
    SCREEN_SPRITES.with(|m| {
        let mut map = m.borrow_mut();
        let Some(entry) = map.get_mut(&(table, index as u32)) else { return false };
        if let ScreenImage::Rgba { w, h, data } = entry {
            let (palette, last) = screen_palette(a, table, index as u32);
            let px = map_to_palette(data, &palette, last, &mut HashMap::new());
            *entry = ScreenImage::Indexed(Indexed { w: *w, h: *h, px });
        }
        let ScreenImage::Indexed(img) = entry else { return false };
        if let Some(original) = a.sprite(table, index) {
            if img.w > original.width as i32 || img.h > original.height as i32 {
                return false;
            }
        }
        blit(surf, x, y, img, |_, v| v);
        true
    })
}

/// RGBA8 pixels to palette indices: nearest color among 1..=`last`, alpha
/// under 128 transparent (0). `nearest` remembers the colors already mapped.
fn map_to_palette(data: &[u8], palette: &Rgb8Palette, last: usize, nearest: &mut HashMap<[u8; 3], u8>) -> Vec<u8> {
    data.as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            if p[3] < 128 {
                return 0;
            }
            *nearest.entry([p[0], p[1], p[2]]).or_insert_with(|| {
                let dist = |c: &[u8; 3]| {
                    let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
                    d(c[0], p[0]) + d(c[1], p[1]) + d(c[2], p[2])
                };
                (1..=last).min_by_key(|&i| dist(&palette[i])).unwrap_or(1) as u8
            })
        })
        .collect()
}

/// No mod image: the original game (replays, demos, level without mods).
pub fn clear() {
    MOD_SPRITES.with(|m| *m.borrow_mut() = ModSprites::default());
}

/// Loads the coin, enemy and explosion images of the enabled mods for the
/// level about to start. Returns the number of images kept.
pub fn set(sprites: &[SpriteAssignment]) -> usize {
    let mut out = ModSprites::default();
    for s in sprites {
        if !matches!(s.table, SpriteTable::Coins | SpriteTable::Enemies | SpriteTable::Explosions) || s.index < 0 {
            continue;
        }
        let Some((w, h, data)) = load_rgba(&s.path, s.hue) else { continue };
        let index = s.index as u32;
        match s.table {
            SpriteTable::Coins => out.push_coin_frames(index, w, h, data),
            SpriteTable::Enemies => {
                let sheet = bank_sheet_byte(s.bank);
                if sheet > 0 {
                    out.pending.push(Rgba { key: Key::Enemy(sheet as u8, index), w, h, data });
                }
            }
            _ => out.pending.push(Rgba { key: Key::Explosion(index), w, h, data }),
        }
    }
    let n = out.pending.len();
    MOD_SPRITES.with(|m| *m.borrow_mut() = out);
    n
}

/// RGBA8 pixels of a PNG, tinted like `Mods._tint` (mods.gd) when the mod asks
/// for a hue.
fn load_rgba(path: &Path, hue: Option<f32>) -> Option<(i32, i32, Vec<u8>)> {
    let mut img = Image::load_from_file(path.to_string_lossy().as_ref())?;
    if img.is_empty() {
        return None;
    }
    img.convert(Format::RGBA8);
    let (w, h) = (img.get_width(), img.get_height());
    let mut data = img.get_data().to_vec();
    if data.len() != (w * h * 4) as usize {
        return None;
    }
    if let Some(hue) = hue {
        for px in data.as_chunks_mut::<4>().0 {
            tint(px, hue);
        }
    }
    Some((w, h, data))
}

/// `sprite_override.c:apply_tint`: replaces the hue of the colored pixels,
/// leaves the grays (outlines) as they are.
fn tint(px: &mut [u8], hue: f32) {
    if px[3] == 0 {
        return;
    }
    let (r, g, b) = (px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let v = mx;
    let s = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
    if s < 0.05 {
        return;
    }
    let s = (s * 1.2).min(1.0);
    let h6 = hue.rem_euclid(1.0) * 6.0;
    let i = h6 as i32;
    let f = h6 - i as f32;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (nr, ng, nb) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    px[0] = (nr * 255.0) as u8;
    px[1] = (ng * 255.0) as u8;
    px[2] = (nb * 255.0) as u8;
}

impl ModSprites {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.enemies.is_empty() && self.explosions.is_empty()
    }

    /// A coin PNG is the strip of its animation: frames of 12x14 (frame k is
    /// image `index + k`) or of 24x28 (a coin of 2x2 cells, frame k is image
    /// `index + 2k`). Any other size is a single image.
    fn push_coin_frames(&mut self, index: u32, w: i32, h: i32, data: Vec<u8>) {
        let (fw, fh) = if h == CELL_H { (CELL_W, CELL_H) } else { (2 * CELL_W, 2 * CELL_H) };
        if h != fh || w % fw != 0 || w == fw {
            self.pending.push(Rgba { key: Key::Enemy(SHEET_TYRIAN_SHP_11, index), w, h, data });
            return;
        }
        for k in 0..w / fw {
            let mut frame = Vec::with_capacity((fw * fh * 4) as usize);
            for y in 0..fh {
                let start = ((y * w + k * fw) * 4) as usize;
                frame.extend_from_slice(&data[start..start + (fw * 4) as usize]);
            }
            let image = index + (k * (fw / CELL_W)) as u32;
            self.pending.push(Rgba { key: Key::Enemy(SHEET_TYRIAN_SHP_11, image), w: fw, h: fh, data: frame });
        }
    }

    /// Maps the loaded images to `palette` (`level_palette`; rule of
    /// `sprite_override.c:build_palette_lut`): nearest color among indices
    /// 1..255, alpha under 128 transparent.
    pub fn ensure_indexed(&mut self, palette: &Rgb8Palette) {
        if self.pending.is_empty() {
            return;
        }
        let mut nearest: HashMap<[u8; 3], u8> = HashMap::new();
        for img in std::mem::take(&mut self.pending) {
            let px = map_to_palette(&img.data, palette, 255, &mut nearest);
            let indexed = Indexed { w: img.w, h: img.h, px };
            match img.key {
                Key::Enemy(sheet, image) => {
                    self.enemies.insert((sheet, image), indexed);
                }
                Key::Explosion(sprite) => {
                    self.explosions.insert(sprite, indexed);
                }
            }
        }
    }

    /// Image a mod gives to image `image` of enemy sheet `sheet`.
    pub fn enemy(&self, sheet: u8, image: u32) -> Option<&Indexed> {
        self.enemies.get(&(sheet, image))
    }

    /// Image a mod gives to the explosion image `sprite` (`explosions[].sprite`).
    pub fn explosion(&self, sprite: u32) -> Option<&Indexed> {
        self.explosions.get(&sprite)
    }

    /// An enemy of 2x2 cells is drawn as four pieces (images +0, +1, +19, +20).
    /// A mod image wider or taller than one cell, placed on the first piece,
    /// stands for the whole enemy: true if `image` is a piece it covers (like
    /// the 2x2 coins of `enemy_render.c:blit_enemy`).
    pub fn covers(&self, sheet: u8, image: u32) -> bool {
        [(1u32, true, false), (19, false, true), (20, true, true)].iter().any(|&(back, wide, tall)| {
            image
                .checked_sub(back)
                .and_then(|base| self.enemy(sheet, base))
                .is_some_and(|img| (!wide || img.w > CELL_W) && (!tall || img.h > CELL_H))
        })
    }
}

/// Draws `img`, its corner at (`x`, `y`), on a 320x200 surface. `f(under, pixel)`
/// gives the value written for each opaque pixel (plain copy, blend, filter or
/// darkening, like the `blit_sprite2*` variants).
pub fn blit(surf: &mut Surface, x: i32, y: i32, img: &Indexed, f: impl Fn(u8, u8) -> u8) {
    let (sw, sh) = (surf.w as i32, surf.h as i32);
    for row in 0..img.h {
        let dy = y + row;
        if dy < 0 || dy >= sh {
            continue;
        }
        for col in 0..img.w {
            let dx = x + col;
            if dx < 0 || dx >= sw {
                continue;
            }
            let v = img.px[(row * img.w + col) as usize];
            if v != 0 {
                let i = (dy * sw + dx) as usize;
                surf.pixels[i] = f(surf.pixels[i], v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(w: i32, h: i32, color: [u8; 4]) -> Vec<u8> {
        (0..w * h).flat_map(|_| color).collect()
    }

    fn palette() -> Rgb8Palette {
        let mut p = [[0u8; 3]; 256];
        p[5] = [255, 0, 255];
        p[9] = [0, 255, 255];
        p
    }

    /// A 72x14 coin strip gives six frames on consecutive images; a 72x28
    /// strip gives three frames of 2x2 cells, two images apart.
    #[test]
    fn coin_strips_are_split_into_animation_frames() {
        let mut m = ModSprites::default();
        m.push_coin_frames(1, 72, 14, rgba(72, 14, [255, 0, 255, 255]));
        m.push_coin_frames(191, 72, 28, rgba(72, 28, [0, 255, 255, 255]));
        m.ensure_indexed(&palette());
        for image in 1..=6 {
            let img = m.enemy(SHEET_TYRIAN_SHP_11, image).unwrap_or_else(|| panic!("frame {image}"));
            assert_eq!((img.w, img.h, img.px[0]), (12, 14, 5));
        }
        assert!(m.enemy(SHEET_TYRIAN_SHP_11, 7).is_none());
        for image in [191, 193, 195] {
            let img = m.enemy(SHEET_TYRIAN_SHP_11, image).unwrap_or_else(|| panic!("frame {image}"));
            assert_eq!((img.w, img.h, img.px[0]), (24, 28, 9));
        }
        assert!(m.enemy(SHEET_TYRIAN_SHP_11, 192).is_none());
        // The three other cells of a 2x2 coin are covered by its image.
        for image in [192, 210, 211] {
            assert!(m.covers(SHEET_TYRIAN_SHP_11, image), "image {image}");
        }
        // A one-cell coin covers nothing else.
        assert!(!m.covers(SHEET_TYRIAN_SHP_11, 2 + 19));
    }

    /// Transparent pixels stay transparent, the others take the nearest color
    /// of the palette, never index 0; the blit clips at the surface edges.
    #[test]
    fn images_are_mapped_to_the_palette_and_clipped() {
        let mut data = rgba(2, 1, [250, 5, 250, 255]);
        data[7] = 0; // second pixel transparent
        let mut m = ModSprites::default();
        m.pending.push(Rgba { key: Key::Explosion(3), w: 2, h: 1, data });
        m.ensure_indexed(&palette());
        let img = m.explosion(3).unwrap();
        assert_eq!(img.px, vec![5, 0]);
        let mut surf = Surface::new(320, 200);
        blit(&mut surf, 319, 199, img, |_, v| v);
        assert_eq!(surf.pixels[199 * 320 + 319], 5);
        blit(&mut surf, -1, 0, img, |_, v| v);
        assert_eq!(surf.pixels[0], 0, "the opaque pixel is off the left edge, the other is transparent");
        // Filter variant: the value written comes from the callback.
        blit(&mut surf, 10, 10, img, |under, v| under | (v + 1));
        assert_eq!(surf.pixels[10 * 320 + 10], 6);
    }

    /// A screen image is mapped to the palette of its screen, limited to the
    /// colors that screen loads: 207 for a portrait.
    #[test]
    fn portraits_only_use_the_colors_their_screen_loads() {
        let mut p = [[0u8; 3]; 256];
        p[100] = [10, 10, 10];
        p[230] = [255, 0, 255];
        let px = map_to_palette(&[255, 0, 255, 255, 0, 0, 0, 0], &p, 207, &mut HashMap::new());
        assert!(px[0] >= 1 && px[0] <= 207, "index {} is beyond the colors of the screen", px[0]);
        assert_eq!(px[1], 0);
        let px = map_to_palette(&[255, 0, 255, 255], &p, 255, &mut HashMap::new());
        assert_eq!(px, vec![230]);
    }

    #[test]
    fn bank_sheet_bytes_follow_the_simulation() {
        assert_eq!(bank_sheet_byte(1), b'2' as i32);
        assert_eq!(bank_sheet_byte(5), b'a' as i32);
        assert_eq!(bank_sheet_byte(21), 11);
        assert_eq!(bank_sheet_byte(26), 10);
        assert_eq!(bank_sheet_byte(0), 0);
        assert_eq!(bank_sheet_byte(37), 0);
    }
}
