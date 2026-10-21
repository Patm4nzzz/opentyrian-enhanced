//! Item tables indexed like the C global arrays
//! (`episodes.c:JE_loadItemDat`: `weapons[WEAP_NUM + 1]`, `weaponPort[PORT_NUM + 1]`,
//! `special[SPECIAL_NUM + 1]`, `powerSys[POWER_NUM + 1]`, `ships[SHIP_NUM + 1]`,
//! `options[OPTION_NUM + 1]`, `shields[SHIELD_NUM + 1]`, `enemyDat[ENEMY_NUM + 1]`).
//! Entries absent from the file are zero, like the C `memset`.

use crate::behavior::{EnemyBehavior, TablePatch};
use tyrian_formats::hdt::{
    EnemyDat, Generator, ItemTables, Shield, Ship, Sidekick, Special, Weapon, WeaponPort, ITEM_BLOCK_SIZE,
};

/// `lvlmast.h`.
pub const WEAP_NUM: usize = 2999;
pub const PORT_NUM: usize = 80;
pub const POWER_NUM: usize = 6;
pub const OPTION_NUM: usize = 50;
pub const SHIP_NUM: usize = 32;
pub const SHIELD_NUM: usize = 20;
pub const SPECIAL_NUM: usize = 70;
pub const ENEMY_NUM: usize = 1850;

fn fill<T: Clone>(src: &[T], len: usize, z: T) -> Vec<T> {
    let mut v: Vec<T> = src.iter().take(len).cloned().collect();
    v.resize(len, z);
    v
}

#[derive(Clone)]
pub struct Tables {
    pub weapons: Vec<Weapon>,
    pub ports: Vec<WeaponPort>,
    pub specials: Vec<Special>,
    pub generators: Vec<Generator>,
    pub ships: Vec<Ship>,
    pub options: Vec<Sidekick>,
    pub shields: Vec<Shield>,
    pub enemies: Vec<EnemyDat>,
    /// Data-driven behaviors (behavior.rs); empty for the original game.
    pub behaviors: Vec<EnemyBehavior>,
    /// 1-based index into `behaviors` per enemy (0 = none), allocated
    /// only if a mod attaches a behavior.
    pub enemy_behavior: Vec<u16>,
}

impl Tables {
    pub fn new(items: &ItemTables) -> Self {
        // Zero records: an item block that is entirely zero.
        let z = ItemTables::parse(&vec![0u8; ITEM_BLOCK_SIZE]).expect("zero block");
        let mut weapons = vec![z.weapons[0].clone(); WEAP_NUM + 1];
        for (id, w) in weapons.iter_mut().enumerate() {
            if let Some(src) = items.weapon(id) {
                *w = src.clone();
            }
        }
        let mut enemies = vec![z.enemies[0].clone(); ENEMY_NUM + 1];
        for (id, e) in enemies.iter_mut().enumerate() {
            if let Some(src) = items.enemy(id) {
                *e = src.clone();
            }
        }
        Self {
            weapons,
            ports: fill(&items.weapon_ports, PORT_NUM + 1, z.weapon_ports[0].clone()),
            specials: fill(&items.specials, SPECIAL_NUM + 1, z.specials[0].clone()),
            generators: fill(&items.generators, POWER_NUM + 1, z.generators[0].clone()),
            ships: fill(&items.ships, SHIP_NUM + 1, z.ships[0].clone()),
            options: fill(&items.sidekicks, OPTION_NUM + 1, z.sidekicks[0].clone()),
            shields: fill(&items.shields, SHIELD_NUM + 1, z.shields[0].clone()),
            enemies,
            behaviors: Vec::new(),
            enemy_behavior: Vec::new(),
        }
    }

    /// Writes a mod's records over the original tables.
    /// Out-of-table indices are ignored (the mod loader has already
    /// rejected them; this is only a second barrier, without panicking).
    pub fn apply(&mut self, patch: &TablePatch) {
        fn put<T: Clone>(v: &mut [T], id: usize, x: &T) {
            if let Some(slot) = v.get_mut(id) {
                *slot = x.clone();
            }
        }
        for (id, w) in &patch.weapons {
            put(&mut self.weapons, *id as usize, w);
        }
        for (id, p) in &patch.ports {
            put(&mut self.ports, *id as usize, p);
        }
        for (id, s) in &patch.ships {
            put(&mut self.ships, *id as usize, s);
        }
        for (id, s) in &patch.options {
            put(&mut self.options, *id as usize, s);
        }
        for (id, s) in &patch.shields {
            put(&mut self.shields, *id as usize, s);
        }
        for (id, s) in &patch.specials {
            put(&mut self.specials, *id as usize, s);
        }
        for (id, e) in &patch.enemies {
            put(&mut self.enemies, *id as usize, e);
        }
        if !patch.behaviors.is_empty() {
            self.behaviors = patch.behaviors.clone();
            self.enemy_behavior = vec![0; ENEMY_NUM + 1];
            for &(enemy, b) in &patch.enemy_behavior {
                if (enemy as usize) <= ENEMY_NUM && (b as usize) < self.behaviors.len() {
                    self.enemy_behavior[enemy as usize] = b + 1;
                }
            }
        }
    }

    /// Behavior (1-based) attached to enemy `id`, if there is one.
    pub fn behavior_of(&self, id: usize) -> Option<u16> {
        match self.enemy_behavior.get(id) {
            Some(&b) if b > 0 => Some(b),
            _ => None,
        }
    }

    pub fn weapon(&self, id: usize) -> &Weapon {
        &self.weapons[id.min(WEAP_NUM)]
    }

    pub fn enemy(&self, id: usize) -> &EnemyDat {
        &self.enemies[id.min(ENEMY_NUM)]
    }

    /// `episodes.c:weapon_port_level_count`: defined levels (trailing zero
    /// slots of the table removed).
    pub fn port_level_count(&self, port: usize, mode: usize) -> usize {
        if port > PORT_NUM || mode > 1 {
            return 0;
        }
        let mut n = 11;
        while n > 0 && self.ports[port].op[mode][n - 1] == 0 {
            n -= 1;
        }
        n
    }

    /// `episodes.c:weapon_port_shot`: weapon fired by a port at a level,
    /// capped at the last defined level.
    pub fn port_shot(&self, port: usize, mode: usize, level: usize) -> u16 {
        let defined = self.port_level_count(port, mode);
        if defined == 0 {
            return 0;
        }
        self.ports[port].op[mode][level.min(defined - 1)]
    }
}
