//! Interface in HD mode (AI): x4 image of what a `TyrianVga` surface
//! displays, made of the upscaled images its pixels come from.
//!
//! The drawing stays the C one, pixel for pixel, in the 8-bit surfaces; in
//! HD mode each surface also tracks the provenance of its pixels
//! (`Surface::trace`: image of `tyrian.pic` or sprite, and pixel in that
//! image). `hd_image` composes the 1280x800 image: each tracked pixel there becomes
//! the matching 4x4 block of the upscaled image (`upscaled/pics`,
//! `upscaled/sprites`), recolored if the palette or the index differ (fades,
//! tints, shadows). The other pixels (texts, flat fills) stay transparent:
//! the `hd_interface.gdshader` shader upscales them with xBRZ.

use std::collections::HashMap;
use std::sync::Mutex;

use godot::classes::image::Format;
use godot::classes::{FileAccess, Image, Json};
use godot::prelude::*;
use tyrian_formats::palette::{FACE_PALETTE, GAMEPLAY_PALETTE, PCX_PALETTE};
use tyrian_render::assets::{FACE_SHAPES, FONT_SHAPES, SMALL_FONT_SHAPES, TINY_FONT};
use tyrian_render::surface::{origin_parts, origin_tag, ORIGIN_PIC, ORIGIN_SIMPLE, ORIGIN_SPRITE2};
use tyrian_render::Surface;

use crate::vga::{with_assets, TyrianVga};

const K: usize = 4;

/// Loaded upscaled image: RGBA x4 and original palette of its colors.
struct HdImage {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
    palette: usize,
}

/// Location of a source image in an upscaled image (original pixels).
#[derive(Clone, Copy)]
struct Frame {
    image: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    off_x: i32,
    off_y: i32,
}

#[derive(Default)]
struct Store {
    imported: String,
    upscaled: String,
    images: Vec<HdImage>,
    /// Upscaled file -> index in `images` (None: absent).
    files: HashMap<String, Option<usize>>,
    /// (kind, sheet, atlas palette, frame) -> frame (None: no HD image).
    frames: HashMap<(u64, u32, usize, u32), Option<Frame>>,
    /// Named Sprite2 sheets (`newsh1.shp`) -> number in the tag.
    sheet_ids: Vec<String>,
    /// Christmas mode: the sheets of shots, ships and bonuses come from `tyrianc.shp`.
    xmas: bool,
}

/// Sheets of `tyrian.shp` that `tyrianc.shp` redraws (shots, ships, bonuses).
const CHRISTMAS_SHEETS: [u32; 4] = [7, 8, 9, 11];

