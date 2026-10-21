//! GDExtension: bridge between Godot 4 and the `tyrian-sim` simulation core.
//!
//! Godot reads the simulation state on every frame and owns none of it
//! (docs/01-architecture.md section 3). The `TyrianSim` node only relays
//! inputs to `Sim::step` and flattens `SimState` into Godot types.
//!
//! Flattened arrays returned (PackedInt32Array):
//! - `enemies()`: stride of 8 -- slot, x, y, sheet (ASCII byte of the `newsh?`
//!   sheet), frame (1-based), size, layer, armor;
//! - `player_shots()`, `enemy_shots()`, `explosions()`: stride of 3 -- x, y, sprite;
//! - `player(idx)`: stride of 8 -- x, y, armor, shield, alive, invulnerable_ticks,
//!   banking, cash (clamped to i32).

mod ach_api;
mod audio_fx;
mod bench;
mod campaign;
mod destruct_api;
mod dev_api;
mod dodge;
mod endless;
mod fx_api;
pub(crate) mod fx_render;
mod hd_ui;
mod hud;
mod midi;
mod modern_explosions;
mod modes_api;
pub(crate) mod playfield;
mod quit_box;
mod rec_api;
mod render;
mod rng_api;
mod shop;
mod starlib;
mod story_api;

use std::cell::RefCell;
use std::path::Path;
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

thread_local! {
    /// `mtrand.c` outside the golden master: the generator that JE_main uses for
    /// initialize_starfield is not reseeded between levels; it starts from
    /// the default seed (5489) at launch.
    pub(crate) static STARFIELD_MT: RefCell<tyrian_sim::rng::MersenneTwister> = RefCell::new(tyrian_sim::rng::MersenneTwister::new(5489));

    /// Current `config.c:processorType` (Setup > Graphics > Visual FX), for
    /// screens that have no `TyrianSim` at hand (shop,
    /// title): their starfield follows the effects level.
    pub(crate) static PROCESSOR_TYPE: std::cell::Cell<u8> = const { std::cell::Cell::new(4) };
}

use godot::prelude::*;

mod mods;
pub use mods::TyrianMods;
use tyrian_sim::{Loadout, MusicCue, PlayerInput, Sim, SimState};

mod vga;

struct TyrianExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TyrianExtension {}

const ENEMY_STRIDE: i32 = 8;
const SHOT_STRIDE: i32 = 3;
const PLAYER_STRIDE: i32 = 8;

/// Starting equipment of a new game (mainint.c:JE_initPlayerData).
fn default_loadout() -> Loadout {
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
        cash: 0,
    }
}

#[derive(GodotClass)]
#[class(base = Node)]
pub struct TyrianSim {
    sim: Option<Sim>,
    loadout: Loadout,
    assist: bool,
    /// "$" level at the next `load_level` (tyrian2.c:1481).
    bonus_level_current: bool,
    /// Campaign: applied at the next `load_level`.
    campaign_initial_difficulty: Option<u8>,
    campaign_carry: Option<tyrian_sim::LevelCarry>,
    modes: tyrian_sim::modes::ModeSetup,
    /// Endless armor multiplier at the next `load_level` (endless module).
    endless_armor_mult: Option<tyrian_sim::fmath::F32>,
    /// Endless lives (`endless.c:s_lives_p`) at the next `load_level`.
    endless_lives: Option<[i32; 2]>,
    /// State carried over from the previous Endless wave (`set_endless_carry`).
    endless_carry: Option<tyrian_sim::LevelCarry>,
    /// Player 2 equipment carried over from one Endless wave to the next (ENH-03).
    endless_loadout2: Option<Loadout>,
    /// Two-player Endless (`endless.c:endless_start(true)`).
    endless_two_player: bool,
    /// `initialDifficulty` of the Endless run (`set_endless_initial_difficulty`).
    endless_initial_difficulty: Option<u8>,
    /// Anomaly mask of the Endless deck (`anomaly_deck_reapply`).
    endless_anomalies: u16,
    /// Active combos (`combos.c`, `combo_enabled`): duration of a loop in ms.
    combo_frame_ms: Option<u32>,
    /// `config.c:processorType` (Setup > Graphics > Visual FX; 1 = Classic,
    /// 3 = High, 4 = Wild). Applied at each `load_level`, reread by the
    /// simulation and the rendering. Default 4 (config.c:JE_loadConfiguration).
    processor_type: u8,
    last_error: GString,
    /// Playback of a `.rep` from the C version in progress (`load_replay`).
    replay: Option<tyrian_sim::replay::ReplayCursor>,
    /// Recording of a `.rep` in progress (rec_api module) and request set
    /// for the next `load_level`: (section, header seed, speed).
    recorder: Option<tyrian_sim::replay::ReplayRecorder>,
    record_request: Option<(u8, u64, u8)>,
    /// Mods hooked by `set_mods` (ADR 0080); None = original game.
    mods: Option<Gd<TyrianMods>>,
    /// Player's Enhanced effects (`set_effects`, fx_api module); false for
    /// the tools, which keep the golden master settings.
    fx_on: bool,
    fx_cfg: tyrian_sim::fx::FxConfig,
    fx_opts: fx_render::FxOptions,
    base: Base<Node>,
}

