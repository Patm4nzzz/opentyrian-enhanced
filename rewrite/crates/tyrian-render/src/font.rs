//! Bitmap fonts of `tyrian.shp`: `fonthand.c` and `font.c`.
//!
//! Text arrives as UTF-8 and is converted to CP437 bytes like the C strings
//! (locale files and `tyrian.hdt` are in CP437).

use crate::assets::{Assets, TINY_FONT};
use crate::surface::Surface;

/// `font.h:Font`: large_font = 0 (FONT_SHAPES), normal_font = 1
/// (SMALL_FONT_SHAPES), small_font = 2 (TINY_FONT) -- same number as the table.
pub type Font = usize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Align::Center,
            2 => Align::Right,
            _ => Align::Left,
        }
    }
}

/// `fonthand.c:font_ascii`.
pub const FONT_ASCII: [i16; 256] = {
    let mut t = [-1i16; 256];
    let rows: [[i16; 16]; 7] = [
        [-1, 26, 33, 60, 61, 62, -1, 32, 64, 65, 63, 84, 29, 83, 28, 80],
        [79, 70, 71, 72, 73, 74, 75, 76, 77, 78, 31, 30, -1, 85, -1, 27],
        [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
        [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 68, 82, 69, -1, -1],
        [-1, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48],
        [49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 66, 81, 67, -1, -1],
        [86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101],
    ];
    let mut r = 0;
    while r < 7 {
        let mut c = 0;
        while c < 16 {
            t[32 + r * 16 + c] = rows[r][c];
            c += 1;
        }
        r += 1;
    }
    let row9: [i16; 16] = [102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117];
    let mut c = 0;
    while c < 16 {
        t[0x90 + c] = row9[c];
        c += 1;
    }
    let row_a: [i16; 9] = [118, 119, 120, 121, 122, 123, 124, 125, 126];
    let mut c = 0;
    while c < 9 {
        t[0xA0 + c] = row_a[c];
        c += 1;
    }
    t
};

/// `fonthand.c:cp437_unaccent`.
fn cp437_unaccent(c: u8) -> u8 {
    match c {
        0x80 => b'C',
        0x81 => b'u',
        0x82..=0x82 => b'e',
        0x83..=0x86 => b'a',
        0x87 => b'c',
        0x88..=0x8A => b'e',
        0x8B..=0x8D => b'i',
        0x8E | 0x8F => b'A',
        0x90 => b'E',
        0x91 => b'a',
        0x92 => b'A',
        0x93..=0x95 => b'o',
        0x96 | 0x97 => b'u',
        0x98 => b'y',
        0x99 => b'O',
        0x9A => b'U',
        0xA0 => b'a',
        0xA1 => b'i',
        0xA2 => b'o',
        0xA3 => b'u',
        0xA4 => b'n',
        0xA5 => b'N',
        0xA8 => b'?',
        0xE1 => b's',
        _ => 0,
    }
}

/// Unicode -> CP437, table of `locale.c:utf8_to_cp437` (locale and
/// mod texts): accented letters of the font, fallback to the bare letter for
/// accented "a" letters (unreadable at 4x6 in the original font, a C choice)
/// and for accented capitals without a glyph, typographic punctuation to
/// ASCII, "°" removed. Outside the table: "?".
pub fn to_cp437(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        let mapped: &[u8] = match c {
            c if (c as u32) < 128 => {
                out.push(c as u8);
                continue;
            }
            'Ç' => &[0x80],
            'ü' => &[0x81],
            'é' => &[0x82],
            'ç' => &[0x87],
            'ê' => &[0x88],
            'ë' => &[0x89],
            'è' => &[0x8A],
            'ï' => &[0x8B],
            'î' => &[0x8C],
            'ì' => &[0x8D],
            'Ä' => &[0x8E],
            'Å' => &[0x8F],
            'æ' => &[0x91],
            'Æ' => &[0x92],
            'ô' => &[0x93],
            'ö' => &[0x94],
            'ò' => &[0x95],
            'û' => &[0x96],
            'ù' => &[0x97],
            'Ö' => &[0x99],
            'Ü' => &[0x9A],
            'í' => &[0xA1],
            'ó' => &[0xA2],
            'ú' => &[0xA3],
            'ñ' => &[0xA4],
            'Ñ' => &[0xA5],
            '¿' => &[0xA8],
            'à' | 'á' | 'â' | 'ä' | 'å' | 'ã' => b"a",
            'À' | 'Á' | 'Â' | 'Ã' => b"A",
            'É' | 'È' | 'Ê' | 'Ë' => b"E",
            'Ì' | 'Î' | 'Í' | 'Ï' => b"I",
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ø' => b"O",
            'Ù' | 'Ú' | 'Û' => b"U",
            'Ý' => b"Y",
            'ß' => b"ss",
            'õ' | 'ð' | 'ø' => b"o",
            'ý' | 'ÿ' => b"y",
            '¡' => b"!",
            '\u{a0}' => b" ",
            '«' | '»' | '\u{201C}' | '\u{201D}' | '\u{201F}' => b"\"",
            '°' => b"",
            '■' | '\u{2010}'..='\u{2015}' => b"-",
            '\u{2018}' | '\u{2019}' | '\u{201B}' => b"'",
            '…' => b"...",
            _ => b"?",
        };
        out.extend_from_slice(mapped);
    }
    out
}

