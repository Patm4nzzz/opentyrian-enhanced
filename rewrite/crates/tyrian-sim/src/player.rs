//! Player: `player_movement.c:JE_playerMovement`, `mainint.c:JE_SFCodes`,
//! `mainint.c:JE_mainGamePlayerFunctions`, `varz.c:JE_doSpecialShot`,
//! `varz.c:JE_specialComplete`, `player_collide.c:JE_playerCollide`,
//! `player.c` (weapon power).
//!
//! Dragonwing linking (`twoPlayerLinked`): `link` module.

use crate::fmath;
use crate::world::*;
use crate::PlayerInput;

/// `varz.c:keyboardCombos[26][8]`.
const KEYBOARD_COMBOS: [[u8; 8]; 26] = [
    [2, 1, 2, 5, 137, 0, 0, 0],
    [4, 3, 2, 5, 138, 0, 0, 0],
    [3, 4, 6, 139, 0, 0, 0, 0],
    [2, 5, 142, 0, 0, 0, 0, 0],
    [6, 2, 6, 143, 0, 0, 0, 0],
    [6, 7, 5, 8, 6, 7, 5, 112],
    [7, 8, 101, 0, 0, 0, 0, 0],
    [1, 7, 6, 146, 0, 0, 0, 0],
    [8, 6, 7, 1, 120, 0, 0, 0],
    [3, 6, 8, 5, 121, 0, 0, 0],
    [1, 2, 7, 8, 119, 0, 0, 0],
    [3, 4, 3, 6, 123, 0, 0, 0],
    [6, 7, 5, 8, 124, 0, 0, 0],
    [1, 6, 125, 0, 0, 0, 0, 0],
    [9, 5, 126, 0, 0, 0, 0, 0],
    [1, 7, 127, 0, 0, 0, 0, 0],
    [1, 8, 128, 0, 0, 0, 0, 0],
    [9, 7, 129, 0, 0, 0, 0, 0],
    [9, 8, 130, 0, 0, 0, 0, 0],
    [4, 2, 3, 5, 131, 0, 0, 0],
    [3, 1, 2, 8, 132, 0, 0, 0],
    [2, 4, 5, 133, 0, 0, 0, 0],
    [3, 4, 2, 8, 134, 0, 0, 0],
    [1, 4, 6, 135, 0, 0, 0, 0],
    [1, 3, 6, 137, 0, 0, 0, 0],
    [1, 4, 3, 4, 7, 136, 0, 0],
];

/// `varz.c:shipCombosB[21]`: Super Tyrian combinations (all the special weapons).
const SHIP_COMBOS_B: [u8; 21] = [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 7, 8, 5, 25, 14, 4, 6, 3, 9, 2, 26];

/// `varz.c:shipCombos[SHIP_NUM + 1][3]`.
const SHIP_COMBOS: [[u8; 3]; 19] = [
    [5, 4, 7],
    [1, 2, 0],
    [14, 4, 0],
    [4, 5, 0],
    [6, 5, 0],
    [7, 8, 0],
    [7, 9, 0],
    [10, 3, 5],
    [5, 8, 9],
    [1, 3, 0],
    [7, 16, 17],
    [2, 11, 12],
    [3, 8, 10],
    [0, 0, 0],
    [1, 0, 0],
    [4, 0, 0],
    [5, 9, 2],
    [0, 0, 0],
    [0, 0, 0],
];

/// `player.c:calc_purple_balls_needed`.
const PURPLE_BALLS_REQUIRED: [u32; 12] = [1, 1, 2, 4, 8, 12, 16, 20, 25, 30, 40, 50];

/// `varz.c:specialArcadeWeapon[PORT_NUM]` (the C code only gives 42 values, the rest is 0).
const SPECIAL_ARCADE_WEAPON: [u8; 42] = [
    17, 17, 18, 0, 0, 0, 10, 0, 0, 0, 0, 0, 44, 0, 10, 0, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 45, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0,
];

/// `varz.c:optionSelect[16][3][2]`.
const OPTION_SELECT: [[[u8; 2]; 3]; 16] = [
    [[0, 0], [0, 0], [0, 0]],
    [[1, 1], [16, 16], [30, 30]],
    [[2, 2], [29, 29], [29, 20]],
    [[3, 3], [21, 21], [12, 0]],
    [[4, 4], [18, 18], [16, 23]],
    [[0, 0], [0, 0], [0, 0]],
    [[6, 6], [29, 16], [0, 22]],
    [[7, 7], [19, 19], [19, 28]],
    [[0, 0], [0, 0], [0, 0]],
    [[0, 0], [0, 0], [0, 0]],
    [[10, 10], [21, 21], [21, 27]],
    [[0, 0], [0, 0], [0, 0]],
    [[0, 0], [0, 0], [0, 0]],
    [[13, 13], [17, 17], [13, 26]],
    [[0, 0], [0, 0], [0, 0]],
    [[15, 15], [15, 16], [15, 16]],
];

impl World {
    /// `player.c:calc_purple_balls_needed`.
    pub fn calc_purple_balls_needed(&mut self, pi: usize) {
        let lives = self.players[pi].lives(pi).min(11) as usize;
        self.players[pi].purple_balls_needed = PURPLE_BALLS_REQUIRED[lives];
    }

    /// `player.c:power_up_weapon`.
    pub fn power_up_weapon(&mut self, pi: usize, port: usize) -> bool {
        let p = &mut self.players[pi];
        let can = p.items.weapon[port].id != WeaponId(0) && p.items.weapon[port].power < 11;
        if can {
            p.items.weapon[port].power += 1;
            p.shot_multi_pos[port] = 0;
            self.calc_purple_balls_needed(pi);
        } else {
            p.cash = p.cash.wrapping_add(1000);
        }
        can
    }

    /// `player.c:handle_got_purple_ball`.
    fn handle_got_purple_ball(&mut self, pi: usize) {
        if self.players[pi].purple_balls_needed > 1 {
            self.players[pi].purple_balls_needed -= 1;
        } else {
            let port = if self.players[pi].is_dragonwing { 1 } else { 0 };
            self.power_up_weapon(pi, port);
        }
    }

    /// `varz.c:JE_drawOptions`: sidekick state recomputed from the table.
    pub fn draw_options(&mut self) {
        self.render.hud_ops.push(crate::world::HudOp::Options);
        let pi = if self.two_player_mode { 1 } else { 0 };
        for i in 0..2 {
            let o =
                self.tables.options[self.players[pi].items.sidekick[i].index().min(crate::tables::OPTION_NUM)].clone();
            let s = &mut self.players[pi].sidekick[i];
            s.ammo = o.ammo as i32;
            s.ammo_max = o.ammo as i32;
            s.ammo_refill_ticks = ((105 - s.ammo) * 4) as u32;
            s.ammo_refill_ticks_max = s.ammo_refill_ticks;
            s.style = o.tr as u32;
            s.animation_enabled = o.option == 1;
            s.animation_frame = 0;
            s.charge = 0;
            s.charge_ticks = 20;
        }
    }

    /// `mainint.c:JE_SFCodes`: recognition of the key combinations
    /// ("twiddles") that trigger a special weapon.
    fn sf_codes(&mut self, player_num: u8, px: i32, py: i32, mouse_x: i32, mouse_y: i32) {
        let mut ship = self.players[(player_num - 1) as usize].items.ship.index();
        if player_num == 2 && ship < 15 {
            ship = 0;
        }
        if ship >= 15 {
            return;
        }
        let up = (mouse_y > py) as u8;
        let down = (mouse_y < py) as u8;
        let left = (px < mouse_x) as u8;
        let right = (px > mouse_x) as u8;
        let mut dir_count = up + down + left + right;
        let mut direction = up + down * 2 + left * 3 + right * 4;
        if direction == 0 {
            if !self.controls.button[0] {
                direction = 9;
                dir_count = 1;
            } else {
                dir_count = 0;
                direction = 99;
            }
        }
        if dir_count != 1 {
            return;
        }
        direction += self.controls.button[0] as u8 * 4;
        let pn = (player_num - 1) as usize;
        // mainint.c:1923-1928: Super Tyrian reads 21 combinations from shipCombosB.
        let combo_max = if self.mode.super_tyrian { 21 } else { 3 };
        for ci in 0..combo_max {
            let combo_id = if self.mode.super_tyrian {
                SHIP_COMBOS_B[ci]
            } else if ship < SHIP_COMBOS.len() {
                SHIP_COMBOS[ship][ci]
            } else {
                0
            };
            if combo_id == 0 {
                self.controls.sf_current_code[pn][ci] = 0;
                continue;
            }
            let cur = self.controls.sf_current_code[pn][ci] as usize;
            let combo = &KEYBOARD_COMBOS[(combo_id - 1) as usize];
            let combo_key = combo[cur.min(7)];
            if combo_key == direction {
                self.controls.sf_current_code[pn][ci] += 1;
                let next = combo[(cur + 1).min(7)];
                if next > 100 && next as usize <= 100 + crate::tables::SPECIAL_NUM {
                    self.controls.sf_current_code[pn][ci] = 0;
                    self.controls.sf_executed[pn] = next - 100;
                }
            } else if direction != 9
                && (combo_key as i32 - 1).rem_euclid(4) != (direction as i32 - 1).rem_euclid(4)
                && (cur == 0 || combo[cur - 1] != direction)
            {
                // `(combo_key - 1) % 4`: int operands, C remainder (sign of the dividend).
                self.controls.sf_current_code[pn][ci] = 0;
            }
        }
    }

