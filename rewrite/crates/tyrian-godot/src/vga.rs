//! `TyrianVga`: palettized 8-bit framebuffer exposed to GDScript (ADR 0020).
//!
//! Reproduces the three 8-bit SDL surfaces of the C (`VGAScreen` = 0,
//! `VGAScreen2` = 1, `game_screen` = 2), the hardware palette and its fades,
//! the blitters of `sprite.c` and the fonts. GDScript calls the same
//! functions, in the same order, as the C code it reproduces, then displays
//! `image()` (indices passed through the palette).

use std::sync::Mutex;

use godot::classes::image::Format;
use godot::classes::Image;
use godot::prelude::*;
use tyrian_render::assets::Rgb8Palette;
use tyrian_render::font::{self, Align};
use tyrian_render::{Assets, HwPalette, Surface};

pub(crate) static ASSETS: Mutex<Option<Assets>> = Mutex::new(None);

/// Access to the shared graphics data (loaded once).
pub fn with_assets<R>(f: impl FnOnce(&mut Assets) -> R) -> Option<R> {
    let mut g = ASSETS.lock().ok()?;
    g.as_mut().map(f)
}

#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct TyrianVga {
    pub surfaces: [Surface; 3],
    /// `level_helpers.c:JE_whoa`: TempScreen1 / TempScreen2 buffers of
    /// the effect in progress (`whoa_begin`), `None` outside the effect.
    whoa: Option<(Vec<u8>, Vec<u8>)>,
    /// HD mode: last image read by `indices` and its provenance, restored
    /// by `set_indices` when the same image is put back (shop background,
    /// save menu background).
    saved_trace: Option<(Vec<u8>, Surface)>,
    /// Last HD image composed per surface (`hd_image`).
    pub hd_cache: [Option<crate::hd_ui::HdCache>; 3],
    pub palette: HwPalette,
    /// `palette.c:colors`: target palette of the fades (set by `load_pic`).
    pub colors: Rgb8Palette,
    /// Screen and palette from before a box placed in true colors (quit_box.rs).
    pub(crate) box_backup: Option<crate::quit_box::BoxBackup>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for TyrianVga {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            surfaces: [Surface::new(320, 200), Surface::new(320, 200), Surface::new(320, 200)],
            whoa: None,
            saved_trace: None,
            hd_cache: [None, None, None],
            palette: HwPalette::default(),
            colors: [[0; 3]; 256],
            box_backup: None,
            base,
        }
    }
}

/// `level_helpers.c:JE_whoa` : topBorder (4 lignes), bottomBorder (7 lignes).
const WHOA_TOP_BORDER: usize = 320 * 4;
const WHOA_BOTTOM_BORDER: usize = 320 * 7;

fn s(text: &GString) -> Vec<u8> {
    font::to_cp437(&text.to_string())
}

impl TyrianVga {
    fn surf(&mut self, i: i32) -> &mut Surface {
        &mut self.surfaces[i.clamp(0, 2) as usize]
    }
}

#[godot_api]
impl TyrianVga {
    /// Loads `tyrian.shp`, `palette.dat`, `tyrian.pic` from `data/`.
    /// No effect if already loaded. Returns false on error.
    #[func]
    fn load_assets(data_dir: GString) -> bool {
        let mut g = ASSETS.lock().unwrap();
        if g.is_some() {
            return true;
        }
        match Assets::load(std::path::Path::new(&data_dir.to_string()), false) {
            Ok(a) => {
                *g = Some(a);
                true
            }
            Err(e) => {
                godot_error!("TyrianVga: {e}");
                false
            }
        }
    }

    /// Christmas mode (opentyr.c: `JE_loadMainShapeTables(xmas ? "tyrianc.shp" : "tyrian.shp")`):
    /// reloads the resources with the Christmas shape table or the ordinary one, and
    /// tells the HD mode to stop looking for upscaled images for the sheets
    /// that Christmas redraws (the C then ignores the HD overrides).
    #[func]
    fn set_christmas(data_dir: GString, xmas: bool) -> bool {
        match Assets::load(std::path::Path::new(&data_dir.to_string()), xmas) {
            Ok(a) => {
                *ASSETS.lock().unwrap() = Some(a);
                crate::hd_ui::set_christmas(xmas);
                true
            }
            Err(e) => {
                godot_error!("TyrianVga: {e}");
                false
            }
        }
    }

