//! Native campaign save (ADR 0129): one readable JSON file per slot,
//! `saves/slot_NN.json` in the user folder.
//!
//! The format is specific to the rewrite: it reuses neither the encryption nor
//! the byte arrays of `config.c:JE_saveGame` (see `save.rs`, which the game
//! no longer reads for the campaign). Equipment is written BY NAME, using
//! the item table names of the saved episode (`tyrian.hdt` or the table
//! embedded in `tyrianN.lvl`); a name carried by several items of the same
//! table ("Multi-Cannon" front and rear, "Pearl Wind"...) gets the
//! suffix ` #<number>` to stay unambiguous, and an item without a name
//! (mod item outside the original tables) is written as `#<number>`.
//!
//! Strict reading: format tag, known `format_version`, known fields only,
//! names present in the tables, values within bounds.
//! Any error returns a message, never a panic: the game then rejects the
//! slot and shows it as unreadable.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hdt::{HdtFile, ItemTables, PascalName};
use crate::lvl::LvlFile;

/// Value of the `format` field: tells this file apart from any other JSON.
pub const FORMAT_TAG: &str = "opentyrian-rewrite-campaign-save";
/// Only version read and written. An incompatible change increments this
/// number and adds the conversion from the old version (ADR 0129).
pub const FORMAT_VERSION: u32 = 2;
/// Old version (without 2 players): read, written as the current version.
const FORMAT_VERSION_V1: u32 = 1;
/// `config.h:SAVE_FILES_NUM`: 22 slots (11 one-player + 11 two-player,
/// config.c:JE_saveGame/JE_loadGame: (slot-1) > 10 = two players); the
/// last slot of each page is the "LAST LEVEL" autosave.
pub const SLOT_COUNT: u8 = 22;
pub const SLOT_COUNT_PER_PAGE: u8 = 11;
pub const AUTOSAVE_SLOT: u8 = 11;
/// First 2-player slot (config.c: slot 12, index 11).
pub const SLOT_TWO_PLAYER_FIRST: u8 = 12;
/// `mainint.c:JE_operation`: 14 characters of name at most.
pub const NAME_MAX: usize = 14;
/// Five episodes (`episodes.h:EPISODE_MAX`).
pub const EPISODE_MAX: u8 = 5;
/// `mainint.c:JE_doFunkyScreen` / shop: weapon power 1..=11.
pub const POWER_MAX: u8 = 11;

/// Difficulty levels by name (`helptext.c:difficultyNameB`, 0..=10).
pub const DIFFICULTY_NAMES: [&str; 11] = [
    "wimp",
    "easy",
    "normal",
    "hard",
    "impossible",
    "insanity",
    "suicide",
    "maniacal",
    "zinglon",
    "nortaneous",
    "level_10",
];

