//! Reading and validation of the files of a mod folder, independent of
//! the episode: `mod.json` (bounded JSON5), PNG inventory (header checked),
//! README. Nothing here depends on the game tables.

use crate::json::{self, Value};
use crate::Diagnostics;
use std::path::{Path, PathBuf};

/// `mod_manager.h:MAX_MODS`.
pub const MAX_MODS: usize = 32;
/// `mod_manager.h:MOD_README_MAX` (1,023 characters + NUL).
pub const MOD_README_MAX: usize = 1023;
/// Maximum size of a mod PNG (the largest shipped sheet is ~20 KiB).
pub const MAX_PNG_BYTES: u64 = 4 << 20;
/// Maximum dimensions accepted for a PNG (game sprites are at most 300 px).
pub const MAX_PNG_SIDE: u32 = 2048;
/// PNGs at most per category folder.
pub const MAX_PNG_PER_DIR: usize = 1024;

/// Subfolders of `sprites/` read by `mod_manager.c:load_mod_sprites`, in
/// the same order, with the override table they feed.
pub const SPRITE_DIRS: [(&str, SpriteTable); 13] = [
    ("shots", SpriteTable::Shots),
    ("ships", SpriteTable::Ships),
    ("sidekicks", SpriteTable::Sidekicks),
    ("pickups", SpriteTable::Pickups),
    ("pickups/weapons", SpriteTable::ShopWeapons),
    ("pickups/shields", SpriteTable::ShopShields),
    ("pickups/specials", SpriteTable::Pickups),
    ("coins", SpriteTable::Coins),
    ("weapon_icons", SpriteTable::WeaponIcons),
    ("explosions", SpriteTable::Explosions),
    ("portraits", SpriteTable::Portraits),
    ("planets", SpriteTable::Planets),
    // Enemy PNGs: `sprites/enemies/<enemy name>.png` (rewrite/mods/original-names.json),
    // inventoried for Godot rendering.
    ("enemies", SpriteTable::Enemies),
];

/// Ship slots the mods share (`SHIP_RANGE` 19..=`SHIP_NUM` in `resolve`).
pub const MOD_SHIP_SLOTS: i32 = 14;
/// Images of one ship: neutral, half and full bank to each side (`ship_poses`).
pub const SHIP_POSES: i32 = 5;
/// Step between two automatically placed ship images: the engine copies each
/// neutral image to -4, -2, +2 and +4 around it (`resolve/sprites.rs`).
pub const SHIP_IMAGE_STEP: i32 = 5;
/// First automatically placed ship image (`sprite_override.c:table_auto_base`).
const SHIP_AUTO_BASE: i32 = 805;
/// Capacity of the ship image table (ADR 0134): every mod ship slot can hold
/// its five poses as five PNGs (14 × 5 × 5 = 350 places from 805), plus the
/// +4 pose copy around the last one. The former 1024 left 44 places (8.8 ships).
const SHIP_TABLE_CAPACITY: i32 = SHIP_AUTO_BASE + MOD_SHIP_SLOTS * SHIP_POSES * SHIP_IMAGE_STEP + 5;

/// Override tables of `sprite_override.c` (capacity, automatic assignment base
/// `table_auto_base`, manifest section `table_manifest_section`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpriteTable {
    Shots,
    Ships,
    Sidekicks,
    ShopWeapons,
    ShopShields,
    Pickups,
    Coins,
    Explosions,
    WeaponIcons,
    Portraits,
    Planets,
    Enemies,
}

impl SpriteTable {
    /// `sprite_override.c:sprite_override_init` (`table_init`).
    pub fn capacity(self) -> i32 {
        match self {
            SpriteTable::Shots => 1024,
            SpriteTable::Ships => SHIP_TABLE_CAPACITY,
            SpriteTable::ShopShields | SpriteTable::Planets => 256,
            SpriteTable::Portraits => 32,
            SpriteTable::Enemies => 608,
            _ => 512,
        }
    }

