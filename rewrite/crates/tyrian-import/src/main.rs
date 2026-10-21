//! `tyrian-import`: converts the original Tyrian 2000 data into
//! resources ready for Godot (PNG, JSON, WAV).
//!
//! Output (deterministic: no timestamps, fixed order):
//! - `palettes.json`
//! - `tiles/<shapesX>.png|.json`: atlas of the 24x28 tiles
//! - `sprites/<sheet>.png|.json`: tables of `tyrian.shp`, `newsh?.shp`, `estsc.shp`
//! - `pics/pic_NN.png`: images of `tyrian.pic`
//! - `levels/ep<N>_<NN>.json`, `episodes/ep<N>.json`
//! - `items.json`
//! - `sfx/<n>.wav` (+ `sfx/christmas/`), `sfx/index.json`

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Serialize;
use serde_json::{json, Value};

use tyrian_formats::episode::EpisodeScript;
use tyrian_formats::hdt::{self, HdtFile, ItemTables};
use tyrian_formats::lvl::{self, LvlFile};
use tyrian_formats::palette::{self, Palette, PaletteFile};
use tyrian_formats::pic::PicFile;
use tyrian_formats::snd::{self, SoundFile};
use tyrian_formats::sprite::{self, IndexedImage, MainShapes, Sprite2Sheet, SpriteTable};
use tyrian_formats::tiles::{self, TileSet};

#[derive(Parser, Debug)]
#[command(about = "Converts the Tyrian 2000 data to PNG/JSON/WAV for Godot")]
struct Args {
    /// Directory of the original data (contains tyrian1.lvl).
    #[arg(long)]
    data: PathBuf,
    /// Output directory (created if needed).
    #[arg(long)]
    out: PathBuf,
    /// Number of tiles per row in the tile atlases.
    #[arg(long, default_value_t = 20)]
    tile_columns: usize,
    /// Maximum width of the sprite atlases (pixels).
    #[arg(long, default_value_t = 512)]
    sheet_width: usize,
}

struct Ctx {
    data: PathBuf,
    out: PathBuf,
    palettes: PaletteFile,
    files_written: usize,
}

impl Ctx {
    fn read(&self, name: &str) -> Result<Vec<u8>> {
        fs::read(self.data.join(name)).with_context(|| format!("reading {name}"))
    }

    fn write_bytes(&mut self, rel: &str, bytes: &[u8]) -> Result<()> {
        let p = self.out.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&p, bytes).with_context(|| format!("writing {}", p.display()))?;
        self.files_written += 1;
        Ok(())
    }

    fn write_json<T: Serialize>(&mut self, rel: &str, v: &T) -> Result<()> {
        let mut s = serde_json::to_string_pretty(v)?;
        s.push('\n');
        self.write_bytes(rel, s.as_bytes())
    }

    fn write_png(&mut self, rel: &str, width: usize, height: usize, rgba: &[u8]) -> Result<()> {
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, width as u32, height as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header()?;
            w.write_image_data(rgba)?;
        }
        self.write_bytes(rel, &buf)
    }

    fn palette(&self, i: usize) -> &Palette {
        &self.palettes.palettes[i]
    }
}

/// Godot-safe file name: non-alphanumeric characters encoded `xHH`.
fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c.to_string() } else { format!("x{:02x}", c as u32) })
        .collect()
}

// ---------------------------------------------------------------------------
// Sprite atlases (shelf packing, sprite order preserved)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Frame {
    /// Index of the sprite in its table (0-based for simple tables,
    /// 1-based for Sprite2 sheets, as in the C code).
    index: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    /// Offset of the top-left corner relative to the C drawing point.
    offset_x: i32,
    offset_y: i32,
}

struct Packed {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    frames: Vec<Frame>,
}

// 16 megapixels: at most 64 MiB of RGBA per atlas, before PNG compression.
const MAX_ATLAS_PIXELS: usize = 16 * 1024 * 1024;

