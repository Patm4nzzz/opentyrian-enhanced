//! Performance bench: enemy shots in very large numbers.
//!
//! The "bullet hell" mutator of the C version (`enemy_render.c:JE_drawEnemy`,
//! `mut_bullet_hell`: an extra `eshotwait` countdown per tick) is
//! not yet ported ("modes" batch) and would still be capped by `ENEMY_SHOT_MAX`
//! (60, `varz.h`). To measure the cost of thousands of projectiles, this module
//! enlarges the enemy shot array and injects shots into it; it is only used
//! for measurements (docs/performance.md) and for the Godot bench scene.
//!
//! No game rule is added: the injected shots follow exactly
//! `shot_update.c:JE_updateEnemyShots` (movement, leaving the screen, collision
//! with the player), which walks over `enemy_shots.len()` slots.
//! Without a call to this module, the capacity stays 60 and the behavior is
//! identical to the C (golden masters unchanged).

use crate::world::EnemyShot;
use crate::Sim;

/// Local SplitMix64: the bench does not consume the game generators,
/// so as not to disturb the game's sequence of draws.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Sim {
    /// Raises the enemy shot capacity to `capacity` slots (at least
    /// the current capacity; shots in flight are kept).
    pub fn bench_set_enemy_shot_capacity(&mut self, capacity: usize) {
        let w = &mut self.world;
        if capacity > w.enemy_shots.len() {
            w.enemy_shots.resize(capacity, EnemyShot::default());
            w.enemy_shot_avail.resize(capacity, true);
        }
    }

    /// Number of enemy shots in flight.
    pub fn bench_enemy_shot_count(&self) -> usize {
        self.world.enemy_shot_avail.iter().filter(|a| !**a).count()
    }

    /// Fills the free slots up to `target` shots in flight.
    ///
    /// Each shot takes the graphic and lifetime of a real enemy weapon of the
    /// level (first frame `sg[0]` of a `tur` turret of the table's
    /// enemies); position and speed are drawn within the play
    /// area, downwards, with no damage (`sdmg = 0`) so that the ship survives
    /// the bench. Returns the number of shots added.
    pub fn bench_fill_enemy_shots(&mut self, target: usize, seed: &mut u64) -> usize {
        let w = &mut self.world;
        let mut graphics: Vec<(u16, u16)> = Vec::new();
        for e in &w.script.level_enemy {
            let dat = w.tables.enemy(*e as usize);
            for &t in &dat.tur {
                // 252..=255: special shots (enemy_render.c, switch (temp3)).
                if t != 0 && t < 252 {
                    let wp = w.tables.weapon(t as usize);
                    if wp.sg[0] != 0 {
                        graphics.push((wp.sg[0], wp.weapani));
                    }
                }
            }
        }
        if graphics.is_empty() {
            graphics.push((1, 0));
        }
        let mut live = w.enemy_shot_avail.iter().filter(|a| !**a).count();
        let mut added = 0;
        for z in 0..w.enemy_shots.len() {
            if live >= target {
                break;
            }
            if !w.enemy_shot_avail[z] {
                continue;
            }
            let r = next(seed);
            let (sgr, animax) = graphics[(r % graphics.len() as u64) as usize];
            w.enemy_shot_avail[z] = false;
            w.enemy_shots[z] = EnemyShot {
                // Exit bounds of JE_updateEnemyShots: 0 < sx <= 275, -14 < sy <= 190.
                sx: (1 + (r >> 8) % 274) as i16,
                sy: (-13 + ((r >> 20) % 120) as i64) as i16,
                sxm: ((r >> 32) % 5) as i16 - 2,
                sym: 1 + ((r >> 40) % 3) as i16,
                sxc: 0,
                syc: 0,
                tx: 0,
                ty: 0,
                sgr,
                sdmg: 0,
                duration: 255,
                animate: 0,
                animax,
            };
            live += 1;
            added += 1;
        }
        added
    }
}