#[godot_api]
impl INode for TyrianSim {
    fn init(base: Base<Node>) -> Self {
        Self {
            sim: None,
            loadout: default_loadout(),
            assist: false,
            bonus_level_current: false,
            campaign_initial_difficulty: None,
            endless_carry: None,
            campaign_carry: None,
            modes: Default::default(),
            endless_armor_mult: None,
            endless_lives: None,
            endless_two_player: false,
            endless_initial_difficulty: None,
            endless_loadout2: None,
            endless_anomalies: 0,
            combo_frame_ms: None,
            processor_type: 4,
            last_error: GString::new(),
            replay: None,
            recorder: None,
            record_request: None,
            mods: None,
            fx_on: false,
            fx_cfg: Default::default(),
            fx_opts: Default::default(),
            base,
        }
    }
}

#[godot_api]
impl TyrianSim {
    #[constant]
    const ENEMY_STRIDE: i32 = ENEMY_STRIDE;
    #[constant]
    const SHOT_STRIDE: i32 = SHOT_STRIDE;
    #[constant]
    const PLAYER_STRIDE: i32 = PLAYER_STRIDE;

    /// Loads level `lvl_file_num` (1-based) of `tyrian<episode>.lvl`
    /// from the original `data/` folder. Returns false on error
    /// (message in `last_error()`).
    #[func]
    fn load_level(&mut self, data_dir: GString, episode: i32, lvl_file_num: i32, difficulty: i32, seed: i64) -> bool {
        let dir = data_dir.to_string();
        // Recording request consumed even if the load fails.
        let record_request = self.record_request.take();
        let result = tyrian_sim::setup::load_level(
            Path::new(&dir),
            episode.clamp(1, 5) as u8,
            0,
            lvl_file_num.max(1) as usize,
            difficulty.clamp(0, 255) as u8,
            self.loadout,
            seed as u64,
        );
        match result {
            Ok(mut setup) => {
                setup.assist = self.assist;
                // config.c:processorType: effects detail level, read by
                // the simulation (hitstop, stars, layer 2) at startup.
                setup.processor_type = self.processor_type;
                if self.campaign_initial_difficulty.is_some() {
                    // Game in progress: player[1] keeps the equipment of
                    // JE_initPlayerData, initialDifficulty stays the menu choice.
                    setup.initial_difficulty = self.campaign_initial_difficulty;
                    setup.second_player_loadout = Some(default_loadout());
                    setup.carry_over = self.campaign_carry;
                } else if self.endless_lives.is_some() {
                    // Next Endless wave: player[] and fireButtonHeld are not
                    // reset by the C between two waves (tyrian2.c, goto start_level_first).
                    setup.carry_over = self.endless_carry;
                }
                setup.modes = self.modes;
                setup.endless_armor_mult = self.endless_armor_mult;
                if let Some(l) = self.endless_lives {
                    // endless.c:endless_start: twoPlayerMode = two_player,
                    // onePlayerAction = !two_player, player 2 items;
                    // deck anomalies (anomaly_deck_reapply), menu mutators.
                    let mutators = setup.modes.mutators;
                    setup.modes = tyrian_sim::endless::mode_setup(self.endless_two_player, l);
                    if let Some(d) = self.endless_initial_difficulty {
                        setup.modes.initial_difficulty = d;
                    }
                    setup.modes.mutators = mutators;
                    setup.modes.anomalies = self.endless_anomalies;
                    if let (true, Some(lo2)) = (self.endless_two_player, self.endless_loadout2) {
                        setup.modes.loadout2 = Some(lo2);
                        setup.modes.cash.1 = lo2.cash as u64;
                    }
                }
                if let Some(m) = self.mods.as_mut() {
                    m.bind_mut().apply_to(&mut setup, lvl_file_num.max(1) as u16);
                }
                self.recorder = None;
                let mut sim = match record_request {
                    // Recording requested (replay.c:replay_level_start_hook):
                    // generators and stars come from the file's seed.
                    Some((section, rec_seed, speed)) => {
                        match tyrian_sim::replay::ReplayRecorder::begin(
                            Path::new(&dir),
                            setup.clone(),
                            section,
                            lvl_file_num.max(1) as usize,
                            rec_seed,
                            speed,
                        ) {
                            Ok((rec, sim)) => {
                                self.recorder = Some(rec);
                                sim
                            }
                            Err(e) => {
                                godot_warn!("TyrianSim.load_level: recording refused: {e}");
                                self.last_error = GString::from(e.as_str());
                                let mut sim = Sim::new(setup);
                                STARFIELD_MT.with(|mt| {
                                    sim.set_starfield(tyrian_sim::world::initialize_starfield(
                                        self.processor_type,
                                        &mut mt.borrow_mut(),
                                    ))
                                });
                                sim
                            }
                        }
                    }
                    None => {
                        let mut sim = Sim::new(setup);
                        STARFIELD_MT.with(|mt| {
                            sim.set_starfield(tyrian_sim::world::initialize_starfield(
                                self.processor_type,
                                &mut mt.borrow_mut(),
                            ))
                        });
                        sim
                    }
                };
                sim.set_bonus_level_current(self.bonus_level_current);
                if let Some(ms) = self.combo_frame_ms {
                    sim.enable_combos(ms);
                }
                // Rendering at the state of the frame presented by the C (end of loop).
                sim.set_keep_presented(true);
                self.sim = Some(sim);
                self.replay = None;
                if self.recorder.is_none() {
                    self.last_error = GString::new();
                }
                self.fx_level_start(seed as u64, false);
                true
            }
            Err(e) => {
                godot_error!("TyrianSim.load_level: {e}");
                self.last_error = GString::from(e.as_str());
                self.sim = None;
                false
            }
        }
    }