fn pack(images: &[(usize, i32, i32, &IndexedImage)], palette: &Palette, max_width: usize) -> Result<Packed> {
    const PAD: usize = 1;
    let mut placements = Vec::with_capacity(images.len());
    let (mut x, mut y, mut row_h, mut width) = (0usize, 0usize, 0usize, 1usize);
    for &(_, _, _, img) in images {
        let pixels =
            img.width.checked_mul(img.height).filter(|&n| n <= MAX_ATLAS_PIXELS).context("sprite over atlas budget")?;
        if img.width > MAX_ATLAS_PIXELS
            || img.height > MAX_ATLAS_PIXELS
            || img.pixels.len() != pixels
            || img.opaque.len() != pixels
        {
            bail!("sprite: invalid dimensions or buffers");
        }
        if x > 0 && x.checked_add(img.width).context("atlas width out of range")? > max_width {
            x = 0;
            y = y.checked_add(row_h + PAD).context("atlas height out of range")?;
            row_h = 0;
        }
        placements.push((x, y));
        x = x.checked_add(img.width + PAD).context("atlas width out of range")?;
        row_h = row_h.max(img.height);
        width = width.max(x);
    }
    let height = y.checked_add(row_h).context("atlas height out of range")?.max(1);
    let pixels = width
        .checked_mul(height)
        .filter(|&n| n <= MAX_ATLAS_PIXELS)
        .context("atlas exceeds the 16-megapixel budget")?;
    let mut rgba = vec![0u8; pixels * 4];
    let mut frames = Vec::with_capacity(images.len());
    for (&(index, ox, oy, img), &(px, py)) in images.iter().zip(&placements) {
        let src = img.to_rgba(palette);
        for row in 0..img.height {
            let d = ((py + row) * width + px) * 4;
            let s = row * img.width * 4;
            rgba[d..d + img.width * 4].copy_from_slice(&src[s..s + img.width * 4]);
        }
        frames.push(Frame { index, x: px, y: py, w: img.width, h: img.height, offset_x: ox, offset_y: oy });
    }
    Ok(Packed { width, height, rgba, frames })
}

fn export_sprite2(
    ctx: &mut Ctx,
    name: &str,
    source: &str,
    sheet: &Sprite2Sheet,
    pal: usize,
    max_w: usize,
) -> Result<()> {
    let decoded: Vec<_> = (1..=sheet.count()).filter_map(|i| sheet.decode(i).map(|d| (i, d))).collect();
    let images: Vec<_> = decoded.iter().map(|(i, d)| (*i, d.offset_x, d.offset_y, &d.image)).collect();
    let p = pack(&images, ctx.palette(pal), max_w)?;
    ctx.write_png(&format!("sprites/{name}.png"), p.width, p.height, &p.rgba)?;
    ctx.write_json(
        &format!("sprites/{name}.json"),
        &json!({
            "source": source,
            "format": "sprite2",
            "sprite_count": sheet.count(),
            "row_width": sprite::SPRITE2_ROW_WIDTH,
            "palette": pal,
            "image": format!("{name}.png"),
            "width": p.width,
            "height": p.height,
            "frames": p.frames,
        }),
    )
}

fn export_simple(
    ctx: &mut Ctx,
    name: &str,
    source: &str,
    table: &SpriteTable,
    pal: impl Fn(usize) -> usize,
    max_w: usize,
) -> Result<()> {
    // One palette per table for the atlas; the faces each have their own
    // (`facepal`): they are then exported one by one in addition to the atlas.
    // Cumulative check BEFORE keeping the decoded images.
    let mut pixels = 0usize;
    for s in table.entries.iter().filter_map(|e| e.sprite.as_ref()) {
        s.validate_dimensions()?;
        pixels = pixels
            .checked_add(s.width as usize * s.height as usize)
            .filter(|&n| n <= MAX_ATLAS_PIXELS)
            .context("simple table exceeds the atlas budget")?;
    }
    let decoded: Vec<_> = table
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.sprite.as_ref().map(|s| (i, s.decode())))
        .filter(|(_, img)| img.width > 0 && img.height > 0)
        .collect();
    let images: Vec<_> = decoded.iter().map(|(i, img)| (*i, 0, 0, img)).collect();
    let p0 = pal(0);
    let p = pack(&images, ctx.palette(p0), max_w)?;
    ctx.write_png(&format!("sprites/{name}.png"), p.width, p.height, &p.rgba)?;
    ctx.write_json(
        &format!("sprites/{name}.json"),
        &json!({
            "source": source,
            "format": "simple",
            "sprite_count": table.entries.len(),
            "palette": p0,
            "image": format!("{name}.png"),
            "width": p.width,
            "height": p.height,
            "frames": p.frames,
        }),
    )
}

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

