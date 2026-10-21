//! Music orders of the level loop (ADR 0010).
//!
//! 1. The orders do not touch the simulated state: replaying a golden master
//!    by draining the queue at each tick or without ever draining it gives the same
//!    state fingerprint at each tick.
//! 2. A level completed alive emits `play_song(9)` (SONG_LEVELEND), like
//!    `tyrian2.c`: "if (!allPlayersGone && levelEnd > 0 && endLevel)
//!    play_song(9)"; a death emits the fade then the game over.

use std::path::PathBuf;
use tyrian_golden::GoldenMaster;
use tyrian_sim::{MusicCue, PlayerInput};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load(name: &str) -> Option<GoldenMaster> {
    let data = root().join("data");
    let path = root().join("rewrite/golden").join(name);
    if !data.join("tyrian.hdt").exists() || !path.exists() {
        eprintln!("data or golden master missing: test skipped");
        return None;
    }
    Some(GoldenMaster::load(&path).expect("golden master"))
}

#[test]
fn music_cues_do_not_change_simulated_state() {
    let Some(gm) = load("e1-tyrian-autopilot.gm") else {
        return;
    };
    let data = root().join("data");
    let mut drained = gm.make_sim(&data).unwrap();
    let mut kept = gm.make_sim(&data).unwrap();
    kept.set_music_volume(40);
    let mut cues = Vec::new();
    for exp in &gm.ticks {
        if drained.state().level_ended {
            break;
        }
        drained.step([PlayerInput(exp.mask), PlayerInput(0)]);
        kept.step([PlayerInput(exp.mask), PlayerInput(0)]);
        cues.extend(drained.take_music_cues());
        assert_eq!(drained.state_hash(), kept.state_hash(), "tick {}", exp.tick);
        assert_eq!(drained.frame_record(), kept.frame_record(), "tick {}", exp.tick);
    }
    assert!(drained.state().level_ended, "the level must end as in the C version");
    // Level won: end jingle (SONG_LEVELEND = 9, musmast.h), never the game over (10).
    assert!(cues.contains(&MusicCue::PlaySong(9)), "cues: {cues:?}");
    assert!(!cues.contains(&MusicCue::PlaySong(10)), "cues: {cues:?}");
    // The undrained queue stays bounded (identical consecutive orders merged).
    assert!(kept.take_music_cues().len() < 64);
}

/// e1-tyrian-script: the player dies (C "level" end by game over).
/// Expected sequence of the C code:
/// - `varz.c:JE_playerDamage` sets `tempVolume = tyrMusicVolume` on death;
/// - `tyrian2.c` (GAME OVER block) sets `musicFade` while the ship
///   explodes, then the MUSIC FADE block lowers the volume by 1 per tick
///   (`set_volume(--tempVolume)`);
/// - at the end of the explosion: `play_song(SONG_GAMEOVER)` then
///   `set_volume(tyrMusicVolume)`, and `firstGameOver = false`.
#[test]
fn death_fades_then_plays_game_over() {
    let Some(gm) = load("e1-tyrian-script.gm") else { return };
    let mut sim = gm.make_sim(&root().join("data")).unwrap();
    // First tick: player alive, no level end -> branch
    // "else if (!playing && firstGameOver)" reached.
    sim.step([PlayerInput(gm.ticks[0].mask), PlayerInput(0)]);
    assert!(sim.music_level_song_branch());
    assert!(sim.take_music_cues().is_empty());
    let mut all = Vec::new();
    for exp in &gm.ticks[1..] {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        all.extend(sim.take_music_cues());
    }
    let n = all.len();
    assert!(n > 10, "{all:?}");
    assert_eq!(&all[n - 2..], &[MusicCue::PlaySong(10), MusicCue::SetVolume(191)], "{all:?}");
    // Fade: 190, 189, ... strictly decreasing by 1, never below 10.
    for (k, c) in all[..n - 2].iter().enumerate() {
        assert_eq!(*c, MusicCue::SetVolume(190 - k as u16), "{all:?}");
    }
    assert!(!sim.first_game_over());
    // The MUSIC FADE block precedes the GAME OVER block in the same iteration
    // (tyrian2.c): at the game over tick the branch was still evaluated
    // with firstGameOver true; at the next tick it no longer is.
}

/// "Game Over" screen (tyrian2.c): "newkey || button[0..2] || newmouse"
/// ends the level. With no input at all it stays displayed; any
/// key (newkey, not fire) closes it on the next tick.
#[test]
fn any_key_dismisses_game_over() {
    let Some(gm) = load("e1-tyrian-script.gm") else { return };
    // First pass: tick of the ship's destruction.
    let mut probe = gm.make_sim(&root().join("data")).unwrap();
    let death = gm.ticks.iter().position(|exp| {
        probe.step([PlayerInput(exp.mask), PlayerInput(0)]);
        probe.world().ending.all_players_gone
    });
    let death = death.expect("the golden master must lose the ship");
    // Second pass: fire released on the last living tick (C button[]
    // keeps its last value during the explosion), then no input.
    let mut sim = gm.make_sim(&root().join("data")).unwrap();
    for (k, exp) in gm.ticks.iter().enumerate() {
        if !sim.first_game_over() || sim.state().level_ended {
            break;
        }
        let mask = if k >= death { 0 } else { exp.mask };
        sim.step([PlayerInput(mask), PlayerInput(0)]);
    }
    assert!(!sim.first_game_over(), "the golden master must reach the game over");
    for _ in 0..50 {
        sim.step([PlayerInput(0), PlayerInput(0)]);
    }
    assert!(!sim.state().level_ended, "without a key press, the Game Over screen stays displayed");
    sim.set_new_key(true);
    sim.step([PlayerInput(0), PlayerInput(0)]);
    assert!(sim.state().level_ended, "a key press must close the Game Over screen");
}
