//! Explosions: `varz.c:JE_setupExplosion`, `JE_setupExplosionLarge` and
//! `explosion_update.c:JE_updateRepeatingExplosions`, `JE_updateAndDrawExplosions`.
//! The Enhanced effects (particles, "modern" style) are disabled by
//! `golden_master.c:golden_master_level_start` (particles_enabled = false),
//! so the classic path is the only one simulated.

use crate::world::*;

/// `varz.c:JE_setupExplosion`: table `explosion_data[54]` (sprite, ttl), copied
/// entry by entry; the coin values (types 36 to 53) are part of it.
const EXPLOSION_DATA: [(u16, u8); 54] = [
    (144, 7),
    (120, 12),
    (190, 12),
    (209, 12),
    (152, 12),
    (171, 12),
    (133, 7),
    (1, 12),
    (20, 12),
    (39, 12),
    (58, 12),
    (110, 3),
    (76, 7),
    (91, 3),
    (227, 3),
    (230, 3),
    (233, 3),
    (252, 3),
    (246, 3),
    (249, 3),
    (265, 3),
    (268, 3),
    (271, 3),
    (236, 3),
    (239, 3),
    (242, 3),
    (261, 3),
    (274, 3),
    (277, 3),
    (280, 3),
    (299, 3),
    (284, 3),
    (287, 3),
    (290, 3),
    (293, 3),
    (165, 8),
    (184, 8),
    (203, 8),
    (222, 8),
    (168, 8),
    (187, 8),
    (206, 8),
    (225, 10),
    (169, 10),
    (188, 10),
    (207, 20),
    (226, 14),
    (170, 14),
    (189, 14),
    (208, 14),
    (246, 14),
    (227, 14),
    (265, 14),
    (96, 3),
];

impl World {
    /// `varz.c:JE_setupExplosion`.
    pub fn setup_explosion(
        &mut self,
        x: i16,
        y: i16,
        delta_y: i16,
        mut ty: i16,
        fixed_position: bool,
        follow_player: bool,
    ) {
        let mut spawned = false;
        if y > -16 && y < 190 {
            for ex in self.explosions.iter_mut() {
                if ex.ttl == 0 {
                    ex.x = x;
                    ex.y = y;
                    if ty == 6 {
                        ex.y = ex.y.wrapping_add(12);
                        ex.x = ex.x.wrapping_add(2);
                    } else if ty == 98 || ty == 198 {
                        ty = 6;
                    }
                    // Outside the table: the C code would read past the array; we take 0.
                    let (sprite, ttl) = EXPLOSION_DATA.get(ty as usize).copied().unwrap_or((0, 0));
                    ex.sprite = sprite;
                    ex.ttl = ttl;
                    ex.follow_player = follow_player;
                    ex.fixed_position = fixed_position;
                    ex.delta_y = delta_y;
                    // varz.c: main explosions (types 1..10 except 6) ->
                    // particles_spawn_explosion_fx(x, y, false, false).
                    if !fixed_position && (1..=10).contains(&ty) && ty != 6 {
                        spawned = true;
                    }
                    break;
                }
            }
        }
        if spawned {
            self.fx_event(crate::fx::FxEvent::Explosion { x: x as i32, y: y as i32, large: false, ground: false });
        }
    }

    /// `varz.c:JE_setupExplosionLarge`. In Modern style (`fx.modern_explosions`,
    /// never under golden master) the four angle sprites are not placed.
    pub fn setup_explosion_large(&mut self, enemy_ground: bool, mut explo_num: u8, x: i16, y: i16) {
        if y < 0 {
            return;
        }
        self.fx_event(crate::fx::FxEvent::Explosion { x: x as i32, y: y as i32, large: true, ground: enemy_ground });
        if self.fx.modern_explosions {
        } else if enemy_ground {
            self.setup_explosion(x - 6, y - 14, 0, 2, false, false);
            self.setup_explosion(x + 6, y - 14, 0, 4, false, false);
            self.setup_explosion(x - 6, y, 0, 3, false, false);
            self.setup_explosion(x + 6, y, 0, 5, false, false);
        } else {
            self.setup_explosion(x - 6, y - 14, 0, 7, false, false);
            self.setup_explosion(x + 6, y - 14, 0, 9, false, false);
            self.setup_explosion(x - 6, y, 0, 8, false, false);
            self.setup_explosion(x + 6, y, 0, 10, false, false);
        }
        let big = if explo_num > 10 {
            explo_num -= 10;
            true
        } else {
            false
        };
        if explo_num != 0 {
            for r in self.rep_explosions.iter_mut() {
                if r.ttl == 0 {
                    r.ttl = explo_num as u32;
                    r.delay = 2;
                    r.x = x as i32 as u32;
                    r.y = y as i32 as u32;
                    r.big = big;
                    break;
                }
            }
        }
    }

