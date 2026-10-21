//! Endless batch: C level golden masters in `--goto=endless`
//! (`rewrite/golden/endless/*.gm`, `c-final:utils/golden/record.sh`, scenarios
//! `endless-*`). The rotation is drawn with `GOLDEN_SEED`
//! (`endless.c:endless_start`), then `golden_master.c:golden_master_level_start`
//! reseeds rng and mt. The `arcade.in` script dies three times: two
//! respawns through `endless.c:endless_try_respawn` (3 lives), then the end.

use std::path::{Path, PathBuf};
use tyrian_golden::{compare, GoldenMaster, GROUPS};
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn endless_levels_match_c() {
    let data = root().join("data");
    let dir = root().join("rewrite/golden/endless");
    if !data.join("tyrian.hdt").exists() || !dir.exists() {
        eprintln!("data or Endless golden masters missing: test skipped");
        return;
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gm"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for f in files {
        let gm = GoldenMaster::load(&f).expect("golden master");
        let e = gm.header.endless.expect("endless line");
        assert_eq!(e.lives, [3, 3], "{}: starting lives (ENDLESS_START_LIVES)", f.display());
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let r = compare(&gm, &data, &name, |_, _, _| {}).expect("comparison");
        for g in GROUPS {
            assert_eq!(r.first_divergence[g], None, "{name}: group {g}");
        }
        assert!(r.end_matches && r.ended_early_at.is_none(), "{name}: level end");

        // Lives are consumed in the Endless counter, not in the
        // front weapon power (alias *player[0].lives of the C code).
        let mut sim = gm.make_sim(&data).unwrap();
        let mut respawns = 0;
        let mut last_armor = sim.world().players[0].armor;
        for t in &gm.ticks {
            if sim.state().level_ended {
                break;
            }
            sim.step([PlayerInput(t.mask), PlayerInput(0)]);
            let a = sim.world().players[0].armor;
            if last_armor == 0 && a > 0 {
                respawns += 1;
            }
            last_armor = a;
        }
        assert_eq!(respawns, 2, "{name}: respawns");
        assert_eq!(sim.world().endless_lives.unwrap()[0], 0, "{name}: lives exhausted");
        assert_eq!(sim.world().players[0].items.weapon[0].power, 5, "{name}: weapon power intact");
        // Endless score (endless.c:on_enemy_killed, combos turned off by the golden master).
        let end = gm.endless_end.expect("endless_end line");
        assert_eq!(sim.world().endless_score, end.score, "{name}: Endless score");
        assert_eq!(sim.world().endless_lives.unwrap(), end.lives, "{name}: final lives");
    }
}

/// Game with continuous fire and no movement (seed 7, SAVARA V): the ship ends up
/// losing its three lives, the respawn goes through `endless_try_respawn` and
/// the level end arrives with a counter at 0 (`lives_exhausted`).
#[test]
fn idle_run_exhausts_lives() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/endless/endless-arcade-seed7.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return;
    }
    let gm = GoldenMaster::load(&f).expect("golden master");
    let mut sim = gm.make_sim(&data).expect("sim");
    let mut t = 0;
    // tyrian2.c, GAME OVER block: once allPlayersGone, the screen waits for
    // "newkey || button[0..2] || newmouse". button[] is only re-read while
    // the ship is alive (player_movement.c): we fire continuously, like the
    // arcade.in script of the golden masters.
    while !sim.state().level_ended && t < 40_000 {
        sim.step([PlayerInput(0x10), PlayerInput(0)]);
        t += 1;
    }
    let lives = sim.world().endless_lives.unwrap();
    eprintln!("end at tick {t}, lives {lives:?}, score {}", sim.world().endless_score);
    assert!(sim.state().level_ended);
    assert_eq!(lives[0], 0);
    assert!(!sim.world().players[0].is_alive);
}

/// Two-player Endless (`--goto=endless2`): the equipment and mode read from
/// the C header are those that `tyrian_sim::endless` builds for Godot
/// (`endless.c:endless_start(true)`).
#[test]
fn two_player_setup_matches_c_header() {
    let f = root().join("rewrite/golden/endless/endless2-arcade-seed1.gm");
    if !f.exists() {
        return;
    }
    let gm = GoldenMaster::load(&f).expect("golden master");
    let [p1, p2] = tyrian_sim::endless::start_loadouts_two_player();
    assert_eq!(gm.header.loadout, p1, "player 1 items");
    let m = gm.header.modes.expect("mode line");
    let want = tyrian_sim::endless::mode_setup(true, [3, 3]);
    assert_eq!(m.loadout2, Some(p2), "items2");
    assert_eq!(
        (m.two_player, m.one_player_action, m.galaga, m.devices, m.initial_difficulty),
        (want.two_player, want.one_player_action, want.galaga, want.devices, want.initial_difficulty)
    );
    assert_eq!(gm.header.endless.unwrap().lives, [3, 3]);
    // One player: "arcade" mode (onePlayerAction), no items2.
    let one = tyrian_sim::endless::mode_setup(false, [3, 3]);
    assert!(one.one_player_action && !one.two_player && one.loadout2.is_none());
}