    // --- Surfaces -----------------------------------------------------------

    #[func]
    fn clear(&mut self, surf: i32, color: i32) {
        self.surf(surf).clear(color as u8);
    }

    /// `memcpy(dst->pixels, src->pixels, …)`.
    #[func]
    fn copy(&mut self, dst: i32, src: i32) {
        let from = self.surfaces[src.clamp(0, 2) as usize].clone();
        let to = self.surf(dst);
        to.pixels.copy_from_slice(&from.pixels);
        if to.trace.is_some() {
            to.trace = from.trace;
        }
    }

    /// Copy of a rectangle (`JE_*` that copy an area from one screen to another).
    #[func]
    fn copy_rect(&mut self, dst: i32, src: i32, x: i32, y: i32, w: i32, h: i32) {
        let from = self.surfaces[src.clamp(0, 2) as usize].clone();
        let to = self.surf(dst);
        for yy in y.max(0)..(y + h).min(200) {
            for xx in x.max(0)..(x + w).min(320) {
                let i = yy as usize * 320 + xx as usize;
                to.pixels[i] = from.pixels[i];
                if let (Some(t), Some(f)) = (to.trace.as_mut(), from.trace.as_ref()) {
                    t.origin[i] = f.origin[i];
                    t.source[i] = f.source[i];
                    t.written[i] = f.written[i];
                    t.under[i] = f.under[i];
                }
            }
        }
    }

    /// `game_menu_nav.c:JE_scaleBitmap`: the whole `src` surface shrunk into
    /// rectangle (x1, y1)-(x2, y2) of `dst`, nearest neighbor (cumulative float
    /// steps, like the C).
    #[func]
    fn scale_bitmap(&mut self, dst: i32, src: i32, x1: i32, y1: i32, x2: i32, y2: i32) {
        if x1 < 0 || y1 < 0 || x2 >= 320 || y2 >= 200 || x2 < x1 || y2 < y1 {
            return;
        }
        let from = self.surfaces[src.clamp(0, 2) as usize].pixels.clone();
        let to = self.surf(dst);
        let (w, h) = ((x2 - x1 + 1) as usize, (y2 - y1 + 1) as usize);
        let (skip_w, skip_h) = (320.0f32 / w as f32, 200.0f32 / h as f32);
        let mut cum_h = 0.0f32;
        for i in 0..h {
            let row = 320 * (cum_h as usize);
            cum_h += skip_h;
            let mut cum_w = 0.0f32;
            for j in 0..w {
                let k = (y1 as usize + i) * 320 + x1 as usize + j;
                to.pixels[k] = from[row + cum_w as usize];
                to.forget(k);
                cum_w += skip_w;
            }
        }
    }

    /// End of `game_menu_nav.c:JE_drawShipSpecs`: each pixel of `src` lighter
    /// (low 4 bits) than the average of its four neighbors is placed in
    /// `dst` in green (`| 0xc0`); the other pixels of `dst` stay.
    /// `level_helpers.c:JE_whoa`, start (ENGAGE code of the title): top
    /// (4 rows) and bottom (7 rows) edges of `surf` cleared, TempScreen1 to zero,
    /// TempScreen2 = copy of the screen.
    #[func]
    fn whoa_begin(&mut self, surf: i32) {
        let sf = self.surf(surf);
        let (top, bottom) = (WHOA_TOP_BORDER, WHOA_BOTTOM_BORDER);
        let size = sf.pixels.len();
        sf.pixels[..top].fill(0);
        sf.pixels[size - bottom..].fill(0);
        for i in (0..top).chain(size - bottom..size) {
            sf.forget(i);
        }
        let copy = sf.pixels.clone();
        self.whoa = Some((vec![0; size], copy));
    }

