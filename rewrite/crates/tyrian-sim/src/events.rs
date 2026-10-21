//! Level events: `level_events.c:JE_eventSystem` and the helpers of
//! `level_helpers.c` (`JE_createNewEventEnemy`, `JE_eventJump`, `JE_searchFor`).
//!
//! Purely audiovisual events (text, music, sound, color
//! filters, sprite sheet loading) only modify rendering variables;
//! they are kept when they touch the state read by Godot.

use crate::world::*;
use tyrian_formats::lvl::Event;

impl World {
    /// `level_events.c:event_jump_relative`.
    fn event_jump_relative(&mut self, delta: i32) {
        let dest = self.script.event_loc as i32 + delta;
        self.script.event_loc = dest.clamp(1, self.script.max_event as i32) as u16;
    }

    /// `level_helpers.c:JE_eventJump`.
    pub fn event_jump(&mut self, jump: u16) {
        if jump == 65535 {
            self.script.cur_loc = self.script.return_loc;
        } else {
            self.script.return_loc = self.script.cur_loc.wrapping_add(1);
            self.script.cur_loc = jump;
        }
        // Search only the real entries and the sentinel, not the zeros
        // outside the ev() array. A mod can contain times greater than 65500.
        if let Some(index) = self
            .script
            .event_rec
            .iter()
            .take(self.script.max_event as usize + 1)
            .position(|ev| ev.eventtime >= self.script.cur_loc)
        {
            // event_system then increments this zero-indexed destination.
            self.script.event_loc = index as u16;
        } else {
            self.stop_event_script("jump/return has no destination");
        }
    }

    /// Stops an invalid or unbounded script without running the following events.
    pub(crate) fn stop_event_script(&mut self, reason: &str) {
        eprintln!(
            "invalid level event script: {reason} (event {}, time {})",
            self.script.event_loc, self.script.cur_loc
        );
        self.script.event_loc = self.script.max_event.saturating_add(1);
        self.ending.really_end_level = true;
    }

    /// `level_helpers.c:JE_searchFor`: last active enemy (avail 0) of this link.
    pub fn search_for(&mut self, pl_type: u8) -> Option<usize> {
        let mut found = None;
        for i in 0..ENEMY_MAX {
            if self.enemy_avail[i] == 0 && self.enemy[i].linknum == pl_type {
                found = Some(i);
                // level_helpers.c:JE_searchFor: in Galaga, the enemy's value is doubled.
                if self.mode.galaga_mode {
                    self.enemy[i].evalue = self.enemy[i].evalue.wrapping_add(self.enemy[i].evalue);
                }
            }
        }
        found
    }

    /// `level_helpers.c:JE_createNewEventEnemy`: returns the slot taken
    /// (the C global `b`), even if `make_enemy` left it free.
    pub fn create_new_event_enemy(
        &mut self,
        enemy_type_ofs: u8,
        enemy_offset: usize,
        unique_shape: i16,
    ) -> Option<usize> {
        let bi = (enemy_offset..enemy_offset + 25).find(|&i| self.enemy_avail[i] == 1)?;
        let ev = self.cur_ev();
        let temp_w = (ev.eventdat as i32 + enemy_type_ofs as i32) as u16;
        let mut e = self.enemy[bi];
        let avail = self.make_enemy(&mut e, temp_w, unique_shape);
        self.enemy[bi] = e;
        self.enemy_avail[bi] = avail;

        if ev.eventdat2 == -200 {
            let v = (self.rng.next_u32() % 208) as i16 + 24;
            self.cur_ev_mut().eventdat2 = v;
        }
        let ev = self.cur_ev();
        let map_x = self.background.map_x as i32;
        let e = &mut self.enemy[bi];
        if ev.eventdat2 != -99 {
            match enemy_offset {
                0 => {
                    e.ex = (ev.eventdat2 as i32 - (map_x - 1) * 24) as i16;
                    e.ey = (e.ey as i32 - self.background.back_move[1] as i32) as i16;
                }
                25 | 75 => {
                    e.ex = (ev.eventdat2 as i32 - (map_x - 1) * 24 - 12) as i16;
                    e.ey = (e.ey as i32 - self.background.back_move[0] as i32) as i16;
                }
                50 => {
                    if self.background.background3x1 {
                        e.ex = (ev.eventdat2 as i32 - (map_x - 1) * 24 - 12) as i16;
                    } else {
                        e.ex = (ev.eventdat2 as i32 - self.background.map_x3 as i32 * 24 - 24 * 2 + 6) as i16;
                    }
                    e.ey = (e.ey as i32 - self.background.back_move[2] as i32) as i16;
                    if self.background.background3x1b {
                        e.ex -= 6;
                    }
                }
                _ => {}
            }
            // level_helpers.c (upstream tyrian2.c order): ey set after the
            // switch, which cancels the "ey -= backMove*" above.
            e.ey = -28;
            if self.background.background3x1b && enemy_offset == 50 {
                e.ey += 4;
            }
        }
        if self.script.small_enemy_adjust && e.size == 0 {
            e.ex -= 10;
            e.ey -= 7;
        }
        e.ey = e.ey.wrapping_add(ev.eventdat5 as i16);
        e.eyc = e.eyc.wrapping_add(ev.eventdat3);
        e.linknum = ev.eventdat4;
        e.fixedmovey = ev.eventdat6 as i16;
        // level_helpers.c:JE_createNewEventEnemy, mirror mutators then fast enemies.
        if self.mode.mutator(crate::modes::MUT_MIRROR) {
            let e = &mut self.enemy[bi];
            e.ex = 264 - e.ex;
            e.exc = e.exc.wrapping_neg();
        }
        if self.mode.mutator(crate::modes::MUT_FAST_ENEMIES) {
            let e = &mut self.enemy[bi];
            e.exc = crate::modes::scale_speed(e.exc as i32) as i8;
            e.eyc = crate::modes::scale_speed(e.eyc as i32) as i8;
        }
        Some(bi)
    }

