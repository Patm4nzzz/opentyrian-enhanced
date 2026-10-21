//! Full simulation state: transcription of the C global variables
//! involved in `tyrian2.c:JE_main/level_loop` and the functions
//! it calls. The integer types reproduce those of the C code (overflows
//! included); each field cites its original declaration.

use crate::rng::{MersenneTwister, Xoshiro256};
use crate::tables::Tables;
pub use crate::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};
use tyrian_formats::lvl::Event;

/// `varz.h`: ENEMY_MAX, ENEMY_SHOT_MAX, MAX_EXPLOSIONS, MAX_REPEATING_EXPLOSIONS.
pub const ENEMY_MAX: usize = 100;
pub const ENEMY_SHOT_MAX: usize = 60;
pub const MAX_EXPLOSIONS: usize = 200;
pub const MAX_REPEATING_EXPLOSIONS: usize = 20;
/// `shots.h:MAX_PWEAPON`.
pub const MAX_PWEAPON: usize = 81;
/// `lvlmast.h:EVENT_MAXIMUM`.
pub const EVENT_MAXIMUM: usize = 2500;

/// `config.h`: SHOT_* enumeration.
pub const SHOT_FRONT: usize = 0;
pub const SHOT_REAR: usize = 1;
pub const SHOT_LEFT_SIDEKICK: usize = 2;
pub const SHOT_RIGHT_SIDEKICK: usize = 3;
pub const SHOT_MISC: usize = 4;
pub const SHOT_P2_CHARGE: usize = 5;
pub const SHOT_P1_SUPERBOMB: usize = 6;
pub const SHOT_P2_SUPERBOMB: usize = 7;
pub const SHOT_SPECIAL: usize = 8;
pub const SHOT_NORTSPARKS: usize = 9;
pub const SHOT_SPECIAL2: usize = 10;

/// `config.h`: difficulty levels.
pub const DIFFICULTY_EASY: i8 = 1;
pub const DIFFICULTY_NORMAL: i8 = 2;
pub const DIFFICULTY_HARD: i8 = 3;
pub const DIFFICULTY_MANIACAL: i8 = 7;
pub const DIFFICULTY_10: i8 = 10;

/// Sprite sheets outside `newsh?.shp` (`enemy_spawn.c:JE_makeEnemy`):
/// bank 21 = `spriteSheet11` (coins and gems), 26 = `spriteSheet10`
/// (two-player objects), both in `tyrian.shp`.
pub const SHEET_TYRIAN_SHP_10: u8 = 10;
pub const SHEET_TYRIAN_SHP_11: u8 = 11;

/// `lvlmast.c:shapeFile`: enemy bank (1-based) → character of the `newsh?.shp` file.
pub const SHAPE_FILE: [u8; 36] = *b"2478ABCDEFGHIJKLMNOPQRSTU5#V0@3^59'%";

/// `varz.h:struct JE_SingleEnemyType` (game fields; purely rendering ones omitted).
#[derive(Debug, Clone, Copy, Default)]
pub struct Enemy {
    pub ex: i16,
    pub ey: i16,
    pub exc: i8,
    pub eyc: i8,
    pub excc: i8,
    pub eycc: i8,
    pub exccw: i8,
    pub eyccw: i8,
    pub armorleft: u8,
    pub eshotwait: [u8; 3],
    pub eshotmultipos: [u8; 3],
    pub enemycycle: u8,
    pub ani: u8,
    pub egr: [u16; 20],
    pub size: u8,
    pub linknum: u8,
    pub aniactive: u8,
    pub animax: u8,
    pub aniwhenfire: u8,
    /// `sprite2s`: character of the `newsh?.shp` file or SHEET_TYRIAN_SHP_*, 0 = none.
    pub sheet: u8,
    pub exrev: i8,
    pub eyrev: i8,
    pub exccadd: i16,
    pub eyccadd: i16,
    pub exccwmax: u8,
    pub eyccwmax: u8,
    pub edamaged: bool,
    pub enemytype: u16,
    pub animin: u8,
    pub edgr: u16,
    pub edlevel: i8,
    pub edani: i8,
    pub filter: u8,
    /// Rendering only: (ex, ey, enemycycle, filter, mapoffset) at the time of
    /// blit_enemy (enemy_render.c), `None` before the first drawing.
    pub drawn: Option<(i16, i16, u8, u8, u16)>,
    /// Rendering only: "motion blur" trail (enemy_render.c, Wild):
    /// `trail_ex/trail_ey` of the previous drawing (`trail_prev`, None after
    /// JE_makeEnemy: `trail_valid = false`) and the darkened position
    /// drawn at this tick (`trail_drawn`, if |exc| + |eyc| > 2).
    pub trail_prev: Option<(i16, i16)>,
    pub trail_drawn: Option<(i16, i16)>,
    pub evalue: i16,
    pub fixedmovey: i16,
    pub freq: [u8; 3],
    pub launchwait: u8,
    pub launchtype: u16,
    pub launchfreq: u8,
    pub xaccel: u8,
    pub yaccel: u8,
    pub tur: [u8; 3],
    pub enemydie: u16,
    pub enemyground: bool,
    pub explonum: u8,
    pub mapoffset: u16,
    pub scoreitem: bool,
    pub special: bool,
    pub flagnum: u8,
    pub setto: bool,
    pub iced: u8,
    pub launchspecial: u8,
    pub xminbounce: i16,
    pub xmaxbounce: i16,
    pub yminbounce: i16,
    pub ymaxbounce: i16,
    /// Data-driven behavior (behavior.rs): 1-based in `Tables::behaviors`,
    /// 0 = none (all the original content).
    pub behavior: u16,
    pub behavior_seg: u8,
    pub behavior_seg_tick: u16,
    pub behavior_tick: u16,
    pub behavior_turret_wait: [u8; crate::behavior::MAX_BEHAVIOR_TURRETS],
}

