//! Chained Endless waves: C game (c-final, `make GOLDEN=1`) on autopilot,
//! seed 7, three waves (SAVARA V, ASTEROID?, SAVARA), recorded
//! by `--goto=endless GOLDEN_INPUT=autopilot GOLDEN_SEED=7 GOLDEN_CONTINUE=2
//! GOLDEN_LEVELS=3` in `rewrite/golden/endless-chain/endless-chain-seed7-K.gm`.
//! Each wave restarts from the state left by the previous one
//! (`GoldenMaster::load_chained`); the special fire held at the end of a wave
//! (`varz.c:fireButtonHeld`) must carry the special weapon over to the first tick of the
//! next one, as in the C code.

use std::path::{Path, PathBuf};
use tyrian_golden::{compare, GoldenMaster, GROUPS};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn chained_endless_waves_match_c() {
    let data = root().join("data");
    let dir = root().join("rewrite/golden/endless-chain");
    if !data.join("tyrian.hdt").exists() || !dir.exists() {
        eprintln!("data or chained Endless golden masters missing: test skipped");
        return;
    }
    for k in 1..=3 {
        let f = dir.join(format!("endless-chain-seed7-{k}.gm"));
        let gm = GoldenMaster::load_chained(&f, &data).expect("golden master");
        let e = gm.header.endless.expect("endless line");
        assert_eq!(e.wave, k, "{}: wave", f.display());
        assert_eq!(gm.carry.is_some(), k > 1, "{}: state carried over from the previous wave", f.display());
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let r = compare(&gm, &data, &name, |_, _, _| {}).expect("comparison");
        for g in GROUPS {
            assert_eq!(r.first_divergence[g], None, "{name}: group {g}");
        }
        assert!(r.end_matches && r.ended_early_at.is_none(), "{name}: level end");
    }
}

/// Without carrying over the held special fire, the second wave diverges from tick 0
/// (two special shots too many): the test above does not pass by chance.
#[test]
fn special_fire_held_is_what_carries_over() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/endless-chain/endless-chain-seed7-2.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        return;
    }
    let mut gm = GoldenMaster::load_chained(&f, &data).expect("golden master");
    let carry = gm.carry.as_mut().expect("carried-over state");
    assert!(carry.special_fire_held, "the pilot holds fire at the end of wave 1");
    carry.special_fire_held = false;
    let r = compare(&gm, &data, "without carry-over", |_, _, _| {}).expect("comparison");
    assert_eq!(r.first_divergence["pshots"], Some(0));
}