    /// `sprite_override.c:table_auto_base`.
    pub fn auto_base(self) -> i32 {
        match self {
            SpriteTable::Shots => 800,
            SpriteTable::Ships => SHIP_AUTO_BASE,
            SpriteTable::Sidekicks => 300,
            SpriteTable::ShopWeapons => 250,
            SpriteTable::ShopShields => 50,
            SpriteTable::Pickups => 300,
            SpriteTable::Coins => 150,
            SpriteTable::WeaponIcons => 100,
            SpriteTable::Portraits => 18,
            SpriteTable::Planets => 152,
            _ => 300,
        }
    }

    /// `sprite_override.c:table_manifest_section`.
    pub fn section(self) -> &'static str {
        match self {
            SpriteTable::Shots => "shots",
            SpriteTable::Ships => "ships",
            SpriteTable::Sidekicks => "sidekicks",
            SpriteTable::ShopWeapons | SpriteTable::ShopShields | SpriteTable::Pickups => "pickups",
            SpriteTable::Coins => "coins",
            SpriteTable::Explosions => "explosions",
            SpriteTable::WeaponIcons => "weapon_icons",
            SpriteTable::Portraits => "portraits",
            SpriteTable::Planets => "planets",
            SpriteTable::Enemies => "",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SpriteTable::Shots => "shots",
            SpriteTable::Ships => "ships",
            SpriteTable::Sidekicks => "sidekicks",
            SpriteTable::ShopWeapons => "shop_weapons",
            SpriteTable::ShopShields => "shop_shields",
            SpriteTable::Pickups => "pickups",
            SpriteTable::Coins => "coins",
            SpriteTable::Explosions => "explosions",
            SpriteTable::WeaponIcons => "weapon_icons",
            SpriteTable::Portraits => "portraits",
            SpriteTable::Planets => "planets",
            SpriteTable::Enemies => "enemies",
        }
    }
}

/// A PNG found in a mod folder (header validated, pixels not decoded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngFile {
    pub path: PathBuf,
    /// Subfolder of `sprites/` ("shots", "pickups/weapons", "enemies/4"…).
    pub dir: String,
    /// Name without extension.
    pub basename: String,
    pub width: u32,
    pub height: u32,
}

/// Mod folder read from disk.
#[derive(Debug, Clone, Default)]
pub struct ModFolder {
    /// Name of the folder (identity key and load order).
    pub folder: String,
    pub path: PathBuf,
    /// Parsed `mod.json`; None if absent or invalid (the mod stays listed, in error).
    pub root: Option<Value>,
    pub enabled: bool,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub readme: String,
    pub pngs: Vec<PngFile>,
    /// Error message shown in the mod list (`ModInfo.error`).
    pub error: String,
    /// Translation key of `error` (locales `mod_err_*`) and its arguments,
    /// for the game menus; `error` stays the text of the `check` tool.
    pub error_key: String,
    pub error_args: Vec<String>,
}

/// Resolves the links then requires a target inside the mod root.
pub fn contained_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let target = path.canonicalize().map_err(|e| e.to_string())?;
    if !target.starts_with(&root) {
        return Err(format!("{} leaves the mod root", path.display()));
    }
    Ok(target)
}

/// Check before opening (FIFO) then on the handle actually read.
fn open_regular(path: &Path) -> Result<std::fs::File, String> {
    if !std::fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err("regular file required".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    // Avoids blocking if a file is replaced by a FIFO between the checks.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x800); // O_NONBLOCK on Linux.
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("regular file required".into());
    }
    Ok(file)
}

/// Reads a text file, refusing beyond `max` bytes.
pub fn read_text_bounded(path: &Path, max: usize) -> Result<String, String> {
    use std::io::Read;
    let file = open_regular(path)?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if meta.len() > max as u64 {
        return Err(format!("{} bytes, maximum {max}", meta.len()));
    }
    // The file may grow or announce zero bytes: bound the effective read.
    let mut bytes = Vec::new();
    file.take((max as u64).saturating_add(1)).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err(format!("file too large, maximum {max} bytes"));
    }
    if bytes.is_empty() {
        return Err("empty file".into());
    }
    String::from_utf8(bytes).map_err(|_| "non-UTF-8 file".to_string())
}