/// Read or write error, with a message for the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// Unreadable file (input/output).
    Io(String),
    /// Invalid JSON (file truncated or edited by hand).
    Corrupt(String),
    /// Other format, or version written by a newer game.
    Version(String),
    /// Valid JSON but content rejected (unknown name, value out of bounds).
    Invalid(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(m) => write!(f, "cannot read: {m}"),
            SaveError::Corrupt(m) => write!(f, "corrupt file: {m}"),
            SaveError::Version(m) => write!(f, "unsupported version: {m}"),
            SaveError::Invalid(m) => write!(f, "rejected content: {m}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Mounted weapon and its power.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponEntry {
    pub name: String,
    pub power: u8,
}

/// File contents (`format_version` 1 or 2), fields in write order.
/// The `*2` fields are absent from one-player saves (version 1 or 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignSaveFile {
    pub format: String,
    pub format_version: u32,
    pub slot: u8,
    pub name: String,
    /// Local date of the write, `YYYY-MM-DDTHH:MM:SS` (informational).
    pub saved_at: String,
    pub players: u8,
    pub episode: u8,
    /// Section of `levelsN.dat` to resume from (`saveLevel` in the C code).
    pub section: u16,
    /// Last completed level (`lastLevelName`), empty at the start of a game.
    pub last_level: String,
    pub difficulty: String,
    pub initial_difficulty: String,
    /// Player cash, which is also the score (`player[0].cash`).
    pub cash: u64,
    /// Data cubes of the last level (`lastCubeMax`).
    pub data_cubes: u8,
    pub ship: String,
    pub front_weapon: WeaponEntry,
    pub rear_weapon: Option<WeaponEntry>,
    pub shield: String,
    pub generator: String,
    pub left_sidekick: Option<String>,
    pub right_sidekick: Option<String>,
    pub special: Option<String>,
    /// Player 2 equipment (config.c:lastItems/JE_saveGame, format_version 2):
    /// present if and only if `players` == 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub ship2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub front_weapon2: Option<WeaponEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub rear_weapon2: Option<WeaponEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub shield2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub generator2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub left_sidekick2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub right_sidekick2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub special2: Option<String>,
    /// Player 2 cash (`player[1].cash`, config.c:score2).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cash2: Option<u64>,
    /// Player input devices (config.c:input1/input2, format_version 2).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub input_devices: Option<[u8; 2]>,
    /// Episode where the game started (config.c: `pItems[8]` =
    /// `initial_episode_num`): only this episode, once completed, opens the
    /// high score entry (mainint.c:JE_nextEpisode). Absent from older
    /// files: the saved episode is used instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub initial_episode: Option<u8>,
    /// `config.c:gameHasRepeated`: the game has already gone through all
    /// the episodes (no more high score possible).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub game_has_repeated: Option<bool>,
    /// `config.c:secretHint` (1..=3): series of secret hints shown at the
    /// end of the episodes (`level_load.c`, `case 'Q'`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub secret_hint: Option<u8>,
    /// One-player game mode (config.c: `pItems[2]` =
    /// `super_arcade_mode`): 1..=9 Super Arcade (ship), 254 Super Tyrian,
    /// 255 arcade. Absent = full game (older files included).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub super_arcade_mode: Option<u8>,
}

/// Campaign state as numbers (order of `TyrianSim.set_loadout`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CampaignState {
    pub slot: u8,
    pub name: String,
    pub saved_at: String,
    pub episode: u8,
    pub section: u16,
    pub last_level: String,
    pub difficulty: u8,
    pub initial_difficulty: u8,
    pub data_cubes: u8,
    /// Ship, front weapon, power, rear weapon, power, shield,
    /// generator, left sidekick, right sidekick, special, cash.
    pub loadout: [u64; 11],
    /// Player 2 equipment (config.c:lastItems) if `players` == 2. Same order
    /// as `loadout`. Empty ([0;11]) for a one-player game.
    pub loadout2: [u64; 11],
    /// Input devices (config.c:input1/input2); 0 = not saved.
    pub input_devices: [u8; 2],
    /// `initial_episode_num` (0 = unknown, old file).
    pub initial_episode: u8,
    /// `gameHasRepeated`.
    pub game_has_repeated: bool,
    /// `secretHint` (0 = unknown, old file).
    pub secret_hint: u8,
    /// `super_arcade_mode` from the C code (0 = full game).
    pub super_arcade_mode: u8,
}

/// Item names of an episode, one table per category.
#[derive(Debug, Clone, Default)]
pub struct ItemNames {
    pub ships: Vec<String>,
    pub weapon_ports: Vec<String>,
    pub shields: Vec<String>,
    pub generators: Vec<String>,
    pub sidekicks: Vec<String>,
    pub specials: Vec<String>,
}

fn names<'a>(v: impl Iterator<Item = &'a PascalName>) -> Vec<String> {
    v.map(|n| n.text().trim().to_string()).collect()
}