    /// Creates an event enemy then forces its height (events 17, 18,
    /// 23, 32, 56: enemies rising from the bottom of the screen).
    fn create_new_event_enemy_at_y(&mut self, enemy_offset: usize, ey: i32) {
        if let Some(slot) = self.create_new_event_enemy(0, enemy_offset, 0) {
            self.enemy[slot].ey = ey as i16;
        }
    }

    /// `level_events.c:JE_eventSystem`: runs `eventRec[eventLoc-1]` then advances.
    pub fn event_system(&mut self) {
        let ev = self.cur_ev();
        match ev.eventtype {
            1 | 2 | 3 | 4 | 83 | 8 | 9 | 21 | 22 | 28 | 29 | 30 | 42 | 43 | 48 | 64 | 65 | 72 | 73 | 77 | 81 => {
                self.event_background(ev)
            }
            5 | 6 | 7 | 10 | 12 | 15 | 16 | 17 | 18 | 23 | 32 | 49..=52 | 56 => self.event_spawn_enemy(ev),
            19 | 20 | 24 | 25 | 27 | 31 | 33 | 39 | 41 | 45 | 47 | 55 | 58 | 59 | 68 | 60 | 74 | 75 | 85 => {
                self.event_modify_enemies(ev)
            }
            11 | 13 | 14 | 36 | 37 | 38 | 53 | 54 | 57 | 61 | 62 | 63 | 66 | 67 | 70 | 71 | 76 | 80 | 84 => {
                self.event_script_flow(ev)
            }
            26 | 34 | 35 | 40 | 44 | 46 | 69 | 78 | 79 | 82 | 99 => self.event_level_settings(ev),
            _ => {}
        }
        self.script.event_loc = self.script.event_loc.wrapping_add(1);
    }