    /// `varz.c:JE_specialComplete`.
    fn special_complete(&mut self, player_num: u8, special_type: u8, pwr: u8) {
        self.special_weapon.next_special_wait = 0;
        let sp = self.tables.specials[(special_type as usize).min(crate::tables::SPECIAL_NUM)].clone();
        let pi = (player_num - 1) as usize;
        let front_power = self.players[0].items.weapon[0].power as u16;
        match sp.stype {
            1 => {
                let (x, y) = (self.players[pi].x as u16, self.players[pi].y as u16);
                self.player_shot_create(
                    0,
                    SHOT_SPECIAL2,
                    x,
                    y,
                    self.controls.mouse_x,
                    self.controls.mouse_y,
                    sp.wpn,
                    player_num,
                );
                self.shot_repeat[SHOT_SPECIAL] = self.shot_repeat[SHOT_SPECIAL2];
            }
            2 => {
                let (px, py) = (self.players[0].x, self.players[0].y);
                for t in 0..self.enemy_shots.len() {
                    if !self.enemy_shot_avail[t] {
                        let s = &mut self.enemy_shots[t];
                        if px > s.sx as i32 {
                            s.sxm -= 1;
                        } else if px < s.sx as i32 {
                            s.sxm += 1;
                        }
                        if py > s.sy as i32 {
                            s.sym -= 1;
                        } else if py < s.sy as i32 {
                            s.sym += 1;
                        }
                    }
                }
            }
            3 => {
                self.special_weapon.zinglon_duration = 50;
                self.shot_repeat[SHOT_SPECIAL] = 100;
                let x = self.players[0].x;
                self.queue_sound_at(7, crate::sound::S_SOUL_OF_ZINGLON, x);
            }
            4 => {
                let (px, py) = (self.players[0].x, self.players[0].y);
                for t in 0..100 {
                    let e = &mut self.enemy[t];
                    if self.enemy_avail[t] != 1 && e.scoreitem && e.evalue != 0 {
                        if px > e.ex as i32 {
                            e.exc += 1;
                        } else if px < e.ex as i32 {
                            e.exc -= 1;
                        }
                        if py > e.ey as i32 {
                            e.eyc += 1;
                        } else if py < e.ey as i32 {
                            e.eyc -= 1;
                        }
                    }
                }
            }
            5..=11 | 16 => {
                if self.special_weapon.flare_duration == 0 {
                    self.special_weapon.flare_start = true;
                }
                self.special_weapon.special_weapon_wpn = sp.wpn;
                self.special_weapon.link_to_player = false;
                self.special_weapon.spray_special = false;
                match sp.stype {
                    5 => {
                        self.special_weapon.special_weapon_filter = 7;
                        self.special_weapon.special_weapon_freq = 2;
                        self.special_weapon.flare_duration = 50;
                    }
                    6 => {
                        self.special_weapon.special_weapon_filter = 1;
                        self.special_weapon.special_weapon_freq = 7;
                        self.special_weapon.flare_duration = 200 + 25 * front_power;
                    }
                    7 => {
                        self.special_weapon.special_weapon_filter = 3;
                        self.special_weapon.special_weapon_freq = 3;
                        self.special_weapon.flare_duration = 50 + 10 * front_power;
                        self.special_weapon.zinglon_duration = 50;
                        self.shot_repeat[SHOT_SPECIAL] = 100;
                        self.queue_sound(7, crate::sound::S_SOUL_OF_ZINGLON);
                    }
                    8 => {
                        self.special_weapon.special_weapon_filter = -99;
                        self.special_weapon.special_weapon_freq = 7;
                        self.special_weapon.flare_duration = 10 + front_power;
                    }
                    9 => {
                        self.special_weapon.special_weapon_filter = -99;
                        self.special_weapon.special_weapon_freq = 8;
                        self.special_weapon.flare_duration = 8 + 2 * front_power;
                        self.special_weapon.link_to_player = true;
                        self.special_weapon.next_special_wait = sp.pwr;
                    }
                    10 => {
                        self.special_weapon.special_weapon_filter = -99;
                        self.special_weapon.special_weapon_freq = 8;
                        self.special_weapon.flare_duration = 14 + 4 * front_power;
                        self.special_weapon.link_to_player = true;
                    }
                    11 => {
                        self.special_weapon.special_weapon_filter = -99;
                        self.special_weapon.special_weapon_freq = sp.pwr as i8;
                        self.special_weapon.flare_duration = 10 + 10 * front_power;
                        self.special_weapon.astral_duration = (20 + 10 * front_power) as u8;
                    }
                    _ => {
                        self.special_weapon.special_weapon_filter = -99;
                        self.special_weapon.special_weapon_freq = 8;
                        self.special_weapon.flare_duration = pwr as u16 * 16 + 8;
                        self.special_weapon.link_to_player = true;
                        self.special_weapon.spray_special = true;
                    }
                }
            }
            12 => {
                self.players[pi].invulnerable_ticks = pwr as u32 * 10;
                // varz.c:JE_specialComplete case 12: in Super Arcade (1..=SA), special shot 707.
                if self.mode.super_arcade_mode > 0 && self.mode.super_arcade_mode <= crate::modes::SA {
                    self.shot_repeat[SHOT_SPECIAL] = 250;
                    let (x, y) = (self.players[0].x as u16, self.players[0].y as u16);
                    self.player_shot_create(
                        0,
                        SHOT_SPECIAL2,
                        x,
                        y,
                        self.controls.mouse_x,
                        self.controls.mouse_y,
                        707,
                        1,
                    );
                    self.players[0].invulnerable_ticks = 100;
                }
            }
            13 => {
                self.players[0].armor += (pwr / 4 + 1) as u32;
                let x = self.players[0].x;
                self.queue_sound_at(3, crate::sound::S_POWERUP, x);
            }
            14 => {
                self.players[1].armor += (pwr / 4 + 1) as u32;
                let x = self.players[1].x;
                self.queue_sound_at(3, crate::sound::S_POWERUP, x);
            }
            17 => {
                let x = self.players[0].x;
                self.queue_sound_at(3, crate::sound::S_POWERUP, x);
                let w = sp.wpn as u8;
                if self.players[0].items.sidekick[0] == SidekickId(w) {
                    self.players[0].items.sidekick[1] = SidekickId(w);
                    self.players[0].shot_multi_pos[SHOT_RIGHT_SIDEKICK] = 0;
                } else {
                    self.players[0].items.sidekick[0] = SidekickId(w);
                    self.players[0].shot_multi_pos[SHOT_LEFT_SIDEKICK] = 0;
                }
                self.draw_options();
            }
            18 => {
                self.players[0].items.sidekick[1] = SidekickId(sp.wpn as u8);
                self.draw_options();
                let x = self.players[0].x;
                self.queue_sound_at(4, crate::sound::S_POWERUP, x);
                self.players[0].shot_multi_pos[SHOT_RIGHT_SIDEKICK] = 0;
            }
            _ => {}
        }
    }

