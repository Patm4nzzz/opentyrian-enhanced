//! Reading of the mod's enemies (`enemies`), enemy weapons
//! (`enemy_weapons`) and behaviors (ADR 0081).

use super::*;

/// Fields of an enemy (`EnemyDat`) that a mod can set, plus `name`, `base`, `behavior`.
pub(super) const ENEMY_KEYS: &[&str] = &[
    "name",
    "base",
    "behavior",
    "ani",
    "xmove",
    "ymove",
    "xaccel",
    "yaccel",
    "xcaccel",
    "ycaccel",
    "startx",
    "starty",
    "startxc",
    "startyc",
    "armor",
    "esize",
    "explosiontype",
    "animate",
    "shapebank",
    "xrev",
    "yrev",
    "dgr",
    "dlevel",
    "dani",
    "elaunchfreq",
    "elaunchtype",
    "value",
    "eenemydie",
    "tur",
    "freq",
    "egraphic",
];
pub(super) const BEHAVIOR_KEYS: &[&str] = &["path", "loop", "track", "turrets"];
pub(super) const PATH_KEYS: &[&str] = &["ticks", "vx", "vy"];
pub(super) const TRACK_KEYS: &[&str] = &["axis", "max_speed", "every"];
pub(super) const TURRET_KEYS: &[&str] = &["weapon", "every", "start", "direction"];

