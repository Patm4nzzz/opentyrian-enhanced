//! Builds a test C save: `cargo run --release -p tyrian-formats --example make_test_save -- OUTPUT [SCORE]`
//!
//! `opentyrian.dat` container with a single fresh `GAME` section
//! (`GameSave::new_empty`, `initialItemAvail` from `config.c`) whose twenty
//! high score tables are all SCORE (default 99,999,999). Used for
//! campaign recordings: `JE_highScoreCheck` (end of episode) then never
//! opens the name entry, which the autopilot cannot
//! validate (`highscore.c`: `new_text` without characters does not leave the loop).
//! Without `SETT`, the C code keeps its default settings.

use tyrian_formats::save::{GameSave, OtDat, T2kHighScore};

/// `config.c:initialItemAvail`.
const INITIAL_ITEM_AVAIL: [u8; 100] = [
    1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, // weapons 1-38 (40)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, // padding (11)
    1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, // sidekicks 51-68 (18)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // padding (11)
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // specials 81-93 (15)
    0, 0, 0, 0, 0, // padding (5)
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.first().expect("OUTPUT");
    let score: i32 = args.get(1).map_or(99_999_999, |s| s.parse().expect("SCORE"));
    let mut game = GameSave::new_empty(&INITIAL_ITEM_AVAIL);
    for (t, table) in game.high_scores.iter_mut().enumerate() {
        for e in table.iter_mut() {
            *e = T2kHighScore::new(score, "TEST", 2, t >= 10);
        }
    }
    let mut dat = OtDat::new();
    dat.set_section(b"GAME", game.write(), true);
    std::fs::write(out, dat.write()).expect("write");
    println!("{out}: {} bytes", std::fs::metadata(out).unwrap().len());
}
