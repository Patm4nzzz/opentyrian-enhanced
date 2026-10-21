//! Shots: creation (`shots.c:player_shot_create`), movement and collisions
//! of player shots (`shots.c:player_shot_move_and_draw`,
//! `shot_update.c:JE_updatePlayerShots`), enemy shots
//! (`shot_update.c:JE_updateEnemyShots`) and damage to the player
//! (`varz.c:JE_playerDamage`).

use crate::tables::{PORT_NUM, WEAP_NUM};
use crate::world::*;

/// Moved shot: (x, y, damage, filter, chain, player, special, x radius, y radius).
type MovedShot = (i32, i32, i16, u8, u8, u8, bool, u16, u16);

impl World {
    /// `varz.c:JE_doSP`: "superpixel" particles. They are only
    /// drawn, but each consumes three `rng_next_float` (one
    /// xoshiro draw each) that must be reproduced to keep the random stream.
    /// The raw draws are kept for rendering (`World::superpixels`).
    pub fn do_sp(&mut self, x: u16, y: u16, num: u16, explowidth: u8, color: u8) {
        for _ in 0..num {
            let rnd = [self.rng.next_float_bits(), self.rng.next_float_bits(), self.rng.next_float_bits()];
            self.render.last_superpixel += 1;
            if self.render.last_superpixel >= MAX_SUPERPIXELS {
                self.render.last_superpixel = 0;
            }
            self.render.superpixels[self.render.last_superpixel] =
                Superpixel { cx: x, cy: y, explowidth, color, rnd, z: 15, drawn_z: 0, moves: 0 };
        }
    }

    /// `varz.c:JE_drawSP` (counters only; positions and pixels: rendering).
    pub fn draw_sp_tick(&mut self) {
        for sp in self.render.superpixels.iter_mut() {
            if sp.z > 0 {
                sp.moves += 1;
                sp.drawn_z = sp.z;
                sp.z -= 1;
            } else {
                sp.drawn_z = 0;
            }
        }
    }