fn export_palettes(ctx: &mut Ctx) -> Result<()> {
    let list: Vec<Value> = ctx
        .palettes
        .palettes
        .iter()
        .enumerate()
        .map(|(i, p)| json!({ "index": i, "colors": p.to_rgb8().iter().map(|c| [c.r, c.g, c.b]).collect::<Vec<_>>() }))
        .collect();
    let v = json!({
        "conversion": "c8 = (c6 << 2) | (c6 >> 4)",
        "gameplay_palette": palette::GAMEPLAY_PALETTE,
        "pic_palette": palette::PCX_PALETTE,
        "face_palette": palette::FACE_PALETTE,
        "palettes": list,
    });
    ctx.write_json("palettes.json", &v)
}

fn export_tiles(ctx: &mut Ctx, columns: usize) -> Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(&ctx.data)?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .filter(|n| tyrian_formats::classify(n) == Some(tyrian_formats::FileKind::TileSet))
        .collect();
    names.sort();
    let pal = palette::GAMEPLAY_PALETTE;
    for n in &names {
        let set = TileSet::parse(&ctx.read(n)?)?;
        let (w, h, px) = set.atlas(columns);
        let rgba = palette::indexed_to_rgba(&px, ctx.palette(pal), true);
        let stem = safe_name(n.trim_end_matches(".dat"));
        ctx.write_png(&format!("tiles/{stem}.png"), w, h, &rgba)?;
        let blank: Vec<usize> =
            set.tiles.iter().enumerate().filter(|(_, t)| t.is_blank()).map(|(i, _)| i + 1).collect();
        let fill: Vec<bool> = set.tiles.iter().map(|t| t.fill()).collect();
        ctx.write_json(
            &format!("tiles/{stem}.json"),
            &json!({
                "source": n,
                "image": format!("{stem}.png"),
                "tile_count": set.tiles.len(),
                "columns": columns,
                "rows": set.tiles.len().div_ceil(columns),
                "tile_width": tiles::TILE_WIDTH,
                "tile_height": tiles::TILE_HEIGHT,
                "numbering": "tile n (1-based, mapSh) at (col, row) = ((n-1) % columns, (n-1) / columns)",
                "palette": pal,
                "transparent_index": 0,
                "blank_tiles": blank,
                "fill": fill,
            }),
        )?;
    }
    Ok(names)
}

/// (table of tyrian.shp, palette of palette.dat) of the atlases exported in addition
/// to `sprites/palNN/`: planets and logo (title in palette 8, navigation
/// map in 17), options and weapons (menus in palette 0).
const INTERFACE_PALETTES: &[(usize, usize)] = &[(3, 8), (3, 17), (5, 0), (6, 0)];

fn export_sprites(ctx: &mut Ctx, max_w: usize) -> Result<usize> {
    let pal = palette::GAMEPLAY_PALETTE;
    let mut count = 0;
    for (file, prefix) in [("tyrian.shp", "main"), ("tyrianc.shp", "main_christmas")] {
        let Ok(bytes) = ctx.read(file) else { continue };
        let m = MainShapes::parse(&bytes)?.with_lowercase_s().with_redrawn_x();
        for (i, t) in m.simple.iter().enumerate() {
            let name = format!("{prefix}_{:02}_{}", i, sprite::MAIN_TABLE_NAMES[i]);
            export_simple(ctx, &name, &format!("{file}#{i}"), t, |_| pal, max_w)?;
            count += 1;
            // HD mode of the interface: tables displayed on screens with a
            // different palette (title logo, menus), same atlas in that
            // palette, upscaled separately (hd_upscale.py interface).
            for &(table, p) in INTERFACE_PALETTES {
                if table == i {
                    export_simple(ctx, &format!("pal{p:02}/{name}"), &format!("{file}#{i}"), t, |_| p, max_w)?;
                }
            }
            if i == 4 {
                // Faces: one PNG per face with its palette (`facepal`).
                for (k, e) in t.entries.iter().enumerate() {
                    let Some(s) = &e.sprite else { continue };
                    let img = s.decode();
                    if img.width == 0 || img.height == 0 {
                        continue;
                    }
                    let fp = palette::FACE_PALETTE.get(k).map(|&p| p as usize).unwrap_or(pal);
                    let rgba = img.to_rgba(ctx.palette(fp));
                    ctx.write_png(&format!("sprites/{prefix}_faces/face_{k:02}.png"), img.width, img.height, &rgba)?;
                }
            }
        }
        for (k, s) in m.sheets.iter().enumerate() {
            let i = k + sprite::MAIN_SIMPLE_TABLES;
            let name = format!("{prefix}_{:02}_{}", i, sprite::MAIN_TABLE_NAMES[i]);
            export_sprite2(ctx, &name, &format!("{file}#{i}"), s, pal, max_w)?;
            count += 1;
        }
    }
    let mut names: Vec<String> =
        fs::read_dir(&ctx.data)?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        let lower = n.to_ascii_lowercase();
        match tyrian_formats::classify(&lower) {
            Some(tyrian_formats::FileKind::Sprite2) => {
                let sheet = Sprite2Sheet::parse(&ctx.read(&n)?)?;
                let name = safe_name(lower.trim_end_matches(".shp"));
                export_sprite2(ctx, &name, &lower, &sheet, pal, max_w)?;
                count += 1;
            }
            Some(tyrian_formats::FileKind::ExtraShips) => {
                let f = sprite::ExtraShipsFile::parse(&ctx.read(&n)?)?;
                let name = safe_name(lower.trim_end_matches(".shp"));
                export_sprite2(ctx, &name, &lower, &f.sheet, pal, max_w)?;
                count += 1;
            }
            Some(tyrian_formats::FileKind::SpriteTable) => {
                let t = SpriteTable::parse(&ctx.read(&n)?)?;
                let name = safe_name(lower.trim_end_matches(".shp"));
                export_simple(ctx, &name, &lower, &t, |_| pal, max_w)?;
                count += 1;
            }
            _ => {}
        }
    }
    Ok(count)
}