impl ItemNames {
    pub fn from_tables(t: &ItemTables) -> Self {
        Self {
            ships: names(t.ships.iter().map(|x| &x.name)),
            weapon_ports: names(t.weapon_ports.iter().map(|x| &x.name)),
            shields: names(t.shields.iter().map(|x| &x.name)),
            generators: names(t.generators.iter().map(|x| &x.name)),
            sidekicks: names(t.sidekicks.iter().map(|x| &x.name)),
            specials: names(t.specials.iter().map(|x| &x.name)),
        }
    }

    /// Item tables of the episode (`episodes.c:JE_loadItemDat`): the one
    /// embedded in `tyrianN.lvl` if present, otherwise `tyrian.hdt`.
    pub fn load(data_dir: &Path, episode: u8) -> Result<Self, String> {
        let lvl_path = data_dir.join(format!("tyrian{episode}.lvl"));
        let bytes = std::fs::read(&lvl_path).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
        let lvl = LvlFile::parse(&bytes).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
        let tables = match lvl.item_tables() {
            Some(t) => t.map_err(|e| e.to_string())?,
            None => {
                let hdt = data_dir.join("tyrian.hdt");
                let b = std::fs::read(&hdt).map_err(|e| format!("{}: {e}", hdt.display()))?;
                HdtFile::parse(&b).map_err(|e| e.to_string())?.items
            }
        };
        Ok(Self::from_tables(&tables))
    }
}

/// Name written for item `id` of `table`: the bare name if it is unique,
/// `name #id` if it is shared, `#id` if outside the table or unnamed.
fn item_name(table: &[String], id: u64) -> String {
    match table.get(id as usize) {
        Some(n) if !n.is_empty() => {
            if table.iter().filter(|x| *x == n).count() == 1 {
                n.clone()
            } else {
                format!("{n} #{id}")
            }
        }
        _ => format!("#{id}"),
    }
}

/// Inverse of [`item_name`].
fn item_id(table: &[String], what: &str, name: &str) -> Result<u64, SaveError> {
    let unknown = || SaveError::Invalid(format!("unknown {what} '{name}' (item of a disabled or retired mod?)"));
    if let Some(id) = name.strip_prefix('#') {
        let id: u64 = id.parse().map_err(|_| unknown())?;
        // A named item is written by its name: "#n" only designates an
        // item outside the table (mod item).
        return if (id as usize) < table.len() && !table[id as usize].is_empty() || id > 255 {
            Err(unknown())
        } else {
            Ok(id)
        };
    }
    if let Some((base, id)) = name.rsplit_once(" #") {
        if let Ok(id) = id.parse::<u64>() {
            if table.get(id as usize).is_some_and(|n| n == base) && table.iter().filter(|x| *x == base).count() > 1 {
                return Ok(id);
            }
        }
    }
    let mut hits = table.iter().enumerate().filter(|(_, n)| n.as_str() == name);
    match (hits.next(), hits.next()) {
        (Some((i, _)), None) => Ok(i as u64),
        _ => Err(unknown()),
    }
}

/// Optional item: number 0 = nothing (`null`).
fn optional_name(table: &[String], id: u64) -> Option<String> {
    (id != 0).then(|| item_name(table, id))
}

fn optional_id(table: &[String], what: &str, name: &Option<String>) -> Result<u64, SaveError> {
    match name {
        None => Ok(0),
        Some(n) => match item_id(table, what, n)? {
            0 => Err(SaveError::Invalid(format!("{what} '{n}': write null for none"))),
            id => Ok(id),
        },
    }
}

fn difficulty_id(what: &str, name: &str) -> Result<u8, SaveError> {
    DIFFICULTY_NAMES.iter().position(|n| *n == name).map(|i| i as u8).ok_or_else(|| {
        SaveError::Invalid(format!("unknown {what} '{name}' (expected: {})", DIFFICULTY_NAMES.join(", ")))
    })
}

fn check_power(what: &str, p: u8) -> Result<u8, SaveError> {
    if (1..=POWER_MAX).contains(&p) {
        Ok(p)
    } else {
        Err(SaveError::Invalid(format!("{what} power {p} outside 1..={POWER_MAX}")))
    }
}

