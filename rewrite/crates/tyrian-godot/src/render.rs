//! Filling of the projectile MultiMesh buffers (scripts/shot_batch.gd).
//!
//! With thousands of shots, the GDScript "one instance at a time" loop
//! (set_instance_transform_2d + set_instance_custom_data) cost several
//! milliseconds per frame; here the full buffer is built in Rust and
//! set in a single call (`MultiMesh.buffer`). Read-only on `SimState`.

use godot::prelude::*;
use tyrian_sim::ShotView;

use crate::TyrianSim;

/// Floats per instance: Transform2D (8, see `MultiMesh.buffer`) + custom (4).
const INSTANCE_FLOATS: usize = 12;
/// Floats per frame in the table passed by GDScript:
/// u, v, width, height (normalized), width px, height px, offset x, y.
const FRAME_FLOATS: usize = 8;

fn fill(
    shots: &[ShotView],
    sprite_min: i32,
    sprite_max: i32,
    index_base: i32,
    x_offset: i32,
    frames: &[f32],
) -> PackedFloat32Array {
    let mut out: Vec<f32> = Vec::with_capacity(shots.len() * INSTANCE_FLOATS);
    for s in shots {
        let sprite = s.sprite as i32;
        if sprite < sprite_min || sprite > sprite_max {
            continue;
        }
        let idx = sprite - index_base;
        if idx < 0 {
            continue;
        }
        let Some(f) = frames.get(idx as usize * FRAME_FLOATS..idx as usize * FRAME_FLOATS + FRAME_FLOATS) else {
            continue;
        };
        let (w, h) = (f[4], f[5]);
        if w <= 0.0 {
            continue;
        }
        let x = (s.x + x_offset) as f32 + f[6];
        let y = s.y as f32 + f[7];
        // Transform2D in rows: (x.x, y.x, 0, origin.x, x.y, y.y, 0, origin.y).
        out.extend_from_slice(&[w, 0.0, 0.0, x, 0.0, h, 0.0, y, f[0], f[1], f[2], f[3]]);
    }
    PackedFloat32Array::from(out)
}

#[godot_api(secondary)]
impl TyrianSim {
    /// MultiMesh buffer (12 floats per instance) of the enemy shots
    /// (`enemy` = true) or of the player whose frame is in
    /// `sprite_min..=sprite_max`, frame = sprite - `index_base`, x shifted by
    /// `x_offset`. `frames`: table of `FRAME_FLOATS` floats per frame.
    #[func]
    fn shot_instances(
        &self,
        enemy: bool,
        sprite_min: i32,
        sprite_max: i32,
        index_base: i32,
        x_offset: i32,
        frames: PackedFloat32Array,
    ) -> PackedFloat32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedFloat32Array::new();
        };
        let st = sim.presented_state();
        let list = if enemy { &st.enemy_shots } else { &st.player_shots };
        fill(list, sprite_min, sprite_max, index_base, x_offset, frames.as_slice())
    }

    /// Number of player shots, enemy shots and explosions, without copying
    /// the arrays (sounds and sparks of level.gd).
    #[func]
    fn counts(&self) -> Vector3i {
        let Some(sim) = self.sim.as_ref() else {
            return Vector3i::ZERO;
        };
        let st = sim.presented_state();
        Vector3i::new(st.player_shots.len() as i32, st.enemy_shots.len() as i32, st.explosions.len() as i32)
    }
}
