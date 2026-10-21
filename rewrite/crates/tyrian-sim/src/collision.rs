//! Consequences of a player shot on an enemy: `collision.c`
//! (`collision_process_damage_chain`, `collision_process_kill_chain`,
//! `boss_track_*`).

use crate::world::*;

/// `collision.c:boss_kill_track`.
#[derive(Default)]
struct BossTrack {
    hit: [bool; 2],
    /// Position, value and image of the shot of the last member killed (event_bus.h:boss_defeated).
    at: [(i32, i32, i32, u16); 2],
}

impl World {
    fn boss_track_kill(&self, t: &mut BossTrack, linknum: u8, at: (i32, i32, i32, u16)) {
        if linknum == 0 {
            return;
        }
        for i in 0..2 {
            if self.script.boss_bar[i].link_num == linknum {
                t.hit[i] = true;
                t.at[i] = at;
            }
        }
    }

    /// `collision.c`: EVENT_ENEMY_KILLED and its simulation-side listeners
    /// (`hitstop.c:on_enemy_killed`); the rest is rendering.
    fn emit_enemy_killed(&mut self, x: i32, y: i32, damage: i32, value: i32, shot: crate::fx::ShotInfo) {
        self.fx_event(crate::fx::FxEvent::EnemyKilled {
            x,
            y,
            damage,
            value,
            frame: shot.frame,
            shot_dx: shot.dx,
            shot_dy: shot.dy,
        });
        self.hitstop_on_enemy_killed(value, damage);
    }

    /// `collision.c:boss_link_dead`.
    fn boss_link_dead(&self, linknum: u8) -> bool {
        let mut armor: u32 = 256;
        for i in 0..ENEMY_MAX {
            if self.enemy_avail[i] != 1 && self.enemy[i].linknum == linknum && (self.enemy[i].armorleft as u32) < armor
            {
                armor = self.enemy[i].armorleft as u32;
            }
        }
        armor > 255 || armor == 0
    }

    /// `collision.c:boss_track_flush` then the EVENT_BOSS_DEFEATED listener of
    /// `hitstop.c:on_boss_defeated`: with processorType >= 2 (default value
    /// 4, `config.c`), it draws 5 × 2 `rng_range` for the cascade
    /// of secondary explosions, even when hitstop is disabled (the cascade
    /// is then never played: `secondary_explosions_update` returns
    /// immediately). The particles.c listener also draws the C randomness; here the
    /// particles have their own generator (rendering, tyrian-godot).
    fn boss_track_flush(&mut self, t: &BossTrack) {
        for i in 0..2 {
            if !t.hit[i] || self.script.boss_bar[i].link_num == 0 {
                continue;
            }
            let mut dup = false;
            for j in 0..i {
                if t.hit[j] && self.script.boss_bar[j].link_num == self.script.boss_bar[i].link_num {
                    dup = true;
                }
            }
            if dup || !self.boss_link_dead(self.script.boss_bar[i].link_num) {
                continue;
            }
            let (x, y, value, frame) = t.at[i];
            self.fx_event(crate::fx::FxEvent::BossDefeated { x, y, value, frame });
            self.hitstop_on_boss_defeated(x, y);
            self.ach_events.bosses_defeated += 1;
        }
    }

    /// `collision.c:collision_process_damage_chain`.
    pub fn process_damage_chain(&mut self, b: usize, damage: i16, temp: u8, shot: crate::fx::ShotInfo) {
        let mut track = BossTrack::default();
        for t3 in 0..ENEMY_MAX {
            if self.enemy_avail[t3] == 1 {
                continue;
            }
            let linknum = self.enemy[t3].linknum as i32;
            let tt = temp as i32;
            let hit = t3 == b
                || (temp != 255
                    && ((self.enemy[t3].edlevel > 0 && linknum == tt)
                        || (self.script.enemy_continual_damage && tt - 100 == linknum)
                        || (linknum > 40 && linknum / 20 == tt / 20 && linknum <= tt)));
            if !hit {
                continue;
            }
            {
                let e = &mut self.enemy[t3];
                e.enemycycle = 1;
                e.edamaged = !e.edamaged;
            }
            let e = self.enemy[t3];
            if e.edani != 0 {
                let e = &mut self.enemy[t3];
                e.ani = (e.edani as i32).unsigned_abs() as u8;
                e.aniactive = 1;
                e.animax = 0;
                e.animin = e.edgr as u8;
                e.enemycycle = e.animin.wrapping_sub(1);
            } else if e.edgr > 0 {
                let e = &mut self.enemy[t3];
                e.egr[0] = e.edgr;
                e.ani = 1;
                e.aniactive = 0;
                e.animax = 0;
                e.animin = 1;
            } else {
                self.enemy_avail[t3] = 1;
                self.enemy_killed = self.enemy_killed.wrapping_add(1);
                // collision.c (lethal damage): EVENT_ENEMY_KILLED, .value = evalue,
                // at (ex + mapoffset, ey).
                let (kx, ky) = (e.ex as i32 + e.mapoffset as i32, e.ey as i32);
                self.emit_enemy_killed(kx, ky, damage as i32, e.evalue as i32, shot);
                self.endless_on_enemy_killed(e.evalue as i32);
                self.ach_events.kills += 1;
                if e.evalue > 0 {
                    self.ach_events.kill_credits += e.evalue as u64;
                }
                self.boss_track_kill(&mut track, e.linknum, (kx, ky, e.evalue as i32, shot.frame));
            }
            let e = &mut self.enemy[t3];
            e.aniwhenfire = 0;
            if e.armorleft > e.edlevel as u8 {
                e.armorleft = e.edlevel as u8;
            }
            let temp_x = e.ex.wrapping_add(e.mapoffset as i16);
            let temp_y = e.ey;
            let (ground, explonum, ty) = (e.enemyground, e.explonum, e.enemytype);
            if self.tables.enemy(ty as usize).esize != 1 {
                self.setup_explosion(temp_x, temp_y - 6, 0, 1, false, false);
            } else {
                self.setup_explosion_large(ground, explonum / 2, temp_x, temp_y);
            }
        }
        self.boss_track_flush(&track);
    }

