//! Play area background composed in 8 bits like the C.
//!
//! The scenery layers drawn by Godot TileMapLayers are opaque;
//! the C, on the other hand, writes palette indices into `game_screen` and blends
//! layer 2 by index when `wild` is true (processorType = 4, default
//! value: config.c:JE_initProcessorType / line 742):
//! `backgrnd.c:blit_background_row_blend`. This module replays, for the current
//! frame, `draw_background_1` then `draw_background_2[_blend]`
//! (tyrian2.c:JE_main, `background2over == 0` or `3`) into a
//! 320x200 surface and returns the visible area (x 24..287, y 0..183) as RGBA.
//! Read-only on the simulation state: positions at the time of drawing
//! (`World::drawn_layer`, like `layer_scroll`).

use std::cell::RefCell;
use std::path::Path;
use tyrian_sim::ShipId;

use godot::classes::image::Format;
use godot::classes::Image;
use godot::prelude::*;
use tyrian_formats::lvl::LevelMap;
use tyrian_formats::tiles::{TileSet, TILE_SIZE, TILE_WIDTH};

use crate::vga::with_assets;
use crate::TyrianSim;
use tyrian_render::assets::SimpleSprite;
use tyrian_render::Surface;
use tyrian_sim::world::{StarfieldStar, World};
use tyrian_sim::{LevelSetup, Sim};

/// One half of the image presented by the C (see `Sim::presented_world`):
/// the state at the time tyrian2.c draws these elements.
#[derive(Clone, Copy)]
struct View<'a> {
    w: &'a World,
    setup: &'a LevelSetup,
}

impl<'a> View<'a> {
    /// Scenery, enemies and player shots (head of the iteration).
    fn top(sim: &'a Sim) -> Self {
        Self { w: sim.presented_top_world(), setup: sim.setup() }
    }

    /// Player, enemy shots, late passes, explosions, texts.
    fn bottom(sim: &'a Sim) -> Self {
        Self { w: sim.presented_world(), setup: sim.setup() }
    }

    fn world(&self) -> &'a World {
        self.w
    }

    fn setup(&self) -> &'a LevelSetup {
        self.setup
    }
}

const PITCH: i32 = 320;
const SURFACE_LEN: i32 = 320 * 200;
const PLAY_AREA_X: usize = 24;
pub(crate) const PLAY_W: usize = 264;
pub(crate) const PLAY_H: usize = 184;
const LAYER_WIDTH: [i32; 3] = [14, 14, 15];

thread_local! {
    /// Tileset of the current level (file name, tiles).
    static TILESET: RefCell<Option<(String, TileSet)>> = const { RefCell::new(None) };
    /// The C `game_screen`, kept from one frame to the next: the image filters
    /// (lava_filter, water_filter, iced_blur_filter, blur_filter) blend the
    /// new image with the previous one.
    static GAME_SCREEN: RefCell<Vec<u8>> = RefCell::new(vec![0u8; SURFACE_LEN as usize]);
    /// `GAME_SCREEN` from before the last `playfield_filters_tick`: the image that
    /// the filters of the current tick blend with the new one (the C composes
    /// only once per tick; each Godot render redoes the composition).
    static GAME_SCREEN_PREV: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    /// `video_rgba.c:advanced_lighting_enabled` (true by default; false with
    /// the Visual FX presets Off and Basic, visual_fx.c:preset_table).
    static ADVANCED_LIGHTING: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// `level_load.c:JE_loadMap`: tile pointer of a cell (`ref[layer][x]`).
fn tile_of<'a>(map: &LevelMap, tiles: &'a TileSet, layer: usize, cell: u8) -> Option<&'a [u8]> {
    let n = map.resolved_tile(layer, cell, |sh| tiles.tile(sh).is_none_or(|t| t.is_blank()))?;
    let t = tiles.tile(n)?;
    // Layer 1: an empty tile is loaded as 672 zeros (memset); it
    // writes nothing (`*data != 0`), same effect as None.
    t.pixels.as_deref()
}

/// `backgrnd.c:blit_background_row` / `blit_background_row_blend`:
/// linear addressing of the surface (a row of 12 tiles that goes past
/// x = 319 continues on the next line, like the C pointer).
fn blit_row(px: &mut [u8], x: i32, y: i32, row: &[Option<&[u8]>; 12], blend: bool) {
    let mut p = y * PITCH + x;
    for ty in 0..28usize {
        if p + 12 * 24 < 0 {
            p += PITCH;
            continue;
        }
        for data in row.iter() {
            let Some(data) = data else {
                p += 24;
                continue;
            };
            let line = &data[ty * TILE_WIDTH..ty * TILE_WIDTH + TILE_WIDTH];
            for &d in line {
                if p >= SURFACE_LEN {
                    return;
                }
                if p >= 0 && d != 0 {
                    let dst = &mut px[p as usize];
                    *dst = if blend { (d & 0xf0) | (((*dst & 0x0f) + (d & 0x0f)) / 2) } else { d };
                }
                p += 1;
            }
        }
        p += PITCH - 12 * 24;
    }
}

/// `draw_background_1/2`: 8 rows (`i = -1..6`) starting from the flat index
/// `ypos + bp - 12` of `megaDataN.mainmap`.
#[allow(clippy::too_many_arguments)] // same parameters as the C function
fn draw_layer(
    px: &mut [u8],
    map: &LevelMap,
    tiles: &TileSet,
    layer: usize,
    ypos: i32,
    bp: i32,
    xpos: i32,
    back: i32,
    blend: bool,
) {
    let cells = map.layer(layer);
    let width = LAYER_WIDTH[layer];
    let mut flat = ypos + bp - 12;
    for i in -1..7 {
        let mut row: [Option<&[u8]>; 12] = [None; 12];
        for (k, slot) in row.iter_mut().enumerate() {
            let f = flat + k as i32;
            if f >= 0 && (f as usize) < cells.len() {
                *slot = tile_of(map, tiles, layer, cells[f as usize]);
            }
        }
        blit_row(px, xpos, i * 28 + back, &row, blend);
        flat += width;
    }
}

/// `draw_background_2` / `draw_background_2_blend` depending on `wild` (true,
/// processorType 4: config.c) and `background2notTransparent`.
fn draw_layer2(px: &mut [u8], sim: View, tiles: &TileSet) {
    let w = sim.world();
    if !background2(w) {
        return;
    }
    let map = &sim.setup().level.map;
    let (y, b) = w.render.drawn_layer[1]
        .map_or((w.background.map_y_pos[1], w.background.back_pos[1] as i32), |(y, b)| (y, b as i32));
    if w.background.background2over == 3 || w.background.background2_not_transparent {
        // draw_background_2: water (smoothies[1]) synchronizes x on layer 1.
        let water = w.background.smoothies[1];
        let (bp, x) = if water {
            (w.background.map_x_bp_pos, w.background.map_x_pos)
        } else {
            (w.background.map_x2_bp_pos, w.background.map_x2_pos)
        };
        draw_layer(px, map, tiles, 1, y, bp, x as i32, b, false);
    } else {
        // draw_background_2_blend only if `wild` (processorType 4 or 6);
        // otherwise draw_background_2: opaque layer 2 (tyrian2.c:1096,1145,1298).
        let blend = tyrian_sim::world::pt_wild(w.processor_type);
        draw_layer(px, map, tiles, 1, y, w.background.map_x2_bp_pos, w.background.map_x2_pos as i32, b, blend);
    }
}

/// Mask of the palette rows (high nibble) that make up at least 2% of the
/// non-zero pixels of the tiles placed on layer 2 (0xffff if empty).
fn layer2_hue_rows(map: &LevelMap, tiles: &TileSet) -> i32 {
    let mut count = [0u64; 16];
    for &cell in map.layer(1) {
        let t = tile_of(map, tiles, 1, cell);
        for &p in t.into_iter().flatten().filter(|&&p| p != 0) {
            count[(p >> 4) as usize] += 1;
        }
    }
    let total: u64 = count.iter().sum();
    let mask = (0..16).filter(|&r| total > 0 && count[r] * 50 >= total).fold(0, |m, r| m | 1 << r);
    if mask == 0 {
        0xffff
    } else {
        mask
    }
}

/// Sprite2 sheet of an enemy (`EnemyView::sheet`) for `Assets::sheet`.
fn enemy_sheet_name(sheet: u8) -> Option<String> {
    match sheet {
        0 => None,
        10 => Some("main:9".into()),  // spriteSheet10
        11 => Some("main:10".into()), // spriteSheet11
        c => Some(format!("newsh{}.shp", (c as char).to_ascii_lowercase())),
    }
}

/// `enemy_render.c:blit_enemy` for the enemies drawn by `JE_drawEnemy`
/// in this iteration (`RenderOnly::drawn_enemies`) whose slot is
/// in `layers` (0 = slots 25..49, 1 = 75..99: ground; 2 = 0..24:
/// sky; 3 = 50..74: top), filter included. The C's list, not the state
/// after the tick: an enemy destroyed later in the loop has already been
/// drawn, an enemy outside the horizontal window is not.
fn draw_enemies(px: &mut [u8], sim: View, layers: &[u8]) {
    let mut surf = Surface::new(320, 200);
    surf.pixels.copy_from_slice(px);
    with_assets(|a| {
        for &layer in layers {
            for (sheet, x, y, idx, mode, filter) in enemy_blits(sim, layer) {
                let Some(name) = enemy_sheet_name(sheet) else { continue };
                let Some(data) = a.sheet(&name) else { continue };
                match mode {
                    EnemyBlit::Darken => surf.blit_sprite2_darken(x, y, data, idx),
                    EnemyBlit::Filter => surf.blit_sprite2_filter(x, y, data, idx, filter),
                    EnemyBlit::Normal => surf.blit_sprite2(x, y, data, idx),
                }
            }
        }
    });
    px.copy_from_slice(&surf.pixels);
}

/// Drawing mode of an enemy piece (values of `hd_enemy_sprites`).
#[derive(Clone, Copy)]
enum EnemyBlit {
    Normal = 0,
    Darken = 2,
    Filter = 4,
}

/// Pieces drawn by `JE_drawEnemy` for layer `layer`, in C order:
/// (sheet byte, x, y, frame, mode, filter).
fn enemy_blits(sim: View, layer: u8) -> Vec<(u8, i32, i32, u32, EnemyBlit, u8)> {
    let w = sim.world();
    let mut out = Vec::new();
    for e in w.render.drawn_enemies.iter().filter(|e| enemy_layer(e.slot) == layer) {
        if e.sheet == 0 {
            continue;
        }
        let ofs = e.mapoffset as i32;
        let idx = e.frame as u32;
        let mut push = |x: i32, y: i32, i: u32, m: EnemyBlit| out.push((e.sheet, x, y, i, m, e.filter));
        // enemy_render.c: darkened trail (Wild) before the enemy,
        // same frames, at trail + tempMapXOfs, same bounds in y.
        if let Some((tx, ty)) = e.trail {
            let (tx, ty) = (tx as i32 + ofs, ty as i32);
            if e.size == 1 {
                if ty > -13 {
                    push(tx - 6, ty - 7, idx, EnemyBlit::Darken);
                    push(tx + 6, ty - 7, idx + 1, EnemyBlit::Darken);
                }
                if ty > -26 && ty < 182 {
                    push(tx - 6, ty + 7, idx + 19, EnemyBlit::Darken);
                    push(tx + 6, ty + 7, idx + 20, EnemyBlit::Darken);
                }
            } else if ty > -13 {
                push(tx, ty, idx, EnemyBlit::Darken);
            }
        }
        // JE_drawEnemy: upper half if ey > -13, lower if -26 < ey < 182.
        let (x, y) = (e.ex as i32 + ofs, e.ey as i32);
        let parts: &[(i32, i32, u32, bool)] = if e.size == 1 {
            &[(-6, -7, 0, true), (6, -7, 1, true), (-6, 7, 19, false), (6, 7, 20, false)]
        } else {
            &[(0, 0, 0, true)]
        };
        let mode = if e.filter != 0 { EnemyBlit::Filter } else { EnemyBlit::Normal };
        for &(dx, dy, di, upper) in parts {
            let shown = if upper { y > -13 } else { y > -26 && y < 182 };
            if shown {
                push(x + dx, y + dy, idx + di, mode);
            }
        }
    }
    // The C draws "trail then enemy" enemy by enemy: one enemy's trail
    // then darkened an enemy already drawn underneath. All the
    // trails of the layer go first, then the enemies (relative order
    // kept within each group).
    let (trails, bodies): (Vec<_>, Vec<_>) = out.into_iter().partition(|b| matches!(b.4, EnemyBlit::Darken));
    trails.into_iter().chain(bodies).collect()
}

/// Drawing layer of an enemy slot (see `draw_enemies`).
fn enemy_layer(slot: u8) -> u8 {
    match slot / 25 {
        0 => 2,
        1 => 0,
        2 => 3,
        _ => 1,
    }
}

/// `draw_background_3`: 8 rows of 15 cells (positions at draw time).
fn draw_layer3(px: &mut [u8], sim: View, tiles: &TileSet) {
    let w = sim.world();
    let (y, b) = w.render.drawn_layer[2]
        .map_or((w.background.map_y_pos[2], w.background.back_pos[2] as i32), |(y, b)| (y, b as i32));
    draw_layer(
        px,
        &sim.setup().level.map,
        tiles,
        2,
        y,
        w.background.map_x3_bp_pos,
        w.background.map_x3_pos as i32,
        b,
        false,
    );
}

/// `varz.c:shadowYDist`.
const SHADOW_Y_DIST: i32 = 10;

