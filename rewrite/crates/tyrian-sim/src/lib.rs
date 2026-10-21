//! Deterministic fixed-step simulation core (see docs/01-architecture.md §3).
//!
//! No dependency on Godot, no floating point in the game logic (the two
//! C floating-point computations that touch state are reproduced bit for bit in
//! integers by [`fmath`]). One call to [`Sim::step`] advances one tick of the original
//! game (one iteration of `tyrian2.c:JE_main/level_loop`).
//!
//! API CONTRACT — used both by `tyrian-golden` (comparison with the C code)
//! and by `tyrian-godot` (rendering). The public names and types of this file do not
//! change without updating these two crates.
//!
//! # Breakdown of a tick
//!
//! The C golden master (`golden_master.c:golden_master_inject`) writes the line
//! of tick k in the middle of iteration k+1, right after `JE_playerCollide`.
//! To match this boundary:
//! - [`Sim::new`] plays the level initialization then the top of the first
//!   iteration (events, scrolling, enemies, shots, collisions);
//! - [`Sim::step`] applies the aids (`assist`), plays the bottom of the current
//!   iteration with the supplied input (player movement and fire, enemy
//!   shots, explosions, level end), then the top of the next iteration.
//!   [`Sim::frame_record`] then corresponds to line k of the `.gm` file.
//!   When the level ends (`reallyEndLevel`), the top of the next iteration
//!   is not played, like `golden_master_level_end`.
//!
//! # Coordinate conventions (views for Godot)
//!
//! All view positions are in the frame of the C game surface
//! (`game_screen`, 320×200) — the same as `player[].x/y`. The visible play area
//! starts at x = [`PLAY_AREA_X`] (24): `JE_starShowVGA` copies columns
//! 24..288 of it to the screen. Subtract 24 to get a pixel of the play
//! area.
//! - [`PlayerView`]: `x, y` = `player[].x/y`; the ship (2×2 frames of
//!   12×14) is drawn at (x - 5, y - 7) (`player_movement.c`).
//! - [`EnemyView`]: real top-left corner of the sprite; an enemy with `size == 1`
//!   (2×2) occupies 4 frames of 12×14: `frame`, `frame+1` (top row),
//!   `frame+19`, `frame+20` (bottom row), like `enemy_render.c:JE_drawEnemy`.
//! - [`ShotView`]: top-left corner (player shots: `shotX + 1`, like the C code).
//! - [`ExplosionView`]: top-left corner, `sprite` = 1-based index into
//!   `newsh6.shp` (`explosionSpriteSheet`, `sprite + 1` in the C code).
//! - [`LayerScroll`]: map pixel displayed at the top-left corner of the visible
//!   play area for each layer (24×28 tiles, row 0 at the top of the
//!   map; layers 14, 14 and 15 tiles wide). Computation:
//!   `backgrnd.c:draw_background_1/2/3`.

pub mod achievements;
pub mod anomalies;
pub mod behavior;
pub mod bench;
pub mod campaign;
pub mod collision;
pub mod combos;
pub mod demo;
pub mod destruct;
pub mod destruct_config;
pub mod dev;
pub mod endless;
pub mod enemies;
pub mod events;
pub mod explosions;
pub mod f32ops;
pub mod fmath;
pub mod fx;
pub mod level;
mod link;
pub mod modes;
pub mod option_shapes;
pub mod player;
pub mod replay;
pub mod rng;
pub mod setup;
pub mod shop;
pub mod shop_preview;
pub mod shots;
pub mod sound;
pub mod tables;
pub mod world;

use tyrian_formats::hdt::ItemTables;
use tyrian_formats::lvl::Level;
pub use world::MusicCue;
use world::World;

/// Horizontal offset of the visible play area in the C surface (`video.h:PLAY_AREA_X`).
pub const PLAY_AREA_X: i32 = 24;

/// Input of one player for a tick: same encoding as the C golden master
/// (`src/golden_master.c`) — bit 0 up, 1 down, 2 left, 3 right, 4 fire,
/// 5 fire mode change, 6 left sidekick, 7 right sidekick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerInput(pub u8);

impl PlayerInput {
    pub const UP: u8 = 1 << 0;
    pub const DOWN: u8 = 1 << 1;
    pub const LEFT: u8 = 1 << 2;
    pub const RIGHT: u8 = 1 << 3;
    pub const FIRE: u8 = 1 << 4;
    pub const CHANGE_FIRE: u8 = 1 << 5;
    pub const LEFT_SIDEKICK: u8 = 1 << 6;
    pub const RIGHT_SIDEKICK: u8 = 1 << 7;

    pub fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

/// Player item identifier: index into the table of its family
/// (`tyrian.hdt`, tables patched by mods), 0 = none. Families named
/// as in the mod format (ADR 0083: ship, weapon, sidekick, shield,
/// special), plus the generator.
macro_rules! item_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub u8);

        impl $name {
            /// Index into the family table.
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }

        /// The number, as written in the golden masters and the traces.
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
item_id!(ShipId, "Ship (`ships`).");
item_id!(WeaponId, "Front or rear weapon (`weaponPort`).");
item_id!(ShieldId, "Shield (`shields`).");
item_id!(GeneratorId, "Generator (`powerSys`).");
item_id!(SidekickId, "Sidekick (`options`).");
item_id!(SpecialId, "Special weapon (`special`).");

/// Player equipment at level start (`player.h:PlayerItems`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loadout {
    pub ship: ShipId,
    pub front_weapon: WeaponId,
    /// Power level 1..=11 of the front weapon.
    pub front_power: u8,
    pub rear_weapon: WeaponId,
    pub rear_power: u8,
    pub shield: ShieldId,
    pub generator: GeneratorId,
    pub left_sidekick: SidekickId,
    pub right_sidekick: SidekickId,
    pub special: SpecialId,
    pub cash: u32,
}

