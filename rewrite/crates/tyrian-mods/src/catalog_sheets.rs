//! Preview sheets of the named original enemies (`tyrian-mods catalog
//! --sheets DIR`): one PNG image per image bank, each enemy drawn
//! with its first image and the suffix of its name (`<theme>_<suffix>`).
//! Drawing goes through the `tyrian-render` blitters (same images as the
//! game); the output is deterministic.

use std::path::Path;

use tyrian_render::font::{draw_font_hv, text_width, Align};
use tyrian_render::{assets::TINY_FONT, Assets, Surface};
use tyrian_sim::world::SHAPE_FILE;

use tyrian_mods::originals::{OriginalEnemy, OriginalNames, BANK_THEMES};

const CELL_W: i32 = 34;
const CELL_H: i32 = 40;
const COLUMNS: i32 = 16;
const HEADER: i32 = 14;
/// Palette index of the background and text tint (game palette).
const BACKGROUND: u8 = 0;
const TEXT_HUE: u8 = 15;

/// Sprite2 sheet of a bank (`playfield.rs:enemy_sheet_name`).
fn sheet_name(bank: u8) -> String {
    match bank {
        21 => "main:10".into(),
        26 => "main:9".into(),
        b => format!("newsh{}.shp", (SHAPE_FILE[b as usize - 1] as char).to_ascii_lowercase()),
    }
}

/// Text in the small font; `_` does not exist there: drawn as a dash.
fn label(sf: &mut Surface, a: &Assets, x: i32, y: i32, text: &str) {
    let mut cx = x;
    for part in text.split_inclusive('_') {
        let word = part.trim_end_matches('_');
        let bytes = word.as_bytes();
        draw_font_hv(sf, a, cx, y, bytes, TINY_FONT, Align::Left, TEXT_HUE, 4);
        cx += text_width(a, bytes, TINY_FONT);
        if part.ends_with('_') {
            sf.fill_rect(cx, y + 6, 4, 1, 0xf6);
            cx += 5;
        }
    }
}

fn write_png(path: &Path, sf: &Surface, palette: &[[u8; 3]; 256]) -> Result<(), String> {
    let mut rgb = Vec::with_capacity(sf.pixels.len() * 3);
    for &p in &sf.pixels {
        rgb.extend_from_slice(&palette[p as usize]);
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), sf.w as u32, sf.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&rgb).map_err(|e| e.to_string())
}

/// Writes `<dir>/<theme>.png` for each bank that has named enemies.
/// Returns the number of sheets written.
pub fn write_sheets(data: &Path, names: &OriginalNames, dir: &Path) -> Result<usize, String> {
    let mut a = Assets::load(data, false)?;
    let palette = a.palettes[tyrian_formats::palette::GAMEPLAY_PALETTE];
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut written = 0;
    for bank in 1..=BANK_THEMES.len() as u8 {
        let theme = BANK_THEMES[bank as usize - 1];
        let list: Vec<(&String, &OriginalEnemy)> = names.enemies.iter().filter(|(_, e)| e.bank == bank).collect();
        if list.is_empty() {
            continue;
        }
        let Some(sheet) = a.sheet(&sheet_name(bank)).map(<[u8]>::to_vec) else {
            continue;
        };
        let rows = (list.len() as i32 + COLUMNS - 1) / COLUMNS;
        let mut sf = Surface::new((CELL_W * COLUMNS) as usize, (HEADER + rows * CELL_H) as usize);
        sf.clear(BACKGROUND);
        label(&mut sf, &a, 2, 3, &format!("{theme}_  (bank {bank}, {} enemies)", list.len()));
        for (k, (name, e)) in list.iter().enumerate() {
            let (col, row) = (k as i32 % COLUMNS, k as i32 / COLUMNS);
            let (x, y) = (col * CELL_W + 5, HEADER + row * CELL_H + 1);
            let g = e.record.egraphic[0] as u32;
            if e.record.esize == 1 {
                for (dx, dy, di) in [(0, 0, 0), (12, 0, 1), (0, 14, 19), (12, 14, 20)] {
                    sf.blit_sprite2(x + dx, y + dy, &sheet, g + di);
                }
            } else {
                sf.blit_sprite2(x + 6, y + 7, &sheet, g);
            }
            let suffix = &name[theme.len() + 1..];
            label(&mut sf, &a, col * CELL_W + 2, y + 30, suffix);
        }
        write_png(&dir.join(format!("{theme}.png")), &sf, &palette)?;
        written += 1;
    }
    Ok(written)
}
