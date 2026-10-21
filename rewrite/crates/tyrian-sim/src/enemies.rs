//! Enemies: creation (`enemy_spawn.c:JE_newEnemy`, `JE_makeEnemy`) and
//! update (`enemy_render.c:JE_drawEnemy`, which moves, animates and makes
//! enemies fire in addition to drawing them).

use crate::fmath;
use crate::world::*;

impl World {
    /// `enemy_spawn.c:JE_newEnemy`: first free slot of the block of 25;
    /// returns the slot of the enemy created (the C code returns it 1-based in the
    /// global `b`, 0 = none).
    pub fn new_enemy(&mut self, enemy_offset: usize, e_dat_i: u16, unique_shape: i16) -> Option<usize> {
        for i in enemy_offset..enemy_offset + 25 {
            if self.enemy_avail[i] == 1 {
                let mut e = self.enemy[i];
                let avail = self.make_enemy(&mut e, e_dat_i, unique_shape);
                self.enemy[i] = e;
                self.enemy_avail[i] = avail;
                return if avail == 1 { None } else { Some(i) };
            }
        }
        None
    }

    /// `enemy_spawn.c:JE_makeEnemy`: initializes an enemy from `enemyDat`.
    /// Returns the value of `enemyAvail` (0 active, 2 collectible object).
    pub fn make_enemy(&mut self, enemy: &mut Enemy, e_dat_i: u16, unique_shape: i16) -> u8 {
        // enemy_spawn.c: trail_valid = false (rendering only).
        enemy.trail_prev = None;
        enemy.trail_drawn = None;
        // enemy_spawn.c:JE_makeEnemy: in Super Arcade, object 534 becomes 533.
        let e_dat_i = if self.mode.super_arcade_mode != 0 && e_dat_i == 534 { 533 } else { e_dat_i };
        let d = self.tables.enemy(e_dat_i as usize).clone();
        // Rendering only: a new enemy has not been drawn yet.
        enemy.drawn = None;
        // mutators.c:mutators_block_pickup: "no bonus" blocks values > 20000.
        if self.mode.mutator(crate::modes::MUT_NO_POWERUPS) && d.value as i32 > 20000 {
            return 1;
        }
        let shape_table: u8 = if unique_shape > 0 { unique_shape as u8 } else { d.shapebank };
        // Sprite sheet: if the bank is not loaded, the C code keeps the
        // previous sheet of the slot.
        let mut sheet = None;
        if shape_table == 21 {
            sheet = Some(SHEET_TYRIAN_SHP_11);
        } else if shape_table == 26 {
            sheet = Some(SHEET_TYRIAN_SHP_10);
        } else {
            for &id in self.script.enemy_sprite_sheet_ids.iter() {
                if shape_table == id && id > 0 {
                    sheet = Some(SHAPE_FILE[(id - 1) as usize].to_ascii_lowercase());
                }
            }
        }
        if let Some(s) = sheet {
            enemy.sheet = s;
        }
        enemy.mapoffset = 0;
        enemy.eshotmultipos = [0; 3];
        enemy.enemyground = (d.explosiontype & 1) == 0;
        enemy.explonum = d.explosiontype >> 1;
        enemy.launchfreq = d.elaunchfreq;
        enemy.launchwait = d.elaunchfreq;
        if e_dat_i > 1000 {
            enemy.launchtype = d.elaunchtype;
            enemy.launchspecial = 0;
        } else {
            enemy.launchtype = d.elaunchtype % 1000;
            enemy.launchspecial = (d.elaunchtype / 1000) as u8;
        }
        enemy.xaccel = d.xaccel as u8;
        enemy.yaccel = d.yaccel as u8;
        enemy.xminbounce = -10000;
        enemy.xmaxbounce = 10000;
        enemy.yminbounce = -10000;
        enemy.ymaxbounce = 10000;
        enemy.tur = d.tur;
        enemy.ani = d.ani;
        enemy.animin = 1;
        match d.animate {
            0 => {
                enemy.enemycycle = 1;
                enemy.aniactive = 0;
                enemy.animax = 0;
                enemy.aniwhenfire = 0;
            }
            1 => {
                enemy.enemycycle = 0;
                enemy.aniactive = 1;
                enemy.animax = 0;
                enemy.aniwhenfire = 0;
            }
            2 => {
                enemy.enemycycle = 1;
                enemy.aniactive = 2;
                enemy.animax = enemy.ani;
                enemy.aniwhenfire = 2;
            }
            _ => {}
        }
        // `rng_next() % (startxc * 2)`: the negative int operand becomes a
        // very large unsigned, like in C.
        if d.startxc != 0 {
            let m = (d.startxc as i32 * 2) as u32;
            let r = self.rng.next_u32() % m;
            enemy.ex =
                (d.startx as i32 as u32).wrapping_add(r).wrapping_sub(d.startxc as i32 as u32).wrapping_add(1) as i16;
        } else {
            enemy.ex = d.startx.wrapping_add(1);
        }
        if d.startyc != 0 {
            let m = (d.startyc as i32 * 2) as u32;
            let r = self.rng.next_u32() % m;
            enemy.ey =
                (d.starty as i32 as u32).wrapping_add(r).wrapping_sub(d.startyc as i32 as u32).wrapping_add(1) as i16;
        } else {
            enemy.ey = d.starty.wrapping_add(1);
        }
        enemy.exc = d.xmove;
        enemy.eyc = d.ymove;
        enemy.excc = d.xcaccel;
        enemy.eycc = d.ycaccel;
        enemy.exccw = (enemy.excc as i32).abs() as i8;
        enemy.exccwmax = enemy.exccw as u8;
        enemy.eyccw = (enemy.eycc as i32).abs() as i8;
        enemy.eyccwmax = enemy.eyccw as u8;
        enemy.exccadd = if enemy.excc > 0 { 1 } else { -1 };
        enemy.eyccadd = if enemy.eycc > 0 { 1 } else { -1 };
        enemy.special = false;
        enemy.iced = 0;
        enemy.exrev = match d.xrev {
            0 => 100,
            -99 => 0,
            v => v,
        };
        enemy.eyrev = match d.yrev {
            0 => 100,
            -99 => 0,
            v => v,
        };
        enemy.enemytype = e_dat_i;
        for i in 0..3 {
            enemy.eshotwait[i] = if enemy.tur[i] == 252 {
                1
            } else if enemy.tur[i] > 0 {
                20
            } else {
                255
            };
        }
        enemy.egr = d.egraphic;
        enemy.size = d.esize;
        enemy.linknum = 0;
        enemy.edamaged = d.dani < 0;
        enemy.enemydie = d.eenemydie;
        enemy.freq = d.freq;
        // enemy_spawn.c: "bullet storm" anomaly, fire interval divided by 2.
        if self.mode.anomaly(crate::modes::ANOM_BULLET_STORM) {
            for f in enemy.freq.iter_mut() {
                if *f > 1 {
                    *f /= 2;
                }
            }
        }
        enemy.edani = d.dani;
        enemy.edgr = d.dgr;
        enemy.edlevel = d.dlevel;
        enemy.fixedmovey = 0;
        enemy.filter = 0;
        // Data-driven behavior (behavior.rs): 0 for the original content.
        let (behavior, turret_wait) = self.behavior_init(e_dat_i);
        enemy.behavior = behavior;
        enemy.behavior_seg = 0;
        enemy.behavior_seg_tick = 0;
        enemy.behavior_tick = 0;
        enemy.behavior_turret_wait = turret_wait;

        // Value according to difficulty. The C float factors (0.75f,
        // 1.125f, 1.5f, 2.5f) are exact in binary and the product is
        // truncated: the same integer is obtained by integer division.
        if d.value > 1 && d.value < 10000 {
            let v = d.value as i32;
            let mut tv = match self.difficulty {
                -1 | 0 => v * 3 / 4,
                1 | 2 => v,
                3 => v * 9 / 8,
                4 => v * 3 / 2,
                5 => v * 2,
                6 => v * 5 / 2,
                7 | 8 => v * 4,
                9 | 10 => v * 8,
                _ => 0,
            };
            if tv > 10000 {
                tv = 10000;
            }
            enemy.evalue = tv as i16;
        } else {
            enemy.evalue = d.value;
        }

        let avail;
        if d.armor > 0 {
            let a = d.armor as i32;
            let mut ta;
            if d.armor != 255 {
                // 0.5f and 0.75f are exact (truncation = integer division).
                // 1.2f and 1.8f are not: product rounded to float then
                // truncated, reproduced exactly by fmath (IEEE mantissas of
                // 1.2f = 0x99999A / 2^23 and 1.8f = 0xE66666 / 2^23).
                let mul_f = |m: i128| fmath::round_rational(a as i128 * m, 1 << 23).floorf() as i32;
                ta = match self.difficulty {
                    -1 | 0 => a / 2 + 1,
                    1 => a * 3 / 4 + 1,
                    2 => a,
                    3 => mul_f(0x99999A),
                    4 => a * 3 / 2,
                    5 => mul_f(0xE66666),
                    6 => a * 2,
                    7 => a * 3,
                    8 => a * 4,
                    9 | 10 => a * 8,
                    _ => 1,
                };
                if ta > 254 {
                    ta = 254;
                }
            } else {
                ta = 255;
            }
            enemy.armorleft = ta as u8;
            // enemy_spawn.c:JE_makeEnemy, in C order: "armored" mutator
            // (armor doubled, cap 254), then the "Endless" block (condition on
            // tempArmor), then "hardened" (+50%) and "heavy iron" (+100%) anomalies.
            if self.mode.mutator(crate::modes::MUT_ARMORED) && ta < 255 {
                enemy.armorleft = (enemy.armorleft as i32 * 2).min(254) as u8;
            }
            if let Some(m) = self.endless_armor_mult {
                if ta < 255 {
                    enemy.armorleft = crate::endless::scale_armor(enemy.armorleft, m);
                }
            }
            if self.mode.anomaly(crate::modes::ANOM_HARDENED) && enemy.armorleft < 255 {
                enemy.armorleft = (enemy.armorleft as i32 * 3 / 2).min(254) as u8;
            }
            if self.mode.anomaly(crate::modes::ANOM_HEAVY_IRON) && enemy.armorleft < 255 {
                enemy.armorleft = (enemy.armorleft as i32 * 2).min(254) as u8;
            }
            avail = 0;
            enemy.scoreitem = false;
        } else {
            avail = 2;
            enemy.armorleft = 255;
            if enemy.evalue != 0 {
                enemy.scoreitem = true;
            }
        }
        if !enemy.scoreitem {
            self.total_enemy = self.total_enemy.wrapping_add(1);
        }
        avail
    }

