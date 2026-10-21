//! Mod PNGs placed in the sprite override tables
//! (`sprite_override.c`), ship poses and tints.

use super::*;

/// `mod_manager.c:parse_tint`.
pub(super) fn parse_tint(v: Option<&Value>) -> Option<f32> {
    match v? {
        Value::Number(h) => (*h >= 0.0 && *h < 1.0).then_some(*h as f32),
        Value::String(s) => match s.as_str() {
            "red" => Some(0.00),
            "orange" => Some(0.08),
            "yellow" => Some(0.15),
            "green" => Some(0.30),
            "cyan" => Some(0.50),
            "blue" => Some(0.66),
            "purple" => Some(0.75),
            "magenta" => Some(0.83),
            "pink" => Some(0.92),
            _ => None,
        },
        _ => None,
    }
}

/// State of the sprite override tables (`sprite_override.c`): only the occupied
/// indices count for automatic assignment.
#[derive(Default)]
pub(super) struct SpriteTables {
    pub(super) occupied: BTreeMap<SpriteTable, BTreeSet<i32>>,
    pub(super) out: Vec<SpriteAssignment>,
}

impl SpriteTables {
    pub(super) fn has(&self, t: SpriteTable, i: i32) -> bool {
        self.occupied.get(&t).is_some_and(|s| s.contains(&i))
    }
    pub(super) fn set(&mut self, a: SpriteAssignment) {
        self.occupied.entry(a.table).or_default().insert(a.index);
        // table_set replaces the existing entry.
        self.out.retain(|x| !(x.table == a.table && x.index == a.index && x.bank == a.bank));
        self.out.push(a);
    }
    pub(super) fn get(&self, t: SpriteTable, i: i32) -> Option<&SpriteAssignment> {
        self.out.iter().find(|a| a.table == t && a.index == i)
    }
    /// `sprite_override_copy_in_table`: never replaces an occupied entry.
    pub(super) fn copy(&mut self, t: SpriteTable, src: i32, dst: i32) {
        if src == dst || dst < 0 || dst >= t.capacity() || self.has(t, dst) {
            return;
        }
        if let Some(a) = self.get(t, src).cloned() {
            self.set(SpriteAssignment { index: dst, ..a });
        }
    }
    /// `sprite_override_clone_retint`.
    pub(super) fn clone_retint(&mut self, t: SpriteTable, src: i32, start: i32, hue: f32) -> Option<i32> {
        let a = self.get(t, src)?.clone();
        let dst = (start.max(0)..t.capacity()).find(|&i| !self.has(t, i))?;
        self.set(SpriteAssignment { index: dst, hue: Some(hue), ..a });
        Some(dst)
    }
}

