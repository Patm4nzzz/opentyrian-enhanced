//! Loads a level from the original `data/` directory.

use crate::{LevelSetup, Loadout};
use std::path::Path;
use tyrian_formats::hdt::{HdtFile, ItemTables};
use tyrian_formats::lvl::LvlFile;

/// Reads `tyrianN.lvl` and the episode's item tables, then builds the
/// `LevelSetup` of level `lvl_file_num` (1-based, `]L[` column of levelsN.dat).
///
/// Tables: episodes 1 to 3 → `tyrian.hdt`; episodes 4 and 5 → item block at the
/// end of `tyrianN.lvl` (episodes.c:JE_loadItemDat).
pub fn load_level(
    data_dir: &Path,
    episode: u8,
    section: u8,
    lvl_file_num: usize,
    difficulty: u8,
    loadout: Loadout,
    seed: u64,
) -> Result<LevelSetup, String> {
    let lvl_path = data_dir.join(format!("tyrian{episode}.lvl"));
    let bytes = std::fs::read(&lvl_path).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
    let lvl = LvlFile::parse(&bytes).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
    let level =
        lvl.level(lvl_file_num).ok_or_else(|| format!("{}: no level {lvl_file_num}", lvl_path.display()))?.clone();
    let items: ItemTables = match lvl.item_tables() {
        Some(t) => t.map_err(|e| e.to_string())?,
        None => {
            let hdt_path = data_dir.join("tyrian.hdt");
            let b = std::fs::read(&hdt_path).map_err(|e| format!("{}: {e}", hdt_path.display()))?;
            HdtFile::parse(&b).map_err(|e| e.to_string())?.items
        }
    };
    Ok(LevelSetup {
        episode,
        section,
        difficulty,
        level,
        items,
        loadout,
        seed,
        assist: false,
        initial_difficulty: None,
        second_player_loadout: None,
        carry_over: None,
        modes: Default::default(),
        endless_armor_mult: None,
        patch: Default::default(),
        // config.c:JE_loadConfiguration — factory default 4 (Wild / Extended).
        processor_type: 4,
    })
}