/// Everything needed to start a level.
#[derive(Debug, Clone)]
pub struct LevelSetup {
    pub episode: u8,
    /// Section of `levelsN.dat` (`mainLevel`), informational.
    pub section: u8,
    pub difficulty: u8,
    pub level: Level,
    /// Item/enemy tables of the episode (`tyrian.hdt` or the episode 4-5 block).
    pub items: ItemTables,
    pub loadout: Loadout,
    pub seed: u64,
    /// Autopilot aids (hull, shield, invulnerability, weapon power
    /// held: `autopilot.c:autopilot_drive`), to replay the golden
    /// masters recorded with `assist 1`.
    pub assist: bool,
    /// The C `initialDifficulty` when it differs from `difficulty`: in
    /// the campaign, `mainint.c:adjust_difficulty` raises `difficultyLevel` over
    /// the levels while `initialDifficulty` (menu choice) stays fixed.
    /// `None`: identical to `difficulty` (isolated level, `--goto`).
    pub initial_difficulty: Option<u8>,
    /// Equipment of `player[1]` when it is not copied from `loadout`: the C code
    /// only does `player[1].items = player[0].items` once, in
    /// `mainint.c:JE_initPlayerData` (new game); the shop only touches
    /// `player[0]`. At level N of a campaign, `player[1]` therefore keeps
    /// the starting equipment. `None`: copy of `loadout` (isolated level).
    pub second_player_loadout: Option<Loadout>,
    /// Fields of `player[]` that `tyrian2.c:JE_main` does not reset at the
    /// start of a level and which therefore pass from one level to the next in
    /// the campaign (taken at the end of the previous level). `None`: values
    /// of a program that is starting (`--goto`).
    pub carry_over: Option<LevelCarry>,
    /// Side modes (block 5); default = normal solo game.
    pub modes: modes::ModeSetup,
    /// Endless mode: armor multiplier `endless_armor_multiplier()`
    /// applied to new enemies (`enemy_spawn.c:JE_makeEnemy`), `None` outside
    /// Endless. See [`endless::armor_mult_for_levels`].
    pub endless_armor_mult: Option<fmath::F32>,
    /// Tables and behaviors added by mods (empty = original game).
    pub patch: behavior::TablePatch,
    /// `config.c:processorType` (Setup > Graphics > Visual FX; 1 = Classic,
    /// 3 = High, 4 = Wild by default). Read by the simulation (draws of
    /// `hitstop.c:on_boss_defeated`, layer 2 of `tyrian2.c`, stars of
    /// `backgrnd.c:initialize_starfield`) and by rendering.
    pub processor_type: u8,
}

/// Player state inherited from the previous level (see [`LevelSetup::carry_over`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerCarry {
    /// `player_movement.c:JE_playerMovement`: friction counters;
    /// `JE_main` resets the speeds to zero but not these counters.
    pub x_friction_ticks: u32,
    pub y_friction_ticks: u32,
    /// The player's `items.sidekick_level` / `sidekick_series`: Endless only resets them to zero
    /// at the start of the run (`endless.c:849`), not at each wave.
    pub sidekick_level: u8,
    pub sidekick_series: u8,
}

/// C state that passes from one level to the next (campaign, Endless waves, demo):
/// that of each player and `varz.c:fireButtonHeld`, a static variable that
/// `tyrian2.c:JE_main` never resets. True at level end as long as the
/// fire button is held, it holds back the special weapon (`JE_doSpecialShot`) on the first tick
/// of the next level: without it, two special shots too many at each new wave.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelCarry {
    pub players: [PlayerCarry; 2],
    pub special_fire_held: bool,
}

impl LevelCarry {
    /// Taken at the end of a simulated level.
    pub fn from_sim(sim: &Sim) -> LevelCarry {
        let p = &sim.world().players;
        LevelCarry {
            players: [0, 1].map(|i| PlayerCarry {
                x_friction_ticks: p[i].x_friction_ticks,
                y_friction_ticks: p[i].y_friction_ticks,
                sidekick_level: p[i].items.sidekick_level,
                sidekick_series: p[i].items.sidekick_series,
            }),
            special_fire_held: sim.world().special_weapon.fire_button_held,
        }
    }

    /// Order of the Godot bridge (`TyrianSim.campaign_carry`): friction x, y of
    /// player 1, then of player 2, then the held special shot (0 or 1), then level and
    /// sidekick series of player 2.
    pub fn to_values(&self) -> [i32; 7] {
        let [a, b] = self.players;
        [
            a.x_friction_ticks as i32,
            a.y_friction_ticks as i32,
            b.x_friction_ticks as i32,
            b.y_friction_ticks as i32,
            self.special_fire_held as i32,
            b.sidekick_level as i32,
            b.sidekick_series as i32,
        ]
    }