    /// `enemy_render.c:JE_drawEnemy(enemyOffset)` for the slots
    /// `enemyOffset-25 .. enemyOffset-1`. The C code offsets `player[0].x` by -25
    /// during the whole loop (coordinates without the left margin).
    pub fn draw_enemy(&mut self, enemy_offset: usize) {
        self.players[0].x -= 25;
        for i in enemy_offset - 25..enemy_offset {
            if self.enemy_avail[i] != 1 {
                self.update_one_enemy(i, enemy_offset);
            }
        }
        self.players[0].x += 25;
    }

    fn update_one_enemy(&mut self, i: usize, enemy_offset: usize) {
        let tmx = self.background.temp_map_x_ofs as i32;
        self.enemy[i].mapoffset = self.background.temp_map_x_ofs;
        if self.enemy[i].behavior != 0 {
            self.behavior_motion(i);
        }

        // Acceleration towards the player (xaccel/yaccel > 89).
        let px = self.players[0].x;
        let py = self.players[0].y;
        {
            let xa = self.enemy[i].xaccel;
            if xa != 0 && (xa as u32).wrapping_sub(89) > self.rng.next_u32() % 11 {
                let e = &mut self.enemy[i];
                let lim = xa as i32 - 89;
                if px > e.ex as i32 {
                    if (e.exc as i32) < lim {
                        e.exc = e.exc.wrapping_add(1);
                    }
                } else if e.exc >= 0 || -(e.exc as i32) < lim {
                    e.exc = e.exc.wrapping_sub(1);
                }
            }
            let ya = self.enemy[i].yaccel;
            if ya != 0 && (ya as u32).wrapping_sub(89) > self.rng.next_u32() % 11 {
                let e = &mut self.enemy[i];
                let lim = ya as i32 - 89;
                if py > e.ey as i32 {
                    if (e.eyc as i32) < lim {
                        e.eyc = e.eyc.wrapping_add(1);
                    }
                } else if e.eyc >= 0 || -(e.eyc as i32) < lim {
                    e.eyc = e.eyc.wrapping_sub(1);
                }
            }
        }

        // Animation (only if the enemy is within the horizontal window).
        {
            let e = &mut self.enemy[i];
            if e.ex as i32 + tmx > -29 && (e.ex as i32 + tmx) < 300 {
                if e.aniactive == 1 {
                    e.enemycycle = e.enemycycle.wrapping_add(1);
                    if e.enemycycle == e.animax {
                        e.aniactive = e.aniwhenfire;
                    } else if e.enemycycle > e.ani {
                        e.enemycycle = e.animin;
                    }
                }
                // egr[enemycycle-1] == 999: disappearance. enemycycle == 0 reads
                // egr[-1] in C (bytes of `ani`), never equal to 999.
                if e.enemycycle >= 1 && e.enemycycle <= 20 && e.egr[(e.enemycycle - 1) as usize] == 999 {
                    self.enemy_avail[i] = 1;
                    return;
                }
                // Rendering only: what blit_enemy draws (enemy_render.c),
                // before the enemy movement further down in JE_drawEnemy.
                e.drawn = Some((e.ex, e.ey, e.enemycycle, e.filter, e.mapoffset));
                // enemy_render.c:477: trail only at processorType == 4
                // (Wild), then trail_ex/ey = ex/ey, trail_valid = true.
                e.trail_drawn = if self.processor_type == 4 {
                    e.trail_prev.filter(|_| (e.exc as i32).abs() + (e.eyc as i32).abs() > 2)
                } else {
                    None
                };
                e.trail_prev = Some((e.ex, e.ey));
                let (ex, ey, exc, eyc, filter, armorleft, ety) =
                    (e.ex as i32, e.ey as i32, e.exc as i32, e.eyc as i32, e.filter, e.armorleft, e.enemytype);
                if self.fx.events {
                    // enemy_render.c: "Raiden" smoke of a moving enemy at
                    // less than 20% armor (same clock as the blinking).
                    let max_armor = self.tables.enemy(ety as usize).armor as u32;
                    if filter == 0 && (exc != 0 || eyc != 0) && max_armor > 0 {
                        let pct = armorleft as u32 * 100 / max_armor;
                        let now =
                            crate::achievements::clock_ms(self.iteration).wrapping_add(self.render.clock_offset_ms);
                        if pct < 20 && (now / 33).is_multiple_of(12 + pct / 2) {
                            self.fx_event(crate::fx::FxEvent::EnemySmoke { x: ex + tmx + 6, y: ey + 4 });
                        }
                    }
                    // enemy_render.c: particles_spawn_enemy_trail (processorType == 4,
                    // speed > 3).
                    if exc.abs() + eyc.abs() > 3 {
                        self.fx_event(crate::fx::FxEvent::EnemyTrail { x: ex + tmx, y: ey });
                    }
                }
                let e = &mut self.enemy[i];
                let cyc = e.enemycycle as usize;
                let flash = low_armor_flash(
                    e.filter,
                    e.exc,
                    e.eyc,
                    e.armorleft,
                    self.tables.enemy(e.enemytype as usize).armor,
                    crate::achievements::clock_ms(self.iteration).wrapping_add(self.render.clock_offset_ms),
                );
                let drawn = DrawnEnemy {
                    slot: i as u8,
                    ex: e.ex,
                    ey: e.ey,
                    frame: if (1..=20).contains(&cyc) { e.egr[cyc - 1] } else { 0 },
                    filter: flash.unwrap_or(e.filter),
                    mapoffset: e.mapoffset,
                    sheet: e.sheet,
                    size: e.size,
                    trail: e.trail_drawn,
                };
                self.render.drawn_enemies.push(drawn);
                let e = &mut self.enemy[i];
                // The color filter set above is reset to zero after drawing.
                e.filter = 0;
            }
        }

        // enemy_render.c: "Hitstop: skip movement and AI, keep drawing"
        // (goto draw_enemy_end).
        if self.hitstop_active() {
            self.hitstop.frozen_updates += 1;
            return;
        }

        // Cyclic acceleration.
        {
            let e = &mut self.enemy[i];
            if e.excc != 0 {
                e.exccw = e.exccw.wrapping_sub(1);
                if e.exccw <= 0 {
                    if e.exc == e.exrev {
                        e.excc = e.excc.wrapping_neg();
                        e.exrev = e.exrev.wrapping_neg();
                        e.exccadd = -e.exccadd;
                    } else {
                        e.exc = (e.exc as i16).wrapping_add(e.exccadd) as i8;
                        e.exccw = e.exccwmax as i8;
                        if e.exc == e.exrev {
                            e.excc = e.excc.wrapping_neg();
                            e.exrev = e.exrev.wrapping_neg();
                            e.exccadd = -e.exccadd;
                        }
                    }
                }
            }
            if e.eycc != 0 {
                e.eyccw = e.eyccw.wrapping_sub(1);
                if e.eyccw <= 0 {
                    if e.eyc == e.eyrev {
                        e.eycc = e.eycc.wrapping_neg();
                        e.eyrev = e.eyrev.wrapping_neg();
                        e.eyccadd = -e.eyccadd;
                    } else {
                        e.eyc = (e.eyc as i16).wrapping_add(e.eyccadd) as i8;
                        e.eyccw = e.eyccwmax as i8;
                        if e.eyc == e.eyrev {
                            e.eycc = e.eycc.wrapping_neg();
                            e.eyrev = e.eyrev.wrapping_neg();
                            e.eyccadd = -e.eyccadd;
                        }
                    }
                }
            }
            e.ey = e.ey.wrapping_add(e.fixedmovey);
            e.ex = e.ex.wrapping_add(e.exc as i16);
            if e.ex < -80 || e.ex > 340 {
                self.enemy_avail[i] = 1;
                return;
            }
            e.ey = e.ey.wrapping_add(e.eyc as i16);
            if e.ey < -112 || e.ey > 190 {
                self.enemy_avail[i] = 1;
                return;
            }
            if e.ex <= e.xminbounce || e.ex >= e.xmaxbounce {
                e.exc = e.exc.wrapping_neg();
            }
            if e.ey <= e.yminbounce || e.ey >= e.ymaxbounce {
                e.eyc = e.eyc.wrapping_neg();
            }
            if e.scoreitem {
                if e.ex < -5 {
                    e.ex += 1;
                }
                if e.ex > 245 {
                    e.ex -= 1;
                }
            }
            e.ey = e.ey.wrapping_add(self.background.temp_back_move as i16);
            if e.ex <= -24 || e.ex >= 296 {
                return;
            }
        }
        let temp_x = self.enemy[i].ex;
        let temp_y = self.enemy[i].ey;
        if self.enemy[i].edamaged {
            return;
        }
        self.enemy_on_screen = self.enemy_on_screen.wrapping_add(1);
        if self.enemy[i].iced != 0 {
            self.enemy[i].iced -= 1;
            if self.enemy[i].enemyground {
                self.enemy[i].filter = 0x09;
            }
            return;
        }

        // Turrets (j = 3, 2, 1).
        for j in (1..=3usize).rev() {
            if self.enemy[i].freq[j - 1] == 0 {
                continue;
            }
            let temp3 = self.enemy[i].tur[j - 1];
            // enemy_render.c: "bullet hell" mutator, one tick less of waiting.
            if self.mode.mutator(crate::modes::MUT_BULLET_HELL) && self.enemy[i].eshotwait[j - 1] > 1 {
                self.enemy[i].eshotwait[j - 1] -= 1;
            }
            self.enemy[i].eshotwait[j - 1] = self.enemy[i].eshotwait[j - 1].wrapping_sub(1);
            if !(self.enemy[i].eshotwait[j - 1] == 0 && temp3 != 0) {
                continue;
            }
            let mut w = self.enemy[i].freq[j - 1];
            if self.difficulty > DIFFICULTY_NORMAL {
                w = (w / 2) + 1;
                if self.difficulty > DIFFICULTY_MANIACAL {
                    w = (w / 2) + 1;
                }
            }
            self.enemy[i].eshotwait[j - 1] = w;
            // enemy_render.c: "if (galagaMode && (eyc == 0 || rng_next() % 400 >= galagaShotFreq))
            // goto draw_enemy_end".
            if self.mode.galaga_mode
                && (self.enemy[i].eyc == 0 || (self.rng.next_u32() % 400) as u16 >= self.script.galaga_shot_freq)
            {
                return;
            }
            match temp3 {
                252 => {
                    // Double missile of the Savara boss: two explosions.
                    if temp_y > 20 {
                        let y = temp_y as i32 - 20 - self.background.back_move[0] as i32 * 8;
                        self.setup_explosion((temp_x as i32 - 8 + tmx) as i16, y as i16, -2, 6, false, false);
                        self.setup_explosion((temp_x as i32 + 4 + tmx) as i16, y as i16, -2, 6, false, false);
                    }
                }
                251 => {
                    let p = &mut self.players[0];
                    let attraction = (4 - ((p.x - temp_x as i32).abs() + (p.y - temp_y as i32).abs()) / 100) as i16;
                    if attraction > 0 {
                        p.x_velocity += if p.x > temp_x as i32 { -(attraction as i32) } else { attraction as i32 };
                    }
                }
                253 | 254 => {
                    let delta = if temp3 == 253 { 2 } else { -2 };
                    let p = &mut self.players[0];
                    if (p.x + 25 - 14 - temp_x as i32).abs() < 24 && (p.y - temp_y as i32).abs() < 28 {
                        p.x_velocity += delta;
                    }
                    if self.two_player_mode {
                        let p1 = &mut self.players[1];
                        if (p1.x - 14 - temp_x as i32).abs() < 24 && (p1.y - temp_y as i32).abs() < 28 {
                            p1.x_velocity += delta;
                        }
                    }
                }
                255 => {
                    if self.difficulty != DIFFICULTY_EASY {
                        if j == 3 {
                            self.enemy[i].filter = 0x70;
                        } else {
                            let p = &mut self.players[0];
                            let repulsion =
                                (4 - ((p.x - temp_x as i32).abs() + (p.y - temp_y as i32).abs()) / 20) as i16;
                            if repulsion > 0 {
                                p.x_velocity +=
                                    if p.x > temp_x as i32 { repulsion as i32 } else { -(repulsion as i32) };
                            }
                        }
                    }
                }
                _ => {
                    if !self.fire_enemy_turret(i, j, temp3 as usize, temp_x, temp_y, tmx) {
                        return;
                    }
                }
            }
        }
        if self.enemy[i].behavior != 0 && !self.behavior_turrets(i, temp_x, temp_y, tmx) {
            return;
        }

        // Enemy launching.
        if self.enemy[i].launchfreq != 0 {
            self.enemy[i].launchwait = self.enemy[i].launchwait.wrapping_sub(1);
            if self.enemy[i].launchwait == 0 {
                self.enemy[i].launchwait = self.enemy[i].launchfreq;
                if self.enemy[i].launchspecial != 0 && (self.enemy[i].ey as i32 - self.players[0].y).abs() > 5 {
                    return;
                }
                if self.enemy[i].aniactive == 2 {
                    self.enemy[i].aniactive = 1;
                }
                if self.enemy[i].launchtype == 0 {
                    return;
                }
                let temp_w = self.enemy[i].launchtype;
                let off = if enemy_offset == 50 { 75 } else { enemy_offset - 25 };
                if let Some(bi) = self.new_enemy(off, temp_w, 0) {
                    let startyc = self.tables.enemy(self.enemy[bi].enemytype as usize).startyc;
                    let px = self.players[0].x;
                    let py = self.players[0].y;
                    let parent_link = self.enemy[i].linknum;
                    let parent_special = self.enemy[i].launchspecial;
                    {
                        let e = &mut self.enemy[bi];
                        e.ex = temp_x;
                        e.ey = temp_y.wrapping_add(startyc as i16);
                        if e.size == 0 {
                            e.ey -= 7;
                        }
                    }
                    let (lt, lf) = (self.enemy[bi].launchtype, self.enemy[bi].launchfreq);
                    if lt > 0 && lf == 0 {
                        if lt > 90 {
                            let m = ((lt as i32 - 90) * 4) as u32;
                            let r = self.rng.next_u32() % m;
                            let delta = r.wrapping_sub(((lt as i32 - 90) * 2) as u32);
                            let e = &mut self.enemy[bi];
                            e.ex = (e.ex as i32 as u32).wrapping_add(delta) as i16;
                        } else {
                            let mut aim_x = ((px + 25) - temp_x as i32 - tmx - 4) as i16;
                            if aim_x == 0 {
                                aim_x = 1;
                            }
                            let mut aim_y = (py - temp_y as i32) as i16;
                            if aim_y == 0 {
                                aim_y = 1;
                            }
                            let max_mag = (aim_x as i32).abs().max((aim_y as i32).abs()) as i16;
                            let e = &mut self.enemy[bi];
                            e.exc = fmath::round_div_mul(aim_x as i64, max_mag as i64, lt as i64) as i8;
                            e.eyc = fmath::round_div_mul(aim_y as i64, max_mag as i64, lt as i64) as i8;
                        }
                    }
                    // Launch sound: randomly drawn channel (except 3) then sample
                    // (enemy_render.c: randomEnemyLaunchSounds[rng_next() % 3]).
                    let chan = loop {
                        let t = self.rng.next_u32() % 8;
                        if t != 3 {
                            break t as usize;
                        }
                    };
                    let launch = crate::sound::RANDOM_ENEMY_LAUNCH_SOUNDS[(self.rng.next_u32() % 3) as usize];
                    let launch_x = self.enemy[i].ex as i32;
                    self.queue_sound_at(chan, launch, launch_x);
                    if parent_special == 1 && parent_link < 100 {
                        self.enemy[bi].linknum = parent_link;
                    }
                }
            }
        }
    }