    /// `shots.c:player_shot_create`. Returns the index of the last shot created or
    /// MAX_PWEAPON.
    #[allow(clippy::too_many_arguments)]
    pub fn player_shot_create(
        &mut self,
        port_num: u16,
        bay_i: usize,
        px: u16,
        py: u16,
        mouse_x: u16,
        mouse_y: u16,
        wp_num: u16,
        player_num: u8,
    ) -> usize {
        if port_num as usize > PORT_NUM || wp_num == 0 || wp_num as usize > WEAP_NUM {
            return MAX_PWEAPON;
        }
        let weapon = self.tables.weapon(wp_num as usize).clone();
        let mut poweruse = self.tables.ports[(port_num as usize).min(crate::tables::PORT_NUM)].poweruse as u32;
        // shots.c: "twin cannons" anomaly, doubled front shot and doubled consumption.
        let twin = self.mode.anomaly(crate::modes::ANOM_TWIN_CANNONS) && bay_i == SHOT_FRONT && weapon.multi > 0;
        if twin {
            poweruse *= 2;
        }
        if self.energy.power < poweruse {
            return MAX_PWEAPON;
        }
        self.energy.power -= poweruse;
        // shots.c: soundQueue[soundChannel[bay_i]] = weapon->sound; soundQueueX = PX.
        if weapon.sound > 0 {
            self.queue_sound_at(crate::sound::SHOT_SOUND_CHANNEL[bay_i.min(10)], weapon.sound, px as i32);
        }
        let pi = (player_num - 1) as usize;
        let mut shot_id = MAX_PWEAPON;
        let effective_multi = if twin { weapon.multi as i32 * 2 } else { weapon.multi as i32 };
        for _multi_i in 1..=effective_multi {
            shot_id = 0;
            while shot_id < MAX_PWEAPON && self.shot_avail[shot_id] != 0 {
                shot_id += 1;
            }
            if shot_id == MAX_PWEAPON {
                return MAX_PWEAPON;
            }
            {
                let mp = &mut self.players[pi].shot_multi_pos[bay_i];
                if *mp == weapon.max || *mp >= 8 {
                    *mp = 1;
                } else {
                    *mp += 1;
                }
            }
            let mp = self.players[pi].shot_multi_pos[bay_i] as usize;
            let k = mp.wrapping_sub(1).min(7);
            let dx = self.players[pi].delta_x_shot_move;
            let dy = self.players[pi].delta_y_shot_move;
            let s = &mut self.player_shots[shot_id];
            s.chain_reaction = 0;
            s.player_number = player_num;
            s.shot_bay = bay_i as u8;
            s.shot_ani = 0;
            s.shot_complicated = weapon.circlesize != 0;
            if weapon.circlesize == 0 {
                s.shot_dev_x = 0;
                s.shot_dir_x = 0;
                s.shot_dev_y = 0;
                s.shot_dir_y = 0;
                s.shot_cir_size_x = 0;
                s.shot_cir_size_y = 0;
            } else {
                let mut circsize = weapon.circlesize;
                if circsize > 19 {
                    let m20 = circsize % 20;
                    s.shot_cir_size_x = m20 as i16;
                    s.shot_dev_x = (m20 >> 1) as i16;
                    circsize /= 20;
                    s.shot_cir_size_y = circsize as i16;
                    s.shot_dev_y = (circsize >> 1) as i16;
                } else {
                    s.shot_cir_size_x = circsize as i16;
                    s.shot_cir_size_y = circsize as i16;
                    s.shot_dev_x = (circsize >> 1) as i16;
                    s.shot_dev_y = (circsize >> 1) as i16;
                }
                s.shot_dir_x = 1;
                s.shot_dir_y = -1;
            }
            s.shot_trail = weapon.trail;
            if s.shot_trail == 198 && mp > 1 {
                s.shot_trail = 255;
            }
            let atk = weapon.attack[k];
            if atk > 99 && atk < 250 {
                s.chain_reaction = atk - 100;
                s.shot_dmg = 1;
            } else {
                s.shot_dmg = atk;
            }
            s.shot_blast_filter = weapon.shipblastfilter;
            let mut tmp_by = weapon.by[k] as i16;
            s.shot_x = (px as i32 + weapon.bx[k] as i32) as i16;
            s.shot_y = (py as i32 + tmp_by as i32) as i16;
            s.shot_yc = -(weapon.acceleration as i16);
            s.shot_xc = weapon.accelerationx as i16;
            s.shot_xm = weapon.sx[k] as i16;
            let mut del = weapon.del[k];
            if del == 121 {
                s.shot_trail = 0;
                del = 255;
            }
            s.shot_gr = weapon.sg[k];
            self.shot_avail[shot_id] = if s.shot_gr == 0 { 0 } else { del };
            s.shot_ani_max =
                if del > 100 && del < 120 { (del - 100 + 1) as u16 } else { weapon.weapani.wrapping_add(1) };
            if del == 99 || del == 98 {
                tmp_by = (px as i32 - mouse_x as i32) as i16;
                tmp_by = tmp_by.clamp(-5, 5);
                s.shot_xm += tmp_by;
            }
            let sy = weapon.sy[k];
            let sx = weapon.sx[k];
            if del == 99 || del == 100 {
                let t = (py as i32 - mouse_y as i32 - sy as i32) as i16;
                s.shot_ym = t.clamp(-4, 4);
            } else if sy == 98 {
                s.shot_ym = 0;
                s.shot_yc = -1;
            } else if sy > 100 {
                // sy is int8_t: never > 100 in practice (max bound 127).
                s.shot_ym = sy as i16;
                s.shot_y = (s.shot_y as i32 - dy) as i16;
            } else {
                s.shot_ym = -(sy as i16);
            }
            if sx > 100 {
                s.shot_xm = sx as i16;
                s.shot_x = (s.shot_x as i32 - dx) as i16;
                if s.shot_xm == 101 {
                    s.shot_y = (s.shot_y as i32 - dy) as i16;
                }
            }
            if weapon.aim > 5 {
                let mut best_dist: u32 = 65000;
                let mut closest: u8 = 0;
                let (sxp, syp) = (s.shot_x as i32, s.shot_y as i32);
                for x in 0..ENEMY_MAX {
                    if self.enemy_avail[x] != 1 && !self.enemy[x].scoreitem {
                        let y = ((self.enemy[x].ex as i32 - sxp).abs() + (self.enemy[x].ey as i32 - syp).abs()) as u16
                            as u32;
                        if y < best_dist {
                            best_dist = y;
                            closest = x as u8 + 1;
                        }
                    }
                }
                let s = &mut self.player_shots[shot_id];
                s.aim_at_enemy = closest;
                s.aim_delay = 5;
                s.aim_delay_max = weapon.aim - 5;
            } else {
                self.player_shots[shot_id].aim_at_enemy = 0;
            }
            self.shot_repeat[bay_i] = weapon.shotrepeat;
            // shots.c: "slow hands" (fire rate ×10/7) and "frozen hands" (speed ×2/3) anomalies.
            if self.mode.anomaly(crate::modes::ANOM_SLOW_HANDS) && bay_i == SHOT_FRONT && self.shot_repeat[bay_i] < 200
            {
                self.shot_repeat[bay_i] = (self.shot_repeat[bay_i] as u32 * 10).div_ceil(7) as u8;
            }
            if self.mode.anomaly(crate::modes::ANOM_FROST_HANDS) && bay_i == SHOT_FRONT {
                let s = &mut self.player_shots[shot_id];
                s.shot_xm = ((s.shot_xm as i32 * 2) / 3) as i16;
                s.shot_ym = ((s.shot_ym as i32 * 2) / 3) as i16;
            }
        }
        // shots.c:player_shot_create: stats_count_shot + EVENT_WEAPON_FIRED.
        if shot_id < MAX_PWEAPON {
            self.ach_events.shots_fired += 1;
        }
        shot_id
    }

