//! Sprite name manifest (`data/sprite_manifest.json`) and name table
//! enriched by mod PNGs, like `hdt_json.c:sprite_names`.

use crate::json::{self, Value};
use crate::Diagnostics;
use std::path::Path;

/// `hdt_json.c:MAX_SPRITE_NAMES`.
pub const MAX_SPRITE_NAMES: usize = 2048;
/// `hdt_json.c:load_sprite_manifest`: sections read, in this order.
pub const SECTIONS: [&str; 9] =
    ["shots", "ships", "sidekicks", "pickups", "coins", "explosions", "weapon_icons", "portraits", "planets"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteName {
    /// Cut at 39 bytes (`strncpy(..., 39)`).
    pub name: String,
    /// Empty for a name registered by a mod (`section[0] = '\0'`).
    pub section: String,
    pub index: i32,
}

#[derive(Debug, Clone, Default)]
pub struct SpriteNames {
    pub rows: Vec<SpriteName>,
}

fn clip(s: &str, max: usize) -> String {
    // strncpy cuts at a byte; we cut at the closest character boundary.
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl SpriteNames {
    /// Reads the manifest; absent or unreadable → empty table (like the C code) with a warning.
    pub fn load(data_dir: &Path, diag: &mut Diagnostics) -> Self {
        let path = data_dir.join("sprite_manifest.json");
        let text = match crate::read_text_bounded(&path, json::MAX_TEXT_BYTES) {
            Ok(t) => t,
            Err(e) => {
                diag.warn(
                    "",
                    format!(
                        "sprite_manifest.json unreadable ({e}): only numeric indexes and mod PNGs will be resolved"
                    ),
                );
                return Self::default();
            }
        };
        match json::parse(&text) {
            Ok(v) => Self::from_value(&v, diag),
            Err(e) => {
                diag.warn("", format!("sprite_manifest.json: {e}"));
                Self::default()
            }
        }
    }

    pub fn from_value(root: &Value, diag: &mut Diagnostics) -> Self {
        let mut t = Self::default();
        for sec in SECTIONS {
            let Some(entries) = root.get(sec).and_then(Value::as_object) else {
                continue;
            };
            for (name, entry) in entries {
                if t.rows.len() >= MAX_SPRITE_NAMES {
                    break;
                }
                let index = match entry {
                    Value::Number(_) => entry.as_c_int().unwrap_or(0),
                    Value::Object(_) => entry.get("index").and_then(Value::as_c_int).unwrap_or(0),
                    _ => continue,
                };
                // Some sections are shared: keep their largest table;
                // the destination table is checked again at resolution.
                let capacity = crate::folder::SPRITE_DIRS
                    .iter()
                    .filter(|(_, t)| t.section() == sec)
                    .map(|(_, t)| t.capacity())
                    .max()
                    .unwrap_or(0);
                if index < 0 || index >= capacity {
                    diag.warn(
                        "",
                        format!("sprite_manifest: {sec}/{name}, index {index} outside 0..{capacity}, ignored"),
                    );
                    continue;
                }
                let name = clip(name, 39);
                if let Some(d) = t.rows.iter().find(|r| r.name == name && r.section == sec) {
                    if d.index != index {
                        diag.warn(
                            "",
                            format!(
                                "sprite_manifest: duplicate name '{name}' in '{sec}', index {} kept, {index} ignored",
                                d.index
                            ),
                        );
                    }
                    continue;
                }
                t.rows.push(SpriteName { name, section: sec.to_string(), index });
            }
        }
        t
    }

    /// `hdt_json.c:find_sprite_name_row`: exact section first, otherwise
    /// first match across all sections.
    fn row(&self, name: &str, section: Option<&str>) -> Option<&SpriteName> {
        let mut fallback = None;
        for r in &self.rows {
            if r.name != name {
                continue;
            }
            if let Some(s) = section {
                if !s.is_empty() && r.section == s {
                    return Some(r);
                }
            }
            if fallback.is_none() {
                fallback = Some(r);
            }
        }
        fallback
    }

    pub fn lookup_section(&self, name: &str, section: &str) -> Option<i32> {
        if name.is_empty() {
            return None;
        }
        self.row(name, Some(section)).map(|r| r.index)
    }

    /// `hdt_json.c:hdt_json_lookup_sprite_name`: first match.
    pub fn lookup(&self, name: &str) -> Option<i32> {
        if name.is_empty() {
            return None;
        }
        self.rows.iter().find(|r| r.name == name).map(|r| r.index)
    }

    /// `hdt_json.c:hdt_json_max_sprite_index_section`.
    pub fn max_index_section(&self, section: &str) -> i32 {
        self.rows.iter().filter(|r| r.section == section).map(|r| r.index).max().unwrap_or(-1)
    }

    /// `hdt_json.c:hdt_json_register_sprite_name`: a name already present keeps its index.
    pub fn register(&mut self, name: &str, index: i32, diag: &mut Diagnostics, folder: &str) {
        if name.is_empty() || index <= 0 {
            return;
        }
        let name = clip(name, 39);
        if self.rows.iter().any(|r| r.name == name) {
            return;
        }
        if self.rows.len() >= MAX_SPRITE_NAMES {
            diag.warn(folder, format!("sprite name table full: '{name}' not registered"));
            return;
        }
        self.rows.push(SpriteName { name, section: String::new(), index });
    }
}

/// `strtol(s, &end, 10)` requiring `*end == '\0'` (leading spaces and sign accepted).
pub fn strtol_full(s: &str) -> Option<i64> {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut v: i64 = 0;
    for c in digits.bytes() {
        v = v.saturating_mul(10).saturating_add((c - b'0') as i64);
    }
    Some(if neg { -v } else { v })
}