fn export_pics(ctx: &mut Ctx) -> Result<usize> {
    let p = PicFile::parse(&ctx.read("tyrian.pic")?)?;
    let mut list = Vec::new();
    for i in 0..p.entries.len() {
        let (px, _) = p.decode(i)?;
        let pal = PicFile::palette_index(i).unwrap_or(0);
        let rgba = palette::indexed_to_rgba(&px, ctx.palette(pal), false);
        let name = format!("pic_{:02}.png", i + 1);
        ctx.write_png(&format!("pics/{name}"), 320, 200, &rgba)?;
        list.push(json!({ "number": i + 1, "image": name, "palette": pal }));
    }
    ctx.write_json("pics/index.json", &json!({ "numbering": "1-based (]P, JE_loadPic)", "pics": list }))?;
    Ok(p.entries.len())
}

fn layer_json(map: &lvl::LevelMap, layer: usize, tileset: &TileSet) -> Value {
    let (w, h) = lvl::LAYER_DIMS[layer];
    let cells = map.layer(layer);
    let blank = |n: u16| tileset.tile(n).map(|t| t.is_blank()).unwrap_or(true);
    let tiles: Vec<u16> = cells.iter().map(|&c| map.resolved_tile(layer, c, blank).unwrap_or(0)).collect();
    json!({
        "width": w,
        "height": h,
        "order": "row-major, row 0 = first row of the file",
        "tiles": tiles,
        "cells": cells,
    })
}

fn export_levels(ctx: &mut Ctx) -> Result<usize> {
    let mut count = 0;
    for ep in 1..=5u8 {
        let file = format!("tyrian{ep}.lvl");
        let Ok(bytes) = ctx.read(&file) else { continue };
        let lvl = LvlFile::parse(&bytes)?;
        for (i, level) in lvl.levels.iter().enumerate() {
            let h = &level.events.header;
            let shapes = h.shapes_file_name();
            let tileset = TileSet::parse(&ctx.read(&shapes)?)?;
            let tileset_name = safe_name(shapes.trim_end_matches(".dat"));
            let v = json!({
                "episode": ep,
                "lvl_file_num": i + 1,
                "source": file,
                "header": {
                    "map_file": (h.map_file as char).to_string(),
                    "shape_file": (h.shape_file as char).to_string(),
                    "map_x": h.map_x,
                    "map_x2": h.map_x2,
                    "map_x3": h.map_x3,
                },
                "tileset": tileset_name,
                "tileset_file": shapes,
                "tile_values": "1-based tile number in the tileset, 0 = empty (JE_loadMap rules applied)",
                "map_sh": level.map.map_sh.iter().map(|l| l.to_vec()).collect::<Vec<_>>(),
                "layers": [
                    layer_json(&level.map, 0, &tileset),
                    layer_json(&level.map, 1, &tileset),
                    layer_json(&level.map, 2, &tileset),
                ],
                "enemies": level.events.enemies,
                "events": level.events.events,
            });
            ctx.write_json(&format!("levels/ep{ep}_{:02}.json", i + 1), &v)?;
            count += 1;
        }
    }
    Ok(count)
}