    /// Turret shot (`JE_drawEnemy`, `default` case). Returns `false`
    /// when the C code jumps to `draw_enemy_end` (no free shot slot left).
    pub(crate) fn fire_enemy_turret(
        &mut self,
        i: usize,
        j: usize,
        weapon: usize,
        temp_x: i16,
        temp_y: i16,
        tmx: i32,
    ) -> bool {
        let w = self.tables.weapon(weapon).clone();
        let mut count = w.multi as i32;
        while count > 0 {
            let mut b = 0usize;
            while b < self.enemy_shots.len() {
                if self.enemy_shot_avail[b] {
                    break;
                }
                b += 1;
            }
            if b == self.enemy_shots.len() {
                return false;
            }
            self.enemy_shot_avail[b] = false;
            if w.sound > 0 {
                // enemy_render.c: soundQueue[temp] = weapons[temp3].sound, channel drawn excluding 3.
                let chan = loop {
                    let t = self.rng.next_u32() % 8;
                    if t != 3 {
                        break t as usize;
                    }
                };
                let fire_x = self.enemy[i].ex as i32;
                self.queue_sound_at(chan, w.sound, fire_x);
            }
            let e = &mut self.enemy[i];
            if e.aniactive == 2 {
                e.aniactive = 1;
            }
            e.eshotmultipos[j - 1] = e.eshotmultipos[j - 1].wrapping_add(1);
            if e.eshotmultipos[j - 1] > w.max {
                e.eshotmultipos[j - 1] = 1;
            }
            let pos = (e.eshotmultipos[j - 1] as usize).wrapping_sub(1).min(7);
            let s = &mut self.enemy_shots[b];
            s.sx = (temp_x as i32 + w.bx[pos] as i32 + tmx) as i16;
            s.sy = (temp_y as i32 + w.by[pos] as i32) as i16;
            s.sdmg = w.attack[pos];
            s.tx = w.tx;
            s.ty = w.ty;
            s.duration = w.del[pos];
            s.animate = 0;
            s.animax = w.weapani;
            s.sgr = w.sg[pos];
            match j {
                1 => {
                    s.syc = w.acceleration;
                    s.sxc = w.accelerationx;
                    s.sxm = w.sx[pos] as i16;
                    s.sym = w.sy[pos] as i16;
                }
                3 => {
                    s.sxc = w.acceleration.wrapping_neg();
                    s.syc = w.accelerationx;
                    s.sxm = -(w.sy[pos] as i16);
                    s.sym = -(w.sx[pos] as i16);
                }
                _ => {
                    s.sxc = w.acceleration;
                    s.syc = w.acceleration.wrapping_neg();
                    s.sxm = w.sy[pos] as i16;
                    s.sym = -(w.sx[pos] as i16);
                }
            }
            if w.aim > 0 {
                let mut aim = w.aim;
                if self.difficulty > DIFFICULTY_NORMAL {
                    aim = aim.wrapping_add((self.difficulty - 2) as u8);
                }
                let mut target_x = self.players[0].x as u16;
                let mut target_y = self.players[0].y as u16;
                if self.two_player_mode {
                    let t = if self.players[0].is_alive && !self.players[1].is_alive {
                        0
                    } else if self.players[1].is_alive && !self.players[0].is_alive {
                        1
                    } else {
                        self.rng.next_u32() % 2
                    };
                    if t == 1 {
                        target_x = (self.players[1].x - 25) as u16;
                        target_y = self.players[1].y as u16;
                    }
                }
                let mut aim_x = ((target_x as i32 + 25) - temp_x as i32 - tmx - 4) as i16;
                if aim_x == 0 {
                    aim_x = 1;
                }
                let mut aim_y = (target_y as i32 - temp_y as i32) as i16;
                if aim_y == 0 {
                    aim_y = 1;
                }
                let max_mag = (aim_x as i32).abs().max((aim_y as i32).abs()) as i16;
                let s = &mut self.enemy_shots[b];
                s.sxm = fmath::round_div_mul(aim_x as i64, max_mag as i64, aim as i64) as i16;
                s.sym = fmath::round_div_mul(aim_y as i64, max_mag as i64, aim as i64) as i16;
            }
            count -= 1;
        }
        true
    }
}

/// enemy_render.c (Enhanced): a moving enemy under 30% armor blinks
/// white (filter 0xF0) with a period of 60 to 330 ms, based on `SDL_GetTicks()`.
/// Rendering only (the C code resets the filter after drawing); `now_ms`:
/// nominal clock of 28.75 ms per frame like GOLDEN_ACH_CLOCK
/// (achievements::clock_ms), plus `RenderOnly::clock_offset_ms`.
fn low_armor_flash(filter: u8, exc: i8, eyc: i8, armorleft: u8, max_armor: u8, now_ms: u32) -> Option<u8> {
    if filter != 0 || (exc == 0 && eyc == 0) || max_armor == 0 {
        return None;
    }
    let pct = armorleft as u32 * 100 / max_armor as u32;
    if pct >= 30 {
        return None;
    }
    let period = 60 + pct * 9;
    ((now_ms / period) & 1 == 0).then_some(0xF0)
}
