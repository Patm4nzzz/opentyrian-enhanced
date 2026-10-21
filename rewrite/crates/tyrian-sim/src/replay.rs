//! Playback of the C version's `.rep` replays (`src/replay.c`) in the core.
//!
//! The C code does not replay keys: `replay_inject_frame`, called at the top of
//! each iteration of `tyrian2.c:level_loop`, FORCES the player 1 position
//! taken at recording time and sets the four buttons (fire, fire mode
//! change, sidekicks) in `keysactive[]`. The direction keys stay
//! released. This exact path is reproduced, including its consequences:
//! shots start from the position forced at the top of the frame (and not from the one
//! reached after the recorded movement), so that a
//! C playback can drift from the recorded game (ADR 0090). The
//! parity reference is therefore the C playback, not the recording.
//!
//! Generators: `replay.c:replay_level_start_hook` re-seeds them at the start of the
//! level (`reseed_for_level`), right after `JE_loadMap`; then
//! `backgrnd.c:initialize_starfield` (called by `tyrian2.c:JE_main` before the
//! loop) draws 4 `mt_rand` per star for its 100 stars.

use crate::WeaponId;
use crate::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId};
use std::path::Path;

use tyrian_formats::episode::EpisodeScript;
use tyrian_formats::replay::{Replay, ReplayFrame};

use crate::rng::{MersenneTwister, Xoshiro256};
use crate::{LevelSetup, Loadout, PlayerInput, Sim};

/// Section of `levelsN.dat` → level (1-based) of `tyrianN.lvl`.
/// `level_load.c:JE_loadMap` positions itself on the `section`-th `*` line, then
/// reads lines up to the first `]L[` (`case 'L'`) without stopping at the following
/// `*`: a section without `]L[` (cutscene, shop) leads to the level of
/// the next one. `]h[` (`case 'h'`) skips the next line when
/// `initialDifficulty > DIFFICULTY_NORMAL`; the Replays menu launcher sets
/// `initialDifficulty = difficultyLevel` from the `.rep` (title_screen.c). Section
/// jumps (`]J[`, `]H[`, `]2[`...) are not followed: `replay.c`
/// records `mainLevel` AFTER `JE_loadMap`.
pub fn lvl_file_of_section(data_dir: &Path, episode: u8, section: u8, initial_difficulty: u8) -> Result<usize, String> {
    const DIFFICULTY_NORMAL: u8 = 2;
    let path = data_dir.join(format!("levels{episode}.dat"));
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let script = EpisodeScript::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let sections = script.sections();
    let start = (section as usize)
        .checked_sub(1)
        .filter(|&i| i < sections.len())
        .ok_or_else(|| format!("{}: no section {section}", path.display()))?;
    let lines = script.plain_lines();
    let mut i = sections[start].line_index + 1;
    while i < lines.len() {
        let s = &lines[i];
        if s.len() >= 2 && s[0] == b']' {
            match s[1] {
                b'L' => {
                    let l = tyrian_formats::episode::LevelDirective::parse(i, s);
                    return Ok(l.lvl_file_num.max(1) as usize);
                }
                b'h' if initial_difficulty > DIFFICULTY_NORMAL => i += 1,
                _ => {}
            }
        }
        i += 1;
    }
    Err(format!("{}: no ]L[ after section {section}", path.display()))
}

/// Section of `levelsN.dat` to write into a replay header so that
/// playback loads level `lvl_file`: `hint` if it leads there (this is the
/// C `mainLevel`), otherwise the last section that leads there (a section without
/// `]L[` falls through to the next one, the last one is the one carrying the `]L[`).
pub fn section_for_level(
    data_dir: &Path,
    episode: u8,
    hint: u8,
    lvl_file: usize,
    initial_difficulty: u8,
) -> Result<u8, String> {
    if hint >= 1 && lvl_file_of_section(data_dir, episode, hint, initial_difficulty).is_ok_and(|l| l == lvl_file) {
        return Ok(hint);
    }
    (1..=u8::MAX)
        .take_while(|&s| lvl_file_of_section(data_dir, episode, s, initial_difficulty).is_ok() || s == 1)
        .filter(|&s| lvl_file_of_section(data_dir, episode, s, initial_difficulty).is_ok_and(|l| l == lvl_file))
        .last()
        .ok_or_else(|| format!("replay: no section of episode {episode} leads to level {lvl_file}"))
}