/// The C `background2` (see `tyrian_sim::world::pt_background2`).
fn background2(w: &tyrian_sim::world::World) -> bool {
    tyrian_sim::world::pt_background2(w.processor_type, w.background.background2over)
}

/// A player shot to draw, with the ground shadow decision.
struct ShotDraw {
    x: i32,
    y: i32,
    kind: ShotKind,
    /// Darkened shadow at +shadowYDist under the shot.
    shadow: bool,
}

enum ShotKind {
    /// Sprite2 sheet (7: `main:7`, 11: `main:11`) and frame.
    Sheet(u8, u32),
    /// 0-based frame of OPTION_SHAPES, blended with the background.
    OptionShape(usize),
}

/// Player shots in C order, with the shadow decision for each:
/// darkened ground shadow (`background2` true: processorType 4,
/// config.c:JE_initProcessorType) at +shadowYDist if `y + 10 < 190` and
/// `shotXM < 100`; frame > 60000: OPTION_SHAPES blended; > 1000: JE_doSP
/// particles (not rendered here) and frame % 1000.
fn player_shot_draws(w: &tyrian_sim::world::World) -> Vec<ShotDraw> {
    let shadows = background2(w);
    let mut out = Vec::new();
    for s in &w.render.drawn_player_shots {
        let (x, y) = (s.x as i32 + 1, s.y as i32);
        let mut frame = s.frame as u32;
        if frame > 60000 {
            out.push(ShotDraw { x, y, kind: ShotKind::OptionShape((frame - 60001) as usize), shadow: false });
            continue;
        }
        if frame > 1000 {
            frame %= 1000;
        }
        let kind = if frame > 500 { ShotKind::Sheet(11, frame - 500) } else { ShotKind::Sheet(7, frame) };
        out.push(ShotDraw { x, y, kind, shadow: shadows && y + SHADOW_Y_DIST < 190 && s.xm < 100 });
    }
    out
}

/// Places the shots on `surf`: all the ground shadows, then all the shots.
/// `sheets`: Sprite2 of `main:7` and `main:11`; `option_shapes`: OPTION_SHAPES.
///
/// The C draws shot by shot "shadow then shot" (shots.c:player_shot_move_and_draw):
/// a shot's shadow then fell on a shot already drawn 10 px lower, hence dark
/// blotches on overlapping bombs. Here the shadow never covers
/// a shot (the maintainer's decision: no compatibility mode).
fn blit_player_shots(
    surf: &mut Surface,
    draws: &[ShotDraw],
    sheets: [&[u8]; 2],
    option_shapes: &[Option<SimpleSprite>],
) {
    let sheet = |n: u8| if n == 11 { sheets[1] } else { sheets[0] };
    for d in draws.iter().filter(|d| d.shadow) {
        if let ShotKind::Sheet(n, idx) = d.kind {
            surf.blit_sprite2_darken(d.x, d.y + SHADOW_Y_DIST, sheet(n), idx);
        }
    }
    for d in draws {
        match d.kind {
            ShotKind::Sheet(n, idx) => surf.blit_sprite2(d.x, d.y, sheet(n), idx),
            ShotKind::OptionShape(n) => {
                if let Some(Some(sp)) = option_shapes.get(n) {
                    surf.blit_sprite_blend(d.x, d.y, sp);
                }
            }
        }
    }
}

/// `tyrian2.c:JE_updatePlayerShots`: player shots and their shadows.
fn draw_player_shots(px: &mut [u8], sim: View) {
    let draws = player_shot_draws(sim.world());
    let mut surf = Surface::new(320, 200);
    surf.pixels.copy_from_slice(px);
    with_assets(|a| {
        let (Some(s7), Some(s11)) = (a.main_sheets.first(), a.main_sheets.get(4)) else { return };
        let shapes = a.tables.get(tyrian_render::assets::OPTION_SHAPES).map_or(&[][..], |t| t.as_slice());
        blit_player_shots(&mut surf, &draws, [s7, s11], shapes);
    });
    px.copy_from_slice(&surf.pixels);
}

/// Bits of `playfield_parts`: what `playfield_background` has composed.
const PART_LAYER2: i32 = 1;
const PART_LAYER3: i32 = 2;
const PART_SKY: i32 = 4;
const PART_TOP: i32 = 8;
const PART_PLAYER_SHOTS: i32 = 16;

fn parts(_sim: View) -> i32 {
    // The whole 8-bit image of the play area is composed here (layers 1-3,
    // enemies, shots, ships, explosions).
    PART_PLAYER_SHOTS | PART_LAYER2 | PART_LAYER3 | PART_SKY | PART_TOP | PART_PLAYER | PART_LATE
}

#[godot_api(secondary)]
impl TyrianSim {
    /// True if `playfield_background` also contains layer 2
    /// (`background2over` 0, 1 or 3: drawn before the sky enemies;
    /// 2 = after the explosions, left to the Godot nodes).
    #[func]
    fn playfield_has_layer2(&self) -> bool {
        self.sim.as_ref().is_some_and(|s| parts(View::bottom(s)) & PART_LAYER2 != 0)
    }

    /// True if layer 2 is blended with what it covers
    /// (draw_background_2_blend: `wild`, always true here, without
    /// `background2notTransparent` or `background2over == 3`). Used for the HD
    /// (AI) scenery, which has no palette indices to blend.
    #[func]
    fn layer2_blended(&self) -> bool {
        self.sim.as_ref().is_some_and(|s| {
            let b = &View::top(s).world().background;
            b.background2over != 3
                && !b.background2_not_transparent
                && tyrian_sim::world::pt_wild(View::top(s).world().processor_type)
        })
    }

    /// Palette rows (high nibble, bit mask) that make up at least
    /// 2% of the non-zero pixels of the level's layer 2 tiles: colors
    /// allowed to recover the indices of the HD layer 2 (without this
    /// limit, ice blue falls on other hues: dark blotches).
    #[func]
    fn layer2_hue_rows(&self, data_dir: GString) -> i32 {
        let Some(sim) = self.sim.as_ref() else { return 0xffff };
        let level = &sim.setup().level;
        let name = level.events.header.shapes_file_name();
        let Ok(data) = std::fs::read(Path::new(&data_dir.to_string()).join(&name)) else {
            return 0xffff;
        };
        let Ok(tiles) = TileSet::parse(&data) else {
            return 0xffff;
        };
        layer2_hue_rows(&level.map, &tiles)
    }

    /// Mask of what `playfield_background` composes in addition to layer 1
    /// and the ground enemies: 1 layer 2, 2 layer 3, 4 sky enemies,
    /// 8 top enemies, 16 player shots, 32 ships,
    /// 64 enemy shots and explosions (order of tyrian2.c:JE_main).
    #[func]
    fn playfield_parts(&self) -> i32 {
        self.sim.as_ref().map_or(0, |s| parts(View::bottom(s)))
    }

    /// Localized texts of the in-game messages: hud_warning, miscText_59,
    /// miscText_66, misc_game_over (fonthand.c, CP437).
    #[func]
    fn set_playfield_texts(texts: PackedStringArray) {
        let v: Vec<Vec<u8>> = texts.as_slice().iter().map(|t| tyrian_render::font::to_cp437(&t.to_string())).collect();
        if v.len() == 4 {
            TEXTS.with(|t| *t.borrow_mut() = [v[0].clone(), v[1].clone(), v[2].clone(), v[3].clone()]);
        }
    }

    /// "INSERT COIN" text (miscText[7], localized) of the title
    /// demo (`player_movement.c`: "if (play_demo) JE_dString(VGAScreen,
    /// 115, 10, miscText[7], SMALL_FONT_SHAPES)").
    #[func]
    fn set_playfield_demo_text(text: GString) {
        let v = tyrian_render::font::to_cp437(&text.to_string());
        DEMO_TEXT.with(|t| *t.borrow_mut() = v);
    }

    /// Achievement banner (localized text, empty = none) that
    /// `playfield_background` composes before the RGBA layer, like the C.
    #[func]
    fn set_playfield_banner(text: GString) {
        let v = tyrian_render::font::to_cp437(&text.to_string());
        BANNER.with(|b| *b.borrow_mut() = v);
    }

    /// Area of the play area darkened by an overlay (statistics: y 12..136;
    /// achievements: y 4..183; `stats.c:stats_display`, `achievements.c:achievements_display`),
    /// two passes of `JE_barShade`. `y1 < y0`: none. The achievement banner
    /// then moves to y 170 (`achievements_draw_notification`).
    #[func]
    fn set_playfield_shade(y0: i32, y1: i32) {
        SHADE.with(|s| s.set((y0, y1)));
    }

    /// After each tick, if an image filter (water, lava, blurs) is active:
    /// composes the play area to keep the game_screen up to date that the filter
    /// of the next tick blends with the new image (tyrian2.c, one frame per tick).
    #[func]
    fn playfield_filters_tick(&self, data_dir: GString) {
        let Some(full) = self.sim.as_ref() else { return };
        if View::top(full).world().background.smoothies[..5].iter().any(|&b| b) {
            let before = GAME_SCREEN.with(|g| g.borrow().clone());
            compose_indexed(full, &data_dir.to_string());
            GAME_SCREEN_PREV.with(|p| *p.borrow_mut() = Some(before));
        } else {
            GAME_SCREEN_PREV.with(|p| *p.borrow_mut() = None);
        }
    }

    /// Visual FX preset (visual_fx.c:visual_fx_apply_preset):
    /// `advanced_lighting_enabled`, true only for Extended. Rendering
    /// only (RGBA layer), never read by the simulation.
    #[func]
    fn set_advanced_lighting(on: bool) {
        ADVANCED_LIGHTING.with(|l| l.set(on));
    }

    #[func]
    fn advanced_lighting() -> bool {
        ADVANCED_LIGHTING.with(|l| l.get())
    }

    /// Frozen game image for the screens placed on top (pause menu
    /// mainint.c:JE_inGameSetup, help JE_inGameHelp): 8-bit indices
    /// 320x200 of the play area composed like `playfield_background`
    /// (game_screen, play area at x 24..287), without the RGBA layer that the C
    /// clears after each presentation. The game_screen kept for the
    /// image filters is not modified. Empty array if nothing is loaded.
    #[func]
    fn playfield_indices(&self, data_dir: GString) -> PackedByteArray {
        let Some(full) = self.sim.as_ref() else {
            return PackedByteArray::new();
        };
        let bottom = View::bottom(full);
        let saved = GAME_SCREEN.with(|g| g.borrow().clone());
        let composed = compose_for_render(full, &data_dir.to_string());
        GAME_SCREEN.with(|g| g.borrow_mut().copy_from_slice(&saved));
        let Some((mut px, _)) = composed else {
            return PackedByteArray::new();
        };
        star_show_copy(&mut px, bottom);
        draw_overlay_shade(&mut px);
        draw_banner(&mut px, bottom);
        draw_wave_intro(&mut px);
        PackedByteArray::from(px)
    }

    /// Endless wave entry banner (endless.c:endless_draw_hud, two
    /// lines in SMALL_FONT_SHAPES by JE_dString, darkening shadow on
    /// the game image); empty texts = none.
    #[func]
    fn set_playfield_wave_intro(line1: GString, line2: GString) {
        let v = [tyrian_render::font::to_cp437(&line1.to_string()), tyrian_render::font::to_cp437(&line2.to_string())];
        WAVE_INTRO.with(|w| *w.borrow_mut() = v);
    }

    /// 8-bit image of the play area composed like tyrian2.c, seen through
    /// `palette` (768 bytes RGB, hardware palette of
    /// `TyrianVga.palette_bytes`). 264x184 RGBA8 image, or null.
    #[func]
    fn playfield_background(&self, data_dir: GString, palette: PackedByteArray) -> Option<Gd<Image>> {
        let full = self.sim.as_ref()?;
        let bottom = View::bottom(full);
        let pal = palette.as_slice();
        if pal.len() < 768 {
            return None;
        }
        let (mut px, after_player) = compose_for_render(full, &data_dir.to_string())?;
        // RGBA layer composed at presentation (JE_starShowVGA).
        let glow = glow_pixels(bottom, pal, Some(&after_player), true);
        star_show_copy(&mut px, bottom);
        draw_overlay_shade(&mut px);
        draw_banner(&mut px, bottom);
        draw_wave_intro(&mut px);
        let mut out = Vec::with_capacity(PLAY_W * PLAY_H * 4);
        for y in 0..PLAY_H {
            for x in 0..PLAY_W {
                let j = y * 320 + x + PLAY_AREA_X;
                let i = px[j] as usize * 3;
                // RGBAScreen is in screen coordinates: play area at x 0..263.
                let c = blend([pal[i], pal[i + 1], pal[i + 2]], glow[y * 320 + x]);
                out.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        Image::create_from_data(PLAY_W as i32, PLAY_H as i32, false, Format::RGBA8, &PackedByteArray::from(out))
    }
}

#[godot_api(secondary)]
impl TyrianSim {
    /// Starfield alone, for the HD (AI) scenery which has no palette
    /// indices: the original layer 1 is composed as indices like in
    /// `playfield_background`, then `update_and_draw_starfield` places its
    /// pixels there (only on index 0, so where layer 1 has no
    /// tile). Only the pixels placed by the stars are opaque. 264x184
    /// RGBA8 image (play area), to be placed above layer 1 and below
    /// layer 2, or null.
    #[func]
    fn playfield_stars(&self, data_dir: GString, palette: PackedByteArray) -> Option<Gd<Image>> {
        let sim = View::top(self.sim.as_ref()?);
        let pal = palette.as_slice();
        if pal.len() < 768 {
            return None;
        }
        let level = &sim.setup().level;
        let name = level.events.header.shapes_file_name();
        TILESET.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.as_ref().is_none_or(|(n, _)| *n != name) {
                let data = std::fs::read(Path::new(&data_dir.to_string()).join(&name)).ok()?;
                *cache = Some((name.clone(), TileSet::parse(&data).ok()?));
            }
            let (_, tiles) = cache.as_ref()?;
            let w = sim.world();
            let mut px = vec![0u8; SURFACE_LEN as usize];
            if w.special_weapon.astral_duration == 0 {
                let (y, b) = w.render.drawn_layer[0]
                    .map_or((w.background.map_y_pos[0], w.background.back_pos[0] as i32), |(y, b)| (y, b as i32));
                draw_layer(
                    &mut px,
                    &level.map,
                    tiles,
                    0,
                    y,
                    w.background.map_x_bp_pos,
                    w.background.map_x_pos as i32,
                    b,
                    false,
                );
            }
            let under = px.clone();
            draw_starfield(&mut px, sim);
            let mut out = vec![0u8; PLAY_W * PLAY_H * 4];
            for y in 0..PLAY_H {
                for x in 0..PLAY_W {
                    let j = y * 320 + x + PLAY_AREA_X;
                    if px[j] != under[j] {
                        let i = px[j] as usize * 3;
                        out[(y * PLAY_W + x) * 4..][..4].copy_from_slice(&[pal[i], pal[i + 1], pal[i + 2], 255]);
                    }
                }
            }
            Image::create_from_data(PLAY_W as i32, PLAY_H as i32, false, Format::RGBA8, &PackedByteArray::from(out))
        })
    }