    /// `varz.c:JE_doSpecialShot`.
    fn do_special_shot(&mut self, player_num: u8) {
        let pi = (player_num - 1) as usize;
        // Special weapon indicator (rendering only), drawn by the C code before
        // the decrement: 94 ready, 93 recharging.
        if self.players[0].items.special > SpecialId(0) {
            let w = &self.special_weapon;
            let ready = self.shot_repeat[SHOT_SPECIAL] == 0
                && w.special_wait == 0
                && w.flare_duration < 2
                && w.zinglon_duration < 2;
            self.render.drawn_special_icon = if ready { 94 } else { 93 };
        }
        if self.shot_repeat[SHOT_SPECIAL] > 0 {
            self.shot_repeat[SHOT_SPECIAL] -= 1;
        }
        if self.special_weapon.special_wait > 0 {
            self.special_weapon.special_wait -= 1;
        }
        let sf = self.controls.sf_executed[pi];
        let mut pwr: u8 = 0;
        if sf > 0 && self.shot_repeat[SHOT_SPECIAL] == 0 && self.special_weapon.flare_duration == 0 {
            pwr = self.tables.specials[(sf as usize).min(crate::tables::SPECIAL_NUM)].pwr;
            let mut can_afford = true;
            if pwr > 0 {
                let p = &mut self.players[pi];
                if pwr < 98 {
                    if p.shield >= pwr as u32 {
                        p.shield -= pwr as u32;
                    } else {
                        can_afford = false;
                    }
                } else if pwr == 98 {
                    if p.shield < 4 {
                        can_afford = false;
                    }
                    pwr = p.shield as u8;
                    p.shield = 0;
                } else if pwr == 99 {
                    pwr = (p.shield / 2) as u8;
                    p.shield = pwr as u32;
                } else {
                    pwr -= 100;
                    if p.armor > pwr as u32 {
                        p.armor -= pwr as u32;
                    } else {
                        can_afford = false;
                    }
                }
            }
            self.players[pi].shot_multi_pos[SHOT_SPECIAL] = 0;
            self.players[pi].shot_multi_pos[SHOT_SPECIAL2] = 0;
            if can_afford {
                self.special_complete(player_num, sf, pwr);
            }
            self.controls.sf_executed[pi] = 0;
            self.draw_armor_clamp();
            self.hud_bars(true, true, true);
        }
        if player_num == 1 && self.players[0].items.special > SpecialId(0) {
            // varz.c:JE_doSpecialShot: in Super Arcade, the sidekick buttons also trigger the special.
            let sa_side = self.mode.super_arcade_mode != 0 && (self.controls.button[1] || self.controls.button[2]);
            if sa_side {
                self.special_weapon.fire_button_held = false;
            }
            if !self.controls.button[0] && !sa_side {
                self.special_weapon.fire_button_held = false;
            } else if self.shot_repeat[SHOT_SPECIAL] == 0
                && !self.special_weapon.fire_button_held
                && self.special_weapon.flare_duration == 0
                && self.special_weapon.special_wait == 0
            {
                self.special_weapon.fire_button_held = true;
                let s = self.players[0].items.special;
                self.special_complete(player_num, s.0, pwr);
            }
        }
        if self.special_weapon.astral_duration > 0 {
            self.special_weapon.astral_duration -= 1;
        }
        self.shot_avail[MAX_PWEAPON - 1] = 0;
        if self.special_weapon.flare_duration > 1 {
            if self.special_weapon.special_weapon_filter != -99 {
                if self.filter.level_filter == -99 && self.filter.level_brightness == -99 {
                    self.filter.filter_active = false;
                }
                if !self.filter.filter_active {
                    self.filter.level_filter = self.special_weapon.special_weapon_filter;
                    if self.filter.level_filter == 7 {
                        self.filter.level_brightness = 0;
                    }
                    self.filter.filter_active = true;
                }
                self.special_weapon.flare_col_chg = if self.rng.next_u32().is_multiple_of(2) { -1 } else { 1 };
                if self.filter.level_filter == 7 {
                    if self.filter.level_brightness < -6 {
                        self.special_weapon.flare_col_chg = 1;
                    }
                    if self.filter.level_brightness > 6 {
                        self.special_weapon.flare_col_chg = -1;
                    }
                    self.filter.level_brightness += self.special_weapon.flare_col_chg;
                }
            }
            if ((self.rng.next_u32() % 6) as i32) < self.special_weapon.special_weapon_freq as i32 {
                let mut b = MAX_PWEAPON;
                let (wpn, mx, my) =
                    (self.special_weapon.special_weapon_wpn, self.controls.mouse_x, self.controls.mouse_y);
                if self.special_weapon.link_to_player {
                    if self.shot_repeat[SHOT_SPECIAL] == 0 {
                        let (x, y) = (self.players[0].x as u16, self.players[0].y as u16);
                        b = self.player_shot_create(0, SHOT_SPECIAL, x, y, mx, my, wpn, player_num);
                    }
                } else {
                    // varz.c:JE_doSpecialShot: "player_shot_create(..., rng_next() % 280,
                    // rng_next() % 180, ...)". GCC x86-64 evaluates these arguments from right
                    // to left: y is drawn before x (verified by mode-superarcade1).
                    let y = (self.rng.next_u32() % 180) as u16;
                    let x = (self.rng.next_u32() % 280) as u16;
                    b = self.player_shot_create(0, SHOT_SPECIAL, x, y, mx, my, wpn, player_num);
                }
                if self.special_weapon.spray_special && b != MAX_PWEAPON {
                    let xm = (self.rng.next_u32() % 5) as i16 - 2;
                    let ym = (self.rng.next_u32() % 5) as i16 - 2;
                    let s = &mut self.player_shots[b];
                    s.shot_xm = xm;
                    s.shot_ym = ym;
                    if s.shot_ym == 0 {
                        s.shot_ym += 1;
                    }
                }
            }
            self.special_weapon.flare_duration -= 1;
            if self.special_weapon.flare_duration == 1 {
                self.special_weapon.special_wait = self.special_weapon.next_special_wait;
            }
        } else if self.special_weapon.flare_start {
            self.special_weapon.flare_start = false;
            self.shot_repeat[SHOT_SPECIAL] = if self.special_weapon.link_to_player { 15 } else { 200 };
            self.special_weapon.flare_duration = 0;
            if self.filter.level_filter == self.special_weapon.special_weapon_filter {
                self.filter.level_filter = -99;
                self.filter.level_brightness = -99;
                self.filter.filter_active = false;
            }
        }
        if self.special_weapon.zinglon_duration > 1 {
            let t = 25 - (self.special_weapon.zinglon_duration as i32 - 25).abs();
            self.render.drawn_zinglon.push((self.players[0].x + 7, t));
            self.special_weapon.zinglon_duration -= 1;
            if self.special_weapon.zinglon_duration.is_multiple_of(5) {
                self.shot_avail[MAX_PWEAPON - 1] = 1;
            }
        }
    }

    /// `player_movement.c:JE_playerMovement` for one player, in steps:
    /// death or respawn, commands, level end, movement, shots,
    /// sidekick recharge (keyboard; mouse in mouse mode).
    pub fn player_movement(&mut self, pi: usize, input: PlayerInput) {
        let player_num = pi as u8 + 1;
        if player_num == 2 {
            self.render.drawn_charge_sprite = 0;
        }
        if pi == 0 {
            self.render.drawn_special_icon = 0;
            self.render.drawn_zinglon.clear();
        }
        if player_num == 2 || !self.two_player_mode {
            let max_modes = self.tables.ports[self.players[pi].items.weapon[1].id.index().min(crate::tables::PORT_NUM)]
                .opnum as u32;
            if self.players[pi].weapon_mode > max_modes {
                self.players[pi].weapon_mode = 1;
            }
        }
        // C "goto redo": a respawn resumes the movement from the start.
        while self.player_dead_or_respawned(pi) {}
        if !self.players[pi].is_alive {
            return;
        }
        let accel = self.read_player_controls(pi, player_num, input);
        self.level_end_warp(pi);
        self.move_player_ship(pi, player_num, accel);
        if self.controls.move_ok && self.players[pi].is_alive && !self.ending.end_level {
            {
                let p = &mut self.players[pi];
                p.delta_x_shot_move = p.x - p.last_x_shot_move;
                p.delta_y_shot_move = p.y - p.last_y_shot_move;
            }
            self.change_fire_mode(pi);
            // player_movement.c: "if (!galagaMode) JE_doSpecialShot".
            if !self.mode.galaga_mode {
                self.do_special_shot(player_num);
            }
            self.fire_main_weapons(pi, player_num);
            if player_num == 2 {
                self.fire_charge_gun(pi, player_num);
            }
            self.fire_superbomb(pi, player_num);
            self.update_sidekick_positions(pi);
            if player_num == 2 || !self.two_player_mode {
                self.fire_sidekicks(pi, player_num);
            }
        }
        if (player_num == 2 || !self.two_player_mode) && !self.ending.end_level {
            for i in 0..2 {
                let o = self.tables.options[self.players[pi].items.sidekick[i].index().min(crate::tables::OPTION_NUM)]
                    .clone();
                // Rendering only: the image is drawn with the charge from before the recharge.
                self.render.drawn_sidekick_charge[pi][i] = self.players[pi].sidekick[i].charge;
                let s = &mut self.players[pi].sidekick[i];
                if o.option > 0 && s.animation_enabled {
                    s.animation_frame += 1;
                    if s.animation_frame >= o.ani as u32 {
                        s.animation_frame = 0;
                        s.animation_enabled = o.option == 1;
                    }
                }
                s.charge_ticks = s.charge_ticks.wrapping_sub(1);
                if s.charge_ticks == 0 {
                    if s.charge < o.pwr as u32 {
                        s.charge += 1;
                    }
                    s.charge_ticks = 20;
                }
            }
        }
    }

