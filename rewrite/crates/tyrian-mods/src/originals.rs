//! Named catalog of the original content a mod can cite: enemies and
//! enemy weapons from the tables of `tyrian.hdt` (episodes 1 to 3) and from the end
//! block of `tyrian4.lvl` (episodes 4 and 5, different table: the same
//! slot there often holds another enemy).
//!
//! A mod never cites a slot: it cites a NAME, and the loader
//! finds the slots in the table of the episode being played. Names are
//! computed from the data, deterministically, and published in
//! `rewrite/mods/original-names.json` (`tyrian-mods catalog`); a test
//! checks that the published file is the one the data produces.
//!
//! Naming rules (one name = one slot; the same slot of the two
//! tables carries the same name when it shows the same images there):
//! - enemy: theme of its image bank (`BANK_THEMES`), rank of its
//!   first image in the bank (`_01`, `_02`…), then a letter for the
//!   other enemies with the same images (`_02b`); a record without an image
//!   bank is called `no_image_NNNN` (in slot order);
//! - enemy weapon (slots 1..255): name (sprite manifest) of its
//!   first drawn projectile then a rank (`_01`); `no_sprite_NN` if none.

use std::collections::BTreeMap;
use tyrian_formats::hdt::{enemy_id, weapon_id, EnemyDat, ItemTables, Weapon};

use crate::manifest::SpriteNames;

/// Original tables: episodes 1 to 3, episodes 4 and 5.
pub const TABLE_GROUPS: [&str; 2] = ["1-3", "4-5"];

/// Original table used by an episode (`episodes.c:JE_loadItemDat`).
pub fn table_group(episode: u8) -> usize {
    usize::from(episode >= 4)
}

/// Theme of each enemy image bank (`shapebank` 1..36), chosen
/// from the images it contains (sheets in `docs/original-enemies/`).
pub const BANK_THEMES: [&str; 36] = [
    "tech_corridor",    // 1  newsh2.shp
    "insect_boss",      // 2  newsh4.shp
    "bank_03",          // 3  newsh7.shp
    "rock_caves",       // 4  newsh8.shp
    "pickup_items",     // 5  newsha.shp
    "bank_06",          // 6  newshb.shp
    "fleet_assault",    // 7  newshc.shp
    "asteroid_field",   // 8  newshd.shp
    "lava_centipede",   // 9  newshe.shp
    "jungle_fighters",  // 10 newshf.shp
    "gem_minefield",    // 11 newshg.shp
    "dark_waterfall",   // 12 newshh.shp
    "bank_13",          // 13 newshi.shp
    "jungle_fortress",  // 14 newshj.shp
    "fireball_station", // 15 newshk.shp
    "orbital_weapons",  // 16 newshl.shp
    "cargo_depot",      // 17 newshm.shp
    "organic_horror",   // 18 newshn.shp
    "mech_walker",      // 19 newsho.shp
    "bank_20",          // 20 newshp.shp
    "bank_21",          // 21 tyrian.shp, coins and gems
    "dome_city",        // 22 newshr.shp
    "rail_platform",    // 23 newshs.shp
    "ocean_world",      // 24 newsht.shp
    "skull_fleet",      // 25 newshu.shp
    "bank_26",          // 26 tyrian.shp, two-player items
    "blade_spinners",   // 27 newsh#.shp
    "pipe_cannons",     // 28 newshv.shp
    "dragon_boss",      // 29 newsh0.shp
    "explosion_fx",     // 30 newsh@.shp
    "volcanic_ruins",   // 31 newsh3.shp
    "spike_fortress",   // 32 newsh^.shp
    "bank_33",          // 33 newsh5.shp
    "cyber_spheres",    // 34 newsh9.shp
    "garden_temple",    // 35 newsh'.shp
    "crystal_hangar",   // 36 newsh%.shp
];

