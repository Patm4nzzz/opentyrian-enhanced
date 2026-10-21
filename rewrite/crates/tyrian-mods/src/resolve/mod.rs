//! Resolution of mods onto the tables of an episode (ADR 0083: single format).
//!
//! Everything a mod creates (weapon, ship, wingman, shield, special, enemy
//! weapon, enemy) is designated by its NAME; slots in the game
//! tables are assigned here, never by the mod author. The cited original
//! content (enemies, enemy weapons) is also cited by name
//! (`crate::originals`).
//!
//! Assignment and application follow the order of the C loader
//! (`mod_manager.c`, `hdt_json.c`) so that objects cloned by `base`
//! behave as in the original game. The C tables are reproduced by
//! `tyrian_sim::tables::Tables`; the modified records form a
//! `TablePatch`.
//!
//! Layout:
//! - `reading`: reading of player objects (weapons, ships, wingmen,
//!   shields, specials), the start and the levels; allowed keys;
//! - `enemy_reading`: enemies, enemy weapons and behaviors;
//! - `names`: name resolution (original objects, identities, cited
//!   sprites, enemy weapons, start, ship images and icons);
//! - `sprites`: mod PNGs placed in the override tables, ship
//!   poses, tints.

mod enemy_reading;
mod names;
mod reading;
mod sprites;

use names::*;
use reading::*;
use sprites::*;
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, WeaponId};

use crate::folder::{ModFolder, SpriteTable};
use crate::json::Value;
use crate::levels::{self, LevelOverride};
use crate::manifest::{strtol_full, SpriteNames};
use crate::originals::OriginalNames;
use crate::{Diagnostics, ModSet};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tyrian_formats::hdt::{EnemyDat, ItemTables, PascalName};
use tyrian_sim::behavior::{
    BehaviorTurret, EnemyBehavior, PathSegment, TablePatch, Track, MAX_BEHAVIOR_TURRETS, MAX_PATH_SEGMENTS,
};
use tyrian_sim::tables::{Tables, OPTION_NUM, PORT_NUM, POWER_NUM, SHIELD_NUM, SHIP_NUM, SPECIAL_NUM, WEAP_NUM};
use tyrian_sim::{LevelSetup, Loadout};

// mod_manager.c: free identifier ranges for mods.
const SHIP_RANGE: (usize, usize) = (19, SHIP_NUM);
// The ship image table is sized for every slot of this range (folder.rs, ADR 0134).
const _: () = assert!(SHIP_RANGE.1 - SHIP_RANGE.0 + 1 == crate::folder::MOD_SHIP_SLOTS as usize);
const WEAPON_RANGE: (usize, usize) = (61, PORT_NUM);
const SIDEKICK_RANGE: (usize, usize) = (38, OPTION_NUM);
const SHIELD_RANGE: (usize, usize) = (12, SHIELD_NUM);
const SPECIAL_RANGE: (usize, usize) = (55, SPECIAL_NUM);
/// hdt_json.c:next_free_mod_weapon_id: projectile definitions of the mods.
const MOD_WEAPON_ID: (usize, usize) = (1800, WEAP_NUM);
/// New enemies, in the 851..1000 gap of `enemyDat[]`
/// (never read from the original files, rejected by the C loader:
/// no possible conflict). ADR 0081, 0083.
pub const NEW_ENEMY_RANGE: (usize, usize) = (900, 999);
/// hdt_json.h:MAX_ALWAYS_AVAIL.
const MAX_ALWAYS_AVAIL: usize = 16;
/// lvlmast.h.
const SHIELD_TPWR_MAX: i32 = 28;
const SHIELD_MPWR_MAX: i32 = 18;
/// mod_identity.h:MOD_ITEM_KEY_MAX.
const MOD_ITEM_KEY_MAX: usize = 384;
/// mod_manager.c:PENDING_NAME_MAX (47 useful characters).
const PENDING_NAME_MAX: usize = 47;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemKind {
    Ship,
    Weapon,
    Sidekick,
    Shield,
    Special,
}

/// A mod PNG placed at an index of an override table.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteAssignment {
    pub table: SpriteTable,
    pub index: i32,
    pub path: PathBuf,
    /// Target HSV tint (0..1) applied at load time (`sprite_override.c:apply_tint`); rendering only.
    pub hue: Option<f32>,
    /// Enemy bank (`shapebank`) for the `Enemies` table.
    pub bank: i32,
}