/// Equipment of the `.rep` in the core's format (`title_screen.c`: `player[0].items
/// = *replay_get_items()`, `player[0].cash = replay_get_cash()`).
pub fn loadout_of(rep: &Replay) -> Loadout {
    let it = &rep.items;
    Loadout {
        ship: ShipId(it.ship),
        front_weapon: WeaponId(it.weapon[0].0),
        front_power: it.weapon[0].1,
        rear_weapon: WeaponId(it.weapon[1].0),
        rear_power: it.weapon[1].1,
        shield: ShieldId(it.shield),
        generator: GeneratorId(it.generator),
        left_sidekick: SidekickId(it.sidekick[0]),
        right_sidekick: SidekickId(it.sidekick[1]),
        special: SpecialId(it.special),
        cash: rep.cash,
    }
}

/// Core input mask for one frame: only the four buttons, bits 4
/// to 7 (`replay_inject_frame` does not touch the directions).
pub fn input_of(f: &ReplayFrame) -> PlayerInput {
    PlayerInput((f.buttons & 0x0f) << 4)
}

/// Generators at the start of the level loop of a recorded or
/// replayed game: `reseed_for_level` then `initialize_starfield`.
/// The stars drawn here (100 × 4 `mt_rand`) are the ones the playback displays.
pub fn level_start_rng(
    rep: &Replay,
    processor_type: u8,
) -> (Xoshiro256, MersenneTwister, [crate::world::StarfieldStar; crate::world::MAX_STARS]) {
    let s = rep.level_seed(rep.episode, rep.level);
    let rng = Xoshiro256::new(s);
    let mut mt = MersenneTwister::new(Replay::mt_seed(s));
    let stars = crate::world::initialize_starfield(processor_type, &mut mt);
    (rng, mt, stars)
}

/// Level prepared like the Replays menu launcher (`title_screen.c`) and
/// `JE_main` up to `level_loop`, with the level file played.
pub fn setup_of(data_dir: &Path, replay: &Replay, initial_difficulty: u8) -> Result<(LevelSetup, usize), String> {
    let lvl = lvl_file_of_section(data_dir, replay.episode, replay.level, initial_difficulty)?;
    let mut setup: LevelSetup = crate::setup::load_level(
        data_dir,
        replay.episode,
        replay.level,
        lvl,
        replay.difficulty,
        loadout_of(replay),
        replay.seed,
    )?;
    setup.assist = false;
    setup.initial_difficulty = Some(initial_difficulty);
    setup.modes.mutators =
        replay.mutators.iter().enumerate().fold(0u8, |m, (i, &v)| if v != 0 { m | (1 << i) } else { m });
    // title_screen.c: player[1].items = replay_get_items_p2(), restored for all
    // modes (in single player ship 2 is unused but its counters, including
    // the lives aliased onto weapon[].power, enter the hash).
    let p2 = Replay { items: replay.items_p2, ..replay.clone() };
    setup.modes.loadout2 = Some(loadout_of(&p2));
    if replay.game_mode != 0 {
        // Mode 1: onePlayerAction = true, superArcadeMode = 0, timedBattleMode = false.
        // Mode 2: twoPlayerMode = true.
        setup.modes.one_player_action = replay.game_mode == 1;
        setup.modes.two_player = replay.game_mode == 2;
        setup.modes.cash = (replay.cash as u64, 0);
        setup.modes.initial_difficulty = initial_difficulty;
        setup.modes.devices = (1, 2);
    }
    Ok((setup, lvl))
}

