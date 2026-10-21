//! `.rep` replays produced by the C version (`src/replay.c`, recorded by
//! `c-final:utils/golden/record_replay.sh`, ADR 0090): each one is read back in
//! `tyrian-sim` and must reproduce, tick by tick and to the end, the state
//! written by the golden master of the C replay of the same file
//! (`NAME.play.gm`).

use std::path::PathBuf;
use tyrian_golden::{compare_replay, GoldenMaster};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn c_replays_replay_identically() {
    let data = root().join("data");
    let dir = root().join("rewrite/golden/replays");
    if !data.join("tyrian.hdt").exists() || !dir.exists() {
        eprintln!("data or replays missing: test skipped");
        return;
    }
    let mut reps: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rep"))
        .collect();
    reps.sort();
    assert!(!reps.is_empty(), "no .rep in {}", dir.display());
    for rep in reps {
        let gm_path = rep.with_extension("play.gm");
        let gm = GoldenMaster::load(&gm_path).unwrap();
        let name = rep.file_name().unwrap().to_string_lossy().to_string();
        let (r, _) = compare_replay(&rep, &gm, &data, &name).unwrap();
        assert_eq!(r.matched_prefix, r.ticks, "{name}: divergence {:?}", r.first_divergence);
        assert!(r.end_matches, "{name}: different end (C: {})", gm.end_reason);
    }
}

/// One-player arcade (`game_mode` 1): the file rewritten by `Replay::write`
/// (after the corrections of `replay_load`) reads back, and its replay gives the same state fingerprint at each tick
/// as the replay of the original file (recording → writing → replay).
#[test]
fn arcade_one_player_replay_roundtrip_gives_identical_hashes() {
    let data = root().join("data");
    let rep_path = root().join("rewrite/golden/replays/e1-arcade-sweep.rep");
    if !data.join("tyrian.hdt").exists() || !rep_path.exists() {
        eprintln!("data or replay missing: test skipped");
        return;
    }
    let bytes = std::fs::read(&rep_path).unwrap();
    let original = tyrian_formats::replay::Replay::load(&bytes).unwrap();
    assert_eq!(original.game_mode, 1, "the file must be a one-player arcade replay");
    let rewritten = original.write();
    let mut a = tyrian_sim::replay::ReplayRun::new(&data, original).unwrap();
    let mut b =
        tyrian_sim::replay::ReplayRun::new(&data, tyrian_formats::replay::Replay::load(&rewritten).unwrap()).unwrap();
    assert!(a.sim.world().one_player_action, "onePlayerAction must be set");
    let mut ticks = 0;
    while a.step() {
        assert!(b.step());
        assert_eq!(a.sim.state_hash(), b.sim.state_hash(), "different hash at tick {ticks}");
        ticks += 1;
    }
    assert!(!b.step());
    assert!(ticks > 1000);
}
