//! `TyrianMods` node: exposes to Godot the mod system of `tyrian-mods`
//! (ADR 0080). Godot interprets no mod file: it receives the list
//! of mods, the diagnostics, the sprites to override, the shop
//! items and the starting equipment, already validated by Rust.
//!
//! `TyrianSim.set_mods(mods)` hooks the mods into the next `load_level`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

use godot::prelude::*;
use tyrian_formats::campaign_save::ItemNames;
use tyrian_mods::{episode_items, ItemKind, ModSet, ResolvedMods, Severity};
use tyrian_sim::shop::AlwaysAvailable;
use tyrian_sim::tables::Tables;
use tyrian_sim::{LevelSetup, Loadout};

// `tool`: the level editor plugin calls `export_level` in the editor.
#[derive(GodotClass)]
#[class(tool, base = RefCounted)]
pub struct TyrianMods {
    data_dir: PathBuf,
    dirs: Vec<PathBuf>,
    /// Player preferences (folder -> enabled), taking priority over `mod.json`.
    overrides: BTreeMap<String, bool>,
    set: Option<ModSet>,
    /// Resolution per episode (the tables differ from one episode to another).
    resolved: BTreeMap<u8, ResolvedMods>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for TyrianMods {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            data_dir: PathBuf::new(),
            dirs: Vec::new(),
            overrides: BTreeMap::new(),
            set: None,
            resolved: BTreeMap::new(),
            base,
        }
    }
}

fn kind_name(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Ship => "ship",
        ItemKind::Weapon => "weapon",
        ItemKind::Sidekick => "sidekick",
        ItemKind::Shield => "shield",
        ItemKind::Special => "special",
    }
}

#[godot_api]
impl TyrianMods {
    /// Reads the mod folders (`mods_dirs`, in priority order) with
    /// the manifest of `data_dir`. Returns the number of active mods.
    #[func]
    fn load(&mut self, data_dir: GString, mods_dirs: PackedStringArray) -> i32 {
        self.data_dir = PathBuf::from(data_dir.to_string());
        self.dirs = mods_dirs.as_slice().iter().map(|s| PathBuf::from(s.to_string())).collect();
        self.reload()
    }

    /// Player preference for a mod; taken into account at the next `reload`.
    #[func]
    fn set_enabled(&mut self, folder: GString, enabled: bool) {
        self.overrides.insert(folder.to_string(), enabled);
    }

    /// Rereads the folders (hot reload when leaving the mods menu).
    #[func]
    fn reload(&mut self) -> i32 {
        let mut set = ModSet::load_dirs(&self.data_dir, &self.dirs);
        for (folder, &on) in &self.overrides {
            set.set_enabled(folder, on);
        }
        let n = set.enabled_count() as i32;
        self.set = Some(set);
        self.resolved.clear();
        n
    }

