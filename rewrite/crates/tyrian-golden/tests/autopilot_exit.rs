//! Regression D1/D2: DREAD-NOT (ep. 4) and ASSASSIN (ep. 1), recorded
//! in the C code with `GOLDEN_INPUT=autopilot`, end with the forced exit of the pilot
//! (`autopilot.c`: map frozen too long → `endLevel = true; levelEnd = 40`),
//! 30 ticks before the end. Without this abandonment, the sim keeps playing and diverges.

use std::path::PathBuf;
use tyrian_golden::{compare, GoldenMaster};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn strict(name: &str) {
    let data = root().join("data");
    let path = root().join("rewrite/golden").join(name);
    if !data.join("tyrian.hdt").exists() || !path.exists() {
        eprintln!("data missing: test skipped");
        return;
    }
    let gm = GoldenMaster::load(&path).unwrap();
    let r = compare(&gm, &data, name, |_, _, _| {}).expect("replay");
    assert_eq!(r.matched_prefix, r.ticks, "{name}: divergence, identical prefix {}/{}", r.matched_prefix, r.ticks);
    assert!(r.end_matches, "{name}: different level end");
}

#[test]
fn dreadnot_autopilot_exit_matches_c() {
    strict("e4-dreadnot-autopilot.gm");
}

#[test]
fn assassin_autopilot_exit_matches_c() {
    strict("e1-assassin-autopilot.gm");
}