    /// Dead player: explosion, then respawn if lives allow it
    /// (true = respawned, the movement resumes from the start).
    fn player_dead_or_respawned(&mut self, pi: usize) -> bool {
        if !self.players[pi].is_alive {
            if self.players[pi].exploding_ticks > 0 {
                self.players[pi].exploding_ticks -= 1;
                if self.ending.level_end_fx_wait > 0 {
                    self.ending.level_end_fx_wait -= 1;
                } else {
                    self.ending.level_end_fx_wait = (self.rng.next_u32() % 6) as u16 + 3;
                    if self.rng.next_u32() % 3 == 1 {
                        self.queue_sound(6, crate::sound::S_EXPLOSION_9);
                    } else {
                        self.queue_sound(5, crate::sound::S_EXPLOSION_11);
                    }
                }
                let ex = self.players[pi].x + (self.rng.next_u32() % 32) as i32 - 16;
                let ey = self.players[pi].y + (self.rng.next_u32() % 32) as i32 - 16;
                self.setup_explosion_large(false, 0, ex as i16, (ey + 7) as i16);
                let (x, y) = (self.players[pi].x, self.players[pi].y);
                self.setup_explosion_large(false, 0, x as i16, (y + 7) as i16);
                if self.ending.level_end > 0 {
                    self.ending.level_end -= 1;
                }
            } else if self.two_player_mode || self.one_player_action {
                // player_movement.c:JE_playerMovement, respawn of the arcade and two-player
                // modes ("can_respawn = !mut_one_life && *lives > 1" outside Endless).
                // In Endless: `endless.c:endless_try_respawn(p)` (lives in s_lives_p,
                // no respawn with the "one life" mutator or the glass hull).
                let can_respawn = match self.endless_lives.as_mut() {
                    Some(lives) => {
                        if self.mode.mutator(crate::modes::MUT_ONE_LIFE)
                            || self.mode.anomaly(crate::modes::ANOM_GLASS_HULL)
                        {
                            lives[pi] = 0;
                            false
                        } else {
                            crate::endless::try_respawn(&mut lives[pi])
                        }
                    }
                    None => !self.mode.mutator(crate::modes::MUT_ONE_LIFE) && self.players[pi].lives(pi) > 1,
                };
                if can_respawn {
                    if self.endless_lives.is_none() {
                        self.players[pi].items.weapon[pi].power -= 1;
                    }
                    self.ending.really_end_level = false;
                    self.players[pi].shot_multi_pos[pi] = 0;
                    self.calc_purple_balls_needed(pi);
                    self.mode.two_player_linked = false;
                    if self.mode.galaga_mode {
                        self.two_player_mode = false;
                    }
                    let galaga = self.mode.galaga_mode;
                    let ep4 = self.episode == 4;
                    let p = &mut self.players[pi];
                    p.y = 160;
                    p.invulnerable_ticks = 100;
                    p.is_alive = true;
                    p.armor = if galaga || ep4 { p.initial_armor } else { p.initial_armor / 2 };
                    p.shield = if galaga { 0 } else { p.shield_max / 2 };
                    self.ending.end_level = false;
                    self.draw_armor_clamp();
                    self.hud_bars(false, true, true);
                    // "goto redo": resume from the start of the movement.
                    return true;
                } else if self.mode.galaga_mode {
                    self.two_player_mode = false;
                }
            }
        }
        if !self.players[pi].is_alive {
            self.explosion_follow_amount_x = 0;
            self.explosion_follow_amount_y = 0;
        }
        false
    }

    /// Player commands (keyboard, mouse); returns the acceleration (`accelXC`, `accelYC`).
    fn read_player_controls(&mut self, pi: usize, player_num: u8, input: PlayerInput) -> (i32, i32) {
        let (mut accel_xc, mut accel_yc) = (0i32, 0i32);
        if !self.ending.end_level {
            self.controls.mouse_x = self.players[pi].x as u16;
            self.controls.mouse_y = self.players[pi].y as u16;
            self.controls.button = [false; 4];
            // mainint.c:3365-3370 (c-final): in single player too, the ship reads
            // `inputDevice[0]` (0 = all devices). The demo stays on the keyboard
            // only (set_demo).
            let device = self.mode.devices[pi];
            // player_movement.c: "if (play_demo) { if (!replay_demo_keys()) {
            // endLevel = true; levelEnd = 40; } }" before the devices;
            // the keyboard is ignored during the demo, its keys stand in for it.
            let mut input = input;
            if pi == 0 {
                if let Some(d) = self.demo.as_mut() {
                    match d.next_keys() {
                        Some(k) => input = PlayerInput(k),
                        None => {
                            input = PlayerInput(0);
                            self.ending.end_level = true;
                            self.ending.level_end = 40;
                        }
                    }
                }
            }
            // player_movement.c: mouse if inputDevice is 0 or 2 (has_mouse true in v2).
            let (mut mouse_xc, mut mouse_yc) = (0i32, 0i32);
            if device == 0 || device == 2 {
                let m = self.mode.mouse;
                self.controls.button[0] |= m.mask & 0x10 != 0;
                self.controls.button[1] |= m.mask & 0x40 != 0;
                self.controls.button[2] |= m.mask & 0x80 != 0;
                self.controls.button[3] |= m.mask & 0x20 != 0;
                if self.mode.mouse_pending {
                    mouse_xc += m.dx;
                    mouse_yc += m.dy;
                    self.mode.mouse_pending = false;
                }
            }
            if device == 0 || device == 1 {
                // player_movement.c, gamepad in analog mode:
                // "mouseXC += joystick_axis_reduce(j, joystick[j].x)" (same for y).
                // The rewrite binds the gamepad to the keyboard device (1).
                let (ax, ay) = self.mode.analog[pi];
                mouse_xc += ax;
                mouse_yc += ay;
                {
                    let p = &mut self.players[pi];
                    if input.has(PlayerInput::UP) {
                        p.y -= 1;
                    }
                    if input.has(PlayerInput::DOWN) {
                        p.y += 1;
                    }
                    if input.has(PlayerInput::LEFT) {
                        p.x -= 1;
                    }
                    if input.has(PlayerInput::RIGHT) {
                        p.x += 1;
                    }
                }
                self.controls.button[0] = self.controls.button[0] || input.has(PlayerInput::FIRE);
                self.controls.button[3] = self.controls.button[3] || input.has(PlayerInput::CHANGE_FIRE);
                self.controls.button[1] = self.controls.button[1] || input.has(PlayerInput::LEFT_SIDEKICK);
                self.controls.button[2] = self.controls.button[2] || input.has(PlayerInput::RIGHT_SIDEKICK);
                // player_movement.c: "if (constantPlay) { button[0..3] = true;
                // ++this_player->y; this_player->x += constantLastX; }"
                // (dev_console.c, Ctrl+Shift+G; false outside --devmode).
                if self.dev.constant_play {
                    self.controls.button = [true; 4];
                    let p = &mut self.players[pi];
                    p.y += 1;
                    p.x += self.dev.constant_last_x;
                }
            }
            accel_xc += self.players[pi].x - self.controls.mouse_x as i32;
            accel_yc += self.players[pi].y - self.controls.mouse_y as i32;
            // mouseXC/mouseYC (int16_t) clamped to ±30, then movement rounded outwards.
            mouse_xc = mouse_xc as i16 as i32;
            mouse_yc = mouse_yc as i16 as i32;
            mouse_xc = mouse_xc.clamp(-30, 30);
            mouse_yc = mouse_yc.clamp(-30, 30);
            {
                let p = &mut self.players[pi];
                if mouse_xc > 0 {
                    p.x += (mouse_xc + 3) / 4;
                } else if mouse_xc < 0 {
                    p.x += (mouse_xc - 3) / 4;
                }
                if mouse_yc > 0 {
                    p.y += (mouse_yc + 3) / 4;
                } else if mouse_yc < 0 {
                    p.y += (mouse_yc - 3) / 4;
                }
            }
            if mouse_xc > 3 {
                accel_xc += 1;
            } else if mouse_xc < -2 {
                accel_xc -= 1;
            }
            if mouse_yc > 2 {
                accel_yc += 1;
            } else if mouse_yc < -2 {
                accel_yc -= 1;
            }
            self.controls.move_ok = true;
            let (px, py, mx, my) =
                (self.players[pi].x, self.players[pi].y, self.controls.mouse_x as i32, self.controls.mouse_y as i32);
            self.sf_codes(player_num, px, py, mx, my);
            if self.controls.move_ok {
                self.link_routines(pi, player_num);
            }
        }
        (accel_xc, accel_yc)
    }

    /// Level end: the ship flies away and the level ends.
    fn level_end_warp(&mut self, pi: usize) {
        if self.ending.level_end > 0 && self.all_players_dead() {
            self.ending.really_end_level = true;
        }
        if self.players[pi].is_alive && self.ending.end_level {
            if self.ending.level_end == 0 {
                self.ending.really_end_level = true;
            } else {
                self.players[pi].y -= self.ending.level_end_warp as i32;
                if self.players[pi].y < -200 {
                    self.ending.really_end_level = true;
                }
            }
        }
    }