/// Checks the signature and the IHDR header of a PNG; returns (width, height).
pub fn png_header(path: &Path) -> Result<(u32, u32), String> {
    use std::io::Read;
    let mut f = open_regular(path)?;
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if meta.len() > MAX_PNG_BYTES {
        return Err(format!("PNG too large ({} bytes, maximum {MAX_PNG_BYTES})", meta.len()));
    }
    let mut head = [0u8; 24];
    f.read_exact(&mut head).map_err(|_| "truncated PNG".to_string())?;
    if head[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] || &head[12..16] != b"IHDR" {
        return Err("invalid PNG signature".into());
    }
    let w = u32::from_be_bytes([head[16], head[17], head[18], head[19]]);
    let h = u32::from_be_bytes([head[20], head[21], head[22], head[23]]);
    if w == 0 || h == 0 || w > MAX_PNG_SIDE || h > MAX_PNG_SIDE {
        return Err(format!("dimensions {w}×{h} out of bounds (1..{MAX_PNG_SIDE})"));
    }
    Ok((w, h))
}

fn list_pngs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("png" | "PNG")))
        .collect();
    // readdir has no guaranteed order in C; we sort for a reproducible result.
    v.sort();
    v
}

fn str_field(root: &Value, key: &str, def: &str) -> String {
    root.get(key).and_then(Value::as_str).unwrap_or(def).to_string()
}

impl ModFolder {
    /// `mod_manager.c:load_single_mod`, reading part: an unreadable mod stays
    /// listed with an error, like in the Setup → Mods menu.
    pub fn read(path: &Path, folder: &str, diag: &mut Diagnostics) -> ModFolder {
        let mut m = ModFolder {
            folder: folder.to_string(),
            path: path.to_path_buf(),
            root: None,
            enabled: false,
            name: folder.to_string(),
            author: "Unknown".into(),
            version: "1.0".into(),
            description: String::new(),
            readme: String::new(),
            pngs: Vec::new(),
            error: String::new(),
            error_key: String::new(),
            error_args: Vec::new(),
        };
        let text = match contained_path(path, &path.join("mod.json"))
            .and_then(|p| read_text_bounded(&p, json::MAX_TEXT_BYTES))
        {
            Ok(t) => t,
            Err(e) => {
                m.error = format!("mod.json missing or unreadable ({e})");
                m.error_key = "mod_err_unreadable".into();
                diag.error(folder, m.error.clone());
                return m;
            }
        };
        let root = match json::parse(&text) {
            Ok(v) if v.as_object().is_some() => v,
            Ok(v) => {
                m.error = format!("mod.json: object expected at the root, found {}", v.type_name());
                m.error_key = "mod_err_not_object".into();
                diag.error(folder, m.error.clone());
                return m;
            }
            Err(e) => {
                m.error = format!("mod.json: syntax error {e}");
                m.error_key = "mod_err_syntax".into();
                diag.error(folder, m.error.clone());
                return m;
            }
        };
        m.name = str_field(&root, "name", folder);
        // A single format (ADR 0083): a mod.json without "format_version" is a
        // C-version mod; it is listed in error, never interpreted.
        match root.get("format_version").and_then(Value::as_c_int) {
            Some(crate::FORMAT_VERSION) => {}
            Some(v) => {
                m.error = format!("unknown format_version {v} (this engine reads version {})", crate::FORMAT_VERSION);
                m.error_key = "mod_err_format_unknown".into();
                m.error_args = vec![v.to_string(), crate::FORMAT_VERSION.to_string()];
                diag.error(folder, m.error.clone());
                return m;
            }
            None => {
                m.error =
                    "mod.json without \"format_version\": 1 — mod for the C version, must be converted (MODDING.md §11)".into();
                m.error_key = "mod_err_c_format".into();
                diag.error(folder, m.error.clone());
                return m;
            }
        }
        m.enabled = root.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        m.author = str_field(&root, "author", "Unknown");
        m.version = str_field(&root, "version", "1.0");
        m.description = str_field(&root, "description", "");
        for name in ["README.md", "README.txt", "about.txt"] {
            if !path.join(name).exists() {
                continue;
            }
            let read = (|| -> Result<Vec<u8>, String> {
                use std::io::Read;
                let p = contained_path(path, &path.join(name))?;
                let mut bytes = Vec::new();
                open_regular(&p)?.take(MOD_README_MAX as u64).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                Ok(bytes)
            })();
            match read {
                Ok(bytes) => {
                    m.readme = String::from_utf8_lossy(&bytes).into_owned();
                    break;
                }
                Err(e) => diag.warn(folder, format!("{name} ignored: {e}")),
            }
        }
        for (dir, table) in SPRITE_DIRS {
            if table == SpriteTable::Enemies {
                // A single convention (ADR 0128): sprites/enemies/<name>.png. A
                // subfolder (old <bank>/<image>.png convention) is an error.
                let base = path.join("sprites").join("enemies");
                if let Ok(rd) = std::fs::read_dir(&base) {
                    let mut subdirs: Vec<String> = rd
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .filter_map(|e| e.file_name().to_str().map(str::to_string))
                        .collect();
                    subdirs.sort();
                    if let Some(d) = subdirs.first() {
                        diag.error(folder, format!("sprites/enemies/{d}/: an enemy PNG is named after the enemy, sprites/enemies/<name>.png (rewrite/mods/original-names.json), without a subfolder"));
                    }
                }
            }
            m.add_pngs(&path.join("sprites").join(dir), dir, diag);
        }
        m.root = Some(root);
        m
    }