/// Starting equipment imposed by the mods (`hdt_json.c:starter_*`, -1 = unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarterLoadout {
    pub ship: i32,
    pub front_weapon: i32,
    pub rear_weapon: i32,
    pub shield: i32,
    pub generator: i32,
    pub sidekick_left: i32,
    pub sidekick_right: i32,
}

impl Default for StarterLoadout {
    fn default() -> Self {
        Self {
            ship: -1,
            front_weapon: -1,
            rear_weapon: -1,
            shield: -1,
            generator: -1,
            sidekick_left: -1,
            sidekick_right: -1,
        }
    }
}

impl StarterLoadout {
    pub fn apply(&self, l: &mut Loadout) {
        // mainint.c:JE_initPlayerData ("starter_* >= 0" block).
        if self.ship >= 0 {
            l.ship = ShipId(self.ship as u8);
        }
        if self.front_weapon >= 0 {
            l.front_weapon = WeaponId(self.front_weapon as u8);
        }
        if self.rear_weapon >= 0 {
            l.rear_weapon = WeaponId(self.rear_weapon as u8);
        }
        if self.shield >= 0 {
            l.shield = ShieldId(self.shield as u8);
        }
        if self.generator >= 0 {
            l.generator = GeneratorId(self.generator as u8);
        }
        if self.sidekick_left >= 0 {
            l.left_sidekick = SidekickId(self.sidekick_left as u8);
        }
        if self.sidekick_right >= 0 {
            l.right_sidekick = SidekickId(self.sidekick_right as u8);
        }
    }
}

/// Per-mod summary (`ModInfo` of the C code).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModReport {
    pub folder: String,
    pub name: String,
    pub enabled: bool,
    pub error: String,
    pub weapons: usize,
    pub ships: usize,
    pub sidekicks: usize,
    pub shields: usize,
    pub specials: usize,
    pub enemies: usize,
    pub sprites: usize,
    pub enemy_weapons: usize,
    pub new_enemies: usize,
    pub levels: usize,
}

/// Result of the resolution for an episode.
#[derive(Clone, Default)]
pub struct ResolvedMods {
    pub episode: u8,
    pub patch: TablePatch,
    pub sprites: Vec<SpriteAssignment>,
    pub starter: StarterLoadout,
    /// "Always available" identifiers in the shop (`always_avail_*`).
    pub shop: BTreeMap<&'static str, Vec<u8>>,
    /// Identity "length:folder/name" per (category, identifier) — `mod_identity.c`.
    pub identities: BTreeMap<(ItemKind, u16), String>,
    /// New enemies: name → index in `enemyDat`.
    pub new_enemies: BTreeMap<String, u16>,
    /// Mod enemy weapons: name → index in `weapons`.
    pub enemy_weapons: BTreeMap<String, u16>,
    /// Replaced levels: (episode, level no.) → events.
    pub levels: BTreeMap<(u8, u16), LevelOverride>,
    pub reports: Vec<ModReport>,
    pub diagnostics: Diagnostics,
    /// Complete tables after application (tooling, tests, shop).
    pub tables: Option<Tables>,
}

impl ResolvedMods {
    /// Injects the patch into a level to start. `lvl_file_num` is used to
    /// find a level replaced by a mod. Returns true if the level was replaced.
    pub fn apply_to(&self, setup: &mut LevelSetup, lvl_file_num: u16) -> bool {
        setup.patch = self.patch.clone();
        match self.levels.get(&(setup.episode, lvl_file_num)) {
            Some(l) => {
                l.apply(&mut setup.level.events);
                true
            }
            None => false,
        }
    }
}