/// `shots.h:PlayerShotDataType`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerShot {
    pub shot_x: i16,
    pub shot_y: i16,
    pub shot_xm: i16,
    pub shot_ym: i16,
    pub shot_xc: i16,
    pub shot_yc: i16,
    pub shot_complicated: bool,
    pub shot_dev_x: i16,
    pub shot_dir_x: i16,
    pub shot_dev_y: i16,
    pub shot_dir_y: i16,
    pub shot_cir_size_x: i16,
    pub shot_cir_size_y: i16,
    pub shot_trail: u8,
    pub shot_gr: u16,
    pub shot_ani: u16,
    pub shot_ani_max: u16,
    /// Rendering only, outside the hash: image drawn by
    /// `shots.c:player_shot_move_and_draw` (`shotGr + shotAni` before
    /// the increment of `shotAni`).
    pub drawn_frame: u16,
    pub shot_dmg: u8,
    pub shot_blast_filter: u8,
    pub chain_reaction: u8,
    pub player_number: u8,
    pub aim_at_enemy: u8,
    pub aim_delay: u8,
    pub aim_delay_max: u8,
    /// `shots.c:player_shot_create`: `shot->shotBay = bay_i` — only used for the
    /// end-of-level tally (`stats.c:level_stats_record_damage`), outside the
    /// hash (`golden_master.c:hash_player_shots` does not read it).
    pub shot_bay: u8,
}

/// `varz.h:EnemyShotType`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnemyShot {
    pub sx: i16,
    pub sy: i16,
    pub sxm: i16,
    pub sym: i16,
    pub sxc: i8,
    pub syc: i8,
    pub tx: u8,
    pub ty: u8,
    pub sgr: u16,
    pub sdmg: u8,
    pub duration: u8,
    pub animate: u16,
    pub animax: u16,
}

/// `varz.h:Explosion`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Explosion {
    pub ttl: u8,
    pub x: i16,
    pub y: i16,
    pub sprite: u16,
    pub follow_player: bool,
    pub fixed_position: bool,
    pub delta_y: i16,
}

/// `varz.h:rep_explosion_type`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepExplosion {
    pub delay: u32,
    pub ttl: u32,
    pub x: u32,
    pub y: u32,
    pub big: bool,
}

/// Mounted weapon (`player.h:PlayerItems.weapon[]`): identifier and power level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeaponSlot {
    pub id: WeaponId,
    pub power: u8,
}

/// `player.h:PlayerItems`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Items {
    pub ship: ShipId,
    pub generator: GeneratorId,
    pub shield: ShieldId,
    /// Front and rear weapons; the `power` of player p's weapon also serves as a
    /// lives counter (`mainint.c:JE_initPlayerData`: `lives = &items.weapon[p].power`).
    pub weapon: [WeaponSlot; 2],
    pub sidekick: [SidekickId; 2],
    pub special: SpecialId,
    pub sidekick_series: u8,
    pub sidekick_level: u8,
}

/// `player.h:Player.sidekick[]`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SidekickState {
    pub ammo_max: i32,
    pub ammo_refill_ticks_max: u32,
    pub style: u32,
    pub x: i32,
    pub y: i32,
    pub ammo: i32,
    pub ammo_refill_ticks: u32,
    pub animation_enabled: bool,
    pub animation_frame: u32,
    pub charge: u32,
    pub charge_ticks: u32,
}

/// `player.h:Player`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Player {
    pub cash: u64,
    pub items: Items,
    pub is_dragonwing: bool,
    pub shield_max: u32,
    pub initial_armor: u32,
    pub shot_hit_area_x: u32,
    pub shot_hit_area_y: u32,
    pub is_alive: bool,
    pub invulnerable_ticks: u32,
    pub exploding_ticks: u32,
    pub shield: u32,
    pub armor: u32,
    pub weapon_mode: u32,
    pub superbombs: u32,
    pub purple_balls_needed: u32,
    pub x: i32,
    pub y: i32,
    pub old_x: [i32; 20],
    pub old_y: [i32; 20],
    pub x_velocity: i32,
    pub y_velocity: i32,
    pub x_friction_ticks: u32,
    pub y_friction_ticks: u32,
    pub shot_multi_pos: [u8; 11],
    pub delta_x_shot_move: i32,
    pub delta_y_shot_move: i32,
    pub last_x_shot_move: i32,
    pub last_y_shot_move: i32,
    pub last_x_explosion_follow: i32,
    pub last_y_explosion_follow: i32,
    pub sidekick: [SidekickState; 2],
    /// Banking computed by `player_movement.c` (rendering).
    pub banking: i32,
}

impl Player {
    /// `player.c`: `*lives` = power of the player's weapon.
    pub fn lives(&self, index: usize) -> u8 {
        self.items.weapon[index].power
    }
}

