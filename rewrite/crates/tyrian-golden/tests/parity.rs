//! Parity non-regression: each golden master available in
//! `rewrite/golden/` is replayed; for each group, the first
//! divergent tick must never move back compared to `tests/baseline.json`
//! (`null` = identical over the whole file). Parity can therefore only
//! progress. A file absent from the baseline must only
//! replay without error.

use std::collections::BTreeMap;
use std::path::PathBuf;
use tyrian_golden::{compare, FileReport, GoldenMaster, GROUPS};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn parity_never_regresses() {
    let data = root().join("data");
    let golden = root().join("rewrite/golden");
    if !data.join("tyrian.hdt").exists() || !golden.exists() {
        eprintln!("data or golden masters missing: test skipped");
        return;
    }
    let baseline: Vec<FileReport> = serde_json::from_str(include_str!("baseline.json")).expect("invalid baseline.json");
    let base: BTreeMap<String, FileReport> = baseline.into_iter().map(|r| (r.file.clone(), r)).collect();

    let mut files: Vec<PathBuf> = std::fs::read_dir(&golden)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gm"))
        .collect();
    files.sort();
    let mut checked = 0;
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let gm = match GoldenMaster::load(&f) {
            Ok(g) => g,
            // File being written by the recorder: not usable yet.
            Err(_) => continue,
        };
        let got = compare(&gm, &data, &name, |_, _, _| {}).expect("replay");
        let Some(b) = base.get(&name) else { continue };
        if b.ticks != got.ticks {
            // Re-recorded golden master: the baseline no longer applies.
            continue;
        }
        for g in GROUPS {
            // Baseline predating the "mode" group: absent = identical.
            let before = b.first_divergence.get(g).copied().flatten().unwrap_or(u64::MAX);
            let now = got.first_divergence[g].unwrap_or(u64::MAX);
            assert!(now >= before, "{name}: group {g} now diverges at tick {now} (reference {before})");
        }
        assert!(
            got.matched_prefix >= b.matched_prefix,
            "{name}: identical prefix {} < {}",
            got.matched_prefix,
            b.matched_prefix
        );
        if b.end_matches {
            assert!(got.end_matches, "{name}: the level end no longer matches");
        }
        checked += 1;
    }
    eprintln!("{checked} golden masters checked against the baseline");
}