    /// Ship movement: inertia, bounds, banking, Nort sparks.
    fn move_player_ship(&mut self, pi: usize, player_num: u8, (accel_xc, accel_yc): (i32, i32)) {
        let mx = self.controls.mouse_x as i32;
        let my = self.controls.mouse_y as i32;
        if self.players[pi].is_alive && !self.ending.end_level {
            // player_movement.c: "if (!twoPlayerLinked || playerNum_ < 2)", otherwise the link module.
            if !(self.mode.two_player_linked && player_num == 2) {
                let p = &mut self.players[pi];
                if p.sidekick[0].style == 0 {
                    p.sidekick[0].x = mx - 14;
                    p.sidekick[0].y = my;
                }
                if p.sidekick[1].style == 0 {
                    p.sidekick[1].x = mx + 16;
                    p.sidekick[1].y = my;
                }
                if p.x_friction_ticks > 0 {
                    p.x_friction_ticks -= 1;
                } else {
                    p.x_friction_ticks = 1;
                    p.x_velocity -= p.x_velocity.signum();
                }
                if p.y_friction_ticks > 0 {
                    p.y_friction_ticks -= 1;
                } else {
                    p.y_friction_ticks = 2;
                    p.y_velocity -= p.y_velocity.signum();
                }
                // accelXC/accelYC are int16_t.
                p.x_velocity += accel_xc as i16 as i32;
                p.y_velocity += accel_yc as i16 as i32;
                p.x_velocity = p.x_velocity.clamp(-4, 4);
                p.y_velocity = p.y_velocity.clamp(-4, 4);
                p.x += p.x_velocity;
                p.y += p.y_velocity;
                if p.x - mx != 0 || p.y - my != 0 {
                    for i in 1..20 {
                        p.old_x[i - 1] = p.old_x[i];
                        p.old_y[i - 1] = p.old_y[i];
                    }
                    p.old_x[19] = p.x;
                    p.old_y[19] = p.y;
                }
            } else {
                self.linked_player_movement(pi, player_num);
            }
            let p = &mut self.players[pi];
            // player_movement.c: play area bounds (outside level end);
            // the constantPlay drift bounces off the edges.
            if p.x > 256 {
                p.x = 256;
                self.dev.constant_last_x = -self.dev.constant_last_x;
            }
            if p.x < 40 {
                p.x = 40;
                self.dev.constant_last_x = -self.dev.constant_last_x;
            }
            p.y = p.y.clamp(10, 160);
            let ship_banking = (p.x_velocity / 2 + (p.x - mx) / 6).clamp(-2, 2);
            p.banking = ship_banking;
            self.explosion_follow_amount_x = (p.x - p.last_x_explosion_follow) as i16;
            self.explosion_follow_amount_y = (p.y - p.last_y_explosion_follow) as i16;
            if self.explosion_follow_amount_y < 0 {
                self.explosion_follow_amount_y = 0;
            }
            p.last_x_explosion_follow = p.x;
            p.last_y_explosion_follow = p.y;
            // Rendering only: blended ship and aura of this tick.
            self.render.drawn_invulnerable[pi] = p.invulnerable_ticks > 0;
            if p.invulnerable_ticks > 0 {
                p.invulnerable_ticks -= 1;
            } else if self.ship_gr == 1 {
                // Nort ship: sparks (player_movement.c, ship_sprite = banking*2 + 1).
                let ship_sprite = ship_banking * 2 + 1;
                let (spark_x, bank) = match ship_sprite {
                    5 => (p.x - 7, -2i32),
                    3 => (p.x - 7, -1),
                    -1 => (p.x + 9, 1),
                    -3 => (p.x + 9, 2),
                    _ => (0, 0),
                };
                if bank != 0 {
                    if self.shot_repeat[SHOT_NORTSPARKS] > 0 {
                        self.shot_repeat[SHOT_NORTSPARKS] -= 1;
                    } else {
                        let y = self.players[pi].y;
                        // Arguments evaluated from right to left by GCC x86-64:
                        // the y offset is drawn before the x offset (cf. JE_doSpecialShot).
                        let sy = (y + (self.rng.next_u32() % 8) as i32 - 4) as u16;
                        let sx = (spark_x + (self.rng.next_u32() % 8) as i32 - 4) as u16;
                        self.player_shot_create(
                            0,
                            SHOT_NORTSPARKS,
                            sx,
                            sy,
                            self.controls.mouse_x,
                            self.controls.mouse_y,
                            671,
                            1,
                        );
                        self.shot_repeat[SHOT_NORTSPARKS] = bank.unsigned_abs() as u8 - 1;
                    }
                }
            }
        }
    }

    /// Fire mode change key: rear weapon mode (or Super Arcade special).
    fn change_fire_mode(&mut self, pi: usize) {
        if self.controls.button[3] {
            self.controls.port_config_change = true;
            if self.controls.port_config_done {
                self.players[pi].shot_multi_pos[SHOT_REAR] = 0;
                let sam = self.mode.super_arcade_mode;
                if sam != 0 && sam <= crate::modes::SA {
                    // player_movement.c: in Super Arcade, the fire mode change alternates
                    // SASpecialWeapon / SASpecialWeaponB.
                    let p = &mut self.players[pi];
                    p.shot_multi_pos[SHOT_SPECIAL] = 0;
                    p.shot_multi_pos[SHOT_SPECIAL2] = 0;
                    let i = (sam - 1) as usize;
                    if self.players[0].items.special == SpecialId(crate::modes::SA_SPECIAL_WEAPON[i]) {
                        self.players[0].items.special = SpecialId(crate::modes::SA_SPECIAL_WEAPON_B[i]);
                        self.players[pi].weapon_mode = 2;
                    } else {
                        self.players[0].items.special = SpecialId(crate::modes::SA_SPECIAL_WEAPON[i]);
                        self.players[pi].weapon_mode = 1;
                    }
                    self.controls.port_config_done = false;
                } else {
                    let opnum = self.tables.ports[self.players[if self.two_player_mode { 1 } else { 0 }].items.weapon
                        [1]
                    .id
                    .index()
                    .min(crate::tables::PORT_NUM)]
                    .opnum as u32;
                    self.players[pi].weapon_mode += 1;
                    if self.players[pi].weapon_mode > opnum {
                        self.players[pi].weapon_mode = 1;
                    }
                    self.controls.port_config_done = false;
                }
            }
        }
    }

    /// Front and rear weapons.
    fn fire_main_weapons(&mut self, pi: usize, player_num: u8) {
        let (min, max) = if !self.two_player_mode { (1, 2) } else { (player_num as usize, player_num as usize) };
        // player_movement.c: "if (!(twoPlayerLinked && playerNum_ == 2))" normal weapons.
        let skip_main = self.mode.two_player_linked && player_num == 2;
        for slot in (min - 1)..max {
            if skip_main {
                break;
            }
            let item = self.players[pi].items.weapon[slot].id.0 as u16;
            if item == 0 {
                continue;
            }
            if self.shot_repeat[slot] > 0 {
                self.shot_repeat[slot] -= 1;
            } else if self.controls.button[0] {
                let power = self.players[pi].items.weapon[slot].power as usize;
                let item_power = if self.mode.galaga_mode { 0 } else { power.wrapping_sub(1) };
                let item_mode = if slot == 1 { self.players[pi].weapon_mode as usize - 1 } else { 0 };
                let wp = self.tables.port_shot(item as usize, item_mode, item_power);
                let (x, y) = (self.players[pi].x as u16, self.players[pi].y as u16);
                self.player_shot_create(item, slot, x, y, self.controls.mouse_x, self.controls.mouse_y, wp, player_num);
            }
        }
    }

    /// `player_movement.c` "Super Charge Weapons": charged cannon of player 2.
    fn fire_charge_gun(&mut self, pi: usize, player_num: u8) {
        let rear_power = self.players[pi].items.weapon[1].power;
        // Rendering only: charge icon (spriteSheet10, 77 + level
        // + chargeGr * 19) outside linked mode, then chargeGr animation.
        let level_in = self.mode.charge_level;
        if !self.mode.two_player_linked {
            self.render.drawn_charge_sprite = 77 + level_in as u32 + self.render.charge_gr as u32 * 19;
        }
        if self.render.charge_gr_wait > 0 {
            self.render.charge_gr_wait -= 1;
        } else {
            self.render.charge_gr = (self.render.charge_gr + 1) % 4;
            self.render.charge_gr_wait = 3;
        }
        if level_in > 0 {
            self.render.hud_ops.push(crate::world::HudOp::ChargeCell { level: level_in, color: 193 });
        }
        if self.mode.charge_wait > 0 {
            self.mode.charge_wait -= 1;
        } else {
            if self.mode.charge_level < self.mode.charge_max {
                self.mode.charge_level += 1;
            }
            self.mode.charge_wait = 28u8.wrapping_sub(rear_power.wrapping_mul(2));
            if self.difficulty > DIFFICULTY_HARD {
                self.mode.charge_wait = self.mode.charge_wait.wrapping_sub(5);
            }
        }
        if self.mode.charge_level > 0 {
            self.render.hud_ops.push(crate::world::HudOp::ChargeCell { level: self.mode.charge_level, color: 204 });
        }
        if self.shot_repeat[SHOT_P2_CHARGE] > 0 {
            self.shot_repeat[SHOT_P2_CHARGE] -= 1;
        } else if self.controls.button[0] && (!self.mode.two_player_linked || self.mode.charge_level > 0) {
            self.players[pi].shot_multi_pos[SHOT_P2_CHARGE] = 0;
            let rear = self.players[1].items.weapon[1].id.index();
            if rear > 0 && rear <= crate::tables::PORT_NUM {
                // C array of PORT_NUM entries, the following ones are 0.
                let wp = crate::modes::CHARGE_GUN_WEAPONS.get(rear - 1).copied().unwrap_or(0)
                    + self.mode.charge_level as u16;
                let (x, y) = (self.players[pi].x as u16, self.players[pi].y as u16);
                self.player_shot_create(
                    16,
                    SHOT_P2_CHARGE,
                    x,
                    y,
                    self.controls.mouse_x,
                    self.controls.mouse_y,
                    wp,
                    player_num,
                );
            }
            if self.mode.charge_level > 0 {
                self.render.hud_ops.push(crate::world::HudOp::ChargeCell { level: self.mode.charge_level, color: 193 });
            }
            self.mode.charge_level = 0;
            self.mode.charge_wait = 30u8.wrapping_sub(rear_power.wrapping_mul(2));
        }
    }

