//! 8-bit surface (`SDL_Surface` 8 bpp of `video.c`) and blitters of `sprite.c`.
//!
//! The blitters reproduce the C pointer arithmetic: linear position
//! `y * pitch + x`, the only bounds checked = start and end of the buffer
//! ("does not clip on left or right edges of surface"). A sprite that
//! overflows on the right therefore reappears on the left of the next line, as in C.

use crate::assets::SimpleSprite;

#[derive(Clone)]
pub struct Surface {
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<u8>,
    /// HD mode (AI) of the interface: provenance of each pixel (None: not
    /// tracked). No effect on `pixels`.
    pub trace: Option<Box<OriginTrace>>,
    /// Source of the next tracked blit (`origin_tag`), 0: untracked pixels
    /// (texts, flat fills: upscaled by xBRZ).
    pub tag: u64,
}

/// Provenance of the pixels of a surface: which original image (image of
/// `tyrian.pic` or sprite) and which pixel of that image each pixel comes from.
/// The HD mode of the interface reads the matching upscaled image from it. A pixel
/// is only valid as long as its index is the one the tracked blit wrote
/// (`written`): any untracked drawing (text, rectangle) invalidates it without
/// erasing the trace, which a background copy (`set_indices`) makes valid again.
#[derive(Clone)]
pub struct OriginTrace {
    /// `origin_tag` | local x << 16 | local y (biased by 0x8000).
    pub origin: Vec<u64>,
    /// Index of the pixel in the source image (before tint or blending).
    pub source: Vec<u8>,
    /// Index written into the surface by this blit (or adjusted by bar_shade).
    pub written: Vec<u8>,
    /// Provenance covered by the last tracked blit (origin, source
    /// index, displayed index): edges of an upscaled sprite that are narrower than
    /// its original pixels.
    pub under: Vec<(u64, u8, u8)>,
}

/// Kind of source in `origin_tag` (bits 56..63).
pub const ORIGIN_PIC: u64 = 1;
pub const ORIGIN_SIMPLE: u64 = 2;
pub const ORIGIN_SPRITE2: u64 = 3;
const LOCAL_BIAS: i32 = 0x8000;

/// Source tag: kind, sheet (image, table or Sprite2 sheet),
/// frame in the sheet.
pub fn origin_tag(kind: u64, sheet: u32, index: u32) -> u64 {
    (kind << 56) | ((sheet as u64 & 0xff) << 48) | ((index as u64 & 0xffff) << 32)
}

/// (kind, sheet, frame, local x, local y) of a provenance.
pub fn origin_parts(o: u64) -> (u64, u32, u32, i32, i32) {
    (
        o >> 56,
        ((o >> 48) & 0xff) as u32,
        ((o >> 32) & 0xffff) as u32,
        ((o >> 16) & 0xffff) as i32 - LOCAL_BIAS,
        (o & 0xffff) as i32 - LOCAL_BIAS,
    )
}

