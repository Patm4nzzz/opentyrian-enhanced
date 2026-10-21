//! Cash earned in campaign (measurement of the "cash" workstream, 2026-10-07).
//!
//! The Enhanced C code multiplies the reward of a destroyed enemy by the current combo
//! (`collision.c:collision_process_kill_chain`, "evalue * (combo_enabled ?
//! combo_multiplier : 1)", combos active by default); the original C version
//! (`tyrian2.c`, "cash += evalue") has no combos. Decision: in
//! campaign, the rewrite keeps the gains of the original (Godot client:
//! `TyrianSim.set_combos` is only called in Endless); the combo keeps its
//! effect in Endless.
//!
//! Measurement (local traces, not versioned; level TYRIAN, seed 1,
//! inputs of `e1-tyrian-autopilot.gm`): original 14,978, C Enhanced with combos
//! off 14,983, C Enhanced with combos on (28 ms per loop) 19,807.

use std::path::{Path, PathBuf};
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Replays the golden master inputs; returns the final cash and the list of
/// (gain, combo multiplier at the time of the gain).
fn play(gm: &GoldenMaster, data: &Path, combo_ms: Option<u32>) -> (u64, Vec<(u64, u32)>) {
    let mut sim = gm.make_sim(data).expect("simulation");
    if let Some(ms) = combo_ms {
        sim.enable_combos(ms);
    }
    let mut cash = sim.world().players[0].cash;
    let mut gains = Vec::new();
    for t in &gm.ticks {
        if sim.state().level_ended {
            break;
        }
        let mult_before = sim.world().combo_multiplier;
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
        let c = sim.world().players[0].cash;
        if c != cash {
            gains.push((c - cash, mult_before.max(sim.world().combo_multiplier)));
            cash = c;
        }
    }
    (cash, gains)
}

#[test]
fn campaign_cash_is_not_multiplied_by_combos() {
    let data = root().join("data");
    let path = root().join("rewrite/golden/e1-tyrian-autopilot.gm");
    if !data.join("tyrian.hdt").exists() || !path.exists() {
        eprintln!("data or golden master missing: test skipped");
        return;
    }
    let gm = GoldenMaster::load(&path).unwrap();
    assert_eq!(gm.header.combo_frame_ms, None, "campaign golden master: combos off");
    // Campaign (Sim without enable_combos, like the client outside Endless): gains
    // of the original, each destruction pays its value, multiplier 1.
    let (cash, gains) = play(&gm, &data, None);
    let c_cash = gm.ticks.last().unwrap().cash;
    assert_eq!(cash, c_cash, "final money = C with combos off");
    assert_eq!(cash, 14_983);
    assert!(gains.iter().all(|&(_, m)| m == 1), "no multiplied gain in campaign");
    // Coins and enemies of TYRIAN: 2 to 250 credits (enemyDat.value).
    assert!(gains.iter().all(|&(g, _)| (2..=250 * 5).contains(&g)), "gains {gains:?}");
    // Forced combos (Endless, or C Enhanced): same game, multiplied cash,
    // value measured on the C Enhanced binary (GOLDEN_COMBO_MS=28).
    let (combo_cash, combo_gains) = play(&gm, &data, Some(28));
    assert_eq!(combo_cash, 19_807, "C Enhanced, combos on, same game");
    assert!(combo_gains.iter().any(|&(_, m)| m > 1));
}