/// `fonthand.c:font_sprite_id`.
pub fn font_sprite_id(a: &Assets, c: u8, font: Font) -> Option<usize> {
    let id = FONT_ASCII[c as usize];
    if id < 0 {
        return None;
    }
    if a.sprite(font, id as usize).is_some() {
        return Some(id as usize);
    }
    let base = cp437_unaccent(c);
    if base == 0 {
        return None;
    }
    let id = FONT_ASCII[base as usize];
    if id >= 0 && a.sprite(font, id as usize).is_some() {
        Some(id as usize)
    } else {
        None
    }
}

fn width_of(a: &Assets, font: Font, id: usize) -> i32 {
    a.sprite(font, id).map(|s| s.width as i32).unwrap_or(0)
}

/// `fonthand.c:JE_textWidth`.
pub fn text_width(a: &Assets, s: &[u8], font: Font) -> i32 {
    let mut x = 0;
    for &c in s {
        if c == b' ' {
            x += 6;
        } else if let Some(id) = font_sprite_id(a, c, font) {
            x += width_of(a, font, id) + 1;
        }
    }
    x
}

fn align_x(a: &Assets, x: i32, s: &[u8], font: Font, align: Align) -> i32 {
    match align {
        Align::Left => x,
        Align::Center => x - text_width(a, s, font) / 2,
        Align::Right => x - text_width(a, s, font),
    }
}

