//! Achievements unlocked on golden masters of the C version: the game is
//! replayed in tyrian-sim (identical to the C code tick by tick, tests/parity.rs) and
//! `achievements_check_level` (achievements.c) is applied to the end-of-level
//! state. C reference: `rewrite/golden/achievements/*.unlocked.txt`, the
//! "achievement unlocked" log of the GOLDEN binary run with GOLDEN_ACH=1
//! (golden_master.c then emits the real EVENT_LEVEL_COMPLETE before quitting;
//! the .gm produced is identical to the versioned golden master).

use std::path::PathBuf;
use tyrian_golden::GoldenMaster;
use tyrian_sim::achievements::{AchievementTracker, ACHIEVEMENTS, ACH_MAX};
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn unlocked_on(name: &str, spectator: bool) -> Option<Vec<&'static str>> {
    let data = root().join("data");
    let f = root().join("rewrite/golden").join(name);
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return None;
    }
    let gm = GoldenMaster::load(&f).unwrap();
    let mut sim = gm.make_sim(&data).unwrap();
    let mut tr = AchievementTracker::new([false; ACH_MAX], spectator);
    tr.level_start(&sim);
    // Same loop as tyrian_golden::compare.
    for exp in &gm.ticks {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        tr.after_tick(&sim);
    }
    assert!(sim.state().level_ended, "{name}: level end not reached");
    Some(tr.level_complete(&sim).into_iter().map(|i| ACHIEVEMENTS[i].0).collect())
}

/// All the unlocks of the level (per-frame listeners then level end).
fn full_log(name: &str) -> Option<Vec<&'static str>> {
    let data = root().join("data");
    let f = root().join("rewrite/golden").join(name);
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return None;
    }
    let gm = GoldenMaster::load(&f).unwrap();
    let mut sim = gm.make_sim(&data).unwrap();
    let mut tr = AchievementTracker::new([false; ACH_MAX], false);
    tr.level_start(&sim);
    for exp in &gm.ticks {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        tr.after_tick(&sim);
    }
    tr.level_complete(&sim);
    Some(tr.log.iter().map(|&i| ACHIEVEMENTS[i].1).collect())
}

/// Complete C log under virtual clock (GOLDEN_ACH_CLOCK=1, ADR 0091),
/// the .gm of this run being identical to the versioned golden master: the 16
/// C achievements, no more no less, in the same order. The C virtual clock is
/// frozen during level loading and `lastFrameTicks` re-read just before
/// the loop (golden_master_level_clock_start): the DPS window of `dps.c` has
/// the same phase as in tyrian-sim.
#[test]
fn full_unlock_log_matches_c_with_virtual_clock() {
    let Some(got) = full_log("e1-tyrian-hard-autopilot.gm") else {
        return;
    };
    let log =
        std::fs::read_to_string(root().join("rewrite/golden/achievements/e1-tyrian-hard-autopilot.clock.unlocked.txt"))
            .unwrap();
    let c: Vec<&str> = log.lines().collect();
    assert_eq!(c.len(), 16);
    assert_eq!(got, c);
}

#[test]
fn hard_autopilot_unlocks_survivor_max_power_hard_impossible() {
    // difficulty 4 (Impossible), assist 1: armor and power maintained
    // by autopilot_drive, no death.
    let Some(got) = unlocked_on("e1-tyrian-hard-autopilot.gm", false) else {
        return;
    };
    assert_eq!(got, ["SURVIVOR", "MAX_POWER", "HARD_MODE", "IMPOSSIBLE", "BULLETPROOF"]);
}

#[test]
fn replay_playback_unlocks_nothing() {
    // ach_spectator_mode() : replay_state == REPLAY_PLAYING.
    let Some(got) = unlocked_on("e1-tyrian-hard-autopilot.gm", true) else {
        return;
    };
    assert!(got.is_empty());
}