    /// For the HD (AI) scenery: what `playfield_background` draws
    /// over the scenery and sprites and the Godot nodes do not have --
    /// level texts (WARNING, Secret Level, countdown, GAME OVER),
    /// boss bars, sparks (JE_drawSP) -- then the C RGBA layer
    /// (invulnerability aura, shot glows). Drawn on two surfaces
    /// cleared to 0 and to 1: a pixel is opaque if it changed on one
    /// of them (color read on the black surface, like a spark on
    /// space). 264x184 RGBA8 image, above the whole game, or null.
    #[func]
    fn playfield_hd_overlay(&self, palette: PackedByteArray) -> Option<Gd<Image>> {
        let bottom = View::bottom(self.sim.as_ref()?);
        let pal = palette.as_slice();
        if pal.len() < 768 {
            return None;
        }
        // C order: texts and sparks, level filter, boss bars,
        // then lamp (JE_starShowVGA) on the whole image. Here the filter and
        // the lamp only change the pixels of this layer; the rest of the play
        // area gets them from the shader (TyrianSim.hd_screen_effects).
        let w = bottom.world();
        let drawn = |fill: u8, bars: bool| {
            let mut surf = Surface::new(320, 200);
            surf.clear(fill);
            with_assets(|a| draw_demo_text(&mut surf, a, bottom));
            with_assets(|a| draw_texts(&mut surf, a, bottom));
            // Damage numbers (damagenumbers.c), as in draw_rest.
            with_assets(|a| crate::fx_render::FX.with(|f| f.borrow().draw_numbers(&mut surf, a)));
            draw_superpixels(&mut surf.pixels, bottom);
            if bars {
                draw_boss_bars(&mut surf, bottom);
            }
            surf.pixels
        };
        let (black, one) = (drawn(0, true), drawn(1, true));
        let before_bars = drawn(0, false);
        let lamp = (w.render.star_show_code == 2).then(|| (281 - w.players[0].x, 172 - w.players[0].y));
        let glow = glow_pixels(bottom, pal, None, false);
        let mut out = vec![0u8; PLAY_W * PLAY_H * 4];
        for y in 0..PLAY_H {
            for x in 0..PLAY_W {
                let j = y * 320 + x + PLAY_AREA_X;
                let o = &mut out[(y * PLAY_W + x) * 4..][..4];
                if black[j] != 0 || one[j] != 1 {
                    let mut v = black[j];
                    if let Some((col, int_)) = w.render.drawn_filter {
                        // Boss bars drawn after the filter: untouched.
                        if before_bars[j] == v {
                            v = filter_index(v, col, int_);
                        }
                    }
                    if let Some((lx, ly)) = lamp {
                        v = lamp_index(v, x, y, lx, ly);
                    }
                    let i = v as usize * 3;
                    let c = blend([pal[i], pal[i + 1], pal[i + 2]], glow[y * 320 + x]);
                    o.copy_from_slice(&[c[0], c[1], c[2], 255]);
                } else {
                    o.copy_from_slice(&glow[y * 320 + x]);
                }
            }
        }
        Image::create_from_data(PLAY_W as i32, PLAY_H as i32, false, Format::RGBA8, &PackedByteArray::from(out))
    }
}

#[godot_api(secondary)]
impl TyrianSim {
    /// Tools for comparison with the C: ms counted by the golden master virtual
    /// clock outside the game frames (index.txt, 3rd column), added
    /// to the low-armor blink clock.
    #[func]
    fn set_render_clock_offset(&mut self, ms: i64) {
        if let Some(sim) = self.sim.as_mut() {
            sim.set_render_clock_offset(ms.clamp(0, u32::MAX as i64) as u32);
        }
    }

    /// HD (AI) mode: Zinglon rays of this iteration (varz.c:JE_doSpecialShot),
    /// in pairs center x (game_screen), half-width.
    #[func]
    fn hd_zinglon(&self) -> PackedInt32Array {
        let Some(full) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        View::bottom(full).world().render.drawn_zinglon.iter().flat_map(|&(x, t)| [x, t]).collect()
    }

    /// HD (AI) mode: player ships, shadows, charge icon and
    /// sidekicks, in C drawing order (same pieces as the 8-bit
    /// rendering), as records of 5 integers: sheet (8, 9 or 12 for
    /// main:8, main:9, main:12), x, y (game_screen), Sprite2 frame, mode
    /// (0 normal, 1 blend, 2 darkened).
    #[func]
    fn hd_player_sprites(&self) -> PackedInt32Array {
        let Some(full) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let (top, bottom) = (View::top(full), View::bottom(full));
        let shadow_ofs = top.world().background.map_x2_ofs as i32;
        let mut blits = Vec::new();
        ship_shadow_blits(&mut blits, bottom, shadow_ofs);
        level_end_trail_blits(&mut blits, bottom);
        player_blits(&mut blits, bottom);
        // varz.c:JE_doSpecialShot: special weapon marker after the ship.
        let icon = bottom.world().render.drawn_special_icon;
        if icon != 0 {
            blits.push(("main:8", 47, 4, icon, BlitMode::Normal));
        }
        after_player_blits(&mut blits, bottom);
        let mut out = Vec::with_capacity(blits.len() * 5);
        for (name, x, y, index, mode) in blits {
            let sheet = name.strip_prefix("main:").and_then(|n| n.parse().ok()).unwrap_or(0);
            out.extend_from_slice(&[sheet, x, y, index as i32, mode as i32]);
        }
        PackedInt32Array::from(out)
    }

    /// HD (AI) mode: tile pieces that `draw_background_1/2/3` places in
    /// game_screen for layer `layer` (0..2) in this frame, to draw
    /// the upscaled map at the same places as the C. The C addresses the map and
    /// the screen flat: a row of 12 tiles overflows onto the next line
    /// of the map, and a tile that goes past x = 319 continues on the left, one
    /// line lower (`blit_background_row`). Records of 6 integers:
    /// column and row of the cell in the map, x and y on screen (game_screen),
    /// first pixel of the tile and width of the piece (a tile cut by the
    /// screen edge gives two pieces). An empty cell is transparent
    /// in the upscaled image.
    #[func]
    fn hd_layer_cells(&self, layer: i32) -> PackedInt32Array {
        let Some(full) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let layer = layer.clamp(0, 2) as usize;
        let top = View::top(full);
        let b = &top.world().background;
        // Layer 3 "over 1" and layer 2 "over 2": drawn at the end of the iteration.
        let late = (layer == 2 && b.background3over == 1) || (layer == 1 && b.background2over == 2);
        let sim = if late { View::bottom(full) } else { top };
        let w = sim.world();
        let bg = &w.background;
        let map = &sim.setup().level.map;
        let (ypos, back) = w.render.drawn_layer[layer]
            .map_or((bg.map_y_pos[layer], bg.back_pos[layer] as i32), |(y, b)| (y, b as i32));
        let (bp, xpos) = match layer {
            0 => (bg.map_x_bp_pos, bg.map_x_pos as i32),
            1 if (bg.background2over == 3 || bg.background2_not_transparent) && bg.smoothies[1] => {
                (bg.map_x_bp_pos, bg.map_x_pos as i32)
            }
            1 => (bg.map_x2_bp_pos, bg.map_x2_pos as i32),
            _ => (bg.map_x3_bp_pos, bg.map_x3_pos as i32),
        };
        if layer == 0 && w.special_weapon.astral_duration != 0 {
            return PackedInt32Array::new();
        }
        if layer == 1 && !background2(w) {
            return PackedInt32Array::new();
        }
        let cells = map.layer(layer);
        let width = LAYER_WIDTH[layer];
        let mut out = Vec::new();
        let mut flat = ypos + bp - 12;
        for i in -1..7 {
            for k in 0..12 {
                let f = flat + k;
                if f < 0 || f as usize >= cells.len() {
                    continue;
                }
                let p = (i * 28 + back) * PITCH + xpos + k * 24;
                let (sx, sy) = (p.rem_euclid(PITCH), p.div_euclid(PITCH));
                let (col, row) = (f % width, f / width);
                let first = (PITCH - sx).min(24);
                out.extend_from_slice(&[col, row, sx, sy, 0, first]);
                if first < 24 {
                    out.extend_from_slice(&[col, row, 0, sy + 1, first, 24 - first]);
                }
            }
            flat += width;
        }
        PackedInt32Array::from(out)
    }

    /// HD (AI) mode: shots drawn in this iteration, like
    /// `draw_player_shots` (all darkened shadows, then the shots) and `draw_enemy_shots`,
    /// as records of 6 integers: 0 player shot / 1 enemy shot,
    /// sheet (7 or 11 for main:7, main:11; 5 for the simple table
    /// OPTION_SHAPES, 0-based frame), x, y (game_screen), frame, mode
    /// (0 normal, 1 blend, 2 darkened).
    #[func]
    fn hd_shot_sprites(&self) -> PackedInt32Array {
        let Some(full) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let mut out = Vec::new();
        // Same shots as the 8-bit rendering: all the shadows, then all the shots.
        let draws = player_shot_draws(View::top(full).world());
        for d in draws.iter().filter(|d| d.shadow) {
            if let ShotKind::Sheet(n, idx) = d.kind {
                out.extend_from_slice(&[0, n as i32, d.x, d.y + SHADOW_Y_DIST, idx as i32, 2]);
            }
        }
        for d in &draws {
            match d.kind {
                ShotKind::Sheet(n, idx) => out.extend_from_slice(&[0, n as i32, d.x, d.y, idx as i32, 0]),
                ShotKind::OptionShape(n) => out.extend_from_slice(&[0, 5, d.x, d.y, n as i32, 1]),
            }
        }
        for &(x, y, sprite) in &View::bottom(full).world().render.drawn_enemy_shots {
            let frame = sprite as i32;
            let (sheet, idx) = if frame >= 500 { (11, frame - 500) } else { (7, frame) };
            out.extend_from_slice(&[1, sheet, x as i32, y as i32, idx, 0]);
        }
        PackedInt32Array::from(out)
    }

    /// HD (AI) mode: enemies drawn in this iteration, same pieces as
    /// the 8-bit rendering (`JE_drawEnemy`, darkened trails and filters
    /// included), as records of 7 integers: layer (0, 1 ground, 2 sky,
    /// 3 top, like `enemies()`), sheet byte (`enemy_sheet_name` in
    /// GDScript), x, y (game_screen), Sprite2 frame, mode (0 normal,
    /// 2 darkened, 4 filter), filter (tint << 4).
    #[func]
    fn hd_enemy_sprites(&self) -> PackedInt32Array {
        let Some(full) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let mut out = Vec::new();
        let b = &View::top(full).world().background;
        for layer in 0..4u8 {
            // Like playfield_background: top enemies (topEnemyOver) and
            // sky enemies (skyEnemyOverAll) drawn with the end of the iteration.
            let late = (layer == 3 && b.top_enemy_over) || (layer == 2 && b.sky_enemy_over_all);
            let view = if late { View::bottom(full) } else { View::top(full) };
            for (sheet, x, y, idx, mode, filter) in enemy_blits(view, layer) {
                out.extend_from_slice(&[layer as i32, sheet as i32, x, y, idx as i32, mode as i32, filter as i32]);
            }
        }
        PackedInt32Array::from(out)
    }