/// Rendering only: what the tyrian2.c loop draws into game_screen
/// in addition to the objects ("Warning" block, "secret level", "Level Timer",
/// "GAME OVER", level_helpers.c:draw_boss_bar), with the values at the time
/// of drawing. Outside the hash and the golden master.
#[derive(Debug, Clone, Copy, Default)]
pub struct DrawnTexts {
    /// `warningCol` / `warningColChange` (tyrian2.c: 120 and 1 at level start).
    pub warning_col: u8,
    pub warning_col_change: i8,
    /// Bars and WARNING word drawn this tick.
    pub warning: bool,
    /// `flash` / `flashChange` ("Secret Level"); `secret`: text drawn this tick.
    pub flash: u8,
    pub flash_change: i8,
    pub secret: bool,
    /// Value of `flash` at drawing (before `flash += flashChange`).
    pub secret_flash: u8,
    /// Countdown drawn (value after decrement), None otherwise.
    pub timer: Option<u16>,
    pub game_over: bool,
    /// Boss bars: count, then (armor, color before decrement).
    pub boss_bars: u8,
    pub boss: [(u32, u32); 2],
}

/// `level_helpers.c` / `tyrian2.c`: boss health bar (`boss_bar[2]`).
#[derive(Debug, Clone, Copy, Default)]
pub struct BossBar {
    pub link_num: u8,
    pub armor: u32,
    pub color: u32,
}

/// All the state variables of the level loop.
/// Cumulative counters of the event emissions that the listeners of
/// `stats.c` / `achievements.c` consume (`event_bus_emit` in the C code). Purely
/// observational: neither hashed nor read by the simulation, hence no effect on the
/// golden masters. `AchievementTracker` reads their differences at each tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AchEvents {
    /// EVENT_ENEMY_KILLED (`collision.c`, both death chains).
    pub kills: u32,
    /// `shots.c:player_shot_create` → `stats_count_shot` (shot created, shot_id < MAX_PWEAPON).
    pub shots_fired: u32,
    /// EVENT_SHOT_HIT (`shot_update.c`, non-lethal and lethal branches).
    pub shots_hit: u32,
    /// Sum of `shot_hit.damage` (`dps.c:stats_add_damage`).
    pub damage_dealt: u32,
    /// EVENT_PLAYER_HIT (`varz.c:JE_playerDamage`, damage > 0, any player).
    pub player_hits: u32,
    /// EVENT_BOSS_SPAWNED (`level_events.c`, event 79).
    pub boss_spawns: u32,
    /// EVENT_BOSS_DEFEATED (`collision.c:boss_track_flush`).
    pub bosses_defeated: u32,
    /// Sum of the `enemy_killed.value > 0` of EVENT_ENEMY_KILLED
    /// (`stats.c:stats_on_enemy_killed` → `stats_add_credits`).
    pub kill_credits: u64,
}

/// `stats.h:LevelStats`, fields read by the end-of-level tally screen
/// (`mainint.c:JE_endLevelAni`). Reset to zero at the start of each level
/// (`tyrian2.c:JE_main` → `stats.c:level_stats_reset`). Purely observational.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelStats {
    /// `damage_by_bay[5]`: front, rear, left sidekick, right sidekick,
    /// special (`stats.c:level_stats_record_damage`).
    pub damage_by_bay: [u32; 5],
    /// `damage_taken` / `damage_blocked` (`stats.c:level_stats_record_hit`).
    pub damage_taken: u32,
    pub damage_blocked: u32,
    /// `credits_start = player[0].cash` at level start.
    pub credits_start: u64,
}

impl LevelStats {
    /// `stats.c:level_stats_record_damage`: raw bay (config.h SHOT_*) →
    /// tally row; orphan SHOT_MISC: cannot be attributed.
    pub fn record_damage(&mut self, bay: u8, damage: u32) {
        let row = match bay as usize {
            SHOT_FRONT | SHOT_REAR | SHOT_LEFT_SIDEKICK | SHOT_RIGHT_SIDEKICK => bay as usize,
            SHOT_P2_CHARGE => SHOT_REAR,
            SHOT_SPECIAL | SHOT_SPECIAL2 | SHOT_P1_SUPERBOMB | SHOT_P2_SUPERBOMB | SHOT_NORTSPARKS => 4,
            _ => return,
        };
        self.damage_by_bay[row] = self.damage_by_bay[row].wrapping_add(damage);
    }
}

/// `varz.h:MAX_SUPERPIXELS`.
pub const MAX_SUPERPIXELS: usize = 101;

/// A "superpixel" of `varz.c:JE_doSP`, in integers: center, radius,
/// color and the three raw `rng_next_float` draws (24 bits); the floating
/// geometry (sinf/cosf/roundf) is computed by rendering
/// (tyrian-godot, playfield module). `z`: counter of `JE_drawSP`;
/// `drawn_z`: value of `z` at the last drawing (0 = not drawn), `moves`:
/// number of `x += delta_x` steps performed.
/// `backgrnd.c:StarfieldStar`: a star of the starfield (rendering only).
#[derive(Debug, Clone, Copy, Default)]
pub struct StarfieldStar {
    pub color: u8,
    /// Index into game_screen (stride 320), uint16_t that wraps around.
    pub position: u16,
    pub speed: i32,
    /// 0 far, 1 medium, 2 near.
    pub layer: u8,
}

impl World {
    /// `backgrnd.c:update_and_draw_starfield`, advancing the positions:
    /// `position += (speed + move_speed) * pitch` (uint16_t).
    pub fn advance_starfield(&mut self) {
        let speed = self.background.starfield_speed;
        for s in self.render.starfield.iter_mut() {
            s.position = (s.position as i32 + (s.speed + speed) * 320) as u16;
        }
    }
}