/// Displayed names of the achievements that the C code unlocks at the end of the level, restricted
/// to the ported rules: same list, same order as tyrian-sim.
#[test]
fn level_end_unlocks_match_c_log() {
    let Some(got) = unlocked_on("e1-tyrian-hard-autopilot.gm", false) else {
        return;
    };
    let ported: Vec<&str> = [
        "SURVIVOR",
        "PHOENIX",
        "CLOSE_CALL",
        "MAX_POWER",
        "HARD_MODE",
        "IMPOSSIBLE",
        "UNTOUCHABLE",
        "IRONMAN",
        "IMMORTAL",
        "ADAMANTIUM",
        "BULLETPROOF",
        "SPEED_DEMON",
    ]
    .iter()
    .map(|id| ACHIEVEMENTS.iter().find(|a| a.0 == *id).unwrap().1)
    .collect();
    // Log under virtual clock: the real-time one (.unlocked.txt) depends
    // on the speed of the machine (Speed Demon falls there because the golden master runs as fast as possible).
    let log =
        std::fs::read_to_string(root().join("rewrite/golden/achievements/e1-tyrian-hard-autopilot.clock.unlocked.txt"))
            .unwrap();
    let c: Vec<&str> = log.lines().filter(|n| ported.contains(n)).collect();
    let rust: Vec<&str> = got.iter().map(|id| ACHIEVEMENTS.iter().find(|a| a.0 == *id).unwrap().1).collect();
    assert_eq!(rust, c);
}

/// Modded Run: a level completed with at least one active mod (`enabled_mod_count`),
/// on the real level of the golden master; without a mod, nothing.
#[test]
fn modded_run_unlocks_only_with_an_enabled_mod() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/e1-tyrian-hard-autopilot.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return;
    }
    let run = |mods: u32| {
        let gm = GoldenMaster::load(&f).unwrap();
        let mut sim = gm.make_sim(&data).unwrap();
        let mut tr = AchievementTracker::new([false; ACH_MAX], false);
        tr.enabled_mod_count = mods;
        tr.level_start(&sim);
        for exp in &gm.ticks {
            if sim.state().level_ended {
                break;
            }
            sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
            tr.after_tick(&sim);
        }
        tr.level_complete(&sim).into_iter().map(|i| ACHIEVEMENTS[i].0).collect::<Vec<_>>()
    };
    assert!(!run(0).contains(&"MODDED_RUN"));
    assert!(run(1).contains(&"MODDED_RUN"));
}

/// R-04: in 2 players, the death of each player counts (`stats_on_player_died`,
/// varz.c:1204-1216 emits EVENT_PLAYER_DIED for any player).
/// 2-player Endless with no input at all: both ships eventually die.
#[test]
fn two_player_deaths_are_counted_for_both_players() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/endless/endless2-arcade-seed1.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return;
    }
    let gm = GoldenMaster::load(&f).unwrap();
    let mut sim = gm.make_sim(&data).unwrap();
    let mut tr = AchievementTracker::new([false; ACH_MAX], false);
    tr.level_start(&sim);
    let mut was_alive = [true; 2];
    let mut deaths = [0u64; 2];
    for _ in 0..40_000 {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(0), PlayerInput(0)]);
        tr.after_tick(&sim);
        for i in 0..2 {
            let alive = sim.world().players[i].is_alive;
            if was_alive[i] && !alive {
                deaths[i] += 1;
            }
            was_alive[i] = alive;
        }
    }
    assert!(deaths[1] > 0, "player 2 must die at least once ({deaths:?})");
    assert_eq!(tr.game_stats.deaths, deaths[0] + deaths[1], "deaths of both players counted");
}

/// Survivor (achievements.c:891): refused if player 2 died during the
/// level, even when player 1 survived (deaths_this_level counts the deaths
/// of both players, varz.c:1204-1216). Two players with no input; player 1
/// keeps its armor (dev Ctrl+Shift+H cheat from dev_console.c) until the death of
/// player 2, then the level is ended (Ctrl+Shift+N).
#[test]
fn survivor_is_refused_when_player_two_died() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/mode-2p-link.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return;
    }
    let gm = GoldenMaster::load(&f).unwrap();
    let mut sim = gm.make_sim(&data).unwrap();
    let mut tr = AchievementTracker::new([false; ACH_MAX], false);
    tr.level_start(&sim);
    let mut p2_dead_at = None;
    for t in 0..40_000u32 {
        if sim.state().level_ended {
            break;
        }
        sim.dev_restore_armor();
        if p2_dead_at.is_some_and(|d| t == d + 5) {
            sim.dev_complete_level();
        }
        sim.step([PlayerInput(0), PlayerInput(0)]);
        tr.after_tick(&sim);
        if p2_dead_at.is_none() && !sim.world().players[1].is_alive {
            p2_dead_at = Some(t);
        }
    }
    assert!(p2_dead_at.is_some(), "player 2 never died");
    assert!(sim.world().players[0].is_alive, "player 1 must have survived");
    let unlocked: Vec<&str> = tr.level_complete(&sim).into_iter().map(|i| ACHIEVEMENTS[i].0).collect();
    assert!(!unlocked.contains(&"SURVIVOR"), "Survivor granted although player 2 died: {unlocked:?}");
}
