//! Mod levels: `levels/epN_NN.json` in the mod folder modifies the
//! events (and optionally the random enemies) of the original level N_NN.
//! The map (tiles) stays that of the game.
//!
//! Two modes: `"events"` REPLACES the original list; `"add_events"`
//! inserts events into the original list, each after all the
//! original events of the same `time` (stable, deterministic insertion).
//!
//! Named events (ADR 0128): `{ "time": 300, "spawn": "my_drone", "x": 130 }`.
//! The type key (`spawn`, `stop_map`, `jump`…) carries the main parameter
//! (or `true` if there is none), the other parameters are named.
//! `EVENT_KINDS` is the only description of the types: the reader and
//! the level editor exporter (`export_level`) both use it. An enemy, a sprite bank or a special
//! weapon is referred to by
//! name; the special values of the fields have a name (`"random"`,
//! `"keep"`, `{ "picked": n }`…), never their code. Any unknown key is
//! an error.

use crate::json::{self, Value};
use crate::Diagnostics;
use std::path::Path;
use tyrian_formats::lvl::{Event, LevelEvents};

/// `lvlmast.h:EVENT_MAXIMUM` (the C code keeps one slot for the sentinel).
pub const EVENT_MAXIMUM: usize = 2500;
/// `level_load.c:JE_loadMap` reads at most 40 random enemies (`levelEnemy[40]`).
pub const MAX_RANDOM_ENEMIES: usize = 40;
/// Level files read at most per mod.
pub const MAX_LEVEL_FILES: usize = 64;
const LEVEL_KEYS: &[&str] = &["episode", "lvl_file_num", "events", "add_events", "random_enemies"];

/// Kind of an event parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    /// Number; `codes`: special values, written by their name only.
    Int(&'static [(i32, &'static str)]),
    /// Scroll position 0..65535 (16-bit field of the C code); `codes` as for `Int`.
    Time(&'static [(i32, &'static str)]),
    /// Enemy (new in the mod or original), by its name.
    Enemy,
    /// Enemy sprite bank, by its theme (`originals::BANK_THEMES`); null = none.
    Bank,
    /// Special weapon, by its item name.
    Special,
    /// Number, or `{ "picked": n }` = link drawn by `pick_random_link` (code 80 + n).
    IntOrPicked(&'static [(i32, &'static str)]),
    /// Drawn link slot 0..9 (code 80 + n).
    PickSlot,
}

/// Named parameter and `eventdat` field (1..6) that carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub slot: u8,
    pub param: Param,
}

/// Named event type (`level_events.c:JE_eventSystem`, docs/events-spec.md).
#[derive(Debug, Clone, Copy)]
pub struct EventKind {
    pub name: &'static str,
    pub eventtype: u8,
    /// The first field is the main parameter (value of the type key);
    /// without a field, the type key is `true`.
    pub fields: &'static [Field],
}

const fn f(name: &'static str, slot: u8, param: Param) -> Field {
    Field { name, slot, param }
}
const INT: Param = Param::Int(&[]);
const TIME: Param = Param::Time(&[]);
const KEEP: Param = Param::Int(&[(-99, "keep")]);
const X: Param = Param::Int(&[(-200, "random"), (-99, "none")]);
const LINK_ALL: Param = Param::Int(&[(99, "all")]);
/// Enemy created by the script (`level_helpers.c:JE_createNewEventEnemy`).
const SPAWN: &[Field] = &[
    f("enemy", 1, Param::Enemy),
    f("x", 2, X),
    f("y_speed", 3, INT),
    f("link", 4, INT),
    f("y", 5, INT),
    f("fixed_move_y", 6, INT),
];
/// "Custom" enemy (49..52): image and bank given, armor.
const CUSTOM: &[Field] = &[
    f("image", 1, INT),
    f("x", 2, X),
    f("bank", 3, Param::Bank),
    f("link", 4, INT),
    f("y", 5, INT),
    f("armor", 6, INT),
];
const SPEEDS: &[Field] = &[f("back", 1, INT), f("back2", 2, INT), f("back3", 3, INT)];
const TIMER: &[Field] = &[f("on", 1, INT), f("target", 2, TIME), f("seconds", 3, INT)];
const DROP: &[Field] = &[f("enemy", 1, Param::Enemy), f("link", 4, INT)];

