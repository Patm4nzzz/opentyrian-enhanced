//! Shop firing demonstration (`shop_preview.c:JE_initWeaponView`,
//! `JE_weaponViewFrame`): the motionless ship at (72, 110) fires with
//! the hovered equipment, in a separate world (shots go through the same
//! `player_shot_create` as the game, then `shots.c:simulate_player_shots`).
//!
//! The C code runs this demonstration on the game globals
//! (`playerShotData`, `power`, `player[0]`); here it has its own
//! [`World`], never that of a level: level simulation does not
//! depend on it. The enemies of the previous level, still in memory in the C code
//! (target of the demonstration's guided shots), are not there: the
//! demonstration aims at nobody.

use crate::world::*;
use crate::LevelSetup;

/// `shop_preview.c:JE_initWeaponView`: ship position.
const SHIP_X: i32 = 72;
const SHIP_Y: i32 = 110;
/// Fixed position of the two sidekicks (`72 ∓ 15`, 120).
const SIDEKICK_Y: i32 = 120;
const SIDEKICK_DX: i32 = 15;

/// A shot drawn by `simulate_player_shots`: corner (x, y) already offset
/// by +1 like `blit_sprite2(VGAScreen, tempShotX+1, …)`, image 0..999
/// (`> 500`: spriteSheet12, `index - 500`; otherwise spriteSheet8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewSprite {
    pub x: i32,
    pub y: i32,
    pub frame: u16,
}

pub struct WeaponPreview {
    world: World,
    /// `backgrnd.c:starfield_stars` (drawn by the caller:
    /// `initialize_starfield` consumes the C global Mersenne Twister).
    pub stars: [StarfieldStar; MAX_STARS],
    /// `shop_preview.c:weaponSimTime` (0..149).
    pub sim_time: u8,
    /// `player[0].weapon_mode`: rear weapon mode (1 or 2), held by
    /// `ShopSession` and shown by the indicator of `JE_weaponViewFrame`.
    pub weapon_mode: u8,
}

impl WeaponPreview {
    /// World of the demonstration: episode tables (`setup.items`), no
    /// enemy, no anomaly.
    pub fn new(setup: &LevelSetup, stars: [StarfieldStar; MAX_STARS]) -> Self {
        let mut p = Self { world: World::start_level(setup), stars, sim_time: 0, weapon_mode: 1 };
        p.init([StarfieldStar::default(); MAX_STARS]);
        p
    }

    /// `JE_initWeaponView` (entering a shop category), except for
    /// the `fill_rectangle_xy`, which is drawing.
    pub fn init(&mut self, stars: [StarfieldStar; MAX_STARS]) {
        self.sim_time = 0;
        let w = &mut self.world;
        let p = &mut w.players[0];
        p.sidekick[0].x = SHIP_X - SIDEKICK_DX;
        p.sidekick[0].y = SIDEKICK_Y;
        p.sidekick[1].x = SHIP_X + SIDEKICK_DX;
        p.sidekick[1].y = SIDEKICK_Y;
        p.x = SHIP_X;
        p.y = SHIP_Y;
        p.delta_x_shot_move = 0;
        p.delta_y_shot_move = 0;
        p.last_x_explosion_follow = SHIP_X;
        p.last_y_explosion_follow = SHIP_Y;
        w.energy.power = 500;
        w.shot_avail.iter_mut().for_each(|a| *a = 0);
        w.shot_repeat = [1; 11];
        self.reset_multi_pos();
        self.stars = stars;
    }

    /// `JE_itemScreen` "JE: {Reset player weapons}": each pass through
    /// the outer loop (each key press) resets `shotMultiPos` to zero.
    pub fn reset_multi_pos(&mut self) {
        for p in self.world.players.iter_mut() {
            p.shot_multi_pos = [0; 11];
        }
    }

    /// `power` (energy bar of the demonstration).
    pub fn power(&self) -> u32 {
        self.world.energy.power
    }