/// `font.c:draw_font_hv` ("~" toggles highlight: value + 4).
#[allow(clippy::too_many_arguments)] // same parameters as the C function
pub fn draw_font_hv(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    font: Font,
    align: Align,
    hue: u8,
    value: i8,
) {
    let mut x = align_x(a, x, s, font, align);
    let mut value = value;
    let mut highlight = false;
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => {
                highlight = !highlight;
                value = if highlight { value.wrapping_add(4) } else { value.wrapping_sub(4) };
            }
            _ => {
                if let Some(id) = font_sprite_id(a, c, font) {
                    let sp = a.sprite(font, id).unwrap();
                    sf.blit_sprite_hv(x, y, sp, hue, value);
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `font.c:draw_font_hv_blend`.
#[allow(clippy::too_many_arguments)] // same parameters as the C function
pub fn draw_font_hv_blend(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    font: Font,
    align: Align,
    hue: u8,
    value: i8,
) {
    let mut x = align_x(a, x, s, font, align);
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => {}
            _ => {
                if let Some(id) = font_sprite_id(a, c, font) {
                    let sp = a.sprite(font, id).unwrap();
                    sf.blit_sprite_hv_blend(x, y, sp, hue, value);
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `font.c:draw_font_dark`.
#[allow(clippy::too_many_arguments)] // same parameters as the C function
pub fn draw_font_dark(sf: &mut Surface, a: &Assets, x: i32, y: i32, s: &[u8], font: Font, align: Align, black: bool) {
    let mut x = align_x(a, x, s, font, align);
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => {}
            _ => {
                if let Some(id) = font_sprite_id(a, c, font) {
                    let sp = a.sprite(font, id).unwrap();
                    sf.blit_sprite_dark(x, y, sp, black);
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `font.c:draw_font_hv_shadow`.
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_shadow(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    font: Font,
    align: Align,
    hue: u8,
    value: i8,
    black: bool,
    dist: i32,
) {
    draw_font_dark(sf, a, x + dist, y + dist, s, font, align, black);
    draw_font_hv(sf, a, x, y, s, font, align, hue, value);
}

/// `font.c:draw_font_hv_full_shadow`.
#[allow(clippy::too_many_arguments)]
pub fn draw_font_hv_full_shadow(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    font: Font,
    align: Align,
    hue: u8,
    value: i8,
    black: bool,
    dist: i32,
) {
    draw_font_dark(sf, a, x, y - dist, s, font, align, black);
    draw_font_dark(sf, a, x + dist, y, s, font, align, black);
    draw_font_dark(sf, a, x, y + dist, s, font, align, black);
    draw_font_dark(sf, a, x - dist, y, s, font, align, black);
    draw_font_hv(sf, a, x, y, s, font, align, hue, value);
}

/// `fonthand.c:JE_dString` (soft shadow at 2 px, tint 15, value -3, "~" +2).
pub fn dstring(sf: &mut Surface, a: &Assets, x: i32, y: i32, s: &[u8], font: Font) {
    let mut x = x;
    let mut bright = 0i8;
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => bright = if bright == 0 { 2 } else { 0 },
            _ => {
                if let Some(id) = font_sprite_id(a, c, font) {
                    let sp = a.sprite(font, id).unwrap();
                    sf.blit_sprite_dark(x + 2, y + 2, sp, false);
                    sf.blit_sprite_hv(x, y, sp, 0xf, -3 + bright);
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `fonthand.c:JE_outText` (small font TINY_FONT; brightness < 0 = black).
pub fn out_text(sf: &mut Surface, a: &Assets, x: i32, y: i32, s: &[u8], colorbank: u8, brightness: i32) {
    let mut x = x;
    let mut bright = 0;
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => bright = if bright == 0 { 4 } else { 0 },
            _ => {
                if let Some(id) = font_sprite_id(a, c, TINY_FONT) {
                    let sp = a.sprite(TINY_FONT, id).unwrap();
                    if brightness >= 0 {
                        sf.blit_sprite_hv(x, y, sp, colorbank, (brightness + bright) as i8);
                    } else {
                        sf.blit_sprite_dark(x, y, sp, true);
                    }
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `fonthand.c:JE_outTextModify`.
#[allow(clippy::too_many_arguments)] // same parameters as the C function
pub fn out_text_modify(sf: &mut Surface, a: &Assets, x: i32, y: i32, s: &[u8], filter: u8, brightness: u8, font: Font) {
    let mut x = x;
    for &c in s {
        if c == b' ' {
            x += 6;
        } else if let Some(id) = font_sprite_id(a, c, font) {
            let sp = a.sprite(font, id).unwrap();
            sf.blit_sprite_hv_blend(x, y, sp, filter, brightness as i8);
            x += sp.width as i32 + 1;
        }
    }
}

/// `fonthand.c:JE_outTextAdjust` (existence tested in TINY_FONT like the C code).
#[allow(clippy::too_many_arguments)]
pub fn out_text_adjust(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    filter: u8,
    brightness: i32,
    font: Font,
    shadow: bool,
) {
    let mut x = x;
    let mut bright = 0;
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => bright = if bright == 0 { 4 } else { 0 },
            _ => {
                if let Some(id) = font_sprite_id(a, c, font) {
                    if a.sprite(TINY_FONT, id).is_none() {
                        continue;
                    }
                    let sp = a.sprite(font, id).unwrap();
                    if shadow {
                        sf.blit_sprite_dark(x + 2, y + 2, sp, false);
                    }
                    sf.blit_sprite_hv(x, y, sp, filter, (brightness + bright) as i8);
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `fonthand.c:JE_outTextAndDarken`.
#[allow(clippy::too_many_arguments)]
pub fn out_text_and_darken(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    colorbank: u8,
    brightness: u32,
    font: Font,
) {
    let mut x = x;
    let mut bright = 0u32;
    for &c in s {
        match c {
            b' ' => x += 6,
            b'~' => bright = if bright == 0 { 4 } else { 0 },
            _ => {
                if let Some(id) = font_sprite_id(a, c, font) {
                    if a.sprite(TINY_FONT, id).is_none() {
                        continue;
                    }
                    let sp = a.sprite(font, id).unwrap();
                    sf.blit_sprite_dark(x + 1, y + 1, sp, false);
                    sf.blit_sprite_hv(x, y, sp, colorbank, (brightness + bright) as i8);
                    x += sp.width as i32 + 1;
                }
            }
        }
    }
}

/// `fonthand.h`: shadow types of `JE_textShade`.
pub const PART_SHADE: i32 = 0;
pub const FULL_SHADE: i32 = 1;
pub const DARKEN: i32 = 2;
pub const TRICK: i32 = 3;

/// `fonthand.c:JE_textShade`.
#[allow(clippy::too_many_arguments)]
pub fn text_shade(
    sf: &mut Surface,
    a: &Assets,
    x: i32,
    y: i32,
    s: &[u8],
    colorbank: u8,
    brightness: i32,
    shadetype: i32,
) {
    match shadetype {
        PART_SHADE => {
            out_text(sf, a, x + 1, y + 1, s, 0, -1);
            out_text(sf, a, x, y, s, colorbank, brightness);
        }
        FULL_SHADE => {
            out_text(sf, a, x - 1, y, s, 0, -1);
            out_text(sf, a, x + 1, y, s, 0, -1);
            out_text(sf, a, x, y - 1, s, 0, -1);
            out_text(sf, a, x, y + 1, s, 0, -1);
            out_text(sf, a, x, y, s, colorbank, brightness);
        }
        DARKEN => out_text_and_darken(sf, a, x + 1, y + 1, s, colorbank, brightness as u32, TINY_FONT),
        TRICK => out_text_modify(sf, a, x, y, s, colorbank, brightness as u8, TINY_FONT),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::to_cp437;

    /// The fallbacks of locale.c:utf8_to_cp437: accented capitals without
    /// a glyph to the bare letter, non-breaking space to space (French
    /// typography), "ß" to two letters.
    #[test]
    fn to_cp437_matches_locale_c_fallbacks() {
        assert_eq!(to_cp437("ÈÌÎÏÒÙ"), b"EIIIOU");
        // Glyph 0x90 (É) has a lowercase body: "TERMINÉ" used to read
        // "TERMINé". Fallback to E, like the other accented capitals.
        assert_eq!(to_cp437("TERMINÉ Échap"), b"TERMINE Echap");
        assert_eq!(to_cp437("Mode\u{a0}:"), b"Mode :");
        assert_eq!(to_cp437("«\u{a0}x\u{a0}»"), b"\" x \"");
        assert_eq!(to_cp437("Größe"), b"Gr\x94sse");
    }
}