    /// Player's superbomb.
    fn fire_superbomb(&mut self, pi: usize, player_num: u8) {
        let bomb = pi;
        if self.players[bomb].superbombs > 0 {
            let k = SHOT_P1_SUPERBOMB + bomb;
            if self.shot_repeat[k] > 0 {
                self.shot_repeat[k] -= 1;
            } else if self.controls.button[2] || self.controls.button[1] {
                self.players[bomb].superbombs -= 1;
                self.players[bomb].shot_multi_pos[k] = 0;
                let (x, y) = (self.players[pi].x as u16, self.players[pi].y as u16);
                self.player_shot_create(16, k, x, y, self.controls.mouse_x, self.controls.mouse_y, 535, player_num);
            }
        }
    }

    /// Sidekick position (`player_movement.c:JE_playerMovement`, "sidekicks"
    /// block). Style 4 (orbit): `sinf/cosf(optionSatelliteRotate)`
    /// reproduced bit for bit by `f32ops` (glibc 2.39 FMA).
    fn update_sidekick_positions(&mut self, pi: usize) {
        use crate::f32ops::{F32, F_0_2};
        // `optionSatelliteRotate += 0.2f` (two orbits) or `+= 0.15f` (one).
        const F_0_15: F32 = F32(0x3E19_999A);
        let (l4, r4) = (self.players[pi].sidekick[0].style == 4, self.players[pi].sidekick[1].style == 4);
        if l4 && r4 {
            self.sidekick_formation.option_satellite_rotate =
                self.sidekick_formation.option_satellite_rotate.add(F_0_2);
        } else if l4 || r4 {
            self.sidekick_formation.option_satellite_rotate =
                self.sidekick_formation.option_satellite_rotate.add(F_0_15);
        }
        // `roundf(sinf(a) * 20)`: product in float then roundf; the addition to
        // the integer x is exact (small values).
        let rot = self.sidekick_formation.option_satellite_rotate;
        let (ox, oy) =
            if l4 || r4 { (rot.sin().mul_int_roundf(20) as i32, rot.cos().mul_int_roundf(20) as i32) } else { (0, 0) };
        let p = &mut self.players[pi];
        match p.sidekick[0].style {
            1 | 3 => {
                p.sidekick[0].x = p.old_x[9];
                p.sidekick[0].y = p.old_y[9];
            }
            2 => {
                p.sidekick[0].x = p.x;
                p.sidekick[0].y = (p.y - 20).max(10);
            }
            4 => {
                p.sidekick[0].x = p.x + ox;
                p.sidekick[0].y = p.y + oy;
            }
            _ => {}
        }
        match p.sidekick[1].style {
            4 => {
                p.sidekick[1].x = p.x - ox;
                p.sidekick[1].y = p.y - oy;
            }
            1 | 3 => {
                p.sidekick[1].x = p.old_x[0];
                p.sidekick[1].y = p.old_y[0];
            }
            2 => {
                if self.sidekick_formation.option_attachment_linked {
                    p.sidekick[1].x = p.x;
                    p.sidekick[1].y = p.y - 20;
                    if self.controls.button[2] {
                        self.sidekick_formation.option_attachment_linked = false;
                        self.sidekick_formation.option_attachment_return = false;
                        self.sidekick_formation.option_attachment_move = -20;
                        self.sound.sample[3] = crate::sound::S_WEAPON_26;
                    }
                } else {
                    p.sidekick[1].y += (self.sidekick_formation.option_attachment_move / 2) as i32;
                    if self.sidekick_formation.option_attachment_move >= -2 {
                        let ret = self.sidekick_formation.option_attachment_return as i16;
                        let mut move_speed: i32 = if self.sidekick_formation.option_attachment_return { 2 } else { 0 };
                        let sy = p.sidekick[1].y;
                        if sy > (p.y - 20) + 5 {
                            move_speed = 2;
                            self.sidekick_formation.option_attachment_move -= 1 + ret;
                        } else if sy > (p.y - 20) {
                            move_speed = 3;
                            if self.sidekick_formation.option_attachment_move > 0 {
                                self.sidekick_formation.option_attachment_move -= 1;
                            } else {
                                self.sidekick_formation.option_attachment_move += 1;
                            }
                        } else if sy > (p.y - 20) - 5 {
                            move_speed = 2;
                            self.sidekick_formation.option_attachment_move += 1;
                        } else if self.sidekick_formation.option_attachment_move < 2 + ret * 4 {
                            self.sidekick_formation.option_attachment_move += 1 + ret;
                        }
                        if self.sidekick_formation.option_attachment_return {
                            move_speed *= 2;
                        }
                        if (p.sidekick[1].x - p.x).abs() < move_speed {
                            move_speed = 1;
                        }
                        if p.sidekick[1].x > p.x {
                            p.sidekick[1].x -= move_speed;
                        } else if p.sidekick[1].x < p.x {
                            p.sidekick[1].x += move_speed;
                        }
                        if (p.sidekick[1].y - (p.y - 20)).abs() + (p.sidekick[1].x - p.x).abs() < 8 {
                            self.sidekick_formation.option_attachment_linked = true;
                            self.sound.sample[2] = crate::sound::S_CLINK;
                        }
                        if self.controls.button[2] {
                            self.sidekick_formation.option_attachment_return = true;
                        }
                    } else {
                        self.sidekick_formation.option_attachment_move +=
                            1 + self.sidekick_formation.option_attachment_return as i16;
                        let (x, y) = (p.sidekick[1].x, p.sidekick[1].y);
                        self.setup_explosion((x + 1) as i16, (y + 10) as i16, 0, 0, false, false);
                    }
                }
                let p = &mut self.players[pi];
                if p.sidekick[1].y < 10 {
                    p.sidekick[1].y = 10;
                }
            }
            _ => {}
        }
    }

    /// Sidekick fire (`player_movement.c`, end of `JE_playerMovement`).
    fn fire_sidekicks(&mut self, pi: usize, player_num: u8) {
        for i in 0..2 {
            let shot_i = if i == 0 { SHOT_LEFT_SIDEKICK } else { SHOT_RIGHT_SIDEKICK };
            let o =
                self.tables.options[self.players[pi].items.sidekick[i].index().min(crate::tables::OPTION_NUM)].clone();
            if o.wport == 0 {
                continue;
            }
            if self.shot_repeat[shot_i] > 0 {
                self.shot_repeat[shot_i] -= 1;
                continue;
            }
            let ammo_max = self.players[pi].sidekick[i].ammo_max;
            let fire = if ammo_max > 0 {
                let s = &mut self.players[pi].sidekick[i];
                if s.ammo_refill_ticks > 0 {
                    s.ammo_refill_ticks -= 1;
                } else {
                    s.ammo_refill_ticks = s.ammo_refill_ticks_max;
                    if s.ammo < ammo_max {
                        s.ammo += 1;
                    }
                }
                self.controls.button[1 + i] && s.ammo > 0
            } else {
                (self.controls.button[0] && o.pwr == 0) || self.controls.button[1 + i]
            };
            if fire {
                let s = self.players[pi].sidekick[i];
                let wp = (o.wpnum as u32 + s.charge) as u16;
                self.player_shot_create(
                    o.wport as u16,
                    shot_i,
                    s.x as u16,
                    s.y as u16,
                    self.controls.mouse_x,
                    self.controls.mouse_y,
                    wp,
                    player_num,
                );
                let s = &mut self.players[pi].sidekick[i];
                if ammo_max > 0 {
                    s.ammo -= 1;
                }
                if s.charge > 0 {
                    self.players[pi].shot_multi_pos[shot_i] = 0;
                    self.players[pi].sidekick[i].charge = 0;
                }
                let s = &mut self.players[pi].sidekick[i];
                s.charge_ticks = 20;
                s.animation_enabled = true;
            }
        }
    }

