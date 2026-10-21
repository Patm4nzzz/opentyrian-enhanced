//! Enemy behaviors described as data (ADR 0081) and table fixes
//! provided by mods (ADR 0080).
//!
//! The original content has NO attached behavior: `Enemy::behavior`
//! is 0 and none of the functions in this module does anything, nor
//! consumes the random generator. The golden masters therefore stay
//! identical by construction.
//!
//! A behavior grafts onto the C enemy model
//! (`enemy_render.c:JE_drawEnemy`) without replacing it:
//! - `path`: sequence of segments that set `exc`/`eyc` (C speeds)
//!   for a number of ticks; the movement itself remains the C one
//!   (`ex += exc`, `ey += eyc`, bounds, scrolling);
//! - `track`: deterministic pursuit of the player (no random draw,
//!   unlike the C `xaccel > 89`);
//! - `turrets`: extra turrets that fire a weapon of any index
//!   (the C limits `tur[]` to one byte) through the same firing code
//!   as the original turrets (`fire_enemy_turret`).
//!
//! Integers only (ADR 0003).

use crate::world::World;
use tyrian_formats::hdt::{EnemyDat, Shield, Ship, Sidekick, Special, Weapon, WeaponPort};

/// Maximum number of segments in a path.
pub const MAX_PATH_SEGMENTS: usize = 32;
/// Maximum number of turrets added by a behavior.
pub const MAX_BEHAVIOR_TURRETS: usize = 4;

/// A path segment: for `ticks` enemy ticks, the C speeds
/// `exc`/`eyc` equal `vx`/`vy` (None = left as they are).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PathSegment {
    pub ticks: u16,
    pub vx: Option<i8>,
    pub vy: Option<i8>,
}

/// Player pursuit: every `every` ticks, the speed on the chosen axis
/// moves one unit closer to the player, capped at `max_speed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Track {
    pub x: bool,
    pub y: bool,
    pub max_speed: u8,
    pub every: u8,
}

/// Added turret: fires weapon `weapon` (index into `weapons[]`, 1..=2999)
/// every `every` ticks (first volley after `start` ticks).
/// `slot` (1..=3) chooses the shot orientation exactly like the C turret
/// index (`enemy_render.c:JE_drawEnemy`: 1 = downwards, 2 and 3 =
/// swapped speeds) and the `eshotmultipos` cycle counter used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BehaviorTurret {
    pub weapon: u16,
    pub every: u8,
    pub start: u8,
    pub slot: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnemyBehavior {
    pub path: Vec<PathSegment>,
    /// Restart the path at the first segment after the last one.
    pub path_loop: bool,
    pub track: Option<Track>,
    pub turrets: Vec<BehaviorTurret>,
}

/// Records written by mods into the episode tables, applied
/// by `Tables::apply` at level start. Empty for the original game.
#[derive(Debug, Clone, Default)]
pub struct TablePatch {
    pub weapons: Vec<(u16, Weapon)>,
    pub ports: Vec<(u8, WeaponPort)>,
    pub ships: Vec<(u8, Ship)>,
    pub options: Vec<(u8, Sidekick)>,
    pub shields: Vec<(u8, Shield)>,
    pub specials: Vec<(u8, Special)>,
    pub enemies: Vec<(u16, EnemyDat)>,
    pub behaviors: Vec<EnemyBehavior>,
    /// (enemy index, index into `behaviors`).
    pub enemy_behavior: Vec<(u16, u16)>,
}

impl TablePatch {
    pub fn is_empty(&self) -> bool {
        self.weapons.is_empty()
            && self.ports.is_empty()
            && self.ships.is_empty()
            && self.options.is_empty()
            && self.shields.is_empty()
            && self.specials.is_empty()
            && self.enemies.is_empty()
            && self.behaviors.is_empty()
    }
}