    /// Background: scrolling, layers, stars, smoothies.
    fn event_background(&mut self, ev: Event) {
        match ev.eventtype {
            1 => self.background.starfield_speed = ev.eventdat as i32,
            2 => {
                self.background.map1_y_delay = 1;
                self.background.map1_y_delay_max = 1;
                self.background.map2_y_delay = 1;
                self.background.map2_y_delay_max = 1;
                self.background.back_move[0] = ev.eventdat as u16;
                self.background.back_move[1] = ev.eventdat2 as u16;
                self.background.explode_move = if self.background.back_move[1] > 0 {
                    self.background.back_move[1]
                } else {
                    self.background.back_move[0]
                };
                self.background.back_move[2] = ev.eventdat3 as i16 as u16;
                if self.background.back_move[0] > 0 {
                    self.background.stop_background_num = 0;
                }
            }
            3 => {
                self.background.back_move[0] = 1;
                self.background.map1_y_delay = 3;
                self.background.map1_y_delay_max = 3;
                self.background.back_move[1] = 1;
                self.background.map2_y_delay = 2;
                self.background.map2_y_delay_max = 2;
                self.background.back_move[2] = 1;
            }
            4 | 83 => {
                self.background.stop_backgrounds = true;
                match ev.eventdat {
                    0 | 1 => self.background.stop_background_num = 1,
                    2 => self.background.stop_background_num = 2,
                    3 => self.background.stop_background_num = 3,
                    _ => {}
                }
            }
            8 => self.background.star_active = false,
            9 => self.background.star_active = true,
            21 => self.background.background3over = 1,
            22 => self.background.background3over = 0,
            28 => self.background.top_enemy_over = false,
            29 => self.background.top_enemy_over = true,
            30 => {
                self.background.map1_y_delay = 1;
                self.background.map1_y_delay_max = 1;
                self.background.map2_y_delay = 1;
                self.background.map2_y_delay_max = 1;
                self.background.back_move[0] = ev.eventdat as u16;
                self.background.back_move[1] = ev.eventdat2 as u16;
                self.background.explode_move = self.background.back_move[1];
                self.background.back_move[2] = ev.eventdat3 as i16 as u16;
            }
            42 => self.background.background3over = 2,
            43 => self.background.background2over = ev.eventdat as u8,
            48 => self.background.background2_not_transparent = true,
            64 => {
                if !(ev.eventdat == 6 && self.two_player_mode && self.difficulty > DIFFICULTY_NORMAL) {
                    let sm = ev.eventdat;
                    if (1..=9).contains(&sm) {
                        self.background.smoothies[(sm - 1) as usize] = ev.eventdat2 != 0;
                        let mut temp = sm as usize;
                        if temp == 5 {
                            temp = 3;
                        }
                        self.background.smoothie_data[temp - 1] = ev.eventdat3 as u8;
                    }
                }
            }
            65 => self.background.background3x1 = ev.eventdat == 0,
            72 => self.background.background3x1b = ev.eventdat == 1,
            73 => self.background.sky_enemy_over_all = ev.eventdat == 1,
            77 => {
                self.background.map_y_pos[0] = (ev.eventdat / 2) as i32;
                if ev.eventdat2 > 0 {
                    self.background.map_y_pos[1] = (ev.eventdat2 / 2) as i32;
                } else {
                    self.background.map_y_pos[1] = (ev.eventdat / 2) as i32;
                }
            }
            81 => {
                self.background.bk_wrap[1] = (ev.eventdat / 2) as i32;
                self.background.bk_wrap_to[1] = (ev.eventdat2 / 2) as i32;
            }
            _ => unreachable!("event {} not handled by event_background", ev.eventtype),
        }
    }