    /// `explosion_update.c:JE_updateRepeatingExplosions`.
    pub fn update_repeating_explosions(&mut self) {
        self.enemy_still_exploding = false;
        for i in 0..MAX_REPEATING_EXPLOSIONS {
            if self.rep_explosions[i].ttl == 0 {
                continue;
            }
            self.enemy_still_exploding = true;
            if self.rep_explosions[i].delay > 0 {
                self.rep_explosions[i].delay -= 1;
                continue;
            }
            let bm2 = self.background.back_move[1] as u32;
            self.rep_explosions[i].y = self.rep_explosions[i].y.wrapping_add(bm2 + 1);
            let r1 = self.rng.next_u32() % 24;
            let temp_x = (self.rep_explosions[i].x.wrapping_add(r1).wrapping_sub(12)) as i16;
            let r2 = self.rng.next_u32() % 27;
            let temp_y = (self.rep_explosions[i].y.wrapping_add(r2).wrapping_sub(24)) as i16;
            if self.rep_explosions[i].big {
                self.setup_explosion_large(false, 2, temp_x, temp_y);
                // Sound choice: `ttl == 1 || rng_next() % 5 == 1` (short-circuit).
                if self.rep_explosions[i].ttl == 1 || self.rng.next_u32() % 5 == 1 {
                    self.queue_sound(7, crate::sound::S_EXPLOSION_11);
                } else {
                    self.queue_sound(6, crate::sound::S_EXPLOSION_9);
                }
                self.rep_explosions[i].delay = 4 + self.rng.next_u32() % 3;
            } else {
                self.setup_explosion(temp_x, temp_y, 0, 1, false, false);
                self.queue_sound(5, crate::sound::S_EXPLOSION_4);
                self.rep_explosions[i].delay = 3;
            }
            self.rep_explosions[i].ttl -= 1;
        }
    }

    /// `explosion_update.c:JE_updateAndDrawExplosions` (movement only).
    pub fn update_explosions(&mut self) {
        let explode_move = self.background.explode_move as i16;
        let (fx, fy) = (self.explosion_follow_amount_x, self.explosion_follow_amount_y);
        self.render.drawn_explosions.clear();
        for ex in self.explosions.iter_mut() {
            if ex.ttl == 0 {
                continue;
            }
            if !ex.fixed_position {
                ex.sprite = ex.sprite.wrapping_add(1);
                ex.y = ex.y.wrapping_add(explode_move);
            } else if ex.follow_player {
                ex.x = ex.x.wrapping_add(fx);
                ex.y = ex.y.wrapping_add(fy);
            }
            ex.y = ex.y.wrapping_add(ex.delta_y);
            if ex.y > 200 - 14 {
                ex.ttl = 0;
            } else {
                // Drawing (blit_sprite2_blend of explosionSpriteSheet, sprite + 1)
                // before the ttl decrement.
                self.render.drawn_explosions.push((ex.x, ex.y, ex.sprite.wrapping_add(1)));
                ex.ttl -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EXPLOSION_DATA;

    /// Table copied from the C code (varz.c:JE_setupExplosion): the coin values
    /// start at index 35 (165), and the five entries 14, 19, 24, 29 and 34
    /// (227, 249, 239, 280, 293) are not missing. A shifted table displayed
    /// the wrong value on collected coins.
    #[test]
    fn explosion_table_matches_c() {
        assert_eq!(EXPLOSION_DATA.len(), 54);
        assert_eq!(EXPLOSION_DATA[14], (227, 3));
        assert_eq!(EXPLOSION_DATA[19], (249, 3));
        assert_eq!(EXPLOSION_DATA[24], (239, 3));
        assert_eq!(EXPLOSION_DATA[29], (280, 3));
        assert_eq!(EXPLOSION_DATA[34], (293, 3));
        assert_eq!(EXPLOSION_DATA[35], (165, 8));
        assert_eq!(EXPLOSION_DATA[45], (207, 20));
        assert_eq!(EXPLOSION_DATA[53], (96, 3));
    }
}
