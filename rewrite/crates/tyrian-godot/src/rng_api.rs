//! `rng.c` generator (xoshiro256**) exposed to the interface screens: the
//! default score tables of `config.c:JE_loadConfiguration` are drawn
//! with `rng_next()` right after `rng_seed(0)` (opentyr.c:main).

use godot::prelude::*;
use tyrian_sim::rng::Xoshiro256;

use crate::vga::TyrianVga;

#[godot_api(secondary)]
impl TyrianVga {
    /// The first `count` `rng_next()` calls after `rng_seed(seed)` (unsigned
    /// 32-bit values, in 64-bit integers).
    #[func]
    fn rng_sequence(seed: i64, count: i32) -> PackedInt64Array {
        let mut rng = Xoshiro256::new(seed as u64);
        (0..count.max(0)).map(|_| rng.next_u32() as i64).collect()
    }
}