    /// One dictionary per mod: folder, name, author, version, description,
    /// readme, enabled, error, path and the counters of episode `episode`.
    #[func]
    fn list(&mut self, episode: i32) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(set) = self.set.clone() else { return out };
        let res = self.resolve(episode);
        for m in &set.folders {
            let r = res.and_then(|r| r.reports.iter().find(|x| x.folder == m.folder));
            let count = |f: fn(&tyrian_mods::ModReport) -> usize| r.map_or(0, f) as i64;
            out.push(&vdict! {
                "folder" => m.folder.as_str(),
                "name" => m.name.as_str(),
                "author" => m.author.as_str(),
                "version" => m.version.as_str(),
                "description" => m.description.as_str(),
                "readme" => m.readme.as_str(),
                "enabled" => m.enabled,
                "error" => m.error.as_str(),
                "error_key" => m.error_key.as_str(),
                // Arguments of the translated message, separated by a tab.
                "error_args" => m.error_args.join("\t").as_str(),
                "path" => m.path.to_string_lossy().as_ref(),
                "weapons" => count(|r| r.weapons),
                "ships" => count(|r| r.ships),
                "sidekicks" => count(|r| r.sidekicks),
                "shields" => count(|r| r.shields),
                "specials" => count(|r| r.specials),
                "enemies" => count(|r| r.enemies),
                "new_enemies" => count(|r| r.new_enemies),
                "levels" => count(|r| r.levels),
                "sprites" => count(|r| r.sprites),
            });
        }
        out
    }

    /// Load diagnostics for the episode: severity (info|warning|error), folder, message.
    #[func]
    fn diagnostics(&mut self, episode: i32) -> Array<VarDictionary> {
        let mut out = Array::new();
        if let Some(res) = self.resolve(episode) {
            for d in &res.diagnostics.items {
                let sev = match d.severity {
                    Severity::Info => "info",
                    Severity::Warning => "warning",
                    Severity::Error => "error",
                };
                out.push(&vdict! { "severity" => sev, "folder" => d.folder.as_str(), "message" => d.message.as_str() });
            }
        }
        out
    }

    /// Sprites to override: table (shots, ships, ...), index, path (PNG), hue
    /// (-1 = no tint), bank (shapebank for the enemies table).
    #[func]
    fn sprites(&mut self, episode: i32) -> Array<VarDictionary> {
        let mut out = Array::new();
        if let Some(res) = self.resolve(episode) {
            for s in &res.sprites {
                out.push(&vdict! {
                    "table" => s.table.name(),
                    "index" => s.index,
                    "path" => s.path.to_string_lossy().as_ref(),
                    "hue" => s.hue.unwrap_or(-1.0),
                    "bank" => s.bank,
                });
            }
        }
        out
    }

    /// Items added by the mods: one dictionary per object
    /// (kind, id, name, cost, itemgraphic, shipgraphic, always_available).
    #[func]
    fn items(&mut self, episode: i32) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(res) = self.resolve(episode) else { return out };
        let Some(t) = res.tables.as_ref() else { return out };
        for (&(kind, id), key) in &res.identities {
            let i = id as usize;
            let (name, cost, icon, ship_gr, shop_key) = match kind {
                ItemKind::Ship => {
                    (t.ships[i].name.text(), t.ships[i].cost, t.ships[i].itemgraphic, t.ships[i].shipgraphic, "ships")
                }
                ItemKind::Weapon => (t.ports[i].name.text(), t.ports[i].cost, t.ports[i].itemgraphic, 0, "weapons"),
                ItemKind::Sidekick => {
                    (t.options[i].name.text(), t.options[i].cost, t.options[i].itemgraphic, 0, "sidekicks")
                }
                ItemKind::Shield => {
                    (t.shields[i].name.text(), t.shields[i].cost, t.shields[i].itemgraphic, 0, "shields")
                }
                ItemKind::Special => (t.specials[i].name.text(), 0, t.specials[i].itemgraphic, 0, "specials"),
            };
            let always = res.shop.get(shop_key).is_some_and(|v| v.contains(&(id as u8)));
            out.push(&vdict! {
                "kind" => kind_name(kind),
                "id" => id as i64,
                "key" => key.as_str(),
                "name" => name.as_str(),
                "cost" => cost as i64,
                "itemgraphic" => icon as i64,
                "shipgraphic" => ship_gr as i64,
                "always_available" => always,
            });
        }
        out
    }

    /// Starting equipment (order of `TyrianSim.set_loadout`) after the
    /// `starter` section of the active mods (mainint.c:JE_initPlayerData).
    #[func]
    fn starter_loadout(&mut self, episode: i32, base_loadout: PackedInt32Array) -> PackedInt32Array {
        let v = base_loadout.as_slice();
        if v.len() < 11 {
            return base_loadout.clone();
        }
        let b = |i: usize| v[i].clamp(0, 255) as u8;
        let mut l = Loadout {
            ship: ShipId(b(0)),
            front_weapon: WeaponId(b(1)),
            front_power: b(2),
            rear_weapon: WeaponId(b(3)),
            rear_power: b(4),
            shield: ShieldId(b(5)),
            generator: GeneratorId(b(6)),
            left_sidekick: SidekickId(b(7)),
            right_sidekick: SidekickId(b(8)),
            special: SpecialId(b(9)),
            cash: v[10].max(0) as u32,
        };
        if let Some(res) = self.resolve(episode) {
            res.starter.apply(&mut l);
        }
        PackedInt32Array::from(vec![
            l.ship.0 as i32,
            l.front_weapon.0 as i32,
            l.front_power as i32,
            l.rear_weapon.0 as i32,
            l.rear_power as i32,
            l.shield.0 as i32,
            l.generator.0 as i32,
            l.left_sidekick.0 as i32,
            l.right_sidekick.0 as i32,
            l.special.0 as i32,
            l.cash as i32,
        ])
    }

    /// `shipgraphic` of ship `ship` in the episode tables, mods included.
    #[func]
    fn ship_graphic(&mut self, episode: i32, ship: i32) -> i32 {
        self.resolve(episode)
            .and_then(|r| r.tables.as_ref())
            .and_then(|t| t.ships.get(ship.max(0) as usize))
            .map_or(-1, |s| s.shipgraphic as i32)
    }

    /// Text of `levels/epN_NN.json` (named events, ADR 0128) for the
    /// events of a level opened in the editor (`LevelEventResource`).
    /// A text `eventdat` is the name of an enemy written by the author.
    /// Returns { "text": ..., "error": "" } or { "text": "", "error": message }.
    #[func]
    fn export_level(
        &mut self,
        episode: i32,
        lvl_file_num: i32,
        events: Array<VarDictionary>,
        random_enemies: PackedInt32Array,
    ) -> VarDictionary {
        use tyrian_formats::lvl::Event;
        use tyrian_mods::levels::EnemyCited;
        let ep = episode.clamp(1, 5) as u8;
        let result = (|| -> Result<String, String> {
            let set = self.set.as_ref().ok_or("call TyrianMods.load first")?;
            let (_, items) = episode_items(&self.data_dir, ep)?;
            let mut list = Vec::with_capacity(events.len());
            for d in events.iter_shared() {
                let int = |k: &str| d.get(k).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0);
                let named =
                    d.get("eventdat").and_then(|v| v.try_to::<GString>().ok()).map(|s| EnemyCited::Name(s.to_string()));
                let ev = Event {
                    eventtime: int("eventtime") as u16,
                    eventtype: int("eventtype") as u8,
                    eventdat: if named.is_some() { 0 } else { int("eventdat") as i16 },
                    eventdat2: int("eventdat2") as i16,
                    eventdat3: int("eventdat3") as i8,
                    eventdat4: int("eventdat4") as u8,
                    eventdat5: int("eventdat5") as i8,
                    eventdat6: int("eventdat6") as i8,
                };
                list.push((ev, named));
            }
            let random: Vec<EnemyCited> =
                random_enemies.as_slice().iter().map(|&r| EnemyCited::Slot(r.max(0) as u16)).collect();
            set.export_level(&items, ep, lvl_file_num.clamp(1, 99) as u16, &list, &random)
        })();
        match result {
            Ok(text) => vdict! { "text" => text.as_str(), "error" => "" },
            Err(e) => vdict! { "text" => "", "error" => e.as_str() },
        }
    }

    /// Check of a mod folder with the game code (that of
    /// `tyrian-mods check`): returns the error messages, empty if the mod
    /// is sound. `mod_path` is the mod folder (with its `mod.json`).
    #[func]
    fn check_mod(&mut self, mod_path: GString, episode: i32) -> PackedStringArray {
        let mut out = PackedStringArray::new();
        let Some((set, folder)) = self.single_mod_set(&mod_path.to_string()) else {
            out.push("mod.json not found");
            return out;
        };
        let ep = episode.clamp(1, 5) as u8;
        let items = match episode_items(&self.data_dir, ep) {
            Ok((_, items)) => items,
            Err(e) => {
                out.push(e.as_str());
                return out;
            }
        };
        let res = set.resolve(ep, &items);
        let mut messages: Vec<String> = res
            .diagnostics
            .items
            .iter()
            .filter(|d| d.severity == Severity::Error && (d.folder == folder || d.folder.is_empty()))
            .map(|d| d.message.clone())
            .collect();
        for r in res.reports.iter().filter(|r| r.folder == folder && !r.error.is_empty()) {
            if !messages.contains(&r.error) {
                messages.push(r.error.clone());
            }
        }
        for m in &messages {
            out.push(m.as_str());
        }
        out
    }

    /// Enemy names that can be cited in this mod: the mod's new enemies and the
    /// original enemies (`rewrite/mods/original-names.json`).
    #[func]
    fn enemy_names(&mut self, mod_path: GString, episode: i32) -> PackedStringArray {
        let mut out = PackedStringArray::new();
        let ep = episode.clamp(1, 5) as u8;
        let Some((set, _)) = self.single_mod_set(&mod_path.to_string()) else {
            return out;
        };
        if let Ok((_, items)) = episode_items(&self.data_dir, ep) {
            let res = set.resolve(ep, &items);
            for k in res.new_enemies.keys() {
                out.push(k.as_str());
            }
        }
        for k in set.originals.enemies.keys() {
            out.push(k.as_str());
        }
        out
    }

    /// Effective events of the level (after a possible `levels/epN_NN.json`),
    /// for the editor. Empty if no mod touches this level.
    #[func]
    fn level_override_source(&mut self, episode: i32, lvl_file_num: i32) -> GString {
        self.resolve(episode)
            .and_then(|r| r.levels.get(&(episode.clamp(1, 5) as u8, lvl_file_num.max(0) as u16)))
            .map_or(GString::new(), |l| GString::from(l.folder.as_str()))
    }
}