/// All the engine's event types. 4/83 and 59/68 share their code:
/// only 4 and 59 are named (the exporter writes 83 and 68 under these names).
pub const EVENT_KINDS: &[EventKind] = &[
    EventKind { name: "nothing", eventtype: 0, fields: &[] },
    EventKind { name: "starfield_speed", eventtype: 1, fields: &[f("speed", 1, INT)] },
    EventKind { name: "scroll_speed", eventtype: 2, fields: SPEEDS },
    EventKind { name: "scroll_slow", eventtype: 3, fields: &[] },
    EventKind { name: "stop_map", eventtype: 4, fields: &[f("layers", 1, INT)] },
    EventKind {
        name: "load_banks",
        eventtype: 5,
        fields: &[
            f("bank1", 1, Param::Bank),
            f("bank2", 2, Param::Bank),
            f("bank3", 3, Param::Bank),
            f("bank4", 4, Param::Bank),
        ],
    },
    EventKind { name: "spawn_ground", eventtype: 6, fields: SPAWN },
    EventKind { name: "spawn_top", eventtype: 7, fields: SPAWN },
    EventKind { name: "stars_off", eventtype: 8, fields: &[] },
    EventKind { name: "stars_on", eventtype: 9, fields: &[] },
    EventKind { name: "spawn_ground2", eventtype: 10, fields: SPAWN },
    EventKind { name: "end_level", eventtype: 11, fields: &[f("immediate", 1, INT)] },
    EventKind {
        name: "spawn_2x2",
        eventtype: 12,
        fields: &[
            f("enemy", 1, Param::Enemy),
            f("x", 2, X),
            f("y_speed", 3, INT),
            f("link", 4, INT),
            f("y", 5, INT),
            f("layer", 6, Param::Int(&[(1, "ground"), (2, "sky"), (3, "top"), (4, "ground2")])),
        ],
    },
    EventKind { name: "random_enemies_off", eventtype: 13, fields: &[] },
    EventKind { name: "random_enemies_on", eventtype: 14, fields: &[] },
    EventKind { name: "spawn", eventtype: 15, fields: SPAWN },
    EventKind { name: "text", eventtype: 16, fields: &[f("message", 1, INT)] },
    EventKind { name: "spawn_ground_bottom", eventtype: 17, fields: SPAWN },
    EventKind { name: "spawn_bottom", eventtype: 18, fields: SPAWN },
    EventKind {
        name: "move_enemies",
        eventtype: 19,
        fields: &[
            f("x_speed", 1, KEEP),
            f("y_speed", 2, KEEP),
            f("targets", 3, Param::IntOrPicked(&[(1, "ground"), (2, "sky"), (3, "top"), (99, "all")])),
            f("link", 4, INT),
            f("cycle", 5, INT),
            f("fixed_move_y", 6, Param::Int(&[(-99, "zero")])),
        ],
    },
    EventKind {
        name: "accelerate_enemies",
        eventtype: 20,
        fields: &[
            f("x", 1, KEEP),
            f("y", 2, KEEP),
            f("targets", 3, Param::IntOrPicked(&[])),
            f("link", 4, INT),
            f("cycle", 5, INT),
            f("animation", 6, INT),
        ],
    },
    EventKind { name: "background3_over", eventtype: 21, fields: &[] },
    EventKind { name: "background3_under", eventtype: 22, fields: &[] },
    EventKind { name: "spawn_top_bottom", eventtype: 23, fields: SPAWN },
    EventKind {
        name: "animate_enemies",
        eventtype: 24,
        fields: &[f("animation", 1, INT), f("cycle", 2, INT), f("mode", 3, INT), f("link", 4, INT)],
    },
    EventKind { name: "set_armor", eventtype: 25, fields: &[f("armor", 1, INT), f("link", 4, INT)] },
    EventKind { name: "small_enemy_adjust", eventtype: 26, fields: &[f("on", 1, INT)] },
    EventKind {
        name: "reverse_enemies",
        eventtype: 27,
        fields: &[f("x", 1, KEEP), f("y", 2, KEEP), f("color", 3, Param::IntOrPicked(&[])), f("link", 4, INT)],
    },
    EventKind { name: "top_enemies_under", eventtype: 28, fields: &[] },
    EventKind { name: "top_enemies_over", eventtype: 29, fields: &[] },
    EventKind { name: "scroll_speed_keep_stop", eventtype: 30, fields: SPEEDS },
    EventKind {
        name: "fire_rate",
        eventtype: 31,
        fields: &[
            f("rate1", 1, INT),
            f("rate2", 2, INT),
            f("rate3", 3, INT),
            f("link", 4, LINK_ALL),
            f("launch_rate", 5, INT),
        ],
    },
    EventKind { name: "spawn_top_low", eventtype: 32, fields: SPAWN },
    EventKind { name: "drop_on_death", eventtype: 33, fields: DROP },
    EventKind { name: "music_fade", eventtype: 34, fields: &[] },
    EventKind { name: "music", eventtype: 35, fields: &[f("song", 1, INT)] },
    EventKind { name: "end_level_when_clear", eventtype: 36, fields: &[] },
    EventKind { name: "random_enemy_rate", eventtype: 37, fields: &[f("rate", 1, INT)] },
    EventKind { name: "jump_to", eventtype: 38, fields: &[f("time", 1, TIME)] },
    EventKind { name: "relink", eventtype: 39, fields: &[f("from", 1, INT), f("to", 2, INT)] },
    EventKind { name: "continual_damage", eventtype: 40, fields: &[] },
    EventKind { name: "clear_enemies", eventtype: 41, fields: &[f("sky_only", 1, INT)] },
    EventKind { name: "background3_mode2", eventtype: 42, fields: &[] },
    EventKind { name: "background2_over", eventtype: 43, fields: &[f("mode", 1, INT)] },
    EventKind {
        name: "color_filter",
        eventtype: 44,
        fields: &[
            f("mode", 1, INT),
            f("color", 2, INT),
            f("brightness", 3, INT),
            f("new_color", 4, INT),
            f("brightness_change", 5, INT),
            f("fade_start", 6, INT),
        ],
    },
    EventKind { name: "drop_on_death_multiplayer", eventtype: 45, fields: DROP },
    EventKind {
        name: "difficulty",
        eventtype: 46,
        fields: &[f("change", 1, INT), f("condition", 2, INT), f("damage_rate", 3, INT)],
    },
    EventKind { name: "set_armor_plain", eventtype: 47, fields: &[f("armor", 1, INT), f("link", 4, INT)] },
    EventKind { name: "background2_opaque", eventtype: 48, fields: &[] },
    EventKind { name: "custom_enemy_ground", eventtype: 49, fields: CUSTOM },
    EventKind { name: "custom_enemy", eventtype: 50, fields: CUSTOM },
    EventKind { name: "custom_enemy_top", eventtype: 51, fields: CUSTOM },
    EventKind { name: "custom_enemy_ground2", eventtype: 52, fields: CUSTOM },
    EventKind { name: "force_events", eventtype: 53, fields: &[f("mode", 1, Param::Int(&[(99, "off")]))] },
    EventKind { name: "jump", eventtype: 54, fields: &[f("time", 1, Param::Time(&[(65535, "return")]))] },
    EventKind {
        name: "set_acceleration",
        eventtype: 55,
        fields: &[f("x", 1, KEEP), f("y", 2, KEEP), f("targets", 3, Param::IntOrPicked(&[])), f("link", 4, INT)],
    },
    EventKind { name: "spawn_ground2_low", eventtype: 56, fields: SPAWN },
    EventKind { name: "jump_on_boss_death", eventtype: 57, fields: &[f("time", 1, TIME)] },
    EventKind { name: "set_launch", eventtype: 58, fields: &[f("enemy", 1, Param::Enemy), f("link", 4, LINK_ALL)] },
    EventKind { name: "replace_enemies", eventtype: 59, fields: DROP },
    EventKind {
        name: "special_enemy",
        eventtype: 60,
        fields: &[f("flag", 1, INT), f("set", 2, INT), f("link", 4, INT)],
    },
    EventKind {
        name: "skip_if_flag",
        eventtype: 61,
        fields: &[f("flag", 1, INT), f("value", 2, INT), f("skip", 3, INT)],
    },
    EventKind { name: "sound", eventtype: 62, fields: &[f("sound", 1, INT)] },
    EventKind { name: "skip_if_one_player", eventtype: 63, fields: &[f("skip", 1, INT)] },
    EventKind { name: "smoothie", eventtype: 64, fields: &[f("effect", 1, INT), f("on", 2, INT), f("data", 3, INT)] },
    EventKind { name: "background3_1x1", eventtype: 65, fields: &[f("mode", 1, INT)] },
    EventKind { name: "skip_by_difficulty", eventtype: 66, fields: &[f("difficulty", 1, INT), f("skip", 2, INT)] },
    EventKind { name: "level_timer", eventtype: 67, fields: TIMER },
    EventKind { name: "invulnerable", eventtype: 69, fields: &[f("ticks", 1, INT)] },
    EventKind {
        name: "jump_if_cleared",
        eventtype: 70,
        fields: &[f("time", 1, TIME), f("link1", 2, INT), f("link2", 3, INT), f("link3", 4, INT)],
    },
    EventKind { name: "jump_if_map_before", eventtype: 71, fields: &[f("time", 1, TIME), f("position", 2, INT)] },
    EventKind { name: "background3_variant_b", eventtype: 72, fields: &[f("on", 1, INT)] },
    EventKind { name: "sky_enemies_over_all", eventtype: 73, fields: &[f("on", 1, INT)] },
    EventKind {
        name: "bounce",
        eventtype: 74,
        fields: &[
            f("x_max", 1, KEEP),
            f("y_max", 2, KEEP),
            f("link", 4, INT),
            f("x_min", 5, KEEP),
            f("y_min", 6, KEEP),
        ],
    },
    EventKind {
        name: "pick_random_link",
        eventtype: 75,
        fields: &[f("from", 1, INT), f("to", 2, INT), f("slot", 3, Param::PickSlot), f("skip", 4, INT)],
    },
    EventKind { name: "return_when_clear", eventtype: 76, fields: &[] },
    EventKind { name: "set_map_position", eventtype: 77, fields: &[f("y", 1, INT), f("y2", 2, INT)] },
    EventKind { name: "galaga_fire_rate", eventtype: 78, fields: &[] },
    EventKind { name: "boss_bar", eventtype: 79, fields: &[f("link", 1, INT), f("link2", 2, INT)] },
    EventKind { name: "skip_if_two_players", eventtype: 80, fields: &[f("skip", 1, INT)] },
    EventKind { name: "wrap_background2", eventtype: 81, fields: &[f("from", 1, INT), f("to", 2, INT)] },
    EventKind { name: "give_special", eventtype: 82, fields: &[f("special", 1, Param::Special)] },
    EventKind { name: "timed_battle_timer", eventtype: 84, fields: TIMER },
    EventKind { name: "timed_battle_drop", eventtype: 85, fields: DROP },
    EventKind { name: "random_explosions", eventtype: 99, fields: &[f("on", 1, INT)] },
];