/// `backgrnd.c:MAX_STARS`.
pub const MAX_STARS: usize = 100;

/// `backgrnd.c:initialize_starfield`: four `mt_rand` per star for
/// `processorType >= 2` (default setting 4, four draws), three for the
/// Classic branch (`processorType < 2`, `backgrnd.c:513-518`): a single
/// layer 1, speed and color of the original path. The two draws of the
/// position are made left then right (GCC), proven against the 100 stars of the
/// C code captured with gdb. The number of draws (three) is the same in both
/// branches: the `mt_rand` stream is not shifted, only the drawn value changes.
pub fn initialize_starfield(processor_type: u8, mt: &mut MersenneTwister) -> [StarfieldStar; MAX_STARS] {
    const HUE: u32 = 0x90;
    let mut stars = [StarfieldStar::default(); MAX_STARS];
    for i in (0..MAX_STARS).rev() {
        let x = mt.next_u32() % 320;
        let y = mt.next_u32() % 200;
        let s = &mut stars[i];
        s.position = (x + y * 320) as u16;
        // backgrnd.c:513: Classic (processorType < 2) — simple star, a
        // single layer, speed/color of the original path.
        if processor_type < 2 {
            s.layer = 1;
            s.speed = (mt.next_u32() % 3 + 2) as i32;
            s.color = (mt.next_u32() % 16 + HUE) as u8;
            continue;
        }
        s.layer = if i < MAX_STARS / 2 {
            0
        } else if i < MAX_STARS * 8 / 10 {
            1
        } else {
            2
        };
        let (speed, color) = match s.layer {
            0 => (mt.next_u32() % 2 + 1, mt.next_u32() % 4 + HUE),
            1 => (mt.next_u32() % 3 + 2, mt.next_u32() % 8 + HUE + 4),
            _ => (mt.next_u32() % 3 + 4, mt.next_u32() % 4 + HUE + 12),
        };
        s.speed = speed as i32;
        s.color = color as u8;
    }
    stars
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Superpixel {
    pub cx: u16,
    pub cy: u16,
    pub explowidth: u8,
    pub color: u8,
    pub rnd: [u32; 3],
    pub z: u8,
    pub drawn_z: u8,
    pub moves: u8,
}

/// Dashboard drawing done by the C code in the middle of the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudOp {
    /// varz.c: JE_wipeShieldArmorBars (if `wipe`), JE_drawShield (if
    /// `shield`), JE_drawArmor (if `armor`), with the shield and armor of both
    /// players at that moment.
    Bars { wipe: bool, shield: bool, armor: bool, shields: [u32; 2], armors: [u32; 2] },
    /// player_movement.c "Super Charge Weapons": cell `level` (1..5) of the
    /// charge gauge of the two-player HUD, color 193 (off) or 204.
    ChargeCell { level: u8, color: u8 },
    /// varz.c:JE_drawOptions (+ JE_drawOptionLevel): icons and gauges of the
    /// sidekicks reset.
    Options,
    /// level_events.c:414: JE_drawTextWindow(outputs[index]), message of an
    /// event 16 (index 0..8) drawn in the bottom bar.
    EventText { index: u8 },
}

/// Flags that `config.c:JE_initProcessorType` derives from `processorType`:
/// `wild` (layer 2 blending) and `filtrationAvail` (tint of the level
/// filter) for 4 and 6; `explosionTransparent` (explosions and filter
/// brightness) for everything except 1.
pub fn pt_wild(pt: u8) -> bool {
    pt == 4 || pt == 6
}
pub fn pt_filtration_avail(pt: u8) -> bool {
    pt == 4 || pt == 6
}
pub fn pt_explosion_transparent(pt: u8) -> bool {
    pt != 1
}
/// `config.c:JE_initProcessorType`: `background2` false at processorType 1
/// (Visual Effects Off): neither layer 2 (`backgrnd.c:188`, draw_background_2
/// draws nothing, scrolling continues) nor ground shadows of ships
/// and shots (`player_movement.c:755`, `shots.c:291`). `tyrian2.c:1091`
/// sets it back to true for an "over 3" layer 2.
pub fn pt_background2(pt: u8, background2over: u8) -> bool {
    pt != 1 || background2over == 3
}

/// Background: scrolling of the three layers, starfield and "smoothies" effects (`backgrnd.c`, `tyrian2.c`).
#[derive(Debug, Clone)]
pub struct Background {
    pub smoothies: [bool; 9],
    pub smoothie_data: [u8; 9],
    pub star_active: bool,
    pub starfield_speed: i32,
    pub back_pos: [u16; 3],
    pub back_move: [u16; 3],
    pub map_x: u16,
    pub map_x2: u16,
    pub map_x3: u16,
    pub map_y: [u16; 3],
    /// `mapYPos`, `mapY2Pos`, `mapY3Pos`: flat indices into `megaDataN.mainmap`.
    pub map_y_pos: [i32; 3],
    /// `BKwrapN` / `BKwrapNto` (flat indices).
    pub bk_wrap: [i32; 3],
    pub bk_wrap_to: [i32; 3],
    pub map_x_pos: u16,
    pub old_map_x_ofs: u16,
    pub map_x_ofs: u16,
    pub map_x2_ofs: u16,
    pub map_x2_pos: u16,
    pub map_x3_pos: u16,
    pub old_map_x3_ofs: u16,
    pub map_x3_ofs: u16,
    pub temp_map_x_ofs: u16,
    pub map_x_bp_pos: i32,
    pub map_x2_bp_pos: i32,
    pub map_x3_bp_pos: i32,
    pub map1_y_delay: u8,
    pub map1_y_delay_max: u8,
    pub map2_y_delay: u8,
    pub map2_y_delay_max: u8,
    pub background2over: u8,
    pub background3over: u8,
    pub background3x1: bool,
    pub background3x1b: bool,
    pub background2_not_transparent: bool,
    pub top_enemy_over: bool,
    pub sky_enemy_over_all: bool,
    pub stop_backgrounds: bool,
    pub stop_background_num: u8,
    pub explode_move: u16,
    pub temp_back_move: u16,
}