    /// Equipment used at the next `load_level`: ship, front_weapon,
    /// front_power, rear_weapon, rear_power, shield, generator,
    /// left_sidekick, right_sidekick, special, cash (11 values).
    #[func]
    fn set_loadout(&mut self, values: PackedInt32Array) -> bool {
        let v = values.as_slice();
        if v.len() < 11 {
            godot_error!("TyrianSim.set_loadout: 11 values expected, {} received", v.len());
            return false;
        }
        let b = |i: usize| v[i].clamp(0, 255) as u8;
        self.loadout = Loadout {
            ship: ShipId(b(0)),
            front_weapon: WeaponId(b(1)),
            front_power: b(2),
            rear_weapon: WeaponId(b(3)),
            rear_power: b(4),
            shield: ShieldId(b(5)),
            generator: GeneratorId(b(6)),
            left_sidekick: SidekickId(b(7)),
            right_sidekick: SidekickId(b(8)),
            special: SpecialId(b(9)),
            cash: v[10].max(0) as u32,
        };
        true
    }

    /// Current equipment, same order as `set_loadout`.
    #[func]
    fn loadout(&self) -> PackedInt32Array {
        let l = &self.loadout;
        PackedInt32Array::from(vec![
            l.ship.0 as i32,
            l.front_weapon.0 as i32,
            l.front_power as i32,
            l.rear_weapon.0 as i32,
            l.rear_power as i32,
            l.shield.0 as i32,
            l.generator.0 as i32,
            l.left_sidekick.0 as i32,
            l.right_sidekick.0 as i32,
            l.special.0 as i32,
            l.cash as i32,
        ])
    }

    /// Game mode at the next `load_level` (block 5, `tyrian_sim::modes::ModeSetup`):
    /// two_player, galaga, one_player_action, super_arcade, super_tyrian, timed,
    /// mutators, anomalies, device_p1, device_p2 (10 values; device
    /// 1 keyboard, 2 mouse).
    #[func]
    fn set_modes(&mut self, values: PackedInt32Array) -> bool {
        let v = values.as_slice();
        if v.len() < 10 {
            godot_error!("TyrianSim.set_modes: 10 values expected, {} received", v.len());
            return false;
        }
        self.modes = tyrian_sim::modes::ModeSetup {
            two_player: v[0] != 0,
            galaga: v[1] != 0,
            one_player_action: v[2] != 0,
            super_arcade: v[3].clamp(0, 255) as u8,
            super_tyrian: v[4] != 0,
            timed: v[5].clamp(0, 255) as u8,
            mutators: v[6].clamp(0, 255) as u8,
            anomalies: v[7].clamp(0, 0x1ff) as u16,
            devices: (v[8].clamp(0, 255) as u8, v[9].clamp(0, 255) as u8),
            ..Default::default()
        };
        true
    }

    /// Mods applied to the next `load_level` calls (null = none).
    #[func]
    fn set_mods(&mut self, mods: Option<Gd<TyrianMods>>) {
        self.mods = mods;
    }

    /// "$" campaign level at the next `load_level`: the death of all
    /// players ends it without a GAME OVER screen (tyrian2.c:1481).
    #[func]
    fn set_bonus_level_current(&mut self, bonus: bool) {
        self.bonus_level_current = bonus;
    }

    /// Autopilot assists at the next `load_level` (replay of a golden
    /// master recorded with "assist 1", see tyrian-sim: LevelSetup::assist).
    #[func]
    fn set_assist(&mut self, enabled: bool) {
        self.assist = enabled;
    }