    /// `JE_whoa`, one turn of the loop (`setDelay(1)` then `JE_showVGA` by
    /// the caller): each pixel = (12 x shifted TempScreen2 + 4 neighbors of
    /// TempScreen1) / 16, computed in place in TempScreen1, copied to
    /// the screen, then swap of the two buffers. Without `whoa_begin`: nothing.
    #[func]
    fn whoa_step(&mut self, surf: i32) {
        let Some((mut t1, mut t2)) = self.whoa.take() else {
            return;
        };
        let size = t1.len();
        let (top, bottom) = (WHOA_TOP_BORDER, WHOA_BOTTOM_BORDER);
        let mut i = size - bottom;
        let mut j = top / 2;
        while i > 0 {
            let offset = j + i / 8192 - 4;
            let color = (t2[offset] as u32 * 12
                + t1[offset - 320] as u32
                + t1[offset - 1] as u32
                + t1[offset + 1] as u32
                + t1[offset + 320] as u32)
                / 16;
            t1[j] = color as u8;
            i -= 1;
            j += 1;
        }
        let sf = self.surf(surf);
        sf.pixels[top..size - bottom + top].copy_from_slice(&t1[top..size - bottom + top]);
        for k in top..size - bottom + top {
            sf.forget(k);
        }
        std::mem::swap(&mut t1, &mut t2);
        self.whoa = Some((t1, t2));
    }

    #[func]
    fn greenify(&mut self, dst: i32, src: i32) {
        let from = self.surfaces[src.clamp(0, 2) as usize].pixels.clone();
        let to = self.surf(dst);
        let v = |i: usize| (from[i] & 0x0f) as i32;
        for y in 0..200usize {
            for x in 0..320usize {
                let i = y * 320 + x;
                let mut avg = 0;
                if y > 0 {
                    avg += v(i - 320);
                }
                if y < 199 {
                    avg += v(i + 320);
                }
                if x > 0 {
                    avg += v(i - 1);
                }
                if x < 319 {
                    avg += v(i + 1);
                }
                if v(i) > avg / 4 {
                    to.pixels[i] = (from[i] & 0x0f) | 0xc0;
                    to.forget(i);
                }
            }
        }
    }

    #[func]
    fn get_pixel(&self, surf: i32, x: i32, y: i32) -> i32 {
        self.surfaces[surf.clamp(0, 2) as usize].get(x, y) as i32
    }

    #[func]
    fn set_pixel(&mut self, surf: i32, x: i32, y: i32, color: i32) {
        if (0..320).contains(&x) && (0..200).contains(&y) {
            let sf = self.surf(surf);
            sf.pixels[y as usize * 320 + x as usize] = color as u8;
            sf.forget(y as usize * 320 + x as usize);
        }
    }

    #[func]
    fn fill_rectangle_xy(&mut self, surf: i32, x: i32, y: i32, x2: i32, y2: i32, color: i32) {
        self.surf(surf).fill_rectangle_xy(x, y, x2, y2, color as u8);
    }

    #[func]
    fn rectangle(&mut self, surf: i32, x: i32, y: i32, x2: i32, y2: i32, color: i32) {
        self.surf(surf).rectangle(x, y, x2, y2, color as u8);
    }

    #[func]
    fn bar_shade(&mut self, surf: i32, x: i32, y: i32, x2: i32, y2: i32) {
        self.surf(surf).bar_shade(x, y, x2, y2);
    }

    #[func]
    fn bar_bright(&mut self, surf: i32, x: i32, y: i32, x2: i32, y2: i32) {
        self.surf(surf).bar_bright(x, y, x2, y2);
    }

    /// Raw indices of the surface (320x200 bytes).
    #[func]
    fn indices(&mut self, surf: i32) -> PackedByteArray {
        let sf = &self.surfaces[surf.clamp(0, 2) as usize];
        if sf.trace.is_some() {
            self.saved_trace = Some((sf.pixels.clone(), sf.clone()));
        }
        PackedByteArray::from(self.surfaces[surf.clamp(0, 2) as usize].pixels.as_slice())
    }