    /// HD (AI) mode: what the C applies to the whole image of the play area
    /// and the Godot nodes do not have -- backgrnd.c:JE_filterScreen (tint,
    /// brightness; -99 = none) and enemy_render.c:JE_starShowVGA (code 1 flipped
    /// image, 2 lamp centered on player 1, lightx, lighty):
    /// [tint, brightness, code, lightx, lighty].
    #[func]
    fn hd_screen_effects(&self) -> PackedInt32Array {
        let Some(full) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let w = View::bottom(full).world();
        let (col, int_) = w.render.drawn_filter.map_or((-99, -99), |(c, i)| (c as i32, i as i32));
        let b = &View::top(full).world().background;
        PackedInt32Array::from(vec![
            col,
            int_,
            w.render.star_show_code as i32,
            281 - w.players[0].x,
            172 - w.players[0].y,
            b.background2over as i32,
            b.background3over as i32,
            b.top_enemy_over as i32,
            b.sky_enemy_over_all as i32,
            // Image filters (smoothies 1 to 5, smoothie_data[0], water tint).
            b.smoothies[0] as i32,
            b.smoothies[1] as i32,
            b.smoothies[2] as i32,
            b.smoothies[3] as i32,
            b.smoothies[4] as i32,
            b.smoothie_data[0] as i32,
            (b.smoothie_data[1] as i32) << 4,
        ])
    }
}

// --- C RGBA layer (video_rgba.c) -------------------------------------------

/// `video_rgba.c:rgba_draw_glow`: additive radial glow, premultiplied alpha,
/// f32 floats like the C. `px`: 320x200 (r, g, b, a).
fn draw_glow(px: &mut [[u8; 4]], cx: i32, cy: i32, radius: i32, color: [u8; 3], intensity: f32) {
    if radius <= 0 || intensity <= 0.0 {
        return;
    }
    let r2 = radius * radius;
    let (y0, y1) = ((cy - radius).max(0), (cy + radius).min(199));
    let (x0, x1) = ((cx - radius).max(0), (cx + radius).min(319));
    for py in y0..=y1 {
        let dy2 = (py - cy) * (py - cy);
        for x in x0..=x1 {
            let d2 = (x - cx) * (x - cx) + dy2;
            if d2 > r2 {
                continue;
            }
            let dist_norm = 1.0f32 - d2 as f32 / r2 as f32;
            let alpha = intensity * dist_norm;
            let na = ((alpha * 255.0) as i32).min(255);
            if na < 2 {
                continue;
            }
            let e = &mut px[(py * 320 + x) as usize];
            for k in 0..3 {
                let n = ((color[k] as f32 * alpha) as i32).min(255);
                e[k] = (e[k] as i32 + n).min(255) as u8;
            }
            e[3] = (e[3] as i32 + na).min(255) as u8;
        }
    }
}

/// `enemy_render.c:sprite2_dominant_hue`: hue (high nibble) of the first
/// opaque pixel of frame `index` (1-based) of a Sprite2 sheet.
pub(crate) fn sprite2_dominant_hue(sheet: &[u8], index: u32, default_hue: u8) -> u8 {
    if index == 0 || (index as usize) * 2 > sheet.len() {
        return default_hue;
    }
    let i = (index as usize - 1) * 2;
    let offset = u16::from_le_bytes([sheet[i], sheet[i + 1]]) as usize;
    if offset >= sheet.len() {
        return default_hue;
    }
    let mut p = offset;
    while p < sheet.len() && sheet[p] != 0x0f {
        if (sheet[p] & 0xf0) >> 4 != 0 {
            return sheet.get(p + 1).map_or(default_hue, |v| (v >> 4) & 0x0f);
        }
        p += 1;
    }
    default_hue
}

/// RGBA layer (r, g, b, a premultiplied) of `playfield_glow`.
/// `vga`: game_screen at the time the ship is drawn (water reflection).
/// `palette_art`: composed over the 8-bit image (Original art).
fn glow_pixels(sim: View, pal: &[u8], vga: Option<&[u8]>, palette_art: bool) -> Vec<[u8; 4]> {
    let w = sim.world();
    let mut px = vec![[0u8; 4]; 320 * 200];
    // player_movement.c / enemy_render.c: "if (advanced_lighting_enabled
    // && RGBAScreen != NULL)" in front of the aura and the glows; the reflection
    // (processorType >= 4) does not depend on it.
    let lighting = ADVANCED_LIGHTING.with(|l| l.get());
    let n = if !lighting {
        0
    } else if w.two_player_mode {
        2
    } else {
        1
    };
    for (pi, p) in w.players.iter().enumerate().take(n) {
        // player_movement.c: drawn right after --invulnerable_ticks;
        // tint = (ticks after decrement x 2) % 128. GAME_COORD_X_OFFSET = -24.
        if p.is_alive && w.render.drawn_invulnerable[pi] {
            let hue = (p.invulnerable_ticks as i32 * 2) % 128;
            let c = [(90 + hue / 2) as u8, (170 + hue / 4) as u8, 255];
            draw_glow(&mut px, p.x - 24, p.y + 4, 18, c, 0.30);
        }
    }
    if let Some(vga) = vga {
        draw_reflection(&mut px, sim, pal, vga);
    }
    with_assets(|a| {
        if !lighting {
            return;
        }
        let Some(sheet) = a.sheet("main:7") else { return };
        for i in 0..w.player_shots.len().min(w.shot_avail.len()) {
            if w.shot_avail[i] == 0 {
                continue;
            }
            let s = &w.player_shots[i];
            let frame = s.drawn_frame as u32;
            let hue = sprite2_dominant_hue(sheet, frame, 15) as usize;
            let c = (hue << 4 | 0x0f) * 3;
            draw_glow(&mut px, s.shot_x as i32 - 24, s.shot_y as i32, 8, [pal[c], pal[c + 1], pal[c + 2]], 0.12);
        }
    });
    // enemy_render.c:JE_starShowVGA, continued: thruster, particles, shock
    // waves, Modern explosions, boss flash (particles.c,
    // hitstop.c; modules fx_render and modern_explosions).
    crate::fx_render::FX.with(|f| {
        let mut f = f.borrow_mut();
        f.set_palette(pal);
        f.reserve = if f.modern.live_count() > 0 { enemy_shot_reserve(sim) } else { Vec::new() };
        f.palette_art = palette_art;
        f.draw_rgba(&mut px, draw_glow);
    });
    px
}

/// Pixels of the enemy shots and the pixel surrounding them, as indices of the
/// 320x200 RGBA layer (game_screen shifted by PLAY_AREA_X): the Modern explosions
/// draw nothing there, the shots stay readable.
fn enemy_shot_reserve(sim: View) -> Vec<bool> {
    let drawn = |fill: u8| {
        let mut surf = Surface::new(320, 200);
        surf.clear(fill);
        with_assets(|a| draw_enemy_shots(&mut surf, a, sim));
        surf.pixels
    };
    let (black, one) = (drawn(0), drawn(1));
    let mut out = vec![false; 320 * 200];
    for y in 0..200usize {
        for x in PLAY_AREA_X..320 {
            let j = y * 320 + x;
            if black[j] == 0 && one[j] == 1 {
                continue;
            }
            let (cx, cy) = ((x - PLAY_AREA_X) as i32, y as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (rx, ry) = (cx + dx, cy + dy);
                    if (0..320).contains(&rx) && (0..200).contains(&ry) {
                        out[(ry * 320 + rx) as usize] = true;
                    }
                }
            }
        }
    }
    out
}

/// `player_movement.c` (Wild, processorType >= 4, smoothies[0] or [1]):
/// flipped reflection of the ship (34x28 read from game_screen) 4 px below it,
/// alpha 45 - 2*rank, written as is (non-additive) into RGBAScreen.
fn draw_reflection(px: &mut [[u8; 4]], sim: View, pal: &[u8], vga: &[u8]) {
    let w = sim.world();
    let p = &w.players[0];
    if !p.is_alive || !(w.background.smoothies[0] || w.background.smoothies[1]) {
        return;
    }
    let (ship_w, ship_h) = (34, 28);
    let (sx, sy) = (p.x - 17, p.y - 7);
    let ry = sy + ship_h + 4;
    let mut row = 0;
    while row < ship_h && ry + row < 190 {
        let src_row = sy + (ship_h - 1 - row);
        let dst_row = ry + row;
        let a = 45 - row * 2;
        if a <= 0 {
            break;
        }
        if (0..200).contains(&src_row) && dst_row >= 0 {
            for col in 0..ship_w {
                let x = sx + col;
                if !(0..260).contains(&x) {
                    continue;
                }
                let idx = vga[(src_row * 320 + x) as usize] as usize;
                let rx = x - 24;
                if idx == 0 || !(0..320).contains(&rx) {
                    continue;
                }
                px[(dst_row * 320 + rx) as usize] = [pal[idx * 3], pal[idx * 3 + 1], pal[idx * 3 + 2], a as u8];
            }
        }
        row += 1;
    }
}

/// SDL_BLENDMODE_BLEND of the layer on the 8-bit image, integer formula of
/// scripts/visual_diff.py:otfr_image (dst = (s*a + d*(255 - a)) / 255).
fn blend(dst: [u8; 3], src: [u8; 4]) -> [u8; 3] {
    let a = src[3] as u32;
    if a == 0 {
        return dst;
    }
    let f = |s: u8, d: u8| ((s as u32 * a + d as u32 * (255 - a)) / 255) as u8;
    [f(src[0], dst[0]), f(src[1], dst[1]), f(src[2], dst[2])]
}

#[godot_api(secondary)]
impl TyrianSim {
    /// C 320x200 RGBA layer (advanced_lighting_enabled, true by default:
    /// video_rgba.c) for the play area: invulnerability aura
    /// (player_movement.c, radius 18, intensity 0.30) and glows of the
    /// player shots (enemy_render.c:JE_starShowVGA, radius 8, 0.12,
    /// palette32[hue << 4 | 0x0F]). `palette`: 768 bytes current RGB.
    /// Alpha and color premultiplied like RGBAScreen; level.gd composes it
    /// in SDL_BLENDMODE_BLEND (dst = src*a + dst*(1 - a)).
    /// Particles (particles.c): disabled by the golden master
    /// (golden_master.c: particles_enabled = false), hence absent here.
    #[func]
    fn playfield_glow(&self, palette: PackedByteArray) -> Option<Gd<Image>> {
        let sim = View::bottom(self.sim.as_ref()?);
        let pal = palette.as_slice();
        if pal.len() < 768 {
            return None;
        }
        let mut px = glow_pixels(sim, pal, None, true);
        // The play area receives the layer in playfield_background (same
        // formula as the C composition); here only what spills over.
        for y in 0..PLAY_H {
            for x in 0..PLAY_W {
                px[y * 320 + x] = [0; 4];
            }
        }
        let out: Vec<u8> = px.iter().flat_map(|p| *p).collect();
        Image::create_from_data(320, 200, false, Format::RGBA8, &PackedByteArray::from(out))
    }
}

// --- Rest of the frame: player, enemy shots, late passes, explosions ---

/// Drawing of a Sprite2 sprite piece (`sprite.c`): sheet ("main:N"),
/// position in game_screen, frame, mode. List shared by the 8-bit rendering
/// (`draw_rest`) and the HD (AI) mode (`hd_player_sprites`), which draws the
/// same pieces with the upscaled sheets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlitMode {
    Normal = 0,
    Blend = 1,
    Darken = 2,
}