/// Slot name: displayable uppercase, 14 characters at most
/// (mainint.c:JE_operation: toupper, 14 characters).
pub fn clean_slot_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .map(|c| c.to_ascii_uppercase())
        .take(NAME_MAX)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// File of a state; `names` = tables of the saved episode.
pub fn to_file(s: &CampaignState, names: &ItemNames) -> Result<CampaignSaveFile, SaveError> {
    if !(1..=SLOT_COUNT).contains(&s.slot) {
        return Err(SaveError::Invalid(format!("slot {} outside 1..={SLOT_COUNT}", s.slot)));
    }
    if !(1..=EPISODE_MAX).contains(&s.episode) {
        return Err(SaveError::Invalid(format!("episode {} outside 1..={EPISODE_MAX}", s.episode)));
    }
    let lo = &s.loadout;
    let diff = |d: u8| {
        DIFFICULTY_NAMES
            .get(d as usize)
            .map(|n| n.to_string())
            .ok_or_else(|| SaveError::Invalid(format!("difficulty {d} outside 0..=10")))
    };
    Ok(CampaignSaveFile {
        format: FORMAT_TAG.into(),
        format_version: FORMAT_VERSION,
        slot: s.slot,
        name: clean_slot_name(&s.name),
        saved_at: s.saved_at.clone(),
        players: if s.loadout2 != [0u64; 11] { 2 } else { 1 },
        episode: s.episode,
        section: s.section,
        last_level: s.last_level.clone(),
        difficulty: diff(s.difficulty)?,
        initial_difficulty: diff(s.initial_difficulty)?,
        cash: lo[10],
        data_cubes: s.data_cubes,
        ship: item_name(&names.ships, lo[0]),
        front_weapon: WeaponEntry {
            name: item_name(&names.weapon_ports, lo[1]),
            power: check_power("front weapon", lo[2] as u8)?,
        },
        rear_weapon: if lo[3] == 0 {
            None
        } else {
            Some(WeaponEntry {
                name: item_name(&names.weapon_ports, lo[3]),
                power: check_power("rear weapon", lo[4] as u8)?,
            })
        },
        shield: item_name(&names.shields, lo[5]),
        generator: item_name(&names.generators, lo[6]),
        left_sidekick: optional_name(&names.sidekicks, lo[7]),
        right_sidekick: optional_name(&names.sidekicks, lo[8]),
        special: optional_name(&names.specials, lo[9]),
        // Player 2 (config.c:lastItems, score2, input1/input2)
        ship2: (s.loadout2 != [0u64; 11]).then(|| item_name(&names.ships, s.loadout2[0])),
        front_weapon2: (s.loadout2 != [0u64; 11]).then(|| WeaponEntry {
            name: item_name(&names.weapon_ports, s.loadout2[1]),
            power: s.loadout2[2].max(1).min(POWER_MAX as u64) as u8,
        }),
        rear_weapon2: (s.loadout2 != [0u64; 11] && s.loadout2[3] != 0).then(|| WeaponEntry {
            name: item_name(&names.weapon_ports, s.loadout2[3]),
            power: s.loadout2[4].max(1).min(POWER_MAX as u64) as u8,
        }),
        shield2: (s.loadout2 != [0u64; 11]).then(|| item_name(&names.shields, s.loadout2[5])),
        generator2: (s.loadout2 != [0u64; 11]).then(|| item_name(&names.generators, s.loadout2[6])),
        left_sidekick2: (s.loadout2 != [0u64; 11]).then(|| optional_name(&names.sidekicks, s.loadout2[7])).flatten(),
        right_sidekick2: (s.loadout2 != [0u64; 11]).then(|| optional_name(&names.sidekicks, s.loadout2[8])).flatten(),
        special2: (s.loadout2 != [0u64; 11]).then(|| optional_name(&names.specials, s.loadout2[9])).flatten(),
        cash2: (s.loadout2 != [0u64; 11]).then_some(s.loadout2[10]),
        input_devices: (s.input_devices != [0u8; 2]).then_some(s.input_devices),
        initial_episode: (1..=EPISODE_MAX).contains(&s.initial_episode).then_some(s.initial_episode),
        game_has_repeated: s.game_has_repeated.then_some(true),
        secret_hint: (1..=3).contains(&s.secret_hint).then_some(s.secret_hint),
        super_arcade_mode: (s.super_arcade_mode != 0).then_some(s.super_arcade_mode),
    })
}