/// Enables or disables the Christmas mode of the HD mode: cached images are discarded.
pub fn set_christmas(xmas: bool) {
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(Store::default);
    if s.xmas != xmas {
        s.xmas = xmas;
        s.frames.clear();
    }
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

/// Sprite2 sheet number in the provenance tag: 7..12 for
/// "main:N", 32 and up for files, in order of appearance.
pub fn sprite2_sheet_id(name: &str) -> u32 {
    if let Some(n) = name.strip_prefix("main:").and_then(|n| n.parse::<u32>().ok()) {
        return n;
    }
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(Store::default);
    match s.sheet_ids.iter().position(|n| n == name) {
        Some(i) => 32 + i as u32,
        None => {
            s.sheet_ids.push(name.to_string());
            31 + s.sheet_ids.len() as u32
        }
    }
}

/// Tag of a simple-table sprite; 0 (not tracked) for the fonts,
/// upscaled by xBRZ like the texts.
pub fn simple_tag(table: i32, index: i32) -> u64 {
    if [FONT_SHAPES, SMALL_FONT_SHAPES, TINY_FONT].contains(&(table as usize)) {
        return 0;
    }
    origin_tag(ORIGIN_SIMPLE, table as u32, index as u32)
}

pub fn sprite2_tag(sheet: &str, index: u32) -> u64 {
    origin_tag(ORIGIN_SPRITE2, sprite2_sheet_id(sheet), index)
}

impl Store {
    fn image(&mut self, rel: &str, palette: usize) -> Option<usize> {
        if let Some(&i) = self.files.get(rel) {
            return i;
        }
        let path = format!("{}/{}", self.upscaled, rel);
        let loaded = FileAccess::file_exists(path.as_str())
            .then(|| Image::load_from_file(path.as_str()))
            .flatten()
            .map(|mut img| {
                img.convert(Format::RGBA8);
                HdImage {
                    w: img.get_width() as usize,
                    h: img.get_height() as usize,
                    rgba: img.get_data().to_vec(),
                    palette,
                }
            });
        let i = loaded.map(|im| {
            self.images.push(im);
            self.images.len() - 1
        });
        self.files.insert(rel.to_string(), i);
        i
    }

    /// Name of the imported atlas JSON whose name starts with `prefix`.
    fn atlas_name(&self, sub: &str, prefix: &str) -> Option<String> {
        let dir = format!("{}/sprites/{sub}", self.imported);
        let list = godot::classes::DirAccess::get_files_at(dir.as_str());
        list.as_slice()
            .iter()
            .map(|f| f.to_string())
            .filter(|f| f.starts_with(prefix) && f.ends_with(".json"))
            .min()
            .map(|f| format!("{sub}{}", f.trim_end_matches(".json")))
    }

    fn atlas_frames(&mut self, name: &str, palette: usize) -> HashMap<u32, Frame> {
        let mut out = HashMap::new();
        let Some(image) = self.image(&format!("sprites/{name}.png"), palette) else {
            return out;
        };
        let text = FileAccess::get_file_as_string(format!("{}/sprites/{name}.json", self.imported).as_str());
        let meta = Json::parse_string(&text);
        let Ok(meta) = meta.try_to::<VarDictionary>() else {
            return out;
        };
        let Some(frames) = meta.get("frames").and_then(|v| v.try_to::<VarArray>().ok()) else {
            return out;
        };
        for f in frames.iter_shared() {
            let Ok(f) = f.try_to::<VarDictionary>() else { continue };
            let n = |k: &str| f.get(k).and_then(|v| v.try_to::<f64>().ok()).unwrap_or(0.0) as i32;
            out.insert(
                n("index") as u32,
                Frame { image, x: n("x"), y: n("y"), w: n("w"), h: n("h"), off_x: n("offset_x"), off_y: n("offset_y") },
            );
        }
        out
    }

    fn frame(&mut self, kind: u64, sheet: u32, index: u32, palette: usize) -> Option<Frame> {
        let key = (kind, sheet, palette, index);
        if let Some(f) = self.frames.get(&key) {
            return *f;
        }
        match kind {
            ORIGIN_PIC => {
                let pal = PCX_PALETTE.get(sheet.wrapping_sub(1) as usize).copied().unwrap_or(0) as usize;
                let f = self.image(&format!("pics/pic_{sheet:02}.png"), pal).map(|image| Frame {
                    image,
                    x: 0,
                    y: 0,
                    w: 320,
                    h: 200,
                    off_x: 0,
                    off_y: 0,
                });
                self.frames.insert(key, f);
            }
            ORIGIN_SIMPLE if sheet as usize == FACE_SHAPES => {
                // Faces: one PNG per face, rendered with its palette (facepal).
                let pal = FACE_PALETTE.get(index as usize).copied().unwrap_or(GAMEPLAY_PALETTE as u8) as usize;
                let size =
                    with_assets(|a| a.sprite(FACE_SHAPES, index as usize).map(|s| (s.width as i32, s.height as i32)))
                        .flatten();
                let f = size.and_then(|(w, h)| {
                    self.image(&format!("sprites/main_faces/face_{index:02}.png"), pal).map(|image| Frame {
                        image,
                        x: 0,
                        y: 0,
                        w,
                        h,
                        off_x: 0,
                        off_y: 0,
                    })
                });
                self.frames.insert(key, f);
            }
            _ => {
                let prefix = match kind {
                    _ if sheet < 32 && self.xmas && CHRISTMAS_SHEETS.contains(&sheet) => {
                        format!("main_christmas_{sheet:02}_")
                    }
                    _ if sheet < 32 => format!("main_{sheet:02}_"),
                    _ => {
                        let name = self.sheet_ids.get((sheet - 32) as usize).cloned().unwrap_or_default();
                        name.trim_end_matches(".shp").to_ascii_lowercase()
                    }
                };
                // Atlas exported in another palette: sprites/palNN/.
                let dir = if palette == GAMEPLAY_PALETTE { String::new() } else { format!("pal{palette:02}/") };
                // Whole sheet at once: a missing frame is None.
                let name = self.atlas_name(&dir, &prefix);
                let frames = name.map(|n| self.atlas_frames(&n, palette)).unwrap_or_default();
                let count = frames.keys().max().copied().unwrap_or(0).max(index);
                for i in 0..=count {
                    self.frames.insert((kind, sheet, palette, i), frames.get(&i).copied());
                }
            }
        }
        self.frames.get(&key).copied().flatten()
    }
}

/// Covered pixel: frame and position in its upscaled image, difference between
/// its displayed color and its atlas color.
struct Under {
    frame: Frame,
    x: i32,
    y: i32,
    shift: [f32; 3],
}

impl Under {
    fn sample(&self, store: &Store, dx: usize, dy: usize) -> Option<[f32; 3]> {
        let img = &store.images[self.frame.image];
        let (sy, sx) = ((self.frame.y + self.y) as usize * K + dy, (self.frame.x + self.x) as usize * K + dx);
        if sy >= img.h || sx >= img.w {
            return None;
        }
        let s = (sy * img.w + sx) * 4;
        (img.rgba[s + 3] >= 128).then(|| [0, 1, 2].map(|ch| img.rgba[s + ch] as f32 + self.shift[ch]))
    }
}

/// Green labels embedded in the dashboard frames (FRONT GUN, REAR
/// GUN, MODE, SHIELD, ARMOR; pic 3: 1 player, pic 6: 2 players), in pixels
/// of the image (box of the green pixels), on the black screen. The model renders them blurry and doubled:
/// they go through xBRZ like the other texts.
const PIC_LABELS: &[(u32, [i32; 4])] = &[
    (3, [285, 5, 308, 15]),
    (3, [285, 26, 302, 36]),
    (3, [284, 54, 302, 58]),
    (3, [285, 138, 289, 172]),
    (3, [296, 138, 300, 166]),
    (6, [285, 17, 289, 51]),
    (6, [296, 17, 300, 45]),
    (6, [272, 127, 277, 142]),
    (6, [285, 151, 289, 185]),
    (6, [296, 151, 300, 179]),
];

fn in_pic_label(sheet: u32, x: i32, y: i32) -> bool {
    PIC_LABELS.iter().any(|&(p, [x0, y0, x1, y1])| p == sheet && x >= x0 && x <= x1 && y >= y0 && y <= y1)
}

/// Palettes in which the atlas of a simple table exists (tyrian-import,
/// INTERFACE_PALETTES): the game's, then those of the interface screens.
fn atlas_palettes(kind: u64, sheet: u32) -> Vec<usize> {
    let extra: &[(u32, usize)] = &[(3, 8), (3, 17), (5, 0), (6, 0)];
    // Planets: the C never draws them in the game palette (navigation
    // map: palette 17; title logo: 8); the atlas of that palette
    // is not upscaled.
    let mut v = if kind == ORIGIN_SIMPLE && sheet == 3 { vec![] } else { vec![GAMEPLAY_PALETTE] };
    if kind == ORIGIN_SIMPLE {
        v.extend(extra.iter().filter(|e| e.0 == sheet).map(|e| e.1));
    }
    v
}

/// 4x4 block of the `out` image (1280x800) for the tracked pixels of `sf`.
/// `chosen`: atlas selected per drawing (`choose_palettes`); `hw`: displayed
/// palette; `palettes`: palettes of palette.dat.
///
/// The upscaled image has the colors of its atlas palette; the screen may
/// display others (other palette, fade, tint, shadow). Each upscaled
/// pixel is related to the neighboring original pixel (3x3, same drawing) whose
/// atlas color is the closest, and shifted by the difference between the
/// displayed color and the atlas color of that neighbor: with no difference, the upscaled image
/// is rendered as is; otherwise its detail follows the displayed colors.
///
/// `dirty`: pixels to recompose (None: all); the blocks of the other pixels
/// are not touched. A block depends on its pixel and its 8 neighbors:
/// the caller marks the neighbors of the changed pixels.
///
/// Each pixel written is `base + hw[index]` (`Recipe`): a fade, which only
/// changes `hw`, is replayed through `recolor` without recomposing.
fn compose(
    store: &mut Store,
    sf: &Surface,
    chosen: &HashMap<u64, usize>,
    hw: &[[u8; 3]; 256],
    palettes: &[[[u8; 3]; 256]],
    recipe: &mut Recipe,
    dirty: Option<&[bool]>,
) {
    let n = sf.pixels.len();
    let (w, h) = (sf.w as i32, sf.h as i32);
    // Pixels whose data is used: those to recompose and their neighbors.
    let need: Option<Vec<bool>> = dirty.map(|d| dilate(d, sf.w, sf.h));
    // Per original pixel: drawing and atlas color.
    let mut drawing = vec![0u64; n];
    let mut atlas = vec![[0f32; 3]; n];
    let mut cells = Vec::new();
    // Frame of the last drawing encountered: neighboring pixels almost always
    // come from the same one (a lookup in `store.frames` per pixel used to cost
    // most of the composition).
    let mut last: Option<(u64, Option<Frame>)> = None;
    for i in 0..n {
        if need.as_ref().is_some_and(|m| !m[i]) {
            continue;
        }
        let Some((o, src)) = sf.origin_at(i) else { continue };
        let (kind, sheet, index, lx, ly) = origin_parts(o);
        if kind == ORIGIN_PIC && in_pic_label(sheet, lx, ly) {
            continue;
        }
        let d = o & !0xffff_ffff;
        let f = match last {
            Some((k, f)) if k == d => f,
            _ => {
                let f = drawing_frame(store, chosen, d, kind, sheet, index);
                last = Some((d, f));
                f
            }
        };
        let Some(f) = f else { continue };
        let Some(pal) = palettes.get(store.images[f.image].palette) else {
            continue;
        };
        drawing[i] = d;
        atlas[i] = pal[src as usize].map(|v| v as f32);
        if dirty.is_none_or(|m| m[i]) {
            cells.push((i, f, lx, ly));
        }
    }
    let ow = sf.w * K;
    for (i, f, lx, ly) in cells {
        let (x, y) = ((i % sf.w) as i32, (i / sf.w) as i32);
        let (fx, fy) = (lx - f.off_x, ly - f.off_y);
        if fx < 0 || fy < 0 || fx >= f.w || fy >= f.h {
            continue;
        }
        let mut near = [0usize; 9];
        let mut nn = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && nx < w && ny < h {
                    let j = (ny * w + nx) as usize;
                    if drawing[j] == drawing[i] {
                        near[nn] = j;
                        nn += 1;
                    }
                }
            }
        }
        let near = &near[..nn];
        // Covered drawing (background) for the transparent edges of the upscaled block.
        let under_idx = sf.under_at(i).map(|u| u.2).unwrap_or(0);
        let under = sf.under_at(i).and_then(|(uo, usrc, _)| {
            let (k2, s2, i2, ulx, uly) = origin_parts(uo);
            let uf = store.frame(k2, s2, i2, GAMEPLAY_PALETTE)?;
            let upal = palettes.get(store.images[uf.image].palette)?;
            let (ux, uy) = (ulx - uf.off_x, uly - uf.off_y);
            (ux >= 0 && uy >= 0 && ux < uf.w && uy < uf.h).then(|| Under {
                frame: uf,
                x: ux,
                y: uy,
                shift: [0, 1, 2].map(|ch| -(upal[usrc as usize][ch] as f32)),
            })
        });
        let img = &store.images[f.image];
        for dy in 0..K {
            for dx in 0..K {
                let d = (y as usize * K + dy) * ow + x as usize * K + dx;
                let (sy, sx) = ((f.y + fy) as usize * K + dy, (f.x + fx) as usize * K + dx);
                let s = (sy * img.w + sx) * 4;
                // Upscaled outline narrower than the original pixel: covered background
                // or displayed color (otherwise the shader would put
                // dotted xBRZ there).
                let (base, idx) = if sy >= img.h || sx >= img.w || img.rgba[s + 3] < 128 {
                    match under.as_ref().and_then(|u| u.sample(store, dx, dy)) {
                        Some(c) => (c, under_idx),
                        None => ([0.0; 3], sf.pixels[i]),
                    }
                } else {
                    let c = [img.rgba[s] as f32, img.rgba[s + 1] as f32, img.rgba[s + 2] as f32];
                    let dist = |j: usize| (0..3).map(|ch| (c[ch] - atlas[j][ch]).powi(2)).sum::<f32>();
                    let j = near.iter().copied().min_by(|&a, &b| dist(a).total_cmp(&dist(b))).unwrap_or(i);
                    ([0, 1, 2].map(|ch| c[ch] - atlas[j][ch]), sf.pixels[j])
                };
                recipe.base[d] = base.map(|v| v.round().clamp(-255.0, 255.0) as i16);
                recipe.idx[d] = idx;
                recipe.on[d] = true;
                recipe.paint(d, hw);
            }
        }
    }
}