    /// `collision.c:collision_process_kill_chain`.
    #[allow(clippy::too_many_arguments)] // same parameters as the C function
    pub fn process_kill_chain(
        &mut self,
        b: usize,
        shot_x: i32,
        shot_y: i32,
        damage: i16,
        player_num: u8,
        temp: u8,
        shot: crate::fx::ShotInfo,
    ) {
        let mut track = BossTrack::default();
        if temp == 254 && self.script.super_enemy_254_jump > 0 {
            self.event_jump(self.script.super_enemy_254_jump);
        }
        for t2 in 0..ENEMY_MAX {
            if self.enemy_avail[t2] == 1 {
                continue;
            }
            let t3 = self.enemy[t2].linknum as i32;
            let tt = temp as i32;
            if !(t2 == b
                || temp == 254
                || (temp != 255 && (tt == t3 || tt - 100 == t3 || (t3 > 40 && t3 / 20 == tt / 20 && t3 <= tt))))
            {
                continue;
            }
            let e = self.enemy[t2];
            let enemy_screen_x = e.ex.wrapping_add(e.mapoffset as i16);
            if e.special && e.flagnum >= 1 && e.flagnum <= 10 {
                self.script.global_flags[(e.flagnum - 1) as usize] = e.setto;
            }
            // collision.c: in Super Arcade, no purple ball (value 30000) is dropped.
            let sa = self.mode.super_arcade_mode != 0;
            if e.enemydie > 0 && !(sa && self.tables.enemy(e.enemydie as usize).value == 30000) {
                let temp_w = e.enemydie;
                let mut off = t2 - (t2 % 25);
                if self.tables.enemy(temp_w as usize).value > 30000 {
                    off = 0;
                }
                if let Some(nb) = self.new_enemy(off, temp_w, 0) {
                    // collision.c: bonus Super Arcade, cycle superArcadePowerUp 1..=5.
                    if sa && self.enemy[nb].evalue > 30000 {
                        self.mode.super_arcade_power_up += 1;
                        if self.mode.super_arcade_power_up > 5 {
                            self.mode.super_arcade_power_up = 1;
                        }
                        let pu = self.mode.super_arcade_power_up;
                        self.enemy[nb].egr[0] = 5 + pu as u16 * 2;
                        self.enemy[nb].evalue = 30000 + pu as i16;
                    }
                    let (px, py) = (self.enemy[t2].ex, self.enemy[t2].ey);
                    let ne = &mut self.enemy[nb];
                    ne.scoreitem = ne.evalue != 0;
                    ne.ex = px;
                    ne.ey = py;
                }
            }
            let e = self.enemy[t2];
            if e.evalue > 0 && e.evalue < 10000 {
                if e.evalue == 1 {
                    self.ending.cube_max = self.ending.cube_max.wrapping_add(1);
                } else {
                    // collision.c: "player[galagaMode ? 0 : playerNum - 1].cash += evalue".
                    // The Zinglon beam belongs to player 1 (ADR 0040).
                    let idx = if self.mode.galaga_mode { 0 } else { (player_num - 1) as usize };
                    // "* (combo_enabled ? combo_multiplier : 1)" (combos.c), before
                    // the combo_on_kill of that same enemy (emitted further down).
                    let mult = self.cash_combo_multiplier() as i64;
                    let p = &mut self.players[idx];
                    p.cash = p.cash.wrapping_add((e.evalue as i64 * mult) as u64);
                }
            }
            if e.edlevel == -1 && tt == t3 {
                self.enemy_avail[t2] = 2;
                let e = &mut self.enemy[t2];
                e.edlevel = 0;
                e.egr[0] = e.edgr;
                e.ani = 1;
                e.aniactive = 0;
                e.animax = 0;
                e.animin = 1;
                e.edamaged = true;
                e.enemycycle = 1;
            } else {
                self.enemy_avail[t2] = 1;
                self.enemy_killed = self.enemy_killed.wrapping_add(1);
                // collision.c:collision_process_kill_chain: EVENT_ENEMY_KILLED
                // at the shot position.
                self.emit_enemy_killed(shot_x, shot_y, damage as i32, e.evalue as i32, shot);
                self.endless_on_enemy_killed(e.evalue as i32);
                self.ach_events.kills += 1;
                if e.evalue > 0 {
                    self.ach_events.kill_credits += e.evalue as u64;
                }
                self.boss_track_kill(&mut track, e.linknum, (shot_x, shot_y, e.evalue as i32, shot.frame));
            }
            let e = self.enemy[t2];
            if self.tables.enemy(e.enemytype as usize).esize == 1 {
                self.setup_explosion_large(e.enemyground, e.explonum, enemy_screen_x, e.ey);
                // collision.c: big enemy, deeper explosion; boss (value >= 500): the deepest.
                self.queue_sound_dmg(
                    6,
                    crate::sound::S_EXPLOSION_9,
                    enemy_screen_x as i32,
                    if e.evalue >= 500 { 3 } else { 2 },
                );
            } else {
                self.setup_explosion(enemy_screen_x, e.ey, 0, 1, false, false);
                self.queue_sound_dmg(6, crate::sound::S_EXPLOSION_8, enemy_screen_x as i32, 1);
            }
        }
        self.boss_track_flush(&track);
    }
}
