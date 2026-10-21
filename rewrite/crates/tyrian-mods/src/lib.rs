//! Mod system of the rewrite (blocks 8 and 16, ADR 0080 to 0082).
//!
//! A single mod format (ADR 0083, `"format_version": 1`): everything a
//! mod creates — weapons, ships, wingmen, shields, specials, enemy
//! weapons, enemies — is referred to by name, and the loader assigns the
//! slots in the game tables. Enemy behaviors and mod
//! levels are described as data. Mods of the C version
//! (without `format_version`) are not read: they are converted
//! once into `rewrite/mods`.
//!
//! Every mod file is untrusted data: bounded reads
//! ([`json`], [`folder`]), no panic, out-of-domain values rejected or
//! clamped with a diagnostic that names the mod and the field.
//!
//! Layout:
//! - [`ModSet::load`] reads the folders once (independent of the episode);
//! - [`ModSet::resolve`] replays the C loader on the tables of an episode and
//!   produces a [`ResolvedMods`]: table patch for `tyrian-sim`,
//!   sprites to load by Godot, shop lists, starting equipment;
//! - [`ResolvedMods::apply_to`] injects it into a `LevelSetup`.

pub mod folder;
pub mod json;
pub mod levels;
pub mod manifest;
pub mod originals;
mod resolve;

pub use folder::{list_mod_dirs, read_text_bounded, scan_mods_dir, ModFolder, PngFile, SpriteTable, MAX_MODS};
pub use resolve::{ItemKind, ModReport, ResolvedMods, SpriteAssignment, StarterLoadout};

use manifest::SpriteNames;
use std::path::{Path, PathBuf};

/// Only mod format version read by the rewrite (ADR 0083). A
/// `mod.json` without `"format_version"` is a C-version mod: rejected.
pub const FORMAT_VERSION: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Folder of the mod concerned ("" = global).
    pub folder: String,
    pub message: String,
}

/// Load log (equivalent of the C `log_warn`s, made visible
/// in the `tyrian-mods check` tool and in the mods menu).
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Log cap: a hostile mod must not be able to make it grow without end.
    pub const MAX: usize = 4096;

    fn push(&mut self, severity: Severity, folder: &str, message: String) {
        if self.items.len() < Self::MAX {
            self.items.push(Diagnostic { severity, folder: folder.to_string(), message });
        }
    }
    pub fn info(&mut self, folder: &str, message: impl Into<String>) {
        self.push(Severity::Info, folder, message.into());
    }
    pub fn warn(&mut self, folder: &str, message: impl Into<String>) {
        self.push(Severity::Warning, folder, message.into());
    }
    pub fn error(&mut self, folder: &str, message: impl Into<String>) {
        self.push(Severity::Error, folder, message.into());
    }
    pub fn count(&self, severity: Severity) -> usize {
        self.items.iter().filter(|d| d.severity == severity).count()
    }
    pub fn for_folder<'a>(&'a self, folder: &'a str) -> impl Iterator<Item = &'a Diagnostic> + 'a {
        self.items.iter().filter(move |d| d.folder == folder)
    }
}

/// Set of installed mods, read from disk.
#[derive(Debug, Clone)]
pub struct ModSet {
    pub data_dir: PathBuf,
    pub folders: Vec<ModFolder>,
    /// Original manifest (`data/sprite_manifest.json`), before the mod names.
    pub manifest: SpriteNames,
    /// Original enemies and weapons by name (tables of episodes 1 and 4).
    pub originals: originals::OriginalNames,
    pub diagnostics: Diagnostics,
}

impl ModSet {
    /// A single mods folder (tests, `tyrian-mods` tool).
    pub fn load_from(data_dir: &Path, mods_dir: &Path) -> ModSet {
        Self::load_dirs(data_dir, &[mods_dir.to_path_buf()])
    }

    /// Several mods folders (ADR 0080, 0083): the player's `user://mods`
    /// and `rewrite/mods` (mods shipped with the rewrite).
    /// All folders found are merged then sorted by name, like the
    /// alphabetical sort of `mod_manager_load`; with equal names, the first
    /// folder of the list wins (warning). At most `MAX_MODS` mods.
    pub fn load_dirs(data_dir: &Path, mods_dirs: &[PathBuf]) -> ModSet {
        let mut diagnostics = Diagnostics::default();
        let manifest = SpriteNames::load(data_dir, &mut diagnostics);
        let originals = match (episode_items(data_dir, 1), episode_items(data_dir, 4)) {
            (Ok((_, a)), Ok((_, b))) => originals::OriginalNames::generate([&a, &b], &manifest),
            (Err(e), _) | (_, Err(e)) => {
                diagnostics.warn(
                    "",
                    format!("original tables unreadable ({e}): no original enemy or weapon can be referenced"),
                );
                originals::OriginalNames::default()
            }
        };
        let mut folders: Vec<ModFolder> = Vec::new();
        for dir in mods_dirs {
            for (name, path) in list_mod_dirs(dir) {
                if folders.iter().any(|m| m.folder == name) {
                    diagnostics.warn(&name, format!("duplicate mod folder ({}), ignored", path.display()));
                    continue;
                }
                folders.push(ModFolder { folder: name, path, ..Default::default() });
            }
        }
        folders.sort_by(|a, b| a.folder.cmp(&b.folder));
        if folders.len() > MAX_MODS {
            diagnostics.warn(
                "",
                format!(
                    "more than {MAX_MODS} mod folders; '{}' and the following ones ignored",
                    folders[MAX_MODS].folder
                ),
            );
            folders.truncate(MAX_MODS);
        }
        let folders = folders.into_iter().map(|m| ModFolder::read(&m.path, &m.folder, &mut diagnostics)).collect();
        ModSet { data_dir: data_dir.to_path_buf(), folders, manifest, originals, diagnostics }
    }

    /// Forces the enabled state of a mod (user preference on the Godot side,
    /// without rewriting the mod's `mod.json`). Returns false if the folder is unknown.
    pub fn set_enabled(&mut self, folder: &str, enabled: bool) -> bool {
        match self.folders.iter_mut().find(|m| m.folder == folder) {
            Some(m) => {
                m.enabled = enabled;
                true
            }
            None => false,
        }
    }

    pub fn enabled_count(&self) -> usize {
        self.folders.iter().filter(|m| m.enabled && m.root.is_some()).count()
    }
}

/// Level file and tables of the episode, like `episodes.c:JE_loadItemDat`:
/// `tyrian.hdt` for episodes 1 to 3, end block of `tyrianN.lvl` beyond.
pub fn episode_items(
    data: &Path,
    episode: u8,
) -> Result<(tyrian_formats::lvl::LvlFile, tyrian_formats::hdt::ItemTables), String> {
    use tyrian_formats::hdt::HdtFile;
    use tyrian_formats::lvl::LvlFile;
    let lvl_path = data.join(format!("tyrian{episode}.lvl"));
    let bytes = std::fs::read(&lvl_path).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
    let lvl = LvlFile::parse(&bytes).map_err(|e| e.to_string())?;
    let items = match lvl.item_tables() {
        Some(t) => t.map_err(|e| e.to_string())?,
        None => {
            let b = std::fs::read(data.join("tyrian.hdt")).map_err(|e| e.to_string())?;
            HdtFile::parse(&b).map_err(|e| e.to_string())?.items
        }
    };
    Ok((lvl, items))
}
