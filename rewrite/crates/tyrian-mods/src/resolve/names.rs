//! Name resolution: original objects cited by `base`, identities of
//! created objects, cited sprites, enemy weapons, original enemies, start,
//! ship images and icons (final passes of `mod_manager.c`).

use super::*;

/// Bytes of the name as seen by the C code (`PascalName::text` without conversion).
pub(super) fn cname(p: &PascalName) -> Vec<u8> {
    let n = (p.len as usize).min(30);
    let mut s = &p.bytes[..n];
    if let Some(z) = s.iter().position(|&b| b == 0) {
        s = &s[..z];
    }
    let mut end = s.len();
    while end > 1 && s[end - 1] == b' ' {
        end -= 1;
    }
    s[..end].to_vec()
}

/// `get_name(obj, key, dst, max)` from mod_manager.c: `strncpy(dst, s, max - 1)`.
pub(super) fn set_name(p: &mut PascalName, s: &str, max_chars: usize) {
    let b = s.as_bytes();
    let n = b.len().min(max_chars);
    let mut bytes = [0u8; 30];
    bytes[..n].copy_from_slice(&b[..n]);
    p.len = n as u8;
    p.bytes = bytes;
}

/// `mod_manager.c:name_equiv`: case-insensitive, ignoring spaces, hyphens and underscores.
pub(super) fn name_equiv(a: &[u8], b: &[u8]) -> bool {
    let norm = |s: &[u8]| -> Vec<u8> {
        s.iter().filter(|&&c| c != b' ' && c != b'-' && c != b'_').map(|c| c.to_ascii_lowercase()).collect()
    };
    norm(a) == norm(b)
}

impl levels::LevelNames for Ctx<'_> {
    fn enemy(&mut self, name: &str) -> Result<u16, String> {
        self.enemy_ref(name)
    }
    fn special(&mut self, name: &str) -> Result<u8, String> {
        self.special_by_name(name)
    }
}