    /// Endless mode at the next `load_level`: IEEE 754 bits of the armor
    /// multiplier (`TyrianEndless.armor_mult_bits`), or -1 outside Endless.
    #[func]
    fn set_endless_armor_mult(&mut self, bits: i64) {
        self.endless_armor_mult = if bits < 0 {
            None
        } else {
            let b = bits as u32;
            Some(tyrian_sim::fmath::F32 {
                neg: false,
                mant: ((b & 0x7f_ffff) | 0x80_0000) as u64,
                exp: ((b >> 23) & 0xff) as i32 - 127 - 23,
            })
        };
    }

    /// Endless mode: starting lives of both players at the next
    /// `load_level` (`endless.c:s_lives_p`), `lives0 < 0` outside Endless.
    #[func]
    fn set_endless_lives(&mut self, lives0: i32, lives1: i32) {
        self.endless_lives = if lives0 < 0 { None } else { Some([lives0, lives1]) };
    }

    /// Endless: state carried over from the previous wave (`campaign_carry` read
    /// at its end), empty for the first wave of a game.
    #[func]
    fn set_endless_carry(&mut self, carry: PackedInt32Array) {
        self.endless_carry = tyrian_sim::LevelCarry::from_values(carry.as_slice());
    }

    /// Two-player Endless: player 2 equipment taken from the previous wave
    /// (11 values, order of `set_loadout`), or empty for the starting equipment.
    #[func]
    fn set_endless_loadout2(&mut self, values: PackedInt32Array) {
        let v = values.as_slice();
        self.endless_loadout2 = (v.len() >= 11).then(|| {
            let b = |i: usize| v[i].clamp(0, 255) as u8;
            Loadout {
                ship: ShipId(b(0)),
                front_weapon: WeaponId(b(1)),
                front_power: b(2),
                rear_weapon: WeaponId(b(3)),
                rear_power: b(4),
                shield: ShieldId(b(5)),
                generator: GeneratorId(b(6)),
                left_sidekick: SidekickId(b(7)),
                right_sidekick: SidekickId(b(8)),
                special: SpecialId(b(9)),
                cash: v[10].max(0) as u32,
            }
        });
    }

    /// Endless at the next `load_level`: two players (`endless_start(true)`)
    /// and anomaly mask of the deck (`TyrianEndless.anomaly_effects`).
    #[func]
    fn set_endless_options(&mut self, two_player: bool, anomalies: i32) {
        self.endless_two_player = two_player;
        self.endless_anomalies = anomalies.clamp(0, 0x1ff) as u16;
    }

    /// Endless: `initialDifficulty` frozen at the start of the run (`endless.c:800-801`).
    #[func]
    fn set_endless_initial_difficulty(&mut self, difficulty: i32) {
        self.endless_initial_difficulty = Some(difficulty.clamp(1, 9) as u8);
    }

    /// Combos (`combos.c`) at the next `load_level`: `frame_ms` > 0 = active,
    /// window counted down by `frame_ms` at each tick (`combo_update`);
    /// 0 = off (`combo_enabled = false`).
    #[func]
    fn set_combos(&mut self, frame_ms: i32) {
        self.combo_frame_ms = if frame_ms > 0 { Some(frame_ms as u32) } else { None };
    }

    /// Sets `config.c:processorType` (1..6; 1 = Classic, 3 = High,
    /// 4 = Wild by default). Reproduces `JE_initProcessorType()`: re-derives
    /// the state (cascade of `hitstop.c`, stars of `backgrnd.c`) and applies it
    /// to the loaded level. Applied automatically at the next `load_level`.
    #[func]
    fn set_processor_type(&mut self, value: i32) {
        let v = value.clamp(1, 6) as u8;
        self.processor_type = v;
        PROCESSOR_TYPE.with(|p| p.set(v));
        self.fx_opts.processor_type = v as i32;
        crate::fx_render::FX.with(|f| f.borrow_mut().options.processor_type = v as i32);
        if let Some(sim) = self.sim.as_mut() {
            sim.set_processor_type(v);
            STARFIELD_MT.with(|mt| sim.set_starfield(tyrian_sim::world::initialize_starfield(v, &mut mt.borrow_mut())));
        }
    }

    /// Current effects detail level (`config.c:processorType`).
    #[func]
    fn processor_type(&self) -> i32 {
        self.processor_type as i32
    }

    /// Combo in progress: [count, multiplier, tier reached this tick
    /// (index of `combos.c:combo_tiers`, -1 otherwise)]. The tier is consumed.
    #[func]
    fn combo_state(&mut self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_mut() else {
            return PackedInt32Array::new();
        };
        let (count, mult, tier) = sim.take_combo();
        PackedInt32Array::from(vec![count as i32, mult as i32, tier.map_or(-1, |t| t as i32)])
    }