    /// Enemy creation by the script.
    fn event_spawn_enemy(&mut self, ev: Event) {
        match ev.eventtype {
            5 => {
                // Loading of sprite banks (only the identifier matters here).
                let tables =
                    [ev.eventdat.max(0) as u8, ev.eventdat2.max(0) as u8, ev.eventdat3.max(0) as u8, ev.eventdat4];
                for (id, &table) in self.script.enemy_sprite_sheet_ids.iter_mut().zip(tables.iter()) {
                    if *id != table {
                        if table as usize > SHAPE_FILE.len() {
                            continue;
                        }
                        *id = table;
                    }
                }
            }
            6 => {
                self.create_new_event_enemy(0, 25, 0);
            }
            7 => {
                self.create_new_event_enemy(0, 50, 0);
            }
            10 => {
                self.create_new_event_enemy(0, 75, 0);
            }
            12 => {
                let temp = match ev.eventdat6 {
                    0 | 1 => 25,
                    2 => 0,
                    3 => 50,
                    4 => 75,
                    _ => 0,
                };
                self.cur_ev_mut().eventdat6 = 0;
                // 2×2 enemy: four quarters offset by 24 px / 28 px.
                self.create_new_event_enemy(0, temp, 0);
                if let Some(slot) = self.create_new_event_enemy(1, temp, 0) {
                    self.enemy[slot].ex += 24;
                }
                if let Some(slot) = self.create_new_event_enemy(2, temp, 0) {
                    self.enemy[slot].ey -= 28;
                }
                if let Some(slot) = self.create_new_event_enemy(3, temp, 0) {
                    let e = &mut self.enemy[slot];
                    e.ex += 24;
                    e.ey -= 28;
                }
            }
            15 => {
                self.create_new_event_enemy(0, 0, 0);
            }
            16 => {
                // Text: only the case "outside the 9 messages but valid enemy
                // identifier" changes the state (replayed like event 10).
                let dat = ev.eventdat;
                if (1..=9).contains(&dat) {
                    // level_events.c: message voice, once per script position.
                    let sfx = crate::sound::WINDOW_TEXT_SAMPLES[(dat - 1) as usize];
                    if self.voice_dedup_allow(sfx, self.script.event_loc) {
                        self.render.hud_ops.push(crate::world::HudOp::EventText { index: (dat - 1) as u8 });
                        self.queue_sound(3, sfx);
                    }
                } else if dat > 9 && dat as usize <= crate::tables::ENEMY_NUM {
                    self.create_new_event_enemy(0, 75, 0);
                }
            }
            17 => {
                self.create_new_event_enemy_at_y(25, 190 + ev.eventdat5 as i32);
            }
            18 => {
                self.create_new_event_enemy_at_y(0, 190 + ev.eventdat5 as i32);
            }
            23 => {
                self.create_new_event_enemy_at_y(50, 180 + ev.eventdat5 as i32);
            }
            32 => {
                self.create_new_event_enemy_at_y(50, 190);
            }
            49..=52 => {
                let temp_dat2 = ev.eventdat;
                let temp_dat = ev.eventdat3;
                let temp_dat3 = ev.eventdat6;
                {
                    let e = self.cur_ev_mut();
                    e.eventdat = 0;
                    e.eventdat3 = 0;
                    e.eventdat6 = 0;
                }
                // enemyDat[0] is modified in place ("custom" enemy).
                self.tables.enemies[0].armor = temp_dat3 as u8;
                self.tables.enemies[0].egraphic[0] = temp_dat2 as u16;
                let temp = match ev.eventtype - 48 {
                    1 => 25,
                    2 => 0,
                    3 => 50,
                    _ => 75,
                };
                self.create_new_event_enemy(0, temp, temp_dat as i16);
                let e = self.cur_ev_mut();
                e.eventdat = temp_dat2;
                e.eventdat3 = temp_dat;
                e.eventdat6 = temp_dat3;
            }
            56 => {
                self.create_new_event_enemy_at_y(75, 190);
            }
            _ => unreachable!("event {} not handled by event_spawn_enemy", ev.eventtype),
        }
    }