/// Value of `chosen` for a drawing with no atlas in the screen colors: its
/// pixels stay transparent, the shader upscales them with xBRZ.
const NO_ATLAS: usize = usize::MAX;

/// Upscaled frame of a drawing `d`: selected atlas, otherwise that of the game palette.
fn drawing_frame(
    store: &mut Store,
    chosen: &HashMap<u64, usize>,
    d: u64,
    kind: u64,
    sheet: u32,
    index: u32,
) -> Option<Frame> {
    match chosen.get(&d).copied().unwrap_or(GAMEPLAY_PALETTE) {
        NO_ATLAS => None,
        p => store.frame(kind, sheet, index, p).or_else(|| store.frame(kind, sheet, index, GAMEPLAY_PALETTE)),
    }
}

/// Drawings whose upscaled image does not have the displayed colors: fewer than half
/// of their pixels that are neither tinted nor darkened (written index = source
/// index) have, in the atlas palette, the color of `target`. The relief
/// of the atlas, recolored in another palette, retains nothing of the original:
/// the title's Quit box (sprite 35 of table 5, title palette 8, brown atlas
/// of the game palette only) came out embossed and noisy on an
/// almost uniform red. These drawings go through xBRZ, as the C upscales
/// the 8-bit image.
fn mark_foreign_atlases(
    store: &mut Store,
    sf: &Surface,
    chosen: &mut HashMap<u64, usize>,
    target: &[[u8; 3]; 256],
    palettes: &[[[u8; 3]; 256]],
) {
    let mut counts: HashMap<u64, (u32, u32)> = HashMap::new();
    let mut last: Option<(u64, Option<usize>)> = None;
    for i in 0..sf.pixels.len() {
        let Some((o, src)) = sf.origin_at(i) else { continue };
        if sf.pixels[i] != src {
            continue;
        }
        let (kind, sheet, index, lx, ly) = origin_parts(o);
        if kind == ORIGIN_PIC && in_pic_label(sheet, lx, ly) {
            continue;
        }
        let d = o & !0xffff_ffff;
        let pal = match last {
            Some((k, p)) if k == d => p,
            _ => {
                let p = drawing_frame(store, chosen, d, kind, sheet, index).map(|f| store.images[f.image].palette);
                last = Some((d, p));
                p
            }
        };
        let Some(pal) = pal.and_then(|p| palettes.get(p)) else { continue };
        let c = counts.entry(d).or_default();
        c.0 += 1;
        c.1 += (pal[src as usize] == target[src as usize]) as u32;
    }
    for (d, (seen, hits)) in counts {
        if hits * 2 < seen {
            chosen.insert(d, NO_ATLAS);
        }
    }
}