/// Level event script and the settings it drives (`level_events.c`, `tyrian2.c`).
#[derive(Debug, Clone)]
pub struct EventScript {
    pub event_rec: Vec<Event>,
    pub max_event: u16,
    pub event_loc: u16,
    pub cur_loc: u16,
    pub return_loc: u16,
    pub level_enemy: Vec<u16>,
    pub level_enemy_max: u16,
    pub level_enemy_frequency: u16,
    pub enemies_active: bool,
    pub new_pl: [u8; 10],
    pub global_flags: [bool; 10],
    pub force_events: bool,
    pub super_enemy_254_jump: u16,
    pub random_explosions: bool,
    pub return_active: bool,
    pub level_timer: bool,
    pub level_timer_countdown: u16,
    pub level_timer_jump_to: u16,
    pub enemy_sprite_sheet_ids: [u8; 4],
    pub small_enemy_adjust: bool,
    pub enemy_continual_damage: bool,
    pub galaga_shot_freq: u16,
    pub boss_bar: [BossBar; 2],
    pub damage_rate: u8,
}

/// Level end and game over (`tyrian2.c`, `player_movement.c`).
#[derive(Debug, Clone)]
pub struct LevelEnding {
    pub end_level: bool,
    pub really_end_level: bool,
    pub ready_to_end_level: bool,
    pub wait_to_end_level: bool,
    pub level_end: u8,
    pub level_end_warp: i8,
    pub level_end_fx_wait: u16,
    pub first_game_over: bool,
    pub all_players_gone: bool,
    pub bonus_level: bool,
    pub next_level: u8,
    pub display_time: u32,
    pub cube_max: u16,
}

/// Special weapon in progress (`varz.c`, static variables of `JE_doSpecialShot`).
#[derive(Debug, Clone)]
pub struct SpecialWeaponState {
    pub special_wait: u8,
    pub next_special_wait: u8,
    pub flare_duration: u16,
    pub flare_start: bool,
    pub flare_col_chg: i8,
    pub zinglon_duration: u8,
    pub astral_duration: u8,
    pub special_weapon_filter: i8,
    pub special_weapon_freq: i8,
    pub special_weapon_wpn: u16,
    pub link_to_player: bool,
    pub spray_special: bool,
    pub fire_button_held: bool,
}

/// Level color filter (`config.c`, `backgrnd.c:JE_filterScreen`).
#[derive(Debug, Clone)]
pub struct LevelFilter {
    pub level_filter: i8,
    pub level_brightness: i8,
    pub filter_active: bool,
    /// Rendering only (`config.c`): level filter fade
    /// (`backgrnd.c:JE_filterScreen`), never read by gameplay.
    pub filter_fade: bool,
    pub filter_fade_start: bool,
    pub level_filter_new: i8,
    pub level_brightness_chg: i8,
}

/// Generator energy and shield recharge (`tyrian2.c`).
#[derive(Debug, Clone)]
pub struct Energy {
    pub power: u32,
    pub power_add: u32,
    pub shield_wait: u8,
    pub shield_t: u32,
}

/// Common sidekick placement (`varz.c`: `optionAttachment*`, `optionSatelliteRotate`).
#[derive(Debug, Clone)]
pub struct SidekickFormation {
    pub option_attachment_move: i16,
    pub option_attachment_linked: bool,
    pub option_attachment_return: bool,
    /// `varz.c:optionSatelliteRotate` (`float`, by its bits): angle of
    /// style 4 sidekicks (orbit). Global in C, never reset to zero.
    pub option_satellite_rotate: crate::f32ops::F32,
}

/// Commands read during `player_movement.c:JE_playerMovement` (mouse, buttons, SF codes, mode change).
#[derive(Debug, Clone)]
pub struct Controls {
    pub mouse_x: u16,
    pub mouse_y: u16,
    pub button: [bool; 4],
    pub move_ok: bool,
    pub sf_current_code: [[u8; 21]; 2],
    pub sf_executed: [u8; 2],
    pub port_config_change: bool,
    pub port_config_done: bool,
}