    /// Modification of the existing enemies of a link (or of all).
    fn event_modify_enemies(&mut self, ev: Event) {
        match ev.eventtype {
            19 => self.event_global_move(),
            20 => self.event_global_accel(),
            24 => {
                for t in 0..ENEMY_MAX {
                    let e = &mut self.enemy[t];
                    if e.linknum == ev.eventdat4 {
                        e.aniactive = 1;
                        e.aniwhenfire = 0;
                        if ev.eventdat2 > 0 {
                            e.enemycycle = ev.eventdat2 as u8;
                            e.animin = e.enemycycle;
                        } else {
                            e.enemycycle = 0;
                        }
                        if ev.eventdat > 0 {
                            e.ani = ev.eventdat as u8;
                        }
                        if ev.eventdat3 == 1 {
                            e.animax = e.ani;
                        } else if ev.eventdat3 == 2 {
                            e.aniactive = 2;
                            e.animax = e.ani;
                            e.aniwhenfire = 2;
                        }
                    }
                }
            }
            25 => {
                for t in 0..ENEMY_MAX {
                    if ev.eventdat4 == 0 || self.enemy[t].linknum == ev.eventdat4 {
                        // level_events.c case 25: in Galaga, roundf(eventdat * (difficultyLevel / 2)),
                        // integer product (roundf has no effect).
                        self.enemy[t].armorleft = if self.mode.galaga_mode {
                            (ev.eventdat as i32 * (self.difficulty as i32 / 2)) as u8
                        } else {
                            ev.eventdat as u8
                        };
                    }
                }
            }
            27 => {
                if ev.eventdat3 > 79 && ev.eventdat3 < 90 {
                    self.cur_ev_mut().eventdat4 = self.script.new_pl[(ev.eventdat3 - 80) as usize];
                }
                let ev = self.cur_ev();
                for t in 0..ENEMY_MAX {
                    let e = &mut self.enemy[t];
                    if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                        if ev.eventdat != -99 {
                            e.exrev = ev.eventdat as i8;
                        }
                        if ev.eventdat2 != -99 {
                            e.eyrev = ev.eventdat2 as i8;
                        }
                        if ev.eventdat3 != 0 && ev.eventdat3 < 17 {
                            e.filter = ev.eventdat3 as u8;
                        }
                    }
                }
            }
            31 => {
                for t in 0..ENEMY_MAX {
                    let e = &mut self.enemy[t];
                    if ev.eventdat4 == 99 || e.linknum == ev.eventdat4 {
                        e.freq = [ev.eventdat as u8, ev.eventdat2 as u8, ev.eventdat3 as u8];
                        e.eshotwait = [1, 1, 1];
                        if e.launchtype > 0 {
                            e.launchfreq = ev.eventdat5 as u8;
                            e.launchwait = 1;
                        }
                    }
                }
            }
            33 => {
                // Only the arcade / two-player / Super Tyrian modes modify the rule.
                if !((ev.eventdat == 512 || ev.eventdat == 513)
                    && (self.two_player_mode || self.one_player_action || self.mode.super_tyrian))
                {
                    if self.mode.super_arcade_mode != 0 {
                        if ev.eventdat == 534 {
                            self.cur_ev_mut().eventdat = 827;
                        }
                    } else if !self.mode.super_tyrian {
                        let lives = self.players[0].lives(0);
                        if ev.eventdat == 533 && (lives == 11 || ((self.rng.next_u32() % 15) as u8) < lives) {
                            let v = 829 + (self.rng.next_u32() % 6) as i16;
                            self.cur_ev_mut().eventdat = v;
                        }
                    }
                    if self.cur_ev().eventdat == 534 && self.mode.super_tyrian {
                        // level_events.c: superTyrianSpecials[4] = {1, 2, 4, 5}.
                        const SUPER_TYRIAN_SPECIALS: [i16; 4] = [1, 2, 4, 5];
                        let v = 828 + SUPER_TYRIAN_SPECIALS[(self.rng.next_u32() % 4) as usize];
                        self.cur_ev_mut().eventdat = v;
                    }
                    let ev = self.cur_ev();
                    for t in 0..ENEMY_MAX {
                        if self.enemy[t].linknum == ev.eventdat4 {
                            self.enemy[t].enemydie = ev.eventdat as u16;
                        }
                    }
                }
            }
            39 => {
                for t in 0..ENEMY_MAX {
                    if self.enemy[t].linknum as i16 == ev.eventdat {
                        self.enemy[t].linknum = ev.eventdat2 as u8;
                    }
                }
            }
            41 => {
                if ev.eventdat == 0 {
                    self.enemy_avail.iter_mut().for_each(|a| *a = 1);
                } else {
                    for x in 0..=24 {
                        self.enemy_avail[x] = 1;
                    }
                }
            }
            45 => {
                // level_events.c:798-816: nothing in Super Tyrian, neither drop nor RNG draw.
                if !self.mode.super_tyrian {
                    let lives = self.players[0].lives(0);
                    if ev.eventdat == 533 && (lives == 11 || ((self.rng.next_u32() % 15) as u8) < lives) {
                        let v = 829 + (self.rng.next_u32() % 6) as i16;
                        self.cur_ev_mut().eventdat = v;
                    }
                    let ev = self.cur_ev();
                    if self.two_player_mode || self.one_player_action {
                        for t in 0..ENEMY_MAX {
                            if self.enemy[t].linknum == ev.eventdat4 {
                                self.enemy[t].enemydie = ev.eventdat as u16;
                            }
                        }
                    }
                }
            }
            47 => {
                for t in 0..ENEMY_MAX {
                    if ev.eventdat4 == 0 || self.enemy[t].linknum == ev.eventdat4 {
                        self.enemy[t].armorleft = ev.eventdat as u8;
                    }
                }
            }
            55 => {
                if ev.eventdat3 > 79 && ev.eventdat3 < 90 {
                    self.cur_ev_mut().eventdat4 = self.script.new_pl[(ev.eventdat3 - 80) as usize];
                }
                let ev = self.cur_ev();
                for t in 0..ENEMY_MAX {
                    let e = &mut self.enemy[t];
                    if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                        if ev.eventdat != -99 {
                            e.xaccel = ev.eventdat as u8;
                        }
                        if ev.eventdat2 != -99 {
                            e.yaccel = ev.eventdat2 as u8;
                        }
                    }
                }
            }
            58 => {
                for t in 0..ENEMY_MAX {
                    if ev.eventdat4 == 99 || self.enemy[t].linknum == ev.eventdat4 {
                        self.enemy[t].launchtype = ev.eventdat as u16;
                    }
                }
            }
            59 | 68 => {
                let e_dat_i = ev.eventdat as u16;
                for t in 0..ENEMY_MAX {
                    if !(ev.eventdat4 == 0 || self.enemy[t].linknum == ev.eventdat4) {
                        continue;
                    }
                    let offset = t - (t % 25);
                    if let Some(nb) = self.new_enemy(offset, e_dat_i, 0) {
                        let (ex, ey) = (self.enemy[t].ex, self.enemy[t].ey);
                        self.enemy[nb].ex = ex;
                        self.enemy[nb].ey = ey;
                    }
                    self.enemy_avail[t] = 1;
                }
            }
            60 => {
                for t in 0..ENEMY_MAX {
                    let e = &mut self.enemy[t];
                    if e.linknum == ev.eventdat4 {
                        e.special = true;
                        e.flagnum = ev.eventdat as u8;
                        e.setto = ev.eventdat2 == 1;
                    }
                }
            }
            74 => {
                for t in 0..ENEMY_MAX {
                    let e = &mut self.enemy[t];
                    if ev.eventdat4 == 0 || e.linknum == ev.eventdat4 {
                        if ev.eventdat5 != -99 {
                            e.xminbounce = ev.eventdat5 as i16;
                        }
                        if ev.eventdat6 != -99 {
                            e.yminbounce = ev.eventdat6 as i16;
                        }
                        if ev.eventdat != -99 {
                            e.xmaxbounce = ev.eventdat;
                        }
                        if ev.eventdat2 != -99 {
                            e.ymaxbounce = ev.eventdat2;
                        }
                    }
                }
            }
            75 => self.event_random_link(),
            // level_events.c:JE_eventSystem case 85: enemy produced by other enemies (Timed Battle).
            85 => {
                if self.mode.timed != 0 {
                    for i in 0..ENEMY_MAX {
                        if self.enemy[i].linknum == ev.eventdat4 {
                            self.enemy[i].enemydie = ev.eventdat as u16;
                        }
                    }
                }
            }
            _ => unreachable!("event {} not handled by event_modify_enemies", ev.eventtype),
        }
    }

    /// Script flow: jumps, level end, timer, random spawn.
    fn event_script_flow(&mut self, ev: Event) {
        match ev.eventtype {
            11 => {
                if self.ending.all_players_gone || ev.eventdat == 1 {
                    self.ending.really_end_level = true;
                } else if !self.ending.end_level {
                    self.ending.ready_to_end_level = false;
                    self.ending.end_level = true;
                    self.ending.level_end = 40;
                }
            }
            13 => self.script.enemies_active = false,
            14 => self.script.enemies_active = true,
            36 => self.ending.ready_to_end_level = true,
            37 => self.script.level_enemy_frequency = ev.eventdat as u16,
            38 => {
                self.script.cur_loc = ev.eventdat as u16;
                let mut new_event_loc = 1usize;
                for w in 0..self.script.max_event as usize {
                    if self.ev(w).eventtime <= self.script.cur_loc {
                        new_event_loc = w + 1 - 1;
                    }
                }
                self.script.event_loc = new_event_loc as u16;
            }
            53 => self.script.force_events = ev.eventdat != 99,
            54 => self.event_jump(ev.eventdat as u16),
            57 => self.script.super_enemy_254_jump = ev.eventdat as u16,
            61 => {
                let flag = ev.eventdat;
                if (1..=10).contains(&flag) && self.script.global_flags[(flag - 1) as usize] as i16 == ev.eventdat2 {
                    self.event_jump_relative(ev.eventdat3 as i32);
                }
            }
            62 => {
                // level_events.c: scripted effect (1..40), same guard as event 16.
                let raw = ev.eventdat;
                if (1..=crate::sound::SOUND_COUNT as i16).contains(&raw)
                    && self.voice_dedup_allow(raw as u8, self.script.event_loc)
                {
                    self.queue_sound(3, raw as u8);
                }
            }
            63 => {
                if !self.two_player_mode && !self.one_player_action {
                    self.event_jump_relative(ev.eventdat as i32);
                }
            }
            66 => {
                if self.initial_difficulty as i32 <= ev.eventdat as i32 {
                    self.event_jump_relative(ev.eventdat2 as i32);
                }
            }
            67 => {
                self.script.level_timer = ev.eventdat == 1;
                self.script.level_timer_countdown = (ev.eventdat3 as i32 * 100) as u16;
                self.script.level_timer_jump_to = ev.eventdat2 as u16;
            }
            70 => {
                if ev.eventdat2 == 0 {
                    let mut found = false;
                    for t in 1..=19u8 {
                        found = found || self.search_for(t).is_some();
                    }
                    if !found {
                        self.event_jump(ev.eventdat as u16);
                    }
                } else if self.search_for(ev.eventdat2 as u8).is_none()
                    && (ev.eventdat3 == 0 || self.search_for(ev.eventdat3 as u8).is_none())
                    && (ev.eventdat4 == 0 || self.search_for(ev.eventdat4).is_none())
                {
                    self.event_jump(ev.eventdat as u16);
                }
            }
            71 => {
                // ((mapYPos - mainmap) / sizeof(ptr)) * 2 <= (unsigned) eventdat2
                let lhs = (self.background.map_y_pos[0] as i64 * 2) as u64;
                if lhs <= ev.eventdat2 as i32 as u32 as u64 {
                    self.event_jump(ev.eventdat as u16);
                }
            }
            76 => self.script.return_active = true,
            80 => {
                if self.two_player_mode {
                    self.event_jump_relative(ev.eventdat as i32);
                }
            }
            // level_events.c:JE_eventSystem case 84: Timed Battle timer (copy of event 67).
            84 => {
                if self.mode.timed != 0 {
                    self.script.level_timer = ev.eventdat == 1;
                    self.script.level_timer_countdown = (ev.eventdat3 as i32 * 100) as u16;
                    self.script.level_timer_jump_to = ev.eventdat2 as u16;
                }
            }
            _ => unreachable!("event {} not handled by event_script_flow", ev.eventtype),
        }
    }

    /// Level settings: difficulty, damage, filters, music, special weapon, boss.
    fn event_level_settings(&mut self, ev: Event) {
        match ev.eventtype {
            26 => self.script.small_enemy_adjust = ev.eventdat != 0,
            // level_events.c, case 34 (Start Music Fade) and 35 (Play new song).
            34 => {
                if self.ending.first_game_over {
                    self.music.music_fade = true;
                    self.music.temp_volume = self.music.tyr_music_volume;
                }
            }
            35 => {
                if self.ending.first_game_over {
                    self.push_music_cue(MusicCue::PlaySong(ev.eventdat as i32 - 1));
                    self.push_music_cue(MusicCue::SetVolume(self.music.tyr_music_volume));
                }
                self.music.music_fade = false;
            }
            40 => self.script.enemy_continual_damage = true,
            44 => {
                self.filter.filter_active = ev.eventdat > 0;
                self.filter.filter_fade = ev.eventdat == 2;
                self.filter.level_filter = ev.eventdat2 as i8;
                self.filter.level_brightness = ev.eventdat3;
                self.filter.level_filter_new = ev.eventdat4 as i8;
                self.filter.level_brightness_chg = ev.eventdat5;
                self.filter.filter_fade_start = ev.eventdat6 == 0;
            }
            46 => {
                if ev.eventdat3 != 0 {
                    self.script.damage_rate = ev.eventdat3 as u8;
                }
                if ev.eventdat2 == 0 || self.two_player_mode || self.one_player_action {
                    let d = (self.difficulty as i32 + ev.eventdat as i32) as i8;
                    self.difficulty = d.clamp(DIFFICULTY_EASY, DIFFICULTY_10);
                }
            }
            69 => {
                for p in self.players.iter_mut() {
                    p.invulnerable_ticks = ev.eventdat as i32 as u32;
                }
            }
            78 => {
                if self.script.galaga_shot_freq < 10 {
                    self.script.galaga_shot_freq += 1;
                }
            }
            79 => {
                self.script.boss_bar[0].link_num = ev.eventdat as u8;
                self.script.boss_bar[1].link_num = ev.eventdat2 as u8;
                self.ach_events.boss_spawns += 1;
            }
            82 => {
                if (0..=crate::tables::SPECIAL_NUM as i16).contains(&ev.eventdat) {
                    self.players[0].items.special = SpecialId(ev.eventdat as u8);
                    self.players[0].shot_multi_pos[SHOT_SPECIAL] = 0;
                    self.shot_repeat[SHOT_SPECIAL] = 0;
                    self.players[0].shot_multi_pos[SHOT_SPECIAL2] = 0;
                    self.shot_repeat[SHOT_SPECIAL2] = 0;
                }
            }
            99 => self.script.random_explosions = ev.eventdat == 1,
            _ => unreachable!("event {} not handled by event_level_settings", ev.eventtype),
        }
    }

    /// Event 19: speed imposed on a group of enemies.
    fn event_global_move(&mut self) {
        let ev = self.cur_ev();
        let (initial_i, max_i, all_enemies);
        if ev.eventdat3 > 79 && ev.eventdat3 < 90 {
            initial_i = 0;
            max_i = 100;
            all_enemies = false;
            self.cur_ev_mut().eventdat4 = self.script.new_pl[(ev.eventdat3 - 80) as usize];
        } else {
            (initial_i, max_i, all_enemies) = match ev.eventdat3 {
                0 => (0, 100, false),
                2 => (0, 25, true),
                1 => (25, 50, true),
                3 => (50, 75, true),
                99 => (0, 100, true),
                _ => (0, 0, false),
            };
        }
        let ev = self.cur_ev();
        for i in initial_i..max_i {
            let e = &mut self.enemy[i];
            if all_enemies || e.linknum == ev.eventdat4 {
                if ev.eventdat != -99 {
                    e.exc = ev.eventdat as i8;
                }
                if ev.eventdat2 != -99 {
                    e.eyc = ev.eventdat2 as i8;
                }
                if ev.eventdat6 != 0 {
                    e.fixedmovey = ev.eventdat6 as i16;
                }
                if ev.eventdat6 == -99 {
                    e.fixedmovey = 0;
                }
                if ev.eventdat5 > 0 {
                    e.enemycycle = ev.eventdat5 as u8;
                }
            }
        }
    }

    /// Event 20: cyclic acceleration of a group of enemies.
    fn event_global_accel(&mut self) {
        let ev = self.cur_ev();
        if ev.eventdat3 > 79 && ev.eventdat3 < 90 {
            self.cur_ev_mut().eventdat4 = self.script.new_pl[(ev.eventdat3 - 80) as usize];
        }
        let ev = self.cur_ev();
        for t in 0..ENEMY_MAX {
            if self.enemy_avail[t] != 1 && (self.enemy[t].linknum == ev.eventdat4 || ev.eventdat4 == 0) {
                let e = &mut self.enemy[t];
                if ev.eventdat != -99 {
                    e.excc = ev.eventdat as i8;
                    e.exccw = (ev.eventdat as i32).abs() as i8;
                    e.exccwmax = (ev.eventdat as i32).unsigned_abs() as u8;
                    e.exccadd = if ev.eventdat > 0 { 1 } else { -1 };
                }
                if ev.eventdat2 != -99 {
                    e.eycc = ev.eventdat2 as i8;
                    e.eyccw = (ev.eventdat2 as i32).abs() as i8;
                    e.eyccwmax = (ev.eventdat2 as i32).unsigned_abs() as u8;
                    e.eyccadd = if ev.eventdat2 > 0 { 1 } else { -1 };
                }
                if ev.eventdat5 > 0 {
                    e.enemycycle = ev.eventdat5 as u8;
                }
                if ev.eventdat6 > 0 {
                    e.ani = ev.eventdat6 as u8;
                    e.animin = ev.eventdat5 as u8;
                    e.animax = 0;
                    e.aniactive = 1;
                }
            }
        }
    }

    /// Event 75: random choice of a link among the stationary enemies.
    fn event_random_link(&mut self) {
        let ev = self.cur_ev();
        let mut any = false;
        for t in 0..ENEMY_MAX {
            let e = &self.enemy[t];
            if self.enemy_avail[t] == 0
                && e.eyc == 0
                && e.linknum as i16 >= ev.eventdat
                && e.linknum as i16 <= ev.eventdat2
            {
                any = true;
            }
        }
        if any {
            let span = (ev.eventdat2 as i32 + 1 - ev.eventdat as i32) as u32;
            let mut temp: u8;
            loop {
                temp = ((self.rng.next_u32() % span) as i32 + ev.eventdat as i32) as u8;
                if let Some(i) = self.search_for(temp) {
                    if self.enemy[i].eyc == 0 {
                        break;
                    }
                }
            }
            if ev.eventdat3 > 79 && ev.eventdat3 < 90 {
                self.script.new_pl[(ev.eventdat3 - 80) as usize] = temp;
            }
        } else {
            if ev.eventdat3 > 79 && ev.eventdat3 < 90 {
                self.script.new_pl[(ev.eventdat3 - 80) as usize] = 255;
            }
            if ev.eventdat4 > 0 {
                let mut skip = self.script.event_loc as usize - 1 + ev.eventdat4 as usize;
                if skip >= EVENT_MAXIMUM {
                    skip = EVENT_MAXIMUM - 1;
                }
                self.script.cur_loc = self.ev(skip).eventtime.wrapping_sub(1);
                self.event_jump_relative(ev.eventdat4 as i32 - 1);
            }
        }
    }
}