/// Marked pixels and their 8 neighbors.
fn dilate(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut out = vec![false; m.len()];
    for i in (0..m.len()).filter(|&i| m[i]) {
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                    out[ny as usize * w + nx as usize] = true;
                }
            }
        }
    }
    out
}

/// Atlas of each drawing: the one whose palette renders the most of its pixels
/// in the screen's target color (`target`: `colors`, before fade).
fn choose_palettes(sf: &Surface, target: &[[u8; 3]; 256], palettes: &[[[u8; 3]; 256]]) -> HashMap<u64, usize> {
    let mut votes: HashMap<(u64, usize), u32> = HashMap::new();
    for i in 0..sf.pixels.len() {
        let Some((o, src)) = sf.origin_at(i) else { continue };
        let (kind, sheet, ..) = origin_parts(o);
        for p in atlas_palettes(kind, sheet) {
            let hit = palettes.get(p).is_some_and(|pal| pal[src as usize] == target[sf.pixels[i] as usize]);
            *votes.entry((o & !0xffff_ffff, p)).or_default() += hit as u32;
        }
    }
    let mut chosen: HashMap<u64, (usize, u32)> = HashMap::new();
    for (&(d, p), &v) in &votes {
        let e = chosen.entry(d).or_insert((GAMEPLAY_PALETTE, 0));
        if v > e.1 || (v == e.1 && p == GAMEPLAY_PALETTE) {
            *e = (p, v);
        }
    }
    chosen.into_iter().map(|(d, (p, _))| (d, p)).collect()
}

