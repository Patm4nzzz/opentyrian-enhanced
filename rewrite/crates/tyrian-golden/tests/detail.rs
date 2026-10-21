//! Effects level (Setup > Graphics > Visual FX, `config.c:processorType`):
//! the C golden masters recorded by `c-final:utils/golden/record_detail.sh`
//! (`rewrite/golden/detail/<scenario>.dN.gm`, "detail N" line) must be
//! replayed identically by `TyrianSim.set_processor_type`. This is the proof
//! of the branches that depend on processorType: cascade draws of
//! `hitstop.c:on_boss_defeated`, layer 2 of `tyrian2.c` (background_2_allowed),
//! stars of `backgrnd.c:initialize_starfield`.

use std::path::PathBuf;
use tyrian_golden::{compare, GoldenMaster};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn detail_goldens_match_at_their_processor_type() {
    let data = root().join("data");
    let dir = root().join("rewrite/golden/detail");
    if !data.join("tyrian.hdt").exists() || !dir.exists() {
        eprintln!("data or 'detail' golden masters missing: test skipped");
        return;
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gm"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .gm in {}", dir.display());
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let gm = GoldenMaster::load(&f).unwrap();
        assert!(gm.header.detail != 4, "{name}: detail golden master at the default 4?");
        let got = compare(&gm, &data, &name, |_, _, _| {}).unwrap();
        assert_eq!(
            got.matched_prefix, got.ticks,
            "{name} (detail {}): divergence {:?}",
            gm.header.detail, got.first_divergence
        );
        assert!(got.end_matches, "{name}: different level end");
    }
}