type Blit = (&'static str, i32, i32, u32, BlitMode);

/// `blit_sprite2x2*` : 4 images index, +1, +19, +20.
fn push_2x2(out: &mut Vec<Blit>, sheet: &'static str, x: i32, y: i32, index: u32, mode: BlitMode) {
    for (dx, dy, di) in [(0, 0, 0), (12, 0, 1), (0, 14, 19), (12, 14, 20)] {
        out.push((sheet, x + dx, y + dy, index + di, mode));
    }
}

/// Runs a list of pieces on an 8-bit surface.
fn blit_all(surf: &mut Surface, a: &mut tyrian_render::Assets, blits: &[Blit]) {
    for &(name, x, y, index, mode) in blits {
        let Some(sheet) = a.sheet(name) else { continue };
        match mode {
            BlitMode::Normal => surf.blit_sprite2(x, y, sheet, index),
            BlitMode::Blend => surf.blit_sprite2_blend(x, y, sheet, index),
            BlitMode::Darken => surf.blit_sprite2_darken(x, y, sheet, index),
        }
    }
}

/// `player_movement.c:JE_playerMovement`, "End Level Fade-Out": during the
/// level exit, the C draws before the ship a trail of copies
/// (up to 19) increasingly spaced, under the ship flying away.
/// Frames without banking: Dragonwing +13/+51, Nort 220/222, otherwise shipGr.
fn level_end_trail_blits(out: &mut Vec<Blit>, sim: View) {
    let w = sim.world();
    let p = &w.players[0];
    if !p.is_alive || !w.ending.end_level || w.ending.level_end == 0 {
        return;
    }
    let gr = w.ship_gr as i32;
    let (sheet, base) = if gr > 500 { ("main:12", gr - 500) } else { ("main:8", gr) };
    let pieces: Vec<(i32, u32)> = match gr {
        0 => vec![(-17, 13), (7, 51)],
        1 => vec![(-17, 220), (7, 222)],
        _ => vec![(-5, base as u32)],
    };
    for trail_y in level_end_trail_ys(p.y, w.ending.level_end) {
        for &(dx, idx) in &pieces {
            push_2x2(out, sheet, p.x + dx, trail_y - 7, idx, BlitMode::Normal);
        }
    }
}

/// Y positions of the copies of the end-of-level trail (exact C computation:
/// `num_trails = min(|41 - levelEnd|, 20)`, increasing spacing, bounds 0..170).
fn level_end_trail_ys(player_y: i32, level_end: u8) -> Vec<i32> {
    let num_trails = (41 - level_end as i32).abs().min(20);
    let mut spacing = 1;
    let mut trail_y = player_y;
    for _ in 0..num_trails {
        trail_y += spacing;
        spacing += 1;
    }
    let mut ys = Vec::new();
    for _ in 1..num_trails {
        trail_y -= spacing;
        spacing -= 1;
        if trail_y > 0 && trail_y < 170 {
            ys.push(trail_y);
        }
    }
    ys
}

/// (x offset, frame) of each 2x2 piece of player 1's ship.
fn ship_pieces(gr: i32, ship_sprite: i32) -> Vec<(i32, u32)> {
    match gr {
        0 => vec![(-17, (ship_sprite + 13) as u32), (7, (ship_sprite + 51) as u32)],
        1 => vec![(-17, 220), (7, 222)],
        _ => vec![(-5, ship_sprite as u32)],
    }
}

/// `player_movement.c:JE_playerMovement` (ship shadows): darkened shadow
/// (`background2` true) of player 1 then, with two players, of player 2's Dragonwing.
/// The C draws them right before each ship, hence after the player's
/// shots: the shadow darkened shots already drawn underneath (rear shots,
/// on the sides). Here they are placed under the shots, before them.
fn ship_shadow_blits(out: &mut Vec<Blit>, sim: View, shadow_ofs: i32) {
    let w = sim.world();
    if !background2(w) {
        return;
    }
    let shadow_x = -shadow_ofs + 30;
    let p = &w.players[0];
    if p.is_alive {
        let gr = w.ship_gr as i32;
        let (sheet, base) = if gr > 500 { ("main:12", gr - 500) } else { ("main:8", gr) };
        let ship_sprite = p.banking.clamp(-2, 2) * 2 + base;
        if gr <= 1 || ship_sprite < 304 {
            for (dx, idx) in ship_pieces(gr, ship_sprite) {
                push_2x2(out, sheet, p.x + dx + shadow_x, p.y - 7 + SHADOW_Y_DIST, idx, BlitMode::Darken);
            }
        }
    }
    let p = &w.players[1];
    if w.two_player_mode && p.is_alive && !w.ending.end_level && p.items.ship <= ShipId(90) {
        let ship_sprite = p.banking.clamp(-2, 2) * 2;
        for (dx, idx) in [(-17, (ship_sprite + 13) as u32), (7, (ship_sprite + 51) as u32)] {
            push_2x2(out, "main:8", p.x + dx + shadow_x, p.y - 7 + SHADOW_Y_DIST, idx, BlitMode::Darken);
        }
    }
}

/// Places the ship shadows (see `ship_shadow_blits`) under the player's shots.
fn draw_ship_shadows(px: &mut [u8], sim: View, shadow_ofs: i32) {
    let mut blits = Vec::new();
    ship_shadow_blits(&mut blits, sim, shadow_ofs);
    if blits.is_empty() {
        return;
    }
    let mut surf = Surface::new(320, 200);
    surf.pixels.copy_from_slice(px);
    with_assets(|a| blit_all(&mut surf, a, &blits));
    px.copy_from_slice(&surf.pixels);
}

/// `player_movement.c:JE_playerMovement` (drawing of player 1): the 2x2 ship, blended during
/// invulnerability. Three forms like the C (and the original, mainint.c):
/// shipGr 0 Dragonwing (two halves, frames +13 and +51), shipGr 1 Nort
/// (frames 220 and 222, shadow included, and the fin 39/40/58/59 depending on
/// banking, except during invulnerability), otherwise ordinary ship (shadow
/// only for a frame < 304).
fn player_blits(out: &mut Vec<Blit>, sim: View) {
    let w = sim.world();
    let p = &w.players[0];
    if !p.is_alive {
        return;
    }
    let gr = w.ship_gr as i32;
    let (sheet, base) = if gr > 500 { ("main:12", gr - 500) } else { ("main:8", gr) };
    let ship_sprite = p.banking.clamp(-2, 2) * 2 + base;
    let pieces = ship_pieces(gr, ship_sprite);
    let mode = if w.render.drawn_invulnerable[0] { BlitMode::Blend } else { BlitMode::Normal };
    for &(dx, idx) in &pieces {
        push_2x2(out, sheet, p.x + dx, p.y - 7, idx, mode);
    }
    // Nort: fin on the banking side (switch (ship_sprite) of the C).
    if gr == 1 && !w.render.drawn_invulnerable[0] {
        let fin = match ship_sprite {
            5 => Some((-17, 40)),
            3 => Some((-17, 39)),
            -1 => Some((19, 58)),
            -3 => Some((19, 59)),
            _ => None,
        };
        if let Some((dx, idx)) = fin {
            out.push((sheet, p.x + dx, p.y + 7, idx, BlitMode::Normal));
        }
    }
}

/// `player_movement.c:JE_playerMovement` (player 2, `shipGr2 == 0`: Dragonwing,
/// spriteSheet9; varz.c:JE_getShipInfo, ship <= 90): inside `if (!endLevel)`,
/// the two halves of the ship (frames +13 and +51), blended during
/// invulnerability. Drawn right after player 1, like the two successive
/// calls of mainint.c:JE_mainGamePlayerFunctions. Its two darkened shadows
/// (x - 17 and x + 7) are in `ship_shadow_blits`.
fn dragonwing_blits(out: &mut Vec<Blit>, sim: View) {
    let w = sim.world();
    let p = &w.players[1];
    if !p.is_alive || w.ending.end_level || p.items.ship > ShipId(90) {
        return;
    }
    let ship_sprite = p.banking.clamp(-2, 2) * 2;
    let halves = [(-17, (ship_sprite + 13) as u32), (7, (ship_sprite + 51) as u32)];
    let mode = if w.render.drawn_invulnerable[1] { BlitMode::Blend } else { BlitMode::Normal };
    for (dx, idx) in halves {
        push_2x2(out, "main:8", p.x + dx, p.y - 7, idx, mode);
    }
}

/// `player_movement.c` "Super Charge Weapons": charge icon of player 2
/// (spriteSheet10, frame 77 + level + chargeGr x 19) at (x + 1 + Dragonwing,
/// y - 13), drawn after its ship, outside linked mode.
fn charge_icon_blits(out: &mut Vec<Blit>, sim: View) {
    let w = sim.world();
    if w.render.drawn_charge_sprite == 0 {
        return;
    }
    let p = &w.players[1];
    let dragonwing = (p.items.ship <= ShipId(90)) as i32;
    out.push(("main:9", p.x + dragonwing + 1, p.y - 13, w.render.drawn_charge_sprite, BlitMode::Normal));
}

/// `player_movement.c:JE_playerMovement`, "draw sidekicks" block: at the end
/// of the movement of the player carrying the sidekicks (player 2 in two-player),
/// if `!endLevel`; frame `gr[animation_frame] + charge`, 2x2 of spriteSheet10
/// at (x - 6, y) for styles 1 and 2, otherwise 1x1 of spriteSheet9 at (x, y).
/// No `is_alive` condition: the C also draws them with the ship destroyed.
fn sidekick_blits(out: &mut Vec<Blit>, sim: View) {
    let w = sim.world();
    if w.ending.end_level {
        return;
    }
    let pi = if w.two_player_mode { 1 } else { 0 };
    let p = &w.players[pi];
    for i in 0..2 {
        let Some(o) = w.tables.options.get(p.items.sidekick[i].index()) else {
            continue;
        };
        if o.option == 0 {
            continue;
        }
        let s = &p.sidekick[i];
        let sprite =
            (o.gr[(s.animation_frame as usize).min(19)] as u32).wrapping_add(w.render.drawn_sidekick_charge[pi][i]);
        if s.style == 1 || s.style == 2 {
            push_2x2(out, "main:9", s.x - 6, s.y, sprite, BlitMode::Normal);
        } else {
            out.push(("main:8", s.x, s.y, sprite, BlitMode::Normal));
        }
    }
}

/// After player 1's ship: Dragonwing and charge icon (two
/// players), then sidekicks.
fn after_player_blits(out: &mut Vec<Blit>, sim: View) {
    if sim.world().two_player_mode {
        dragonwing_blits(out, sim);
        charge_icon_blits(out, sim);
    }
    sidekick_blits(out, sim);
}

/// `shot_update.c:JE_updateEnemyShots`: blit_sprite2 (spriteSheet12 if
/// sgr >= 500, otherwise spriteSheet8).
fn draw_enemy_shots(surf: &mut Surface, a: &mut tyrian_render::Assets, sim: View) {
    for &(x, y, sprite) in &sim.world().render.drawn_enemy_shots {
        let frame = sprite as u32;
        let (name, idx) = if frame >= 500 { ("main:11", frame - 500) } else { ("main:7", frame) };
        if let Some(sheet) = a.sheet(name) {
            surf.blit_sprite2(x as i32, y as i32, sheet, idx);
        }
    }
}

/// `explosion_update.c:JE_updateAndDrawExplosions`: `explosionTransparent`
/// true (config.c, processorType != 1) => blit_sprite2_blend of newsh6.shp.
/// In Modern style the hollow ring ending a fire explosion is not drawn:
/// the Modern explosions throw embers instead (`modern_explosions`).
fn draw_explosions(surf: &mut Surface, a: &mut tyrian_render::Assets, sim: View) {
    let Some(sheet) = a.sheet("newsh6.shp") else { return };
    // explosion_update.c: blit_sprite2_blend if explosionTransparent, otherwise blit_sprite2.
    let blend = tyrian_sim::world::pt_explosion_transparent(sim.world().processor_type);
    let embers = crate::fx_render::FX.with(|f| f.borrow().modern_explosions_on());
    for &(x, y, sprite) in &sim.world().render.drawn_explosions {
        if embers && crate::modern_explosions::ring_phase_frame(sprite) {
            continue;
        }
        if blend {
            surf.blit_sprite2_blend(x as i32, y as i32, sheet, sprite as u32);
        } else {
            surf.blit_sprite2(x as i32, y as i32, sheet, sprite as u32);
        }
    }
}

const PART_PLAYER: i32 = 32;
const PART_LATE: i32 = 64;

/// End of tyrian2.c:JE_main after JE_updatePlayerShots, on `px`. Returns
/// the image right after the ship is drawn (source of the water reflection).
fn draw_rest(px: &mut [u8], sim: View, tiles: &TileSet) -> Vec<u8> {
    let w = sim.world();
    let mut surf = Surface::new(320, 200);
    surf.pixels.copy_from_slice(px);
    with_assets(|a| draw_demo_text(&mut surf, a, sim));
    let mut blits = Vec::new();
    level_end_trail_blits(&mut blits, sim);
    player_blits(&mut blits, sim);
    with_assets(|a| blit_all(&mut surf, a, &blits));
    // Water reflection: read right after the ship, before the sidekicks.
    let after_player = surf.pixels.clone();
    // player_movement.c: JE_doSpecialShot comes after the ship and its
    // reflection (special weapon marker, Zinglon ray that also lightens the
    // ship), before the charge icon and the sidekicks.
    if w.render.drawn_special_icon != 0 {
        with_assets(|a| blit_all(&mut surf, a, &[("main:8", 47, 4, w.render.drawn_special_icon, BlitMode::Normal)]));
    }
    for &(cx, t) in &w.render.drawn_zinglon {
        surf.bar_bright(cx - t, 0, cx + t, 184);
        surf.bar_bright(cx - t - 2, 0, cx + t + 2, 184);
    }
    blits.clear();
    after_player_blits(&mut blits, sim);
    with_assets(|a| blit_all(&mut surf, a, &blits));
    with_assets(|a| draw_enemy_shots(&mut surf, a, sim));
    px.copy_from_slice(&surf.pixels);
    if w.background.background3over == 1 {
        draw_layer3(px, sim, tiles);
    }
    if w.background.top_enemy_over {
        draw_enemies(px, sim, &[3]);
    }
    if w.background.sky_enemy_over_all {
        draw_enemies(px, sim, &[2]);
    }
    surf.pixels.copy_from_slice(px);
    with_assets(|a| draw_explosions(&mut surf, a, sim));
    px.copy_from_slice(&surf.pixels);
    if w.background.background2over == 2 {
        draw_layer2(px, sim, tiles);
    }
    surf.pixels.copy_from_slice(px);
    with_assets(|a| draw_texts(&mut surf, a, sim));
    // tyrian2.c: damage_numbers_update_and_draw(VGAScreen) before JE_drawSP.
    with_assets(|a| crate::fx_render::FX.with(|f| f.borrow().draw_numbers(&mut surf, a)));
    px.copy_from_slice(&surf.pixels);
    draw_superpixels(px, sim);
    draw_filter(px, sim);
    surf.pixels.copy_from_slice(px);
    draw_boss_bars(&mut surf, sim);
    px.copy_from_slice(&surf.pixels);
    after_player
}

thread_local! {
    /// Localized texts of the in-game messages (TyrianText.L, set by
    /// `set_playfield_texts`): WARNING, Secret Level, countdown, GAME OVER.
    static TEXTS: RefCell<[Vec<u8>; 4]> = RefCell::new([b"WARNING".to_vec(), b"Secret Level".to_vec(), b"TIME REMAINING".to_vec(), b"Game Over".to_vec()]);
    /// Achievement banner displayed (CP437, empty = none), set by
    /// `set_playfield_banner`.
    static BANNER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// "INSERT COIN" of the title demo (`set_playfield_demo_text`).
    static DEMO_TEXT: RefCell<Vec<u8>> = RefCell::new(b"INSERT COIN".to_vec());
    /// Band of the play area darkened by the open overlay (y0, y1), (-1, -1) = none.
    static SHADE: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((-1, -1)) };
    /// Endless wave entry banner (two lines, empty = none).
    static WAVE_INTRO: RefCell<[Vec<u8>; 2]> = const { RefCell::new([Vec::new(), Vec::new()]) };
}