/// Bounds of an `eventdat` field (types of `JE_EventRecType`).
fn slot_range(slot: u8) -> (i32, i32) {
    match slot {
        1 | 2 => (-32768, 32767),
        4 => (0, 255),
        _ => (-128, 127),
    }
}

fn get_slot(e: &Event, slot: u8) -> i32 {
    match slot {
        1 => e.eventdat as i32,
        2 => e.eventdat2 as i32,
        3 => e.eventdat3 as i32,
        4 => e.eventdat4 as i32,
        5 => e.eventdat5 as i32,
        _ => e.eventdat6 as i32,
    }
}

fn set_slot(e: &mut Event, slot: u8, v: i32) {
    match slot {
        1 => e.eventdat = v as i16,
        2 => e.eventdat2 = v as i16,
        3 => e.eventdat3 = v as i8,
        4 => e.eventdat4 = v as u8,
        5 => e.eventdat5 = v as i8,
        _ => e.eventdat6 = v as i8,
    }
}

/// Names that the reader resolves (supplied by the mod loader).
pub trait LevelNames {
    /// New mod enemy or original enemy → `enemyDat` identifier.
    fn enemy(&mut self, name: &str) -> Result<u16, String>;
    /// Special weapon (item) → identifier.
    fn special(&mut self, name: &str) -> Result<u8, String>;
}