/// Playback cursor: the `.rep` and the next frame to inject. The driven
/// `Sim` is held separately (the Godot client keeps its own).
pub struct ReplayCursor {
    pub replay: Replay,
    /// Level of `tyrianN.lvl` played (`lvl_file_of_section`).
    pub lvl_file: usize,
    pos: usize,
    started: bool,
}

impl ReplayCursor {
    /// `title_screen.c` (Replays menu launcher) then `JE_main` up to
    /// `level_loop`. Refuses what the core does not simulate: two players and
    /// one-player arcade (`game_mode` 1) as well as 2 players (2). The file's mutators
    /// are set as the launcher does (`title_screen.c`,
    /// `mut_* = replay_get_mutators()[i]`, bits in the order of `rec_mutators`).
    pub fn start(data_dir: &Path, replay: Replay) -> Result<(Self, Sim), String> {
        let initial_difficulty = replay.difficulty;
        Self::start_with_initial_difficulty(data_dir, replay, initial_difficulty)
    }

    /// [`ReplayCursor::start`] with the game's initial difficulty
    /// (`initialDifficulty`, absent from the `.rep`): the Replays menu launcher
    /// sets it equal to the file's difficulty, whereas the recorded game
    /// may have started from another (two players: `title_screen.c`
    /// raises `difficultyLevel` one notch after setting `initialDifficulty`).
    pub fn start_with_initial_difficulty(
        data_dir: &Path,
        replay: Replay,
        initial_difficulty: u8,
    ) -> Result<(Self, Sim), String> {
        Self::start_with_options(data_dir, replay, initial_difficulty, 4)
    }

    /// [`ReplayCursor::start_with_initial_difficulty`] at the detail level of the
    /// player's effects (`config.c:processorType`): the C code re-reads the current
    /// setting at playback (`JE_initProcessorType`) as at recording,
    /// and it decides the number of starfield draws in the level
    /// generator. 4 = factory setting, the one of the golden masters.
    pub fn start_with_options(
        data_dir: &Path,
        replay: Replay,
        initial_difficulty: u8,
        processor_type: u8,
    ) -> Result<(Self, Sim), String> {
        if replay.frames.is_empty() {
            // replay.c:replay_start_playback refuses a file with no frames.
            return Err("replay: no frames".into());
        }
        let (mut setup, lvl) = setup_of(data_dir, &replay, initial_difficulty)?;
        // config.c:processorType from setup (Setup > Graphics > Visual FX):
        // replay.c:JE_initProcessorType re-derives from the current setting.
        setup.processor_type = processor_type;
        let (rng, mt, stars) = level_start_rng(&replay, setup.processor_type);
        let mut sim = Sim::new_paused(setup, rng, mt);
        sim.set_starfield(stars);
        Ok((Self { replay, lvl_file: lvl, pos: 0, started: false }, sim))
    }

    /// `replay_inject_frame`: second ship only if the file
    /// carries it (v4) and the game is two-player (`rec_game_mode == 2`).
    fn p2_of(&self, f: &ReplayFrame) -> Option<(i32, i32)> {
        (self.replay.version >= 4 && self.replay.game_mode == 2).then_some((f.x2 as i32, f.y2 as i32))
    }

    /// Frames already injected.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Playback is over: file exhausted (`replay_inject_frame`
    /// returns false) or level end (`reallyEndLevel`, `tyrian2.c`).
    pub fn finished(&self, sim: &Sim) -> bool {
        self.pos >= self.replay.frames.len() || sim.state().level_ended
    }