/// achievements.c : ACH_NOTIFY_Y.
const ACH_NOTIFY_Y: i32 = 3;

/// achievements.c:achievements_draw_notification: y position of the banner when
/// the statistics or achievements screen is open.
const ACH_NOTIFY_Y_OVERLAY: i32 = 170;

/// `stats.c:stats_display` and `achievements.c:achievements_display`: two
/// passes of `JE_barShade` on the play area (x 24..287 of game_screen).
/// Darkened band open? "None" is (-1, -1) (`LevelOverlays.shade_rows`)
/// or any empty band: `y1 >= y0` alone took (-1, -1) for a band and
/// placed the achievement banner at y 170 permanently (HD-01).
fn shade_active(y0: i32, y1: i32) -> bool {
    y0 >= 0 && y1 >= y0
}

fn draw_overlay_shade(px: &mut [u8]) {
    let (y0, y1) = SHADE.with(|s| s.get());
    if !shade_active(y0, y1) {
        return;
    }
    let mut surf = Surface::new(320, 200);
    surf.pixels.copy_from_slice(px);
    for _ in 0..2 {
        surf.bar_shade(PLAY_AREA_X as i32, y0, PLAY_AREA_X as i32 + PLAY_W as i32 - 1, y1);
    }
    px.copy_from_slice(&surf.pixels);
}

/// `achievements.c:achievements_draw_notification`, called by
/// JE_starShowVGA right after the copy of game_screen into VGAScreenSeg,
/// hence before the composition of the RGBA layer (the shot glows go
/// over the text). Intended deviations from the C kept: centered in the play
/// area, under the top text (countdown, boss bar, Secret Level).
/// `px`: play area in game_screen coordinates (x + 24).
fn draw_banner(px: &mut [u8], sim: View) {
    BANNER.with(|b| {
        let text = b.borrow();
        if text.is_empty() {
            return;
        }
        let mut surf = Surface::new(320, 200);
        surf.pixels.copy_from_slice(px);
        with_assets(|a| {
            let x = (PLAY_W as i32 - tyrian_render::font::text_width(a, &text, tyrian_render::assets::TINY_FONT)) / 2;
            let y = if SHADE.with(|s| shade_active(s.get().0, s.get().1)) {
                ACH_NOTIFY_Y_OVERLAY
            } else {
                ACH_NOTIFY_Y.max(crate::hud::top_text_bottom(sim.world()) + 2)
            };
            tyrian_render::font::text_shade(
                &mut surf,
                a,
                x + PLAY_AREA_X as i32,
                y,
                &text,
                15,
                4,
                tyrian_render::font::FULL_SHADE,
            );
        });
        px.copy_from_slice(&surf.pixels);
    });
}

/// endless.c:endless_draw_hud, wave entry banner: JE_dString of the two
/// lines, centered on the screen (JE_fontCenter), at h/2 - 18 and h/2 + 6.
fn draw_wave_intro(px: &mut [u8]) {
    WAVE_INTRO.with(|w| {
        let lines = w.borrow();
        if lines[0].is_empty() {
            return;
        }
        let mut surf = Surface::new(320, 200);
        surf.pixels.copy_from_slice(px);
        with_assets(|a| {
            use tyrian_render::assets::SMALL_FONT_SHAPES;
            for (text, y) in [(&lines[0], 200 / 2 - 18), (&lines[1], 200 / 2 + 6)] {
                // Screen (VGAScreenSeg): x + 24 in game_screen.
                let x = 160 - tyrian_render::font::text_width(a, text, SMALL_FONT_SHAPES) / 2;
                tyrian_render::font::dstring(&mut surf, a, x + PLAY_AREA_X as i32, y, text, SMALL_FONT_SHAPES);
            }
        });
        px.copy_from_slice(&surf.pixels);
    });
}

/// tyrian2.c, level loop after layer 2 "over 2": "Warning"
/// block (blinking bars and word, armor < 6), "Secret Level"
/// (displayTime), "Level Timer" and "GAME OVER", in game_screen.
fn draw_texts(surf: &mut Surface, a: &mut tyrian_render::Assets, sim: View) {
    use tyrian_render::assets::{FONT_SHAPES, SMALL_FONT_SHAPES, TINY_FONT};
    use tyrian_render::font;
    let t = &sim.world().render.drawn_texts;
    TEXTS.with(|texts| {
        let texts = texts.borrow();
        if t.warning {
            let c = t.warning_col;
            surf.fill_rectangle_xy(24, 181, 138, 183, c);
            surf.fill_rectangle_xy(175, 181, 287, 183, c);
            surf.fill_rectangle_xy(24, 0, 287, 3, c);
            let ww = font::text_width(a, &texts[0], TINY_FONT);
            font::out_text(surf, a, 138 + (37 - ww) / 2, 178, &texts[0], 7, ((c % 16) / 2) as i32);
        }
        if t.secret {
            font::out_text_and_darken(
                surf,
                a,
                90,
                10,
                &texts[1],
                15,
                (t.secret_flash as u32).wrapping_sub(8),
                FONT_SHAPES,
            );
        }
        if let Some(cd) = t.timer {
            font::text_shade(surf, a, 140, 6, &texts[2], 7, ((cd % 20) / 3) as i32, font::FULL_SHADE);
            let digits = format!("{}.{}", cd / 100, (cd / 10) % 10);
            font::dstring(surf, a, 100, 2, digits.as_bytes(), SMALL_FONT_SHAPES);
        }
        if t.game_over {
            font::dstring(surf, a, 120, 60, &texts[3], FONT_SHAPES);
        }
    });
}

/// `player_movement.c:JE_playerMovement`: during the title demo,
/// "INSERT COIN" in SMALL_FONT_SHAPES at (115, 10) of game_screen, after the
/// trail and before the ship.
fn draw_demo_text(surf: &mut Surface, a: &mut tyrian_render::Assets, sim: View) {
    if sim.world().demo.is_none() {
        return;
    }
    DEMO_TEXT.with(|t| {
        tyrian_render::font::dstring(surf, a, 115, 10, &t.borrow(), tyrian_render::assets::SMALL_FONT_SHAPES)
    });
}

/// `level_helpers.c:draw_boss_bar` (drawing): JE_barX at (x +- 25, y), then the
/// remaining part in color 118 + flash.
fn draw_boss_bars(surf: &mut Surface, sim: View) {
    let w = sim.world();
    let t = &w.render.drawn_texts;
    let bar_x = |surf: &mut Surface, x1: i32, y1: i32, x2: i32, y2: i32, col: u8| {
        surf.fill_rectangle_xy(x1, y1, x2, y1, col.wrapping_add(1));
        surf.fill_rectangle_xy(x1, y1 + 1, x2, y2 - 1, col);
        surf.fill_rectangle_xy(x1, y2, x2, y2, col.wrapping_sub(1));
    };
    for b in 0..t.boss_bars as usize {
        let x = if t.boss_bars == 2 {
            if b == 0 {
                125
            } else {
                185
            }
        } else if w.script.level_timer {
            250
        } else {
            155
        };
        let y = if w.script.level_timer { 15 } else { 7 };
        let (armor, color) = t.boss[b];
        bar_x(surf, x - 25, y, x + 25, y + 5, 115);
        bar_x(surf, x - (armor / 10) as i32, y, x + ((armor + 5) / 10) as i32, y + 5, (118 + color) as u8);
    }
}

/// `backgrnd.c:lava_filter`: average of the source (x2) and of the
/// destination pixels above and below, shifted by a ripple, red tint.
/// C traversal, from the end of the first 185 lines toward the start.
fn lava_filter(dst: &mut [u8], src: Option<&[u8]>, _: u8) {
    let mut w: i32 = 320 * 185 - 1;
    let mut i: i32 = 320 * 185;
    for _ in 0..185 * 40 {
        let waver = (((w >> 9) & 0x0f) - 8).abs() - 1;
        w -= 8;
        for _ in 0..8 {
            i -= 1;
            let s = |k: i32| src.map_or(dst[k as usize], |s| s[k as usize]);
            let mut value: u32 = 0;
            if i + waver >= 0 {
                value += (s(i + waver) & 0x0f) as u32 * 2;
            }
            value += (dst[(i + waver + 320) as usize] & 0x0f) as u32;
            if i + waver - 320 >= 0 {
                value += (dst[(i + waver - 320) as usize] & 0x0f) as u32;
            }
            dst[i as usize] = (value / 4) as u8 | 0x70;
        }
    }
}

/// `backgrnd.c:water_filter`: blue pixel (bits 0x30) averaged with the
/// destination pixel of the line below (ripple), tint `hue`.
fn water_filter(dst: &mut [u8], src: Option<&[u8]>, hue: u8) {
    let mut w: i32 = 320 * 185 - 1;
    let mut i: i32 = 320 * 185;
    for _ in 0..185 * 40 {
        let waver = (((w >> 10) & 0x07) - 4).abs() - 1;
        w -= 8;
        for _ in 0..8 {
            i -= 1;
            let sp = src.map_or(dst[i as usize], |s| s[i as usize]);
            dst[i as usize] = if sp & 0x30 == 0 {
                sp
            } else {
                let value = (sp & 0x0f) + (dst[(i + waver + 320) as usize] & 0x0f);
                (value / 2) | hue
            };
        }
    }
}

/// `backgrnd.c:iced_blur_filter` : moyenne source / destination, teinte glace.
fn iced_blur_filter(dst: &mut [u8], src: Option<&[u8]>, _: u8) {
    for i in 0..184 * 320 {
        let sp = src.map_or(dst[i], |s| s[i]);
        dst[i] = (((sp & 0x0f) + (dst[i] & 0x0f)) / 2) | 0x80;
    }
}

/// `backgrnd.c:blur_filter`: source / destination average, tint of the source.
fn blur_filter(dst: &mut [u8], src: Option<&[u8]>, _: u8) {
    for i in 0..184 * 320 {
        let sp = src.map_or(dst[i], |s| s[i]);
        dst[i] = (((sp & 0x0f) + (dst[i] & 0x0f)) / 2) | (sp & 0xf0);
    }
}

/// `enemy_render.c:JE_starShowVGA`, copy of game_screen to the screen:
/// code 1 image flipped vertically, code 2 (`processorType >= 2`) lamp
/// centered on player 1 (`lighty = 172 - y`, `lightx = 281 - x`), otherwise
/// copy as is. Rewrites the play area of `px` in place.
fn star_show_copy(px: &mut [u8], sim: View) {
    let w = sim.world();
    let row = |y: usize| y * PITCH as usize + PLAY_AREA_X;
    match w.render.star_show_code {
        1 => {
            let src: Vec<u8> = px.to_vec();
            for y in 0..PLAY_H {
                let (d, s) = (row(y), row(PLAY_H - 1 - y));
                px[d..d + PLAY_W].copy_from_slice(&src[s..s + PLAY_W]);
            }
        }
        2 => {
            let lighty = 172 - w.players[0].y;
            let lightx = 281 - w.players[0].x;
            for r in 0..PLAY_H {
                for c in 0..PLAY_W {
                    let i = row(r) + c;
                    px[i] = lamp_index(px[i], c, r, lightx, lighty);
                }
            }
        }
        _ => {}
    }
}

/// `backgrnd.c:JE_darkenBackground` (detail level 6, SuperWild): each
/// pixel of the play area keeps its hue (high nibble) and its low nibble
/// is mixed with a pattern that depends on `neat`, the position and the already
/// processed pixels (`s[-2]` and the line above, read after modification). `x` and `y`
/// count down as in the C (`y` is 184 on the first line).
fn darken_background(px: &mut [u8], neat: u16) {
    for y_count in (1..=PLAY_H as i32).rev() {
        let row = (PLAY_H as i32 - y_count) as usize;
        for x_count in (1..=PLAY_W as i32).rev() {
            let col = PLAY_W - x_count as usize;
            let i = row * PITCH as usize + PLAY_AREA_X + col;
            let s = px[i] as i32;
            let left = if i >= 2 { px[i - 2] as i32 } else { 0 };
            // `*(s - (pitch - 1))`: pixel of the line above, one column to the right.
            let above = if y_count == PLAY_H as i32 { 0 } else { px[i + 1 - PITCH as usize] as i32 };
            let n = (((x_count - neat as i32 - y_count) >> 2) + left + above) & 0x0f;
            let low = (((s & 0x0f) << 4) - (s & 0x0f) + n) >> 4;
            px[i] = (low | (s & 0xf0)) as u8;
        }
    }
}

/// `backgrnd.c:JE_filterScreen`, drawing part (default setting 4:
/// `filtrationAvail` and `explosionTransparent` true): tint imposed on the
/// high nibble, then brightness added to the low nibble, clamped to 15 and
/// brought back to 0 below zero (overflow of the C unsigned integer).
fn draw_filter(px: &mut [u8], sim: View) {
    let Some((col, int_)) = sim.world().render.drawn_filter else {
        return;
    };
    for y in 0..PLAY_H {
        for x in 0..PLAY_W {
            let i = y * PITCH as usize + PLAY_AREA_X + x;
            px[i] = filter_index(px[i], col, int_);
        }
    }
}

/// One pixel of `JE_filterScreen`: tint `col` imposed, brightness `int_`
/// added (-99 = none).
fn filter_index(s: u8, col: i8, int_: i8) -> u8 {
    let mut s = s;
    if col != -99 {
        s = ((col as u8) << 4) | (s & 0x0f);
    }
    if int_ != -99 {
        let t = ((s & 0x0f) as u32).wrapping_add(int_ as i32 as u32);
        let v = if t >= 0x1f {
            0
        } else if t >= 0x0f {
            0x0f
        } else {
            t as u8
        };
        s = (s & 0xf0) | v;
    }
    s
}