/// JSON text (indented, final line ending) of a state.
pub fn to_json(s: &CampaignState, names: &ItemNames) -> Result<String, SaveError> {
    let f = to_file(s, names)?;
    let mut text = serde_json::to_string_pretty(&f).map_err(|e| SaveError::Invalid(e.to_string()))?;
    text.push('\n');
    Ok(text)
}

/// Tag and version read before anything else: a file of a future version
/// is rejected for its version, not for a field it added.
fn check_header(v: &serde_json::Value) -> Result<(), SaveError> {
    let obj = v.as_object().ok_or_else(|| SaveError::Corrupt("JSON object expected".into()))?;
    match obj.get("format").and_then(|f| f.as_str()) {
        Some(FORMAT_TAG) => {}
        Some(other) => return Err(SaveError::Version(format!("format '{other}' (expected '{FORMAT_TAG}')"))),
        None => return Err(SaveError::Version(format!("missing 'format' field (expected '{FORMAT_TAG}')"))),
    }
    match obj.get("format_version").and_then(|n| n.as_u64()) {
        Some(n) if n == FORMAT_VERSION as u64 || n == FORMAT_VERSION_V1 as u64 => Ok(()),
        Some(n) => Err(SaveError::Version(format!(
            "format_version {n} (this game reads versions {FORMAT_VERSION_V1} and {FORMAT_VERSION})"
        ))),
        None => Err(SaveError::Version("'format_version' field missing or not an integer".into())),
    }
}

