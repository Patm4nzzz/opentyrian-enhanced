//! End-of-level summary (`mainint.c:JE_endLevelAni`, "Level Statistics
//! Screen", Enhanced addition): `stats.c:level_stats` sampled on the C Enhanced
//! binary (instrumented copy, `golden_master_level_end` writes
//! `level_stats`; local traces, not versioned) for the
//! same golden masters, compared to `World::level_stats` after the replay.

use std::path::{Path, PathBuf};
use tyrian_golden::{compare, GoldenMaster};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// (golden master, damage_by_bay, damage_taken, damage_blocked, credits_start) of the C code.
const C_VALUES: [(&str, [u32; 5], u32, u32, u64); 4] = [
    ("e1-tyrian-autopilot.gm", [2397, 0, 0, 0, 0], 0, 0, 10000),
    ("e1-tyrian-script.gm", [69, 0, 0, 0, 0], 39, 27, 10000),
    ("e1-option-plasma-pulse.gm", [626, 0, 111, 125, 0], 222, 202, 10000),
    ("e1-special-zinglon-pulse.gm", [923, 0, 0, 0, 0], 157, 137, 10000),
];

#[test]
fn level_stats_match_c_enhanced() {
    let data = root().join("data");
    if !data.join("tyrian.hdt").exists() {
        eprintln!("data missing: test skipped");
        return;
    }
    for (name, bays, taken, blocked, start) in C_VALUES {
        let gm = GoldenMaster::load(&root().join("rewrite/golden").join(name)).unwrap();
        let mut last = None;
        let r = compare(&gm, &data, name, |_, _, sim| last = Some(sim.world().level_stats)).unwrap();
        assert_eq!(r.matched_prefix, r.ticks, "{name}: replay");
        let ls = last.unwrap();
        assert_eq!(ls.damage_by_bay, bays, "{name}: damage per bay");
        assert_eq!((ls.damage_taken, ls.damage_blocked), (taken, blocked), "{name}: damage taken / blocked");
        assert_eq!(ls.credits_start, start, "{name}: starting money");
    }
}