/// One pixel of the `JE_starShowVGA` lamp (code 2), column `c` and row
/// `r` of the play area.
pub(crate) fn lamp_index(s: u8, c: usize, r: usize, lightx: i32, lighty: i32) -> u8 {
    let y = (PLAY_H - r) as i32; // the C counts y from 184 down to 1
    let x = (PLAY_W - c) as i32; // and x from 264 down to 1
    if lighty > y {
        return (s & 0xf0) | ((s >> 2) & 0x03);
    }
    let d = (lightx - x).abs() + lighty;
    if d < y {
        s
    } else if d - y <= 5 {
        (s & 0xf0) | ((((s & 0x0f) as i32 + 3 * (5 - (d - y))) / 4) as u8)
    } else {
        (s & 0xf0) | ((s & 0x0f) >> 2)
    }
}

/// `backgrnd.c:update_and_draw_starfield`, drawing part (positions already
/// advanced by the simulation): from the last star to the first, a
/// pixel is only placed on index 0; 3 px cross (color - 4) outside the
/// far layer, 5 px cross (- 8) for the near layer.
fn draw_starfield(px: &mut [u8], sim: View) {
    let w = sim.world();
    if w.render.starfield_drawn {
        draw_stars(px, &w.render.starfield);
    }
}

/// Drawing of a starfield on a 320-wide screen (play area,
/// shop fire demonstration).
pub(crate) fn draw_stars(px: &mut [u8], stars: &[StarfieldStar]) {
    const HUE: i32 = 0x90;
    fn put(px: &mut [u8], i: usize, c: i32) {
        if px[i] == 0 {
            px[i] = c as u8;
        }
    }
    let pitch = PITCH as usize;
    for s in stars.iter().rev() {
        let p = s.position as usize;
        if p >= 177 * pitch {
            continue;
        }
        let c = s.color as i32;
        put(px, p, c);
        if s.layer == 0 {
            continue;
        }
        if c - 4 >= HUE {
            put(px, p + 1, c - 4);
            if p > 0 {
                put(px, p - 1, c - 4);
            }
            put(px, p + pitch, c - 4);
            if p >= pitch {
                put(px, p - pitch, c - 4);
            }
        }
        if s.layer == 2 && c - 8 >= HUE {
            put(px, p + 2, c - 8);
            if p > 1 {
                put(px, p - 2, c - 8);
            }
            put(px, p + 2 * pitch, c - 8);
            if p >= 2 * pitch {
                put(px, p - 2 * pitch, c - 8);
            }
        }
    }
}

/// `varz.c:JE_drawSP` on `px` from `World::superpixels`.
/// Geometry of `JE_doSP` reproduced in floats like the C:
/// tempr = (float)(rng_next_float() * (2*M_PI)) (product in double),
/// tempy = roundf(cosf(tempr) * f2 * explowidth), tempx = roundf(sinf(tempr) * f3 * explowidth)
/// (glibc sinf/cosf from tyrian_sim::f32ops). After n draws:
/// x = cx + tempx*(n + 1), y = cy + tempy + n*(tempy + 1) (unsigned).
fn draw_superpixels(px: &mut [u8], sim: View) {
    use tyrian_sim::f32ops::F32;
    let w = sim.world();
    for sp in w.render.superpixels.iter().rev() {
        if sp.drawn_z == 0 {
            continue;
        }
        let f = |b: u32| b as f32 / (1u32 << 24) as f32;
        let tempr = (f(sp.rnd[0]) as f64 * (2.0 * std::f64::consts::PI)) as f32;
        let (c, s) = (f32::from_bits(F32(tempr.to_bits()).cos().0), f32::from_bits(F32(tempr.to_bits()).sin().0));
        let ew = sp.explowidth as f32;
        let tempy = (c * f(sp.rnd[1]) * ew).round() as i32;
        let tempx = (s * f(sp.rnd[2]) * ew).round() as i32;
        let n = sp.moves as i32;
        let x = (sp.cx as i32 + tempx * (n + 1)) as u32;
        let y = (sp.cy as i32 + tempy + n * (tempy + 1)) as u32;
        if x >= 320 || y >= 200 {
            continue;
        }
        let z = sp.drawn_z as u32;
        let i = (y * 320 + x) as usize;
        let mix = |v: u8, z: u32| ((((v & 0x0f) as u32 + z) >> 1) as u8).wrapping_add(sp.color);
        px[i] = mix(px[i], z);
        if x > 0 {
            px[i - 1] = mix(px[i - 1], z >> 1);
        }
        if x < 319 {
            px[i + 1] = mix(px[i + 1], z >> 1);
        }
        if y > 0 {
            px[i - 320] = mix(px[i - 320], z >> 1);
        }
        if y < 199 {
            px[i + 320] = mix(px[i + 320], z >> 1);
        }
    }
}

/// Composition for display. With an active image filter,
/// `playfield_filters_tick` has already advanced `GAME_SCREEN` to the image of the
/// current tick: recomposing with it would blend the image with itself (no more
/// persistence, grain different from the C: D2 ep. 3-4). The rendering thus restarts from
/// the state from before the tick and does not touch the advanced state.
fn compose_for_render(full: &Sim, data_dir: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let prev = GAME_SCREEN_PREV.with(|p| p.borrow().clone());
    let Some(prev) = prev else {
        return compose_indexed(full, data_dir);
    };
    let advanced = GAME_SCREEN.with(|g| std::mem::replace(&mut *g.borrow_mut(), prev));
    let out = compose_indexed(full, data_dir);
    GAME_SCREEN.with(|g| *g.borrow_mut() = advanced);
    out
}

/// An image filter of tyrian2.c:JE_main. At the first filter, the scenery drawn
/// in VGAScreen2 (`px`) is blended with the game_screen of the previous image; at the
/// following ones VGAScreen is already game_screen: what has been drawn since the
/// previous filter (layer 2, enemies) is there and must be copied into
/// `game` before filtering (ASSASSIN: lava then blur, clouds gone).
fn chain_filter(px: &mut [u8], game: &mut [u8], on_game: &mut bool, f: fn(&mut [u8], Option<&[u8]>, u8), arg: u8) {
    if *on_game {
        game.copy_from_slice(px);
        f(game, None, arg);
    } else {
        f(game, Some(px), arg);
        *on_game = true;
    }
    px.copy_from_slice(game);
}

/// 8-bit composition of the play area like tyrian2.c, up to the presented
/// image (before JE_starShowVGA): (320x200 pixels, image right after the
/// ship for the reflection). Updates GAME_SCREEN (image filters).
fn compose_indexed(full: &Sim, data_dir: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    // Upper half of the iteration (scenery, enemies, player shots), then
    // lower half (player, enemy shots, explosions, texts).
    let sim = View::top(full);
    let bottom = View::bottom(full);
    let level = &sim.setup().level;
    let name = level.events.header.shapes_file_name();
    TILESET.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.as_ref().is_none_or(|(n, _)| *n != name) {
            let data = std::fs::read(Path::new(data_dir).join(&name)).ok()?;
            *cache = Some((name.clone(), TileSet::parse(&data).ok()?));
        }
        let (_, tiles) = cache.as_ref()?;
        debug_assert_eq!(tiles.tiles.first().and_then(|t| t.pixels.as_ref()).map_or(TILE_SIZE, |p| p.len()), TILE_SIZE);
        let w = sim.world();
        let mut px = vec![0u8; SURFACE_LEN as usize];
        let drawn = |l: usize| {
            w.render.drawn_layer[l]
                .map_or((w.background.map_y_pos[l], w.background.back_pos[l] as i32), |(y, b)| (y, b as i32))
        };
        // tyrian2.c: astralDuration > 0 => JE_clr256 instead of layer 1.
        if w.special_weapon.astral_duration == 0 {
            let (y, b) = drawn(0);
            draw_layer(
                &mut px,
                &level.map,
                tiles,
                0,
                y,
                w.background.map_x_bp_pos,
                w.background.map_x_pos as i32,
                b,
                false,
            );
        }
        draw_starfield(&mut px, sim);
        // tyrian2.c: image filters ("smoothies", processorType 4).
        // With a filter, the scenery is drawn in VGAScreen2 (`px`) then
        // the first filter blends it with the game_screen of the previous image;
        // the rest is drawn in game_screen (`on_game` true).
        let sm = w.background.smoothies;
        let mut game = GAME_SCREEN.with(|g| g.borrow().clone());
        let mut on_game = !(sm[0] || sm[1] || sm[2] || sm[3] || sm[4]);
        let filter =
            |px: &mut Vec<u8>, game: &mut Vec<u8>, on_game: &mut bool, f: fn(&mut [u8], Option<&[u8]>, u8), arg: u8| {
                chain_filter(px, game, on_game, f, arg)
            };
        if on_game {
            game.copy_from_slice(&px);
        }
        if sm[4] {
            filter(&mut px, &mut game, &mut on_game, iced_blur_filter, 0);
        }
        // tyrian2.c:JE_main: layer 2 (over 0/3), ground enemies
        // (JE_drawEnemy(50) then (100)), layer 2 (over 1).
        if matches!(w.background.background2over, 0 | 3) {
            draw_layer2(&mut px, sim, tiles);
        }
        if sm[0] && w.background.smoothie_data[0] == 0 {
            filter(&mut px, &mut game, &mut on_game, lava_filter, 0);
        }
        if sm[1] {
            filter(&mut px, &mut game, &mut on_game, water_filter, w.background.smoothie_data[1] << 4);
        }
        draw_enemies(&mut px, sim, &[0, 1]);
        if sm[0] && w.background.smoothie_data[0] > 0 {
            filter(&mut px, &mut game, &mut on_game, lava_filter, 0);
        }
        let superwild = sim.world().processor_type == 6;
        if superwild {
            darken_background(&mut px, w.render.drawn_neat.0);
            if on_game {
                game.copy_from_slice(&px);
            }
        }
        if w.background.background2over == 1 {
            draw_layer2(&mut px, sim, tiles);
        }
        if superwild {
            darken_background(&mut px, w.render.drawn_neat.1);
            if on_game {
                game.copy_from_slice(&px);
            }
        }
        // Rest of tyrian2.c:JE_main up to the player shots: layer 3
        // (over 2), sky enemies (!skyEnemyOverAll), layer 3 (over 0),
        // top enemies (!topEnemyOver), JE_updatePlayerShots.
        let p = parts(sim);
        if w.background.background3over == 2 {
            draw_layer3(&mut px, sim, tiles);
        }
        if sm[2] {
            filter(&mut px, &mut game, &mut on_game, iced_blur_filter, 0);
        }
        if sm[3] {
            filter(&mut px, &mut game, &mut on_game, blur_filter, 0);
        }
        // With skyEnemyOverAll / topEnemyOver, these enemies are only drawn
        // once, later (draw_rest); drawing them here too
        // would leave their image from the previous tick under the right one.
        if p & PART_SKY != 0 && !w.background.sky_enemy_over_all {
            draw_enemies(&mut px, sim, &[2]);
        }
        if w.background.background3over == 0 {
            draw_layer3(&mut px, sim, tiles);
        }
        if p & PART_TOP != 0 && !w.background.top_enemy_over {
            draw_enemies(&mut px, sim, &[3]);
        }
        // Ship shadows under the shots. `shadow_ofs`: mapX2Ofs from the head of the
        // iteration; the C draws the shadows before recomputing the scroll
        // (mainint.c, "Parallax Map Scrolling").
        draw_ship_shadows(&mut px, bottom, sim.world().background.map_x2_ofs as i32);
        draw_player_shots(&mut px, sim);
        let after_player = draw_rest(&mut px, bottom, tiles);
        // game_screen of this image, read by the filters of the next one.
        GAME_SCREEN.with(|g| g.borrow_mut().copy_from_slice(&px));
        Some((px, after_player))
    })
}

#[cfg(test)]
mod filter_chain_tests {
    use super::*;

    /// Second filter of a chain: what is drawn between the two (px)
    /// must pass into the result (tyrian2.c:1131-1184, VGAScreen = game_screen).
    #[test]
    fn second_filter_keeps_what_was_drawn_since_the_first() {
        let mut px = vec![0u8; SURFACE_LEN as usize];
        let mut game = vec![0u8; SURFACE_LEN as usize];
        let mut on_game = false;
        chain_filter(&mut px, &mut game, &mut on_game, blur_filter, 0);
        assert!(on_game);
        // Drawing of a light tile after the first filter (clouds).
        px[1000] = 0x8f;
        chain_filter(&mut px, &mut game, &mut on_game, blur_filter, 0);
        assert_eq!(px[1000] & 0xf0, 0x80, "the cloud drawn between the filters disappeared");
    }
}

#[cfg(test)]
mod shot_shadow_order_tests {
    use super::*;

    /// Minimal Sprite2 sheet: one row of 12 pixels of value `v`. Two index
    /// entries (the C rejects `index >= count`), frame 1 is the row.
    fn one_row_sheet(v: u8) -> Vec<u8> {
        let mut sheet = vec![4, 0, 4, 0, 0xC0];
        sheet.extend([v; 12]);
        sheet.push(0x0f);
        sheet
    }

    fn shot(x: i32, y: i32) -> ShotDraw {
        ShotDraw { x, y, kind: ShotKind::Sheet(7, 1), shadow: true }
    }