/// Reads a file; `names_for` returns the tables of an episode (1..=5).
pub fn from_json(text: &str, names_for: &dyn Fn(u8) -> Option<ItemNames>) -> Result<CampaignState, SaveError> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| SaveError::Corrupt(e.to_string()))?;
    check_header(&v)?;
    let f: CampaignSaveFile = serde_json::from_value(v).map_err(|e| SaveError::Invalid(e.to_string()))?;
    if !(1..=SLOT_COUNT).contains(&f.slot) {
        return Err(SaveError::Invalid(format!("slot {} outside 1..={SLOT_COUNT}", f.slot)));
    }
    if f.players != 1 && f.players != 2 {
        return Err(SaveError::Invalid(format!("{} players: 1 or 2 players expected", f.players)));
    }
    if !(1..=EPISODE_MAX).contains(&f.episode) {
        return Err(SaveError::Invalid(format!("episode {} outside 1..={EPISODE_MAX}", f.episode)));
    }
    if f.section == 0 {
        return Err(SaveError::Invalid("section 0 (sections start at 1)".into()));
    }
    let names = names_for(f.episode)
        .ok_or_else(|| SaveError::Invalid(format!("item tables of episode {} not found", f.episode)))?;
    let (rear, rear_power) = match &f.rear_weapon {
        None => (0, 1),
        Some(w) => match item_id(&names.weapon_ports, "rear weapon", &w.name)? {
            0 => return Err(SaveError::Invalid("rear weapon: write null for none".into())),
            id => (id, check_power("rear weapon", w.power)?),
        },
    };
    let loadout = [
        item_id(&names.ships, "ship", &f.ship)?,
        item_id(&names.weapon_ports, "front weapon", &f.front_weapon.name)?,
        check_power("front weapon", f.front_weapon.power)? as u64,
        rear,
        rear_power as u64,
        item_id(&names.shields, "shield", &f.shield)?,
        item_id(&names.generators, "generator", &f.generator)?,
        optional_id(&names.sidekicks, "left sidekick", &f.left_sidekick)?,
        optional_id(&names.sidekicks, "right sidekick", &f.right_sidekick)?,
        optional_id(&names.specials, "special", &f.special)?,
        f.cash,
    ];
    // Player 2 equipment (config.c:lastItems, score2)
    let loadout2 = if f.players == 2 {
        let ship2 = f.ship2.as_deref().ok_or_else(|| SaveError::Invalid("ship2 missing for 2 players".into()))?;
        let fw2 = f.front_weapon2.as_ref().ok_or_else(|| SaveError::Invalid("front_weapon2 missing".into()))?;
        let (rear2, rear2_power) = match &f.rear_weapon2 {
            None => (0u64, 1u8),
            Some(w) => match item_id(&names.weapon_ports, "rear weapon p2", &w.name)? {
                0 => return Err(SaveError::Invalid("rear_weapon2: write null for none".into())),
                id => (id, check_power("rear weapon p2", w.power)?),
            },
        };
        [
            item_id(&names.ships, "ship p2", ship2)?,
            item_id(&names.weapon_ports, "front weapon p2", &fw2.name)?,
            check_power("front weapon p2", fw2.power)? as u64,
            rear2,
            rear2_power as u64,
            item_id(
                &names.shields,
                "shield p2",
                f.shield2.as_deref().ok_or_else(|| SaveError::Invalid("shield2 missing".into()))?,
            )?,
            item_id(
                &names.generators,
                "generator p2",
                f.generator2.as_deref().ok_or_else(|| SaveError::Invalid("generator2 missing".into()))?,
            )?,
            optional_id(&names.sidekicks, "left sidekick p2", &f.left_sidekick2)?,
            optional_id(&names.sidekicks, "right sidekick p2", &f.right_sidekick2)?,
            optional_id(&names.specials, "special p2", &f.special2)?,
            f.cash2.ok_or_else(|| SaveError::Invalid("cash2 missing for 2 players".into()))?,
        ]
    } else {
        [0u64; 11]
    };
    Ok(CampaignState {
        slot: f.slot,
        name: f.name,
        saved_at: f.saved_at,
        episode: f.episode,
        section: f.section,
        last_level: f.last_level,
        difficulty: difficulty_id("difficulty", &f.difficulty)?,
        initial_difficulty: difficulty_id("initial difficulty", &f.initial_difficulty)?,
        data_cubes: f.data_cubes,
        loadout,
        loadout2,
        input_devices: f.input_devices.unwrap_or([0u8; 2]),
        initial_episode: f.initial_episode.filter(|e| (1..=EPISODE_MAX).contains(e)).unwrap_or(0),
        game_has_repeated: f.game_has_repeated.unwrap_or(false),
        secret_hint: f.secret_hint.filter(|h| (1..=3).contains(h)).unwrap_or(0),
        super_arcade_mode: f.super_arcade_mode.unwrap_or(0),
    })
}

/// `saves/slot_NN.json`.
pub fn slot_path(dir: &Path, slot: u8) -> PathBuf {
    dir.join(format!("slot_{slot:02}.json"))
}

/// Atomic write: temporary file in the same folder, flushed to disk,
/// then renamed over the old one (a stop during the write leaves the
/// old save intact).
pub fn write_atomic(path: &Path, text: &str) -> Result<(), SaveError> {
    use std::io::Write;
    let io = |e: std::io::Error| SaveError::Io(format!("{}: {e}", path.display()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut f = std::fs::File::create(&tmp).map_err(io)?;
    f.write_all(text.as_bytes()).map_err(io)?;
    f.sync_all().map_err(io)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io(e)
    })
}

/// Saves the state into its slot of `dir`.
pub fn save(dir: &Path, s: &CampaignState, names: &ItemNames) -> Result<PathBuf, SaveError> {
    let text = to_json(s, names)?;
    let path = slot_path(dir, s.slot);
    write_atomic(&path, &text)?;
    Ok(path)
}