fn export_episodes(ctx: &mut Ctx) -> Result<usize> {
    let mut count = 0;
    for ep in 1..=5u8 {
        let file = format!("levels{ep}.dat");
        let Ok(bytes) = ctx.read(&file) else { continue };
        let s = EpisodeScript::parse(&bytes)?;
        let v = json!({
            "episode": ep,
            "source": file,
            "sections": s.sections(),
        });
        ctx.write_json(&format!("episodes/ep{ep}.json"), &v)?;
        count += 1;
    }
    Ok(count)
}

fn items_json(t: &ItemTables) -> Result<Value> {
    let mut v = serde_json::to_value(t)?;
    // Adds the C index (`weapons[id]`, `enemyDat[id]`) to each record.
    for (key, id_of) in [("weapons", hdt::weapon_id as fn(usize) -> usize), ("enemies", hdt::enemy_id)] {
        if let Some(arr) = v.get_mut(key).and_then(Value::as_array_mut) {
            for (rec, item) in arr.iter_mut().enumerate() {
                if let Some(o) = item.as_object_mut() {
                    o.insert("id".into(), json!(id_of(rec)));
                }
            }
        }
    }
    for key in ["weapon_ports", "specials", "generators", "ships", "sidekicks", "shields"] {
        if let Some(arr) = v.get_mut(key).and_then(Value::as_array_mut) {
            for (rec, item) in arr.iter_mut().enumerate() {
                if let Some(o) = item.as_object_mut() {
                    o.insert("id".into(), json!(rec));
                }
            }
        }
    }
    Ok(v)
}

fn export_items(ctx: &mut Ctx) -> Result<()> {
    let hdt = HdtFile::parse(&ctx.read("tyrian.hdt")?)?;
    let mut out = serde_json::Map::new();
    out.insert("episodes_1_3".into(), items_json(&hdt.items)?);
    for ep in [4u8, 5] {
        if let Ok(bytes) = ctx.read(&format!("tyrian{ep}.lvl")) {
            if let Some(t) = LvlFile::parse(&bytes)?.item_tables() {
                out.insert(format!("episode_{ep}"), items_json(&t?)?);
            }
        }
    }
    ctx.write_json("items.json", &Value::Object(out))
}

fn export_sfx(ctx: &mut Ctx) -> Result<usize> {
    let mut list = Vec::new();
    let mut count = 0;
    // C 1-based numbering: 1..31 = tyrian.snd, 32..40 = voices (`SFX_COUNT + vi`).
    let sources: [(&str, &str, usize, bool); 3] = [
        ("tyrian.snd", "sfx", 1, false),
        ("voices.snd", "sfx", 1 + snd::SFX_COUNT, true),
        ("voicesc.snd", "sfx/christmas", 1 + snd::SFX_COUNT, true),
    ];
    for (file, dir, first, voice) in sources {
        let Ok(bytes) = ctx.read(file) else { continue };
        let f = SoundFile::parse(&bytes)?;
        for i in 0..f.samples.len() {
            let n = first + i;
            let s = f.played_sample(i, voice).unwrap_or(&[]);
            ctx.write_bytes(&format!("{dir}/{n}.wav"), &snd::pcm_s8_to_wav(s, snd::SAMPLE_RATE))?;
            list.push(json!({ "number": n, "file": format!("{dir}/{n}.wav").trim_start_matches("sfx/"), "source": file, "index": i, "samples": s.len() }));
            count += 1;
        }
    }
    ctx.write_json(
        "sfx/index.json",
        &json!({ "sample_rate": snd::SAMPLE_RATE, "format": "WAV PCM 8-bit unsigned mono", "numbering": "1-based (JE_playSampleNum)", "sounds": list }),
    )?;
    Ok(count)
}