fn bank_by_name(name: &str) -> Option<i32> {
    crate::originals::BANK_THEMES.iter().position(|&t| t == name).map(|i| i as i32 + 1)
}

fn quoted(codes: &[(i32, &str)]) -> String {
    codes.iter().map(|(_, n)| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
}

/// Number (or named code) within `lo..=hi`; a number equal to a code is rejected.
fn read_int(v: &Value, codes: &[(i32, &str)], lo: i32, hi: i32) -> Result<i32, String> {
    match v {
        Value::String(s) => codes.iter().find(|(_, n)| n == s).map(|(c, _)| *c).ok_or_else(|| {
            if codes.is_empty() {
                format!("'{s}': number expected")
            } else {
                format!("'{s}': number or {} expected", quoted(codes))
            }
        }),
        Value::Number(_) => {
            let n = v.as_c_int().unwrap_or(0);
            if let Some((_, name)) = codes.iter().find(|(c, _)| *c == n) {
                return Err(format!("{n}: write \"{name}\""));
            }
            if n < lo || n > hi {
                return Err(format!("{n} outside {lo}..{hi}"));
            }
            Ok(n)
        }
        other => Err(format!("{}: number expected", other.type_name())),
    }
}

fn read_param(v: &Value, field: &Field, names: &mut dyn LevelNames) -> Result<i32, String> {
    let (lo, hi) = slot_range(field.slot);
    let r = match field.param {
        Param::Int(codes) => read_int(v, codes, lo, hi),
        Param::Time(codes) => read_int(v, codes, 0, 65535).map(|t| t as u16 as i16 as i32),
        Param::IntOrPicked(codes) => match v.get("picked") {
            Some(p) if v.as_object().is_some_and(|o| o.len() == 1) => match p.as_c_int() {
                Some(n) if matches!(p, Value::Number(_)) && (0..=9).contains(&n) => Ok(80 + n),
                _ => Err("picked: 0..9 expected".to_string()),
            },
            _ => match read_int(v, codes, lo, hi) {
                Ok(n) if (80..=89).contains(&n) => Err(format!("{n}: write {{ \"picked\": {} }}", n - 80)),
                r => r,
            },
        },
        Param::PickSlot => match v {
            Value::Number(_) => match v.as_c_int() {
                Some(n) if (0..=9).contains(&n) => Ok(80 + n),
                _ => Err("0..9 expected".to_string()),
            },
            other => Err(format!("{}: 0..9 expected", other.type_name())),
        },
        Param::Enemy => match v {
            Value::String(s) => names.enemy(s).map(i32::from),
            other => Err(format!("{other:?} — an enemy is referenced by name (rewrite/mods/original-names.json)")),
        },
        Param::Bank => match v {
            Value::Null => Ok(0),
            Value::String(s) => {
                bank_by_name(s).ok_or_else(|| format!("unknown bank '{s}' (rewrite/mods/original-names.json)"))
            }
            other => Err(format!("{other:?} — an image bank is referenced by name")),
        },
        Param::Special => match v {
            Value::String(s) => names.special(s).map(i32::from),
            other => Err(format!("{other:?} — a special weapon is referenced by name")),
        },
    };
    r.map_err(|e| format!("{}: {e}", field.name))
}

/// Reads a named event.
fn read_event(e: &Value, names: &mut dyn LevelNames) -> Result<Event, String> {
    let Some(obj) = e.as_object() else {
        return Err("object expected".into());
    };
    let kinds: Vec<&EventKind> = obj.iter().filter_map(|(k, _)| EVENT_KINDS.iter().find(|t| t.name == k)).collect();
    let kind = match kinds.as_slice() {
        [k] => *k,
        [] => {
            let keys: Vec<&str> = obj.iter().map(|(k, _)| k.as_str()).filter(|k| *k != "time").collect();
            return Err(format!("missing or unknown event type ({}); types: MODDING.md §8", keys.join(", ")));
        }
        _ => {
            return Err(format!(
                "several event types ({})",
                kinds.iter().map(|k| k.name).collect::<Vec<_>>().join(", ")
            ))
        }
    };
    let mut ev = Event {
        eventtime: 0,
        eventtype: kind.eventtype,
        eventdat: 0,
        eventdat2: 0,
        eventdat3: 0,
        eventdat4: 0,
        eventdat5: 0,
        eventdat6: 0,
    };
    match e.get("time") {
        Some(v @ Value::Number(_)) => match v.as_c_int() {
            Some(t) if (0..=65535).contains(&t) => ev.eventtime = t as u16,
            _ => return Err("time: 0..65535 expected".into()),
        },
        _ => return Err(format!("{}: 'time' (number) required", kind.name)),
    }
    for (key, v) in obj {
        if key == "time" {
            continue;
        }
        let field = if key == kind.name {
            match kind.fields.first() {
                Some(f) => f,
                None if matches!(v, Value::Bool(true)) => continue,
                None => return Err(format!("{}: true expected (no parameter)", kind.name)),
            }
        } else {
            match kind.fields.iter().skip(1).find(|f| f.name == key) {
                Some(f) => f,
                None => return Err(format!("{}: unknown parameter '{key}'", kind.name)),
            }
        };
        let n = read_param(v, field, names).map_err(|m| format!("{}: {m}", kind.name))?;
        set_slot(&mut ev, field.slot, n);
    }
    Ok(ev)
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelOverride {
    pub folder: String,
    pub episode: u8,
    pub lvl_file_num: u16,
    pub events: Vec<Event>,
    /// true: `events` is inserted into the original list instead of replacing it.
    pub add: bool,
    pub random_enemies: Option<Vec<u16>>,
}

impl LevelOverride {
    pub fn apply(&self, lvl: &mut LevelEvents) {
        if self.add {
            for e in &self.events {
                let at = lvl.events.iter().position(|o| o.eventtime > e.eventtime).unwrap_or(lvl.events.len());
                lvl.events.insert(at, *e);
            }
        } else {
            lvl.events = self.events.clone();
        }
        if let Some(r) = &self.random_enemies {
            lvl.enemies = r.clone();
        }
    }
}

/// Keys outside `allowed`: the first one found.
fn unknown_key<'a>(obj: &'a Value, allowed: &[&str]) -> Option<&'a str> {
    obj.as_object()?.iter().map(|(k, _)| k.as_str()).find(|k| !allowed.contains(k))
}

