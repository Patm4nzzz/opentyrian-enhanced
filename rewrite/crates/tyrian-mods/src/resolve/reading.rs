//! Reading of `mod.json`: fields, allowed keys, player objects
//! (weapons, ships, wingmen, shields, specials), start and levels.

use super::*;

pub(super) fn get_int(obj: &Value, key: &str) -> Option<i32> {
    obj.get(key).filter(|v| matches!(v, Value::Number(_))).and_then(Value::as_c_int)
}

pub(super) fn int_or(obj: &Value, key: &str, def: i32) -> i32 {
    get_int(obj, key).unwrap_or(def)
}

pub(super) fn get_str<'a>(obj: &'a Value, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

pub(super) fn get_bool(obj: &Value, key: &str, def: bool) -> bool {
    obj.get(key).and_then(Value::as_bool).unwrap_or(def)
}

/// `stash_pending`: non-empty string, cut at 47 bytes.
pub(super) fn pending(obj: &Value, key: &str) -> String {
    match obj.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() => {
            let mut end = s.len().min(PENDING_NAME_MAX);
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            s[..end].to_string()
        }
        _ => String::new(),
    }
}

/// Recognized keys, per object: any other key is an error (typo
/// or leftover from an old format), never silently ignored.
/// `$schema` (path of the schema `rewrite/mods/mod.schema.json`, for
/// text editors) is accepted and ignored.
pub(super) const TOP_KEYS: &[&str] = &[
    "$schema",
    "format_version",
    "name",
    "author",
    "version",
    "description",
    "enabled",
    "tint",
    "weapons",
    "ships",
    "sidekicks",
    "shields",
    "specials",
    "enemy_weapons",
    "enemies",
    "starter",
];
pub(super) const WEAPON_KEYS: &[&str] =
    &["name", "base", "cost", "power_use", "icon_sprite", "always_available", "tint", "front", "rear"];
/// A weapon power level, or an enemy weapon (`name` in addition).
pub(super) const FIRING_KEYS: &[&str] = &[
    "name",
    "fire_delay",
    "projectile_count",
    "animation",
    "max_on_screen",
    "offset_x",
    "offset_y",
    "homing",
    "accel_x",
    "accel_y",
    "circle_radius",
    "sound",
    "trail",
    "ship_blast_filter",
    "projectiles",
];
pub(super) const PROJECTILE_KEYS: &[&str] =
    &["damage", "sub_delay", "speed_x", "speed_y", "spawn_x", "spawn_y", "sprite"];
pub(super) const SHIP_KEYS: &[&str] =
    &["name", "base", "cost", "damage_resistance", "ship_sprite", "ship_poses", "icon_sprite", "always_available"];
pub(super) const SHIP_POSE_KEYS: &[&str] = &["neutral", "left_full", "left", "right", "right_full"];
pub(super) const SIDEKICK_KEYS: &[&str] = &["name", "base", "cost", "trail", "ammo", "icon_sprite", "always_available"];
pub(super) const SHIELD_KEYS: &[&str] =
    &["name", "base", "cost", "shield_power", "max_power", "icon_sprite", "always_available"];
pub(super) const SPECIAL_KEYS: &[&str] = &["name", "base", "power", "subtype", "icon_sprite", "always_available"];
pub(super) const STARTER_KEYS: &[&str] =
    &["ship", "front_weapon", "rear_weapon", "shield", "generator", "sidekick_left", "sidekick_right"];