impl Surface {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, pixels: vec![0; w * h], trace: None, tag: 0 }
    }

    /// Enables provenance tracking (HD mode of the interface).
    pub fn enable_trace(&mut self) {
        if self.trace.is_none() {
            let n = self.pixels.len();
            self.trace = Some(Box::new(OriginTrace {
                origin: vec![0; n],
                source: vec![0; n],
                written: vec![0; n],
                under: vec![(0, 0, 0); n],
            }));
        }
    }

    /// Valid provenance of pixel `i`: (origin, source index), otherwise None.
    pub fn origin_at(&self, i: usize) -> Option<(u64, u8)> {
        let t = self.trace.as_ref()?;
        let o = t.origin[i];
        (o != 0 && t.written[i] == self.pixels[i]).then_some((o, t.source[i]))
    }

    /// Provenance covered at pixel `i` by the blit that placed it: (origin,
    /// source index, index displayed at that time).
    pub fn under_at(&self, i: usize) -> Option<(u64, u8, u8)> {
        let t = self.trace.as_ref()?;
        self.origin_at(i)?;
        let u = t.under[i];
        (u.0 != 0).then_some(u)
    }

    /// Pixel `pos` redrawn without provenance (text, flat fill).
    pub fn forget(&mut self, pos: usize) {
        if let Some(t) = self.trace.as_mut() {
            t.origin[pos] = 0;
        }
    }

    /// Whole image of `tyrian.pic` placed by `JE_loadPic`.
    pub fn trace_pic(&mut self, number: u32) {
        let w = self.w;
        if let Some(t) = self.trace.as_mut() {
            let tag = origin_tag(ORIGIN_PIC, number, 0);
            for (i, &v) in self.pixels.iter().enumerate() {
                t.origin[i] = tag | local(i % w, i / w);
                t.source[i] = v;
                t.written[i] = v;
            }
        }
    }

    fn record(&mut self, pos: usize, lx: i32, ly: i32, src: u8, old: u8) {
        if self.trace.is_none() {
            return;
        }
        if self.tag == 0 {
            self.forget(pos);
            return;
        }
        if self.tag == SHADE_TAG {
            // Shadow (blit_*_dark, darken): the pixel underneath keeps its provenance,
            // only its index changes.
            self.keep_origin(pos, old);
            return;
        }
        let v = self.pixels[pos];
        if let Some(t) = self.trace.as_mut() {
            t.under[pos] = if t.origin[pos] != 0 && t.written[pos] == old {
                (t.origin[pos], t.source[pos], old)
            } else {
                (0, 0, 0)
            };
            t.origin[pos] = self.tag | local_i(lx, ly);
            t.source[pos] = src;
            t.written[pos] = v;
        }
    }

    pub fn pitch(&self) -> isize {
        self.w as isize
    }

    #[allow(clippy::len_without_is_empty)] // signed length for the C pointer arithmetic
    pub fn len(&self) -> isize {
        self.pixels.len() as isize
    }

    pub fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
        if let Some(t) = self.trace.as_mut() {
            t.origin.fill(0);
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return 0;
        }
        self.pixels[y as usize * self.w + x as usize]
    }

    /// `vga256d.c:fill_rectangle_xy` (inclusive bounds, clipped to the screen
    /// by SDL_FillRect).
    pub fn fill_rectangle_xy(&mut self, x: i32, y: i32, x2: i32, y2: i32, color: u8) {
        self.fill_rect(x, y, x2 - x + 1, y2 - y + 1, color);
    }

    /// `SDL_FillRect` clipped to the surface.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u8) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.w as i32);
        let y1 = (y + h).min(self.h as i32);
        for yy in y0..y1 {
            for xx in x0..x1 {
                let k = yy as usize * self.w + xx as usize;
                self.pixels[k] = color;
                self.forget(k);
            }
        }
    }

    /// `vga256d.c:JE_rectangle`: outline of a rectangle (inclusive bounds),
    /// entirely ignored if a corner is off the screen.
    pub fn rectangle(&mut self, a: i32, b: i32, c: i32, d: i32, color: u8) {
        let (w, h) = (self.w as i32, self.h as i32);
        if !(a >= 0 && b >= 0 && c >= 0 && d >= 0 && a < w && b < h && c < w && d < h) {
            return;
        }
        let p = self.w as i32;
        let mut put = |i: i32| {
            if i >= 0 && (i as usize) < self.pixels.len() {
                self.pixels[i as usize] = color;
                if let Some(t) = self.trace.as_mut() {
                    t.origin[i as usize] = 0;
                }
            }
        };
        for k in 0..=(c - a) {
            put(b * p + a + k);
            put(d * p + a + k);
        }
        let mut i = (b + 1) * p + a;
        while i < d * p + a {
            put(i);
            put(i + (c - a));
            i += p;
        }
    }

    /// `vga256d.c:JE_barShade`.
    pub fn bar_shade(&mut self, a: i32, b: i32, c: i32, d: i32) {
        self.bar_map(a, b, c, d, |v| ((v & 0x0f) >> 1) | (v & 0xf0));
    }

    /// `vga256d.c:JE_barBright`.
    pub fn bar_bright(&mut self, a: i32, b: i32, c: i32, d: i32) {
        self.bar_map(a, b, c, d, |v| ((v & 0x0f) + 2).min(0x0f) + (v & 0xf0));
    }

    fn bar_map(&mut self, a: i32, b: i32, c: i32, d: i32, f: impl Fn(u8) -> u8) {
        // Tracking keeps the provenance: the upscaled image is darkened or
        // brightened like the index.
        let (w, h) = (self.w as i32, self.h as i32);
        if !(a < w && b < h && c < w && d < h) {
            return;
        }
        let p = self.w as i32;
        let width = c - a + 1;
        let mut i = b * p + a;
        while i <= d * p + a {
            for j in 0..width {
                let k = i + j;
                if k >= 0 && (k as usize) < self.pixels.len() {
                    let k = k as usize;
                    let old = self.pixels[k];
                    self.pixels[k] = f(old);
                    self.keep_origin(k, old);
                }
            }
            i += p;
        }
    }

    // --- Simple tables (blit_sprite*) -------------------------------------

    /// Common traversal of simple-table sprites; `op(dst, src)` gives the
    /// new pixel.
    fn blit_simple(&mut self, x: i32, y: i32, s: &SimpleSprite, mut op: impl FnMut(u8, u8) -> u8) {
        let pitch = self.pitch();
        let ul = self.len();
        let width = s.width as isize;
        let mut pos = y as isize * pitch + x as isize;
        let mut x_offset: isize = 0;
        let mut row = 0i32;
        let data = &s.data;
        let mut i = 0usize;
        while i < data.len() {
            match data[i] {
                255 => {
                    i += 1;
                    let n = *data.get(i).unwrap_or(&0) as isize;
                    pos += n;
                    x_offset += n;
                }
                254 => {
                    pos += width - x_offset;
                    x_offset = width;
                }
                253 => {
                    pos += 1;
                    x_offset += 1;
                }
                v => {
                    if pos >= ul {
                        return;
                    }
                    if pos >= 0 {
                        let d = self.pixels[pos as usize];
                        self.pixels[pos as usize] = op(d, v);
                        self.record(pos as usize, x_offset as i32, row, v, d);
                    }
                    pos += 1;
                    x_offset += 1;
                }
            }
            if x_offset >= width {
                pos += pitch - x_offset;
                x_offset = 0;
                row += 1;
            }
            i += 1;
        }
    }

    /// `blit_sprite`.
    pub fn blit_sprite(&mut self, x: i32, y: i32, s: &SimpleSprite) {
        self.blit_simple(x, y, s, |_, v| v);
    }

    /// `blit_sprite_blend`.
    pub fn blit_sprite_blend(&mut self, x: i32, y: i32, s: &SimpleSprite) {
        self.blit_simple(x, y, s, |d, v| (v & 0xf0) | (((d & 0x0f) + (v & 0x0f)) / 2));
    }

    /// `blit_sprite_hv` and `blit_sprite_hv_unsafe`: the Enhanced version of the
    /// two clamps the value to [0, 15] (sprite.c, "clamp" comments).
    pub fn blit_sprite_hv(&mut self, x: i32, y: i32, s: &SimpleSprite, hue: u8, value: i8) {
        let hue = hue.wrapping_shl(4);
        self.blit_simple(x, y, s, |_, v| {
            let n = ((v & 0x0f) as i32 + value as i32).clamp(0, 15);
            hue | n as u8
        });
    }

    /// `blit_sprite_hv_blend` (original Uint8 arithmetic, no clamp).
    pub fn blit_sprite_hv_blend(&mut self, x: i32, y: i32, s: &SimpleSprite, hue: u8, value: i8) {
        let hue = hue.wrapping_shl(4);
        self.blit_simple(x, y, s, |d, v| {
            let mut t = (v & 0x0f).wrapping_add(value as u8);
            if t > 0xf {
                t = if t >= 0x1f { 0 } else { 0xf };
            }
            hue | (((d & 0x0f) + t) / 2)
        });
    }

    /// `blit_sprite_dark`.
    pub fn blit_sprite_dark(&mut self, x: i32, y: i32, s: &SimpleSprite, black: bool) {
        // Text shadow: upscaled together with the text (xBRZ), provenance forgotten.
        let tag = std::mem::replace(&mut self.tag, 0);
        self.blit_simple(x, y, s, |d, _| if black { 0 } else { (d & 0xf0) | ((d & 0x0f) / 2) });
        self.tag = tag;
    }

    // --- Sprite2 (blit_sprite2*) ------------------------------------------

    /// Common traversal of `blit_sprite2`, `_blend`, `_darken`, `_filter`:
    /// the line skip uses `VGAScreen->pitch - 12` (= pitch of the surface
    /// here, all are 320).
    fn blit_s2(&mut self, x: i32, y: i32, sheet: &[u8], index: u32, mut op: impl FnMut(u8, u8) -> u8) {
        if index == 0 || sheet.len() < 4 {
            return;
        }
        let count = sprite2_count(sheet);
        if index as usize > count {
            return;
        }
        let p = (index as usize - 1) * 2;
        let mut i = u16::from_le_bytes([sheet[p], sheet[p + 1]]) as usize;
        let pitch = self.pitch();
        let ul = self.len();
        let mut pos = y as isize * pitch + x as isize;
        // Local coordinates like SpriteSheet::sprite_pixels (provenance).
        let (mut col, mut row) = (0i32, 0i32);
        while i < sheet.len() && sheet[i] != 0x0f {
            let b = sheet[i];
            pos += (b & 0x0f) as isize;
            col += (b & 0x0f) as i32;
            let mut count = (b >> 4) as usize;
            if count == 0 {
                pos += pitch - 12;
                row += 1;
                col -= 12;
            } else {
                while count > 0 {
                    count -= 1;
                    i += 1;
                    let v = *sheet.get(i).unwrap_or(&0);
                    if pos >= ul {
                        return;
                    }
                    if pos >= 0 {
                        let d = self.pixels[pos as usize];
                        self.pixels[pos as usize] = op(d, v);
                        self.record(pos as usize, col, row, v, d);
                    }
                    pos += 1;
                    col += 1;
                }
            }
            i += 1;
        }
    }

    pub fn blit_sprite2(&mut self, x: i32, y: i32, sheet: &[u8], index: u32) {
        // blit_sprite2 rejects index >= count (strict bound, sprite.c).
        if index as usize >= sprite2_count(sheet) {
            return;
        }
        self.blit_s2(x, y, sheet, index, |_, v| v);
    }

    pub fn blit_sprite2_blend(&mut self, x: i32, y: i32, sheet: &[u8], index: u32) {
        if index as usize * 2 > sheet.len() {
            return;
        }
        self.blit_s2(x, y, sheet, index, |d, v| (((v & 0x0f) + (d & 0x0f)) / 2) | (v & 0xf0));
    }

    pub fn blit_sprite2_darken(&mut self, x: i32, y: i32, sheet: &[u8], index: u32) {
        if index as usize * 2 > sheet.len() {
            return;
        }
        self.with_shade_tag(|sf| sf.blit_s2(x, y, sheet, index, |d, _| ((d & 0x0f) / 2) + (d & 0xf0)));
    }

    pub fn blit_sprite2_filter(&mut self, x: i32, y: i32, sheet: &[u8], index: u32, filter: u8) {
        if index as usize * 2 > sheet.len() {
            return;
        }
        self.blit_s2(x, y, sheet, index, |_, v| filter | (v & 0x0f));
    }

    /// `blit_sprite2_clip` / `_filter_clip`: clipping by row and column.
    pub fn blit_sprite2_clip(&mut self, x: i32, y: i32, sheet: &[u8], index: u32, filter: Option<u8>) {
        if index == 0 || sheet.len() < 4 || index as usize * 2 > sheet.len() {
            return;
        }
        let p = (index as usize - 1) * 2;
        let mut i = u16::from_le_bytes([sheet[p], sheet[p + 1]]) as usize;
        let (x0, y0) = (x, y);
        let (mut x, mut y) = (x, y);
        while i < sheet.len() && sheet[i] != 0x0f {
            if y >= self.h as i32 {
                return;
            }
            let skip = (sheet[i] & 0x0f) as i32;
            let fill = (sheet[i] >> 4) as i32;
            x += skip;
            if fill == 0 {
                y += 1;
                x -= 12;
            } else if y >= 0 {
                for _ in 0..fill {
                    i += 1;
                    let v = *sheet.get(i).unwrap_or(&0);
                    if x >= 0 && x < self.w as i32 {
                        let k = y as usize * self.w + x as usize;
                        let d = self.pixels[k];
                        self.pixels[k] = match filter {
                            Some(f) => f | (v & 0x0f),
                            None => v,
                        };
                        self.record(k, x - x0, y - y0, v, d);
                    }
                    x += 1;
                }
            } else {
                i += fill as usize;
                x += fill;
            }
            i += 1;
        }
    }

    pub fn blit_sprite2x2(&mut self, x: i32, y: i32, sheet: &[u8], index: u32) {
        self.blit_sprite2(x, y, sheet, index);
        self.blit_sprite2(x + 12, y, sheet, index + 1);
        self.blit_sprite2(x, y + 14, sheet, index + 19);
        self.blit_sprite2(x + 12, y + 14, sheet, index + 20);
    }
}

