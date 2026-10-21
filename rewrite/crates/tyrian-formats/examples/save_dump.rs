//! Inspection of C saves: `cargo run --release -p tyrian-formats --example save_dump -- FILE...`
//!
//! For each file: read, byte-exact round-trip (container and `GAME`
//! section), then list of the used slots. Exit code 1 if a
//! round-trip differs.

use tyrian_formats::save::{load_any, OtDat};

fn main() {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                println!("{path}: {e}");
                continue;
            }
        };
        let rt = match OtDat::parse(&bytes) {
            Ok(dat) => {
                let same = dat.write() == bytes;
                let game_same =
                    dat.section(b"GAME").map(|g| tyrian_formats::save::GameSave::parse(g).map(|s| s.write() == g));
                format!("container {} GAME {:?}", if same { "identical" } else { "DIFFERENT" }, game_same)
            }
            Err(e) => format!("unreadable container ({e})"),
        };
        if rt.contains("DIFF") || rt.contains("Ok(false)") {
            failed = true;
        }
        match load_any(&bytes) {
            Ok(save) => {
                let used: Vec<String> = save
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.is_used())
                    .map(|(i, s)| {
                        format!(
                            "#{} '{}' ep{} sect{} '{}' cash {} diff {}",
                            i + 1,
                            s.name().trim_end(),
                            s.episode,
                            s.level,
                            s.level_name(),
                            s.score,
                            s.difficulty
                        )
                    })
                    .collect();
                println!("{path}: {rt}; {} slot(s) {}", used.len(), used.join(" | "));
            }
            Err(e) => println!("{path}: {rt}; unreadable GAME ({e})"),
        }
    }
    std::process::exit(failed as i32);
}