    /// `shots.c:player_shot_move_and_draw`. Returns `None` if the shot left
    /// the screen, otherwise (x, y, damage, filter, chain, player, special, x radius, y radius).
    fn player_shot_move(&mut self, shot_id: usize) -> Option<MovedShot> {
        self.shot_avail[shot_id] = self.shot_avail[shot_id].wrapping_sub(1);
        if shot_id == MAX_PWEAPON - 1 {
            return Some((0, 0, 0, 0, 0, 0, false, 0, 0));
        }
        let pn = self.player_shots[shot_id].player_number.max(1) as usize - 1;
        let (dx, dy) = (self.players[pn].delta_x_shot_move, self.players[pn].delta_y_shot_move);
        let s = &mut self.player_shots[shot_id];
        s.shot_xm = s.shot_xm.wrapping_add(s.shot_xc);
        let tmp_shot_xm = s.shot_xm;
        s.shot_x = s.shot_x.wrapping_add(s.shot_xm);
        if s.shot_xm > 100 {
            if s.shot_xm == 101 {
                s.shot_x = (s.shot_x as i32 - 101 + dx) as i16;
                s.shot_y = (s.shot_y as i32 + dy) as i16;
            } else {
                s.shot_x = (s.shot_x as i32 - 120 + dx) as i16;
            }
        }
        s.shot_ym = s.shot_ym.wrapping_add(s.shot_yc);
        s.shot_y = s.shot_y.wrapping_add(s.shot_ym);
        if s.shot_ym > 100 {
            s.shot_y = (s.shot_y as i32 - 120 + dy) as i16;
        }
        // shots.c:shot_apply_circular_motion
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
        let (ox, oy) = (s.shot_x as i32, s.shot_y as i32);
        if s.shot_x < -34 || s.shot_x > 290 || s.shot_y < -15 || s.shot_y > 190 {
            self.shot_avail[shot_id] = 0;
            return None;
        }
        let s = self.player_shots[shot_id];
        if s.shot_trail != 255 {
            if s.shot_trail == 98 || s.shot_trail == 198 {
                self.setup_explosion(s.shot_x - s.shot_xm, s.shot_y - s.shot_ym, 0, s.shot_trail as i16, false, false);
            } else {
                self.setup_explosion(s.shot_x, s.shot_y, 0, s.shot_trail as i16, false, false);
            }
        }
        if s.aim_at_enemy != 0 {
            let st = &mut self.player_shots[shot_id];
            st.aim_delay = st.aim_delay.wrapping_sub(1);
            if st.aim_delay == 0 {
                st.aim_delay = st.aim_delay_max;
                let t = (s.aim_at_enemy - 1) as usize;
                if self.enemy_avail[t] != 1 {
                    if st.shot_x < self.enemy[t].ex {
                        st.shot_xm += 1;
                    } else {
                        st.shot_xm -= 1;
                    }
                    if st.shot_y < self.enemy[t].ey {
                        st.shot_ym += 1;
                    } else {
                        st.shot_ym -= 1;
                    }
                } else if st.shot_xm > 0 {
                    st.shot_xm += 1;
                } else {
                    st.shot_xm -= 1;
                }
            }
        }
        let st = &mut self.player_shots[shot_id];
        let sprite_frame = st.shot_gr.wrapping_add(st.shot_ani);
        st.drawn_frame = sprite_frame;
        st.shot_ani = st.shot_ani.wrapping_add(1);
        if st.shot_ani == st.shot_ani_max {
            st.shot_ani = 0;
        }
        let st = *st;
        self.render.drawn_player_shots.push(DrawnShot {
            x: ox as i16,
            y: oy as i16,
            frame: sprite_frame,
            xm: tmp_shot_xm,
        });
        let is_special = sprite_frame > 60000;
        let (mut rw, mut rh) = (0u16, 0u16);
        if is_special {
            // `sprite(OPTION_SHAPES, sprite_frame - 60001)->width / 2` (and height).
            (rw, rh) = crate::option_shapes::option_shape_radius((sprite_frame - 60001) as usize);
        } else if sprite_frame > 1000 {
            // shots.c: JE_doSP(shotx + 1 + 6, shoty + 6, 5, 3, (frame / 1000) << 4).
            self.do_sp((ox + 1 + 6) as u16, (oy + 6) as u16, 5, 3, ((sprite_frame / 1000) << 4) as u8);
        }
        Some((
            ox,
            oy,
            st.shot_dmg as i16,
            st.shot_blast_filter,
            st.chain_reaction,
            st.player_number,
            is_special,
            rw,
            rh,
        ))
    }