impl TyrianMods {
    /// Tables of the episode, mods included, and `always_available` items, for
    /// the shop (`game_menu.c:JE_itemScreen`). None if no mod is enabled.
    pub(crate) fn shop_data(&mut self, episode: i32) -> Option<(Tables, AlwaysAvailable)> {
        let res = self.resolve(episode)?;
        let tables = res.tables.clone()?;
        let list = |k: &str| res.shop.get(k).cloned().unwrap_or_default();
        let always = AlwaysAvailable {
            ships: list("ships"),
            front_weapons: list("weapons_front"),
            rear_weapons: list("weapons_rear"),
            sidekicks: list("sidekicks"),
            shields: list("shields"),
        };
        Some((tables, always))
    }

    /// Item names of the episode, mods included, for the saves: a mod item
    /// is written by its name, not by its number (which depends on the
    /// enabled mods). `base` provides the categories that mods do not touch.
    pub(crate) fn save_item_names(&mut self, episode: i32, base: &ItemNames) -> ItemNames {
        let Some(t) = self.resolve(episode).and_then(|r| r.tables.as_ref()) else { return base.clone() };
        let clean = |n: String| n.trim().to_string();
        ItemNames {
            ships: t.ships.iter().map(|x| clean(x.name.text())).collect(),
            weapon_ports: t.ports.iter().map(|x| clean(x.name.text())).collect(),
            shields: t.shields.iter().map(|x| clean(x.name.text())).collect(),
            sidekicks: t.options.iter().map(|x| clean(x.name.text())).collect(),
            specials: t.specials.iter().map(|x| clean(x.name.text())).collect(),
            generators: base.generators.clone(),
        }
    }