/// `tag` of the blits that darken what is underneath without placing their pixels there.
const SHADE_TAG: u64 = u64::MAX;

impl Surface {
    /// Pixel `pos` has just been modified in place (shadow, darkened bar):
    /// its provenance stays valid if it was valid just before.
    fn keep_origin(&mut self, pos: usize, old: u8) {
        let v = self.pixels[pos];
        if let Some(t) = self.trace.as_mut() {
            if t.written[pos] == old {
                t.written[pos] = v;
            }
        }
    }

    fn with_shade_tag(&mut self, f: impl FnOnce(&mut Self)) {
        let tag = self.tag;
        if self.trace.is_some() {
            self.tag = SHADE_TAG;
        }
        f(self);
        self.tag = tag;
    }
}

fn local(x: usize, y: usize) -> u64 {
    local_i(x as i32, y as i32)
}

fn local_i(x: i32, y: i32) -> u64 {
    ((((x + LOCAL_BIAS) as u64) & 0xffff) << 16) | (((y + LOCAL_BIAS) as u64) & 0xffff)
}

/// `sprite.c:sprite2_count`: `offsets[0] / 2`.
pub fn sprite2_count(sheet: &[u8]) -> usize {
    if sheet.len() < 2 {
        return 0;
    }
    let first = u16::from_le_bytes([sheet[0], sheet[1]]) as usize;
    if first == 0 || first > sheet.len() {
        0
    } else {
        first / 2
    }
}