impl Ctx<'_> {
    // ---------------------------------------------------------------- enemies

    /// `hdt_json.c:apply_enemy`: only the fields present are written.
    /// References are by name: `tur` cites original weapons,
    /// `elaunchtype` and `eenemydie` an already defined new enemy or an original enemy.
    pub(super) fn apply_enemy_fields(&mut self, e: &mut EnemyDat, obj: &Value, errors: &mut Vec<String>) {
        macro_rules! f {
            ($field:ident, $key:literal, $ty:ty) => {
                if let Some(v) = get_int(obj, $key) {
                    e.$field = v as $ty;
                }
            };
        }
        f!(ani, "ani", u8);
        f!(xmove, "xmove", i8);
        f!(ymove, "ymove", i8);
        f!(xaccel, "xaccel", i8);
        f!(yaccel, "yaccel", i8);
        f!(xcaccel, "xcaccel", i8);
        f!(ycaccel, "ycaccel", i8);
        f!(startx, "startx", i16);
        f!(starty, "starty", i16);
        f!(startxc, "startxc", i8);
        f!(startyc, "startyc", i8);
        f!(armor, "armor", u8);
        f!(esize, "esize", u8);
        f!(explosiontype, "explosiontype", u8);
        f!(animate, "animate", u8);
        f!(shapebank, "shapebank", u8);
        f!(xrev, "xrev", i8);
        f!(yrev, "yrev", i8);
        f!(dgr, "dgr", u16);
        f!(dlevel, "dlevel", i8);
        f!(dani, "dani", i8);
        f!(elaunchfreq, "elaunchfreq", u8);
        f!(value, "value", i16);
        for key in ["elaunchtype", "eenemydie"] {
            let r = match obj.get(key) {
                None => continue,
                Some(Value::String(n)) => self.enemy_ref(n),
                Some(v) => Err(format!("{v:?} — an enemy is referenced by name")),
            };
            match r {
                Ok(id) if key == "elaunchtype" => e.elaunchtype = id,
                Ok(id) => e.eenemydie = id,
                Err(m) => errors.push(format!("{key}: {m}")),
            }
        }
        if let Some(a) = obj.get("tur").and_then(Value::as_array) {
            for (i, v) in a.iter().take(3).enumerate() {
                // An enemy's turrets only read weapons with index < 256
                // (`EnemyDat.tur` on one byte): a mod enemy weapon goes through "behavior.turrets".
                let r = match v {
                    Value::Null => Ok(0),
                    Value::String(n) if self.enemy_weapons.contains_key(n) => Err(format!(
                        "enemy weapon '{n}' is fired with behavior.turrets (tur only addresses original weapons)"
                    )),
                    Value::String(n) => self.original_weapon(n),
                    other => Err(format!("{other:?} — an original weapon is referenced by name, null = none")),
                };
                match r {
                    Ok(w) => e.tur[i] = w as u8,
                    Err(m) => errors.push(format!("tur: {m}")),
                }
            }
        }
        if let Some(a) = obj.get("freq").and_then(Value::as_array) {
            for (i, v) in a.iter().take(3).enumerate() {
                if let Some(n) = v.as_c_int() {
                    e.freq[i] = n as u8;
                }
            }
        }
        if let Some(a) = obj.get("egraphic").and_then(Value::as_array) {
            for (i, v) in a.iter().take(20).enumerate() {
                if let Some(n) = v.as_c_int() {
                    e.egraphic[i] = n as u16;
                }
            }
        }
    }

    /// Mod enemies (`enemies`): an entry with `name` creates a new
    /// enemy (optional copy of an original enemy via `base`); an entry
    /// with `original` modifies an original enemy, cited by its name, at all its
    /// slots in the episode table (none if the episode table
    /// does not contain it). Only the fields present are written.
    /// Returns (modified original enemies, new enemies).
    pub(super) fn load_enemies(&mut self, root: &Value, folder: &str) -> (usize, usize) {
        let (mut patched, mut created) = (0, 0);
        for item in self.items(root, "enemies", folder) {
            let name = get_str(item, "name").filter(|s| !s.is_empty());
            let label = match (name, item.get("original")) {
                (Some(n), _) => format!("enemy '{n}'"),
                (None, Some(Value::String(o))) => format!("original enemy '{o}'"),
                (None, Some(o)) => format!("original enemy {o:?}"),
                _ => "enemy".to_string(),
            };
            let mut keys: Vec<&str> = ENEMY_KEYS.to_vec();
            keys.push("original");
            if !self.check_keys(item, &keys, folder, &label) {
                continue;
            }
            let id: usize = match (name, item.get("original")) {
                (Some(_), Some(_)) => {
                    self.diag.error(
                        folder,
                        format!("{label}: 'name' (new enemy) and 'original' (original enemy) are mutually exclusive"),
                    );
                    continue;
                }
                (None, None) => {
                    self.diag
                        .error(folder, "enemy without 'name' (new enemy) or 'original' (original enemy to modify)");
                    continue;
                }
                (None, Some(o)) => {
                    let Some(oname) = self.original_enemy_name(o, folder, &label) else {
                        continue;
                    };
                    if item.get("base").is_some() {
                        self.diag.error(folder, format!("{label}: 'base' only applies to a new enemy"));
                        continue;
                    }
                    let Some(s) = self.original_enemy_slot(oname) else {
                        // Absent from this episode's table (e.g. enemy of episodes 4-5): nothing to modify.
                        continue;
                    };
                    let mut rec = self.t.enemies[s].clone();
                    let mut errs = Vec::new();
                    self.apply_enemy_fields(&mut rec, item, &mut errs);
                    for e in errs {
                        self.diag.error(folder, format!("{label}: {e}"));
                    }
                    self.t.enemies[s] = rec;
                    patched += 1;
                    s
                }
                (Some(n), None) => {
                    if self.new_enemies.contains_key(n) {
                        self.diag.error(folder, format!("{label} defined twice"));
                        continue;
                    }
                    let Some(id) = self.free_new_enemy_slot() else {
                        self.diag.error(
                            folder,
                            format!(
                                "{label}: no free new-enemy slot left ({} at most)",
                                NEW_ENEMY_RANGE.1 - NEW_ENEMY_RANGE.0 + 1
                            ),
                        );
                        break;
                    };
                    let mut rec = self.t.enemies[0].clone();
                    match item.get("base") {
                        None => {
                            // Safe defaults: visible, destructible, stays on screen.
                            rec.armor = 10;
                            rec.explosiontype = 1;
                            rec.animate = 0;
                            rec.xrev = 0;
                            rec.yrev = 0;
                        }
                        Some(b) => match self.original_enemy_name(b, folder, &format!("{label}, base")) {
                            Some(src) => rec = self.original_enemy_record(src),
                            None => continue,
                        },
                    }
                    // Reserved before the references: `elaunchtype` may copy an original enemy.
                    self.new_enemies.insert(n.to_string(), id as u16);
                    let mut errs = Vec::new();
                    self.apply_enemy_fields(&mut rec, item, &mut errs);
                    for e in errs {
                        self.diag.error(folder, format!("{label}: {e}"));
                    }
                    if rec.armor == 0 {
                        self.diag.warn(
                            folder,
                            format!("{label}: armor 0 turns it into a pickup item (enemy_spawn.c:JE_makeEnemy)"),
                        );
                    }
                    self.t.enemies[id] = rec;
                    created += 1;
                    id
                }
            };
            self.touched_enemies.insert(id);
            if let Some(b) = item.get("behavior") {
                self.attach_behavior(id as u16, b, folder, &label);
            }
        }
        (patched, created)
    }

    /// Named enemy weapons (`enemy_weapons`): same fields as a weapon level.
    pub(super) fn load_enemy_weapons(&mut self, root: &Value, folder: &str) -> usize {
        let mut count = 0;
        for item in self.items(root, "enemy_weapons", folder) {
            let Some(name) = get_str(item, "name").filter(|s| !s.is_empty()) else {
                self.diag.error(folder, "enemy_weapons: entry without 'name'");
                continue;
            };
            if self.enemy_weapons.contains_key(name) {
                self.diag.error(folder, format!("enemy weapon '{name}' defined twice"));
                continue;
            }
            let Some(wid) = self.next_mod_weapon_id() else {
                self.diag.error(folder, format!("enemy weapon '{name}': no free projectile identifier left"));
                break;
            };
            self.mod_weapon_id_used[wid] = true;
            self.fill_weapon(item, wid, folder, &format!("enemy weapon '{name}'"));
            self.enemy_weapons.insert(name.to_string(), wid as u16);
            count += 1;
        }
        count
    }

    /// Validates and attaches a behavior (ADR 0081). Any error rejects the
    /// whole behavior (never a half-behavior).
    pub(super) fn attach_behavior(&mut self, enemy: u16, b: &Value, folder: &str, label: &str) {
        let ctx = format!("{label}, behavior");
        if !self.check_keys(b, BEHAVIOR_KEYS, folder, &ctx) {
            self.diag.error(folder, format!("{ctx}: rejected (see previous errors)"));
            return;
        }
        let mut beh = EnemyBehavior::default();
        let mut ok = true;
        let in_i8 = |v: i32| (-128..=127).contains(&v);
        if let Some(path) = b.get("path") {
            let Some(segs) = path.as_array() else {
                self.diag.error(folder, format!("{ctx}: 'path' must be an array"));
                return;
            };
            if segs.len() > MAX_PATH_SEGMENTS {
                self.diag.error(folder, format!("{ctx}: {} segments, maximum {MAX_PATH_SEGMENTS}", segs.len()));
                return;
            }
            for (k, s) in segs.iter().enumerate() {
                if !self.check_keys(s, PATH_KEYS, folder, &format!("{ctx}, segment {k}")) {
                    ok = false;
                    continue;
                }
                let ticks = int_or(s, "ticks", 0);
                let vx = get_int(s, "vx");
                let vy = get_int(s, "vy");
                if !(1..=10_000).contains(&ticks) || vx.is_some_and(|v| !in_i8(v)) || vy.is_some_and(|v| !in_i8(v)) {
                    self.diag.error(folder, format!("{ctx}: invalid segment {k} (ticks 1..10000, vx/vy -128..127)"));
                    ok = false;
                    continue;
                }
                beh.path.push(PathSegment { ticks: ticks as u16, vx: vx.map(|v| v as i8), vy: vy.map(|v| v as i8) });
            }
            beh.path_loop = get_bool(b, "loop", false);
        }
        if let Some(t) = b.get("track") {
            if !self.check_keys(t, TRACK_KEYS, folder, &format!("{ctx}, track")) {
                ok = false;
            }
            let axis = get_str(t, "axis").unwrap_or("x");
            let max_speed = int_or(t, "max_speed", 2);
            let every = int_or(t, "every", 4);
            if !matches!(axis, "x" | "y" | "xy") || !(0..=127).contains(&max_speed) || !(1..=255).contains(&every) {
                self.diag
                    .error(folder, format!("{ctx}: invalid 'track' (axis x|y|xy, max_speed 0..127, every 1..255)"));
                ok = false;
            } else {
                beh.track = Some(Track {
                    x: axis.contains('x'),
                    y: axis.contains('y'),
                    max_speed: max_speed as u8,
                    every: every as u8,
                });
            }
        }
        if let Some(ts) = b.get("turrets") {
            let Some(ts) = ts.as_array() else {
                self.diag.error(folder, format!("{ctx}: 'turrets' must be an array"));
                return;
            };
            if ts.len() > MAX_BEHAVIOR_TURRETS {
                self.diag.error(folder, format!("{ctx}: {} turrets, maximum {MAX_BEHAVIOR_TURRETS}", ts.len()));
                return;
            }
            for (k, t) in ts.iter().enumerate() {
                if !self.check_keys(t, TURRET_KEYS, folder, &format!("{ctx}, turret {k}")) {
                    ok = false;
                    continue;
                }
                let w = self.resolve_weapon_ref(t.get("weapon"), folder, &format!("{ctx}, turret {k}"));
                let every = int_or(t, "every", 0);
                let start = int_or(t, "start", every);
                let slot = match t.get("direction") {
                    None => 1,
                    Some(Value::String(s)) if s == "down" => 1,
                    Some(Value::String(s)) if s == "side" => 2,
                    Some(Value::String(s)) if s == "side_reverse" => 3,
                    Some(_) => 0,
                };
                if w.is_none() || !(1..=255).contains(&every) || !(1..=255).contains(&start) || !(1..=3).contains(&slot)
                {
                    if w.is_some() {
                        self.diag.error(
                            folder,
                            format!("{ctx}, turret {k}: every/start 1..255, direction down|side|side_reverse"),
                        );
                    }
                    ok = false;
                    continue;
                }
                beh.turrets.push(BehaviorTurret {
                    weapon: w.unwrap(),
                    every: every as u8,
                    start: start as u8,
                    slot: slot as u8,
                });
            }
        }
        if !ok {
            self.diag.error(folder, format!("{ctx}: rejected (see previous errors)"));
            return;
        }
        if beh.path.is_empty() && beh.track.is_none() && beh.turrets.is_empty() {
            self.diag.warn(folder, format!("{ctx}: empty, ignored"));
            return;
        }
        let idx = self.behaviors.len() as u16;
        self.behaviors.push(beh);
        self.enemy_behavior.retain(|(e, _)| *e != enemy);
        self.enemy_behavior.push((enemy, idx));
    }
}