/// An enemy's turrets (`EnemyDat.tur`) fit in one byte: only
/// weapons 1..255 can be cited there, and a chain reaction (damage
/// 100..249) only designates weapons 0..149.
pub const MAX_TURRET_WEAPON: u16 = 255;
pub const MAX_CHAIN_WEAPON: u16 = 149;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalEnemy {
    /// Image bank (`shapebank`), 0 = none.
    pub bank: u8,
    /// Slot (`enemyDat` index) in each original table, if it appears there.
    pub slots: [Option<u16>; 2],
    /// Record from the first table that contains it.
    pub record: EnemyDat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalWeapon {
    /// Slot (`weapons` index, 1..255) in each original table.
    pub slots: [Option<u16>; 2],
    /// Name of the first drawn projectile (manifest), empty if none.
    pub sprite: String,
}

#[derive(Debug, Clone, Default)]
pub struct OriginalNames {
    pub enemies: BTreeMap<String, OriginalEnemy>,
    pub weapons: BTreeMap<String, OriginalWeapon>,
}

/// Variant suffix: "", "b", "c"… then "z2", "z3"… beyond 25 variants.
fn variant(n: usize) -> String {
    match n {
        0 => String::new(),
        1..=24 => ((b'b' + (n - 1) as u8) as char).to_string(),
        _ => format!("z{}", n - 23),
    }
}

/// Groups the slots of the two tables: the same slot with the same
/// `key` in both tables forms a single entry.
fn entries<K: PartialEq + Clone>(count: usize, key: impl Fn(usize, usize) -> Option<K>) -> Vec<(K, [Option<u16>; 2])> {
    let mut out = Vec::new();
    for rec in 0..count {
        match (key(0, rec), key(1, rec)) {
            (Some(a), Some(b)) if a == b => out.push((a, [Some(rec as u16), Some(rec as u16)])),
            (a, b) => {
                if let Some(a) = a {
                    out.push((a, [Some(rec as u16), None]));
                }
                if let Some(b) = b {
                    out.push((b, [None, Some(rec as u16)]));
                }
            }
        }
    }
    out
}

impl OriginalNames {
    /// Computes the names from the two original tables (episode 1, episode 4).
    pub fn generate(tables: [&ItemTables; 2], sprites: &SpriteNames) -> Self {
        let mut out = Self::default();
        let n = tables[0].enemies.len().min(tables[1].enemies.len());
        let list = entries(n, |g, rec| {
            let e = &tables[g].enemies[rec];
            Some((e.shapebank, e.egraphic))
        });
        // Image rank per bank: first distinct images, sorted.
        let mut images: BTreeMap<u8, Vec<u16>> = BTreeMap::new();
        for ((bank, g), _) in &list {
            images.entry(*bank).or_default().push(g[0]);
        }
        for v in images.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        let mut variants: BTreeMap<String, usize> = BTreeMap::new();
        let mut no_image = 0;
        for ((bank, gfx), slots) in list {
            let rec = slots.iter().position(Option::is_some).unwrap();
            let record = tables[rec].enemies[slots[rec].unwrap() as usize].clone();
            let slots = slots.map(|s| s.map(|r| enemy_id(r as usize) as u16));
            let name = if bank == 0 || bank as usize > BANK_THEMES.len() {
                no_image += 1;
                format!("no_image_{no_image:04}")
            } else {
                let rank = images[&bank].iter().position(|&g| g == gfx[0]).unwrap() + 1;
                let base = format!("{}_{rank:02}", BANK_THEMES[bank as usize - 1]);
                let v = variants.entry(base.clone()).or_insert(0);
                *v += 1;
                format!("{base}{}", variant(*v - 1))
            };
            out.enemies.insert(name, OriginalEnemy { bank, slots, record });
        }
        // Enemy weapons: slots 1..255 (`EnemyDat.tur` on one byte).
        let sprite_name = |w: &Weapon| -> String {
            w.sg.iter()
                .find(|&&g| g != 0)
                .and_then(|&g| sprites.rows.iter().find(|r| r.section == "shots" && r.index == g as i32))
                .map(|r| r.name.clone())
                .unwrap_or_default()
        };
        let n = tables[0].weapons.len().min(tables[1].weapons.len()).min(MAX_TURRET_WEAPON as usize + 1);
        let list = entries(n, |g, rec| (weapon_id(rec) != 0).then(|| sprite_name(&tables[g].weapons[rec])));
        let mut ranks: BTreeMap<String, usize> = BTreeMap::new();
        for (sprite, slots) in list {
            let slots = slots.map(|s| s.map(|r| weapon_id(r as usize) as u16));
            let stem = if sprite.is_empty() { "no_sprite".to_string() } else { sprite.clone() };
            let k = ranks.entry(stem.clone()).or_insert(0);
            *k += 1;
            out.weapons.insert(format!("{stem}_{k:02}"), OriginalWeapon { slots, sprite });
        }
        out
    }

    /// Name of the original enemy at slot `slot` of table `group`.
    pub fn enemy_name(&self, slot: u16, group: usize) -> Option<&str> {
        self.enemies.iter().find(|(_, e)| e.slots[group] == Some(slot)).map(|(n, _)| n.as_str())
    }

    /// Published file (`rewrite/mods/original-names.json`): stable text,
    /// one line per name, sorted.
    pub fn to_json(&self) -> String {
        let slot = |v: Option<u16>| v.map_or("null".to_string(), |s| s.to_string());
        let mut s = String::from("{\n");
        s += "  \"about\": \"Names of the original enemies and enemy weapons a mod can cite (MODDING.md section 7). Generated by: tyrian-mods catalog. Slots are informative: a mod never writes them.\",\n";
        s += "  \"enemies\": {\n";
        let n = self.enemies.len();
        for (k, (name, e)) in self.enemies.iter().enumerate() {
            let theme = if e.bank == 0 { "none" } else { BANK_THEMES[e.bank as usize - 1] };
            s += &format!(
                "    \"{name}\": {{ \"bank\": \"{theme}\", \"armor\": {}, \"value\": {}, \"large\": {}, \"slot_episodes_1_3\": {}, \"slot_episodes_4_5\": {} }}{}\n",
                e.record.armor,
                e.record.value,
                e.record.esize == 1,
                slot(e.slots[0]),
                slot(e.slots[1]),
                if k + 1 < n { "," } else { "" }
            );
        }
        s += "  },\n  \"enemy_weapons\": {\n";
        let n = self.weapons.len();
        for (k, (name, w)) in self.weapons.iter().enumerate() {
            s += &format!(
                "    \"{name}\": {{ \"sprite\": \"{}\", \"slot_episodes_1_3\": {}, \"slot_episodes_4_5\": {} }}{}\n",
                w.sprite,
                slot(w.slots[0]),
                slot(w.slots[1]),
                if k + 1 < n { "," } else { "" }
            );
        }
        s += "  }\n}\n";
        s
    }
}