/// Rendering only: values taken at the moment the C code draws, outside the hash and the golden master.
#[derive(Debug, Clone)]
pub struct RenderOnly {
    /// `power` at the moment tyrian2.c draws the HUD energy gauge
    /// (after shield recharge and `power += powerAdd`, before the shots):
    /// rendering only, outside the state hash.
    pub hud_power: u32,
    /// Rendering only: (`mapYPos`, `backPos`) of each layer at the moment
    /// backgrnd.c draws it (layers 1 and 2 before their movement, layer 3
    /// after). `None`: layer not drawn yet.
    pub drawn_layer: [Option<(i32, u16)>; 3],
    /// Rendering only: `varz.c:superpixels` (JE_doSP / JE_drawSP), see
    /// [`Superpixel`]. Outside the hash and the golden master.
    pub superpixels: [Superpixel; MAX_SUPERPIXELS],
    /// Rendering only, outside the hash: starfield (`backgrnd.c`).
    /// The C code fills it with the JE_main MT generator, before the golden
    /// master re-seeds: it therefore touches neither `mt` nor `h_mt` (divergence 13).
    pub starfield: [StarfieldStar; MAX_STARS],
    /// True if the current iteration drew the stars
    /// (`starActive || astralDuration > 0`).
    pub starfield_drawn: bool,
    /// Rendering only: `tyrian2.c:neat` (static uint16_t, never reset to
    /// zero), phase counter of `JE_darkenBackground` at detail level 6
    /// (SuperWild). Advances by 3 then by 1 at each iteration: value of the
    /// darkening before layer 2, then before layer 3.
    pub superwild_neat: u16,
    pub drawn_neat: (u16, u16),
    pub last_superpixel: usize,
    /// Rendering only: `invulnerable_ticks > 0` at ship drawing
    /// (player_movement.c, before the decrement): blending and aura.
    pub drawn_invulnerable: [bool; 2],
    /// Rendering only: special weapon indicator of varz.c:JE_doSpecialShot
    /// (blit_sprite2 of spriteSheet9 at (47, 4): 94 ready, 93 recharging),
    /// 0 if none drawn at this iteration.
    pub drawn_special_icon: u32,
    /// Rendering only: Zinglon beam of JE_doSpecialShot (two JE_barBright
    /// centered on player[0].x + 7), half-width `25 - |duration - 25|`,
    /// one per call of JE_doSpecialShot at this iteration (one per player).
    pub drawn_zinglon: Vec<(i32, i32)>,
    /// Rendering only: ms added to the low armor blinking clock.
    /// The C golden master under GOLDEN_ACH_CLOCK counts 1 ms per call
    /// outside game frames (menus, loading); the comparison tools
    /// take over this count (index.txt of the C code) to draw at the same instant.
    pub clock_offset_ms: u32,
    /// Rendering only: `sidekick[i].charge` at sidekick drawing
    /// (player_movement.c, "draw sidekicks" precedes the tick's recharge).
    pub drawn_sidekick_charge: [[u32; 2]; 2],
    /// Rendering only: texts and bars of tyrian2.c drawn into
    /// game_screen this tick (see [`DrawnTexts`]).
    pub drawn_texts: DrawnTexts,
    /// Rendering only: `chargeGr` / `chargeGrWait` (animation of the charge icon
    /// above player 2) and image drawn this tick (0 = none).
    pub charge_gr: u8,
    pub charge_gr_wait: u8,
    pub drawn_charge_sprite: u32,
    /// Rendering only, outside the hash: incremental dashboard drawings
    /// done by the C code in VGAScreenSeg during the simulation, in
    /// order (see [`HudOp`]). Flushed by the display at each tick.
    pub hud_ops: Vec<HudOp>,
    /// Rendering only: dashboard drawings done during the iteration
    /// head (`loop_top`: shield recharge, player collisions).
    /// `step` plays it at the end of the call whereas the C code only presents these drawings
    /// at the end of this iteration, i.e. on the next call: they join
    /// `hud_ops` at the start of `loop_bottom`.
    pub hud_ops_next: Vec<HudOp>,
    /// Rendering only: (tint, brightness) passed to `JE_filterScreen`
    /// at this iteration, None if `filterActive` was false.
    pub drawn_filter: Option<(i8, i8)>,
    /// Rendering only: `starShowVGASpecialCode` of the iteration
    /// (tyrian2.c, at the top of level_loop): 1 flipped image, 2 lamp.
    pub star_show_code: u8,
    /// Rendering only, outside the hash: explosions drawn by
    /// `JE_updateAndDrawExplosions` at this iteration (x, y, 1-based image),
    /// in array order. The state published after the tick is not the
    /// same list: it loses those whose ttl just dropped to 0 (drawn
    /// one last time) and gains those created further down the loop.
    pub drawn_explosions: Vec<(i16, i16, u16)>,
    /// Rendering only, outside the hash: enemies drawn by blit_enemy at
    /// this iteration (JE_drawEnemy), in C order. An enemy destroyed
    /// further down the loop stays in it (the C code already drew it); an enemy
    /// outside the horizontal window is not in it (the C code does not draw it).
    pub drawn_enemies: Vec<DrawnEnemy>,
    /// Rendering only, outside the hash: player shots drawn by
    /// player_shot_move_and_draw at this iteration, in C order. A shot
    /// that hits an enemy in the same pass stays in it (drawn before the
    /// collision, then removed from the array).
    pub drawn_player_shots: Vec<DrawnShot>,
    /// Rendering only, outside the hash: enemy shots drawn by
    /// JE_updateEnemyShots at this iteration (x, y, sgr + animate). Those that
    /// enemies fire further down the loop are not in it yet.
    pub drawn_enemy_shots: Vec<(i16, i16, u16)>,
    /// Rendering only, outside the hash: effect events (particles,
    /// damage numbers, vibration), see `fx::FxEvent`.
    pub fx_events: Vec<(u64, crate::fx::FxEvent)>,
}