    /// Loads only the folder `mod_path` (force-enabled) with the original tables.
    fn single_mod_set(&self, mod_path: &str) -> Option<(ModSet, String)> {
        let path = Path::new(mod_path);
        let folder = path.file_name()?.to_str()?.to_string();
        let mut set = ModSet::load_dirs(&self.data_dir, &[path.parent()?.to_path_buf()]);
        set.folders.retain(|m| m.folder == folder);
        if set.folders.is_empty() {
            return None;
        }
        set.set_enabled(&folder, true);
        Some((set, folder))
    }

    fn resolve(&mut self, episode: i32) -> Option<&ResolvedMods> {
        let ep = episode.clamp(1, 5) as u8;
        if !self.resolved.contains_key(&ep) {
            let set = self.set.as_ref()?;
            let (_, items) = match episode_items(Path::new(&self.data_dir), ep) {
                Ok(x) => x,
                Err(e) => {
                    godot_error!("TyrianMods: unreadable tables for episode {ep}: {e}");
                    return None;
                }
            };
            self.resolved.insert(ep, set.resolve(ep, &items));
        }
        self.resolved.get(&ep)
    }

    /// Hooks the mods into a level about to start (called by `TyrianSim::load_level`).
    pub fn apply_to(&mut self, setup: &mut LevelSetup, lvl_file_num: u16) {
        let ep = setup.episode;
        if !self.resolved.contains_key(&ep) {
            if let Some(set) = self.set.as_ref() {
                let r = set.resolve(ep, &setup.items);
                self.resolved.insert(ep, r);
            }
        }
        if let Some(r) = self.resolved.get(&ep) {
            r.apply_to(setup, lvl_file_num);
        }
    }
}