    /// Inverse of [`to_values`](Self::to_values); 4 values (saves and
    /// settings from before this field): special shot released. `None` under 4 values.
    pub fn from_values(v: &[i32]) -> Option<LevelCarry> {
        (v.len() >= 4).then(|| LevelCarry {
            players: [0, 1].map(|i| PlayerCarry {
                x_friction_ticks: v[2 * i].max(0) as u32,
                y_friction_ticks: v[2 * i + 1].max(0) as u32,
                // Values 5 and 6: player 2 only (older captures: 0).
                sidekick_level: if i == 1 { v.get(5).map_or(0, |&x| x.clamp(0, 255) as u8) } else { 0 },
                sidekick_series: if i == 1 { v.get(6).map_or(0, |&x| x.clamp(0, 255) as u8) } else { 0 },
            }),
            special_fire_held: v.get(4).is_some_and(|&f| f != 0),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerView {
    pub x: i32,
    pub y: i32,
    pub armor: i32,
    pub shield: i32,
    pub alive: bool,
    pub invulnerable_ticks: i32,
    pub cash: i64,
    /// Ship banking (-2..2) to choose the image.
    pub banking: i32,
}

/// Position of the three background layers, in pixels in map space:
/// the top-left corner of the play area displays pixel (`x_px`, `y_px`) of
/// the layer (24×28 tiles; map row 0 at the top).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayerScroll {
    pub x_px: i32,
    pub y_px: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnemyView {
    pub slot: u16,
    /// Position of the sprite's top-left corner, `game_screen` frame (see the module header).
    pub x: i32,
    pub y: i32,
    /// Sprite2 sheet used: character of the `newsh?.shp` file in
    /// lowercase (`b'0'`, `b'a'`...). Two special values designate
    /// tables of `tyrian.shp`: 10 (`spriteSheet10`, objects) and 11
    /// (`spriteSheet11`, coins and gems). 0 = no sheet (not drawn).
    pub sheet: u8,
    /// Index of the image in the sheet (1-based as in the C code).
    pub frame: u16,
    /// 1 = 1×1, 2 = 2×2 images (large enemies).
    pub size: u8,
    /// Drawing layer (0 ground, 1 high ground, 2 sky, 3 top), order of `tyrian2.c`.
    pub layer: u8,
    pub armor: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShotView {
    pub x: i32,
    pub y: i32,
    /// Image in the shot sheet (`shotGr` / `sgr`).
    pub sprite: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExplosionView {
    pub x: i32,
    pub y: i32,
    pub sprite: u16,
}

/// Same fields, in the same order, as a line of the C golden master
/// (`src/golden_master.c:write_tick`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameRecord {
    pub tick: u64,
    pub mask: u8,
    pub px: i32,
    pub py: i32,
    pub armor: u32,
    pub shield: u32,
    pub cash: u64,
    pub cur_loc: u32,
    pub enemies: u32,
    pub player_shots: u32,
    pub enemy_shots: u32,
    pub h_player: u64,
    pub h_scroll: u64,
    pub h_enemies: u64,
    pub h_player_shots: u64,
    pub h_enemy_shots: u64,
    pub h_mt: u64,
    pub h_rng: u64,
}

impl FrameRecord {
    /// Line in the exact format of `golden_master.c:write_tick` (18 columns).
    /// Shared by the writing of references (tyrian-golden) and by the
    /// tick-exact check in the exported game (TyrianSim.frame_line).
    pub fn golden_line(&self) -> String {
        format!(
            "{} {:02x} {} {} {} {} {} {} {} {} {} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x}",
            self.tick,
            self.mask,
            self.px,
            self.py,
            self.armor,
            self.shield,
            self.cash,
            self.cur_loc,
            self.enemies,
            self.player_shots,
            self.enemy_shots,
            self.h_player,
            self.h_scroll,
            self.h_enemies,
            self.h_player_shots,
            self.h_enemy_shots,
            self.h_mt,
            self.h_rng
        )
    }
}

/// State readable by Godot at each render frame (Godot owns none of it).
#[derive(Debug, Clone, Default)]
pub struct SimState {
    pub tick: u64,
    pub players: [PlayerView; 2],
    pub layers: [LayerScroll; 3],
    pub enemies: Vec<EnemyView>,
    pub player_shots: Vec<ShotView>,
    pub enemy_shots: Vec<ShotView>,
    pub explosions: Vec<ExplosionView>,
    /// `curLoc`: scroll position that drives the events.
    pub cur_loc: u32,
    pub level_ended: bool,
}

/// Simulation of a level. All the game logic lives here.
pub struct Sim {
    setup: LevelSetup,
    state: SimState,
    last_mask: u8,
    world: World,
    /// Presented frame (rendering only, see [`Sim::set_keep_presented`]).
    /// An iteration of tyrian2.c draws in two halves: background, enemies and
    /// player shots at the top (`loop_top`), then player, enemy shots,
    /// explosions and texts before presentation (`loop_bottom`). `step`
    /// chains the bottom half of one iteration and the top half of the
    /// next: we keep the state from before the bottom half (top drawings)
    /// and the one after (bottom drawings).
    presented_top: Option<Box<(World, SimState)>>,
    presented: Option<Box<(World, SimState)>>,
    keep_presented: bool,
    /// Positions of both ships at the end of the bottom half of the last
    /// iteration, before the head of the next one (`tyrian2.c`: where
    /// `replay_capture_frame` takes them). Used by the replay recorder.
    bottom_positions: [(i32, i32); 2],
}

const FNV_INIT: u64 = 0xcbf2_9ce4_8422_2325;

/// `golden_master.c:MIX`: FNV-1a 64 on a little-endian int64.
fn mix(h: u64, v: i64) -> u64 {
    let mut h = h;
    for b in v.to_le_bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Positions forced on the next tick of a replay: player 1, then player 2 if playing.
pub type ReplayPositions = ((i32, i32), Option<(i32, i32)>);

impl Sim {
    pub fn new(setup: LevelSetup) -> Self {
        let mut world = World::start_level(&setup);
        world.loop_top();
        let mut sim = Self {
            setup,
            state: SimState::default(),
            last_mask: 0,
            world,
            presented_top: None,
            presented: None,
            keep_presented: false,
            bottom_positions: [(0, 0); 2],
        };
        sim.refresh_state();
        sim
    }

    /// Level loaded, generators forced, BEFORE the head of the first
    /// iteration: replay path (`replay.rs`), where `replay_inject_frame`
    /// runs at the top of `level_loop`, before everything else. Then advance
    /// with [`Sim::replay_begin`] then [`Sim::replay_iteration`].
    pub fn new_paused(setup: LevelSetup, rng: rng::Xoshiro256, mt: rng::MersenneTwister) -> Self {
        let mut world = World::start_level(&setup);
        world.rng = rng;
        world.mt = mt;
        let mut sim = Self {
            setup,
            state: SimState::default(),
            last_mask: 0,
            world,
            presented_top: None,
            presented: None,
            keep_presented: false,
            bottom_positions: [(0, 0); 2],
        };
        sim.refresh_state();
        sim
    }

    /// `replay_inject_frame`: player 1 position forced at the top
    /// of the iteration, then the top of `level_loop` (`loop_top`).
    pub fn replay_begin(&mut self, x: i32, y: i32) {
        self.replay_begin_2p(x, y, None);
    }

    /// [`Sim::replay_begin`] with, in two-player, the position of the second
    /// ship (`replay_inject_frame`, `rec_game_mode == 2`).
    pub fn replay_begin_2p(&mut self, x: i32, y: i32, p2: Option<(i32, i32)>) {
        self.world.players[0].x = x;
        self.world.players[0].y = y;
        if let Some((x2, y2)) = p2 {
            self.world.players[1].x = x2;
            self.world.players[1].y = y2;
        }
        self.world.loop_top();
        self.refresh_state();
    }

    /// End of the current iteration with the buttons of its frame, then, if
    /// `next` carries the next frame, its injection and the head of the
    /// next iteration. The resulting state is the one the C golden master
    /// writes for this tick (closed in the middle of the next iteration, or as is
    /// when the `.rep` is exhausted: `golden_master_replay_exhausted`).
    pub fn replay_iteration(&mut self, input: PlayerInput, next: Option<(i32, i32)>) {
        self.replay_iteration_2p(input, next.map(|n| (n, None)));
    }

    /// [`Sim::replay_iteration`] in two-player: both ships read
    /// the same buttons (`replay_capture_frame` copies the mask) and the
    /// position of the second is forced along with the first.
    pub fn replay_iteration_2p(&mut self, input: PlayerInput, next: Option<ReplayPositions>) {
        if self.state.level_ended {
            return;
        }
        self.last_mask = input.0;
        self.keep_presented_top();
        let input2 = if self.world.two_player_mode { input } else { PlayerInput(0) };
        self.world.loop_bottom([input, input2]);
        self.keep_presented_frame();
        if self.world.ending.really_end_level {
            self.state.level_ended = true;
        } else if let Some(((x, y), p2)) = next {
            self.world.players[0].x = x;
            self.world.players[0].y = y;
            if let Some((x2, y2)) = p2 {
                self.world.players[1].x = x2;
                self.world.players[1].y = y2;
            }
            self.world.loop_top();
        }
        self.state.tick += 1;
        self.refresh_state();
    }

    pub fn setup(&self) -> &LevelSetup {
        &self.setup
    }

    /// Full internal state (read-only), for tooling and debugging.
    /// Level stars (rendering only): drawn by the client with its own
    /// generator, like the C code between two levels.
    /// To be called before the first step: the iteration head already played by
    /// `Sim::new` advanced the stars once, this advance is redone.
    pub fn set_starfield(&mut self, stars: [world::StarfieldStar; world::MAX_STARS]) {
        self.world.render.starfield = stars;
        if self.world.render.starfield_drawn && self.state.tick == 0 {
            self.world.advance_starfield();
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Title demo (`play_demo`): player 1 keys come
    /// from `demo` (`mainint.c:replay_demo_keys`), the keyboard is the only
    /// device (`player_movement.c`: "inputDevice = 1") and death or
    /// the end of the file ends the level without a "Game Over" screen.
    pub fn set_demo(&mut self, demo: demo::DemoInput) {
        self.world.mode.devices[0] = 1;
        self.world.demo = Some(demo);
    }

    /// Campaign level marked "$" (`bonusLevelCurrent` or `normalBonusLevelCurrent`).
    pub fn set_bonus_level_current(&mut self, bonus: bool) {
        self.world.bonus_level_current = bonus;
    }

    /// Cursor of the demo in progress (`None` outside a demo).
    pub fn demo(&self) -> Option<&demo::DemoInput> {
        self.world.demo.as_ref()
    }

    /// Tools for comparison with the C code: see `RenderOnly::clock_offset_ms`.
    pub fn set_render_clock_offset(&mut self, ms: u32) {
        self.world.render.clock_offset_ms = ms;
    }

    /// Sets `config.c:processorType` (1..6; 1 = Classic, 3 = High, 4 = Wild).
    /// Reproduces `JE_initProcessorType()` mid-level: the re-derived state
    /// (`hitstop.c` cascade, layer 2 of `tyrian2.c`) takes effect on the next
    /// tick. The stars of `backgrnd.c` are regenerated by the caller
    /// (`initialize_starfield`, which consumes the global Mersenne Twister).
    pub fn set_processor_type(&mut self, value: u8) {
        let v = value.clamp(1, 6);
        self.world.processor_type = v;
        self.setup.processor_type = v;
    }

    /// Enables combos (`combos.c`, `combo_enabled = true`) with a fixed duration
    /// of `frame_ms` per loop for `combo_update` (C golden master:
    /// `GOLDEN_COMBO_MS`). Without this call, combos are off as under golden master.
    pub fn enable_combos(&mut self, frame_ms: u32) {
        self.world.combo.enabled = true;
        self.world.combo.frame_ms = frame_ms;
    }

    /// Combo in progress for the HUD: (count, multiplier, tier reached
    /// since the last call — `EVENT_COMBO_TIER_REACHED`, consumed).
    pub fn take_combo(&mut self) -> (u32, u32, Option<usize>) {
        let c = &mut self.world.combo;
        let tier = c.tier_reached.take();
        (c.count, self.world.combo_multiplier, tier)
    }

    /// Mouse of the next tick (`golden_master.c:golden_master_inject`).
    pub fn set_mouse(&mut self, mouse: modes::MouseInput) {
        self.world.mode.mouse = mouse;
    }

    /// Analog stick of both players, values already reduced
    /// (`joystick.c:joystick_axis_reduce`), held until the next call.
    pub fn set_analog(&mut self, analog: [(i32, i32); 2]) {
        self.world.mode.analog = analog;
    }

    /// `newkey` of the next tick (a key pressed since the previous one);
    /// consumed by the tick.
    pub fn set_new_key(&mut self, pressed: bool) {
        self.world.mode.new_key = pressed;
    }

    /// Player's Enhanced effects that act on the simulation
    /// (`fx::FxConfig`); never called by the golden masters or the replays.
    pub fn set_fx_config(&mut self, cfg: fx::FxConfig) {
        self.world.fx = cfg;
    }

    /// Effect events recorded since the last call (rendering only),
    /// with the loop iteration in which they occurred.
    pub fn take_fx_events(&mut self) -> Vec<(u64, fx::FxEvent)> {
        std::mem::take(&mut self.world.render.fx_events)
    }

    /// `hitstop.c:freeze_frames` and number of enemy updates skipped
    /// by the freeze since the start of the level.
    pub fn hitstop_state(&self) -> (u32, u64) {
        (self.world.hitstop.freeze_frames, self.world.hitstop.frozen_updates)
    }

    /// Advances one tick.
    pub fn step(&mut self, inputs: [PlayerInput; 2]) {
        self.world.mode.mouse_pending = true;
        if self.state.level_ended {
            self.world.mode.new_key = false;
            return;
        }
        self.last_mask = inputs[0].0;
        self.keep_presented_top();
        if self.world.assist {
            self.world.apply_assist();
        }
        self.world.loop_bottom(inputs);
        self.bottom_positions =
            [(self.world.players[0].x, self.world.players[0].y), (self.world.players[1].x, self.world.players[1].y)];
        // tyrian2.c: dev_console_poll, after the bottom half (--devmode).
        self.world.apply_dev_pending();
        self.keep_presented_frame();
        if self.world.ending.really_end_level {
            self.state.level_ended = true;
        } else {
            self.world.loop_top();
        }
        self.world.mode.new_key = false;
        self.state.tick += 1;
        self.refresh_state();
    }

    /// Positions of both ships before the head of the current iteration
    /// (end of the bottom half of the previous one).
    pub fn bottom_positions(&self) -> [(i32, i32); 2] {
        self.bottom_positions
    }

    pub fn state(&self) -> &SimState {
        &self.state
    }

    /// Keeps a copy of the presented frame at each tick (rendering). Without it,
    /// rendering reads the state after the top half of the next iteration:
    /// background, enemies and explosions one tick ahead of the player and the
    /// texts. Disabled by default (golden masters, bots): nothing is copied.
    pub fn set_keep_presented(&mut self, on: bool) {
        self.keep_presented = on;
        if !on {
            self.presented_top = None;
            self.presented = None;
        }
    }

    fn keep_copy(slot: &mut Option<Box<(World, SimState)>>, w: &World, st: &SimState) {
        match slot.as_mut() {
            Some(f) => {
                f.0.clone_from(w);
                f.1.clone_from(st);
            }
            None => *slot = Some(Box::new((w.clone(), st.clone()))),
        }
    }

    fn keep_presented_top(&mut self) {
        if self.keep_presented {
            Self::keep_copy(&mut self.presented_top, &self.world, &self.state);
        }
    }

    fn keep_presented_frame(&mut self) {
        if self.keep_presented {
            self.refresh_state();
            Self::keep_copy(&mut self.presented, &self.world, &self.state);
        }
    }

    /// State of the bottom-half drawings of the frame presented at the last
    /// tick (rendering); before the first tick or without
    /// [`Sim::set_keep_presented`], the current state.
    pub fn presented_world(&self) -> &World {
        self.presented.as_ref().map_or(&self.world, |f| &f.0)
    }

    /// Bottom-half views, see [`Sim::presented_world`].
    pub fn presented_state(&self) -> &SimState {
        self.presented.as_ref().map_or(&self.state, |f| &f.1)
    }

    /// State of the top-half drawings of the presented frame (background,
    /// enemies, player shots), see [`Sim::presented_world`].
    pub fn presented_top_world(&self) -> &World {
        self.presented_top.as_ref().map_or(&self.world, |f| &f.0)
    }

    /// Music orders emitted since the last call (play_song / set_volume
    /// of the level loop, ADR 0010). Does not touch the hashed state.
    /// Dashboard drawings made during the simulation since the
    /// last call (rendering only, outside the hashed state).
    pub fn take_hud_ops(&mut self) -> Vec<world::HudOp> {
        std::mem::take(&mut self.world.render.hud_ops)
    }

    pub fn take_music_cues(&mut self) -> Vec<MusicCue> {
        std::mem::take(&mut self.world.music.music_cues)
    }

    /// Sound effects emitted by `loudness.c:JE_flushSoundQueue` since the
    /// last call (rendering only, outside the hashed state, see [`sound`]).
    pub fn take_sounds(&mut self) -> Vec<sound::SoundEvent> {
        std::mem::take(&mut self.world.sound.played)
    }

    /// `firstGameOver` (tyrian2.c): true as long as game over has not been reached.
    pub fn first_game_over(&self) -> bool {
        self.world.ending.first_game_over
    }

    /// "Simulation" part of `tyrian2.c` "else if (!playing &&
    /// firstGameOver) play_song(levelSong - 1)" at the last tick: true if the
    /// C code reached this branch with `firstGameOver` true. The client, which alone
    /// knows whether its player is still playing (`playing`, lds_play.c), then restarts
    /// the level's song.
    pub fn music_level_song_branch(&self) -> bool {
        self.world.music.music_level_song_branch
    }

    /// `tyrMusicVolume` (0..255) used by events 34/35 and game over.
    pub fn set_music_volume(&mut self, volume: u16) {
        self.world.music.tyr_music_volume = volume.min(255);
    }

    fn refresh_state(&mut self) {
        let w = &self.world;
        for (i, p) in w.players.iter().enumerate() {
            self.state.players[i] = PlayerView {
                x: p.x,
                y: p.y,
                armor: p.armor as i32,
                shield: p.shield as i32,
                alive: p.is_alive,
                invulnerable_ticks: p.invulnerable_ticks as i32,
                cash: p.cash as i64,
                banking: p.banking,
            };
        }
        self.state.cur_loc = w.script.cur_loc as u32;
        self.state.layers = self.layer_scroll();
        self.state.enemies.clear();
        for i in 0..world::ENEMY_MAX {
            if w.enemy_avail[i] == 1 {
                continue;
            }
            let e = &w.enemy[i];
            // Position and image of the drawing (enemy_render.c:blit_enemy), not
            // those after the movement.
            let (ex, ey, cyc, mapoffset) =
                e.drawn.map_or((e.ex, e.ey, e.enemycycle, e.mapoffset), |d| (d.0, d.1, d.2, d.4));
            let cyc = cyc as usize;
            let frame = if (1..=20).contains(&cyc) { e.egr[cyc - 1] } else { 0 };
            let x0 = ex as i32 + mapoffset as i32;
            let (x, y) = if e.size == 1 { (x0 - 6, ey as i32 - 7) } else { (x0, ey as i32) };
            let layer = match i / 25 {
                0 => 2,
                1 => 0,
                2 => 3,
                _ => 1,
            };
            self.state.enemies.push(EnemyView {
                slot: i as u16,
                x,
                y,
                sheet: e.sheet,
                frame,
                size: if e.size == 1 { 2 } else { 1 },
                layer,
                armor: e.armorleft as i32,
            });
        }
        self.state.player_shots.clear();
        for i in 0..world::MAX_PWEAPON - 1 {
            if w.shot_avail[i] != 0 {
                let s = &w.player_shots[i];
                self.state.player_shots.push(ShotView {
                    x: s.shot_x as i32 + 1,
                    y: s.shot_y as i32,
                    sprite: s.drawn_frame,
                });
            }
        }
        self.state.enemy_shots.clear();
        for i in 0..w.enemy_shots.len() {
            if !w.enemy_shot_avail[i] {
                let s = &w.enemy_shots[i];
                self.state.enemy_shots.push(ShotView {
                    x: s.sx as i32,
                    y: s.sy as i32,
                    sprite: s.sgr.wrapping_add(s.animate),
                });
            }
        }
        self.state.explosions.clear();
        for ex in w.explosions.iter() {
            if ex.ttl != 0 {
                self.state.explosions.push(ExplosionView { x: ex.x as i32, y: ex.y as i32, sprite: ex.sprite + 1 });
            }
        }
    }

    /// `backgrnd.c:draw_background_1/2/3`: row `i = 0` of the drawing starts at
    /// the flat index `mapYPos + mapXbpPos - 12 + width`, at y = `backPos` and
    /// x = `mapXPos` in the surface; the visible area starts at x = 24.
    fn layer_scroll(&self) -> [LayerScroll; 3] {
        let w = &self.world;
        let water = w.background.smoothies[1];
        let params = [
            (
                w.background.map_y_pos[0],
                w.background.map_x_bp_pos,
                w.background.map_x_pos as i32,
                w.background.back_pos[0] as i32,
                14,
            ),
            (
                w.background.map_y_pos[1],
                if water { w.background.map_x_bp_pos } else { w.background.map_x2_bp_pos },
                if water { w.background.map_x_pos as i32 } else { w.background.map_x2_pos as i32 },
                w.background.back_pos[1] as i32,
                14,
            ),
            (
                w.background.map_y_pos[2],
                w.background.map_x3_bp_pos,
                w.background.map_x3_pos as i32,
                w.background.back_pos[2] as i32,
                15,
            ),
        ];
        let mut out = [LayerScroll::default(); 3];
        for (l, (ypos, bp, xpos, back, width)) in params.into_iter().enumerate() {
            // Position at drawing time (backgrnd.c), not after the movement.
            let (ypos, back) = w.render.drawn_layer[l].map_or((ypos, back), |(y, b)| (y, b as i32));
            let flat = ypos + bp - 12 + width;
            let row = flat.div_euclid(width);
            let col = flat.rem_euclid(width);
            out[l] = LayerScroll { x_px: col * 24 - xpos + PLAY_AREA_X, y_px: row * 28 - back };
        }
        out
    }

    /// `golden_master.c:hash_player`.
    fn hash_player(&self) -> u64 {
        let mut h = FNV_INIT;
        for p in self.world.players.iter() {
            let cash = p.cash;
            h = mix(h, p.x as i64);
            h = mix(h, p.y as i64);
            h = mix(h, p.armor as i64);
            h = mix(h, p.shield as i64);
            h = mix(h, p.is_alive as i64);
            h = mix(h, p.invulnerable_ticks as i64);
            h = mix(h, p.exploding_ticks as i64);
            h = mix(h, cash as i64);
            h = mix(h, p.items.weapon[0].power as i64);
            h = mix(h, p.items.weapon[1].power as i64);
        }
        h
    }

    /// `golden_master.c:hash_scroll`.
    fn hash_scroll(&self) -> u64 {
        let w = &self.world;
        let mut h = FNV_INIT;
        for v in [
            w.script.cur_loc as i64,
            w.script.event_loc as i64,
            w.background.map_y[0] as i64,
            w.background.map_y[1] as i64,
            w.background.map_y[2] as i64,
            w.background.back_move[0] as i64,
            w.background.back_move[1] as i64,
            w.background.back_move[2] as i64,
        ] {
            h = mix(h, v);
        }
        h
    }

    /// `golden_master.c:hash_enemies`; also returns the number of enemies.
    fn hash_enemies(&self) -> (u64, u32) {
        let w = &self.world;
        let mut h = FNV_INIT;
        let mut n = 0;
        for i in 0..world::ENEMY_MAX {
            if w.enemy_avail[i] == 1 {
                continue;
            }
            n += 1;
            let e = &w.enemy[i];
            for v in [
                i as i64,
                w.enemy_avail[i] as i64,
                e.ex as i64,
                e.ey as i64,
                e.exc as i64,
                e.eyc as i64,
                e.armorleft as i64,
                e.enemytype as i64,
            ] {
                h = mix(h, v);
            }
        }
        (h, n)
    }

    /// `golden_master.c:hash_player_shots`.
    fn hash_player_shots(&self) -> (u64, u32) {
        let w = &self.world;
        let mut h = FNV_INIT;
        let mut n = 0;
        for i in 0..world::MAX_PWEAPON {
            if w.shot_avail[i] == 0 {
                continue;
            }
            n += 1;
            let s = &w.player_shots[i];
            for v in [i as i64, s.shot_x as i64, s.shot_y as i64, s.shot_gr as i64, s.shot_dmg as i64] {
                h = mix(h, v);
            }
        }
        (h, n)
    }

    /// `golden_master.c:hash_enemy_shots`.
    fn hash_enemy_shots(&self) -> (u64, u32) {
        let w = &self.world;
        let mut h = FNV_INIT;
        let mut n = 0;
        for i in 0..w.enemy_shots.len() {
            if w.enemy_shot_avail[i] {
                continue;
            }
            n += 1;
            let s = &w.enemy_shots[i];
            for v in [i as i64, s.sx as i64, s.sy as i64, s.sgr as i64, s.sdmg as i64] {
                h = mix(h, v);
            }
        }
        (h, n)
    }

    /// `golden_master.c:hash_mode`: state specific to the side modes (format v2).
    pub fn hash_mode(&self) -> u64 {
        let w = &self.world;
        let m = &w.mode;
        let mut h = FNV_INIT;
        for v in [
            w.two_player_mode as i64,
            m.two_player_linked as i64,
            m.link_gun_direc as i64,
            m.galaga_mode as i64,
            m.super_arcade_mode as i64,
            m.super_arcade_power_up as i64,
            w.script.level_timer as i64,
            w.script.level_timer_countdown as i64,
            w.ending.end_level as i64,
            w.ending.level_end as i64,
        ] {
            h = mix(h, v);
        }
        for p in &w.players {
            for v in [
                p.items.weapon[0].id.0 as i64,
                p.items.weapon[1].id.0 as i64,
                p.items.special.0 as i64,
                p.x_velocity as i64,
                p.y_velocity as i64,
                p.purple_balls_needed as i64,
                p.initial_armor as i64,
            ] {
                h = mix(h, v);
            }
        }
        h
    }

    /// Extended fields of a v2 line; the mouse fields (inputs) stay at zero.
    pub fn mode_record(&self) -> modes::ModeRecord {
        let p2 = &self.world.players[1];
        modes::ModeRecord { p2x: p2.x, p2y: p2.y, h_mode: self.hash_mode(), ..Default::default() }
    }

    /// Line comparable to the golden master for the tick that was just played.
    pub fn frame_record(&self) -> FrameRecord {
        let p = &self.world.players[0];
        let (he, ne) = self.hash_enemies();
        let (hps, nps) = self.hash_player_shots();
        let (hes, nes) = self.hash_enemy_shots();
        FrameRecord {
            tick: self.state.tick.saturating_sub(1),
            mask: self.last_mask,
            px: p.x,
            py: p.y,
            armor: p.armor,
            shield: p.shield,
            cash: p.cash,
            cur_loc: self.world.script.cur_loc as u32,
            enemies: ne,
            player_shots: nps,
            enemy_shots: nes,
            h_player: self.hash_player(),
            h_scroll: self.hash_scroll(),
            h_enemies: he,
            h_player_shots: hps,
            h_enemy_shots: hes,
            h_mt: self.world.mt.state_hash(),
            h_rng: self.world.rng.state_hash(),
        }
    }

    /// FNV-1a 64 hash of the full state (replays, tests).
    pub fn state_hash(&self) -> u64 {
        let mut h = FNV_INIT;
        for b in self.snapshot() {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    /// Canonical serialization of the state (quick save, rewind).
    /// Fixed size: does not depend on the content (fixed-size C arrays).
    pub fn snapshot(&self) -> Vec<u8> {
        let w = &self.world;
        let mut out = Vec::with_capacity(16 * 1024);
        let mut put = |v: i64| out.extend_from_slice(&v.to_le_bytes());
        put(self.state.tick as i64);
        for s in w.rng.state() {
            put(s as i64);
        }
        put(w.mt.state_hash() as i64);
        for p in &w.players {
            for v in [
                p.x as i64,
                p.y as i64,
                p.armor as i64,
                p.shield as i64,
                p.is_alive as i64,
                p.invulnerable_ticks as i64,
                p.exploding_ticks as i64,
                p.cash as i64,
                p.items.weapon[0].power as i64,
                p.items.weapon[1].power as i64,
                p.x_velocity as i64,
                p.y_velocity as i64,
                p.weapon_mode as i64,
            ] {
                put(v);
            }
        }
        for v in [
            w.script.cur_loc as i64,
            w.script.event_loc as i64,
            w.background.map_y[0] as i64,
            w.background.map_y[1] as i64,
            w.background.map_y[2] as i64,
            w.background.back_move[0] as i64,
            w.background.back_move[1] as i64,
            w.background.back_move[2] as i64,
            w.background.back_pos[0] as i64,
            w.background.back_pos[1] as i64,
            w.background.back_pos[2] as i64,
            w.energy.power as i64,
            w.ending.level_end as i64,
            w.ending.end_level as i64,
        ] {
            put(v);
        }
        for i in 0..world::ENEMY_MAX {
            let e = &w.enemy[i];
            for v in [
                w.enemy_avail[i] as i64,
                e.ex as i64,
                e.ey as i64,
                e.exc as i64,
                e.eyc as i64,
                e.armorleft as i64,
                e.enemytype as i64,
                e.enemycycle as i64,
            ] {
                put(v);
            }
        }
        for i in 0..world::MAX_PWEAPON {
            let s = &w.player_shots[i];
            for v in [w.shot_avail[i] as i64, s.shot_x as i64, s.shot_y as i64, s.shot_gr as i64, s.shot_dmg as i64] {
                put(v);
            }
        }
        for i in 0..w.enemy_shots.len() {
            let s = &w.enemy_shots[i];
            for v in [w.enemy_shot_avail[i] as i64, s.sx as i64, s.sy as i64, s.sgr as i64, s.sdmg as i64] {
                put(v);
            }
        }
        for ex in &w.explosions {
            for v in [ex.ttl as i64, ex.x as i64, ex.y as i64, ex.sprite as i64] {
                put(v);
            }
        }
        out
    }

    /// Advances without input up to `cur_loc` (editor: "play from here").
    pub fn fast_forward_to(&mut self, cur_loc: u32) {
        let mut guard = 0u32;
        while self.state.cur_loc < cur_loc && !self.state.level_ended && guard < 200_000 {
            self.step([PlayerInput::default(); 2]);
            guard += 1;
        }
    }
}

#[cfg(test)]
mod bonus_game_over_tests {
    use super::*;

    fn loadout() -> Loadout {
        Loadout {
            ship: ShipId(1),
            front_weapon: WeaponId(1),
            front_power: 1,
            rear_weapon: WeaponId(0),
            rear_power: 1,
            shield: ShieldId(4),
            generator: GeneratorId(2),
            left_sidekick: SidekickId(0),
            right_sidekick: SidekickId(0),
            special: SpecialId(0),
            cash: 10000,
        }
    }

    /// Player destroyed then explosion finished; returns (level finished, GAME OVER text seen).
    fn kill_player(bonus: bool) -> Option<(bool, bool)> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        if !dir.join("tyrian1.lvl").exists() {
            return None;
        }
        let mut sim = Sim::new(setup::load_level(&dir, 1, 2, 9, 2, loadout(), 7).unwrap());
        sim.set_bonus_level_current(bonus);
        for _ in 0..20 {
            sim.step([PlayerInput(0), PlayerInput(0)]);
        }
        sim.world.players[0].is_alive = false;
        sim.world.players[0].exploding_ticks = 0;
        let mut game_over_seen = false;
        for _ in 0..60 {
            sim.step([PlayerInput(0), PlayerInput(0)]);
            game_over_seen |= sim.world.render.drawn_texts.game_over;
        }
        Some((sim.state.level_ended, game_over_seen))
    }

    /// player.h:all_players_alive: in two-player, both must be alive.
    #[test]
    fn two_player_all_alive_needs_both_players() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        if !dir.join("tyrian1.lvl").exists() {
            return;
        }
        let mut sim = Sim::new(setup::load_level(&dir, 1, 2, 9, 2, loadout(), 7).unwrap());
        sim.world.two_player_mode = true;
        assert!(sim.world.all_players_alive());
        // Player 2 dead, player 1 alive: player.h no longer counts "all alive"
        // (debrief "Exiting" title, ]l jump) but the game goes on.
        sim.world.players[1].is_alive = false;
        assert!(!sim.world.all_players_alive());
        assert!(!sim.world.all_players_dead());
        sim.world.two_player_mode = false;
        assert!(sim.world.all_players_alive(), "one player: only player 1 counts");
    }

    /// tyrian2.c:1481: a "$" level ends at death, with no GAME OVER text or wait for a key.
    #[test]
    fn bonus_level_ends_without_game_over_screen() {
        if let Some((ended, game_over)) = kill_player(true) {
            assert!(ended, "the bonus level must end without waiting for a key");
            assert!(!game_over, "no GAME OVER text in a bonus level");
        }
    }

    /// Ordinary level: GAME OVER stays displayed and waits for a key.
    #[test]
    fn normal_level_waits_on_game_over_screen() {
        if let Some((ended, game_over)) = kill_player(false) {
            assert!(!ended, "a regular level waits for a key");
            assert!(game_over, "GAME OVER text shown");
        }
    }
}