    #[func]
    fn set_indices(&mut self, surf: i32, data: PackedByteArray) {
        let d = data.as_slice();
        let saved = self.saved_trace.as_ref().filter(|(px, _)| px.as_slice() == d).map(|(_, s)| s.trace.clone());
        let sf = self.surf(surf);
        let n = d.len().min(sf.pixels.len());
        sf.pixels[..n].copy_from_slice(&d[..n]);
        if sf.trace.is_some() {
            match saved {
                Some(t) => sf.trace = t,
                None => (0..n).for_each(|i| sf.forget(i)),
            }
        }
    }

    // --- Images and palette ---------------------------------------------------

    /// `picload.c:JE_loadPic`: draws image `number` (1-based) and copies its
    /// palette into `colors`; `storepal` also sets it in the hardware palette.
    #[func]
    fn load_pic(&mut self, surf: i32, number: i32, storepal: bool) -> bool {
        let Some((px, pal)) = with_assets(|a| a.pic(number as usize).map(|(px, p)| (px, a.palettes[p]))).flatten()
        else {
            return false;
        };
        self.surf(surf).pixels.copy_from_slice(&px);
        self.surf(surf).trace_pic(number as u32);
        self.colors = pal;
        if storepal {
            self.palette.set_palette(&self.colors, 0, 255);
        }
        true
    }

    /// `colors = palettes[n]` (palette.dat, 0-based).
    #[func]
    fn load_palette(&mut self, n: i32) {
        if let Some(p) = with_assets(|a| a.palettes.get(n as usize).copied()).flatten() {
            self.colors = p;
        }
    }

    /// `colors[first..=last] = palettes[n][first..=last]` (palette.dat,
    /// 0-based): face of a data cube (game_menu.c, `facepal`).
    #[func]
    fn load_palette_range(&mut self, n: i32, first: i32, last: i32) {
        if let Some(p) = with_assets(|a| a.palettes.get(n as usize).copied()).flatten() {
            let (a, b) = (first.clamp(0, 255) as usize, last.clamp(0, 255) as usize);
            if a <= b {
                self.colors[a..=b].copy_from_slice(&p[a..=b]);
            }
        }
    }

    #[func]
    fn set_palette(&mut self, first: i32, last: i32) {
        let c = self.colors;
        self.palette.set_palette(&c, first as usize, last as usize);
    }

    #[func]
    fn set_colors(&mut self, color: Color, first: i32, last: i32) {
        let c = [color.r8(), color.g8(), color.b8()];
        self.palette.set_colors(c, first as usize, last as usize);
    }

    /// `fade_palette(colors, ...)`: to be called once, then `fade_step` for
    /// steps, steps-1, ..., 1 (one frame presented per step).
    #[func]
    fn fade_palette_init(&mut self, first: i32, last: i32) {
        let c = self.colors;
        self.palette.init_fade_palette(&c, first as usize, last as usize);
    }

    /// `fade_solid` / `fade_black` / `fade_white`.
    #[func]
    fn fade_solid_init(&mut self, color: Color, first: i32, last: i32) {
        self.palette.init_fade_solid([color.r8(), color.g8(), color.b8()], first as usize, last as usize);
    }

    #[func]
    fn fade_step(&mut self, steps: i32, first: i32, last: i32) {
        self.palette.step_fade(steps, first as usize, last as usize);
    }

    /// Current hardware palette (256 x RGB).
    #[func]
    fn palette_bytes(&self) -> PackedByteArray {
        PackedByteArray::from(self.palette.rgb_bytes())
    }