/// Pixels whose index or provenance differs between two states of a surface.
fn changed_pixels(a: &Surface, b: &Surface) -> Vec<bool> {
    let (ta, tb) = (a.trace.as_deref(), b.trace.as_deref());
    (0..b.pixels.len())
        .map(|i| {
            a.pixels[i] != b.pixels[i]
                || match (ta, tb) {
                    (Some(x), Some(y)) => {
                        x.origin[i] != y.origin[i]
                            || x.source[i] != y.source[i]
                            || x.written[i] != y.written[i]
                            || x.under[i] != y.under[i]
                    }
                    (None, None) => false,
                    _ => true,
                }
        })
        .collect()
}

/// Pixels of the HD image: RGBA, and for each written pixel its color without the
/// displayed palette (`base`) and the index whose displayed color it takes.
struct Recipe {
    out: Vec<u8>,
    base: Vec<[i16; 3]>,
    idx: Vec<u8>,
    on: Vec<bool>,
}

impl Recipe {
    fn new(n: usize) -> Self {
        Self { out: vec![0; n * 4], base: vec![[0; 3]; n], idx: vec![0; n], on: vec![false; n] }
    }

    fn paint(&mut self, d: usize, hw: &[[u8; 3]; 256]) {
        let c = hw[self.idx[d] as usize];
        for (ch, &v) in c.iter().enumerate() {
            self.out[d * 4 + ch] = (self.base[d][ch] + v as i16).clamp(0, 255) as u8;
        }
        self.out[d * 4 + 3] = 255;
    }

