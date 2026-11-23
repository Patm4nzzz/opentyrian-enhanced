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

#[cfg(test)]
mod hitstop_tests {
    use crate::{
        fx::FxConfig, setup, GeneratorId, Loadout, PlayerInput, ShieldId, ShipId, SidekickId, Sim, SpecialId, WeaponId,
    };

    fn loadout() -> Loadout {
        Loadout {
            ship: ShipId(1),
            front_weapon: WeaponId(1),
            front_power: 1,
            rear_weapon: WeaponId(0),
            rear_power: 1,
            shield: ShieldId(4),
            generator: GeneratorId(2),
            left_sidekick: SidekickId(0),
            right_sidekick: SidekickId(0),
            special: SpecialId(0),
            cash: 10000,
        }
    }

    /// The ship neither fires nor moves: kept alive to watch the level.
    fn idle(sim: &mut Sim, frames: usize) {
        for _ in 0..frames {
            sim.world.players[0].invulnerable_ticks = 100;
            sim.step([PlayerInput(0), PlayerInput(0)]);
        }
    }

    /// DELIANI (episode 1, level file 5), boss fight in place. Three escort ships (links 1 to
    /// 3, value 250) come first; the boss (link 10) arrives at time 4780 with event 36 ("ready
    /// to end the level"); event 4 stops the scrolling at 5015 until the boss layer is empty,
    /// and event 11 (end of level) waits at 5020.
    fn deliani_boss_fight(hitstop: bool) -> Option<Sim> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        if !dir.join("tyrian1.lvl").exists() {
            return None;
        }
        let mut sim = Sim::new(setup::load_level(&dir, 1, 19, 5, 2, loadout(), 7).unwrap());
        sim.set_fx_config(FxConfig { hitstop, ..Default::default() });
        let mut guard = 0;
        while sim.world.script.cur_loc < 5015 && guard < 100_000 {
            idle(&mut sim, 1);
            guard += 1;
        }
        idle(&mut sim, 30);
        assert_eq!(sim.world.script.cur_loc, 5015, "the scrolling must be stopped for the boss fight");
        assert!(sim.world.ending.ready_to_end_level, "event 36 must have armed the level end");
        assert!(!alive(&sim, 10).is_empty(), "the boss (link 10) must be on screen");
        assert!(!sim.world.ending.end_level);
        Some(sim)
    }

    fn alive(sim: &Sim, link: u8) -> Vec<usize> {
        (0..crate::world::ENEMY_MAX)
            .filter(|&i| sim.world.enemy_avail[i] == 0 && sim.world.enemy[i].linknum == link)
            .collect()
    }

    /// Destroys one escort ship as a player shot does (lethal hit on one of its parts).
    fn destroy_escort(sim: &mut Sim, link: u8) {
        let b = alive(sim, link)[0];
        let (x, y) = (sim.world.enemy[b].ex as i32, sim.world.enemy[b].ey as i32);
        sim.world.process_kill_chain(b, x, y, 10, 1, link, Default::default());
        assert!(alive(sim, link).is_empty());
    }

    /// Player report on v1.0-rc1: DELIANI ended with its boss alive when the ships that
    /// escort it were destroyed. The freeze frames of a kill (value 250: 3 frames) hid every
    /// enemy from the on-screen count, which released the stopped scrolling; the level then
    /// ran into its end event with the boss alive.
    #[test]
    fn destroying_escort_does_not_end_level_while_boss_alive() {
        let Some(mut sim) = deliani_boss_fight(true) else { return };
        for link in [1, 2, 3] {
            destroy_escort(&mut sim, link);
            assert!(sim.world.hitstop_active(), "a kill worth 250 freezes the frame");
            for _ in 0..120 {
                idle(&mut sim, 1);
                assert!(!sim.world.ending.end_level, "the level ended with its boss alive");
            }
        }
        assert!(!alive(&sim, 10).is_empty());
        assert_eq!(sim.world.script.cur_loc, 5015, "the scrolling waits for the boss");
    }

    /// A freeze with no repeating explosion in progress (small enemy destroyed by a hit of 50
    /// or more) ended the level at once through event 36, boss alive.
    #[test]
    fn freeze_frame_alone_does_not_end_level_while_boss_alive() {
        let Some(mut sim) = deliani_boss_fight(true) else { return };
        sim.world.hitstop_on_enemy_killed(5, 60);
        assert!(sim.world.hitstop_active());
        for _ in 0..10 {
            idle(&mut sim, 1);
            assert!(!sim.world.ending.end_level, "the level ended with its boss alive");
            assert!(sim.world.enemy_on_screen > 0, "frozen enemies are still on screen");
        }
        assert!(!alive(&sim, 10).is_empty());
    }

    /// Same fight with Freeze Frames off: the level goes on (reference behavior).
    #[test]
    fn level_goes_on_without_freeze_frames() {
        let Some(mut sim) = deliani_boss_fight(false) else { return };
        for link in [1, 2, 3] {
            destroy_escort(&mut sim, link);
            idle(&mut sim, 120);
        }
        assert!(!sim.world.ending.end_level);
        assert_eq!(sim.world.script.cur_loc, 5015);
    }

    /// A freeze must not leave the ground enemies behind their scenery: the backgrounds go on
    /// scrolling during a freeze, so an enemy that stands on the ground has to follow them.
    /// TYRIAN (episode 1, level file 9): same level twice, one freeze in the first run only.
    #[test]
    fn frozen_ground_enemies_keep_following_the_scenery() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        if !dir.join("tyrian1.lvl").exists() {
            return;
        }
        let start = || {
            let mut sim = Sim::new(setup::load_level(&dir, 1, 3, 9, 2, loadout(), 7).unwrap());
            sim.set_fx_config(FxConfig { hitstop: true, ..Default::default() });
            sim
        };
        // Enemies of the two ground layers (slots 25..50 and 75..100) with no motion of their own.
        let still = |sim: &Sim| -> Vec<usize> {
            (25..50)
                .chain(75..100)
                .filter(|&i| {
                    let e = &sim.world.enemy[i];
                    sim.world.enemy_avail[i] == 0
                        && (e.exc, e.eyc, e.fixedmovey, e.excc, e.eycc) == (0, 0, 0, 0, 0)
                        && e.ey > 0
                        && e.ey < 100
                })
                .collect()
        };
        let (mut frozen, mut reference) = (start(), start());
        let mut guard = 0;
        while still(&frozen).len() < 3 && guard < 20_000 {
            idle(&mut frozen, 1);
            idle(&mut reference, 1);
            guard += 1;
        }
        let watched = still(&frozen);
        assert!(watched.len() >= 3, "TYRIAN must show ground enemies");
        let before: Vec<i16> = watched.iter().map(|&i| frozen.world.enemy[i].ey).collect();

        frozen.world.hitstop_on_enemy_killed(250, 1);
        idle(&mut frozen, 6);
        idle(&mut reference, 6);
        assert!(frozen.world.hitstop.frozen_updates > 0, "the enemies must have been frozen");
        assert_eq!(reference.world.hitstop.frozen_updates, 0);
        for (k, &i) in watched.iter().enumerate() {
            assert_ne!(reference.world.enemy[i].ey, before[k], "the scenery must be scrolling");
            assert_eq!(
                frozen.world.enemy[i].ey, reference.world.enemy[i].ey,
                "ground enemy {i} left behind its scenery by a freeze"
            );
        }
    }
}
