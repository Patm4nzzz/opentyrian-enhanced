//! `opentyrian.dat` actually written by the C binary (GOLDEN_ACH_CLOCK run of the
//! Impossible golden master, blank HOME): container read back and rewritten byte for
//! byte, OTYE block found in the GAME section, achievements unlocked = C log.

use std::path::PathBuf;
use tyrian_formats::achievements::{AchievementBlock, ACHIEVEMENTS};
use tyrian_formats::ot_dat::OtDat;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden/achievements")
}

#[test]
fn c_written_opentyrian_dat_round_trips_and_holds_unlocks() {
    let bytes = std::fs::read(dir().join("e1-tyrian-hard-autopilot.opentyrian.dat")).unwrap();
    let dat = OtDat::parse(&bytes).unwrap();
    assert_eq!(dat.write(), bytes, "byte-exact round trip of the container");
    let game = dat.section(b"GAME").expect("valid signed GAME section");
    let (pos, block) = AchievementBlock::find_in_game_section(game).expect("OTYE block");
    // Position computed like config.c:JE_loadConfiguration (saveTemp + T2K scores).
    assert_eq!(pos, AchievementBlock::GAME_OFFSET);
    assert_eq!(AchievementBlock::at_exact_position(game).as_ref(), Some(&block));
    // Block rewritten in place: GAME section unchanged.
    let mut g2 = game[..pos].to_vec();
    g2.extend_from_slice(&block.write());
    assert_eq!(g2, game);
    let names: Vec<&str> =
        (0..block.unlocked.len()).filter(|&i| block.unlocked[i]).map(|i| ACHIEVEMENTS[i].1).collect();
    let log = std::fs::read_to_string(dir().join("e1-tyrian-hard-autopilot.clock.unlocked.txt")).unwrap();
    let mut c: Vec<&str> = log.lines().collect();
    c.sort();
    let mut got = names.clone();
    got.sort();
    assert_eq!(got, c);
}