    /// Table color -> nearest palette index (squared RGB distance,
    /// first index on ties), for the index blending
    /// of layer 2 in HD (AI) mode (shaders/hd_layer_blend.gdshader).
    /// 512x512 L8 image: the color quantized to 64 levels (r, g, b) is
    /// at pixel (r + (b % 8) x 64, g + (b / 8) x 64). `palette`: 768 bytes;
    /// `rows`: mask of the allowed rows of 16 colors (high nibble).
    #[func]
    fn nearest_index_lut(palette: PackedByteArray, rows: i32) -> Option<Gd<Image>> {
        let pal = palette.as_slice();
        if pal.len() < 768 || rows & 0xffff == 0 {
            return None;
        }
        let level = |q: usize| ((q * 255 + 31) / 63) as i32;
        let mut out = vec![0u8; 512 * 512];
        for b in 0..64 {
            for g in 0..64 {
                for r in 0..64 {
                    let (cr, cg, cb) = (level(r), level(g), level(b));
                    let mut best = (i32::MAX, 0u8);
                    for (i, c) in pal[..768].as_chunks::<3>().0.iter().enumerate() {
                        if rows & (1 << (i >> 4)) == 0 {
                            continue;
                        }
                        let d = (cr - c[0] as i32).pow(2) + (cg - c[1] as i32).pow(2) + (cb - c[2] as i32).pow(2);
                        if d < best.0 {
                            best = (d, i as u8);
                        }
                    }
                    out[(g + (b / 8) * 64) * 512 + r + (b % 8) * 64] = best.1;
                }
            }
        }
        Image::create_from_data(512, 512, false, Format::L8, &PackedByteArray::from(out))
    }

    /// 320x200 RGB8 image of the surface seen through the hardware palette.
    #[func]
    fn image(&self, surf: i32) -> Option<Gd<Image>> {
        let rgb = tyrian_render::to_rgb(&self.surfaces[surf.clamp(0, 2) as usize], &self.palette);
        Image::create_from_data(320, 200, false, Format::RGB8, &PackedByteArray::from(rgb))
    }

    /// OTFR file (same format as golden_master.c:golden_master_frame)
    /// to compare frame by frame with the C version.
    #[func]
    fn save_otfr(&self, surf: i32, path: GString, tick: i64) -> bool {
        let mut out = Vec::with_capacity(12 + 64000 + 768 + 256000);
        out.extend_from_slice(b"OTFR");
        out.extend_from_slice(&(tick as u32).to_le_bytes());
        out.extend_from_slice(&320u16.to_le_bytes());
        out.extend_from_slice(&200u16.to_le_bytes());
        out.extend_from_slice(&self.surfaces[surf.clamp(0, 2) as usize].pixels);
        out.extend_from_slice(&self.palette.rgb_bytes());
        out.resize(out.len() + 320 * 200 * 4, 0);
        std::fs::write(path.to_string(), out).is_ok()
    }

    // --- Sprites -------------------------------------------------------------

    /// Height and width of a simple-table sprite (0 if absent).
    #[func]
    fn sprite_size(table: i32, index: i32) -> Vector2i {
        with_assets(|a| {
            a.sprite(table as usize, index as usize).map(|s| Vector2i::new(s.width as i32, s.height as i32))
        })
        .flatten()
        .unwrap_or(Vector2i::ZERO)
    }

    #[func]
    fn blit_sprite(&mut self, surf: i32, x: i32, y: i32, table: i32, index: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        sf.tag = crate::hd_ui::simple_tag(table, index);
        with_assets(|a| {
            if let Some(sp) = a.sprite(table as usize, index as usize) {
                sf.blit_sprite(x, y, sp)
            }
        });
        sf.tag = 0;
    }

    #[func]
    fn blit_sprite_blend(&mut self, surf: i32, x: i32, y: i32, table: i32, index: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        sf.tag = crate::hd_ui::simple_tag(table, index);
        with_assets(|a| {
            if let Some(sp) = a.sprite(table as usize, index as usize) {
                sf.blit_sprite_blend(x, y, sp)
            }
        });
        sf.tag = 0;
    }

    #[func]
    #[allow(clippy::too_many_arguments)] // same parameters as the C function
    fn blit_sprite_hv(&mut self, surf: i32, x: i32, y: i32, table: i32, index: i32, hue: i32, value: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        sf.tag = crate::hd_ui::simple_tag(table, index);
        with_assets(|a| {
            if let Some(sp) = a.sprite(table as usize, index as usize) {
                sf.blit_sprite_hv(x, y, sp, hue as u8, value as i8)
            }
        });
        sf.tag = 0;
    }