    /// 4x4 block of original pixel `i` cleared (transparent: xBRZ).
    fn clear_block(&mut self, i: usize, w: usize) {
        let (x, y) = (i % w, i / w);
        for dy in 0..K {
            let d = (y * K + dy) * w * K + x * K;
            self.on[d..d + K].fill(false);
            self.out[d * 4..(d + K) * 4].fill(0);
        }
    }

    /// All written pixels replayed in the displayed palette `hw` (fade).
    fn recolor(&mut self, hw: &[[u8; 3]; 256]) {
        for d in 0..self.on.len() {
            if self.on[d] {
                self.paint(d, hw);
            }
        }
    }
}

/// Last HD image composed for a surface (`hd_image`) and what it depends on.
pub struct HdCache {
    part: Surface,
    hw: [[u8; 3]; 256],
    colors: [[u8; 3]; 256],
    chosen: HashMap<u64, usize>,
    recipe: Recipe,
}

#[godot_api(secondary)]
impl TyrianVga {
    /// Folders of the imported images (atlas JSON) and upscaled images: to be called
    /// before `hd_image`.
    #[func]
    fn hd_set_dirs(imported: GString, upscaled: GString) {
        let mut g = STORE.lock().unwrap();
        let s = g.get_or_insert_with(Store::default);
        if s.imported != imported.to_string() || s.upscaled != upscaled.to_string() {
            let ids = std::mem::take(&mut s.sheet_ids);
            let xmas = s.xmas;
            *s = Store {
                xmas,
                imported: imported.to_string(),
                upscaled: upscaled.to_string(),
                sheet_ids: ids,
                ..Default::default()
            };
        }
    }

