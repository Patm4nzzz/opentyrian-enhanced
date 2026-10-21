//! Real saves of the C version (copies of the `opentyrian.dat` found
//! in WSL, see `tests/saves/README.md`): reading, byte-exact round-trip
//! of the container and of the `GAME` section, decoding of the slots.

use std::fs;
use std::path::PathBuf;

use tyrian_formats::save::{load_any, GameSave, OtDat, SAVE_FILES_NUM};

fn saves() -> Vec<(String, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/saves");
    let mut v: Vec<_> = fs::read_dir(&dir)
        .expect("tests/saves")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "dat"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), fs::read(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

#[test]
fn c_saves_round_trip_byte_exact() {
    let all = saves();
    assert!(all.len() >= 3, "missing fixtures");
    for (name, bytes) in all {
        let dat = OtDat::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(dat.sections.iter().all(|s| s.is_valid()), "{name}: invalid section");
        assert_eq!(dat.write(), bytes, "{name}: rewritten container differs");
        let game = dat.section(b"GAME").expect("section GAME");
        let save = GameSave::parse(game).unwrap_or_else(|e| panic!("{name} GAME: {e}"));
        assert_eq!(save.slots.len(), SAVE_FILES_NUM);
        assert_eq!(save.write(), game, "{name}: rewritten GAME section differs");
        // Full rewrite through the "modified" path: the GAME section is
        // rebuilt from the structure, the container recomputes its digests.
        let mut dat2 = dat.clone();
        dat2.set_section(b"GAME", save.write(), true);
        assert_eq!(dat2.write(), bytes, "{name}: rebuild differs");
        assert_eq!(load_any(&bytes).unwrap(), save);
    }
}

#[test]
fn c_save_contents_decode() {
    for (name, bytes) in saves() {
        let save = load_any(&bytes).unwrap();
        for (i, s) in save.slots.iter().enumerate() {
            if s.is_used() {
                println!(
                    "{name} slot {:2}: '{}' section {} episode {} level '{}' cash {} diff {} ship {} front {}",
                    i + 1,
                    s.name(),
                    s.level,
                    s.episode,
                    s.level_name(),
                    s.score,
                    s.difficulty,
                    s.items[11],
                    s.items[0]
                );
            }
        }
        for (t, table) in save.high_scores.iter().enumerate().skip(10) {
            println!(
                "{name} scores table {t}: {}",
                table.iter().map(|e| format!("{} {}", e.score, e.name())).collect::<Vec<_>>().join(" / ")
            );
        }
        println!("{name} editorLevel {} stats {:?}", save.editor_level(), save.enhanced_stats());
        // The campaign tables carry the constant written by the C code.
        assert!(save.high_scores[10..].iter().flatten().all(|e| e.unknown == Some(0x1234_5678)), "{name}");
    }
}