    /// One iteration of `level_loop`. The first one injects frame 0 and
    /// plays the loop head; each call finishes the iteration of frame
    /// `pos` with its buttons, then injects the next frame and plays the head
    /// of the next iteration (the C golden master closes the tick at this
    /// point, `tyrian2.c` before `autopilot_drive`). Returns false if nothing
    /// was played.
    pub fn step(&mut self, sim: &mut Sim) -> bool {
        if self.finished(sim) {
            return false;
        }
        if !self.started {
            let f = self.replay.frames[0];
            sim.replay_begin_2p(f.x as i32, f.y as i32, self.p2_of(&f));
            self.started = true;
        }
        let f = self.replay.frames[self.pos];
        let next = self.replay.frames.get(self.pos + 1).map(|n| ((n.x as i32, n.y as i32), self.p2_of(n)));
        self.pos += 1;
        sim.replay_iteration_2p(input_of(&f), next);
        true
    }
}

/// A complete playback: cursor and simulation together (tooling, tests).
pub struct ReplayRun {
    pub cursor: ReplayCursor,
    pub sim: Sim,
}

impl ReplayRun {
    pub fn new(data_dir: &Path, replay: Replay) -> Result<Self, String> {
        let (cursor, sim) = ReplayCursor::start(data_dir, replay)?;
        Ok(Self { cursor, sim })
    }

    pub fn load(data_dir: &Path, path: &Path) -> Result<Self, String> {
        Self::new(data_dir, load_file(path)?)
    }

    pub fn finished(&self) -> bool {
        self.cursor.finished(&self.sim)
    }

    pub fn step(&mut self) -> bool {
        self.cursor.step(&mut self.sim)
    }
}

/// Replay recorder (`replay.c:begin_recording`, `replay_capture_frame`,
/// `replay_stop_recording`): writes the same format as the C code, readable by
/// [`ReplayCursor`] and by the C binary.
///
/// Like the C code, the file does not keep the direction keys: at each
/// iteration it takes the POSITION of both ships and the four buttons,
/// at the top of the loop (`tyrian2.c:level_loop`, right before the simulation of
/// the iteration).
pub struct ReplayRecorder {
    replay: Replay,
    /// Positions of both ships at level start, taken by the
    /// first iteration before its loop head.
    start_positions: [(i32, i32); 2],
}

impl ReplayRecorder {
    /// Recording start at the start of a level (`replay_level_start_hook`
    /// with an armed request): header taken like `begin_recording`
    /// (episode, section, difficulty, seed, speed, mode, equipment,
    /// money, mutators) on the game as it starts, whose
    /// generators are re-seeded from the file seed
    /// (`reseed_for_level`) then the starfield drawn, as at playback.
    /// `section`: `mainLevel` announced by the caller (0 = unknown), `lvl_file`:
    /// level played, `seed`: header seed (`rec_seed`, never 0).
    ///
    /// Refuses what `replay.c` cannot replay: Galaga, Super Arcade,
    /// Timed Battle, Super Tyrian, anomalies, Endless (the header only carries
    /// the one or two player mode, the C playback resets `superArcadeMode` and
    /// `timedBattleMode` to zero), an active mod, or a section that does not lead
    /// to this level. Better to record nothing than an unplayable file.
    pub fn begin(
        data_dir: &Path,
        setup: LevelSetup,
        section: u8,
        lvl_file: usize,
        seed: u64,
        game_speed: u8,
    ) -> Result<(Self, Sim), String> {
        let m = &setup.modes;
        if m.galaga
            || m.super_arcade != 0
            || m.timed != 0
            || m.super_tyrian
            || m.anomalies != 0
            || m.endless_lives.is_some()
        {
            return Err("replay: this game mode cannot be replayed (Galaga, Super Arcade, Timed Battle, Super Tyrian, anomalies, Endless)".into());
        }
        if !setup.patch.is_empty() {
            return Err("replay: a mod is active, playback would not reproduce it".into());
        }
        let episode = setup.episode;
        let difficulty = setup.difficulty;
        let section = section_for_level(data_dir, episode, section, lvl_file, difficulty)?;
        let seed = if seed == 0 { 0x5_DEEC_E66D } else { seed };
        let mut replay = Replay {
            version: tyrian_formats::replay::REPLAY_VERSION,
            episode,
            level: section,
            difficulty,
            seed,
            game_speed_x10: game_speed.saturating_mul(10),
            game_mode: if m.two_player {
                2
            } else if m.one_player_action {
                1
            } else {
                0
            },
            items: Default::default(),
            cash: 0,
            mutators: [0; 8],
            items_p2: Default::default(),
            frames: Vec::new(),
        };
        replay.mutators = std::array::from_fn(|i| (m.mutators >> i) & 1);
        // Equipment and money as the game starts them (`begin_recording`:
        // `player[0].items`, `player[1].items`, `player[0].cash`).
        let w = crate::world::World::start_level(&setup);
        replay.items = items_of(&w.players[0].items);
        replay.items_p2 = items_of(&w.players[1].items);
        replay.cash = w.players[0].cash.min(u32::MAX as u64) as u32;
        let (rng, mt, stars) = level_start_rng(&replay, setup.processor_type);
        let mut sim = Sim::new_paused(setup, rng, mt);
        sim.set_starfield(stars);
        let w = sim.world();
        let start_positions = [(w.players[0].x, w.players[0].y), (w.players[1].x, w.players[1].y)];
        // Head of the first iteration, like `ReplayCursor::step` at playback.
        sim.replay_begin(start_positions[0].0, start_positions[0].1);
        Ok((Self { replay, start_positions }, sim))
    }