    /// `mainint.c:JE_mainGamePlayerFunctions`.
    pub fn main_game_player_functions(&mut self, inputs: [PlayerInput; 2]) {
        if self.ending.end_level && self.ending.level_end > 0 {
            self.ending.level_end -= 1;
            self.ending.level_end_warp = self.ending.level_end_warp.wrapping_add(1);
        }
        self.controls.sf_executed = [0; 2];
        self.controls.port_config_change = false;
        if self.two_player_mode {
            self.player_movement(0, inputs[0]);
            self.player_movement(1, inputs[1]);
        } else {
            self.player_movement(0, inputs[0]);
        }
        let temp_x: u16 = if self.two_player_mode {
            ((self.players[0].x + self.players[1].x) / 2) as u16
        } else {
            self.players[0].x as u16
        };
        // floorf((float)(260 - (tempX - 36)) / (260 - 36) * (24 * 3) - 1)
        let a = 260 - (temp_x as i64 - 36);
        let scroll_ofs = fmath::floor_div_mul_minus1(a, 224, 72) as u16;
        self.background.map_x3_ofs = scroll_ofs;
        self.background.map_x3_pos = self.background.map_x3_ofs % 24;
        self.background.map_x3_bp_pos = 1 - (self.background.map_x3_ofs / 24) as i32;
        self.background.map_x2_ofs = ((scroll_ofs as u32 * 2) / 3) as u16;
        self.background.map_x2_pos = self.background.map_x2_ofs % 24;
        self.background.map_x2_bp_pos = 1 - (self.background.map_x2_ofs / 24) as i32;
        self.background.old_map_x_ofs = self.background.map_x_ofs;
        self.background.map_x_ofs = self.background.map_x2_ofs / 2;
        self.background.map_x_pos = self.background.map_x_ofs % 24;
        self.background.map_x_bp_pos = 1 - (self.background.map_x_ofs / 24) as i32;
        if self.background.background3x1 {
            self.background.map_x3_ofs = self.background.map_x_ofs;
            self.background.map_x3_pos = self.background.map_x_pos;
            self.background.map_x3_bp_pos = self.background.map_x_bp_pos - 1;
        }
    }