    /// `shot_update.c:JE_updatePlayerShots`.
    pub fn update_player_shots(&mut self) {
        // `shot_update.c:JE_updatePlayerShots`: the Soul of Zinglon radius
        // (MAX_PWEAPON - 1) follows `player[0].x` and belongs to player 1 (ADR 0040).
        self.render.drawn_player_shots.clear();
        for z in 0..MAX_PWEAPON {
            if self.shot_avail[z] == 0 {
                continue;
            }
            let Some((tsx, tsy, mut damage, mut temp2, mut chain, mut player_num, is_special, rw, rh)) =
                self.player_shot_move(z)
            else {
                continue;
            };
            if z == MAX_PWEAPON - 1 {
                player_num = 1;
            }
            let mut b = 0usize;
            while b < ENEMY_MAX {
                if self.enemy_avail[b] == 0 {
                    let e = self.enemy[b];
                    let ex = e.ex as i32 + e.mapoffset as i32;
                    let collided = if z == MAX_PWEAPON - 1 {
                        let t = (25 - (self.special_weapon.zinglon_duration as i32 - 25).abs()) as u8;
                        temp2 = 9;
                        chain = 0;
                        damage = 10;
                        (ex - (self.players[0].x + 7)).abs() < t as i32
                    } else if is_special {
                        let (rw, rh) = (rw as i32, rh as i32);
                        (e.enemycycle == 0
                            && (ex - tsx - rw).abs() < 25 + rw
                            && (e.ey as i32 - tsy - 12 - rh).abs() < 29 + rh)
                            || (e.enemycycle > 0
                                && (ex - tsx - rw).abs() < 13 + rw
                                && (e.ey as i32 - tsy - 6 - rh).abs() < 15 + rh)
                    } else {
                        (e.enemycycle == 0 && (ex - tsx).abs() < 25 && (e.ey as i32 - tsy - 12).abs() < 29)
                            || (e.enemycycle > 0 && (ex - tsx).abs() < 13 && (e.ey as i32 - tsy - 6).abs() < 15)
                    };
                    if collided {
                        if chain > 0 {
                            self.players[(player_num.max(1) - 1) as usize].shot_multi_pos[SHOT_MISC] = 0;
                            let child = self.player_shot_create(
                                0,
                                SHOT_MISC,
                                tsx as u16,
                                tsy as u16,
                                self.controls.mouse_x,
                                self.controls.mouse_y,
                                chain as u16,
                                player_num,
                            );
                            // shot_update.c: the spawned shot carries its parent's bay (tally only).
                            if child < MAX_PWEAPON {
                                self.player_shots[child].shot_bay = self.player_shots[z].shot_bay;
                            }
                            self.shot_avail[z] = 0;
                            break;
                        }
                        let mut infinite_shot = false;
                        let do_iced: u8;
                        if damage == 99 {
                            damage = 0;
                            do_iced = 40;
                            self.enemy[b].iced = 40;
                        } else {
                            do_iced = 0;
                            if damage >= 250 {
                                damage -= 250;
                                infinite_shot = true;
                            }
                        }
                        // shot_update.c: "glass cannon", damage dealt doubled.
                        if self.mode.mutator(crate::modes::MUT_GLASS_CANNON) {
                            damage *= 2;
                        }
                        let armorleft = self.enemy[b].armorleft as i32;
                        // Shot at the origin of the hit, for the effect listeners.
                        let ps = &self.player_shots[z];
                        let shot = crate::fx::ShotInfo {
                            frame: ps.shot_gr.wrapping_add(ps.shot_ani),
                            dx: ps.shot_xm as i32,
                            dy: ps.shot_ym as i32,
                        };
                        let mut temp = self.enemy[b].linknum;
                        if temp == 0 {
                            temp = 255;
                        }
                        if self.enemy[b].armorleft < 255 {
                            for bb in self.script.boss_bar.iter_mut() {
                                if temp == bb.link_num {
                                    bb.color = 6;
                                }
                            }
                            if self.enemy[b].enemyground {
                                self.enemy[b].filter = temp2;
                            }
                            for e2 in 0..ENEMY_MAX {
                                if self.enemy[e2].linknum == temp
                                    && self.enemy_avail[e2] != 1
                                    && self.enemy[e2].enemyground
                                {
                                    if do_iced != 0 {
                                        self.enemy[e2].iced = do_iced;
                                    }
                                    self.enemy[e2].filter = temp2;
                                }
                            }
                        }
                        if armorleft > damage as i32 {
                            if z != MAX_PWEAPON - 1 {
                                if self.enemy[b].armorleft != 255 {
                                    self.enemy[b].armorleft = self.enemy[b].armorleft.wrapping_sub(damage as u8);
                                    // shot_update.c: EVENT_SHOT_HIT (non-lethal branch).
                                    self.ach_events.shots_hit += 1;
                                    self.ach_events.damage_dealt += damage as u32;
                                    // shot_update.c: level_stats_record_damage(shotBay, damage).
                                    let bay = self.player_shots[z].shot_bay;
                                    self.level_stats.record_damage(bay, damage as u32);
                                    self.fx_event(crate::fx::FxEvent::ShotHit {
                                        x: tsx,
                                        y: tsy,
                                        damage: damage as i32,
                                        frame: shot.frame,
                                    });
                                    self.setup_explosion(tsx as i16, tsy as i16, 0, 0, false, false);
                                    // shot_update.c: impact wave at the center of the enemy.
                                    let e = &self.enemy[b];
                                    let (hx, hy) = (e.ex as i32 + e.mapoffset as i32 + 6, e.ey as i32 + 7);
                                    self.fx_event(crate::fx::FxEvent::HitRipple { x: hx, y: hy });
                                } else {
                                    // shot_update.c: JE_doSP(tempShotX + 6, tempShotY + 6, damage / 2 + 3, damage / 4 + 2, temp2).
                                    self.do_sp(
                                        (tsx + 6) as u16,
                                        (tsy + 6) as u16,
                                        (damage / 2 + 3) as u16,
                                        (damage / 4 + 2) as u8,
                                        temp2,
                                    );
                                }
                            }
                            // shot_update.c: soundQueue[5] = S_ENEMY_HIT; soundQueueX[5] = enemy[b].ex.
                            self.queue_sound_at(5, crate::sound::S_ENEMY_HIT, self.enemy[b].ex as i32);
                            let e = self.enemy[b];
                            if (armorleft - damage as i32 <= e.edlevel as i32) && ((!e.edamaged) ^ (e.edani < 0)) {
                                self.process_damage_chain(b, damage, temp, shot);
                            }
                        } else {
                            // shot_update.c: EVENT_SHOT_HIT of the fatal hit, damage actually dealt.
                            let dealt = (damage as i32).min(armorleft);
                            if dealt > 0 {
                                self.ach_events.shots_hit += 1;
                                self.ach_events.damage_dealt += dealt as u32;
                                let bay = self.player_shots[z].shot_bay;
                                self.level_stats.record_damage(bay, dealt as u32);
                                self.fx_event(crate::fx::FxEvent::ShotHit {
                                    x: tsx,
                                    y: tsy,
                                    damage: dealt,
                                    frame: shot.frame,
                                });
                            }
                            self.process_kill_chain(b, tsx, tsy, damage, player_num, temp, shot);
                        }
                        if infinite_shot {
                            damage += 250;
                        } else if z != MAX_PWEAPON - 1 {
                            if damage as i32 <= armorleft {
                                self.shot_avail[z] = 0;
                                break;
                            } else {
                                let s = &mut self.player_shots[z];
                                s.shot_dmg = s.shot_dmg.wrapping_sub(armorleft as u8);
                            }
                        }
                    }
                }
                b += 1;
            }
        }
    }

