//! Dragonwing linking of two-player games (`twoPlayerLinked`):
//! `player_movement.c:JE_playerMovement` ("Linking Routines", linked
//! movement and rotating gun `linkGunDirec`), `shots.c:player_shot_set_direction`
//! and the armor/credits sharing of `player_collide.c`.
//!
//! `float` computations (gun angle, `sinf`/`cosf`) go through
//! [`crate::f32ops`] (ADR 0003, bit-exact).

use crate::f32ops::{
    add_double, float_diff_lt_double, ge_double, lt_double, DoubleConst, F32, F_0_2, F_0_3, F_MINUS_3_24,
};
use crate::world::*;

/// `varz.c:linkGunWeapons[PORT_NUM]`.
const LINK_GUN_WEAPONS: [u16; 38] = [
    0, 0, 0, 0, 0, 0, 0, 0, 444, 445, 446, 447, 0, 448, 449, 0, 0, 0, 0, 0, 450, 451, 0, 506, 0, 564, 445, 446, 447,
    448, 449, 445, 446, 447, 448, 449, 450, 451,
];
/// `shots.c:linkMultiGr`, `linkSonicGr`, `linkMult2Gr` (16 directions + wrap-around).
const LINK_MULTI_GR: [u16; 17] = [77, 221, 183, 301, 1, 282, 164, 202, 58, 201, 163, 281, 39, 300, 182, 220, 77];
const LINK_SONIC_GR: [u16; 17] = [85, 242, 131, 303, 47, 284, 150, 223, 66, 224, 149, 283, 9, 302, 130, 243, 85];
const LINK_MULT2_GR: [u16; 17] = [78, 299, 295, 297, 2, 278, 276, 280, 59, 279, 275, 277, 40, 296, 294, 298, 78];

impl World {
    /// `player_movement.c`: "Linking Routines" (after `JE_SFCodes`, if `moveOk`).
    /// `mouse_x`/`mouse_y` = player position before the tick's movement.
    pub(crate) fn link_routines(&mut self, pi: usize, player_num: u8) {
        let galaga = self.mode.galaga_mode;
        let (mx, my) = (self.controls.mouse_x as i32, self.controls.mouse_y as i32);
        let (x, y) = (self.players[pi].x, self.players[pi].y);
        if self.two_player_mode
            && !self.mode.two_player_linked
            && x == mx
            && y == my
            && (self.players[0].x - self.players[1].x).abs() < 8
            && (self.players[0].y - self.players[1].y).abs() < 8
            && self.players[0].is_alive
            && self.players[1].is_alive
            && !galaga
        {
            self.mode.two_player_linked = true;
        }
        if player_num == 1 && (self.controls.button[2] || self.controls.button[1]) && !galaga {
            self.mode.two_player_linked = false;
        }
        if self.two_player_mode && self.mode.two_player_linked && player_num == 2 && (x != mx || y != my) {
            if self.controls.button[0] {
                // No analog gamepad (link_gun_analog) in replay.
                let temp_r = if (x - mx).abs() > (y - my).abs() {
                    if x - mx > 0 {
                        DoubleConst::HalfPi
                    } else {
                        DoubleConst::ThreeHalfPi
                    }
                } else if y - my > 0 {
                    DoubleConst::Zero
                } else {
                    DoubleConst::Pi
                }
                .as_f32();
                let mut d = F32(self.mode.link_gun_direc);
                if d.sub(temp_r).abs().lt_bits(F_0_3) {
                    d = temp_r;
                } else if d.lt(temp_r) && d.sub(temp_r).gt_bits(F_MINUS_3_24) {
                    d = d.add(F_0_2);
                } else if float_diff_lt_double(d, temp_r, DoubleConst::Pi) {
                    d = d.sub(F_0_2);
                } else {
                    d = d.add(F_0_2);
                }
                if ge_double(d, DoubleConst::TwoPi) {
                    d = add_double(d, DoubleConst::TwoPi, true);
                } else if lt_double(d, DoubleConst::Zero) {
                    d = add_double(d, DoubleConst::TwoPi, false);
                }
                self.mode.link_gun_direc = d.0;
            } else if !galaga {
                self.mode.two_player_linked = false;
            }
        }
    }

    /// `player_movement.c`: "twoPlayerLinked" branch of player 2 (alive,
    /// not at level end): stuck below player 1, gun markers and linked fire.
    pub(crate) fn linked_player_movement(&mut self, pi: usize, player_num: u8) {
        // shipGr2 == 0 except for an extra ship (varz.c:JE_getShipInfo, ship > 90).
        let ship_gr2_zero = self.players[1].items.ship <= ShipId(90);
        let (p0x, p0y, p0xv) = (self.players[0].x, self.players[0].y, self.players[0].x_velocity);
        {
            let p = &mut self.players[pi];
            p.x = if ship_gr2_zero { p0x - 1 } else { p0x };
            p.y = p0y + 8;
            p.x_velocity = p0xv;
            p.y_velocity = 4;
        }
        let d = F32(self.mode.link_gun_direc);
        let (x, y) = (self.players[pi].x, self.players[pi].y);
        let (mx, my) = (self.controls.mouse_x, self.controls.mouse_y);
        for (a, gr) in [(d.add(F_0_2), 148u16), (d.sub(F_0_2), 148), (d, 147)] {
            self.players[pi].shot_multi_pos[SHOT_MISC] = 0;
            let sx = x + 1 + a.sin().mul_int_roundf(26) as i32;
            let sy = y + a.cos().mul_int_roundf(26) as i32;
            self.player_shot_create(0, SHOT_MISC, sx as u16, sy as u16, mx, my, gr, player_num);
        }
        if self.shot_repeat[SHOT_REAR] > 0 {
            self.shot_repeat[SHOT_REAR] -= 1;
        } else if self.controls.button[0] {
            self.players[pi].shot_multi_pos[SHOT_REAR] = 0;
            let id = self.players[pi].items.weapon[SHOT_REAR].id.index();
            if id > 0 && id <= crate::tables::PORT_NUM {
                let wp = LINK_GUN_WEAPONS.get(id - 1).copied().unwrap_or(0);
                let sx = x + 1 + d.sin().mul_int_roundf(20) as i32;
                let sy = y + d.cos().mul_int_roundf(20) as i32;
                let b = self.player_shot_create(0, SHOT_REAR, sx as u16, sy as u16, mx, my, wp, player_num);
                self.player_shot_set_direction(b, id, d);
            }
        }
    }

    /// `shots.c:player_shot_set_direction`.
    fn player_shot_set_direction(&mut self, b: usize, weapon_id: usize, d: F32) {
        let Some(s) = self.player_shots.get_mut(b) else { return };
        let ym = s.shot_ym as i64;
        s.shot_xm = -(d.sin().mul_int_roundf(ym)) as i16;
        s.shot_ym = -(d.cos().mul_int_roundf(ym)) as i16;
        let dir16 = || crate::f32ops::round_dir16(d).clamp(0, 16) as usize;
        match weapon_id {
            27 | 32 | 10 => s.shot_gr = LINK_MULTI_GR[dir16()],
            28 | 33 | 11 => s.shot_gr = LINK_SONIC_GR[dir16()],
            30 | 35 | 14 => {
                // No float exactly equals M_PI_2 or 3·M_PI_2 (double): >= is the same as >.
                if ge_double(d, DoubleConst::HalfPi) && lt_double(d, DoubleConst::ThreeHalfPi) {
                    s.shot_yc = 1;
                }
            }
            38 | 22 => s.shot_gr = LINK_MULT2_GR[dir16()],
            _ => {}
        }
    }
}