impl Ctx<'_> {
    // ---------------------------------------------------------------- sprites

    /// `mod_manager.c:load_mod_sprites` + `sprite_override.c:load_png_abs_dir_scaled`.
    pub(super) fn load_sprites(&mut self, m: &ModFolder, hue: Option<f32>) -> usize {
        let mut n = 0;
        for png in &m.pngs {
            let table = match crate::folder::SPRITE_DIRS.iter().find(|(d, _)| *d == png.dir) {
                Some((_, t)) => *t,
                None => continue,
            };
            if table == SpriteTable::Enemies {
                // The PNG carries the name of an original enemy (ADR 0128): it replaces
                // the first image of that enemy (bank `shapebank`, image
                // `egraphic[0]`), the same one in the tables of all episodes.
                match self.originals.enemies.get(&png.basename) {
                    Some(e) if e.bank > 0 && e.record.egraphic[0] > 0 => {
                        self.sprites.out.push(SpriteAssignment {
                            table,
                            index: e.record.egraphic[0] as i32,
                            path: png.path.clone(),
                            hue,
                            bank: e.bank as i32,
                        });
                        n += 1;
                    }
                    Some(_) => self.diag.error(
                        &m.folder,
                        format!("sprites/enemies/{}.png: this original enemy has no image to replace", png.basename),
                    ),
                    None => self.diag.error(
                        &m.folder,
                        format!(
                            "sprites/enemies/{}.png: unknown original enemy '{}' (rewrite/mods/original-names.json)",
                            png.basename, png.basename
                        ),
                    ),
                }
                continue;
            }
            let Some(index) = self.png_index(table, &png.dir, &png.basename, &m.folder) else {
                continue;
            };
            self.sprites.set(SpriteAssignment { table, index, path: png.path.clone(), hue, bank: 0 });
            n += 1;
        }
        n
    }

    pub(super) fn png_index(&mut self, table: SpriteTable, dir: &str, basename: &str, folder: &str) -> Option<i32> {
        if strtol_full(basename).is_some() {
            self.diag.error(folder, format!("sprites/{dir}/{basename}.png: a PNG is named after the sprite it replaces (data/sprite_manifest.json) or has a new name, never a number"));
            return None;
        }
        let section = table.section();
        let leaf = dir.rsplit('/').next().unwrap_or(dir);
        if let Some(i) = self
            .names
            .lookup_section(&format!("{leaf}/{basename}"), section)
            .or_else(|| self.names.lookup_section(basename, section))
        {
            if i < 0 || i >= table.capacity() {
                self.diag.warn(
                    folder,
                    format!(
                        "sprites/{dir}/{basename}.png: named index {i} beyond capacity {}, ignored",
                        table.capacity()
                    ),
                );
                return None;
            }
            return Some(i);
        }
        let mut base = table.auto_base();
        if !matches!(table, SpriteTable::ShopWeapons | SpriteTable::ShopShields) {
            let named_max = self.names.max_index_section(section);
            if let Some(next) = named_max.checked_add(1) {
                if named_max >= base && next < table.capacity() {
                    base = next;
                }
            }
        }
        let step = if table == SpriteTable::Ships { crate::folder::SHIP_IMAGE_STEP } else { 1 };
        let mut i = base;
        while i < table.capacity() {
            if !self.sprites.has(table, i) {
                self.names.register(basename, i, &mut self.diag, folder);
                return Some(i);
            }
            i += step;
        }
        self.diag.warn(folder, format!("sprites/{dir}/{basename}.png: table {} full", table.name()));
        None
    }

    /// `mod_manager.c:auto_bank_mod_ships` (+ named poses from wip/sept-2026).
    ///
    /// The poses sit around the ship's own `shipgraphic` (-4, -2, +2, +4), the
    /// index the game draws them from (`level.gd`, `banking * 2 + shipgraphic`).
    /// The C version also copied them to `shipgraphic - 500`, the sheet-12 index
    /// of an original T2000 ship; the rewrite reads the mod table at the full
    /// index, and with the larger table (ADR 0134) those copies would land on the
    /// original ships 400-657 (Storm, Red Dragon...): they are no longer made.
    pub(super) fn auto_bank_ships(&mut self) {
        const POSE_OFFSETS: [i32; 4] = [-4, -2, 2, 4];
        for id in SHIP_RANGE.0..=SHIP_RANGE.1 {
            if !self.ship_used[id] {
                continue;
            }
            let base = self.t.ships[id].shipgraphic as i32;
            if let Some(poses) = self.pending_ship_poses.get(&id).cloned() {
                for (k, name) in poses.iter().enumerate() {
                    if name.is_empty() {
                        continue;
                    }
                    match self.names.lookup(name) {
                        Some(src) => self.sprites.copy(SpriteTable::Ships, src, base + POSE_OFFSETS[k]),
                        None => self.diag.warn("", format!("ship_poses: '{name}' not found")),
                    }
                }
            }
            if base < 0 || base >= SpriteTable::Ships.capacity() || !self.sprites.has(SpriteTable::Ships, base) {
                continue;
            }
            for off in POSE_OFFSETS {
                self.sprites.copy(SpriteTable::Ships, base, base + off);
            }
        }
    }

    /// `mod_manager.c:apply_per_item_tints`.
    pub(super) fn apply_per_item_tints(&mut self) {
        let tints: Vec<(usize, f32)> = self.weapon_tint.iter().map(|(&i, &h)| (i, h)).collect();
        for (id, hue) in tints {
            if !self.weapon_used[id] {
                continue;
            }
            let icon = self.t.ports[id].itemgraphic as i32;
            if icon > 0 {
                if let Some(ni) = self.sprites.clone_retint(SpriteTable::ShopWeapons, icon, 300, hue) {
                    self.t.ports[id].itemgraphic = ni as u16;
                }
            }
            for bay in 0..2 {
                for lvl in 0..11 {
                    let wid = self.t.ports[id].op[bay][lvl] as usize;
                    if wid == 0 || wid > WEAP_NUM {
                        continue;
                    }
                    for si in 0..8 {
                        let sg = self.t.weapons[wid].sg[si] as i32;
                        if sg == 0 {
                            continue;
                        }
                        if let Some(n) = self.sprites.clone_retint(SpriteTable::Shots, sg, 900, hue) {
                            self.t.weapons[wid].sg[si] = n as u16;
                            self.touched_weapons.insert(wid);
                        }
                    }
                }
            }
        }
    }
}