    /// xBRZ sources of a transparent layer (HD mode): opaque colors
    /// (transparent pixel = color of an opaque neighbor, no black fringe)
    /// and grayscale transparency, upscaled separately by the shader.
    #[func]
    fn hd_xbrz_sources(img: Gd<Image>) -> VarArray {
        let mut img = img.duplicate_resource().cast::<Image>();
        img.convert(Format::RGBA8);
        let (w, h) = (img.get_width() as usize, img.get_height() as usize);
        let px = img.get_data().to_vec();
        let mut rgb = vec![0u8; w * h * 3];
        let mut alpha = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                alpha[i] = px[i * 4 + 3];
                let mut src = i;
                if px[i * 4 + 3] == 0 {
                    'near: for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            let j = ny as usize * w + nx as usize;
                            if px[j * 4 + 3] != 0 {
                                src = j;
                                break 'near;
                            }
                        }
                    }
                }
                rgb[i * 3..i * 3 + 3].copy_from_slice(&px[src * 4..src * 4 + 3]);
            }
        }
        let mut out = VarArray::new();
        out.push(
            &Image::create_from_data(w as i32, h as i32, false, Format::RGB8, &PackedByteArray::from(rgb)).to_variant(),
        );
        out.push(
            &Image::create_from_data(w as i32, h as i32, false, Format::L8, &PackedByteArray::from(alpha)).to_variant(),
        );
        out
    }

    /// Enables provenance tracking of the three surfaces (HD mode).
    #[func]
    fn hd_enable_trace(&mut self) {
        for s in self.surfaces.iter_mut() {
            s.enable_trace();
        }
    }

    /// 1280x800 RGBA image of surface `surf`: tracked pixels rendered from
    /// the upscaled images, transparent elsewhere. Only the pixels of the
    /// rectangle `keep` (x0, y0, x1, y1, original pixels) outside the rectangle
    /// `skip` are composed (`skip` empty: x1 <= x0). Null if nothing changed
    /// since the last call for this surface (indices, palette,
    /// provenance): the caller keeps its texture.
    ///
    /// Only the blocks of changed pixels and their neighbors are recomposed
    /// (a cursor, a digit); a fade recolors the blocks without
    /// recomposing them; everything is recomposed if the target palette changes or if a
    /// drawing changes atlas. The result is the same as a full composition.
    #[func]
    fn hd_image(&mut self, surf: i32, keep: Rect2i, skip: Rect2i) -> Option<Gd<Image>> {
        let si = surf.clamp(0, 2) as usize;
        let sf = &self.surfaces[si];
        let inside = |r: &Rect2i, x: i32, y: i32| {
            x >= r.position.x && y >= r.position.y && x < r.position.x + r.size.x && y < r.position.y + r.size.y
        };
        let mut part = sf.clone();
        if let Some(t) = part.trace.as_mut() {
            for (i, o) in t.origin.iter_mut().enumerate() {
                let (x, y) = ((i % sf.w) as i32, (i / sf.w) as i32);
                if !inside(&keep, x, y) || inside(&skip, x, y) {
                    *o = 0;
                }
            }
        }
        let (hw, colors) = (self.palette.colors, self.colors);
        let palettes = with_assets(|a| a.palettes.clone()).unwrap_or_default();
        let n = part.w * K * part.h * K;
        let mut g = STORE.lock().unwrap();
        let store = g.as_mut()?;
        let mut chosen = choose_palettes(&part, &colors, &palettes);
        mark_foreign_atlases(store, &part, &mut chosen, &colors, &palettes);
        let (mut recipe, dirty, old_hw) = match self.hd_cache[si].take() {
            Some(c) if c.colors == colors && c.chosen == chosen && c.recipe.on.len() == n => {
                let changed = changed_pixels(&c.part, &part);
                if c.hw == hw && !changed.iter().any(|&v| v) {
                    self.hd_cache[si] = Some(c);
                    return None;
                }
                (c.recipe, Some(dilate(&changed, part.w, part.h)), Some(c.hw))
            }
            _ => (Recipe::new(n), None, None),
        };
        if let Some(d) = dirty.as_ref() {
            for i in (0..d.len()).filter(|&i| d[i]) {
                recipe.clear_block(i, part.w);
            }
        }
        compose(store, &part, &chosen, &hw, &palettes, &mut recipe, dirty.as_deref());
        drop(g);
        // Displayed palette changed (fade): the other blocks too.
        if old_hw.is_some_and(|o| o != hw) {
            recipe.recolor(&hw);
        }
        let img = Image::create_from_data(
            (part.w * K) as i32,
            (part.h * K) as i32,
            false,
            Format::RGBA8,
            &PackedByteArray::from(recipe.out.as_slice()),
        );
        self.hd_cache[si] = Some(HdCache { part, hw, colors, chosen, recipe });
        img
    }
}
