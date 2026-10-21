//! Destruct mini-game (`src/destruct.c`), ported for the rewrite.
//!
//! Two sides of eight unit types bombard each other in the manner of Scorched
//! Earth; the terrain is the framebuffer itself ("dirt" pixels of color
//! 25). This module reproduces the C logic tick by tick, with the same
//! generator (`rng.c`, [`Xoshiro256`]) and the same `float` computations in
//! exact software arithmetic ([`F32`], [`D64`], ADR 0003): with equal seed and
//! inputs, the frames are identical to those of the C code.
//!
//! Drawing goes through [`Canvas`]: `VGAScreen` (320×200 pixels) plus the two
//! blitters that the C code calls (`blit_sprite2` on `newsh~.shp`, `JE_outText`).
//! The C temporary screen (`destructTempScreen` = `game_screen`) lives here.
//!
//! The flow of a tick (`destruct.c:DE_RunTick`) is split into two calls
//! so that the host presents the frame (`JE_showVGA`) at the same place as the C code:
//! [`Destruct::tick_draw`] (up to the HUD), then [`Destruct::tick_input`]
//! (inputs, end of round). The host's own keys (F10, F11, P, F1,
//! Escape, Backspace) stay on the Godot side.

use crate::f32ops::{D64, F32};
use crate::rng::Xoshiro256;

pub const MAX_PLAYERS: usize = 2;
pub const PLAYER_LEFT: usize = 0;
pub const PLAYER_RIGHT: usize = 1;
/// `DESTRUCT_MODES` (modes from the data) and `MAX_MODES` (+ "Custom").
pub const DESTRUCT_MODES: usize = 5;
pub const MAX_MODES: usize = 6;
pub const MODE_CUSTOM: usize = 5;

pub const UNIT_TANK: i32 = 0;
pub const UNIT_NUKE: i32 = 1;
pub const UNIT_DIRT: i32 = 2;
pub const UNIT_SATELLITE: i32 = 3;
pub const UNIT_MAGNET: i32 = 4;
pub const UNIT_LASER: i32 = 5;
pub const UNIT_JUMPER: i32 = 6;
pub const UNIT_HELI: i32 = 7;
pub const MAX_UNITS: usize = 8;

pub const SHOT_TRACER: i32 = 0;
pub const SHOT_SMALL: i32 = 1;
pub const SHOT_LARGE: i32 = 2;
pub const SHOT_MICRO: i32 = 3;
pub const SHOT_DEMO: i32 = 5;
pub const SHOT_SMALLNUKE: i32 = 6;
pub const SHOT_SMALLDIRT: i32 = 8;
pub const SHOT_MAGNET: i32 = 10;
pub const SHOT_MINILASER: i32 = 11;
pub const SHOT_MEGALASER: i32 = 12;
pub const SHOT_LASERTRACER: i32 = 13;
pub const SHOT_MINI: i32 = 15;
pub const SHOT_LAST: i32 = 16;
pub const SHOT_INVALID: i32 = -1;
pub const MAX_SHOT_TYPES: usize = 17;

const EXPL_NONE: i32 = 0;
const EXPL_MAGNET: i32 = 1;
const EXPL_DIRT: i32 = 2;
const EXPL_NORMAL: i32 = 3;

const TRAILS_NONE: i32 = 0;
const TRAILS_NORMAL: i32 = 1;
const TRAILS_FULL: i32 = 2;

pub const PIXEL_BLACK: u8 = 0;
pub const PIXEL_DIRT: u8 = 25;

pub const MAP_WALLS: u32 = 0x01;
pub const MAP_RINGS: u32 = 0x02;
pub const MAP_HOLES: u32 = 0x04;
pub const MAP_FUZZY: u32 = 0x08;
pub const MAP_TALL: u32 = 0x10;

/// `de_move_t` / `de_keys_t` (same indices).
pub const MOVE_LEFT: usize = 0;
pub const MOVE_RIGHT: usize = 1;
pub const MOVE_UP: usize = 2;
pub const MOVE_DOWN: usize = 3;
pub const MOVE_CHANGE: usize = 4;
pub const MOVE_FIRE: usize = 5;
pub const MOVE_CYUP: usize = 6;
pub const MOVE_CYDN: usize = 7;
pub const MAX_MOVE: usize = 8;

const W: usize = 320;
const H: usize = 200;
const SCREEN: usize = W * H;

/// Samples of `sndmast.h` used here (1-based numbers).
pub const V_CLEARED_PLATFORM: u8 = 32;

// --- Tables de destruct.c ----------------------------------------------------

const DEMOLISH: [bool; MAX_SHOT_TYPES] =
    [false, false, false, false, false, true, true, true, false, false, false, false, true, false, true, false, true];
const SHOT_TRAIL: [i32; MAX_SHOT_TYPES] = [
    TRAILS_NONE,
    TRAILS_NONE,
    TRAILS_NONE,
    TRAILS_NORMAL,
    TRAILS_NORMAL,
    TRAILS_NORMAL,
    TRAILS_FULL,
    TRAILS_FULL,
    TRAILS_NONE,
    TRAILS_NONE,
    TRAILS_NONE,
    TRAILS_NORMAL,
    TRAILS_FULL,
    TRAILS_NORMAL,
    TRAILS_FULL,
    TRAILS_NORMAL,
    TRAILS_NONE,
];
const SHOT_DELAY: [u32; MAX_SHOT_TYPES] = [10, 30, 80, 20, 60, 100, 140, 200, 20, 60, 5, 15, 50, 5, 80, 16, 0];
/// S_SELECT, S_WEAPON_2, S_WEAPON_1, S_WEAPON_7 … (sndmast.h).
const SHOT_SOUND: [u8; MAX_SHOT_TYPES] = [8, 2, 1, 7, 7, 9, 22, 22, 5, 13, 10, 15, 15, 26, 14, 7, 7];
const EXPLO_SIZE: [u32; MAX_SHOT_TYPES] = [4, 20, 30, 14, 22, 16, 40, 60, 10, 30, 0, 5, 10, 3, 15, 7, 0];
const SHOT_BOUNCE: [bool; MAX_SHOT_TYPES] =
    [false, false, false, false, false, false, false, false, false, false, false, true, true, true, true, false, true];
const EXPLO_DENSITY: [u32; MAX_SHOT_TYPES] = [2, 5, 10, 15, 20, 15, 25, 30, 40, 80, 0, 30, 30, 4, 30, 5, 0];
const SHOT_DIRT: [i32; MAX_SHOT_TYPES] = [
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_DIRT,
    EXPL_DIRT,
    EXPL_MAGNET,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NORMAL,
    EXPL_NONE,
];
const SHOT_COLOR: [u32; MAX_SHOT_TYPES] = [16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 10, 10, 10, 10, 16, 0];

const DEFAULT_WEAPON: [i32; MAX_UNITS] =
    [SHOT_SMALL, SHOT_MICRO, SHOT_SMALLDIRT, SHOT_INVALID, SHOT_MAGNET, SHOT_MINILASER, SHOT_MICRO, SHOT_MINI];
const DEFAULT_CPU_WEAPON: [i32; MAX_UNITS] =
    [SHOT_SMALL, SHOT_MICRO, SHOT_DEMO, SHOT_INVALID, SHOT_MAGNET, SHOT_MINILASER, SHOT_MICRO, SHOT_MINI];
const DEFAULT_CPU_WEAPON_B: [i32; MAX_UNITS] =
    [SHOT_DEMO, SHOT_SMALLNUKE, SHOT_DEMO, SHOT_INVALID, SHOT_MAGNET, SHOT_MEGALASER, SHOT_MICRO, SHOT_MINI];
const SYSTEM_ANGLE: [bool; MAX_UNITS] = [true, true, true, false, false, true, false, false];
const BASE_DAMAGE: [i32; MAX_UNITS] = [200, 120, 400, 300, 80, 150, 600, 40];
const SYSTEM_ANI: [bool; MAX_UNITS] = [false, false, false, true, false, false, false, true];

const WEAPON_SYSTEMS: [[bool; MAX_SHOT_TYPES]; MAX_UNITS] = {
    const T: bool = true;
    const F: bool = false;
    [
        [T, T, T, F, F, T, F, F, F, F, F, F, F, F, F, F, F], // normal
        [F, F, F, T, T, T, T, T, F, F, F, F, F, F, F, F, F], // nuke
        [F, F, F, F, F, T, F, F, T, T, F, F, F, F, F, F, F], // dirt
        [F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F, F], // worthless
        [F, F, F, F, F, F, F, F, F, F, T, F, F, F, F, F, F], // magnet
        [F, F, F, F, F, F, F, F, F, F, F, T, T, F, F, F, F], // laser
        [T, F, F, T, F, F, F, F, F, F, F, F, F, F, T, F, F], // jumper
        [T, F, F, F, F, T, F, F, F, F, F, F, F, F, F, T, F], // helicopter
    ]
};

/// `goodsel`: songs played by Destruct (1-based, `play_song(n - 1)`).
const GOODSEL: [i32; 14] = [1, 2, 6, 12, 13, 14, 17, 23, 24, 26, 28, 29, 32, 33];