    /// `player_collide.c:JE_playerCollide`.
    pub fn player_collide(&mut self, pi: usize) {
        let player_num = pi as u8 + 1;
        for z in 0..ENEMY_MAX {
            if self.enemy_avail[z] == 1 {
                continue;
            }
            let e = self.enemy[z];
            let enemy_screen_x = e.ex as i32 + e.mapoffset as i32;
            let (px, py) = (self.players[pi].x, self.players[pi].y);
            if !((px - enemy_screen_x).abs() < 12 && (py - e.ey as i32).abs() < 14) {
                continue;
            }
            let evalue = e.evalue as i32;
            if evalue > 29999 {
                if evalue == 30000 {
                    self.players[pi].cash += 100;
                    if !self.mode.galaga_mode {
                        self.handle_got_purple_ball(pi);
                    } else {
                        // player_collide.c:75-92: in Galaga the purple ball makes the Dragonwing
                        // appear, linked to player 1 (rear weapon power = lives).
                        if self.two_player_mode {
                            self.players[pi].cash += 2400;
                        }
                        self.two_player_mode = true;
                        self.mode.two_player_linked = true;
                        self.players[1].items.weapon[1].power = 1;
                        self.players[1].armor = 10;
                        self.players[1].is_alive = true;
                    }
                    self.enemy_avail[z] = 1;
                    self.queue_sound(7, crate::sound::S_POWERUP);
                } else if self.mode.super_arcade_mode != 0 && evalue > 30000 {
                    // player_collide.c: Super Arcade weapon bonus.
                    self.players[pi].shot_multi_pos[SHOT_FRONT] = 0;
                    self.shot_repeat[SHOT_FRONT] = 10;
                    let idx = evalue - 30000 - 1;
                    let slot = if (0..=4).contains(&idx) { idx as usize } else { 0 };
                    let sa_weapon = crate::modes::SA_WEAPON[(self.mode.super_arcade_mode - 1) as usize][slot];
                    if sa_weapon == self.players[0].items.weapon[0].id.0 {
                        self.players[pi].cash += 1000;
                        self.power_up_weapon(pi, 0);
                    } else {
                        self.handle_got_purple_ball(pi);
                    }
                    self.players[0].items.weapon[0].id = WeaponId(sa_weapon);
                    self.players[pi].cash += 200;
                    self.queue_sound(7, crate::sound::S_POWERUP);
                    self.enemy_avail[z] = 1;
                } else if evalue > 32100 {
                    if player_num == 1 {
                        let sp = (evalue - 32100).clamp(0, crate::tables::SPECIAL_NUM as i32);
                        let sp = if evalue - 32100 > crate::tables::SPECIAL_NUM as i32 { 0 } else { sp };
                        self.players[pi].cash += 250;
                        self.players[0].items.special = SpecialId(sp as u8);
                        self.players[0].shot_multi_pos[SHOT_SPECIAL] = 0;
                        self.shot_repeat[SHOT_SPECIAL] = 10;
                        self.players[0].shot_multi_pos[SHOT_SPECIAL2] = 0;
                        self.shot_repeat[SHOT_SPECIAL2] = 0;
                        self.queue_sound(7, crate::sound::S_POWERUP);
                        self.enemy_avail[z] = 1;
                    }
                } else if evalue > 32000 {
                    let opt = evalue - 32000;
                    let opt = if opt > crate::tables::OPTION_NUM as i32 { 0 } else { opt } as u8;
                    if player_num == 2 {
                        self.enemy_avail[z] = 1;
                        let p1 = &mut self.players[1];
                        if opt != p1.items.sidekick_series {
                            p1.items.sidekick_series = opt;
                            p1.items.sidekick_level = 101;
                        } else if p1.items.sidekick_level < 103 {
                            p1.items.sidekick_level += 1;
                        }
                        let lvl = (p1.items.sidekick_level - 101) as usize;
                        let series = p1.items.sidekick_series;
                        for (i, slot) in p1.items.sidekick.iter_mut().enumerate().take(2) {
                            *slot = if (series as usize) < OPTION_SELECT.len() {
                                SidekickId(OPTION_SELECT[series as usize][lvl.min(2)][i])
                            } else {
                                SidekickId(series)
                            };
                        }
                        p1.shot_multi_pos[SHOT_LEFT_SIDEKICK] = 0;
                        p1.shot_multi_pos[SHOT_RIGHT_SIDEKICK] = 0;
                        self.draw_options();
                        self.queue_sound(7, crate::sound::S_POWERUP);
                    } else if self.one_player_action {
                        self.enemy_avail[z] = 1;
                        self.players[0].items.sidekick = [SidekickId(opt), SidekickId(opt)];
                        self.players[0].shot_multi_pos[SHOT_LEFT_SIDEKICK] = 0;
                        self.players[0].shot_multi_pos[SHOT_RIGHT_SIDEKICK] = 0;
                        self.draw_options();
                        self.queue_sound(7, crate::sound::S_POWERUP);
                    }
                    if self.enemy_avail[z] == 1 {
                        self.players[pi].cash += 250;
                    }
                } else if evalue > 31000 {
                    let port = evalue - 31000;
                    let port = if port > crate::tables::PORT_NUM as i32 { 0 } else { port } as u8;
                    self.players[pi].cash += 250;
                    if player_num == 2 {
                        self.players[1].items.weapon[1].id = WeaponId(port);
                        self.players[1].shot_multi_pos[SHOT_REAR] = 0;
                        self.enemy_avail[z] = 1;
                        self.queue_sound(7, crate::sound::S_POWERUP);
                    } else if self.one_player_action {
                        self.players[0].items.weapon[1].id = WeaponId(port);
                        self.players[0].shot_multi_pos[SHOT_REAR] = 0;
                        self.enemy_avail[z] = 1;
                        self.queue_sound(7, crate::sound::S_POWERUP);
                        if self.players[0].items.weapon[1].power == 0 {
                            self.players[0].items.weapon[1].power = 1;
                        }
                    }
                } else if evalue > 30000 {
                    let port = evalue - 30000;
                    let port = if port > crate::tables::PORT_NUM as i32 { 0 } else { port } as u8;
                    if (player_num == 1 && self.two_player_mode) || self.one_player_action {
                        self.players[0].items.weapon[0].id = WeaponId(port);
                        self.players[0].shot_multi_pos[SHOT_FRONT] = 0;
                        self.enemy_avail[z] = 1;
                        self.queue_sound(7, crate::sound::S_POWERUP);
                    }
                    if self.enemy_avail[z] == 1 {
                        let idx = evalue - 30000 - 1;
                        let idx = if idx < 0 || idx > crate::tables::PORT_NUM as i32 - 1 { 0 } else { idx } as usize;
                        self.players[0].items.special = SpecialId(SPECIAL_ARCADE_WEAPON.get(idx).copied().unwrap_or(0));
                        if self.players[0].items.special > SpecialId(0) {
                            self.players[0].shot_multi_pos[SHOT_SPECIAL] = 0;
                            self.shot_repeat[SHOT_SPECIAL] = 0;
                            self.players[0].shot_multi_pos[SHOT_SPECIAL2] = 0;
                            self.shot_repeat[SHOT_SPECIAL2] = 0;
                        }
                        self.players[pi].cash += 250;
                    }
                }
            } else if evalue > 20000 {
                if self.mode.two_player_linked {
                    // player_collide.c: armor shared between linked players (integer division).
                    for p in self.players.iter_mut() {
                        p.armor += ((evalue - 20000) / 2) as u32;
                        if p.armor > 28 {
                            p.armor = 28;
                        }
                    }
                } else {
                    let p = &mut self.players[pi];
                    p.armor += (evalue - 20000) as u32;
                    if p.armor > 28 {
                        p.armor = 28;
                    }
                }
                self.enemy_avail[z] = 1;
                self.draw_armor_clamp();
                self.hud_bars(false, false, true);
                self.queue_sound(7, crate::sound::S_POWERUP);
            } else if evalue > 10000 && self.enemy_avail[z] == 2 {
                if !self.ending.bonus_level {
                    self.queue_sound(7, crate::sound::S_POWERUP);
                    self.ending.bonus_level = true;
                    self.ending.next_level = (evalue - 10000) as u8;
                    self.enemy_avail[z] = 1;
                    self.ending.display_time = 150;
                    // EVENT_POWERUP_COLLECTED (hitstop.c:on_powerup_collected: white flash).
                    self.fx_event(crate::fx::FxEvent::SecretWarp);
                }
            } else if e.scoreitem {
                self.enemy_avail[z] = 1;
                self.queue_sound(7, crate::sound::S_ITEM);
                match evalue {
                    1 => {
                        self.ending.cube_max = self.ending.cube_max.wrapping_add(1);
                        self.queue_sound(3, crate::sound::V_DATA_CUBE);
                    }
                    -1 => {
                        self.power_up_weapon(0, 0);
                        self.queue_sound(7, crate::sound::S_POWERUP);
                    }
                    -2 => {
                        let target = if self.two_player_mode { 1 } else { 0 };
                        self.power_up_weapon(target, 1);
                        self.queue_sound(7, crate::sound::S_POWERUP);
                    }
                    -3 => {
                        self.players[pi].shot_multi_pos[SHOT_MISC] = 0;
                        let (x, y) = (self.players[pi].x as u16, self.players[pi].y as u16);
                        self.player_shot_create(
                            0,
                            SHOT_MISC,
                            x,
                            y,
                            self.controls.mouse_x,
                            self.controls.mouse_y,
                            104,
                            player_num,
                        );
                    }
                    -4 => {
                        if self.players[pi].superbombs < 10 {
                            self.players[pi].superbombs += 1;
                        }
                    }
                    -5 => {
                        self.players[0].items.weapon[0].id = WeaponId(25);
                        self.players[0].items.weapon[1].id = WeaponId(26);
                        self.players[1].items.weapon[1].id = WeaponId(26);
                        for p in self.players.iter_mut() {
                            p.weapon_mode = 1;
                            p.shot_multi_pos = [0; 11];
                        }
                    }
                    _ if self.mode.two_player_linked => {
                        // player_collide.c: credits shared between linked players (evalue / COUNTOF: size_t).
                        let share = (evalue as i64 as u64) / 2;
                        for p in self.players.iter_mut() {
                            p.cash = p.cash.wrapping_add(share);
                        }
                    }
                    _ => {
                        let p = &mut self.players[pi];
                        p.cash = p.cash.wrapping_add(evalue as i64 as u64);
                    }
                }
                let ty = self.tables.enemy(e.enemytype as usize).explosiontype as i16;
                self.setup_explosion(enemy_screen_x as i16, e.ey, 0, ty, true, false);
            } else if self.players[pi].invulnerable_ticks == 0
                && self.enemy_avail[z] == 0
                && (self.tables.enemy(e.enemytype as usize).explosiontype & 1) == 0
            {
                let mut armorleft = e.armorleft as i32;
                if armorleft > self.script.damage_rate as i32 {
                    armorleft = self.script.damage_rate as i32;
                }
                self.player_damage(armorleft as u8, pi);
                let e = self.enemy[z];
                if e.armorleft > 0 {
                    let p = &mut self.players[pi];
                    p.x_velocity += (e.exc as i32 * e.armorleft as i32) / 2;
                    p.y_velocity += (e.eyc as i32 * e.armorleft as i32) / 2;
                }
                let mut armorleft2 = e.armorleft as i32;
                if armorleft2 == 255 {
                    armorleft2 = 30000;
                }
                let link = if e.linknum == 0 { 255 } else { e.linknum };
                if armorleft2 > armorleft {
                    if e.armorleft != 255 {
                        self.enemy[z].armorleft -= armorleft as u8;
                    }
                    self.queue_sound(5, crate::sound::S_ENEMY_HIT);
                } else {
                    for ei in 0..ENEMY_MAX {
                        if self.enemy_avail[ei] == 1 {
                            continue;
                        }
                        let other = self.enemy[ei].linknum as i32;
                        let l = link as i32;
                        if ei == z
                            || (link != 255
                                && (l == other
                                    || l - 100 == other
                                    || (other > 40 && other / 20 == l / 20 && other <= l)))
                        {
                            let o = self.enemy[ei];
                            let sx = o.ex.wrapping_add(o.mapoffset as i16);
                            self.enemy[ei].linknum = 0;
                            self.enemy_avail[ei] = 1;
                            if self.tables.enemy(o.enemytype as usize).esize == 1 {
                                self.setup_explosion_large(o.enemyground, o.explonum, sx, o.ey);
                                self.queue_sound(6, crate::sound::S_EXPLOSION_9);
                            } else {
                                self.setup_explosion(sx, o.ey, 0, 1, false, false);
                                self.queue_sound(5, crate::sound::S_EXPLOSION_4);
                            }
                        }
                    }
                    self.enemy_avail[z] = 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod galaga_purple_ball_tests {
    use crate::{
        setup::load_level, GeneratorId, Loadout, PlayerInput, ShieldId, ShipId, SidekickId, Sim, SpecialId, WeaponId,
    };
    use std::path::PathBuf;

    fn sim() -> Option<Sim> {
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        if !data.join("tyrian1.lvl").exists() {
            eprintln!("data/ missing: test skipped");
            return None;
        }
        let loadout = Loadout {
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
            cash: 0,
        };
        let mut s = load_level(&data, 1, 2, 9, 2, loadout, 7).unwrap();
        s.modes.galaga = true;
        s.modes.one_player_action = true;
        Some(Sim::new(s))
    }

    /// Places a purple ball (value 30000) on player 1, then advances one tick.
    fn drop_ball(sim: &mut Sim) {
        for _ in 0..50 {
            sim.step([PlayerInput(0), PlayerInput(0)]);
        }
        let (x, y) = (sim.world.players[0].x, sim.world.players[0].y);
        let z = sim.world.enemy_avail.iter().position(|&a| a == 1).unwrap();
        sim.world.enemy_avail[z] = 0;
        sim.world.enemy[z].ex = x as i16;
        sim.world.enemy[z].ey = y as i16;
        sim.world.enemy[z].mapoffset = 0;
        sim.world.enemy[z].evalue = 30000u16 as _;
        sim.world.player_collide(0);
        assert_eq!(sim.world.enemy_avail[z], 1, "the orb must be picked up");
    }

    #[test]
    fn galaga_purple_ball_spawns_linked_dragonwing() {
        let Some(mut sim) = sim() else { return };
        assert!(!sim.world.two_player_mode);
        let cash = sim.world.players[0].cash;
        drop_ball(&mut sim);
        let w = &sim.world;
        assert!(w.two_player_mode, "twoPlayerMode must become true");
        assert!(w.mode.two_player_linked, "twoPlayerLinked must become true");
        assert!(w.players[1].is_alive);
        assert_eq!(w.players[1].armor, 10);
        assert_eq!(w.players[1].items.weapon[1].power, 1);
        assert_eq!(w.players[0].cash, cash + 100, "+100 only the first time");
    }

    #[test]
    fn galaga_second_purple_ball_gives_2500() {
        let Some(mut sim) = sim() else { return };
        drop_ball(&mut sim);
        let cash = sim.world.players[0].cash;
        let (x, y) = (sim.world.players[0].x, sim.world.players[0].y);
        let z = sim.world.enemy_avail.iter().position(|&a| a == 1).unwrap();
        sim.world.enemy_avail[z] = 0;
        sim.world.enemy[z].ex = x as i16;
        sim.world.enemy[z].ey = y as i16;
        sim.world.enemy[z].mapoffset = 0;
        sim.world.enemy[z].evalue = 30000u16 as _;
        sim.world.player_collide(0);
        assert_eq!(sim.world.players[0].cash, cash + 2500);
    }
}