/// Reads a slot: `None` if the file does not exist.
pub fn load(
    dir: &Path,
    slot: u8,
    names_for: &dyn Fn(u8) -> Option<ItemNames>,
) -> Option<Result<CampaignState, SaveError>> {
    let path = slot_path(dir, slot);
    let text = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(Err(SaveError::Io(format!("{}: {e}", path.display())))),
    };
    let text = match String::from_utf8(text) {
        Ok(t) => t,
        Err(_) => return Some(Err(SaveError::Corrupt("non-UTF-8 text".into()))),
    };
    Some(from_json(&text, names_for).and_then(|s| {
        if s.slot == slot {
            Ok(s)
        } else {
            Err(SaveError::Invalid(format!("the file of slot {slot} claims to be slot {}", s.slot)))
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> ItemNames {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        ItemNames {
            ships: t(&["None", "USP Talon", "SuperCarrot"]),
            weapon_ports: t(&["None", "Pulse-Cannon", "Multi-Cannon", "Multi-Cannon", "Vulcan Cannon"]),
            shields: t(&["None", "Structural Integrity Field"]),
            generators: t(&["None", "Standard MR-9", "Advanced MR-12"]),
            sidekicks: t(&["None", "Single Shot Option", "None"]),
            specials: t(&["None", "Repulsor", "Pearl Wind", "Pearl Wind"]),
        }
    }

    fn state() -> CampaignState {
        CampaignState {
            slot: 3,
            name: "my game".into(),
            saved_at: "2026-10-05T12:00:00".into(),
            episode: 2,
            section: 7,
            last_level: "TORM".into(),
            difficulty: 3,
            initial_difficulty: 2,
            data_cubes: 1,
            loadout: [2, 3, 5, 4, 2, 1, 2, 1, 0, 3, 123456],
            loadout2: [0u64; 11],
            input_devices: [0u8; 2],
            initial_episode: 1,
            game_has_repeated: true,
            secret_hint: 2,
            super_arcade_mode: 3,
        }
    }

    fn lookup(_: u8) -> Option<ItemNames> {
        Some(names())
    }

    #[test]
    fn names_are_written_and_read_back() {
        let text = to_json(&state(), &names()).unwrap();
        assert!(text.contains("\"ship\": \"SuperCarrot\""), "{text}");
        assert!(text.contains("\"name\": \"Multi-Cannon #3\""), "{text}");
        assert!(text.contains("\"special\": \"Pearl Wind #3\""), "{text}");
        assert!(text.contains("\"right_sidekick\": null"), "{text}");
        assert!(text.contains("\"difficulty\": \"hard\""), "{text}");
        assert!(text.contains("\"name\": \"MY GAME\""), "{text}");
        let back = from_json(&text, &lookup).unwrap();
        assert_eq!(back, CampaignState { name: "MY GAME".into(), ..state() });
    }

    #[test]
    fn unknown_version_and_format_are_refused() {
        let text = to_json(&state(), &names()).unwrap();
        // The current version (2) is valid.
        assert!(from_json(&text, &lookup).is_ok(), "v2 must be accepted: {text}");
        // Version 1 is also valid (transparent migration).
        let v1 = text.replace("\"format_version\": 2", "\"format_version\": 1");
        assert!(from_json(&v1, &lookup).is_ok(), "v1 must be accepted");
        // Version 3 (future): rejected.
        let v3 = text.replace("\"format_version\": 2", "\"format_version\": 3");
        assert!(matches!(from_json(&v3, &lookup), Err(SaveError::Version(_))));
        // Unknown field in v3: rejected for version (checked before deserialization).
        let v3b = v3.replace("\"slot\": 3", "\"slot\": 3, \"new_field\": true");
        assert!(matches!(from_json(&v3b, &lookup), Err(SaveError::Version(_))));
        let other = text.replace(FORMAT_TAG, "something-else");
        assert!(matches!(from_json(&other, &lookup), Err(SaveError::Version(_))));
        assert!(matches!(from_json("{}", &lookup), Err(SaveError::Version(_))));
    }
    #[test]
    fn corrupt_and_invalid_files_are_refused() {
        let text = to_json(&state(), &names()).unwrap();
        for cut in [0, 1, text.len() / 2, text.len() - 3] {
            assert!(from_json(&text[..cut], &lookup).is_err(), "truncated at {cut}");
        }
        assert!(matches!(from_json("[1, 2]", &lookup), Err(SaveError::Corrupt(_))));
        let bad = [
            ("\"SuperCarrot\"", "\"Mystery Ship\""),
            ("\"Multi-Cannon #3\"", "\"Multi-Cannon\""),
            ("\"Pearl Wind #3\"", "\"Pearl Wind #1\""),
            ("\"power\": 5", "\"power\": 12"),
            ("\"power\": 5", "\"power\": 0"),
            ("\"difficulty\": \"hard\"", "\"difficulty\": \"3\""),
            ("\"episode\": 2", "\"episode\": 6"),
            ("\"players\": 1", "\"players\": 2"),
            ("\"section\": 7", "\"section\": 0"),
            ("\"right_sidekick\": null", "\"right_sidekick\": \"None\""),
            ("\"cash\": 123456", "\"cash\": -1"),
            // Unknown field: accepted (the optional p2 fields require it)
        ];
        for (from, to) in bad {
            assert!(text.contains(from), "{from}");
            let t = text.replace(from, to);
            assert!(matches!(from_json(&t, &lookup), Err(SaveError::Invalid(_))), "{to} accepted");
        }
        let no_tables = |_: u8| None;
        assert!(from_json(&text, &no_tables).is_err());
    }

    #[test]
    fn mod_items_outside_the_tables_use_their_number() {
        let mut s = state();
        s.loadout[0] = 40;
        let text = to_json(&s, &names()).unwrap();
        assert!(text.contains("\"ship\": \"#40\""), "{text}");
        assert_eq!(from_json(&text, &lookup).unwrap().loadout[0], 40);
        // A named item is not written by number.
        let t = text.replace("\"#40\"", "\"#2\"");
        assert!(from_json(&t, &lookup).is_err());
    }

    #[test]
    fn mod_items_named_in_the_tables_are_saved_by_name_and_refused_without_the_mod() {
        // With the mod enabled, ship 40 has a name: the save writes it by its name.
        let mut with_mod = names();
        with_mod.ships.resize(41, String::new());
        with_mod.ships[40] = "Zica Ember".into();
        let mut s = state();
        s.loadout[0] = 40;
        let text = to_json(&s, &with_mod).unwrap();
        assert!(text.contains("\"ship\": \"Zica Ember\""), "{text}");
        let with_lookup = |_: u8| Some(with_mod.clone());
        assert_eq!(from_json(&text, &with_lookup).unwrap().loadout[0], 40);
        // Without the mod: clean refusal, a message naming the item and the likely cause.
        let err = from_json(&text, &lookup).unwrap_err().to_string();
        assert!(err.contains("Zica Ember") && err.contains("mod"), "{err}");
    }

    #[test]
    fn atomic_write_and_slot_files() {
        let dir = std::env::temp_dir().join(format!("tyrian-save-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load(&dir, 3, &lookup).is_none());
        let p = save(&dir, &state(), &names()).unwrap();
        assert_eq!(p, dir.join("slot_03.json"));
        assert!(!dir.join("slot_03.json.tmp").exists());
        let mut s2 = state();
        s2.section = 9;
        save(&dir, &s2, &names()).unwrap();
        assert_eq!(load(&dir, 3, &lookup).unwrap().unwrap().section, 9);
        // File copied into another slot: rejected.
        std::fs::copy(&p, slot_path(&dir, 4)).unwrap();
        assert!(matches!(load(&dir, 4, &lookup), Some(Err(SaveError::Invalid(_)))));
        std::fs::write(slot_path(&dir, 5), [0xff, 0xfe, 0]).unwrap();
        assert!(matches!(load(&dir, 5, &lookup), Some(Err(SaveError::Corrupt(_)))));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
