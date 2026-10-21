//! "Enhanced" effects of the C version as seen by the simulation.
//!
//! Two families:
//!
//! - what changes the game and that the C code only enables for the player:
//!   `hitstop.c` (enemy freeze, cascade of secondary explosions of a defeated
//!   boss) and `varz.c:JE_setupExplosionLarge` in "Modern" style (the
//!   four angle sprites are no longer placed). `golden_master.c:
//!   golden_master_level_start` forces `hitstop_enabled = false` and
//!   `particles_enabled = false`: [`FxConfig`] is therefore all false by default
//!   (golden masters, replays, tools) and is only filled in by the
//!   player's game (tyrian-godot, Setup settings);
//! - what is only rendering (particles, shockwaves, damage numbers,
//!   vibration): the simulation only records, where the C code emits its
//!   events (`event_bus.c`) or calls its particle generators,
//!   what happened ([`FxEvent`]). The renderer consumes them with its own
//!   generator: nothing here touches `rng` or the hashed state.

use crate::world::World;

/// Player settings that act on the simulation (all false = golden master).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FxConfig {
    /// `hitstop.c:hitstop_enabled` (Setup > Gameplay > Freeze Frames).
    pub hitstop: bool,
    /// `varz.c:JE_setupExplosionLarge`: `modern_fx` (Modern explosion
    /// style, particles active, processorType >= 2, screenshake active).
    pub modern_explosions: bool,
    /// Record the events for rendering ([`World::fx_event`]).
    pub events: bool,
}

/// `hitstop.c`: state of the freeze and of the cascade of the last defeated boss.
#[derive(Debug, Clone, Default)]
pub struct Hitstop {
    /// `hitstop.c:freeze_frames`.
    pub freeze_frames: u32,
    /// `hitstop.c:secondary_explosions[0..secondary_count]`: (x, y, delay).
    /// Filled at each defeated boss even when hitstop is disabled (the
    /// `rng_range` draws happen), played only if it is active.
    pub secondary: Vec<(i32, i32, i32)>,
    /// Enemy updates skipped by the freeze (measurement, outside the hash).
    pub frozen_updates: u64,
}

/// Player shot at the origin of a hit: image (`shotGr + shotAni`) and
/// speed (`shotXM`, `shotYM`), read by the effect listeners.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShotInfo {
    pub frame: u16,
    pub dx: i32,
    pub dy: i32,
}

/// What the C code passes to its effect listeners (`event_bus.h:EventData`)
/// or to its particle generators, in game_screen coordinates.
/// `frame`: image of the player shot (`shotGr + shotAni`), from which rendering
/// derives the hue (`enemy_render.c:sprite2_dominant_hue` of spriteSheet8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxEvent {
    /// EVENT_SHOT_HIT (`shot_update.c`, non-lethal and lethal hits).
    ShotHit { x: i32, y: i32, damage: i32, frame: u16 },
    /// `shot_update.c`: impact wave at the center of the enemy hit
    /// (`shockwave_trigger_sized(.., 0xFFF4E0B0, 8, 5)`).
    HitRipple { x: i32, y: i32 },
    /// EVENT_ENEMY_KILLED (`collision.c`, two death chains).
    EnemyKilled { x: i32, y: i32, damage: i32, value: i32, frame: u16, shot_dx: i32, shot_dy: i32 },
    /// EVENT_BOSS_DEFEATED (`collision.c:boss_track_flush`).
    BossDefeated { x: i32, y: i32, value: i32, frame: u16 },
    /// `shot_update.c:JE_updateEnemyShots`: impact wave on the ship.
    PlayerRipple { x: i32, y: i32 },
    /// `varz.c:JE_playerDamage`: cyan flash of shield absorption.
    ShieldAbsorb { x: i32, y: i32 },
    /// EVENT_PLAYER_HIT (`varz.c:JE_playerDamage`, damage > 0).
    PlayerHit { player: u8, damage: i32 },
    /// EVENT_PLAYER_DIED (`varz.c:JE_playerDamage`).
    PlayerDied { player: u8 },
    /// `varz.c`: `particles_spawn_explosion_fx(x, y, large, ground)`.
    Explosion { x: i32, y: i32, large: bool, ground: bool },
    /// `enemy_render.c`: `particles_spawn_enemy_trail` (fast enemy).
    EnemyTrail { x: i32, y: i32 },
    /// `enemy_render.c`: `particles_spawn_smoke` (enemy below 20%).
    EnemySmoke { x: i32, y: i32 },
    /// EVENT_POWERUP_COLLECTED of a passage to a secret level
    /// (`hitstop.c:on_powerup_collected`, white flash).
    SecretWarp,
}