struct Ctx<'a> {
    t: Tables,
    names: SpriteNames,
    sprites: SpriteTables,
    diag: Diagnostics,
    ship_used: [bool; SHIP_NUM + 1],
    weapon_used: [bool; PORT_NUM + 1],
    sidekick_used: [bool; OPTION_NUM + 1],
    shield_used: [bool; SHIELD_NUM + 1],
    special_used: [bool; SPECIAL_NUM + 1],
    mod_weapon_id_used: Vec<bool>,
    touched_weapons: BTreeSet<usize>,
    touched_enemies: BTreeSet<usize>,
    shop: BTreeMap<&'static str, Vec<u8>>,
    identities: BTreeMap<(ItemKind, u16), String>,
    pending_ship_sprite: BTreeMap<usize, String>,
    pending_ship_icon: BTreeMap<usize, String>,
    pending_ship_poses: BTreeMap<usize, [String; 4]>,
    pending_weapon_icon: BTreeMap<usize, String>,
    pending_sidekick_icon: BTreeMap<usize, String>,
    pending_shield_icon: BTreeMap<usize, String>,
    pending_special_icon: BTreeMap<usize, String>,
    weapon_tint: BTreeMap<usize, f32>,
    starter: StarterLoadout,
    /// `starter` section: key → (mod folder, object name).
    pending_starter: BTreeMap<&'static str, (String, String)>,
    behaviors: Vec<EnemyBehavior>,
    enemy_behavior: Vec<(u16, u16)>,
    new_enemies: BTreeMap<String, u16>,
    enemy_weapons: BTreeMap<String, u16>,
    levels: BTreeMap<(u8, u16), LevelOverride>,
    /// Named original content and table of the episode (0: episodes 1-3, 1: 4-5).
    originals: &'a OriginalNames,
    group: usize,
    /// Original enemies absent from the episode table, copied: name → slot.
    imported_originals: BTreeMap<String, u16>,
}

/// Export names: original content of the episode table.
struct EpisodeNames<'a> {
    originals: &'a OriginalNames,
    group: usize,
    items: &'a ItemTables,
}

impl levels::ExportNames for EpisodeNames<'_> {
    fn enemy_name(&self, slot: u16) -> Option<String> {
        self.originals.enemy_name(slot, self.group).map(str::to_string)
    }
    fn special_name(&self, id: u8) -> Option<String> {
        self.items.specials.get(id as usize).map(|s| s.name.text().trim_end().to_string()).filter(|n| !n.is_empty())
    }
}

impl ModSet {
    /// Blank resolution context for the episode (original tables).
    fn context(&self, episode: u8, items: &ItemTables) -> Ctx<'_> {
        let t = Tables::new(items);
        let mut c = Ctx {
            t,
            names: self.manifest.clone(),
            sprites: SpriteTables::default(),
            diag: self.diagnostics.clone(),
            ship_used: [false; SHIP_NUM + 1],
            weapon_used: [false; PORT_NUM + 1],
            sidekick_used: [false; OPTION_NUM + 1],
            shield_used: [false; SHIELD_NUM + 1],
            special_used: [false; SPECIAL_NUM + 1],
            mod_weapon_id_used: vec![false; WEAP_NUM + 1],
            touched_weapons: BTreeSet::new(),
            touched_enemies: BTreeSet::new(),
            shop: BTreeMap::new(),
            identities: BTreeMap::new(),
            pending_ship_sprite: BTreeMap::new(),
            pending_ship_icon: BTreeMap::new(),
            pending_ship_poses: BTreeMap::new(),
            pending_weapon_icon: BTreeMap::new(),
            pending_sidekick_icon: BTreeMap::new(),
            pending_shield_icon: BTreeMap::new(),
            pending_special_icon: BTreeMap::new(),
            weapon_tint: BTreeMap::new(),
            starter: StarterLoadout::default(),
            pending_starter: BTreeMap::new(),
            behaviors: Vec::new(),
            enemy_behavior: Vec::new(),
            new_enemies: BTreeMap::new(),
            enemy_weapons: BTreeMap::new(),
            levels: BTreeMap::new(),
            originals: &self.originals,
            group: crate::originals::table_group(episode),
            imported_originals: BTreeMap::new(),
        };
        for k in ["ships", "weapons", "weapons_front", "weapons_rear", "sidekicks", "shields", "specials"] {
            c.shop.insert(k, Vec::new());
        }
        c.scan_used_ids();
        c
    }

    /// Reads a mod level text with only the original names of the
    /// episode (no mod loaded): round-trip proof of the export.
    pub fn read_level(
        &self,
        items: &ItemTables,
        episode: u8,
        text: &str,
        diag: &mut Diagnostics,
    ) -> Option<LevelOverride> {
        let mut c = self.context(episode, items);
        levels::parse_level(text, "", "export.json", &mut c, diag)
    }

    /// `levels/epN_NN.json` file (named events) of the events of a
    /// level open in the editor: "Export to mod" button.
    pub fn export_level(
        &self,
        items: &ItemTables,
        episode: u8,
        lvl_file_num: u16,
        events: &[(tyrian_formats::lvl::Event, Option<levels::EnemyCited>)],
        random_enemies: &[levels::EnemyCited],
    ) -> Result<String, String> {
        let names = EpisodeNames { originals: &self.originals, group: crate::originals::table_group(episode), items };
        levels::export_level(episode, lvl_file_num, events, random_enemies, &names)
    }

    /// Applies the enabled mods to the tables of episode `episode` (`items` =
    /// `tyrian.hdt` or end block of `tyrianN.lvl`).
    pub fn resolve(&self, episode: u8, items: &ItemTables) -> ResolvedMods {
        let mut c = self.context(episode, items);
        let mut reports = Vec::new();
        for m in &self.folders {
            reports.push(c.load_one(m));
        }
        c.resolve_graphics_names();
        c.auto_bank_ships();
        c.apply_per_item_tints();
        c.apply_pending_starter_names();
        c.finish(episode, reports)
    }
}