    #[func]
    #[allow(clippy::too_many_arguments)] // same parameters as the C function
    fn blit_sprite_hv_blend(&mut self, surf: i32, x: i32, y: i32, table: i32, index: i32, hue: i32, value: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        sf.tag = crate::hd_ui::simple_tag(table, index);
        with_assets(|a| {
            if let Some(sp) = a.sprite(table as usize, index as usize) {
                sf.blit_sprite_hv_blend(x, y, sp, hue as u8, value as i8)
            }
        });
        sf.tag = 0;
    }

    #[func]
    fn blit_sprite_dark(&mut self, surf: i32, x: i32, y: i32, table: i32, index: i32, black: bool) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        sf.tag = crate::hd_ui::simple_tag(table, index);
        with_assets(|a| {
            if let Some(sp) = a.sprite(table as usize, index as usize) {
                sf.blit_sprite_dark(x, y, sp, black)
            }
        });
        sf.tag = 0;
    }

    /// Sprite2 sheets: `sheet` = "main:7" ... "main:12" (tables of
    /// tyrian.shp) or file name (`newsh6.shp`). `mode`: 0 normal,
    /// 1 blend, 2 darken, 3 filter (`filter` = tint << 4), 4 clip,
    /// 5 filter_clip; `x2`: 2x2 drawing (`blit_sprite2x2*`).
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn blit_sprite2(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        sheet: GString,
        index: i32,
        mode: i32,
        filter: i32,
        x2: bool,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        let name = sheet.to_string();
        let tag_sheet = name.clone();
        with_assets(|a| {
            let Some(data) = a.sheet(&name) else { return };
            let idx = index.max(0) as u32;
            let parts: &[(i32, i32, u32)] =
                if x2 { &[(0, 0, 0), (12, 0, 1), (0, 14, 19), (12, 14, 20)] } else { &[(0, 0, 0)] };
            for &(dx, dy, di) in parts {
                let (px, py, pi) = (x + dx, y + dy, idx + di);
                sf.tag = crate::hd_ui::sprite2_tag(&tag_sheet, pi);
                match mode {
                    1 => sf.blit_sprite2_blend(px, py, data, pi),
                    2 => sf.blit_sprite2_darken(px, py, data, pi),
                    3 => sf.blit_sprite2_filter(px, py, data, pi, filter as u8),
                    4 => sf.blit_sprite2_clip(px, py, data, pi, None),
                    5 => sf.blit_sprite2_clip(px, py, data, pi, Some(filter as u8)),
                    _ => sf.blit_sprite2(px, py, data, pi),
                }
            }
        });
        sf.tag = 0;
    }

    // --- Texte (font.c, fonthand.c) -------------------------------------------

    /// Glyph indices of `text` in font `font`, after the C's UTF-8 -> CP437
    /// conversion (locale.c:utf8_to_cp437, "ß" -> "ss");
    /// space = -2, character with no glyph = -1.
    #[func]
    fn text_glyphs(text: GString, font: i32) -> PackedInt32Array {
        // Character outside the table: nothing rather than the "?" of to_cp437 (the C
        // lets its raw bytes through, with no glyph either).
        let text = text.to_string();
        let bytes: Vec<u8> = text
            .chars()
            .flat_map(|c| {
                if c != '?' && font::to_cp437(&c.to_string()) == b"?" {
                    vec![0]
                } else {
                    font::to_cp437(&c.to_string())
                }
            })
            .collect();
        with_assets(|a| {
            bytes
                .iter()
                .map(
                    |&c| {
                        if c == b' ' {
                            -2
                        } else {
                            font::font_sprite_id(a, c, font as usize).map_or(-1, |id| id as i32)
                        }
                    },
                )
                .collect::<Vec<i32>>()
        })
        .map(PackedInt32Array::from)
        .unwrap_or_default()
    }

    /// Bytes of the text after `locale.c:utf8_to_cp437` (sorting of achievements,
    /// achievements.c:cmp_ach_name compares these bytes).
    #[func]
    fn cp437_bytes(text: GString) -> PackedByteArray {
        PackedByteArray::from(font::to_cp437(&text.to_string()))
    }

    /// `JE_textWidth`.
    #[func]
    fn text_width(text: GString, font: i32) -> i32 {
        with_assets(|a| font::text_width(a, &s(&text), font as usize)).unwrap_or(0)
    }

    /// `draw_font_hv`; `align`: 0 left, 1 centered, 2 right.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn draw_font_hv(&mut self, surf: i32, x: i32, y: i32, text: GString, font: i32, align: i32, hue: i32, value: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| {
            font::draw_font_hv(sf, a, x, y, &s(&text), font as usize, Align::from_i32(align), hue as u8, value as i8)
        });
    }

    #[func]
    #[allow(clippy::too_many_arguments)]
    fn draw_font_hv_blend(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        text: GString,
        font: i32,
        align: i32,
        hue: i32,
        value: i32,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| {
            font::draw_font_hv_blend(
                sf,
                a,
                x,
                y,
                &s(&text),
                font as usize,
                Align::from_i32(align),
                hue as u8,
                value as i8,
            )
        });
    }

    #[func]
    #[allow(clippy::too_many_arguments)] // same parameters as the C function
    fn draw_font_dark(&mut self, surf: i32, x: i32, y: i32, text: GString, font: i32, align: i32, black: bool) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| font::draw_font_dark(sf, a, x, y, &s(&text), font as usize, Align::from_i32(align), black));
    }

    #[func]
    #[allow(clippy::too_many_arguments)]
    fn draw_font_hv_shadow(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        text: GString,
        font: i32,
        align: i32,
        hue: i32,
        value: i32,
        black: bool,
        dist: i32,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| {
            font::draw_font_hv_shadow(
                sf,
                a,
                x,
                y,
                &s(&text),
                font as usize,
                Align::from_i32(align),
                hue as u8,
                value as i8,
                black,
                dist,
            )
        });
    }

    #[func]
    #[allow(clippy::too_many_arguments)]
    fn draw_font_hv_full_shadow(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        text: GString,
        font: i32,
        align: i32,
        hue: i32,
        value: i32,
        black: bool,
        dist: i32,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| {
            font::draw_font_hv_full_shadow(
                sf,
                a,
                x,
                y,
                &s(&text),
                font as usize,
                Align::from_i32(align),
                hue as u8,
                value as i8,
                black,
                dist,
            )
        });
    }

    /// `JE_dString`.
    #[func]
    fn dstring(&mut self, surf: i32, x: i32, y: i32, text: GString, font: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| font::dstring(sf, a, x, y, &s(&text), font as usize));
    }

    /// `JE_outText`.
    #[func]
    fn out_text(&mut self, surf: i32, x: i32, y: i32, text: GString, colorbank: i32, brightness: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| font::out_text(sf, a, x, y, &s(&text), colorbank as u8, brightness));
    }

    /// `JE_outTextAdjust`.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn out_text_adjust(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        text: GString,
        filter: i32,
        brightness: i32,
        font: i32,
        shadow: bool,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| font::out_text_adjust(sf, a, x, y, &s(&text), filter as u8, brightness, font as usize, shadow));
    }

    /// `JE_outTextAndDarken`.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn out_text_and_darken(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        text: GString,
        colorbank: i32,
        brightness: i32,
        font: i32,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| {
            font::out_text_and_darken(sf, a, x, y, &s(&text), colorbank as u8, brightness as u32, font as usize)
        });
    }

    /// `JE_outTextModify`.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn out_text_modify(&mut self, surf: i32, x: i32, y: i32, text: GString, filter: i32, brightness: i32, font: i32) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| font::out_text_modify(sf, a, x, y, &s(&text), filter as u8, brightness as u8, font as usize));
    }

    /// `JE_textShade` (0 PART_SHADE, 1 FULL_SHADE, 2 DARKEN, 3 TRICK).
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn text_shade(
        &mut self,
        surf: i32,
        x: i32,
        y: i32,
        text: GString,
        colorbank: i32,
        brightness: i32,
        shadetype: i32,
    ) {
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        with_assets(|a| font::text_shade(sf, a, x, y, &s(&text), colorbank as u8, brightness, shadetype));
    }
}