impl Ctx<'_> {
    pub(super) fn avail(&mut self, list: &'static str, id: usize, folder: &str, what: &str) {
        let v = self.shop.get_mut(list).expect("shop list");
        if v.len() < MAX_ALWAYS_AVAIL {
            v.push(id as u8);
        } else {
            self.diag
                .warn(folder, format!("{what} removed from the shop: MAX_ALWAYS_AVAIL ({MAX_ALWAYS_AVAIL}) reached"));
        }
    }

    pub(super) fn items<'v>(&mut self, root: &'v Value, key: &str, folder: &str) -> &'v [Value] {
        match root.get(key) {
            None => &[],
            Some(Value::Array(a)) => a,
            Some(v) => {
                self.diag.error(folder, format!("'{key}': array expected, found {}", v.type_name()));
                &[]
            }
        }
    }

    /// Error for any key outside `allowed` (an object expected).
    pub(super) fn check_keys(&mut self, obj: &Value, allowed: &[&str], folder: &str, ctx: &str) -> bool {
        let Some(kv) = obj.as_object() else {
            self.diag.error(folder, format!("{ctx}: object expected, found {}", obj.type_name()));
            return false;
        };
        let mut ok = true;
        for (k, _) in kv {
            if !allowed.contains(&k.as_str()) {
                self.diag.error(folder, format!("{ctx}: unknown key '{k}'"));
                ok = false;
            }
        }
        ok
    }

    // ---------------------------------------------------------------- weapons

    /// `mod_manager.c:load_mod_weapons_auto`.
    pub(super) fn load_weapons(&mut self, root: &Value, folder: &str) -> usize {
        let mut count = 0;
        for item in self.items(root, "weapons", folder) {
            let name = get_str(item, "name").unwrap_or("");
            if !self.check_keys(item, WEAPON_KEYS, folder, &format!("weapon '{name}'")) {
                continue;
            }
            let Some(id) = (WEAPON_RANGE.0..=WEAPON_RANGE.1).find(|&i| !self.weapon_used[i]) else {
                self.diag.error(
                    folder,
                    format!(
                        "weapon '{name}': no free weapon slot left ({} mod weapons at most)",
                        WEAPON_RANGE.1 - WEAPON_RANGE.0 + 1
                    ),
                );
                break;
            };
            if !self.register_identity(ItemKind::Weapon, id, folder, name) {
                self.diag.error(
                    folder,
                    format!("weapon '{name}': identity missing, duplicated or too long (field \"name\")"),
                );
                continue;
            }
            self.weapon_used[id] = true;
            let base = get_str(item, "base");
            if let Some(b) = base {
                match (0..=PORT_NUM).find(|&i| {
                    let n = cname(&self.t.ports[i].name);
                    !n.is_empty() && name_equiv(&n, b.as_bytes())
                }) {
                    Some(src) => self.t.ports[id] = self.t.ports[src].clone(),
                    None => self.diag.error(folder, format!("weapon '{name}': base '{b}' not found")),
                }
            }
            if let Some(s) = get_str(item, "name") {
                set_name(&mut self.t.ports[id].name, s, 29);
            }
            let icon = pending(item, "icon_sprite");
            let p = &mut self.t.ports[id];
            if base.is_none() && icon.is_empty() {
                p.itemgraphic = 13;
            }
            p.cost = int_or(item, "cost", if base.is_some() { p.cost as i32 } else { 1000 }) as u16;
            p.poweruse = int_or(item, "power_use", if base.is_some() { p.poweruse as i32 } else { 20 }) as u16;
            if base.is_none() {
                p.opnum = 1;
            }
            self.pending_weapon_icon.insert(id, icon);
            self.apply_weapon_levels(item, id, folder);
            if let Some(h) = parse_tint(item.get("tint")) {
                self.weapon_tint.insert(id, h);
            }
            if get_bool(item, "always_available", false) {
                let mut has_front = item.get("front").is_some();
                let mut has_rear = item.get("rear").is_some();
                if !has_front && !has_rear {
                    has_front = true;
                    has_rear = true;
                }
                if has_front {
                    self.avail("weapons_front", id, folder, name);
                }
                if has_rear {
                    self.avail("weapons_rear", id, folder, name);
                }
                if self.shop["weapons"].len() < MAX_ALWAYS_AVAIL {
                    self.shop.get_mut("weapons").unwrap().push(id as u8);
                }
            }
            count += 1;
        }
        count
    }

    /// `hdt_json.c:hdt_json_apply_mod_weapon_levels`.
    pub(super) fn apply_weapon_levels(&mut self, item: &Value, port: usize, folder: &str) {
        let front = item.get("front");
        let rear = item.get("rear");
        if front.is_none() && rear.is_none() {
            return;
        }
        let (mut fc, mut rc) = (0, 0);
        if let Some(f) = front {
            fc = self.load_port_levels(f, port, 0, folder);
            if let Some(r) = rear {
                rc = self.load_port_levels(r, port, 1, folder);
            }
        } else if let Some(r) = rear {
            rc = self.load_port_levels(r, port, 0, folder);
        }
        self.t.ports[port].opnum = if fc > 0 && rc > 0 { 2 } else { 1 };
    }

    pub(super) fn next_mod_weapon_id(&self) -> Option<usize> {
        (MOD_WEAPON_ID.0..=MOD_WEAPON_ID.1).find(|&i| !self.mod_weapon_id_used[i])
    }

    /// Power levels of a weapon: each level receives a free projectile
    /// identifier (internal assignment, never written in the mod).
    pub(super) fn load_port_levels(&mut self, arr: &Value, port: usize, op: usize, folder: &str) -> usize {
        let Some(levels) = arr.as_array() else {
            self.diag.error(folder, "'front' / 'rear': array of levels expected");
            return 0;
        };
        if levels.len() > 11 {
            self.diag
                .error(folder, format!("{} power levels, maximum 11: the following ones are ignored", levels.len()));
        }
        let mut idx = 0;
        for level in levels.iter().take(11) {
            let Some(wid) = self.next_mod_weapon_id() else {
                self.diag.error(
                    folder,
                    format!(
                        "no free projectile identifier left ({} at most): level ignored",
                        MOD_WEAPON_ID.1 - MOD_WEAPON_ID.0 + 1
                    ),
                );
                idx += 1;
                continue;
            };
            self.mod_weapon_id_used[wid] = true;
            self.t.ports[port].op[op][idx] = wid as u16;
            self.fill_weapon(level, wid, folder, &format!("level {}", idx + 1));
            idx += 1;
        }
        idx
    }

    /// A weapon level (or an enemy weapon): same fields as `hdt_json.c:fill_mod_weapon`.
    pub(super) fn fill_weapon(&mut self, obj: &Value, wid: usize, folder: &str, ctx: &str) {
        if wid > WEAP_NUM {
            return;
        }
        self.check_keys(obj, FIRING_KEYS, folder, ctx);
        self.touched_weapons.insert(wid);
        let mut sprites = [0u16; 8];
        let mut damages = [0u8; 8];
        let projs = obj.get("projectiles").and_then(Value::as_array);
        if let Some(projs) = projs {
            if projs.len() > 8 {
                self.diag.error(folder, format!("{ctx}: {} projectiles, maximum 8", projs.len()));
            }
            for (i, p) in projs.iter().take(8).enumerate() {
                let pctx = format!("{ctx}, projectile {}", i + 1);
                self.check_keys(p, PROJECTILE_KEYS, folder, &pctx);
                sprites[i] = self.sprite_ref(p.get("sprite"), folder, &pctx);
                match self.damage_code(p.get("damage")) {
                    Ok(d) => damages[i] = d,
                    Err(e) => self.diag.error(folder, format!("{pctx}: damage: {e}")),
                }
            }
        }
        let w = &mut self.t.weapons[wid];
        // `drain` is read by no game code: reset to zero, never exposed to the mod.
        w.drain = 0;
        w.shotrepeat = int_or(obj, "fire_delay", 0) as u8;
        w.multi = int_or(obj, "projectile_count", 0) as u8;
        w.weapani = int_or(obj, "animation", 0) as u16;
        w.max = int_or(obj, "max_on_screen", 0) as u8;
        w.tx = int_or(obj, "offset_x", 0) as u8;
        w.ty = int_or(obj, "offset_y", 0) as u8;
        w.aim = int_or(obj, "homing", 0) as u8;
        w.acceleration = int_or(obj, "accel_y", 0) as i8;
        w.accelerationx = int_or(obj, "accel_x", 0) as i8;
        w.circlesize = int_or(obj, "circle_radius", 0) as u8;
        w.sound = int_or(obj, "sound", 0) as u8;
        w.trail = int_or(obj, "trail", 255) as u8;
        w.shipblastfilter = int_or(obj, "ship_blast_filter", 0) as u8;
        if let Some(Value::Bool(b)) = obj.get("homing") {
            w.aim = *b as u8;
        }
        if let Some(projs) = projs {
            w.attack = [0; 8];
            w.del = [0; 8];
            w.sx = [0; 8];
            w.sy = [0; 8];
            w.bx = [0; 8];
            w.by = [0; 8];
            w.sg = sprites;
            for (i, p) in projs.iter().take(8).enumerate() {
                w.attack[i] = damages[i];
                w.del[i] = int_or(p, "sub_delay", 255) as u8;
                w.sx[i] = int_or(p, "speed_x", 0) as i8;
                w.sy[i] = int_or(p, "speed_y", 0) as i8;
                w.bx[i] = int_or(p, "spawn_x", 0) as i8;
                w.by[i] = int_or(p, "spawn_y", 0) as i8;
            }
        }
        let filled = w.sg.iter().filter(|&&g| g != 0).count() as u8;
        if w.max > filled && filled > 0 {
            self.diag.warn(folder, format!("{ctx}: max_on_screen={} > drawn projectiles={filled}, lowered", w.max));
            w.max = filled;
        }
        if w.max == 0 && filled > 0 {
            w.max = filled;
        }
        // shots.c indexes arrays of 8 by shotMultiPos ≤ max (hdt_json.c:apply_weapon).
        if w.max > 8 {
            w.max = 8;
        }
        if w.sg[0] == 0 && w.multi > 0 {
            self.diag.warn(
                folder,
                format!(
                    "{ctx}: first projectile without a sprite with projectile_count={} — the shot will not be drawn",
                    w.multi
                ),
            );
        }
    }

    /// Damage of a projectile. The game encodes special effects in the value
    /// (`shots.c`, `shot_update.c`); a mod names them:
    /// - a number 0..98: damage per hit;
    /// - `"freeze"`: freezes the enemy without damage (code 99);
    /// - `{ "pierce": n }` (n 0..5): passes through enemies, n damage to each (250 + n);
    /// - `{ "chain": "<original weapon>" }`: 1 damage, the impact fires this weapon (100 + weapon, weapons 1..149).
    pub(super) fn damage_code(&self, v: Option<&Value>) -> Result<u8, String> {
        const HELP: &str = "number 0..98, \"freeze\", { \"pierce\": 0..5 } or { \"chain\": \"<original weapon>\" }";
        match v {
            None => Ok(0),
            Some(Value::Number(_)) => match v.and_then(Value::as_c_int) {
                Some(n) if (0..=98).contains(&n) => Ok(n as u8),
                Some(n) => Err(format!("{n}: special effects are written by name ({HELP})")),
                None => Err(HELP.to_string()),
            },
            Some(Value::String(s)) if s == "freeze" => Ok(99),
            Some(o @ Value::Object(kv)) if kv.len() == 1 => match (o.get("pierce"), o.get("chain")) {
                (Some(p), None) => match get_int(o, "pierce") {
                    Some(n) if (0..=5).contains(&n) => Ok(250 + n as u8),
                    _ => Err(format!("pierce {p:?}: 0..5 expected")),
                },
                (None, Some(Value::String(c))) => {
                    let w = self.original_weapon(c)?;
                    if w > crate::originals::MAX_CHAIN_WEAPON {
                        return Err(format!(
                            "chain: weapon '{c}' (slot {w}) cannot be reached by a chain reaction (1..{})",
                            crate::originals::MAX_CHAIN_WEAPON
                        ));
                    }
                    Ok(100 + w as u8)
                }
                _ => Err(HELP.to_string()),
            },
            Some(_) => Err(HELP.to_string()),
        }
    }

    // ---------------------------------------------------------------- ships & co

    /// `mod_manager.c:load_mod_ships_auto` (+ ship_poses from wip/sept-2026).
    pub(super) fn load_ships(&mut self, root: &Value, folder: &str) -> usize {
        let mut count = 0;
        for item in self.items(root, "ships", folder) {
            let name = get_str(item, "name").unwrap_or("");
            if !self.check_keys(item, SHIP_KEYS, folder, &format!("ship '{name}'")) {
                continue;
            }
            let Some(id) = (SHIP_RANGE.0..=SHIP_RANGE.1).find(|&i| !self.ship_used[i]) else {
                self.diag.error(
                    folder,
                    format!("ship '{name}': no free slot left ({} at most)", SHIP_RANGE.1 - SHIP_RANGE.0 + 1),
                );
                break;
            };
            if !self.register_identity(ItemKind::Ship, id, folder, name) {
                self.diag.error(folder, format!("ship '{name}': identity missing, duplicated or too long"));
                continue;
            }
            self.ship_used[id] = true;
            let base = get_str(item, "base");
            if let Some(b) = base {
                match (0..=SHIP_NUM).find(|&i| {
                    let n = cname(&self.t.ships[i].name);
                    !n.is_empty() && name_equiv(&n, b.as_bytes())
                }) {
                    Some(src) => self.t.ships[id] = self.t.ships[src].clone(),
                    None => self.diag.error(folder, format!("ship '{name}': base '{b}' not found")),
                }
            }
            if let Some(s) = get_str(item, "name") {
                set_name(&mut self.t.ships[id].name, s, 29);
            }
            let s = &mut self.t.ships[id];
            s.dmg = int_or(item, "damage_resistance", if base.is_some() { s.dmg as i32 } else { 20 }) as u8;
            s.cost = int_or(item, "cost", if base.is_some() { s.cost as i32 } else { 5000 }) as u16;
            if base.is_none() {
                s.bigshipgraphic = 32;
            }
            let mut sprite = pending(item, "ship_sprite");
            let icon = pending(item, "icon_sprite");
            if let Some(poses) = item.get("ship_poses") {
                self.check_keys(poses, SHIP_POSE_KEYS, folder, &format!("ship '{name}', ship_poses"));
                sprite = pending(poses, "neutral");
                let p = ["left_full", "left", "right", "right_full"].map(|k| pending(poses, k));
                self.pending_ship_poses.insert(id, p);
            }
            let s = &mut self.t.ships[id];
            if base.is_none() && sprite.is_empty() {
                s.shipgraphic = (400 + (id - SHIP_RANGE.0) * 5) as u16;
            }
            if base.is_none() {
                s.itemgraphic = 277;
            }
            self.pending_ship_sprite.insert(id, sprite);
            self.pending_ship_icon.insert(id, icon);
            if get_bool(item, "always_available", false) {
                self.avail("ships", id, folder, name);
            }
            count += 1;
        }
        count
    }

    /// `mod_manager.c:load_mod_sidekicks_auto`.
    pub(super) fn load_sidekicks(&mut self, root: &Value, folder: &str) -> usize {
        let mut count = 0;
        for item in self.items(root, "sidekicks", folder) {
            let name = get_str(item, "name").unwrap_or("");
            if !self.check_keys(item, SIDEKICK_KEYS, folder, &format!("sidekick '{name}'")) {
                continue;
            }
            let Some(id) = (SIDEKICK_RANGE.0..=SIDEKICK_RANGE.1).find(|&i| !self.sidekick_used[i]) else {
                self.diag.error(
                    folder,
                    format!(
                        "sidekick '{name}': no free slot left ({} at most)",
                        SIDEKICK_RANGE.1 - SIDEKICK_RANGE.0 + 1
                    ),
                );
                break;
            };
            if !self.register_identity(ItemKind::Sidekick, id, folder, name) {
                self.diag.error(folder, format!("sidekick '{name}': identity missing, duplicated or too long"));
                continue;
            }
            self.sidekick_used[id] = true;
            let base = get_str(item, "base");
            if let Some(b) = base {
                match (0..=OPTION_NUM).find(|&i| {
                    let n = cname(&self.t.options[i].name);
                    !n.is_empty() && name_equiv(&n, b.as_bytes())
                }) {
                    Some(src) => self.t.options[id] = self.t.options[src].clone(),
                    None => self.diag.error(folder, format!("sidekick '{name}': base '{b}' not found")),
                }
            }
            if let Some(s) = get_str(item, "name") {
                set_name(&mut self.t.options[id].name, s, 29);
            }
            let icon = pending(item, "icon_sprite");
            let o = &mut self.t.options[id];
            if base.is_none() && icon.is_empty() {
                o.itemgraphic = 13;
            }
            o.cost = int_or(item, "cost", if base.is_some() { o.cost as i32 } else { 5000 }) as u16;
            o.tr = int_or(item, "trail", if base.is_some() { o.tr as i32 } else { 0 }) as u8;
            o.ammo = int_or(item, "ammo", if base.is_some() { o.ammo as i32 } else { 0 }) as u8;
            if base.is_none() {
                o.option = 2;
            }
            self.pending_sidekick_icon.insert(id, icon);
            if get_bool(item, "always_available", false) {
                self.avail("sidekicks", id, folder, name);
            }
            count += 1;
        }
        count
    }

    /// `mod_manager.c:clamp_shield`.
    pub(super) fn clamp_shield(&mut self, obj: &Value, key: &str, def: i32, max: i32, folder: &str) -> u8 {
        let v = int_or(obj, key, def);
        if v < 0 || v > max {
            self.diag.warn(folder, format!("shield {key}={v} outside 0..{max}, clamped"));
            return if v < 0 { 0 } else { max as u8 };
        }
        v as u8
    }

    /// `mod_manager.c:load_mod_shields_auto`.
    pub(super) fn load_shields(&mut self, root: &Value, folder: &str) -> usize {
        let mut count = 0;
        for item in self.items(root, "shields", folder) {
            let name = get_str(item, "name").unwrap_or("");
            if !self.check_keys(item, SHIELD_KEYS, folder, &format!("shield '{name}'")) {
                continue;
            }
            let Some(id) = (SHIELD_RANGE.0..=SHIELD_RANGE.1).find(|&i| !self.shield_used[i]) else {
                self.diag.error(
                    folder,
                    format!("shield '{name}': no free slot left ({} at most)", SHIELD_RANGE.1 - SHIELD_RANGE.0 + 1),
                );
                break;
            };
            if !self.register_identity(ItemKind::Shield, id, folder, name) {
                self.diag.error(folder, format!("shield '{name}': identity missing, duplicated or too long"));
                continue;
            }
            self.shield_used[id] = true;
            let base = get_str(item, "base");
            if let Some(b) = base {
                match (0..=SHIELD_NUM).find(|&i| {
                    let n = cname(&self.t.shields[i].name);
                    !n.is_empty() && name_equiv(&n, b.as_bytes())
                }) {
                    Some(src) => self.t.shields[id] = self.t.shields[src].clone(),
                    None => self.diag.error(folder, format!("shield '{name}': base '{b}' not found")),
                }
            }
            if let Some(s) = get_str(item, "name") {
                set_name(&mut self.t.shields[id].name, s, 29);
            }
            let (tp, mp) = (self.t.shields[id].tpwr as i32, self.t.shields[id].mpwr as i32);
            let tpwr =
                self.clamp_shield(item, "shield_power", if base.is_some() { tp } else { 9 }, SHIELD_TPWR_MAX, folder);
            let mpwr =
                self.clamp_shield(item, "max_power", if base.is_some() { mp } else { 14 }, SHIELD_MPWR_MAX, folder);
            let icon = pending(item, "icon_sprite");
            let s = &mut self.t.shields[id];
            s.tpwr = tpwr;
            s.mpwr = mpwr;
            if base.is_none() && icon.is_empty() {
                s.itemgraphic = 13;
            }
            s.cost = int_or(item, "cost", if base.is_some() { s.cost as i32 } else { 10000 }) as u16;
            self.pending_shield_icon.insert(id, icon);
            if get_bool(item, "always_available", false) {
                self.avail("shields", id, folder, name);
            }
            count += 1;
        }
        count
    }

    /// `mod_manager.c:load_mod_specials_auto`.
    pub(super) fn load_specials(&mut self, root: &Value, folder: &str) -> usize {
        let mut count = 0;
        for item in self.items(root, "specials", folder) {
            let name = get_str(item, "name").unwrap_or("");
            if !self.check_keys(item, SPECIAL_KEYS, folder, &format!("special weapon '{name}'")) {
                continue;
            }
            let Some(id) = (SPECIAL_RANGE.0..=SPECIAL_RANGE.1).find(|&i| !self.special_used[i]) else {
                self.diag.error(
                    folder,
                    format!(
                        "special weapon '{name}': no free slot left ({} at most)",
                        SPECIAL_RANGE.1 - SPECIAL_RANGE.0 + 1
                    ),
                );
                break;
            };
            if !self.register_identity(ItemKind::Special, id, folder, name) {
                self.diag.error(folder, format!("special weapon '{name}': identity missing, duplicated or too long"));
                continue;
            }
            self.special_used[id] = true;
            let base = get_str(item, "base");
            if let Some(b) = base {
                match (0..=SPECIAL_NUM).find(|&i| {
                    let n = cname(&self.t.specials[i].name);
                    !n.is_empty() && name_equiv(&n, b.as_bytes())
                }) {
                    Some(src) => self.t.specials[id] = self.t.specials[src].clone(),
                    None => self.diag.error(folder, format!("special weapon '{name}': base '{b}' not found")),
                }
            }
            if let Some(s) = get_str(item, "name") {
                set_name(&mut self.t.specials[id].name, s, 29);
            }
            let icon = pending(item, "icon_sprite");
            let s = &mut self.t.specials[id];
            if base.is_none() && icon.is_empty() {
                s.itemgraphic = 26;
            }
            s.pwr = int_or(item, "power", if base.is_some() { s.pwr as i32 } else { 100 }) as u8;
            s.stype = int_or(item, "subtype", if base.is_some() { s.stype as i32 } else { 1 }) as u8;
            if base.is_none() {
                s.wpn = 30;
            }
            self.pending_special_icon.insert(id, icon);
            if get_bool(item, "always_available", false) {
                self.avail("specials", id, folder, name);
            }
            count += 1;
        }
        count
    }

    // ---------------------------------------------------------------- levels

    pub(super) fn load_levels(&mut self, m: &ModFolder) -> usize {
        let mut n = 0;
        // All episodes are kept: `apply_to` chooses according to the level started.
        let mut diag = std::mem::take(&mut self.diag);
        let read = levels::read_mod_levels(&m.path, &m.folder, self, &mut diag);
        self.diag = diag;
        for l in read {
            if let Some(prev) = self.levels.get(&(l.episode, l.lvl_file_num)) {
                self.diag.warn(
                    &m.folder,
                    format!(
                        "level ep{}_{:02} already replaced by '{}': the mod loaded last wins",
                        l.episode, l.lvl_file_num, prev.folder
                    ),
                );
            }
            self.levels.insert((l.episode, l.lvl_file_num), l);
            n += 1;
        }
        n
    }

    // ---------------------------------------------------------------- start

    pub(super) fn stash_starter(&mut self, st: &Value, folder: &str) {
        if !self.check_keys(st, STARTER_KEYS, folder, "starter") {
            return;
        }
        for key in STARTER_KEYS {
            match st.get(key) {
                None => {}
                Some(Value::String(s)) if !s.is_empty() && s.len() <= PENDING_NAME_MAX => {
                    self.pending_starter.insert(key, (folder.to_string(), s.clone()));
                }
                Some(v) => self.diag.error(folder, format!("starter '{key}': item name expected, found {v:?}")),
            }
        }
    }

    pub(super) fn starter_field(&mut self, key: &str) -> &mut i32 {
        let s = &mut self.starter;
        match key {
            "ship" => &mut s.ship,
            "front_weapon" => &mut s.front_weapon,
            "rear_weapon" => &mut s.rear_weapon,
            "shield" => &mut s.shield,
            "generator" => &mut s.generator,
            "sidekick_left" => &mut s.sidekick_left,
            _ => &mut s.sidekick_right,
        }
    }
}