impl Ctx<'_> {
    /// `mod_manager.c:scan_used_ids`.
    fn scan_used_ids(&mut self) {
        fn mark<const N: usize>(used: &mut [bool; N], range: (usize, usize), named: impl Fn(usize) -> bool) {
            for u in used.iter_mut().take(range.0).skip(1) {
                *u = true;
            }
            for (i, u) in used.iter_mut().enumerate().take(range.1 + 1).skip(range.0) {
                if named(i) {
                    *u = true;
                }
            }
        }
        let t = &self.t;
        mark(&mut self.ship_used, SHIP_RANGE, |i| !cname(&t.ships[i].name).is_empty());
        mark(&mut self.weapon_used, WEAPON_RANGE, |i| !cname(&t.ports[i].name).is_empty());
        mark(&mut self.sidekick_used, SIDEKICK_RANGE, |i| !cname(&t.options[i].name).is_empty());
        mark(&mut self.shield_used, SHIELD_RANGE, |i| !cname(&t.shields[i].name).is_empty());
        mark(&mut self.special_used, SPECIAL_RANGE, |i| !cname(&t.specials[i].name).is_empty());
    }

    fn load_one(&mut self, m: &ModFolder) -> ModReport {
        let mut r = ModReport {
            folder: m.folder.clone(),
            name: m.name.clone(),
            enabled: m.enabled,
            error: m.error.clone(),
            ..Default::default()
        };
        let Some(root) = m.root.as_ref() else { return r };
        if !m.enabled {
            return r;
        }
        let folder = m.folder.as_str();
        self.check_keys(root, TOP_KEYS, folder, "mod.json");
        // Sprites first: PNG names must exist before the
        // weapons and ships cite them.
        let hue = parse_tint(root.get("tint"));
        r.sprites = self.load_sprites(m, hue);
        r.weapons = self.load_weapons(root, folder);
        r.ships = self.load_ships(root, folder);
        r.sidekicks = self.load_sidekicks(root, folder);
        r.shields = self.load_shields(root, folder);
        r.specials = self.load_specials(root, folder);
        if let Some(st) = root.get("starter") {
            self.stash_starter(st, folder);
        }
        r.enemy_weapons = self.load_enemy_weapons(root, folder);
        (r.enemies, r.new_enemies) = self.load_enemies(root, folder);
        r.levels = self.load_levels(m);
        let mut errs: Vec<String> = Vec::new();
        for d in self.diag.for_folder(folder) {
            if d.severity == crate::Severity::Error {
                errs.push(d.message.clone());
            }
        }
        if r.error.is_empty() {
            if let Some(e) = errs.last() {
                r.error = e.clone();
            }
        }
        r
    }

    fn finish(mut self, episode: u8, reports: Vec<ModReport>) -> ResolvedMods {
        let t = &self.t;
        let mut patch = TablePatch::default();
        for &w in &self.touched_weapons {
            patch.weapons.push((w as u16, t.weapons[w].clone()));
        }
        // Ports modified by `base` may also cite original weapons:
        // only the slots assigned to mods are carried over.
        for id in WEAPON_RANGE.0..=WEAPON_RANGE.1 {
            if self.identities.contains_key(&(ItemKind::Weapon, id as u16)) {
                patch.ports.push((id as u8, t.ports[id].clone()));
            }
        }
        for id in SHIP_RANGE.0..=SHIP_RANGE.1 {
            if self.identities.contains_key(&(ItemKind::Ship, id as u16)) {
                patch.ships.push((id as u8, t.ships[id].clone()));
            }
        }
        for id in SIDEKICK_RANGE.0..=SIDEKICK_RANGE.1 {
            if self.identities.contains_key(&(ItemKind::Sidekick, id as u16)) {
                patch.options.push((id as u8, t.options[id].clone()));
            }
        }
        for id in SHIELD_RANGE.0..=SHIELD_RANGE.1 {
            if self.identities.contains_key(&(ItemKind::Shield, id as u16)) {
                patch.shields.push((id as u8, t.shields[id].clone()));
            }
        }
        for id in SPECIAL_RANGE.0..=SPECIAL_RANGE.1 {
            if self.identities.contains_key(&(ItemKind::Special, id as u16)) {
                patch.specials.push((id as u8, t.specials[id].clone()));
            }
        }
        for &e in &self.touched_enemies {
            patch.enemies.push((e as u16, t.enemies[e].clone()));
        }
        patch.behaviors = std::mem::take(&mut self.behaviors);
        patch.enemy_behavior = std::mem::take(&mut self.enemy_behavior);
        let mut tables = self.t;
        tables.apply(&TablePatch {
            behaviors: patch.behaviors.clone(),
            enemy_behavior: patch.enemy_behavior.clone(),
            ..Default::default()
        });
        ResolvedMods {
            episode,
            patch,
            sprites: self.sprites.out,
            starter: self.starter,
            shop: self.shop,
            identities: self.identities,
            new_enemies: self.new_enemies,
            enemy_weapons: self.enemy_weapons,
            levels: self.levels,
            reports,
            diagnostics: self.diag,
            tables: Some(tables),
        }
    }
}