/// An enemy drawing from `JE_drawEnemy` (rendering only).
#[derive(Debug, Clone, Copy)]
pub struct DrawnEnemy {
    /// Slot in `enemy` (0..100): gives the layer.
    pub slot: u8,
    pub ex: i16,
    pub ey: i16,
    /// `egr[enemycycle - 1]` (0 if enemycycle outside 1..=20).
    pub frame: u16,
    pub filter: u8,
    /// `tempMapXOfs` at drawing.
    pub mapoffset: u16,
    pub sheet: u8,
    /// C `size` (1 = 2×2).
    pub size: u8,
    /// Darkened trail (Wild): `trail_ex/trail_ey` if drawn.
    pub trail: Option<(i16, i16)>,
}

/// A drawing from `shots.c:player_shot_move_and_draw` (rendering only).
#[derive(Debug, Clone, Copy)]
pub struct DrawnShot {
    pub x: i16,
    pub y: i16,
    /// `shotGr + shotAni` before the modulo 1000.
    pub frame: u16,
    /// `tmp_shotXM` (shotXM right after `+= shotXC`): shadow condition.
    pub xm: i16,
}

/// Music (ADR 0010): outside `Sim::snapshot`, hence outside the hash.
#[derive(Debug, Clone)]
pub struct MusicState {
    /// `musmast.c:musicFade`.
    pub music_fade: bool,
    /// `nortsong.c:tempVolume`.
    pub temp_volume: u16,
    /// `tyrMusicVolume` (player setting; 191 by default, config.c / save_json.c:load_audio).
    pub tyr_music_volume: u16,
    /// Audio orders emitted during the ticks, drained by the client (`Sim::take_music_cues`).
    pub music_cues: Vec<MusicCue>,
    /// True if, at the last pass through the MUSIC FADE block, the C code evaluated the
    /// `else if (!playing && firstGameOver)` branch with `firstGameOver` true:
    /// the client then restarts the level song if its player has gone silent.
    pub music_level_song_branch: bool,
}

#[derive(Clone)]
pub struct World {
    pub tables: Tables,
    pub rng: Xoshiro256,
    pub mt: MersenneTwister,

    // --- episode and level ---
    pub episode: u8,
    /// `difficultyLevel` (int8_t).
    pub difficulty: i8,
    /// `initialDifficulty`.
    pub initial_difficulty: i8,
    pub assist: bool,
    /// Counters for the abandon logic of `autopilot.c:autopilot_drive` (statics `stalled`,
    /// `last_armor_sum`, `no_damage`), active with `assist`.
    pub autopilot_stalled: u32,
    pub autopilot_last_armor_sum: i64,
    pub autopilot_no_damage: u32,
    /// `config.c:processorType` (1 = Classic, 3 = High, 4 = Wild by default):
    /// detail level of the effects. Read by the simulation (draws of
    /// `hitstop.c:on_boss_defeated`, layer 2 of `tyrian2.c`) and by rendering.
    pub processor_type: u8,
    /// [`crate::LevelSetup::endless_armor_mult`].
    pub endless_armor_mult: Option<crate::fmath::F32>,

    // --- players ---
    pub players: [Player; 2],
    pub two_player_mode: bool,
    pub mode: crate::modes::ModeState,
    pub one_player_action: bool,
    /// Endless lives per player (`endless.c:s_lives_p`), `None` outside Endless.
    pub endless_lives: Option<[i32; 2]>,
    /// Title demo (`play_demo`, `mainint.c:replay_demo_keys`); `None` outside a demo.
    pub demo: Option<crate::demo::DemoInput>,
    /// Level marked "$" (`bonusLevelCurrent` / `normalBonusLevelCurrent`): the death
    /// of all players ends it without a GAME OVER screen (tyrian2.c:1481).
    pub bonus_level_current: bool,
    /// Endless score (`endless.c:s_score`), accumulated by [`World::endless_on_enemy_killed`].
    pub endless_score: u64,
    /// `combos.c:combo_multiplier`, read by `collision.c` (money) and
    /// `endless.c:on_enemy_killed`; held by `crate::combos` (1 as long as
    /// combos are off, as under golden master without `GOLDEN_COMBO_MS`).
    pub combo_multiplier: u32,
    /// `combos.c`: chain, timer, activation.
    pub combo: crate::combos::ComboState,
    pub ship_gr: u16,
    pub explosion_follow_amount_x: i16,
    pub explosion_follow_amount_y: i16,
    pub armor_ship_delay: u16,

    // --- special weapon (`varz.c`, static variables) ---
    pub shot_repeat: [u8; 11],

    // --- enemies ---
    pub enemy: Vec<Enemy>,
    /// 0 = active, 1 = free, 2 = collectible object (`enemyAvail`).
    pub enemy_avail: Vec<u8>,
    pub enemy_on_screen: u16,
    pub total_enemy: u16,
    pub enemy_killed: u16,

    // --- shots ---
    pub player_shots: Vec<PlayerShot>,
    pub shot_avail: Vec<u8>,
    pub enemy_shots: Vec<EnemyShot>,
    pub enemy_shot_avail: Vec<bool>,