impl World {
    /// Records an effect event, with the iteration in which it happens.
    pub(crate) fn fx_event(&mut self, ev: FxEvent) {
        if !self.fx.events {
            return;
        }
        // With no reader (fast-forward), we do not keep an endless log.
        if self.render.fx_events.len() >= 4096 {
            self.render.fx_events.clear();
        }
        self.render.fx_events.push((self.iteration, ev));
    }

    /// `hitstop.c:hitstop_trigger`.
    fn hitstop_trigger(&mut self, frames: u32) {
        if frames > self.hitstop.freeze_frames {
            self.hitstop.freeze_frames = frames;
        }
    }

    /// `hitstop.c:hitstop_active`.
    pub fn hitstop_active(&self) -> bool {
        self.fx.hitstop && self.hitstop.freeze_frames > 0
    }

    /// `hitstop.c:on_enemy_killed`: 3 frames on a target of value
    /// >= 200, 2 on a hit of 50 or more.
    pub(crate) fn hitstop_on_enemy_killed(&mut self, value: i32, damage: i32) {
        if value >= 200 {
            self.hitstop_trigger(3);
        } else if damage >= 50 {
            self.hitstop_trigger(2);
        }
    }

    /// `hitstop.c:on_boss_defeated` (simulation part): freeze of 3 frames,
    /// then, processorType >= 2 (default value 4, `config.c`), cascade
    /// of five explosions drawn from `rng_range` — draws made even when
    /// hitstop is disabled, like the C code.
    pub(crate) fn hitstop_on_boss_defeated(&mut self, x: i32, y: i32) {
        self.hitstop_trigger(3);
        // hitstop.c:on_boss_defeated — Classic (processorType < 2) returns before
        // the cascade: no rng_range draw, no secondary cascade.
        if self.processor_type < 2 {
            return;
        }
        const DELAYS: [i32; 5] = [4, 14, 26, 42, 60];
        const JITTER: [i32; 5] = [20, 26, 32, 38, 44];
        self.hitstop.secondary.clear();
        for (k, j) in JITTER.into_iter().enumerate() {
            let dx = self.rng.range(-j, j);
            let dy = self.rng.range(-j * 3 / 4, j * 3 / 4);
            self.hitstop.secondary.push((x + dx, y + dy, DELAYS[k]));
        }
    }

    /// tyrian2.c, end of loop: `hitstop_update()` then
    /// `secondary_explosions_update()` (nothing if hitstop is disabled).
    pub(crate) fn hitstop_end_of_loop(&mut self) {
        if self.hitstop.freeze_frames > 0 {
            self.hitstop.freeze_frames -= 1;
        }
        if !self.fx.hitstop {
            return;
        }
        let mut i = 0;
        while i < self.hitstop.secondary.len() {
            self.hitstop.secondary[i].2 -= 1;
            if self.hitstop.secondary[i].2 <= 0 {
                let (x, y, _) = self.hitstop.secondary[i];
                self.setup_explosion(x as i16, y as i16, 0, 1, false, false);
                // "Remove by swapping with last".
                self.hitstop.secondary.swap_remove(i);
                continue;
            }
            i += 1;
        }
    }
}