impl Ctx<'_> {
    // ---------------------------------------------------------------- identities

    /// `mod_identity.c:mod_identity_register`: folder + non-localized name.
    pub(super) fn register_identity(&mut self, kind: ItemKind, id: usize, folder: &str, name: &str) -> bool {
        if folder.is_empty() || name.is_empty() {
            return false;
        }
        let key = format!("{}:{}/{}", folder.len(), folder, name);
        if key.len() >= MOD_ITEM_KEY_MAX {
            return false;
        }
        if self.identities.iter().any(|((k, _), v)| *k == kind && *v == key) {
            return false;
        }
        self.identities.insert((kind, id as u16), key);
        true
    }

    /// Sprite cited by its name (original manifest or mod PNG); 0 if absent.
    pub(super) fn sprite_ref(&mut self, v: Option<&Value>, folder: &str, ctx: &str) -> u16 {
        match v {
            None => 0,
            Some(Value::String(s)) => match self.names.lookup(s) {
                Some(i) => i as u16,
                None => {
                    self.diag
                        .error(folder, format!("{ctx}: unknown sprite '{s}' (data/sprite_manifest.json or mod PNG)"));
                    0
                }
            },
            Some(other) => {
                self.diag.error(folder, format!("{ctx}: sprite {other:?} — a sprite is referenced by name"));
                0
            }
        }
    }

    /// Original enemy cited by its name (`originals.rs`, `rewrite/mods/original-names.json`).
    pub(super) fn original_enemy_name<'v>(&mut self, v: &'v Value, folder: &str, ctx: &str) -> Option<&'v str> {
        match v {
            Value::String(n) if self.originals.enemies.contains_key(n.as_str()) => Some(n),
            Value::String(n) => {
                self.diag
                    .error(folder, format!("{ctx}: unknown original enemy '{n}' (rewrite/mods/original-names.json)"));
                None
            }
            other => {
                self.diag.error(
                    folder,
                    format!(
                        "{ctx}: {other:?} — an original enemy is referenced by name (rewrite/mods/original-names.json)"
                    ),
                );
                None
            }
        }
    }

    /// Slot of the original enemy `name` in the episode table.
    pub(super) fn original_enemy_slot(&self, name: &str) -> Option<usize> {
        self.originals.enemies.get(name).and_then(|e| e.slots[self.group]).map(usize::from)
    }

    /// Current record of an original enemy: that of the episode
    /// table (modifications by previous mods included) if it appears there,
    /// otherwise that of the catalog.
    pub(super) fn original_enemy_record(&self, name: &str) -> EnemyDat {
        match self.original_enemy_slot(name) {
            Some(s) => self.t.enemies[s].clone(),
            None => self.originals.enemies[name].record.clone(),
        }
    }

    /// Free slot in the range of new enemies.
    pub(super) fn free_new_enemy_slot(&self) -> Option<usize> {
        let used: BTreeSet<u16> = self.new_enemies.values().chain(self.imported_originals.values()).copied().collect();
        (NEW_ENEMY_RANGE.0..=NEW_ENEMY_RANGE.1).find(|i| !used.contains(&(*i as u16)))
    }

    /// Enemy cited by a reference (`elaunchtype`, `eenemydie`, level
    /// event): new mod enemy or original enemy. An original enemy
    /// absent from the episode table is copied there into a
    /// new-enemy slot, so that the reference holds everywhere.
    pub(super) fn enemy_ref(&mut self, name: &str) -> Result<u16, String> {
        if let Some(&id) = self.new_enemies.get(name) {
            return Ok(id);
        }
        let originals = self.originals;
        let Some(e) = originals.enemies.get(name) else {
            return Err(format!("unknown enemy '{name}' (a new enemy defined above, or an original enemy from rewrite/mods/original-names.json)"));
        };
        if let Some(s) = e.slots[self.group] {
            return Ok(s);
        }
        if let Some(&id) = self.imported_originals.get(name) {
            return Ok(id);
        }
        let rec = e.record.clone();
        let Some(id) = self.free_new_enemy_slot() else {
            return Err(format!("enemy '{name}': no free new-enemy slot left"));
        };
        self.t.enemies[id] = rec;
        self.touched_enemies.insert(id);
        self.imported_originals.insert(name.to_string(), id as u16);
        Ok(id as u16)
    }

    /// Special weapon (original object or from an already loaded mod) cited by its name.
    pub(super) fn special_by_name(&self, name: &str) -> Result<u8, String> {
        (1..=SPECIAL_NUM)
            .find(|&i| {
                let n = cname(&self.t.specials[i].name);
                !n.is_empty() && name_equiv(&n, name.as_bytes())
            })
            .map(|i| i as u8)
            .ok_or_else(|| format!("unknown special weapon '{name}'"))
    }

    /// Original weapon 1..255 cited by its name (turrets, chain reactions).
    pub(super) fn original_weapon(&self, name: &str) -> Result<u16, String> {
        match self.originals.weapons.get(name) {
            None => Err(format!("unknown original weapon '{name}' (rewrite/mods/original-names.json)")),
            Some(w) => w.slots[self.group].ok_or_else(|| {
                format!(
                    "original weapon '{name}' missing from the tables of episodes {}",
                    crate::originals::TABLE_GROUPS[self.group]
                )
            }),
        }
    }

    /// Weapon of a behavior turret: mod enemy weapon or original weapon, by its name.
    pub(super) fn resolve_weapon_ref(&mut self, v: Option<&Value>, folder: &str, ctx: &str) -> Option<u16> {
        let r = match v {
            Some(Value::String(s)) => match self.enemy_weapons.get(s) {
                Some(&w) => Ok(w),
                None => {
                    self.original_weapon(s).map_err(|e| format!("{e}; nor an enemy weapon of the mod (enemy_weapons)"))
                }
            },
            Some(other) => Err(format!("{other:?} — a weapon is referenced by name")),
            None => Err("missing 'weapon'".to_string()),
        };
        match r {
            Ok(w) => Some(w),
            Err(e) => {
                self.diag.error(folder, format!("{ctx}: {e}"));
                None
            }
        }
    }

    /// Resolves the names of the `starter` section after all mods are loaded.
    pub(super) fn apply_pending_starter_names(&mut self) {
        let pending = std::mem::take(&mut self.pending_starter);
        for (key, (folder, name)) in pending {
            let b = name.as_bytes();
            let t = &self.t;
            let found = match key {
                "ship" => (0..=SHIP_NUM).find(|&i| {
                    let n = cname(&t.ships[i].name);
                    !n.is_empty() && n == b
                }),
                "front_weapon" | "rear_weapon" => (0..=PORT_NUM).find(|&i| {
                    let n = cname(&t.ports[i].name);
                    !n.is_empty() && n == b
                }),
                "shield" => (0..=SHIELD_NUM).find(|&i| {
                    let n = cname(&t.shields[i].name);
                    !n.is_empty() && n == b
                }),
                "generator" => (0..=POWER_NUM).find(|&i| {
                    let n = cname(&t.generators[i].name);
                    !n.is_empty() && n == b
                }),
                _ => (0..=OPTION_NUM).find(|&i| {
                    let n = cname(&t.options[i].name);
                    !n.is_empty() && n == b
                }),
            };
            match found {
                Some(id) => *self.starter_field(key) = id as i32,
                None => self.diag.error(&folder, format!("starter '{key}': no item named '{name}'")),
            }
        }
    }

    // ---------------------------------------------------------------- final passes

    /// `mod_manager.c:resolve_mod_graphics_names`.
    pub(super) fn resolve_graphics_names(&mut self) {
        let lookups: Vec<(usize, String, String)> = self
            .pending_ship_sprite
            .iter()
            .map(|(&id, s)| (id, s.clone(), self.pending_ship_icon.get(&id).cloned().unwrap_or_default()))
            .collect();
        for (id, sprite, icon) in lookups {
            if !sprite.is_empty() {
                match self.names.lookup(&sprite) {
                    Some(r) => self.t.ships[id].shipgraphic = r as u16,
                    None => self.diag.warn(
                        "",
                        format!(
                            "ship '{}': ship_sprite '{sprite}' not found",
                            String::from_utf8_lossy(&cname(&self.t.ships[id].name))
                        ),
                    ),
                }
            }
            if !icon.is_empty() {
                match self.names.lookup(&icon) {
                    Some(r) => self.t.ships[id].itemgraphic = r as u16,
                    None => self.diag.warn("", format!("ship: icon_sprite '{icon}' not found")),
                }
            }
        }
        macro_rules! icons {
            ($pending:ident, $table:ident) => {
                let v: Vec<(usize, String)> =
                    self.$pending.iter().filter(|(_, s)| !s.is_empty()).map(|(&i, s)| (i, s.clone())).collect();
                for (id, name) in v {
                    match self.names.lookup(&name) {
                        Some(r) => self.t.$table[id].itemgraphic = r as u16,
                        None => self.diag.warn("", format!("icon_sprite '{name}' not found (item {id})")),
                    }
                }
            };
        }
        icons!(pending_weapon_icon, ports);
        icons!(pending_sidekick_icon, options);
        icons!(pending_shield_icon, shields);
        icons!(pending_special_icon, specials);
    }
}
