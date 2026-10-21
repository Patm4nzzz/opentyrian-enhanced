//! Very short version of the bot (a few seconds) run by `cargo test`,
//! including games 0 and 1 with out-of-table equipment: they used to panic in
//! `Sim::new` (level.rs, player.rs) before the indices were bounded.

use std::path::Path;
use tyrian_robustness::{play, Catalogue};

/// `None` if the game data is not in the tree (same rule as
/// `tyrian-golden/tests/parity.rs`).
fn catalogue() -> Option<Catalogue> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    if !data.join("tyrian.hdt").exists() {
        eprintln!("data missing: test skipped");
        return None;
    }
    Some(Catalogue::load(&data, &[1, 2, 3, 4, 5]).expect("repository data/"))
}

#[test]
fn random_games_do_not_panic_and_replay_identically() {
    let Some(cat) = catalogue() else { return };
    for wild in [false, true] {
        for id in 0..24 {
            let a = play(id, &cat, 600, wild);
            assert!(a.panic.is_none(), "game {id} (wild={wild}): {:?}", a.panic);
            assert!(a.invariant.is_none(), "game {id} (wild={wild}): {:?}", a.invariant);
            let b = play(id, &cat, 600, wild);
            assert_eq!(a, b, "game {id} (wild={wild}) not reproducible");
        }
    }
}