    /// `varz.c:JE_playerDamage`. Returns `playerDamage` (truncated to uint8_t).
    pub fn player_damage(&mut self, mut temp: u8, pi: usize) -> u8 {
        let mut player_damage: i32 = 0;
        let two = self.two_player_mode;
        // varz.c:JE_playerDamage: soundQueue[7] = S_SHIELD_HIT at the player's position.
        let sound_x = self.players[pi].x;
        self.queue_sound_at(7, crate::sound::S_SHIELD_HIT, sound_x);
        // varz.c:JE_playerDamage: "glass cannon", damage taken doubled (cap 255).
        if self.mode.mutator(crate::modes::MUT_GLASS_CANNON) {
            temp = (temp as i32 * 2).min(255) as u8;
        }
        // varz.c:JE_playerDamage: EVENT_PLAYER_HIT if playerDamage > 0 or temp > 0,
        // i.e. if the damage received (after glass cannon) is non-zero.
        if temp > 0 {
            self.ach_events.player_hits += 1;
            // EVENT_PLAYER_HIT: dmg = playerDamage > 0 ? playerDamage : temp, here
            // always the damage received (emitted at the end of the function by the C code).
        }
        // varz.c: final "temp", read by level_stats_record_hit (shield share).
        let mut stats_temp = temp as u32;
        let hit_damage = temp as i32;
        if self.players[pi].shield < temp as u32 {
            player_damage = temp as i32;
            temp = (temp as u32).wrapping_sub(self.players[pi].shield) as u8;
            stats_temp = temp as u32;
            self.players[pi].shield = 0;
            if temp > 0 {
                let p = &mut self.players[pi];
                if p.armor < temp as u32 {
                    // varz.c: "temp -= this_player->armor" before armor = 0.
                    stats_temp = (temp as u32).wrapping_sub(p.armor) & 0xff;
                    p.armor = 0;
                    if p.is_alive {
                        // varz.c:JE_playerDamage: "if (!timedBattleMode) levelTimer = false".
                        if self.mode.timed == 0 {
                            self.script.level_timer = false;
                        }
                        p.is_alive = false;
                        p.exploding_ticks = 60;
                        self.ending.level_end = 40;
                        self.music.temp_volume = self.music.tyr_music_volume;
                        self.queue_sound_at(1, crate::sound::S_EXPLOSION_22, sound_x);
                        self.fx_event(crate::fx::FxEvent::PlayerDied { player: pi as u8 });
                    }
                } else {
                    p.armor -= temp as u32;
                    self.queue_sound_at(7, crate::sound::S_HULL_HIT, sound_x);
                }
            }
        } else {
            self.players[pi].shield -= temp as u32;
            let (x, y) = (self.players[pi].x, self.players[pi].y);
            let f = !two;
            let pts: [(i32, i32, i16); 9] = [
                (x - 17, y - 12, 14),
                (x - 5, y - 12, 15),
                (x + 7, y - 12, 16),
                (x + 19, y - 12, 17),
                (x - 17, y + 2, 18),
                (x + 19, y + 2, 19),
                (x - 17, y + 16, 20),
                (x - 5, y + 16, 21),
                (x + 7, y + 16, 22),
            ];
            for (ex, ey, t) in pts {
                self.setup_explosion(ex as i16, ey as i16, 0, t, false, f);
            }
            // varz.c: cyan flash of shield absorption (RGBA layer).
            self.fx_event(crate::fx::FxEvent::ShieldAbsorb { x, y });
        }
        if hit_damage > 0 {
            self.fx_event(crate::fx::FxEvent::PlayerHit { player: pi as u8, damage: hit_damage });
        }
        // varz.c:JE_playerDamage: level_stats_record_hit(total_hit, shield_part).
        let total_hit = if player_damage > 0 { player_damage as u32 } else { stats_temp };
        let shield_part = if player_damage > 0 { total_hit.wrapping_sub(stats_temp) } else { total_hit };
        self.level_stats.damage_taken = self.level_stats.damage_taken.wrapping_add(total_hit);
        self.level_stats.damage_blocked = self.level_stats.damage_blocked.wrapping_add(shield_part);
        self.draw_armor_clamp();
        self.hud_bars(true, true, true);
        player_damage as u8
    }