/// Reads a mod level file; None (with an error) if it is invalid.
pub fn parse_level(
    text: &str,
    folder: &str,
    file: &str,
    names: &mut dyn LevelNames,
    diag: &mut Diagnostics,
) -> Option<LevelOverride> {
    let root = match json::parse(text) {
        Ok(v) => v,
        Err(e) => {
            diag.error(folder, format!("levels/{file}: {e}"));
            return None;
        }
    };
    if root.as_object().is_none() {
        diag.error(folder, format!("levels/{file}: object expected"));
        return None;
    }
    if let Some(k) = unknown_key(&root, LEVEL_KEYS) {
        diag.error(folder, format!("levels/{file}: unknown key '{k}'"));
        return None;
    }
    let episode = root.get("episode").and_then(Value::as_c_int).unwrap_or(0);
    let num = root.get("lvl_file_num").and_then(Value::as_c_int).unwrap_or(0);
    if !(1..=5).contains(&episode) || !(1..=99).contains(&num) {
        diag.error(folder, format!("levels/{file}: episode (1..5) and lvl_file_num (1..99) required"));
        return None;
    }
    let (list, add) = match (root.get("events"), root.get("add_events")) {
        (Some(Value::Array(a)), None) => (a, false),
        (None, Some(Value::Array(a))) => (a, true),
        (Some(_), Some(_)) => {
            diag.error(folder, format!("levels/{file}: 'events' and 'add_events' are mutually exclusive"));
            return None;
        }
        _ => {
            diag.error(folder, format!("levels/{file}: missing 'events' or 'add_events' array"));
            return None;
        }
    };
    if list.len() > EVENT_MAXIMUM - 1 {
        diag.error(folder, format!("levels/{file}: {} events, maximum {}", list.len(), EVENT_MAXIMUM - 1));
        return None;
    }
    let mut events = Vec::with_capacity(list.len());
    let mut last_time = 0;
    for (k, e) in list.iter().enumerate() {
        match read_event(e, names) {
            Ok(ev) => {
                if !add && (ev.eventtime as i32) < last_time {
                    // level_events.c does not sort: a late event goes out on the next frame.
                    diag.warn(folder, format!("levels/{file}: event {k} (time {}) before the previous one ({last_time}) — executed late, as in the original game", ev.eventtime));
                }
                last_time = ev.eventtime as i32;
                events.push(ev);
            }
            Err(msg) => {
                diag.error(folder, format!("levels/{file}: event {k}: {msg}"));
                return None;
            }
        }
    }
    let random_enemies = match root.get("random_enemies") {
        None => None,
        Some(v) => {
            let Some(a) = v.as_array() else {
                diag.error(folder, format!("levels/{file}: 'random_enemies' must be an array"));
                return None;
            };
            let mut out = Vec::new();
            for x in a.iter().take(MAX_RANDOM_ENEMIES) {
                let id = match x {
                    Value::String(s) => names.enemy(s),
                    other => Err(format!("{other:?} — an enemy is referenced by name")),
                };
                match id {
                    Ok(n) => out.push(n),
                    Err(e) => {
                        diag.error(folder, format!("levels/{file}: random_enemies: {e}"));
                        return None;
                    }
                }
            }
            Some(out)
        }
    };
    Some(LevelOverride {
        folder: folder.to_string(),
        episode: episode as u8,
        lvl_file_num: num as u16,
        events,
        add,
        random_enemies,
    })
}