    // --- explosions ---
    pub explosions: Vec<Explosion>,
    pub rep_explosions: Vec<RepExplosion>,
    pub enemy_still_exploding: bool,
    /// `hitstop.c`: freeze and cascade of secondary explosions (the cascade is
    /// filled by the BOSS_DEFEATED listener even when hitstop is disabled).
    pub hitstop: crate::fx::Hitstop,
    /// Player's Enhanced effects active in the simulation (all false under
    /// golden master, see `fx::FxConfig`).
    pub fx: crate::fx::FxConfig,
    /// Counters of the events read by stats.c/achievements.c (see `AchEvents`).
    pub ach_events: AchEvents,
    /// `ach_events` at the end of `loop_bottom`, where the C code calls
    /// `achievements_check` (tyrian2.c, end of the loop). `Sim::step` then
    /// plays the head of the next iteration, whose keys and damage are
    /// only seen by the C code at the next check.
    pub ach_events_at_check: AchEvents,
    /// `stats.c:level_stats` (end-of-level tally, mainint.c:JE_endLevelAni
    /// "Level Statistics Screen"): pure observer, outside the hash.
    pub level_stats: LevelStats,

    /// Number of `level_loop` iterations started.
    pub iteration: u64,

    // --- subsets by responsibility ---
    pub background: Background,
    pub script: EventScript,
    pub ending: LevelEnding,
    pub special_weapon: SpecialWeaponState,
    pub filter: LevelFilter,
    pub energy: Energy,
    pub sidekick_formation: SidekickFormation,
    pub controls: Controls,
    pub render: RenderOnly,
    pub music: MusicState,
    /// Development cheats (`dev_console.c`, `--devmode`); inactive
    /// by default.
    pub dev: crate::dev::DevState,
    /// Rendering only, outside the hash: sound effects queue
    /// (`varz.c:soundQueue`), see [`crate::sound`].
    pub sound: crate::sound::SoundQueue,
}

/// Default `tyrMusicVolume` (config.c:735, save_json.c:load_audio).
pub const DEFAULT_MUSIC_VOLUME: u16 = 191;

/// Audio order emitted by the level loop, in the order the C code runs it.
/// Never acts on the simulation: the Godot client plays them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicCue {
    /// `play_song(n)`, n 0-based (`loudness.c:play_song`). Replaying the current song does not restart it
    /// (loudness.c:play_song only reloads if `song_num != song_playing`).
    /// May be -1 (event 35 at 0): the C code tries a song outside the table
    /// (load_song refuses, log_warn) and keeps the previous one.
    PlaySong(i32),
    /// `set_volume(v, fxVolume)`: music volume 0..255.
    SetVolume(u16),
}

impl World {
    /// Adds an audio order; two identical consecutive orders not yet
    /// read count as one (play_song / set_volume are idempotent), which
    /// bounds the queue when nobody drains it (golden master tool).
    pub fn push_music_cue(&mut self, cue: MusicCue) {
        if self.music.music_cues.last() != Some(&cue) {
            self.music.music_cues.push(cue);
        }
    }

    /// "MUSIC FADE" block of `tyrian2.c:JE_main` (level_loop), right after the
    /// computation of `allPlayersGone`. The `else if (!playing && firstGameOver)
    /// play_song(levelSong - 1)` branch depends on the state of the audio player: it is
    /// played by the client (see `Sim::first_game_over`).
    pub fn music_fade_update(&mut self) {
        if self.music.music_fade {
            if self.music.temp_volume > 10 {
                self.music.temp_volume -= 1;
                self.push_music_cue(MusicCue::SetVolume(self.music.temp_volume));
            } else {
                self.music.music_fade = false;
            }
        }
        if !self.ending.all_players_gone && self.ending.level_end > 0 && self.ending.end_level {
            self.push_music_cue(MusicCue::PlaySong(9));
            self.music.music_fade = false;
            self.music.music_level_song_branch = false;
        } else {
            self.music.music_level_song_branch = self.ending.first_game_over;
        }
    }
}

/// Reads an event field `eventRec[eventLoc-1]` tolerating out-of-bounds indices
/// (the C array has EVENT_MAXIMUM entries, zero beyond the sentinel).
impl World {
    pub fn ev(&self, idx: usize) -> Event {
        self.script.event_rec.get(idx).copied().unwrap_or(Event {
            eventtime: 0,
            eventtype: 0,
            eventdat: 0,
            eventdat2: 0,
            eventdat3: 0,
            eventdat5: 0,
            eventdat6: 0,
            eventdat4: 0,
        })
    }

    pub fn cur_ev_index(&self) -> usize {
        (self.script.event_loc as usize).wrapping_sub(1).min(EVENT_MAXIMUM - 1)
    }

    pub fn cur_ev(&self) -> Event {
        self.ev(self.cur_ev_index())
    }

    pub fn cur_ev_mut(&mut self) -> &mut Event {
        let i = self.cur_ev_index();
        &mut self.script.event_rec[i]
    }

    pub fn all_players_dead(&self) -> bool {
        // player.h:all_players_dead
        !self.players[0].is_alive && (!self.two_player_mode || !self.players[1].is_alive)
    }

    /// player.h:all_players_alive: in two-player, both must be alive.
    pub fn all_players_alive(&self) -> bool {
        self.players[0].is_alive && (!self.two_player_mode || self.players[1].is_alive)
    }
}

#[cfg(test)]
mod background2_tests {
    use super::pt_background2;

    /// config.c:433: processorType 1 (Visual Effects Off) cuts layer 2
    /// and the shadows; tyrian2.c:1091 restores it for an "over 3" layer.
    #[test]
    fn off_preset_hides_layer_2_except_over_3() {
        assert!(!pt_background2(1, 0));
        assert!(!pt_background2(1, 1));
        assert!(!pt_background2(1, 2));
        assert!(pt_background2(1, 3));
        for pt in [2, 3, 4, 5, 6] {
            assert!(pt_background2(pt, 0));
        }
    }
}