/// The published schema (`rewrite/mods/mod.schema.json`) accepts exactly the
/// keys that the loader reads, object by object, and rejects all others
/// (`additionalProperties: false`): a mod valid for one is valid for the other.
#[cfg(test)]
mod schema_tests {
    use super::enemy_reading::*;
    use super::reading::*;
    use crate::json::{self, Value};

    fn keys(v: &Value) -> Vec<String> {
        let mut k: Vec<String> =
            v.get("properties").and_then(Value::as_object).unwrap().iter().map(|(k, _)| k.clone()).collect();
        k.sort();
        k
    }

    fn sorted(list: &[&str]) -> Vec<String> {
        let mut k: Vec<String> = list.iter().map(|s| s.to_string()).collect();
        k.sort();
        k
    }

    #[test]
    fn schema_keys_match_the_loader() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../mods/mod.schema.json");
        let schema = json::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        let def = |name: &str| {
            schema.get("definitions").and_then(|d| d.get(name)).unwrap_or_else(|| panic!("definition {name}")).clone()
        };
        let behavior = def("behavior");
        let bp = behavior.get("properties").unwrap();
        let mut enemy_keys = ENEMY_KEYS.to_vec();
        enemy_keys.push("original");
        let cases: Vec<(&str, Value, Vec<String>)> = vec![
            ("root", schema.clone(), sorted(TOP_KEYS)),
            ("weapon", def("weapon"), sorted(WEAPON_KEYS)),
            ("firing", def("firing"), sorted(FIRING_KEYS)),
            ("projectile", def("projectile"), sorted(PROJECTILE_KEYS)),
            ("ship", def("ship"), sorted(SHIP_KEYS)),
            (
                "ship_poses",
                def("ship").get("properties").unwrap().get("ship_poses").unwrap().clone(),
                sorted(SHIP_POSE_KEYS),
            ),
            ("sidekick", def("sidekick"), sorted(SIDEKICK_KEYS)),
            ("shield", def("shield"), sorted(SHIELD_KEYS)),
            ("special", def("special"), sorted(SPECIAL_KEYS)),
            ("starter", def("starter"), sorted(STARTER_KEYS)),
            ("enemy", def("enemy"), sorted(&enemy_keys)),
            ("behavior", behavior.clone(), sorted(BEHAVIOR_KEYS)),
            ("behavior.path", bp.get("path").unwrap().get("items").unwrap().clone(), sorted(PATH_KEYS)),
            ("behavior.track", bp.get("track").unwrap().clone(), sorted(TRACK_KEYS)),
            ("behavior.turrets", bp.get("turrets").unwrap().get("items").unwrap().clone(), sorted(TURRET_KEYS)),
        ];
        for (name, obj, loader) in cases {
            assert_eq!(keys(&obj), loader, "{name}: schema keys ≠ loader keys");
            assert_eq!(
                obj.get("additionalProperties"),
                Some(&Value::Bool(false)),
                "{name}: additionalProperties must be false"
            );
        }
    }
}