    /// Two overlapping shots 10 px apart (overlapping bombs): the shadow of the
    /// upper shot falls on the lower shot. The lower shot must stay whole; its
    /// own shadow stays visible under it.
    #[test]
    fn shadow_of_upper_shot_does_not_darken_lower_shot() {
        let sheet = one_row_sheet(0x4f);
        let mut surf = Surface::new(320, 200);
        surf.pixels.fill(0x3c);
        // The lower shot is drawn first in the C list.
        let draws = [shot(100, 40), shot(100, 30)];
        blit_player_shots(&mut surf, &draws, [&sheet, &sheet], &[]);
        let at = |x: usize, y: usize| surf.pixels[y * 320 + x];
        assert_eq!(at(100, 40), 0x4f, "the lower shot is darkened by the shadow of the upper shot");
        assert_eq!(at(100, 30), 0x4f);
        assert_eq!(at(100, 50), 0x36, "the shadow of the lower shot is missing");
    }

    /// Without overlap, the order changes nothing: the shadow is darkened, the shot intact.
    #[test]
    fn isolated_shot_keeps_its_shadow() {
        let sheet = one_row_sheet(0x4f);
        let mut surf = Surface::new(320, 200);
        surf.pixels.fill(0x3c);
        blit_player_shots(&mut surf, &[shot(100, 40)], [&sheet, &sheet], &[]);
        assert_eq!(surf.pixels[40 * 320 + 100], 0x4f);
        assert_eq!(surf.pixels[50 * 320 + 100], 0x36);
    }
}

#[cfg(test)]
mod golden_image_tests {
    //! Image check of the golden masters (ASSASSIN, GRYPHON, LAVA EXIT): play
    //! area composed as 8-bit indices (scenery, lava, blur, clouds, enemies, shots)
    //! compared with the images that the C (c-final, GOLDEN_FRAMES) presents at the same
    //! ticks. The C RGBA layer (particles, divergence 7) is not in the
    //! indices: the comparison covers the 8-bit image alone.
    use super::*;
    use std::path::PathBuf;
    use tyrian_golden::GoldenMaster;
    use tyrian_sim::PlayerInput;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    /// Score corner (x 0..31, y 172..183 of the play area): drawn by the
    /// Godot dashboard (hud_vga), not by this composition; excluded.
    fn in_score_corner(x: usize, y: usize) -> bool {
        x < 32 && y >= 172
    }

    /// 64-bit FNV-1a fingerprint of the play area indices (x 24..287,
    /// y 0..183), score corner at 0.
    fn play_area_hash(px: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for y in 0..PLAY_H {
            for x in 0..PLAY_W {
                let b = if in_score_corner(x, y) { 0 } else { px[y * 320 + PLAY_AREA_X + x] };
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
        }
        h
    }

    /// Image presented at the tick: like `playfield_indices` without banner or
    /// overlay (none during a tool replay).
    fn presented(full: &Sim, data: &str) -> Vec<u8> {
        let (mut px, _) = compose_for_render(full, data).expect("composition");
        star_show_copy(&mut px, View::bottom(full));
        px
    }

    /// Replays `rewrite/golden/<name>.gm` and compares the images with the fingerprints
    /// `rewrite/golden/frames/<name>.txt`.
    fn frames_match_the_c(name: &str) {
        let data = root().join("data");
        let f = root().join(format!("rewrite/golden/{name}.gm"));
        let refs = root().join(format!("rewrite/golden/frames/{name}.txt"));
        if !data.join("tyrian.hdt").exists() || !f.exists() || !refs.exists() {
            eprintln!("data, golden master or hashes missing: test skipped");
            return;
        }
        let read_hashes = |path: &Path| -> Vec<(u64, u64)> {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|l| !l.starts_with('#') && !l.starts_with("clock_offset") && !l.trim().is_empty())
                .map(|l| {
                    let mut it = l.split_whitespace();
                    (it.next().unwrap().parse().unwrap(), u64::from_str_radix(it.next().unwrap(), 16).unwrap())
                })
                .collect()
        };
        let mut want = read_hashes(&refs);
        // Ticks that deliberately differ from the C (divergence 59a: the shadow no longer darkens
        // an already drawn sprite). Each must exist in the C references and differ from them.
        let overrides = root().join(format!("rewrite/golden/frames/{name}.godot.txt"));
        if overrides.exists() {
            for (tick, hash) in read_hashes(&overrides) {
                let entry = want.iter_mut().find(|(k, _)| *k == tick).expect("tick missing from the C references");
                assert_ne!(entry.1, hash, "{name} tick {tick}: the Godot reference is identical to the C one");
                entry.1 = hash;
            }
        }
        {
            let mut g = crate::vga::ASSETS.lock().unwrap();
            if g.is_none() {
                *g = Some(tyrian_render::Assets::load(&data, false).expect("assets"));
            }
        }
        let data_s = data.to_string_lossy().to_string();
        GAME_SCREEN.with(|g| g.borrow_mut().fill(0));
        GAME_SCREEN_PREV.with(|p| *p.borrow_mut() = None);
        let gm = GoldenMaster::load(&f).expect("golden master");
        let mut sim = gm.make_sim(&data).expect("sim");
        // Like TyrianSim.load_level: rendering starfield (separate generator,
        // seed 5489) and copy of the presented image at each tick.
        let mut mt = tyrian_sim::rng::MersenneTwister::new(5489);
        sim.set_starfield(tyrian_sim::world::initialize_starfield(gm.header.detail, &mut mt));
        sim.set_keep_presented(true);
        // C virtual clock outside the game frames (index.txt of GOLDEN_FRAMES),
        // like --clock-offset: blinking of low-armor enemies.
        let offset = std::fs::read_to_string(&refs)
            .unwrap()
            .lines()
            .find_map(|l| l.strip_prefix("clock_offset ").map(|v| v.trim().parse::<u32>().unwrap()))
            .unwrap_or(0);
        sim.set_render_clock_offset(offset);
        let mut got = Vec::new();
        for t in &gm.ticks {
            sim.step([PlayerInput(t.mask), PlayerInput(0)]);
            // level.gd:step_sim -> playfield_filters_tick after each tick.
            if View::top(&sim).world().background.smoothies[..5].iter().any(|&b| b) {
                let before = GAME_SCREEN.with(|g| g.borrow().clone());
                compose_indexed(&sim, &data_s);
                GAME_SCREEN_PREV.with(|p| *p.borrow_mut() = Some(before));
            } else {
                GAME_SCREEN_PREV.with(|p| *p.borrow_mut() = None);
            }
            if let Some(&(tick, _)) = want.iter().find(|(k, _)| *k == t.tick) {
                let px = presented(&sim, &data_s);
                if let Ok(dir) = std::env::var("GOLDEN_FRAME_DUMP") {
                    std::fs::write(Path::new(&dir).join(format!("godot-{tick}.idx")), &px).unwrap();
                }
                got.push((tick, play_area_hash(&px)));
            }
        }
        assert_eq!(got, want, "{name}: 8-bit frames differ from the C (tick, hash)");
    }

    #[test]
    fn assassin_frames_match_the_c() {
        frames_match_the_c("e1-assassin-autopilot");
    }

    /// GRYPHON: scenery without filter, stars, enemy shots, level end (tick 6900).
    #[test]
    fn gryphon_frames_match_the_c() {
        frames_match_the_c("e2-gryphon-autopilot");
    }

    /// LAVA EXIT (episode 4, file 9): lava and water active together from 3340 to
    /// 3390 (events 64 smoothies 1 and 2), lava alone before and after.
    #[test]
    fn lava_exit_frames_match_the_c() {
        frames_match_the_c("e4-lavaexit-autopilot");
    }
}

#[cfg(test)]
mod modern_explosion_readability_tests {
    //! Enemy shots under the Modern explosions: no enemy shot
    //! pixel is covered (reserve of the shots and their edge), on the
    //! busiest segments (large SAVARA enemy, death of the TYRIAN boss).
    use super::*;
    use tyrian_golden::GoldenMaster;
    use tyrian_sim::PlayerInput;

    /// (shot pixels seen, covered, frames where a proposal draws).
    fn covered(name: &str, ticks: std::ops::Range<u64>) -> Option<(u64, u64, u64)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let data = root.join("data");
        let f = root.join(format!("rewrite/golden/{name}.gm"));
        if !data.join("tyrian.hdt").exists() || !f.exists() {
            return None;
        }
        {
            let mut g = crate::vga::ASSETS.lock().unwrap();
            if g.is_none() {
                *g = Some(tyrian_render::Assets::load(&data, false).expect("assets"));
            }
        }
        let gm = GoldenMaster::load(&f).expect("golden master");
        let mut sim = gm.make_sim(&data).expect("sim");
        sim.set_fx_config(tyrian_sim::fx::FxConfig { events: true, ..Default::default() });
        sim.set_keep_presented(true);
        let mut fx = crate::fx_render::FxRender::new(7);
        fx.options.particles = true;
        fx.options.style = crate::fx_render::STYLE_MODERN;
        let (mut seen, mut hidden, mut frames) = (0u64, 0u64, 0u64);
        let mut layer = vec![[0u8; 4]; 320 * 200];
        for t in &gm.ticks {
            sim.step([PlayerInput(t.mask), PlayerInput(0)]);
            let ev = sim.take_fx_events();
            let w = sim.presented_world();
            fx.frame(ev, w, w.iteration, 0);
            if !ticks.contains(&t.tick) || fx.modern.live_count() == 0 {
                continue;
            }
            frames += 1;
            let view = View::bottom(&sim);
            let reserve = enemy_shot_reserve(view);
            let shots = {
                let mut surf = Surface::new(320, 200);
                surf.clear(0);
                with_assets(|a| draw_enemy_shots(&mut surf, a, view));
                let mut one = Surface::new(320, 200);
                one.clear(1);
                with_assets(|a| draw_enemy_shots(&mut one, a, view));
                (0..320 * 200)
                    .filter(|&j| j % 320 >= PLAY_AREA_X && (surf.pixels[j] != 0 || one.pixels[j] != 1))
                    .map(|j| j - PLAY_AREA_X)
                    .collect::<Vec<_>>()
            };
            if shots.is_empty() {
                continue;
            }
            layer.iter_mut().for_each(|p| *p = [0; 4]);
            fx.modern.draw(&mut layer, &reserve, None);
            hidden += shots.iter().filter(|&&j| layer[j][3] > 0).count() as u64;
            seen += shots.len() as u64;
        }
        Some((seen, hidden, frames))
    }

    fn check(name: &str, ticks: std::ops::Range<u64>) {
        let Some((seen, hidden, frames)) = covered(name, ticks.clone()) else {
            eprintln!("data or golden master missing: test skipped");
            return;
        };
        println!("{name}: {frames} frames drawn, {seen} enemy shot pixels, {hidden} covered");
        assert!(frames > 0, "{name}: no frame with the Modern explosions");
        assert!(seen > 0, "{name}: no enemy shot during the Modern explosions");
        assert_eq!(hidden, 0, "{name}: enemy shots covered");
    }

    #[test]
    fn savara_large_enemy_keeps_enemy_shots_visible() {
        check("e1-savara-autopilot", 1170..1280);
    }

    #[test]
    fn tyrian_boss_death_keeps_enemy_shots_visible() {
        check("e1-tyrian-autopilot", 6205..6975);
    }
}

#[cfg(test)]
mod banner_tests {
    use super::*;

    /// HD-01: without overlay (band (-1, -1)) the banner stays at the top (y 3).
    #[test]
    fn no_overlay_means_no_shade() {
        assert!(!shade_active(-1, -1));
        assert!(!shade_active(5, 4));
        assert!(shade_active(12, 136));
        assert!(shade_active(4, 183));
    }

    /// Ship trail at level end (player_movement.c, "End Level
    /// Fade-Out"): no copy at the first step (levelEnd 40), then up to 19.
    #[test]
    fn level_end_trail_matches_c() {
        assert!(level_end_trail_ys(100, 40).is_empty());
        // levelEnd 39: 2 trails, one copy at y.
        assert_eq!(level_end_trail_ys(100, 39), vec![100]);
        // levelEnd 37: 4 trails, 3 copies: y+5, y+1, y-2.
        assert_eq!(level_end_trail_ys(100, 37), vec![105, 101, 98]);
        // At most 20 trails (19 copies), bounded to 0 < y < 170.
        assert_eq!(level_end_trail_ys(-60, 10), vec![129, 109, 90, 72, 55, 39, 24, 10]);
        assert!(level_end_trail_ys(160, 10).iter().all(|&y| y > 0 && y < 170));
    }
}

#[cfg(test)]
mod darken_tests {
    use super::*;

    /// First pixel of the first line: x = 264, y = 184, no pixel
    /// above; `s[-2]` is 0 here. neat = 3: ((264 - 3 - 184) >> 2) = 19,
    /// 19 & 15 = 3; low = (0xA*16 - 0xA + 3) >> 4 = 9; high kept.
    #[test]
    fn darken_first_pixel_matches_c_formula() {
        let mut px = vec![0u8; SURFACE_LEN as usize];
        px[PLAY_AREA_X] = 0x5A;
        darken_background(&mut px, 3);
        assert_eq!(px[PLAY_AREA_X], 0x59);
    }

    /// Outside the play area (dashboard bars) nothing changes.
    #[test]
    fn darken_leaves_sidebars_alone() {
        let mut px = vec![0x37u8; SURFACE_LEN as usize];
        darken_background(&mut px, 7);
        assert_eq!(px[0], 0x37);
        assert_eq!(px[PLAY_AREA_X + PLAY_W], 0x37);
    }
}