    fn add_pngs(&mut self, dir: &Path, rel: &str, diag: &mut Diagnostics) {
        if !dir.exists() {
            return;
        }
        let dir = match contained_path(&self.path, dir) {
            Ok(p) => p,
            Err(e) => {
                diag.warn(&self.folder, e);
                return;
            }
        };
        let files = list_pngs(&dir);
        if files.len() > MAX_PNG_PER_DIR {
            diag.warn(
                &self.folder,
                format!("sprites/{rel}: {} PNGs, only the first {MAX_PNG_PER_DIR} are read", files.len()),
            );
        }
        for p in files.into_iter().take(MAX_PNG_PER_DIR) {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            // sprite_override.c: file name < 5 characters ignored, basename cut at 63.
            if stem.is_empty() {
                continue;
            }
            let p = match contained_path(&self.path, &p) {
                Ok(p) => p,
                Err(e) => {
                    diag.warn(&self.folder, e);
                    continue;
                }
            };
            match png_header(&p) {
                Ok((width, height)) => {
                    let mut basename = stem;
                    while basename.len() > 63 {
                        basename.pop();
                    }
                    self.pngs.push(PngFile { path: p, dir: rel.to_string(), basename, width, height });
                }
                Err(e) => diag.warn(
                    &self.folder,
                    format!("sprites/{rel}/{} ignored: {e}", p.file_name().and_then(|s| s.to_str()).unwrap_or("?")),
                ),
            }
        }
    }
}

/// Subfolders of a mods folder containing a `mod.json`, sorted by name
/// (byte order, like `mod_manager.c`), at most [`MAX_MODS`].
pub fn scan_mods_dir(mods_dir: &Path, diag: &mut Diagnostics) -> Vec<ModFolder> {
    let mut names: Vec<String> = list_mod_dirs(mods_dir).into_iter().map(|(n, _)| n).collect();
    if names.len() > MAX_MODS {
        diag.warn(
            "",
            format!("more than {MAX_MODS} mod folders; '{}' and the following ones ignored", names[MAX_MODS]),
        );
        names.truncate(MAX_MODS);
    }
    names.iter().map(|n| ModFolder::read(&mods_dir.join(n), n, diag)).collect()
}

/// Subfolders of `mods_dir` that contain a `mod.json`, sorted by name.
pub fn list_mod_dirs(mods_dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| !n.starts_with('.'))
        .filter(|n| mods_dir.join(n).join("mod.json").is_file())
        .collect();
    names.sort();
    names.into_iter().map(|n| (n.clone(), mods_dir.join(n))).collect()
}