    /// `combos.c:combo_multiplier` (lecture seule, HUD Endless).
    #[func]
    fn combo_multiplier(&self) -> i32 {
        self.sim.as_ref().map_or(1, |s| s.world().combo_multiplier as i32)
    }

    /// Endless state of the current wave: [P1 lives, P2 lives, wave score]
    /// (`endless.c:on_enemy_killed`), empty outside Endless.
    #[func]
    fn endless_state(&self) -> PackedInt64Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt64Array::new();
        };
        let w = sim.world();
        match w.endless_lives {
            Some(l) => {
                PackedInt64Array::from(vec![l[0] as i64, l[1] as i64, w.endless_score.min(i64::MAX as u64) as i64])
            }
            None => PackedInt64Array::new(),
        }
    }

    #[func]
    fn is_loaded(&self) -> bool {
        self.sim.is_some()
    }

    #[func]
    fn last_error(&self) -> GString {
        self.last_error.clone()
    }

    /// Advances one tick with the input masks of both players
    /// (bits: 0 up, 1 down, 2 left, 3 right, 4 fire, 5 fire switch,
    /// 6 left sidekick, 7 right sidekick).
    #[func]
    fn step(&mut self, p1_mask: i32, p2_mask: i32) {
        if let (Some(cur), Some(sim)) = (self.replay.as_mut(), self.sim.as_mut()) {
            // Playback: the inputs come from the file (replay_inject_frame).
            cur.step(sim);
            self.fx_after_step();
            return;
        }
        if let Some(sim) = self.sim.as_mut() {
            // replay.c:replay_capture_frame, at the head of the iteration: positions
            // and buttons of player 1 (keyboard and mouse) before the simulation.
            if let Some(rec) = self.recorder.as_mut() {
                if !sim.state().level_ended {
                    rec.capture(sim, (p1_mask & 0xff) as u8, sim.world().mode.mouse.mask);
                }
            }
            sim.step([PlayerInput((p1_mask & 0xff) as u8), PlayerInput((p2_mask & 0xff) as u8)]);
        }
        self.fx_after_step();
    }

    /// Advances with no input up to scroll position `cur_loc`.
    #[func]
    fn fast_forward_to(&mut self, cur_loc: i64) {
        if let Some(sim) = self.sim.as_mut() {
            sim.fast_forward_to(cur_loc.clamp(0, u32::MAX as i64) as u32);
            // Effects of skipped ticks: never displayed.
            sim.take_fx_events();
        }
    }

    #[func]
    fn tick(&self) -> i64 {
        self.sim.as_ref().map_or(0, |s| s.state().tick as i64)
    }

    #[func]
    fn cur_loc(&self) -> i64 {
        self.sim.as_ref().map_or(0, |s| s.state().cur_loc as i64)
    }

    #[func]
    fn level_ended(&self) -> bool {
        if let (Some(cur), Some(sim)) = (self.replay.as_ref(), self.sim.as_ref()) {
            // tyrian2.c: an exhausted replay leaves the loop like a level end.
            return cur.finished(sim);
        }
        self.sim.as_ref().is_some_and(|s| s.state().level_ended)
    }

    /// Loads a `.rep` replay from the C version (`src/replay.c`, ADR 0090):
    /// level, equipment and seeds come from the file; each `step`
    /// then replays one frame (the masks passed are ignored).
    #[func]
    fn load_replay(&mut self, data_dir: GString, path: GString) -> bool {
        let dir = data_dir.to_string();
        let result = tyrian_sim::replay::load_file(Path::new(&path.to_string())).and_then(|rep| {
            // replay.c:JE_initProcessorType: the current setting decides the draws of the
            // starfield, as at recording.
            let initial_difficulty = rep.difficulty;
            tyrian_sim::replay::ReplayCursor::start_with_options(
                Path::new(&dir),
                rep,
                initial_difficulty,
                self.processor_type,
            )
        });
        match result {
            Ok((cur, mut sim)) => {
                sim.set_keep_presented(true);
                self.sim = Some(sim);
                self.recorder = None;
                self.replay = Some(cur);
                self.last_error = GString::new();
                self.fx_level_start(1, true);
                true
            }
            Err(e) => {
                godot_error!("TyrianSim.load_replay: {e}");
                self.last_error = GString::from(e.as_str());
                self.sim = None;
                self.replay = None;
                false
            }
        }
    }

    /// Title demo (`level_load.c` with `play_demo`,
    /// `mainint.c:load_next_demo`): reads `data/demo.N` and loads its level
    /// with the header's equipment, at Normal difficulty; the keys of
    /// player 1 then come from the file at each `step`
    /// (`replay_demo_keys`). `cash`: money of `player[0]` left by the
    /// previous game (the demo does not touch it). Returns a dictionary
    /// `episode`, `lvl_file_num`, `level_name`, `level_song`, `loadout` (11
    /// values, order of `set_loadout`), or empty if the file is unreadable
    /// (`load_next_demo` returns false: `last_error`).
    #[func]
    fn load_demo(&mut self, data_dir: GString, demo_num: i32, seed: i64, cash: i64) -> VarDictionary {
        let mut d = VarDictionary::new();
        let dir = data_dir.to_string();
        let path = Path::new(&dir).join(format!("demo.{demo_num}"));
        let demo = std::fs::read(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|b| tyrian_formats::demo::Demo::parse(&b).map_err(|e| format!("{}: {e}", path.display())));
        let ctx = tyrian_sim::demo::DemoContext {
            cash: cash.clamp(0, u32::MAX as i64) as u32,
            initial_difficulty: self.campaign_initial_difficulty.unwrap_or(0),
            carry: self.campaign_carry,
        };
        let result = demo.and_then(|demo| {
            tyrian_sim::demo::start_demo(Path::new(&dir), &demo, seed as u64, ctx).map(|sim| (demo, sim))
        });
        match result {
            Ok((demo, mut sim)) => {
                STARFIELD_MT.with(|mt| {
                    sim.set_starfield(tyrian_sim::world::initialize_starfield(
                        self.processor_type,
                        &mut mt.borrow_mut(),
                    ))
                });
                sim.set_keep_presented(true);
                let l = tyrian_sim::demo::demo_loadout(&demo, ctx.cash);
                let values = [
                    l.ship.0 as i32,
                    l.front_weapon.0 as i32,
                    l.front_power as i32,
                    l.rear_weapon.0 as i32,
                    l.rear_power as i32,
                    l.shield.0 as i32,
                    l.generator.0 as i32,
                    l.left_sidekick.0 as i32,
                    l.right_sidekick.0 as i32,
                    l.special.0 as i32,
                    l.cash.min(i32::MAX as u32) as i32,
                ];
                d.set("episode", demo.header.episode.clamp(1, 5) as i32);
                d.set("lvl_file_num", demo.header.lvl_file_num as i32);
                d.set("level_name", tyrian_sim::demo::demo_level_name(&demo).as_str());
                d.set("level_song", demo.header.level_song as i32);
                d.set("loadout", &PackedInt32Array::from(&values[..]));
                self.sim = Some(sim);
                self.replay = None;
                self.last_error = GString::new();
                // Demo: damage numbers and particles as in a level, with the
                // golden master settings for the simulation (like a replay).
                self.fx_level_start(1, true);
            }
            Err(e) => {
                godot_error!("TyrianSim.load_demo: {e}");
                self.last_error = GString::from(e.as_str());
                self.sim = None;
            }
        }
        d
    }

    /// Demo in progress: true as long as the demo level runs with
    /// the keys of `demo.N` (false outside a demo).
    #[func]
    fn is_demo(&self) -> bool {
        self.sim.as_ref().is_some_and(|s| s.demo().is_some())
    }

    /// Loaded replay: episode, section, difficulty, frames, level `lvlfile`.
    #[func]
    fn replay_info(&self) -> PackedInt32Array {
        let Some(cur) = self.replay.as_ref() else {
            return PackedInt32Array::new();
        };
        let r = &cur.replay;
        PackedInt32Array::from(vec![
            r.episode as i32,
            r.level as i32,
            r.difficulty as i32,
            r.frames.len() as i32,
            cur.lvl_file as i32,
        ])
    }

    /// Music orders emitted since the last call, flattened in pairs
    /// (kind, value): kind 0 = play_song(value, 0-based), 1 = set_volume(value 0..255).
    #[func]
    fn take_music_cues(&mut self) -> PackedInt32Array {
        let mut out = PackedInt32Array::new();
        if let Some(sim) = self.sim.as_mut() {
            for cue in sim.take_music_cues() {
                let (kind, value) = match cue {
                    MusicCue::PlaySong(n) => (0, n),
                    MusicCue::SetVolume(v) => (1, v as i32),
                };
                out.push(kind);
                out.push(value);
            }
        }
        out
    }

    /// Sound effects output by `loudness.c:JE_flushSoundQueue` since the
    /// last call, flattened in fours (channel, 1-based sample,
    /// `soundQueueX` as screen x 0..320, `soundQueueDmg` 0..3). Rendering only:
    /// does not touch the hashed state.
    #[func]
    fn take_sounds(&mut self) -> PackedInt32Array {
        let mut out = PackedInt32Array::new();
        if let Some(sim) = self.sim.as_mut() {
            for e in sim.take_sounds() {
                out.push(e.channel as i32);
                out.push(e.sample as i32);
                out.push(e.x as i32);
                out.push(e.dmg as i32);
            }
        }
        out
    }

    /// `firstGameOver` (tyrian2.c): true as long as the game over screen has not been reached.
    #[func]
    fn first_game_over(&self) -> bool {
        self.sim.as_ref().is_some_and(|s| s.first_game_over())
    }

    /// See `Sim::music_level_song_branch` (tyrian2.c, MUSIC FADE block).
    #[func]
    fn music_level_song_branch(&self) -> bool {
        self.sim.as_ref().is_some_and(|s| s.music_level_song_branch())
    }

    /// `tyrMusicVolume` (0..255, 191 by default).
    #[func]
    fn set_music_volume(&mut self, volume: i32) {
        if let Some(sim) = self.sim.as_mut() {
            sim.set_music_volume(volume.clamp(0, 255) as u16);
        }
    }

    /// State of player `idx` (0 or 1), stride `PLAYER_STRIDE` (see the top of the file).
    #[func]
    fn player(&self, idx: i32) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let p = &sim.state().players[idx.clamp(0, 1) as usize];
        PackedInt32Array::from(vec![
            p.x,
            p.y,
            p.armor,
            p.shield,
            p.alive as i32,
            p.invulnerable_ticks,
            p.banking,
            p.cash.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        ])
    }

    /// Same content as `player(idx)`, as a dictionary (HUD, tools).
    #[func]
    fn player_dict(&self, idx: i32) -> VarDictionary {
        let Some(sim) = self.sim.as_ref() else {
            return VarDictionary::new();
        };
        let p = &sim.state().players[idx.clamp(0, 1) as usize];
        vdict! {
            "x" => p.x,
            "y" => p.y,
            "armor" => p.armor,
            "shield" => p.shield,
            "alive" => p.alive,
            "invulnerable_ticks" => p.invulnerable_ticks,
            "banking" => p.banking,
            "cash" => p.cash,
        }
    }

    /// Campaign: `initialDifficulty` and state inherited from the previous level
    /// (`carry` = [x_friction p1, y_friction p1, x_friction p2, y_friction p2,
    /// special fire held], `LevelCarry::to_values`,
    /// empty for the first level of a game), taken into account at the next
    /// `load_level`. `initial_difficulty < 0`: isolated level (`--goto`).
    #[func]
    fn set_campaign_state(&mut self, initial_difficulty: i32, carry: PackedInt32Array) {
        self.campaign_initial_difficulty = (initial_difficulty >= 0).then_some(initial_difficulty.min(10) as u8);
        let v = carry.as_slice();
        self.campaign_carry = tyrian_sim::LevelCarry::from_values(v);
    }

    /// State to carry over to the next level (same order as `set_campaign_state`).
    #[func]
    fn campaign_carry(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        PackedInt32Array::from(&tyrian_sim::LevelCarry::from_sim(sim).to_values()[..])
    }

    /// Level summary for the debriefing (`mainint.c:JE_endLevelAni`) and the
    /// rest of the campaign: enemies killed / total, cubes, money, total score
    /// (`JE_totalScore`), players alive.
    #[func]
    fn level_stats(&self) -> VarDictionary {
        let Some(sim) = self.sim.as_ref() else {
            return VarDictionary::new();
        };
        let w = sim.world();
        let p0 = &w.players[0];
        let score = tyrian_sim::campaign::total_score(&w.tables, &p0.items, p0.cash);
        let loadout = PackedInt32Array::from(
            &[
                p0.items.ship.0 as i32,
                p0.items.weapon[0].id.0 as i32,
                p0.items.weapon[0].power as i32,
                p0.items.weapon[1].id.0 as i32,
                p0.items.weapon[1].power as i32,
                p0.items.shield.0 as i32,
                p0.items.generator.0 as i32,
                p0.items.sidekick[0].0 as i32,
                p0.items.sidekick[1].0 as i32,
                p0.items.special.0 as i32,
                p0.cash.min(i32::MAX as u64) as i32,
            ][..],
        );
        let mut d = vdict! {
            "enemy_killed" => w.enemy_killed as i32,
            "total_enemy" => w.total_enemy as i32,
            "cube_max" => w.ending.cube_max as i32,
            "cash" => p0.cash as i64,
            "total_score" => score as i64,
            // player.h:all_players_alive: with two players, both are required
            // ("Exiting" title of the summary, jump ]l of the script); any_player_alive
            // = !all_players_dead (tyrian2.c:285, rest of the campaign).
            "all_players_alive" => w.all_players_alive(),
            "any_player_alive" => !w.all_players_dead(),
            // player_collide.c:307-319: warp picked up, mainLevel = nextLevel
            // (section of the secret level) at level end (tyrian2.c:287).
            "bonus_level" => w.ending.bonus_level,
            "next_level" => w.ending.next_level as i32,
            "ship" => p0.items.ship.0 as i32,
            // Timed Battle (mainint.c:JE_endLevelAni): `levelTimerCountdown` and remaining lives.
            "timer_countdown" => w.script.level_timer_countdown as i32,
            // mainint.c:2199: episode 4 finished by the timer (levelTimer, counts to 0) = play_song(21).
            "level_timer" => w.script.level_timer,
            "lives" => p0.items.weapon[0].power as i32,
        };
        // End-of-level equipment, including power gained in play
        // (order of set_loadout): starting point of the shop.
        d.set("loadout", &loadout);
        // Player 2 (two-player Endless: equipment carried over from wave to wave).
        let p1 = &w.players[1];
        let loadout2 = PackedInt32Array::from(
            &[
                p1.items.ship.0 as i32,
                p1.items.weapon[0].id.0 as i32,
                p1.items.weapon[0].power as i32,
                p1.items.weapon[1].id.0 as i32,
                p1.items.weapon[1].power as i32,
                p1.items.shield.0 as i32,
                p1.items.generator.0 as i32,
                p1.items.sidekick[0].0 as i32,
                p1.items.sidekick[1].0 as i32,
                p1.items.special.0 as i32,
                p1.cash.min(i32::MAX as u64) as i32,
            ][..],
        );
        d.set("loadout2", &loadout2);
        d.set("armor", &PackedInt32Array::from(&[p0.armor as i32, p1.armor as i32][..]));
        // stats.c:level_stats: summary screen (mainint.c:JE_endLevelAni).
        let ls = &w.level_stats;
        d.set("damage_by_bay", &PackedInt32Array::from(&ls.damage_by_bay.map(|v| v.min(i32::MAX as u32) as i32)[..]));
        d.set("damage_taken", ls.damage_taken as i64);
        d.set("damage_blocked", ls.damage_blocked as i64);
        d.set("credits_start", ls.credits_start as i64);
        d
    }

    /// Offset of layer `idx` (0..2): map pixel displayed at the top-left
    /// corner of the play area.
    #[func]
    fn layer_scroll(&self, idx: i32) -> Vector2i {
        let Some(sim) = self.sim.as_ref() else {
            return Vector2i::ZERO;
        };
        let l = &sim.presented_state().layers[idx.clamp(0, 2) as usize];
        Vector2i::new(l.x_px, l.y_px)
    }

    #[func]
    fn enemies(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let list = &sim.presented_state().enemies;
        let mut out = Vec::with_capacity(list.len() * ENEMY_STRIDE as usize);
        for e in list {
            out.extend_from_slice(&[
                e.slot as i32,
                e.x,
                e.y,
                e.sheet as i32,
                e.frame as i32,
                e.size as i32,
                e.layer as i32,
                e.armor,
            ]);
        }
        PackedInt32Array::from(out)
    }

    #[func]
    fn player_shots(&self) -> PackedInt32Array {
        self.flatten3(|s| s.player_shots.iter().map(|v| [v.x, v.y, v.sprite as i32]).collect())
    }

    #[func]
    fn enemy_shots(&self) -> PackedInt32Array {
        self.flatten3(|s| s.enemy_shots.iter().map(|v| [v.x, v.y, v.sprite as i32]).collect())
    }

    #[func]
    fn explosions(&self) -> PackedInt32Array {
        self.flatten3(|s| s.explosions.iter().map(|v| [v.x, v.y, v.sprite as i32]).collect())
    }

    /// `explosions` without the hollow rings that end the fire explosions in
    /// Modern style (`modern_explosions::ring_phase_frame`, embers instead).
    /// level.gd draws image `sprite + 1`.
    #[func]
    fn explosions_to_draw(&self) -> PackedInt32Array {
        let embers = crate::fx_render::FX.with(|f| f.borrow().modern_explosions_on());
        self.flatten3(|s| {
            s.explosions
                .iter()
                .filter(|v| !(embers && crate::modern_explosions::ring_phase_frame(v.sprite.wrapping_add(1))))
                .map(|v| [v.x, v.y, v.sprite as i32])
                .collect()
        })
    }

    /// FNV-1a 64 fingerprint of the state (reinterpreted as a signed integer).
    #[func]
    fn state_hash(&self) -> i64 {
        self.sim.as_ref().map_or(0, |s| s.state_hash() as i64)
    }

    /// Golden master line (`golden_master.c:write_tick`) of the tick that has just
    /// been played: the capture tool compares it with the C one, column by
    /// column (`--replay-strict`), in the exported game itself.
    #[func]
    fn frame_line(&self) -> GString {
        self.sim.as_ref().map_or(GString::new(), |s| GString::from(s.frame_record().golden_line().as_str()))
    }
}

impl TyrianSim {
    fn flatten3(&self, f: impl Fn(&SimState) -> Vec<[i32; 3]>) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        let flat: Vec<i32> = f(sim.presented_state()).into_iter().flatten().collect();
        PackedInt32Array::from(flat)
    }
}