    /// `replay_capture_frame`: to be called right before each `Sim::step`, with
    /// the `Sim` as it is before this step. The positions are those the
    /// C code reads at the top of the iteration: end of the bottom half of the
    /// previous iteration (or level start), before `player_collide`. `buttons`:
    /// player 1 input mask (bits 4 to 7: fire, fire mode change,
    /// sidekicks), `mouse_mask`: mouse buttons in the same format.
    pub fn capture(&mut self, sim: &Sim, buttons: u8, mouse_mask: u8) {
        if self.replay.frames.len() >= tyrian_formats::replay::REPLAY_MAX_FRAMES as usize {
            return;
        }
        let [(x, y), (x2, y2)] =
            if self.replay.frames.is_empty() { self.start_positions } else { sim.bottom_positions() };
        let mask = ((buttons | mouse_mask) >> 4) & 0x0f;
        self.replay.frames.push(ReplayFrame {
            x: x as i16,
            y: y as i16,
            buttons: mask,
            x2: x2 as i16,
            y2: y2 as i16,
            // Both ships read the same keys (`replay_capture_frame`).
            buttons2: mask,
        });
    }

    pub fn frame_count(&self) -> usize {
        self.replay.frames.len()
    }

    /// `replay_stop_recording`: content of the `.rep` file.
    pub fn finish(self) -> Vec<u8> {
        self.replay.write()
    }
}

/// Core `player.h:PlayerItems` → file layout.
fn items_of(it: &crate::world::Items) -> tyrian_formats::replay::ReplayItems {
    tyrian_formats::replay::ReplayItems {
        ship: it.ship.0,
        generator: it.generator.0,
        shield: it.shield.0,
        weapon: [(it.weapon[0].id.0, it.weapon[0].power), (it.weapon[1].id.0, it.weapon[1].power)],
        sidekick: [it.sidekick[0].0, it.sidekick[1].0],
        special: it.special.0,
        sidekick_series: it.sidekick_series,
        sidekick_level: it.sidekick_level,
        super_arcade_mode: 0,
    }
}

/// Reads a `.rep` file (`replay.c:replay_load`).
pub fn load_file(path: &Path) -> Result<Replay, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Replay::load(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_maps_buttons_to_fire_bits() {
        let f = ReplayFrame { buttons: 0x0f, ..Default::default() };
        assert_eq!(input_of(&f).0, 0xf0);
        let f = ReplayFrame { buttons: 0x01, ..Default::default() };
        assert!(input_of(&f).has(PlayerInput::FIRE));
    }
}