fn run(args: Args) -> Result<()> {
    if !args.data.join("tyrian1.lvl").is_file() {
        bail!("{} does not contain tyrian1.lvl", args.data.display());
    }
    if args.tile_columns == 0 || args.sheet_width < 64 {
        bail!("--tile-columns > 0 and --sheet-width ≥ 64 required");
    }
    fs::create_dir_all(&args.out)?;
    let palettes = PaletteFile::parse(&fs::read(args.data.join("palette.dat"))?)?;
    if palettes.palettes.len() < palette::PALETTE_COUNT {
        bail!(
            "palette.dat: {} palettes, at least {} required by the pictures and faces",
            palettes.palettes.len(),
            palette::PALETTE_COUNT
        );
    }
    let mut ctx = Ctx { data: args.data.clone(), out: args.out.clone(), palettes, files_written: 0 };

    export_palettes(&mut ctx)?;
    let tilesets = export_tiles(&mut ctx, args.tile_columns)?;
    let sheets = export_sprites(&mut ctx, args.sheet_width)?;
    let pics = export_pics(&mut ctx)?;
    let levels = export_levels(&mut ctx)?;
    let episodes = export_episodes(&mut ctx)?;
    export_items(&mut ctx)?;
    let sounds = export_sfx(&mut ctx)?;

    println!("tilesets: {}", tilesets.len());
    println!("sprite sheets: {sheets}");
    println!("tyrian.pic pictures: {pics}");
    println!("levels: {levels}");
    println!("episodes: {episodes}");
    println!("sounds: {sounds}");
    println!("files written: {} in {}", ctx.files_written, display(&args.out));
    Ok(())
}

fn display(p: &Path) -> String {
    p.display().to_string()
}

fn main() -> Result<()> {
    run(Args::parse())
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn security_short_palette_rejected_at_import_boundary() {
        let root = std::env::temp_dir().join(format!("tyrian-import-security-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tyrian1.lvl"), []).unwrap();
        for count in [1, 5, 23] {
            fs::write(root.join("palette.dat"), vec![0; 768 * count]).unwrap();
            let err = run(Args { data: root.clone(), out: root.join("out"), tile_columns: 20, sheet_width: 512 })
                .unwrap_err();
            assert!(err.to_string().contains("palette.dat"), "{err:#}");
            assert_eq!(fs::read_dir(root.join("out")).unwrap().count(), 0);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn security_nine_byte_shp_import_returns_error() {
        let root = std::env::temp_dir().join(format!("tyrian-import-shp-security-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tyrian1.lvl"), []).unwrap();
        fs::write(root.join("palette.dat"), vec![0; palette::PALETTE_FILE_SIZE]).unwrap();
        fs::write(root.join("estsc.shp"), [1, 0, 1, 255, 255, 255, 255, 0, 0]).unwrap();
        let err =
            run(Args { data: root.clone(), out: root.join("out"), tile_columns: 20, sheet_width: 512 }).unwrap_err();
        fs::remove_dir_all(root).unwrap();
        assert!(err.to_string().contains("dimensions 65535×65535"), "{err:#}");
    }

    #[test]
    fn security_simple_table_budget_before_decode() {
        let entry = sprite::SimpleEntry {
            populated: 1,
            sprite: Some(sprite::SimpleSprite { width: 320, height: 256, data: vec![] }),
        };
        let table = SpriteTable { entries: vec![entry; 300], tail: vec![] };
        let mut ctx = Ctx {
            data: PathBuf::new(),
            out: PathBuf::new(),
            palettes: PaletteFile { palettes: vec![] },
            files_written: 0,
        };
        let err = export_simple(&mut ctx, "test", "test", &table, |_| 5, 512).unwrap_err();
        assert!(err.to_string().contains("atlas budget"));
        assert_eq!(ctx.files_written, 0);
    }

    #[test]
    fn security_atlas_cumulative_and_padding_budget() {
        let palette = Palette { raw: [[0; 3]; 256] };
        let img = IndexedImage::new(320, 256);
        assert!(pack(&vec![(0, 0, 0, &img); 300], &palette, 512).is_err());
        // A long image and a tall image: small frame area, huge atlas.
        let wide = IndexedImage::new(8192, 1);
        let tall = IndexedImage::new(1, 8192);
        assert!(pack(&[(0, 0, 0, &wide), (1, 0, 0, &tall)], &palette, usize::MAX).is_err());
        let valid = pack(&[(0, 0, 0, &img)], &palette, 512).unwrap();
        assert_eq!(valid.rgba.len(), 321 * 256 * 4);
    }

    #[test]
    fn security_atlas_budget_rejected_before_allocation() {
        let palette = Palette { raw: [[0; 3]; 256] };
        // A dummy image makes it possible to check the rejection without allocating the load.
        let img = IndexedImage { width: 320, height: usize::MAX, pixels: vec![], opaque: vec![] };
        let result = std::panic::catch_unwind(|| pack(&[(0, 0, 0, &img)], &palette, 512));
        assert!(result.is_ok(), "the geometry must be rejected without panicking");
        assert!(result.unwrap().is_err());
    }
}