impl World {
    /// Called by `make_enemy`: attaches the behavior of enemy `e_dat_i`.
    pub(crate) fn behavior_init(&self, e_dat_i: u16) -> (u16, [u8; MAX_BEHAVIOR_TURRETS]) {
        let b = self.tables.behavior_of(e_dat_i as usize);
        let mut wait = [0u8; MAX_BEHAVIOR_TURRETS];
        if let Some(beh) = b.and_then(|b| self.tables.behaviors.get(b as usize - 1)) {
            for (k, t) in beh.turrets.iter().take(MAX_BEHAVIOR_TURRETS).enumerate() {
                wait[k] = t.start.max(1);
            }
        }
        (b.unwrap_or(0), wait)
    }

    /// Path and pursuit, at the top of the update of enemy `i`
    /// (before the C movement code, which applies `exc`/`eyc`).
    pub(crate) fn behavior_motion(&mut self, i: usize) {
        let b = self.enemy[i].behavior;
        let Some(beh) = self.tables.behaviors.get(b as usize - 1) else {
            return;
        };
        let e = &mut self.enemy[i];
        if !beh.path.is_empty() {
            let mut seg = e.behavior_seg as usize;
            if seg < beh.path.len() {
                if e.behavior_seg_tick == 0 {
                    let s = beh.path[seg];
                    if let Some(vx) = s.vx {
                        e.exc = vx;
                    }
                    if let Some(vy) = s.vy {
                        e.eyc = vy;
                    }
                }
                e.behavior_seg_tick = e.behavior_seg_tick.saturating_add(1);
                if e.behavior_seg_tick >= beh.path[seg].ticks.max(1) {
                    e.behavior_seg_tick = 0;
                    seg += 1;
                    if seg >= beh.path.len() && beh.path_loop {
                        seg = 0;
                    }
                    e.behavior_seg = seg.min(255) as u8;
                }
            }
        }
        e.behavior_tick = e.behavior_tick.wrapping_add(1);
        if let Some(t) = beh.track {
            if t.every > 0 && e.behavior_tick.is_multiple_of(t.every as u16) {
                let lim = t.max_speed as i32;
                // draw_enemy has already subtracted 25 from players[0].x (same frame as the C).
                let px = self.players[0].x;
                let py = self.players[0].y;
                let e = &mut self.enemy[i];
                if t.x {
                    let target = if px > e.ex as i32 {
                        1
                    } else if px < e.ex as i32 {
                        -1
                    } else {
                        0
                    };
                    let v = (e.exc as i32 + target).clamp(-lim, lim);
                    e.exc = v as i8;
                }
                if t.y {
                    let target = if py > e.ey as i32 {
                        1
                    } else if py < e.ey as i32 {
                        -1
                    } else {
                        0
                    };
                    let v = (e.eyc as i32 + target).clamp(-lim, lim);
                    e.eyc = v as i8;
                }
            }
        }
    }

    /// Turrets of the behavior, after the C turrets. Same rules as
    /// `JE_drawEnemy`: delay divided according to difficulty, fire through
    /// `fire_enemy_turret`. Returns `false` if no more shots are free.
    pub(crate) fn behavior_turrets(&mut self, i: usize, temp_x: i16, temp_y: i16, tmx: i32) -> bool {
        let b = self.enemy[i].behavior;
        let Some(beh) = self.tables.behaviors.get(b as usize - 1) else {
            return true;
        };
        let turrets: Vec<BehaviorTurret> = beh.turrets.iter().take(MAX_BEHAVIOR_TURRETS).copied().collect();
        for (k, t) in turrets.iter().enumerate() {
            if t.every == 0 || t.weapon == 0 {
                continue;
            }
            let wait = &mut self.enemy[i].behavior_turret_wait[k];
            *wait = wait.saturating_sub(1);
            if *wait != 0 {
                continue;
            }
            let mut w = t.every;
            if self.difficulty > crate::world::DIFFICULTY_NORMAL {
                w = (w / 2) + 1;
                if self.difficulty > crate::world::DIFFICULTY_MANIACAL {
                    w = (w / 2) + 1;
                }
            }
            self.enemy[i].behavior_turret_wait[k] = w;
            if !self.fire_enemy_turret(i, t.slot.clamp(1, 3) as usize, t.weapon as usize, temp_x, temp_y, tmx) {
                return false;
            }
        }
        true
    }
}
