//! Benchmark API exposed to Godot (tools/bench.gd): enemy shot capacity and
//! injection, see `tyrian_sim::bench`. Unused in game.

use godot::prelude::*;

use crate::TyrianSim;

#[godot_api(secondary)]
impl TyrianSim {
    /// Sets the enemy shot capacity to `capacity` slots.
    #[func]
    fn bench_set_enemy_shot_capacity(&mut self, capacity: i32) {
        if let Some(sim) = self.sim.as_mut() {
            sim.bench_set_enemy_shot_capacity(capacity.max(0) as usize);
        }
    }

    /// Fills up to `target` shots in flight; `seed`: state of the benchmark's
    /// generator, returned updated (to be passed back on the next call).
    #[func]
    fn bench_fill_enemy_shots(&mut self, target: i32, seed: i64) -> i64 {
        let mut s = seed as u64;
        if let Some(sim) = self.sim.as_mut() {
            sim.bench_fill_enemy_shots(target.max(0) as usize, &mut s);
        }
        s as i64
    }

    #[func]
    fn bench_enemy_shot_count(&self) -> i32 {
        self.sim.as_ref().map_or(0, |s| s.bench_enemy_shot_count() as i32)
    }
}
