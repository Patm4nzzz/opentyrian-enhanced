//! Recording of a `.rep` replay by the rewrite (`ReplayRecorder`, counterpart
//! of `replay.c:replay_capture_frame` / `replay_stop_recording`): the file
//! written must be readable again, and its replay must give the same state at each tick
//! as a second replay of the same file. The proof against the replay of the
//! C binary is in `replays.rs` (the `.rep` files recorded by the Godot game are
//! stored in `rewrite/golden/replays/` with the golden master of the C replay).

use std::path::PathBuf;

use tyrian_formats::replay::Replay;
use tyrian_sim::modes::ModeSetup;
use tyrian_sim::replay::{lvl_file_of_section, ReplayRecorder, ReplayRun};
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Scripted sweep (same pace as `utils/golden/inputs/sweep.in`): bits 0 up,
/// 1 down, 2 left, 3 right, 4 fire, 5 fire switch, 6-7 sidekicks.
fn script(tick: usize) -> u8 {
    const RUNS: [(usize, u8); 9] = [
        (40, 0x00),
        (150, 0x10),
        (120, 0x14),
        (240, 0x18),
        (120, 0x14),
        (80, 0x11),
        (80, 0x12),
        (240, 0x14),
        (240, 0x18),
    ];
    let mut left = tick % RUNS.iter().map(|r| r.0).sum::<usize>();
    for (n, mask) in RUNS {
        if left < n {
            return mask;
        }
        left -= n;
    }
    0
}

/// Records `ticks` ticks of section `section` of episode 1 (game played in
/// mode `modes`) with `script`, returns the bytes of the `.rep` and the positions that
/// `replay_capture_frame` must have sampled in it.
fn record(modes: ModeSetup, front_power: u8, ticks: usize) -> (Vec<u8>, Vec<(i32, i32)>) {
    let data = root().join("data");
    let lvl = lvl_file_of_section(&data, 1, 2, 2).unwrap();
    let mut loadout = tyrian_golden::new_game_loadout();
    loadout.cash = tyrian_golden::initial_cash(1);
    loadout.front_power = front_power;
    let mut setup = tyrian_sim::setup::load_level(&data, 1, 2, lvl, 2, loadout, 1).unwrap();
    setup.modes = modes;
    let (mut rec, mut sim) = ReplayRecorder::begin(&data, setup, 2, lvl, 0x1234_5678_9abc_def1, 4).unwrap();
    let mut expected = Vec::new();
    for t in 0..ticks {
        if sim.state().level_ended {
            break;
        }
        let mask = script(t);
        // Position read at the top of the iteration by the C code: the one from before the simulation.
        expected.push(if t == 0 {
            (sim.world().players[0].x, sim.world().players[0].y)
        } else {
            sim.bottom_positions()[0]
        });
        rec.capture(&sim, mask, 0);
        sim.step([PlayerInput(mask), PlayerInput(mask)]);
    }
    assert_eq!(rec.frame_count(), expected.len());
    (rec.finish(), expected)
}

/// Replays `bytes` to the end and returns the state fingerprint of each tick.
fn play_hashes(bytes: &[u8]) -> Vec<u64> {
    let data = root().join("data");
    let mut run = ReplayRun::new(&data, Replay::load(bytes).unwrap()).unwrap();
    let mut hashes = Vec::new();
    while run.step() {
        hashes.push(run.sim.state_hash());
    }
    hashes
}

fn check(name: &str, bytes: &[u8], expected: &[(i32, i32)], game_mode: u8) {
    let rep = Replay::load(bytes).unwrap();
    assert_eq!(
        (rep.version, rep.episode, rep.level, rep.difficulty, rep.game_mode),
        (4, 1, 2, 2, game_mode),
        "{name}: header"
    );
    assert_eq!(rep.game_speed_x10, 40, "{name}: speed");
    assert_eq!(rep.frames.len(), expected.len(), "{name}: number of frames");
    for (i, (f, e)) in rep.frames.iter().zip(expected).enumerate() {
        assert_eq!((f.x as i32, f.y as i32), *e, "{name}: position of frame {i}");
    }
    // The buttons are those of the script (bits 4 to 7 → bits 0 to 3).
    for (i, f) in rep.frames.iter().enumerate() {
        assert_eq!(f.buttons, script(i) >> 4, "{name}: buttons of frame {i}");
    }
    // The file is exactly what `Replay::write` rewrites (C format).
    assert!(Replay::parse(bytes).unwrap().write() == bytes, "{name}: bytes");
    // Two replays of the same file give the same state at each tick.
    let a = play_hashes(bytes);
    let b = play_hashes(bytes);
    assert_eq!(a.len(), expected.len(), "{name}: ticks read back");
    assert_eq!(a, b, "{name}: readbacks differ");
}

fn have_data() -> bool {
    let ok = root().join("data/tyrian.hdt").exists();
    if !ok {
        eprintln!("data missing: test skipped");
    }
    ok
}

#[test]
fn campaign_recording_plays_back_identically() {
    if !have_data() {
        return;
    }
    let (bytes, expected) = record(ModeSetup::default(), 1, 900);
    assert!(expected.len() >= 900);
    check("campaign", &bytes, &expected, 0);
}

#[test]
fn arcade_one_player_recording_plays_back_identically() {
    if !have_data() {
        return;
    }
    let modes = ModeSetup { one_player_action: true, ..Default::default() };
    // Arcade: the front weapon power is the number of lives (3).
    let (bytes, expected) = record(modes, 3, 900);
    check("arcade 1P", &bytes, &expected, 1);
}

#[test]
fn two_player_recording_plays_back_identically() {
    if !have_data() {
        return;
    }
    let modes = ModeSetup { two_player: true, one_player_action: false, ..Default::default() };
    let (bytes, expected) = record(modes, 1, 900);
    check("two players", &bytes, &expected, 2);
}

#[test]
fn mutators_are_recorded() {
    if !have_data() {
        return;
    }
    let modes = ModeSetup { mutators: 0x47, ..Default::default() };
    let (bytes, expected) = record(modes, 1, 300);
    let rep = Replay::load(&bytes).unwrap();
    assert_eq!(rep.mutators, [1, 1, 1, 0, 0, 0, 1, 0]);
    check("mutators", &bytes, &expected, 0);
}

/// Modes that `replay.c` cannot replay are rejected at the start: no
/// unplayable file is written.
#[test]
fn unplayable_modes_are_refused() {
    if !have_data() {
        return;
    }
    let data = root().join("data");
    let lvl = lvl_file_of_section(&data, 1, 2, 2).unwrap();
    for modes in [
        ModeSetup { galaga: true, ..Default::default() },
        ModeSetup { super_arcade: 1, one_player_action: true, ..Default::default() },
        ModeSetup { timed: 1, ..Default::default() },
        ModeSetup { endless_lives: Some([3, 3]), ..Default::default() },
    ] {
        let mut setup =
            tyrian_sim::setup::load_level(&data, 1, 2, lvl, 2, tyrian_golden::new_game_loadout(), 1).unwrap();
        setup.modes = modes;
        assert!(ReplayRecorder::begin(&data, setup, 2, lvl, 7, 4).is_err(), "{modes:?} should be rejected");
    }
}