    /// `JE_weaponViewFrame` without the drawing, then the increment of
    /// `weaponSimTime` from `JE_weaponSimUpdate`. `items`: equipment mounted
    /// (hovered item included), `weapon_mode`: `player[0].weapon_mode`
    /// (1 or 2). Returns the shots to draw, in order.
    pub fn frame(&mut self, items: &Items, weapon_mode: u8) -> Vec<PreviewSprite> {
        // update_and_draw_starfield(VGAScreen, 1): advances the positions.
        for s in self.stars.iter_mut() {
            s.position = (s.position as i32 + (s.speed + 1) * 320) as u16;
        }
        let w = &mut self.world;
        w.players[0].items = *items;
        let (x, y) = (SHIP_X as u16, SHIP_Y as u16);
        for i in 0..2 {
            if w.shot_repeat[i] > 0 {
                w.shot_repeat[i] -= 1;
            } else {
                let slot = items.weapon[i];
                let mode = if i == SHOT_REAR { weapon_mode.max(1) as usize - 1 } else { 0 };
                let wp = w.tables.port_shot(slot.id.0 as usize, mode, slot.power.max(1) as usize - 1);
                w.player_shot_create(slot.id.0 as u16, i, x, y, x, y, wp, 1);
            }
        }
        let opt = |w: &World, id: SidekickId| w.tables.options[id.index().min(w.tables.options.len() - 1)].clone();
        let left = opt(w, items.sidekick[0]);
        if left.wport > 0 {
            if w.shot_repeat[SHOT_LEFT_SIDEKICK] > 0 {
                w.shot_repeat[SHOT_LEFT_SIDEKICK] -= 1;
            } else {
                let s = w.players[0].sidekick[0];
                w.player_shot_create(
                    left.wport as u16,
                    SHOT_LEFT_SIDEKICK,
                    s.x as u16,
                    s.y as u16,
                    x,
                    y,
                    left.wpnum,
                    1,
                );
            }
        }
        let right = opt(w, items.sidekick[1]);
        let rs = &mut w.players[0].sidekick[1];
        if right.tr == 2 {
            rs.x = SHIP_X;
            rs.y = (SHIP_Y - 20).max(10);
        } else {
            rs.x = SHIP_X + SIDEKICK_DX;
            rs.y = SIDEKICK_Y;
        }
        if right.wport > 0 {
            if w.shot_repeat[SHOT_RIGHT_SIDEKICK] > 0 {
                w.shot_repeat[SHOT_RIGHT_SIDEKICK] -= 1;
            } else {
                let s = w.players[0].sidekick[1];
                w.player_shot_create(
                    right.wport as u16,
                    SHOT_RIGHT_SIDEKICK,
                    s.x as u16,
                    s.y as u16,
                    x,
                    y,
                    right.wpnum,
                    1,
                );
            }
        }
        let sprites = w.simulate_player_shots();
        // Energy bar: power += powerAdd (generator mounted,
        // varz.c:JE_getShipInfo), capped at 900.
        let power_add = w.tables.generators[items.generator.index().min(w.tables.generators.len() - 1)].power as u32;
        w.energy.power = (w.energy.power + power_add).min(900);
        self.sim_time = (self.sim_time + 1) % 150;
        sprites
    }
}

impl World {
    /// `shots.c:simulate_player_shots`: simplified movement of the
    /// demonstration's shots (no homing, no trail, no collision), exit from the
    /// 0..140 × 0..170 frame, animated image.
    fn simulate_player_shots(&mut self) -> Vec<PreviewSprite> {
        let mut out = Vec::new();
        let dy = self.players[0].delta_y_shot_move;
        for z in 0..MAX_PWEAPON {
            if self.shot_avail[z] == 0 {
                continue;
            }
            self.shot_avail[z] -= 1;
            if z == MAX_PWEAPON - 1 {
                continue;
            }
            let s = &mut self.player_shots[z];
            s.shot_xm = s.shot_xm.wrapping_add(s.shot_xc);
            if s.shot_xm <= 100 {
                s.shot_x = s.shot_x.wrapping_add(s.shot_xm);
            }
            s.shot_ym = s.shot_ym.wrapping_add(s.shot_yc);
            s.shot_y = s.shot_y.wrapping_add(s.shot_ym);
            if s.shot_ym > 100 {
                s.shot_y = (s.shot_y as i32 - 120 + dy) as i16;
            }
            if s.shot_complicated {
                s.shot_dev_x += s.shot_dir_x;
                s.shot_x += s.shot_dev_x;
                if s.shot_dev_x.abs() == s.shot_cir_size_x {
                    s.shot_dir_x = -s.shot_dir_x;
                }
                s.shot_dev_y += s.shot_dir_y;
                s.shot_y += s.shot_dev_y;
                if s.shot_dev_y.abs() == s.shot_cir_size_y {
                    s.shot_dir_y = -s.shot_dir_y;
                }
            }
            if s.shot_x < 0 || s.shot_x > 140 || s.shot_y < 0 || s.shot_y > 170 {
                self.shot_avail[z] = 0;
                continue;
            }
            let mut frame = s.shot_gr.wrapping_add(s.shot_ani);
            s.shot_ani = s.shot_ani.wrapping_add(1);
            if s.shot_ani == s.shot_ani_max {
                s.shot_ani = 0;
            }
            if frame < 60000 {
                if frame > 1000 {
                    frame %= 1000;
                }
                out.push(PreviewSprite { x: s.shot_x as i32 + 1, y: s.shot_y as i32, frame });
            }
        }
        out
    }
}