    /// Rendering only: records a redraw of the shield and armor bars.
    pub fn hud_bars(&mut self, wipe: bool, shield: bool, armor: bool) {
        // Bound: the display flushes the log at each tick; with no reader
        // (tools, fast-forward), only the final state of the bars matters.
        if self.render.hud_ops.len() >= 256 {
            self.render.hud_ops.clear();
        }
        let (p0, p1) = (&self.players[0], &self.players[1]);
        self.render.hud_ops.push(crate::world::HudOp::Bars {
            wipe,
            shield,
            armor,
            shields: [p0.shield, p1.shield],
            armors: [p0.armor, p1.armor],
        });
    }

    /// `varz.c:JE_drawArmor`: only the clamping of armor to a maximum of 28 modifies the state.
    pub fn draw_armor_clamp(&mut self) {
        for p in self.players.iter_mut() {
            if p.armor > 28 {
                p.armor = 28;
            }
        }
    }

    /// `shot_update.c:JE_updateEnemyShots`.
    pub fn update_enemy_shots(&mut self) {
        self.render.drawn_enemy_shots.clear();
        for z in 0..self.enemy_shots.len() {
            if self.enemy_shot_avail[z] {
                continue;
            }
            let (p0x, p0y) = (self.players[0].x, self.players[0].y);
            let s = &mut self.enemy_shots[z];
            s.sxm = s.sxm.wrapping_add(s.sxc as i16);
            s.sx = s.sx.wrapping_add(s.sxm);
            if s.tx != 0 {
                if s.sx as i32 > p0x {
                    if s.sxm > -(s.tx as i16) {
                        s.sxm -= 1;
                    }
                } else if s.sxm < s.tx as i16 {
                    s.sxm += 1;
                }
            }
            s.sym = s.sym.wrapping_add(s.syc as i16);
            s.sy = s.sy.wrapping_add(s.sym);
            if s.ty != 0 {
                if s.sy as i32 > p0y {
                    if s.sym > -(s.ty as i16) {
                        s.sym -= 1;
                    }
                } else if s.sym < s.ty as i16 {
                    s.sym += 1;
                }
            }
            let dur = s.duration;
            s.duration = s.duration.wrapping_sub(1);
            if dur == 0 || s.sy > 190 || s.sy <= -14 || s.sx > 275 || s.sx <= 0 {
                self.enemy_shot_avail[z] = true;
                continue;
            }
            let n = if self.two_player_mode { 2 } else { 1 };
            for i in 0..n {
                let p = self.players[i];
                let s = self.enemy_shots[z];
                if p.is_alive
                    && (s.sx as i32) > p.x - p.shot_hit_area_x as i32
                    && (s.sx as i32) < p.x + p.shot_hit_area_x as i32
                    && (s.sy as i32) > p.y - p.shot_hit_area_y as i32
                    && (s.sy as i32) < p.y + p.shot_hit_area_y as i32
                {
                    self.enemy_shot_avail[z] = true;
                    self.setup_explosion(s.sx, s.sy, 0, 0, false, false);
                    // shot_update.c: impact wave on the hull.
                    self.fx_event(crate::fx::FxEvent::PlayerRipple { x: p.x + 6, y: p.y + 6 });
                    if p.invulnerable_ticks == 0 {
                        let t = self.player_damage(s.sdmg, i) as i32;
                        if t > 0 {
                            self.players[i].x_velocity += (s.sxm as i32 * t) / 2;
                            self.players[i].y_velocity += (s.sym as i32 * t) / 2;
                        }
                    }
                    break;
                }
            }
            if !self.enemy_shot_avail[z] {
                let s = &mut self.enemy_shots[z];
                if s.animax != 0 {
                    s.animate = s.animate.wrapping_add(1);
                    if s.animate >= s.animax {
                        s.animate = 0;
                    }
                }
                // Drawing: blit_sprite2 of sgr + animate (rendering only).
                let s = &self.enemy_shots[z];
                self.render.drawn_enemy_shots.push((s.sx, s.sy, s.sgr.wrapping_add(s.animate)));
            }
        }
    }
}