/// `basetypes`: [0] = number of units, then ten randomly drawn types.
const BASETYPES: [[u8; 11]; 10] = {
    const TA: u8 = UNIT_TANK as u8;
    const NU: u8 = UNIT_NUKE as u8;
    const DI: u8 = UNIT_DIRT as u8;
    const SA: u8 = UNIT_SATELLITE as u8;
    const MA: u8 = UNIT_MAGNET as u8;
    const LA: u8 = UNIT_LASER as u8;
    const JU: u8 = UNIT_JUMPER as u8;
    const HE: u8 = UNIT_HELI as u8;
    [
        [5, TA, TA, NU, DI, DI, SA, MA, LA, JU, HE], // Normal
        [1, TA, TA, TA, TA, TA, TA, TA, TA, TA, TA], // Traditional
        [4, HE, HE, HE, HE, HE, HE, HE, HE, HE, HE], // Weak heli attack fleet
        [8, TA, TA, TA, NU, NU, NU, DI, MA, LA, JU], // Strong heli defense fleet
        [8, HE, HE, HE, HE, HE, HE, HE, HE, HE, HE], // Strong heli attack fleet
        [4, TA, TA, TA, TA, NU, NU, DI, MA, JU, JU], // Weak heli defense fleet
        [8, TA, NU, DI, SA, MA, LA, JU, HE, TA, NU], // Overpowering fleet
        [4, TA, TA, NU, DI, TA, LA, JU, HE, NU, JU], // Weak fleet
        [5, TA, TA, NU, DI, DI, SA, MA, LA, JU, HE], // Left custom
        [5, TA, TA, NU, DI, DI, SA, MA, LA, JU, HE], // Right custom
    ]
};
const BASE_LOOKUP: [[usize; MAX_MODES]; MAX_PLAYERS] = [[0, 1, 3, 4, 6, 8], [0, 1, 2, 5, 7, 9]];
const GRAPHIC_BASE: [[u32; MAX_UNITS]; MAX_PLAYERS] =
    [[1, 6, 11, 58, 63, 68, 96, 153], [20, 25, 30, 77, 82, 87, 115, 172]];
const MODE_SCORE: [[u32; MAX_MODES]; MAX_PLAYERS] = [[1, 0, 0, 5, 0, 1], [1, 0, 5, 0, 1, 1]];

pub const UNIT_NAMES: [&str; MAX_UNITS] = ["tank", "nuke", "dirt", "satellite", "magnet", "laser", "jumper", "heli"];

// --- Float constants (IEEE-754 bits, checked by test) ---------------------

const F_0_0001: F32 = F32::from_bits(0x38D1_B717);
const F_0_01: F32 = F32::from_bits(0x3C23_D70A);
const F_0_02: F32 = F32::from_bits(0x3CA3_D70A);
const F_0_03: F32 = F32::from_bits(0x3CF5_C28F);
const F_0_05: F32 = F32::from_bits(0x3D4C_CCCD);
const F_0_1: F32 = F32::from_bits(0x3DCC_CCCD);
const F_0_2: F32 = F32::from_bits(0x3E4C_CCCD);
const F_0_3: F32 = F32::from_bits(0x3E99_999A);
const F_0_5: F32 = F32::from_bits(0x3F00_0000);
const F_0_8: F32 = F32::from_bits(0x3F4C_CCCD);
const F_0_9: F32 = F32::from_bits(0x3F66_6666);
const F_1_5: F32 = F32::from_bits(0x3FC0_0000);
const F_9_99: F32 = F32::from_bits(0x411F_D70A);

fn fi(v: i64) -> F32 {
    F32::from_int(v)
}

fn fu(v: u32) -> F32 {
    F32::from_u32(v)
}

/// C `float` -> `unsigned int` conversion on x86-64 (64-bit cvttss2si
/// then low 32 bits): a negative value wraps around modulo 2^32.
fn to_u32(f: F32) -> u32 {
    f.trunc() as u32
}

/// `float` -> `int` (truncation).
fn to_i32(f: F32) -> i32 {
    f.trunc() as i32
}

// --- Structures ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unit {
    pub unit_x: u32,
    pub unit_y: F32,
    pub unit_y_mov: F32,
    pub is_y_in_air: bool,
    pub unit_type: i32,
    pub shot_type: i32,
    pub angle: F32,
    pub power: F32,
    pub last_move: i32,
    pub ani_frame: u32,
    pub health: i32,
}