/// Reads `levels/*.json` of a mod folder, sorted by name.
pub fn read_mod_levels(
    mod_path: &Path,
    folder: &str,
    names: &mut dyn LevelNames,
    diag: &mut Diagnostics,
) -> Vec<LevelOverride> {
    let dir = mod_path.join("levels");
    if !dir.exists() {
        return Vec::new();
    }
    let dir = match crate::folder::contained_path(mod_path, &dir) {
        Ok(p) => p,
        Err(e) => {
            diag.error(folder, e);
            return Vec::new();
        }
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    if files.len() > MAX_LEVEL_FILES {
        diag.warn(folder, format!("levels/: {} files, only the first {MAX_LEVEL_FILES} are read", files.len()));
        files.truncate(MAX_LEVEL_FILES);
    }
    let mut out = Vec::new();
    for p in files {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
        match crate::folder::contained_path(mod_path, &p)
            .and_then(|p| crate::read_text_bounded(&p, json::MAX_TEXT_BYTES))
        {
            Ok(t) => {
                if let Some(l) = parse_level(&t, folder, &name, names, diag) {
                    out.push(l);
                }
            }
            Err(e) => diag.error(folder, format!("levels/{name}: {e}")),
        }
    }
    out
}

// ------------------------------------------------------------------ export

/// Enemy of an event to export: slot in the tables, or name already
/// written by the author in the editor (new enemy of their mod).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnemyCited {
    Slot(u16),
    Name(String),
}

/// Names the exporter needs (original content of the episode).
pub trait ExportNames {
    fn enemy_name(&self, slot: u16) -> Option<String>;
    fn special_name(&self, id: u8) -> Option<String>;
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_param(raw: i32, field: &Field, enemy: Option<&EnemyCited>, names: &dyn ExportNames) -> Result<String, String> {
    let int = |raw: i32, codes: &[(i32, &str)]| {
        codes.iter().find(|(c, _)| *c == raw).map_or(raw.to_string(), |(_, n)| json_string(n))
    };
    match field.param {
        Param::Int(codes) => Ok(int(raw, codes)),
        Param::Time(codes) => Ok(int(raw as u16 as i32, codes)),
        Param::IntOrPicked(codes) => {
            Ok(if (80..=89).contains(&raw) { format!("{{ \"picked\": {} }}", raw - 80) } else { int(raw, codes) })
        }
        Param::PickSlot => match raw {
            80..=89 => Ok((raw - 80).to_string()),
            _ => Err(format!("{}: value {raw} outside 80..89", field.name)),
        },
        Param::Enemy => match enemy {
            Some(EnemyCited::Name(n)) => Ok(json_string(n)),
            _ => names
                .enemy_name(raw as u16)
                .map(|n| json_string(&n))
                .ok_or_else(|| format!("enemy {raw} has no name in the episode tables")),
        },
        Param::Bank => match raw {
            0 => Ok("null".into()),
            b if (1..=crate::originals::BANK_THEMES.len() as i32).contains(&b) => {
                Ok(json_string(crate::originals::BANK_THEMES[b as usize - 1]))
            }
            b => Err(format!("bank {b} does not exist")),
        },
        Param::Special => names
            .special_name(raw as u8)
            .map(|n| json_string(&n))
            .ok_or_else(|| format!("special weapon {raw} has no name")),
    }
}

/// Named type of an engine record. Code duplicates (83, 68)
/// are written under the type they repeat; a text 16 whose number is
/// that of an enemy (corrupted record in the shipped levels, replayed
/// as event 10 by the engine) is written `spawn_ground2`; type 108,
/// unknown to the engine and ignored, becomes `nothing`.
pub fn named_type(ev: &Event) -> u8 {
    match ev.eventtype {
        83 => 4,
        68 => 59,
        16 if ev.eventdat > 9 && ev.eventdat as usize <= tyrian_formats::hdt::ENEMY_NUM => 10,
        108 => 0,
        t => t,
    }
}

/// Named event (one JSON line).
pub fn event_to_json(ev: &Event, enemy: Option<&EnemyCited>, names: &dyn ExportNames) -> Result<String, String> {
    let ty = named_type(ev);
    let kind = EVENT_KINDS
        .iter()
        .find(|k| k.eventtype == ty)
        .ok_or_else(|| format!("event type {} has no name", ev.eventtype))?;
    let mut s = format!("{{ \"time\": {}, ", ev.eventtime);
    if kind.fields.is_empty() {
        s += &format!("\"{}\": true", kind.name);
    }
    for (k, field) in kind.fields.iter().enumerate() {
        let raw = get_slot(ev, field.slot);
        if k > 0 && raw == 0 {
            continue;
        }
        let v =
            write_param(raw, field, enemy, names).map_err(|e| format!("{} (time {}): {e}", kind.name, ev.eventtime))?;
        if k == 0 {
            s += &format!("\"{}\": {v}", kind.name);
        } else {
            s += &format!(", \"{}\": {v}", field.name);
        }
    }
    s += " }";
    Ok(s)
}

/// Complete `levels/epN_NN.json` file (`events` key): format written by
/// the editor's "Export to mod" button of the level editor.
pub fn export_level(
    episode: u8,
    lvl_file_num: u16,
    events: &[(Event, Option<EnemyCited>)],
    random_enemies: &[EnemyCited],
    names: &dyn ExportNames,
) -> Result<String, String> {
    let mut s = format!("{{\n  \"episode\": {episode},\n  \"lvl_file_num\": {lvl_file_num},\n  \"events\": [\n");
    for (k, (ev, enemy)) in events.iter().enumerate() {
        s += "    ";
        s += &event_to_json(ev, enemy.as_ref(), names)?;
        s += if k + 1 < events.len() { ",\n" } else { "\n" };
    }
    s += "  ]";
    if !random_enemies.is_empty() {
        let list: Result<Vec<String>, String> = random_enemies
            .iter()
            .map(|r| match r {
                EnemyCited::Name(n) => Ok(json_string(n)),
                EnemyCited::Slot(slot) => names
                    .enemy_name(*slot)
                    .map(|n| json_string(&n))
                    .ok_or_else(|| format!("random_enemies: enemy {slot} has no name")),
            })
            .collect();
        s += &format!(",\n  \"random_enemies\": [{}]", list?.join(", "));
    }
    s += "\n}\n";
    Ok(s)
}