impl Unit {
    /// `DE_isValidUnit`.
    pub fn is_valid(&self) -> bool {
        self.health > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shot {
    pub is_available: bool,
    pub x: F32,
    pub y: F32,
    pub xmov: F32,
    pub ymov: F32,
    pub shottype: u32,
    trailx: [u32; 4],
    traily: [u32; 4],
    trailc: [u32; 4],
}

impl Default for Shot {
    fn default() -> Self {
        Shot {
            is_available: true,
            x: F32::ZERO,
            y: F32::ZERO,
            xmov: F32::ZERO,
            ymov: F32::ZERO,
            shottype: 0,
            trailx: [0; 4],
            traily: [0; 4],
            trailc: [0; 4],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Explosion {
    pub is_available: bool,
    pub x: u32,
    pub y: u32,
    pub explowidth: u32,
    pub explomax: u32,
    pub explofill: u32,
    pub explo_type: i32,
}

impl Default for Explosion {
    fn default() -> Self {
        Explosion { is_available: true, x: 0, y: 0, explowidth: 0, explomax: 0, explofill: 0, explo_type: EXPL_NONE }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Ai {
    c_angle: i32,
    c_power: i32,
    c_fire: i32,
    c_no_down: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub is_cpu: bool,
    ai: Ai,
    pub units: Vec<Unit>,
    pub moves: [bool; MAX_MOVE],
    pub units_remaining: u32,
    pub unit_selected: u32,
    pub shot_delay: u32,
    pub score: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wall {
    pub exist: bool,
    pub x: u32,
    pub y: u32,
}

/// `struct destruct_config_s` and the tables that `load_destruct_config` modifies.
#[derive(Clone, Debug)]
pub struct DestructConfig {
    pub max_shots: u32,
    pub min_walls: u32,
    pub max_walls: u32,
    pub max_explosions: u32,
    pub max_installations: u32,
    pub allow_custom: bool,
    pub alwaysalias: bool,
    pub jumper_straight: [bool; 2],
    pub ai: [bool; 2],
    /// `weaponSystems[UNIT_LASER][SHOT_LASERTRACER]` ("tracer laser").
    pub tracer_laser: bool,
    /// `basetypes[8 + p]`: squads of Custom mode.
    pub custom_units: [[u8; 11]; 2],
    /// `defaultKeyConfig`: codes SDL (0 = SDL_SCANCODE_UNKNOWN).
    pub keys: [[[u16; 4]; MAX_MOVE]; MAX_PLAYERS],
}

impl Default for DestructConfig {
    fn default() -> Self {
        DestructConfig {
            max_shots: 40,
            min_walls: 20,
            max_walls: 20,
            max_explosions: 40,
            max_installations: 10,
            allow_custom: false,
            alwaysalias: false,
            jumper_straight: [true, false],
            ai: [true, false],
            tracer_laser: false,
            custom_units: [BASETYPES[8], BASETYPES[9]],
            keys: crate::destruct_config::DEFAULT_KEYS,
        }
    }
}

/// `VGAScreen` and the two C drawing routines that Destruct calls on it.
pub trait Canvas {
    /// The 320×200 indices of `VGAScreen`.
    fn pixels(&mut self) -> &mut [u8];
    /// `blit_sprite2(VGAScreen, x, y, destructSpriteSheet, index)`.
    fn blit_sprite2(&mut self, x: i32, y: i32, index: u32);
    /// `JE_outText(VGAScreen, x, y, text, colorbank, brightness)` (CP437).
    fn out_text(&mut self, x: i32, y: i32, text: &[u8], colorbank: u8, brightness: i32);
}

/// Result of [`Destruct::tick_input`] (`de_state_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickEnd {
    Continue,
    /// `STATE_RELOAD`: end of round, new map.
    Reload,
}

/// Queued sound (`soundQueue[channel]`, `soundQueueX`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueuedSound {
    pub channel: u8,
    pub sample: u8,
    pub x: u16,
}

/// Full state of a Destruct session (`JE_destructGame`).
pub struct Destruct {
    pub config: DestructConfig,
    basetypes: [[u8; 11]; 10],
    weapon_systems: [[bool; MAX_SHOT_TYPES]; MAX_UNITS],
    pub players: [Player; MAX_PLAYERS],
    /// `world.baseMap[320]` + slot 320 read out of bounds by
    /// `JE_placementPosition` (see [`BASE_MAP_ALIAS`]).
    base_map: [u32; 321],
    pub walls: Vec<Wall>,
    pub map_flags: u32,
    pub mode: usize,
    pub shots: Vec<Shot>,
    pub explosions: Vec<Explosion>,
    /// `destructTempScreen` (= `game_screen`).
    pub temp: Vec<u8>,
    pub rng: Xoshiro256,
    end_delay: u32,
    first_time: bool,
    sound_queue: [u8; 8],
    sound_queue_x: [u16; 8],
    explo_sound_channel: i32,
    /// Song chosen by the last `JE_generateTerrain` (`play_song`).
    pub song: i32,
    weapon_names: Vec<Vec<u8>>,
    hud_dmg: Vec<u8>,
    hud_pts: Vec<u8>,
}

/// `world.baseMap[320]` does not exist: `JE_placementPosition(wallX, 12)` with
/// wallX = 309 reads and writes slot 320, i.e. the low 32 bits of the pointer
/// `world.VGAScreen` that follows the array (undefined behavior in C). On
/// x86-64 this value always exceeds 200: the wall placed there and the terrain columns
/// wallX..318 that it "flattens" at this height disappear
/// (`JE_rectangle` rejects an off-screen rectangle). Any value ≥ 2^31
/// reproduces this result; we keep half of the range.
pub const BASE_MAP_ALIAS: u32 = 0x8000_0000;

impl Destruct {
    /// `JE_destructGame`: arrays allocated to the size of the configuration.
    /// Ungenerated units are zero (fresh malloc memory).
    pub fn new(config: DestructConfig, seed: u64) -> Destruct {
        let mut basetypes = BASETYPES;
        basetypes[8] = config.custom_units[0];
        basetypes[9] = config.custom_units[1];
        let mut weapon_systems = WEAPON_SYSTEMS;
        weapon_systems[UNIT_LASER as usize][SHOT_LASERTRACER as usize] = config.tracer_laser;
        let mut config = config;
        // "Malloc enough structures to cover all of this session's possible needs."
        for b in &basetypes {
            config.max_installations = config.max_installations.max(b[0] as u32);
        }
        let n = config.max_installations as usize;
        let player = Player { units: vec![Unit::default(); n], ..Default::default() };
        let mut base_map = [0u32; 321];
        base_map[320] = BASE_MAP_ALIAS;
        Destruct {
            walls: vec![Wall::default(); config.max_walls as usize],
            shots: vec![Shot::default(); config.max_shots as usize],
            explosions: vec![Explosion::default(); config.max_explosions as usize],
            config,
            basetypes,
            weapon_systems,
            players: [player.clone(), player],
            base_map,
            map_flags: 0,
            mode: 0,
            temp: vec![0; SCREEN],
            rng: Xoshiro256::new(seed),
            end_delay: 0,
            first_time: false,
            sound_queue: [0; 8],
            sound_queue_x: [160; 8],
            explo_sound_channel: 0,
            song: 0,
            weapon_names: (0..MAX_SHOT_TYPES).map(|_| Vec::new()).collect(),
            hud_dmg: b"dmg".to_vec(),
            hud_pts: b"pts".to_vec(),
        }
    }

    /// HUD texts (CP437): `weaponNames[]`, `destruct_hud_dmg`, `destruct_hud_pts`.
    pub fn set_texts(&mut self, weapon_names: Vec<Vec<u8>>, dmg: Vec<u8>, pts: Vec<u8>) {
        self.weapon_names = weapon_names;
        self.hud_dmg = dmg;
        self.hud_pts = pts;
    }

    fn rng_float(&mut self) -> F32 {
        F32::from_unit24(self.rng.next_float_bits())
    }

    /// `DE_ResetPlayers` then `is_cpu = config.ai` (`JE_destructMain`).
    pub fn reset_players(&mut self) {
        for (i, p) in self.players.iter_mut().enumerate() {
            p.is_cpu = false;
            p.unit_selected = 0;
            p.shot_delay = 0;
            p.score = 0;
            p.ai = Ai::default();
            let _ = i;
        }
        self.players[PLAYER_LEFT].is_cpu = self.config.ai[PLAYER_LEFT];
        self.players[PLAYER_RIGHT].is_cpu = self.config.ai[PLAYER_RIGHT];
    }

    /// `do … while (curState == STATE_RELOAD)` loop of `JE_destructMain`:
    /// `destructFirstTime = true`, `DE_ResetUnits`, `DE_ResetLevel`. The caller
    /// has set `JE_loadPic(VGAScreen, 11)`; it presents the frame afterwards
    /// (`JE_showVGA` of `JE_generateTerrain`).
    pub fn start_round(&mut self, c: &mut dyn Canvas, mode: usize) {
        self.mode = mode.min(MAX_MODES - 1);
        self.first_time = true;
        self.reset_units();
        self.reset_weapons();
        self.generate_terrain(c);
        self.reset_ai();
    }

    fn reset_units(&mut self) {
        for p in &mut self.players {
            for u in &mut p.units {
                u.health = 0;
            }
            p.unit_selected = 0;
        }
    }

    fn reset_weapons(&mut self) {
        for s in &mut self.shots {
            s.is_available = true;
        }
        for e in &mut self.explosions {
            e.is_available = true;
        }
    }

    fn reset_ai(&mut self) {
        let walls = self.map_flags & MAP_WALLS != 0;
        for p in &mut self.players {
            if !p.is_cpu {
                continue;
            }
            for u in &mut p.units {
                if !u.is_valid() {
                    continue;
                }
                let t = u.unit_type as usize;
                u.angle =
                    if SYSTEM_ANGLE[t] || u.unit_type == UNIT_HELI { D64::quarter_pi().to_f32() } else { F32::ZERO };
                u.power = if u.unit_type == UNIT_LASER { fi(6) } else { fi(4) };
                u.shot_type = if walls { DEFAULT_CPU_WEAPON_B[t] } else { DEFAULT_CPU_WEAPON[t] };
            }
        }
    }

    // --- Terrain generation -------------------------------------------------

    /// `JE_generateTerrain`.
    fn generate_terrain(&mut self, c: &mut dyn Canvas) {
        self.map_flags = 0;
        if self.rng.next_u32().is_multiple_of(2) {
            self.map_flags |= MAP_WALLS;
        }
        if self.rng.next_u32().is_multiple_of(4) {
            self.map_flags |= MAP_HOLES;
        }
        match self.rng.next_u32() % 4 {
            0 => self.map_flags |= MAP_FUZZY,
            1 => self.map_flags |= MAP_TALL,
            2 => self.map_flags |= MAP_RINGS,
            _ => {}
        }
        self.song = GOODSEL[(self.rng.next_u32() % 14) as usize] - 1;

        self.generate_base_terrain();
        self.generate_units();
        self.generate_walls();
        self.draw_base_terrain(c.pixels());
        if self.map_flags & MAP_RINGS != 0 {
            self.generate_rings(c.pixels(), PIXEL_DIRT);
        }
        if self.map_flags & MAP_HOLES != 0 {
            self.generate_rings(c.pixels(), PIXEL_BLACK);
        }
        alias_dirt(c.pixels());
        self.temp.copy_from_slice(&c.pixels()[..SCREEN]);
    }

    /// `DE_generateBaseTerrain`.
    fn generate_base_terrain(&mut self) {
        let wave = |rng: &mut Self| -> F32 {
            let f = rng.rng_float();
            D64::from_f32(f).mul(D64::pi()).div(D64::from_int(50)).add(D64::from_f32(F_0_01)).to_f32()
        };
        let mut sinewave = wave(self);
        let mut sinewave2 = wave(self);
        let cosinewave = wave(self);
        let cosinewave2 = wave(self);
        let mut height_mul: u32 = 20;
        if self.map_flags & MAP_FUZZY != 0 {
            let f = self.rng_float();
            sinewave = D64::pi().sub(D64::from_f32(f.mul(F_0_3))).to_f32();
            let f = self.rng_float();
            sinewave2 = D64::pi().sub(D64::from_f32(f.mul(F_0_3))).to_f32();
        }
        if self.map_flags & MAP_TALL != 0 {
            height_mul = 100;
        }
        for i in 1..=318u32 {
            let fi_ = fu(i);
            let sum = sinewave
                .mul(fi_)
                .sin()
                .mul(fu(height_mul))
                .add(sinewave2.mul(fi_).sin().mul(fi(15)))
                .add(cosinewave.mul(fi_).cos().mul(fi(10)))
                .add(cosinewave2.mul(fi_).sin().mul(fi(15)));
            let mut newheight = to_u32(fi(sum.roundf()).add(fi(130)));
            newheight = newheight.clamp(40, 195);
            self.base_map[i as usize] = newheight;
        }
    }

    /// `DE_drawBaseTerrain`.
    fn draw_base_terrain(&self, vga: &mut [u8]) {
        for i in 1..=318i32 {
            rectangle(vga, i, self.base_map[i as usize] as i32, i, 199, PIXEL_DIRT);
        }
    }

    /// `JE_placementPosition`.
    fn placement_position(&mut self, passed_x: u32, width: u32) -> u32 {
        let mut new_y = 0;
        let last = passed_x.wrapping_add(width).wrapping_sub(1);
        let mut i = passed_x;
        while i <= last {
            new_y = new_y.max(self.base_map[(i as usize).min(320)]);
            i += 1;
        }
        let mut i = passed_x;
        while i <= last {
            self.base_map[(i as usize).min(320)] = new_y;
            i += 1;
        }
        new_y
    }

    /// `DE_generateUnits`.
    fn generate_units(&mut self) {
        for (i, lookup) in BASE_LOOKUP.iter().enumerate().take(MAX_PLAYERS) {
            let mut num_satellites = 0;
            self.players[i].units_remaining = 0;
            let fleet = self.basetypes[lookup[self.mode]];
            for j in 0..fleet[0] as usize {
                let unit_x = if i == PLAYER_LEFT {
                    (self.rng.next_u32() % 120) + 10
                } else {
                    320 - ((self.rng.next_u32() % 120) + 22)
                };
                let y = self.placement_position(unit_x - 1, 14);
                let unit_type = fleet[((self.rng.next_u32() % 10) + 1) as usize] as i32;
                let mut unit_y = fu(y);
                if unit_type == UNIT_SATELLITE {
                    unit_y = fu(30 + (self.rng.next_u32() % 40));
                    num_satellites += 1;
                } else {
                    self.players[i].units_remaining += 1;
                }
                let t = unit_type as usize;
                self.players[i].units[j] = Unit {
                    unit_x,
                    unit_y,
                    unit_y_mov: F32::ZERO,
                    is_y_in_air: false,
                    unit_type,
                    shot_type: DEFAULT_WEAPON[t],
                    angle: F32::ZERO,
                    power: if unit_type == UNIT_LASER { fi(6) } else { fi(3) },
                    last_move: 0,
                    ani_frame: 0,
                    health: BASE_DAMAGE[t],
                };
            }
            // Satellite team only: the first one becomes a tank.
            if self.players[i].units_remaining == 0 && num_satellites > 0 {
                for j in 0..fleet[0] as usize {
                    if self.players[i].units[j].unit_type != UNIT_SATELLITE {
                        continue;
                    }
                    let x = self.players[i].units[j].unit_x;
                    let y = self.placement_position(x - 1, 14);
                    let u = &mut self.players[i].units[j];
                    u.unit_type = UNIT_TANK;
                    u.unit_y = fu(y);
                    u.power = fi(3);
                    u.shot_type = DEFAULT_WEAPON[UNIT_TANK as usize];
                    u.health = BASE_DAMAGE[UNIT_TANK as usize];
                    self.players[i].units_remaining += 1;
                    break;
                }
            }
        }
    }

    /// `DE_generateWalls`.
    fn generate_walls(&mut self) {
        for w in &mut self.walls {
            w.exist = false;
        }
        if self.map_flags & MAP_WALLS == 0 {
            return;
        }
        let mut remain = self.rng.range(self.config.min_walls as i32, self.config.max_walls as i32) as u32;
        if remain == 0 {
            return;
        }
        loop {
            let mut wall_height = (self.rng.next_u32() % 5) + 1;
            if wall_height > remain {
                wall_height = remain;
            }
            let mut tries = 0;
            let mut wall_x;
            loop {
                let mut good = true;
                wall_x = (self.rng.next_u32() % 300) + 10;
                'outer: for p in &self.players {
                    for u in &p.units[..self.config.max_installations as usize] {
                        if wall_x > u.unit_x.wrapping_sub(12) && wall_x < u.unit_x.wrapping_add(13) {
                            good = false;
                            break 'outer;
                        }
                    }
                }
                tries += 1;
                if good || tries >= 5 {
                    break;
                }
            }
            for i in 1..=wall_height {
                let y = self.placement_position(wall_x, 12).wrapping_sub(14 * i);
                let w = &mut self.walls[(remain - i) as usize];
                w.exist = true;
                w.x = wall_x;
                w.y = y;
            }
            remain -= wall_height;
            if remain == 0 {
                break;
            }
        }
    }

    /// `DE_generateRings`.
    fn generate_rings(&mut self, vga: &mut [u8], pixel: u8) {
        let rings = self.rng.next_u32() % 6 + 1;
        for _ in 1..=rings {
            let x1 = (self.rng.next_u32() % 320) as i32;
            let y1 = ((self.rng.next_u32() % 160) + 20) as i32;
            let size = (self.rng.next_u32() % 40) + 10;
            for _ in 1..=size * size * 2 {
                let f = self.rng_float();
                let radian = D64::from_f32(f).mul(D64::two_pi()).to_f32();
                let f = self.rng_float();
                let r = radian.cos().mul(f.mul(F_0_1).add(F_0_9)).mul(fu(size)).roundf();
                let y2 = to_i32(fi(y1 as i64).add(fi(r)));
                let f = self.rng_float();
                let r = radian.sin().mul(f.mul(F_0_1).add(F_0_9)).mul(fu(size)).roundf();
                let x2 = to_i32(fi(x1 as i64).add(fi(r)));
                if y2 > 12 && y2 < 200 && x2 > 0 && x2 < 319 {
                    vga[(x2 + y2 * W as i32) as usize] = pixel;
                }
            }
        }
    }

    // --- Tick -------------------------------------------------------------------

    /// First half of `DE_RunTick`, up to and including the HUD: the caller
    /// then presents the frame (`JE_showVGA`).
    pub fn tick_draw(&mut self, c: &mut dyn Canvas) {
        self.sound_queue = [0; 8];
        self.temp_screen_checking(c.pixels());
        for p in &mut self.players {
            p.moves = [false; MAX_MOVE];
        }
        self.cycle_dead_units();
        self.run_gravity(c);
        self.animate();
        for w in &self.walls {
            if w.exist {
                c.blit_sprite2(w.x as i32, w.y as i32, 42);
            }
        }
        self.run_explosions();
        self.run_shots(c.pixels());
        self.run_ai();
        self.draw_crosshairs(c.pixels());
        self.draw_hud(c);
    }

    /// True only once after the first frame of a round
    /// (`destructFirstTime`: the caller then plays `fade_palette(colors, 25)`).
    pub fn take_first_time(&mut self) -> bool {
        if self.first_time {
            self.first_time = false;
            self.end_delay = 0;
            return true;
        }
        false
    }

    /// Second half of `DE_RunTick`: `DE_RunTickGetInput` (`keys[p][k]` =
    /// a key for action k of player p is held down), `DE_ProcessInput`
    /// and the end-of-round countdown.
    pub fn tick_input(&mut self, keys: [[bool; MAX_MOVE]; MAX_PLAYERS]) -> TickEnd {
        for (p, k) in self.players.iter_mut().zip(keys.iter()) {
            for (m, &pressed) in p.moves.iter_mut().zip(k.iter()) {
                if pressed {
                    *m = true;
                }
            }
        }
        self.process_input();
        if self.end_delay > 0 {
            self.end_delay -= 1;
            if self.end_delay == 0 {
                return TickEnd::Reload;
            }
        } else if self.check_endgame() {
            self.end_delay = 80;
        }
        TickEnd::Continue
    }

    /// `DE_RunTickPlaySounds`: sounds to play (channel 7 at full volume, the
    /// others at half volume), queue flushed.
    pub fn take_sounds(&mut self) -> Vec<QueuedSound> {
        let mut out = Vec::new();
        for i in 0..8 {
            if self.sound_queue[i] != 0 {
                out.push(QueuedSound { channel: i as u8, sample: self.sound_queue[i], x: self.sound_queue_x[i] });
                self.sound_queue[i] = 0;
                self.sound_queue_x[i] = 160;
            }
        }
        out
    }

    /// `JE_tempScreenChecking`: explosion fade, optional antialiasing,
    /// copy of lines 12..199 to `VGAScreen`.
    fn temp_screen_checking(&mut self, vga: &mut [u8]) {
        let alias = self.config.alwaysalias;
        let t = &mut self.temp;
        for y in 12..H {
            for x in 0..W {
                let k = y * W + x;
                if t[k] >= 241 {
                    if t[k] == 241 {
                        t[k] = PIXEL_BLACK;
                    } else {
                        t[k] -= 1;
                    }
                }
                if alias && t[k] == PIXEL_BLACK {
                    t[k] = alias_dirt_pixel(t, x, y);
                }
                vga[k] = t[k];
            }
        }
    }

    /// `DE_RunTickCycleDeadUnits`.
    fn cycle_dead_units(&mut self) {
        let n = self.config.max_installations;
        for p in &mut self.players {
            if p.units_remaining == 0 {
                continue;
            }
            loop {
                let u = &p.units[p.unit_selected as usize];
                if u.is_valid() && u.shot_type != SHOT_INVALID {
                    break;
                }
                p.unit_selected += 1;
                if p.unit_selected >= n {
                    p.unit_selected = 0;
                }
            }
        }
    }

    fn stability_check(&self, x: u32, y: u32) -> bool {
        stability_check(&self.temp, x, y)
    }

    /// `DE_RunTickGravity` (+ `DE_GravityDrawUnit`).
    fn run_gravity(&mut self, c: &mut dyn Canvas) {
        for i in 0..MAX_PLAYERS {
            for j in 0..self.players[i].units.len() {
                let mut u = self.players[i].units[j];
                if !u.is_valid() {
                    continue;
                }
                match u.unit_type {
                    UNIT_SATELLITE => {}
                    UNIT_HELI | UNIT_JUMPER if u.is_y_in_air => self.gravity_fly_unit(&mut u),
                    _ => self.gravity_lower_unit(&mut u),
                }
                self.players[i].units[j] = u;
                draw_unit(c, i, &u);
            }
        }
    }

    /// `DE_GravityLowerUnit`.
    fn gravity_lower_unit(&self, u: &mut Unit) {
        if u.unit_y.lt(fi(199)) && self.stability_check(u.unit_x, to_u32(fi(u.unit_y.roundf()))) {
            if u.unit_type == UNIT_HELI {
                u.unit_y_mov = F_1_5;
                u.unit_y = u.unit_y.add(F_0_2);
            } else {
                u.unit_y = u.unit_y.add(fi(1));
            }
            if u.unit_y.gt(fi(199)) {
                u.unit_y = fi(199);
            }
        }
    }

    /// `DE_GravityFlyUnit`.
    fn gravity_fly_unit(&self, u: &mut Unit) {
        if u.unit_y.add(u.unit_y_mov).gt(fi(199)) {
            u.unit_y = fi(199);
            u.unit_y_mov = F32::ZERO;
            u.is_y_in_air = false;
            return;
        }
        u.unit_y = u.unit_y.add(u.unit_y_mov);
        if u.unit_y.lt(fi(24)) {
            u.unit_y_mov = F32::ZERO;
            u.unit_y = fi(24);
        }
        u.unit_y_mov = u.unit_y_mov.add(if u.unit_type == UNIT_HELI { F_0_0001 } else { F_0_03 });
        if !self.stability_check(u.unit_x, to_u32(fi(u.unit_y.roundf()))) {
            u.unit_y_mov = F32::ZERO;
            u.is_y_in_air = false;
        }
    }

    /// `DE_RunTickAnimate`.
    fn animate(&mut self) {
        for p in &mut self.players {
            for u in &mut p.units {
                if !u.is_valid() {
                    continue;
                }
                if !SYSTEM_ANI[u.unit_type as usize] && u.ani_frame == 0 {
                    continue;
                }
                u.ani_frame += 1;
                if u.ani_frame > 3 {
                    u.ani_frame = 0;
                }
            }
        }
    }

    /// `DE_RunTickExplosions`.
    fn run_explosions(&mut self) {
        for i in 0..self.explosions.len() {
            if self.explosions[i].is_available {
                continue;
            }
            let e = self.explosions[i];
            for _ in 0..e.explofill {
                let f = self.rng_float();
                let radian = D64::from_f32(f).mul(D64::two_pi()).to_f32();
                let f = self.rng_float();
                let r = radian.cos().mul(f).mul(fu(e.explowidth)).roundf();
                let pos_y = to_i32(fu(e.y).add(fi(r)));
                let f = self.rng_float();
                let r = radian.sin().mul(f).mul(fu(e.explowidth)).roundf();
                let mut pos_x = to_i32(fu(e.x).add(fi(r)));
                while pos_x < 0 {
                    pos_x += 320;
                }
                while pos_x >= 320 {
                    pos_x -= 320;
                }
                if pos_y >= 200 || pos_y <= 15 {
                    continue;
                }
                match e.explo_type {
                    EXPL_DIRT => self.temp[(pos_x + pos_y * W as i32) as usize] = PIXEL_DIRT,
                    EXPL_NORMAL => {
                        super_pixel(&mut self.temp, pos_x as u32, pos_y as u32);
                        self.test_explosion_collision(pos_x as u32, pos_y as u32);
                    }
                    _ => {}
                }
            }
            let e = &mut self.explosions[i];
            e.explowidth += 1;
            if e.explowidth == e.explomax {
                e.is_available = true;
            }
        }
    }

    /// `DE_TestExplosionCollision`.
    fn test_explosion_collision(&mut self, pos_x: u32, pos_y: u32) {
        for i in 0..MAX_PLAYERS {
            for j in 0..self.players[i].units.len() {
                let u = self.players[i].units[j];
                if u.is_valid()
                    && pos_x > u.unit_x
                    && pos_x < u.unit_x.wrapping_add(11)
                    && fu(pos_y).lt(u.unit_y)
                    && fu(pos_y).gt(u.unit_y.sub(fi(11)))
                {
                    self.players[i].units[j].health -= 1;
                    if self.players[i].units[j].health <= 0 {
                        self.destroy_unit(i, j);
                    }
                }
            }
        }
    }

    /// `DE_DestroyUnit`.
    fn destroy_unit(&mut self, player: usize, j: usize) {
        let u = self.players[player].units[j];
        let y = to_u32(fi(u.unit_y.roundf()).sub(fi(5)));
        self.make_explosion(
            u.unit_x.wrapping_add(5),
            y,
            if u.unit_type == UNIT_HELI { SHOT_SMALL } else { SHOT_INVALID },
        );
        if u.unit_type != UNIT_SATELLITE {
            self.players[player].units_remaining = self.players[player].units_remaining.wrapping_sub(1);
            self.players[1 - player].score += 1;
        }
    }

    /// `JE_makeExplosion`.
    fn make_explosion(&mut self, x: u32, y: u32, shottype: i32) {
        let Some(i) = self.explosions.iter().position(|e| e.is_available) else {
            return;
        };
        let e = &mut self.explosions[i];
        e.is_available = false;
        e.x = x;
        e.y = y;
        e.explowidth = 2;
        if shottype != SHOT_INVALID {
            let t = shottype as usize;
            let size = EXPLO_SIZE[t];
            e.explomax = size;
            e.explofill = EXPLO_DENSITY[t];
            e.explo_type = SHOT_DIRT[t];
            if size < 5 {
                self.e_sound(3, x);
            } else if size < 15 {
                self.e_sound(4, x);
            } else if size < 20 {
                self.e_sound(12, x);
            } else if size < 40 {
                self.e_sound(11, x);
            } else {
                self.e_sound(12, x);
                self.e_sound(11, x);
            }
        } else {
            self.e_sound(4, x);
            let m = (self.rng.next_u32() % 40) + 10;
            let f = (self.rng.next_u32() % 60) + 20;
            let e = &mut self.explosions[i];
            e.explomax = m;
            e.explofill = f;
            e.explo_type = EXPL_NORMAL;
        }
    }

    /// `JE_eSound`.
    fn e_sound(&mut self, sound: u8, x: u32) {
        self.explo_sound_channel += 1;
        if self.explo_sound_channel > 5 {
            self.explo_sound_channel = 1;
        }
        let ch = self.explo_sound_channel as usize;
        self.sound_queue[ch] = sound;
        self.sound_queue_x[ch] = x.min(319) as u16;
    }

    /// `DE_RunTickShots`.
    fn run_shots(&mut self, vga: &mut [u8]) {
        for i in 0..self.shots.len() {
            if self.shots[i].is_available {
                continue;
            }
            let mut s = self.shots[i];
            let t = s.shottype as usize;
            s.x = s.x.add(s.xmov);
            s.y = s.y.add(s.ymov);
            if SHOT_BOUNCE[t] {
                if s.y.gt(fi(199)) || s.y.lt(fi(14)) {
                    s.y = s.y.sub(s.ymov);
                    s.ymov = s.ymov.neg();
                }
                if s.x.lt(fi(1)) || s.x.gt(fi(318)) {
                    s.x = s.x.sub(s.xmov);
                    s.xmov = s.xmov.neg();
                }
            } else {
                s.ymov = s.ymov.add(F_0_05);
                if s.y.gt(fi(199)) {
                    s.y = s.y.sub(s.ymov);
                    s.ymov = s.ymov.neg().mul(F_0_8);
                    if s.xmov.eq_f(F32::ZERO) {
                        let f = self.rng_float();
                        s.xmov = s.xmov.add(f.sub(F_0_5));
                    }
                }
            }
            if s.x.gt(fi(318)) || s.x.lt(fi(1)) {
                s.is_available = true;
                self.shots[i] = s;
                continue;
            }
            if !s.y.gt(fi(14)) {
                self.shots[i] = s;
                continue;
            }
            let mut pos_x = to_u32(fi(s.x.roundf()));
            let mut pos_y = to_u32(fi(s.y.roundf()));

            for p in 0..MAX_PLAYERS {
                for k in 0..self.players[p].units.len() {
                    let u = self.players[p].units[k];
                    if !u.is_valid() {
                        continue;
                    }
                    if pos_x > u.unit_x
                        && pos_x < u.unit_x.wrapping_add(11)
                        && fu(pos_y).lt(u.unit_y)
                        && fu(pos_y).gt(u.unit_y.sub(fi(13)))
                    {
                        s.is_available = true;
                        self.make_explosion(pos_x, pos_y, s.shottype as i32);
                    }
                }
            }

            let trails = (SHOT_COLOR[t] << 4).wrapping_sub(3);
            pix_cool(vga, pos_x, pos_y, trails as u8);
            match SHOT_TRAIL[t] {
                TRAILS_NORMAL => draw_trails(vga, &mut s, 2, 4, trails.wrapping_sub(3)),
                TRAILS_FULL => draw_trails(vga, &mut s, 4, 3, trails.wrapping_sub(1)),
                _ => {}
            }

            for j in 0..self.walls.len() {
                let w = self.walls[j];
                if w.exist
                    && pos_x >= w.x
                    && pos_x <= w.x.wrapping_add(11)
                    && pos_y >= w.y
                    && pos_y <= w.y.wrapping_add(14)
                {
                    if DEMOLISH[t] {
                        self.walls[j].exist = false;
                        s.is_available = true;
                        self.make_explosion(pos_x, pos_y, s.shottype as i32);
                        continue;
                    }
                    let px = s.x.sub(s.xmov);
                    if px.lt(fu(w.x)) || px.gt(fu(w.x.wrapping_add(11))) {
                        s.xmov = s.xmov.neg();
                    }
                    let py = s.y.sub(s.ymov);
                    if py.lt(fu(w.y)) || py.gt(fu(w.y.wrapping_add(14))) {
                        s.ymov = if s.ymov.lt(F32::ZERO) { s.ymov.neg() } else { s.ymov.neg().mul(F_0_8) };
                    }
                    pos_x = to_u32(fi(s.x.roundf()));
                    pos_y = to_u32(fi(s.y.roundf()));
                }
            }

            let k = pos_x as usize + pos_y as usize * W;
            if self.temp.get(k).copied() == Some(PIXEL_DIRT) {
                s.is_available = true;
                self.shots[i] = s;
                self.make_explosion(pos_x, pos_y, s.shottype as i32);
                continue;
            }
            self.shots[i] = s;
        }
    }

    /// `DE_RunTickAI`.
    fn run_ai(&mut self) {
        let n = self.config.max_installations as usize;
        for i in 0..MAX_PLAYERS {
            if !self.players[i].is_cpu {
                continue;
            }
            let target = if i + 1 >= MAX_PLAYERS { 0 } else { i + 1 };
            // Helicopter first, chosen before reading the current unit.
            for j in 0..n {
                let u = self.players[i].units[j];
                if u.is_valid() && u.unit_type == UNIT_HELI {
                    self.players[i].unit_selected = j as u32;
                    break;
                }
            }
            let sel = self.players[i].unit_selected as usize;
            let mut cur = self.players[i].units[sel];
            let mut ai = self.players[i].ai;
            let mut moves = self.players[i].moves;

            if ai.c_no_down > 0 {
                ai.c_no_down -= 1;
            }
            if self.rng.next_u32() % 100 > 80 {
                ai.c_angle = ai.c_angle.wrapping_add(((self.rng.next_u32() % 3).wrapping_sub(1)) as i32);
                ai.c_angle = ai.c_angle.clamp(-1, 1);
            }
            if self.rng.next_u32() % 100 > 90 {
                let a = D64::from_f32(cur.angle);
                if (ai.c_angle > 0 && a.gt(D64::half_pi().sub(D64::pi().div(D64::from_int(9)))))
                    || (ai.c_angle < 0 && a.lt(D64::pi().div(D64::from_int(8))))
                {
                    ai.c_angle = 0;
                }
            }
            if self.rng.next_u32() % 100 > 93 {
                ai.c_power = ai.c_power.wrapping_add(((self.rng.next_u32() % 3).wrapping_sub(1)) as i32);
                ai.c_power = ai.c_power.clamp(-1, 1);
            }
            if self.rng.next_u32() % 100 > 90 {
                if (ai.c_power > 0 && cur.power.gt(fi(4))) || (ai.c_power < 0 && cur.power.lt(fi(3))) {
                    ai.c_power = 0;
                } else if cur.power.lt(fi(2)) {
                    ai.c_power = 1;
                }
            }

            if cur.unit_type == UNIT_HELI {
                if !cur.is_y_in_air {
                    ai.c_power = 1;
                }
                if self.rng.next_u32() % cur.unit_x.max(1) > 100 {
                    ai.c_power = 1;
                }
                if self.rng.next_u32() % 240 > cur.unit_x {
                    moves[MOVE_RIGHT] = true;
                } else if (self.rng.next_u32() % 20) + 300 < cur.unit_x {
                    moves[MOVE_LEFT] = true;
                } else if self.rng.next_u32() % 30 == 1 {
                    ai.c_angle = ((self.rng.next_u32() % 3).wrapping_sub(1)) as i32;
                }
                if cur.unit_x > 295 && cur.last_move > 1 {
                    moves[MOVE_LEFT] = true;
                    moves[MOVE_RIGHT] = false;
                }
                if cur.unit_type != UNIT_HELI || cur.last_move > 3 || (cur.unit_x > 160 && cur.last_move > -3) {
                    let d = (to_i32(fi(cur.unit_y.roundf())) as u32).max(1);
                    if self.rng.next_u32() % d < 150
                        && cur.unit_y_mov.lt(F_0_01)
                        && (cur.unit_x < 160 || cur.last_move < 2)
                    {
                        moves[MOVE_FIRE] = true;
                    }
                    let lm = 5 - cur.last_move.abs();
                    ai.c_no_down = (lm * lm + 3) as u32;
                    ai.c_power = 1;
                } else {
                    moves[MOVE_FIRE] = false;
                }
                for j in 0..n {
                    let t = self.players[target].units[j];
                    if (t.unit_x as i32 - cur.unit_x as i32).abs() < 8 {
                        if t.unit_type == UNIT_SATELLITE {
                            moves[MOVE_FIRE] = false;
                        } else {
                            moves[MOVE_LEFT] = false;
                            moves[MOVE_RIGHT] = false;
                            if cur.last_move < -1 {
                                cur.last_move += 1;
                            } else if cur.last_move > 1 {
                                cur.last_move -= 1;
                            }
                        }
                    }
                }
            } else {
                moves[MOVE_FIRE] = true;
            }

            if self.rng.next_u32() % 200 > 198 {
                moves[MOVE_CHANGE] = true;
                ai.c_angle = 0;
                ai.c_power = 0;
                ai.c_fire = 0;
            }
            if self.rng.next_u32() % 100 > 98 || cur.shot_type == SHOT_TRACER {
                moves[MOVE_CYDN] = true;
            }
            if ai.c_angle > 0 {
                moves[MOVE_LEFT] = true;
            }
            if ai.c_angle < 0 {
                moves[MOVE_RIGHT] = true;
            }
            if ai.c_power > 0 {
                moves[MOVE_UP] = true;
            }
            if ai.c_power < 0 && ai.c_no_down == 0 {
                moves[MOVE_DOWN] = true;
            }
            if ai.c_fire > 0 {
                moves[MOVE_FIRE] = true;
            }
            if cur.unit_y_mov.lt(F_0_1.neg()) && cur.unit_type == UNIT_HELI {
                moves[MOVE_FIRE] = false;
            }
            if cur.unit_type == UNIT_LASER || cur.is_y_in_air {
                ai.c_power = 0;
            }
            self.players[i].units[sel] = cur;
            self.players[i].ai = ai;
            self.players[i].moves = moves;
        }
    }

    /// `DE_RunTickDrawCrosshairs`.
    fn draw_crosshairs(&self, vga: &mut [u8]) {
        for (i, p) in self.players.iter().enumerate() {
            let direction: i64 = if i == PLAYER_LEFT { -1 } else { 1 };
            let u = &p.units[p.unit_selected as usize];
            let (x, y);
            if u.unit_type == UNIT_HELI {
                let lm = fi(u.last_move as i64);
                let r = F_0_1.mul(lm).mul(lm).mul(lm).roundf();
                x = to_i32(fu(u.unit_x).add(fi(r)).add(fi(5)));
                y = to_i32(fi(u.unit_y.roundf()).add(fi(1)));
            } else {
                let reach = u.power.mul(fi(8)).add(fi(7));
                x = to_i32(fi(fu(u.unit_x.wrapping_add(6)).sub(u.angle.cos().mul(reach).mul(fi(direction))).roundf()));
                y = to_i32(fi(u.unit_y.sub(fi(7)).sub(u.angle.sin().mul(reach)).roundf()));
            }
            if y > 9 {
                if y > 11 {
                    if y > 13 {
                        pix(vga, x, y - 2, 3);
                    }
                    pix(vga, x + 3, y, 3);
                    pix(vga, x, y, 14);
                    pix(vga, x - 3, y, 3);
                }
                pix(vga, x, y + 2, 3);
            }
        }
    }

    /// `DE_RunTickDrawHUD`.
    fn draw_hud(&self, c: &mut dyn Canvas) {
        for (i, p) in self.players.iter().enumerate() {
            let u = &p.units[p.unit_selected as usize];
            let sx: i32 = if i == PLAYER_LEFT { 0 } else { 320 - 150 };
            {
                let v = c.pixels();
                fill_rectangle_xy(v, sx + 5, 3, sx + 14, 8, 241);
                rectangle(v, sx + 4, 2, sx + 15, 9, 242);
                rectangle(v, sx + 3, 1, sx + 16, 10, 240);
                fill_rectangle_xy(v, sx + 18, 3, sx + 140, 8, 241);
                rectangle(v, sx + 17, 2, sx + 143, 9, 242);
                rectangle(v, sx + 16, 1, sx + 144, 10, 240);
            }
            if u.shot_type != SHOT_INVALID {
                c.blit_sprite2(sx + 4, 0, 191 + u.shot_type as u32);
                let name = self.weapon_names.get(u.shot_type as usize).cloned().unwrap_or_default();
                c.out_text(sx + 20, 3, &name, 15, 2);
            }
            c.out_text(sx + 75, 3, &hud_value(&self.hud_dmg, u.health as i64), 15, 0);
            c.out_text(sx + 110, 3, &hud_value(&self.hud_pts, p.score as i64), 15, 0);
        }
    }

    /// `DE_ProcessInput`.
    fn process_input(&mut self) {
        for pi in 0..MAX_PLAYERS {
            if self.players[pi].units_remaining == 0 {
                continue;
            }
            let direction: i64 = if pi == PLAYER_LEFT { -1 } else { 1 };
            let sel = self.players[pi].unit_selected as usize;
            let moves = self.players[pi].moves;
            let mut u = self.players[pi].units[sel];

            if SYSTEM_ANGLE[u.unit_type.max(0) as usize] {
                if moves[MOVE_LEFT] {
                    if pi == PLAYER_LEFT {
                        raise_angle(&mut u);
                    } else {
                        lower_angle(&mut u);
                    }
                }
                if moves[MOVE_RIGHT] {
                    if pi == PLAYER_LEFT {
                        lower_angle(&mut u);
                    } else {
                        raise_angle(&mut u);
                    }
                }
            } else if u.unit_type == UNIT_HELI {
                if moves[MOVE_LEFT] && u.unit_x > 5 && self.stability_check(u.unit_x - 5, to_u32(fi(u.unit_y.roundf())))
                {
                    if u.last_move > -5 {
                        u.last_move -= 1;
                    }
                    u.unit_x -= 1;
                    if self.stability_check(u.unit_x, to_u32(fi(u.unit_y.roundf()))) {
                        u.is_y_in_air = true;
                    }
                }
                if moves[MOVE_RIGHT]
                    && u.unit_x < 305
                    && self.stability_check(u.unit_x + 5, to_u32(fi(u.unit_y.roundf())))
                {
                    if u.last_move < 5 {
                        u.last_move += 1;
                    }
                    u.unit_x += 1;
                    if self.stability_check(u.unit_x, to_u32(fi(u.unit_y.roundf()))) {
                        u.is_y_in_air = true;
                    }
                }
            }

            if u.unit_type != UNIT_LASER {
                if moves[MOVE_UP] {
                    if u.unit_type == UNIT_HELI {
                        u.is_y_in_air = true;
                        u.unit_y_mov = u.unit_y_mov.sub(F_0_1);
                    } else if u.unit_type == UNIT_JUMPER && !u.is_y_in_air {
                        u.unit_y_mov = fi(-3);
                        u.is_y_in_air = true;
                    } else {
                        u.power = u.power.add(F_0_05);
                        if u.power.gt(fi(5)) {
                            u.power = fi(5);
                        }
                    }
                }
                if moves[MOVE_DOWN] {
                    if u.unit_type == UNIT_HELI && u.is_y_in_air {
                        u.unit_y_mov = u.unit_y_mov.add(F_0_1);
                    } else {
                        u.power = u.power.sub(F_0_05);
                        if u.power.lt(fi(1)) {
                            u.power = fi(1);
                        }
                    }
                }
            }

            if moves[MOVE_CYUP] {
                self.cycle_weapon(&mut u, 1);
            }
            if moves[MOVE_CYDN] {
                self.cycle_weapon(&mut u, -1);
            }
            self.players[pi].units[sel] = u;

            if moves[MOVE_CHANGE] {
                let p = &mut self.players[pi];
                p.unit_selected += 1;
                if p.unit_selected >= self.config.max_installations {
                    p.unit_selected = 0;
                }
            }

            if self.players[pi].shot_delay > 0 {
                self.players[pi].shot_delay -= 1;
            }
            if moves[MOVE_FIRE] && self.players[pi].shot_delay == 0 {
                let t = u.shot_type;
                self.players[pi].shot_delay = SHOT_DELAY[t as usize];
                match SHOT_DIRT[t as usize] {
                    EXPL_MAGNET => self.run_magnet(pi, sel),
                    EXPL_DIRT | EXPL_NORMAL => self.make_shot(pi, &u, direction),
                    _ => {}
                }
            }
        }
    }

    /// `DE_CycleWeaponUp` / `DE_CycleWeaponDown`.
    fn cycle_weapon(&self, u: &mut Unit, dir: i32) {
        let systems = &self.weapon_systems[u.unit_type as usize];
        if !systems.iter().any(|&b| b) {
            return; // satellite: endless loop in the C code, never reached
        }
        loop {
            u.shot_type += dir;
            if u.shot_type > SHOT_LAST {
                u.shot_type = 0;
            }
            if u.shot_type < 0 {
                u.shot_type = SHOT_LAST;
            }
            if systems[u.shot_type as usize] {
                break;
            }
        }
    }

    /// `DE_MakeShot`.
    fn make_shot(&mut self, player: usize, u: &Unit, direction: i64) {
        let Some(si) = self.shots.iter().position(|s| s.is_available) else {
            return;
        };
        if u.unit_type == UNIT_HELI && !u.is_y_in_air {
            return;
        }
        self.sound_queue[player] = SHOT_SOUND[u.shot_type as usize];
        self.sound_queue_x[player] = u.unit_x.wrapping_add(5).min(319) as u16;

        let dir = fi(direction);
        let mut s = self.shots[si];
        let aimed = |s: &mut Shot| {
            let (c, sn) = (u.angle.cos(), u.angle.sin());
            s.x = fu(u.unit_x.wrapping_add(6)).sub(c.mul(fi(10)).mul(dir));
            s.y = u.unit_y.sub(fi(7)).sub(sn.mul(fi(10)));
            s.xmov = c.neg().mul(u.power).mul(dir);
            s.ymov = sn.neg().mul(u.power);
        };
        match u.unit_type {
            UNIT_HELI => {
                s.x = fu(u.unit_x.wrapping_add((u.last_move * 2) as u32).wrapping_add(5));
                let lm = fi(u.last_move as i64);
                s.xmov = F_0_02.mul(lm).mul(lm).mul(lm);
                if self.players[player].moves[MOVE_UP] && u.unit_y.lt(fi(30)) {
                    s.y = u.unit_y;
                    s.ymov = F_0_1;
                    if s.xmov.lt(F32::ZERO) {
                        s.xmov = s.xmov.add(F_0_1);
                    } else if s.xmov.gt(F32::ZERO) {
                        s.xmov = s.xmov.sub(F_0_1);
                    }
                } else {
                    s.y = u.unit_y.add(fi(1));
                    s.ymov = F_0_5.add(u.unit_y_mov.mul(F_0_1));
                }
            }
            UNIT_JUMPER => {
                if self.config.jumper_straight[player] {
                    aimed(&mut s);
                } else {
                    s.x = fu(u.unit_x.wrapping_add(2));
                    s.xmov = u.angle.cos().neg().mul(u.power).mul(dir);
                    if u.is_y_in_air {
                        s.ymov = fi(1);
                        s.y = u.unit_y.add(fi(2));
                    } else {
                        s.ymov = fi(-2);
                        s.y = u.unit_y.sub(fi(12));
                    }
                }
            }
            _ => aimed(&mut s),
        }
        s.is_available = false;
        s.shottype = u.shot_type as u32;
        s.trailc = [0; 4];
        self.shots[si] = s;
    }

    /// `DE_RunMagnet`.
    fn run_magnet(&mut self, player: usize, j: usize) {
        let enemy = 1 - player;
        let direction: i64 = if player == PLAYER_LEFT { -1 } else { 1 };
        let magnet = self.players[player].units[j];
        let push = magnet.power.mul(F_0_1).mul(fi(-direction));
        for s in &mut self.shots {
            if s.is_available {
                continue;
            }
            if (player == PLAYER_LEFT && s.x.gt(fu(magnet.unit_x)))
                || (player == PLAYER_RIGHT && s.x.lt(fu(magnet.unit_x)))
            {
                s.xmov = s.xmov.add(push);
            }
        }
        for u in &mut self.players[enemy].units {
            if u.is_valid()
                && u.unit_type == UNIT_HELI
                && u.is_y_in_air
                && ((enemy == PLAYER_RIGHT && u.unit_x.wrapping_add(11) < 318)
                    || (enemy == PLAYER_LEFT && u.unit_x >= 7))
            {
                u.unit_x = u.unit_x.wrapping_sub((2 * direction) as u32);
            }
        }
        self.players[player].units[j].ani_frame = 1;
    }

    /// `DE_RunTickCheckEndgame`.
    fn check_endgame(&mut self) -> bool {
        for (loser, winner) in [(PLAYER_LEFT, PLAYER_RIGHT), (PLAYER_RIGHT, PLAYER_LEFT)] {
            if self.players[loser].units_remaining == 0 {
                self.players[winner].score += MODE_SCORE[loser][self.mode];
                self.sound_queue[7] = V_CLEARED_PLATFORM;
                return true;
            }
        }
        false
    }

    /// Height of the generated terrain (`world.baseMap[x]`, tests).
    pub fn base_height(&self, x: usize) -> u32 {
        self.base_map[x]
    }

    /// Number of shots in flight.
    pub fn active_shots(&self) -> usize {
        self.shots.iter().filter(|s| !s.is_available).count()
    }

    /// Number of explosions in progress.
    pub fn active_explosions(&self) -> usize {
        self.explosions.iter().filter(|e| !e.is_available).count()
    }

    /// Remaining end-of-round delay (`endDelay`).
    pub fn end_delay(&self) -> u32 {
        self.end_delay
    }
}

// --- C drawing functions ---------------------------------------------

/// `vga256d.c:JE_pix`: `x < pitch && y < h` in signed integers, then write
/// at `y * pitch + x` (a negative x falls on the previous line).
fn pix(vga: &mut [u8], x: i32, y: i32, c: u8) {
    if x < W as i32 && y < H as i32 {
        let k = y as i64 * W as i64 + x as i64;
        if k >= 0 && (k as usize) < vga.len() {
            vga[k as usize] = c;
        }
    }
}

/// `destruct.c:JE_pixCool` (`unsigned int` arguments converted to `int`).
fn pix_cool(vga: &mut [u8], x: u32, y: u32, c: u8) {
    let (x, y) = (x as i32, y as i32);
    pix(vga, x, y, c);
    let d = c.wrapping_sub(2);
    pix(vga, x.wrapping_sub(1), y, d);
    pix(vga, x.wrapping_add(1), y, d);
    pix(vga, x, y.wrapping_sub(1), d);
    pix(vga, x, y.wrapping_add(1), d);
}

/// `vga256d.c:JE_rectangle`.
pub fn rectangle(vga: &mut [u8], a: i32, b: i32, c: i32, d: i32, e: u8) {
    if !(a >= 0 && b >= 0 && c >= 0 && d >= 0 && a < W as i32 && b < H as i32 && c < W as i32 && d < H as i32) {
        return;
    }
    let p = W as i32;
    let mut put = |i: i32| {
        if i >= 0 && (i as usize) < vga.len() {
            vga[i as usize] = e;
        }
    };
    for k in 0..=(c - a) {
        put(b * p + a + k);
        put(d * p + a + k);
    }
    let mut i = (b + 1) * p + a;
    while i < d * p + a {
        put(i);
        i += p;
    }
    let mut i = (b + 1) * p + c;
    while i < d * p + c {
        put(i);
        i += p;
    }
}

/// `vga256d.c:fill_rectangle_xy` (SDL_FillRect clipped to the screen).
fn fill_rectangle_xy(vga: &mut [u8], x: i32, y: i32, x2: i32, y2: i32, color: u8) {
    for yy in y.max(0)..(y2 + 1).min(H as i32) {
        for xx in x.max(0)..(x2 + 1).min(W as i32) {
            vga[yy as usize * W + xx as usize] = color;
        }
    }
}

/// `aliasDirtPixel`: neighbors (including those already modified in this pass).
fn alias_dirt_pixel(s: &[u8], x: usize, y: usize) -> u8 {
    let k = y * W + x;
    let mut c = 0u8;
    if y > 0 && s[k - W] == PIXEL_DIRT {
        c += 1;
    }
    if y < H - 1 && s[k + W] == PIXEL_DIRT {
        c += 3;
    }
    if x > 0 && s[k - 1] == PIXEL_DIRT {
        c += 2;
    }
    if x < W - 1 && s[k + 1] == PIXEL_DIRT {
        c += 2;
    }
    if c != PIXEL_BLACK {
        c + 16
    } else {
        PIXEL_BLACK
    }
}

/// `JE_aliasDirt`.
fn alias_dirt(vga: &mut [u8]) {
    for y in 12..H {
        for x in 0..W {
            let k = y * W + x;
            if vga[k] == PIXEL_BLACK {
                vga[k] = alias_dirt_pixel(vga, x, y);
            }
        }
    }
}

/// `JE_stabilityCheck`: fewer than 10 dirt pixels out of the 12 under the unit.
fn stability_check(temp: &[u8], x: u32, y: u32) -> bool {
    let start = x as i64 + y as i64 * W as i64 - 1;
    let mut n = 0;
    for i in 0..12 {
        let k = start + i;
        if k >= 0 && (k as usize) < temp.len() && temp[k as usize] == PIXEL_DIRT {
            n += 1;
        }
    }
    n < 10
}

/// `JE_superPixel`: 5×5 star on the temporary screen. The pointer advances
/// by `rowLen - 5` even for a skipped line, like the C code.
fn super_pixel(temp: &mut [u8], px: u32, py: u32) {
    const PATTERN: [[u32; 5]; 5] =
        [[0, 0, 246, 0, 0], [0, 247, 249, 247, 0], [246, 249, 252, 249, 246], [0, 247, 249, 247, 0], [0, 0, 246, 0, 0]];
    const INTENSITY: [[u32; 5]; 5] =
        [[0, 0, 1, 0, 0], [0, 1, 2, 1, 0], [1, 2, 4, 2, 1], [0, 1, 2, 1, 0], [0, 0, 1, 0, 0]];
    let row = W as i64;
    let mut s = row * (py as i64 - 2) + (px as i64 - 2);
    for y in 0..5i64 {
        let ly = py as i64 + y - 2;
        if ly < 0 || ly >= H as i64 {
            s += row - 5;
            continue;
        }
        for x in 0..5i64 {
            let lx = px as i64 + x - 2;
            let k = s;
            s += 1;
            if lx < 0 || lx >= W as i64 {
                continue;
            }
            let pat = PATTERN[y as usize][x as usize];
            if pat == 0 || k < 0 || k as usize >= temp.len() {
                continue;
            }
            let v = temp[k as usize] as u32;
            temp[k as usize] = if v < pat {
                pat as u8
            } else if v + INTENSITY[y as usize][x as usize] > 255 {
                255
            } else {
                (v + INTENSITY[y as usize][x as usize]) as u8
            };
        }
        s += row - 5;
    }
}

/// `DE_DrawTrails`.
fn draw_trails(vga: &mut [u8], s: &mut Shot, count: usize, decay: u32, start: u32) {
    for i in (0..count).rev() {
        if s.trailc[i] > 0 && s.traily[i] > 12 {
            pix_cool(vga, s.trailx[i], s.traily[i], s.trailc[i] as u8);
        }
        if i == 0 {
            s.trailx[0] = to_u32(fi(s.x.roundf()));
            s.traily[0] = to_u32(fi(s.y.roundf()));
            s.trailc[0] = start;
        } else {
            s.trailx[i] = s.trailx[i - 1];
            s.traily[i] = s.traily[i - 1];
            if s.trailc[i - 1] > 0 {
                s.trailc[i] = s.trailc[i - 1].wrapping_sub(decay);
            }
        }
    }
}

/// `DE_GravityDrawUnit`.
fn draw_unit(c: &mut dyn Canvas, team: usize, u: &Unit) {
    let mut anim = GRAPHIC_BASE[team][u.unit_type as usize] + u.ani_frame;
    if u.unit_type == UNIT_HELI {
        if u.last_move < -2 {
            anim += 5;
        } else if u.last_move > 2 {
            anim += 10;
        }
    } else {
        let k = D64::from_f32(u.angle.mul(F_9_99)).div(D64::pi()).to_f32().floorf();
        anim = (anim as i64 + k) as u32;
    }
    let y = to_i32(fi(u.unit_y.roundf()).sub(fi(13)));
    c.blit_sprite2(u.unit_x as i32, y, anim);
}

fn raise_angle(u: &mut Unit) {
    u.angle = u.angle.add(F_0_01);
    let lim = D64::half_pi().sub(D64::from_f32(F_0_01));
    if D64::from_f32(u.angle).gt(lim) {
        u.angle = lim.to_f32();
    }
}

fn lower_angle(u: &mut Unit) {
    u.angle = u.angle.sub(F_0_01);
    if u.angle.lt(F32::ZERO) {
        u.angle = F32::ZERO;
    }
}

/// `snprintf(tempstr, 16, "%s~%d~", word, n)`: 15 bytes at most.
fn hud_value(word: &[u8], n: i64) -> Vec<u8> {
    let mut v = word.to_vec();
    v.push(b'~');
    v.extend_from_slice(n.to_string().as_bytes());
    v.push(b'~');
    v.truncate(15);
    v
}

#[cfg(test)]
mod tests;
