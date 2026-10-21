//! Level loop: `tyrian2.c:JE_main`, from `start_level_first` to
//! `level_loop`, split at the golden master injection point
//! (`golden_master_inject`, right after `JE_playerCollide`).
//!
//! - [`World::start_level`]: initialization (`tyrian2.c` lines ~370-760);
//! - [`World::loop_top`]: start of `level_loop` up to the collisions;
//! - [`World::loop_bottom`]: from `JE_mainGamePlayerFunctions` to the end of the iteration.

use crate::rng::{MersenneTwister, Xoshiro256};
use crate::tables::Tables;
use crate::world::WeaponSlot;
use crate::world::*;
use crate::{LevelSetup, PlayerInput};
use tyrian_formats::lvl::Event;

/// Layer height in rows and width in cells (`varz.h:JE_MapType*`).
pub const LAYER_WIDTH: [i32; 3] = [14, 14, 15];

impl World {
    /// Builds the state at the moment of `golden_master_level_start` (right before
    /// `level_loop`).
    pub fn start_level(setup: &LevelSetup) -> World {
        let mut tables = Tables::new(&setup.items);
        tables.apply(&setup.patch);
        let lvl = &setup.level.events;
        // level_load.c:JE_loadMap — events capped at EVENT_MAXIMUM, then
        // sentinel (eventtime 65500, rest at zero).
        let max_event = lvl.events.len().min(EVENT_MAXIMUM - 1) as u16;
        let zero = Event {
            eventtime: 0,
            eventtype: 0,
            eventdat: 0,
            eventdat2: 0,
            eventdat3: 0,
            eventdat5: 0,
            eventdat6: 0,
            eventdat4: 0,
        };
        let mut event_rec = vec![zero; EVENT_MAXIMUM];
        for (i, e) in lvl.events.iter().take(max_event as usize).enumerate() {
            event_rec[i] = *e;
        }
        event_rec[max_event as usize].eventtime = 65500;
        let level_enemy: Vec<u16> = lvl.enemies.iter().take(40).copied().collect();
        let level_enemy_max = level_enemy.len() as u16;

        let lo = setup.loadout;
        // mainint.c:JE_initPlayerData then opentyr.c:apply_goto_spec.
        let mut items0 = Items {
            ship: lo.ship,
            generator: lo.generator,
            shield: lo.shield,
            weapon: [
                WeaponSlot { id: lo.front_weapon, power: lo.front_power },
                WeaponSlot { id: lo.rear_weapon, power: lo.rear_power },
            ],
            sidekick: [lo.left_sidekick, lo.right_sidekick],
            special: lo.special,
            sidekick_series: 0,
            sidekick_level: 0,
        };
        let mut items1 = match setup.second_player_loadout {
            Some(l) => Items {
                ship: l.ship,
                generator: l.generator,
                shield: l.shield,
                weapon: [
                    WeaponSlot { id: l.front_weapon, power: l.front_power },
                    WeaponSlot { id: l.rear_weapon, power: l.rear_power },
                ],
                sidekick: [l.left_sidekick, l.right_sidekick],
                special: l.special,
                sidekick_series: 0,
                sidekick_level: 0,
            },
            None => items0,
        };
        items1.weapon = [WeaponSlot { id: items1.weapon[0].id, power: 1 }, WeaponSlot { id: WeaponId(15), power: 1 }];
        items1.sidekick_level = 101;
        items1.sidekick_series = 0;
        items0.sidekick_series = 0;
        let m = setup.modes;
        if let Some(l2) = m.loadout2 {
            // Golden master v2: player 2 equipment as recorded (mainint.c:JE_initPlayerData,
            // level_load.c "]g[" for Galaga, two-player menus).
            items1.ship = l2.ship;
            items1.generator = l2.generator;
            items1.shield = l2.shield;
            items1.weapon = [
                WeaponSlot { id: l2.front_weapon, power: l2.front_power },
                WeaponSlot { id: l2.rear_weapon, power: l2.rear_power },
            ];
            items1.sidekick = [l2.left_sidekick, l2.right_sidekick];
            items1.special = l2.special;
        }
        // endless.c:endless_start / endless_resume: "memset(&player[p].items, 0, …)"
        // without JE_initPlayerData; player 2's sidekick level stays at 0 (no
        // indicator of varz.c:JE_drawOptionLevel lit), not at 101 as in the campaign.
        if m.endless_lives.is_some() {
            items1.sidekick_level = 0;
            // Next wave: the level acquired by player 2 is kept (endless.c:849
            // only resets it to zero at the start of the run).
            if let Some(c) = setup.carry_over {
                items1.sidekick_level = c.players[1].sidekick_level;
                items1.sidekick_series = c.players[1].sidekick_series;
            }
        }

        // tyrian2.c:JE_main — difficulty: episode 5 removes one notch, floor EASY.
        let mut difficulty = setup.difficulty as i8;
        if setup.episode == 5 {
            difficulty -= 1;
        }
        if difficulty < DIFFICULTY_EASY {
            difficulty = DIFFICULTY_EASY;
        }
        // tyrian2.c:JE_main: "if (galagaMode) difficultyLevel = DIFFICULTY_NORMAL".
        if m.galaga {
            difficulty = DIFFICULTY_NORMAL;
        }

        let mut players = [Player::default(); 2];
        for (i, p) in players.iter_mut().enumerate() {
            p.items = if i == 0 { items0 } else { items1 };
            p.is_dragonwing = i == 1;
            p.weapon_mode = 1;
            p.is_alive = true;
            p.x = if i == 0 { 100 } else { 190 };
            p.y = 180;
            for j in 0..20 {
                p.old_x[j] = p.x - (19 - j as i32);
                p.old_y[j] = p.y - 18;
            }
            p.last_x_shot_move = p.x;
            p.last_y_shot_move = p.y;
            p.invulnerable_ticks = 100;
        }
        players[0].cash = lo.cash as u64;
        if let Some(carry) = setup.carry_over {
            for (p, c) in players.iter_mut().zip(carry.players) {
                p.x_friction_ticks = c.x_friction_ticks;
                p.y_friction_ticks = c.y_friction_ticks;
            }
        }
        if m.loadout2.is_some() {
            players[1].cash = m.cash.1;
        }

        // varz.c:JE_getShipInfo.
        let ship = tables.ships[lo.ship.index().min(crate::tables::SHIP_NUM)].clone();
        players[0].armor = ship.dmg as u32;
        players[1].armor = 10;
        let ship_gr = ship.shipgraphic;
        for p in players.iter_mut().take(2) {
            p.initial_armor = p.armor;
            let ani = tables.ships[p.items.ship.index().min(crate::tables::SHIP_NUM)].ani;
            if ani == 0 {
                p.shot_hit_area_x = 12;
                p.shot_hit_area_y = 10;
            } else {
                p.shot_hit_area_x = 11;
                p.shot_hit_area_y = 14;
            }
        }
        let power_add = tables.generators[lo.generator.index().min(crate::tables::POWER_NUM)].power as u32;
        // tyrian2.c: shields.
        let shield_t = tables.shields[lo.shield.index().min(crate::tables::SHIELD_NUM)].tpwr as u32 * 20;
        for p in players.iter_mut() {
            p.shield = tables.shields[p.items.shield.index().min(crate::tables::SHIELD_NUM)].mpwr as u32;
            p.shield_max = p.shield * 2;
        }

        let map_y = [300 - 8, 600 - 8, 600 - 8];
        let mut w = World {
            tables,
            rng: Xoshiro256::new(setup.seed),
            mt: MersenneTwister::new(setup.seed as u32),
            episode: setup.episode,
            difficulty,
            initial_difficulty: setup.initial_difficulty.unwrap_or(setup.difficulty) as i8,
            assist: setup.assist,
            autopilot_stalled: 0,
            autopilot_last_armor_sum: 0,
            autopilot_no_damage: 0,
            processor_type: setup.processor_type,
            endless_armor_mult: setup.endless_armor_mult,
            players,
            two_player_mode: false,
            mode: crate::modes::ModeState::default(),
            one_player_action: false,
            endless_lives: None,
            demo: None,
            bonus_level_current: false,
            endless_score: 0,
            combo_multiplier: 1,
            combo: crate::combos::ComboState::default(),
            ship_gr,
            explosion_follow_amount_x: 0,
            explosion_follow_amount_y: 0,
            armor_ship_delay: 50,
            // tyrian2.c: memset(shotRepeat, 1, ...).
            shot_repeat: [1; 11],
            enemy: vec![Enemy::default(); ENEMY_MAX],
            enemy_avail: vec![1; ENEMY_MAX],
            enemy_on_screen: 0,
            total_enemy: 0,
            enemy_killed: 0,
            player_shots: vec![PlayerShot::default(); MAX_PWEAPON + 1],
            shot_avail: vec![0; MAX_PWEAPON],
            enemy_shots: vec![EnemyShot::default(); ENEMY_SHOT_MAX],
            enemy_shot_avail: vec![true; ENEMY_SHOT_MAX],
            explosions: vec![Explosion::default(); MAX_EXPLOSIONS],
            rep_explosions: vec![RepExplosion::default(); MAX_REPEATING_EXPLOSIONS],
            enemy_still_exploding: false,
            hitstop: Default::default(),
            fx: Default::default(),
            ach_events: AchEvents::default(),
            ach_events_at_check: AchEvents::default(),
            level_stats: crate::world::LevelStats { credits_start: players[0].cash, ..Default::default() },
            iteration: 0,
            background: Background {
                smoothies: [false; 9],
                smoothie_data: [0; 9],
                star_active: true,
                starfield_speed: 1,
                back_pos: [0; 3],
                back_move: [1, 2, 3],
                map_x: lvl.header.map_x,
                map_x2: lvl.header.map_x2,
                map_x3: lvl.header.map_x3,
                map_y,
                map_y_pos: [map_y[0] as i32 * 14 - 1, map_y[1] as i32 * 14 - 1, map_y[2] as i32 * 15 - 1],
                bk_wrap: [14, 14, 15],
                bk_wrap_to: [14, 14, 15],
                map_x_pos: 0,
                old_map_x_ofs: 0,
                map_x_ofs: 0,
                map_x2_ofs: 0,
                map_x2_pos: 0,
                map_x3_pos: 0,
                old_map_x3_ofs: 0,
                map_x3_ofs: 0,
                temp_map_x_ofs: 0,
                map_x_bp_pos: 0,
                map_x2_bp_pos: 0,
                map_x3_bp_pos: 0,
                map1_y_delay: 1,
                map1_y_delay_max: 1,
                map2_y_delay: 1,
                map2_y_delay_max: 1,
                background2over: 1,
                background3over: 0,
                background3x1: false,
                background3x1b: false,
                background2_not_transparent: false,
                top_enemy_over: false,
                sky_enemy_over_all: false,
                stop_backgrounds: false,
                stop_background_num: 0,
                explode_move: 2,
                temp_back_move: 0,
            },
            script: EventScript {
                event_rec,
                max_event,
                event_loc: 1,
                cur_loc: 0,
                return_loc: 0,
                level_enemy,
                level_enemy_max,
                level_enemy_frequency: 96,
                enemies_active: true,
                new_pl: [0; 10],
                global_flags: [false; 10],
                force_events: false,
                super_enemy_254_jump: 0,
                random_explosions: false,
                return_active: false,
                level_timer: false,
                level_timer_countdown: 0,
                level_timer_jump_to: 0,
                enemy_sprite_sheet_ids: [0; 4],
                small_enemy_adjust: false,
                enemy_continual_damage: false,
                galaga_shot_freq: 0,
                boss_bar: [BossBar::default(); 2],
                damage_rate: 2,
            },
            ending: LevelEnding {
                end_level: false,
                really_end_level: false,
                ready_to_end_level: false,
                wait_to_end_level: false,
                level_end: 255,
                level_end_warp: -4,
                level_end_fx_wait: 0,
                first_game_over: true,
                all_players_gone: false,
                bonus_level: false,
                next_level: 0,
                display_time: 0,
                cube_max: 0,
            },
            special_weapon: SpecialWeaponState {
                special_wait: 0,
                next_special_wait: 0,
                flare_duration: 0,
                flare_start: false,
                flare_col_chg: 0,
                zinglon_duration: 0,
                astral_duration: 0,
                special_weapon_filter: 0,
                special_weapon_freq: 0,
                special_weapon_wpn: 0,
                link_to_player: false,
                spray_special: false,
                fire_button_held: setup.carry_over.is_some_and(|c| c.special_fire_held),
            },
            filter: LevelFilter {
                level_filter: -99,
                level_brightness: -14,
                filter_active: true,
                // tyrian2.c:JE_main ("Filter Status"): opening fade.
                filter_fade: true,
                filter_fade_start: false,
                level_filter_new: 0,
                level_brightness_chg: 1,
            },
            energy: Energy { power: 0, power_add, shield_wait: 1, shield_t },
            sidekick_formation: SidekickFormation {
                option_attachment_move: 0,
                option_attachment_linked: true,
                option_attachment_return: false,
                option_satellite_rotate: crate::f32ops::F32(0),
            },
            controls: Controls {
                mouse_x: 0,
                mouse_y: 0,
                button: [false; 4],
                move_ok: false,
                sf_current_code: [[0; 21]; 2],
                sf_executed: [0; 2],
                port_config_change: false,
                port_config_done: false,
            },
            render: RenderOnly {
                hud_power: 0,
                drawn_layer: [None; 3],
                superpixels: [Superpixel::default(); MAX_SUPERPIXELS],
                // Golden master: JE_main calls initialize_starfield before
                // golden_master_level_start re-seeds; the MT generator is then
                // at the default seed of mtrand.c (5489, first level).
                starfield: initialize_starfield(setup.processor_type, &mut MersenneTwister::new(5489)),
                starfield_drawn: false,
                // The C code draws one frame (neat += 3 then ++) before golden master tick 0.
                superwild_neat: 4,
                drawn_neat: (0, 0),
                last_superpixel: 0,
                drawn_invulnerable: [false; 2],
                drawn_special_icon: 0,
                drawn_zinglon: Vec::new(),
                clock_offset_ms: 0,
                drawn_sidekick_charge: [[0; 2]; 2],
                drawn_texts: crate::world::DrawnTexts {
                    warning_col: 120,
                    warning_col_change: 1,
                    flash_change: 1,
                    ..Default::default()
                },
                charge_gr: 0,
                charge_gr_wait: 3,
                drawn_charge_sprite: 0,
                hud_ops: Vec::new(),
                hud_ops_next: Vec::new(),
                drawn_filter: None,
                star_show_code: 0,
                drawn_explosions: Vec::new(),
                drawn_enemies: Vec::new(),
                drawn_player_shots: Vec::new(),
                drawn_enemy_shots: Vec::new(),
                fx_events: Vec::new(),
            },
            sound: crate::sound::SoundQueue::new(),
            music: MusicState {
                music_fade: false,
                temp_volume: 0,
                tyr_music_volume: DEFAULT_MUSIC_VOLUME,
                music_cues: Vec::new(),
                music_level_song_branch: false,
            },
            dev: crate::dev::DevState::default(),
        };
        // Side modes: tyrian2.c:JE_main ("if (galagaMode) twoPlayerMode = false"),
        // level_load.c (]g[, ]e[), mainint.c:JE_initPlayerData.
        w.two_player_mode = m.two_player && !m.galaga;
        w.one_player_action = m.one_player_action;
        w.endless_lives = m.endless_lives;
        w.mode.galaga_mode = m.galaga;
        w.mode.super_arcade_mode = m.super_arcade;
        w.mode.timed = m.timed;
        w.mode.super_tyrian = m.super_tyrian;
        w.mode.mutators = m.mutators;
        w.mode.anomalies = m.anomalies;
        // varz.c:JE_getShipInfo: "glass hull" anomaly, player 1 armor at 1.
        if w.mode.anomaly(crate::modes::ANOM_GLASS_HULL) {
            w.players[0].initial_armor = 1;
            w.players[0].armor = 1;
        }
        if m.loadout2.is_some() || m.devices != (0, 0) {
            w.mode.devices = [m.devices.0, m.devices.1];
        } else {
            // A player with no device chosen: "all" (0), like the mouse that
            // steered the ship before c-final; the C code defaults to 1 (keyboard),
            // documented divergence (docs/divergences.md).
            w.mode.devices = [0, 0];
        }
        // endless.c:800-801: initialDifficulty frozen at the start of the run (1 or 2 players);
        // achievements.c:963-966 unlocks Hard / Impossible on this value.
        if m.loadout2.is_some() || m.endless_lives.is_some() {
            w.initial_difficulty = m.initial_difficulty as i8;
        }
        // tyrian2.c:JE_main calls JE_drawArmor after JE_getShipInfo: armor capped at 28
        // (initial_armor keeps the ship's value).
        w.draw_armor_clamp();
        // varz.c:JE_drawOptions and player.c:calc_purple_balls_needed.
        w.draw_options();
        w.calc_purple_balls_needed(0);
        w.calc_purple_balls_needed(1);
        // tyrian2.c:JE_main: `JE_drawArmor()` after the shields brings
        // armor down to 28 at most (ships with `dmg` > 28, e.g. Nort).
        w.draw_armor_clamp();
        w
    }

    /// `golden_master.c:apply_assist` (copy of the aids of `autopilot.c:autopilot_drive`).
    pub fn apply_assist(&mut self) {
        let p = &mut self.players[0];
        if p.armor < p.initial_armor {
            p.armor = p.initial_armor;
        }
        if p.shield < p.shield_max {
            p.shield = p.shield_max;
        }
        p.invulnerable_ticks = 2;
        if self.two_player_mode {
            self.players[1].invulnerable_ticks = 2;
        }
        let p = &mut self.players[0];
        if p.items.weapon[0].power < 11 {
            p.items.weapon[0].power = 11;
        }
        if p.items.weapon[1].id != WeaponId(0) && p.items.weapon[1].power < 11 {
            p.items.weapon[1].power = 11;
        }
        self.autopilot_abandon();
    }

    /// `autopilot.c:autopilot_drive`, abandoning a stuck level: the C pilot ends
    /// the level itself (`levelTimer = true; levelTimerCountdown = 0; endLevel = true;
    /// levelEnd = 40`) when the map stays frozen for 12,000 ticks, or frozen for 1,400 ticks with
    /// 3,150 ticks without any change in enemy armor. Recordings made with
    /// `GOLDEN_INPUT=autopilot` (DREAD-NOT, ASSASSIN) contain this forced exit:
    /// 30 ticks before the end of the level (`levelEnd` = 40 → exit flight).
    fn autopilot_abandon(&mut self) {
        if self.background.back_move[0] != 0 {
            self.autopilot_stalled = 0;
        } else if self.autopilot_stalled < (1 << 20) {
            self.autopilot_stalled += 1;
        }
        let mut armor_sum: i64 = 0;
        for e in 0..ENEMY_MAX {
            if self.enemy_avail[e] == 0 && self.enemy[e].armorleft != 0 {
                armor_sum += self.enemy[e].armorleft as i64;
            }
        }
        if armor_sum != self.autopilot_last_armor_sum {
            self.autopilot_last_armor_sum = armor_sum;
            self.autopilot_no_damage = 0;
        } else if self.autopilot_no_damage < (1 << 20) {
            self.autopilot_no_damage += 1;
        }
        if self.autopilot_stalled > 200 {
            let rien_ne_meurt = self.autopilot_stalled > 1400 && self.autopilot_no_damage == 3150;
            let fige_trop_longtemps = self.autopilot_stalled == 12000;
            if rien_ne_meurt || fige_trop_longtemps {
                self.script.level_timer = true;
                self.script.level_timer_countdown = 0;
                self.ending.end_level = true;
                self.ending.level_end = 40;
            }
        }
    }

    /// `backgrnd.c:draw_background_2` / `draw_background_2_blend`: only the
    /// movement part.
    fn background_2_move(&mut self) {
        if self.background.map2_y_delay_max > 1 && self.background.back_move[1] < 2 {
            self.background.back_move[1] = if self.background.map2_y_delay == 1 { 1 } else { 0 };
        }
        if self.background.map_y_pos[1] < LAYER_WIDTH[1] {
            self.background.map_y_pos[1] = LAYER_WIDTH[1];
        }
        self.render.drawn_layer[1] = Some((self.background.map_y_pos[1], self.background.back_pos[1]));
        self.background.map2_y_delay = self.background.map2_y_delay.wrapping_sub(1);
        if self.background.map2_y_delay == 0 {
            self.background.map2_y_delay = self.background.map2_y_delay_max;
            self.background.back_pos[1] = self.background.back_pos[1].wrapping_add(self.background.back_move[1]);
            if self.background.back_pos[1] > 27 {
                self.background.back_pos[1] -= 28;
                self.background.map_y[1] = self.background.map_y[1].wrapping_sub(1);
                self.background.map_y_pos[1] -= 14;
            }
        }
    }

    /// `backgrnd.c:draw_background_3`: movement of layer 3.
    fn background_3_move(&mut self) {
        self.background.back_pos[2] = self.background.back_pos[2].wrapping_add(self.background.back_move[2]);
        if self.background.back_pos[2] > 27 {
            self.background.back_pos[2] -= 28;
            self.background.map_y[2] = self.background.map_y[2].wrapping_sub(1);
            self.background.map_y_pos[2] -= 15;
        }
        if self.background.map_y_pos[2] < LAYER_WIDTH[2] {
            self.background.map_y_pos[2] = LAYER_WIDTH[2];
        }
        self.render.drawn_layer[2] = Some((self.background.map_y_pos[2], self.background.back_pos[2]));
    }

    /// Common condition of the `draw_background_2` calls in `tyrian2.c`
    /// (`tyrian2.c:1096,1145,1298`): `!(smoothies[2-1] && processorType < 4)
    /// && !(smoothies[1-1] && processorType == 3)`. True at processorType = 4
    /// (default setting): water and lava do not block layer 2 at this
    /// detail level, verified by endless-arcade-seed7, SAVARA V.
    pub fn background_2_allowed(&self) -> bool {
        let pt = self.processor_type as i32;
        let sm = &self.background.smoothies;
        !(sm[1] && pt < 4) && !(sm[0] && pt == 3)
    }

    /// Start of `level_loop` up to and including `JE_playerCollide`.
    pub fn loop_top(&mut self) {
        let before = self.render.hud_ops.len();
        self.loop_top_body();
        let ops = &mut self.render.hud_ops;
        let later = if ops.len() >= before { ops.split_off(before) } else { std::mem::take(ops) };
        self.render.hud_ops_next.extend(later);
    }

    fn loop_top_body(&mut self) {
        self.iteration += 1;
        // Rendering only: enemy drawings of this iteration (the late passes
        // of loop_bottom are added to them).
        self.render.drawn_enemies.clear();
        // tyrian2.c: starShowVGASpecialCode = smoothies[9-1] + (smoothies[6-1] << 1).
        self.render.star_show_code = self.background.smoothies[8] as u8 + ((self.background.smoothies[5] as u8) << 1);
        for l in 0..3 {
            if self.background.map_y_pos[l] <= self.background.bk_wrap[l] {
                self.background.map_y_pos[l] = self.background.bk_wrap_to[l];
            }
        }
        {
            // tyrian2.c:JE_main: in Endless, "no lives left" = endless_lives_of(p) <= 1.
            let (p0_no_lives, p1_no_lives) = match self.endless_lives {
                Some(l) => (l[0] <= 1, l[1] <= 1),
                None => (self.players[0].lives(0) == 1, self.players[1].lives(1) == 1),
            };
            self.ending.all_players_gone = self.all_players_dead()
                && ((p0_no_lives && self.players[0].exploding_ticks == 0)
                    || (!self.one_player_action && !self.two_player_mode))
                && ((p1_no_lives && self.players[1].exploding_ticks == 0) || !self.two_player_mode);
        }
        self.music_fade_update();
        if !self.ending.end_level {
            // Shield recharge (solo game).
            if self.mode.galaga_mode {
                // tyrian2.c, "Shield Gen" HUD in Galaga: shields at zero, Dragonwing
                // lost if no more lives or armor, bonus life per money tier.
                for p in self.players.iter_mut() {
                    p.shield = 0;
                }
                if self.players[1].lives(1) == 0 || self.players[1].armor == 0 {
                    self.two_player_mode = false;
                }
                if self.players[0].cash >= self.mode.galaga_life as u64 {
                    self.queue_sound(6, crate::sound::S_EXPLOSION_11);
                    self.queue_sound(7, crate::sound::S_SOUL_OF_ZINGLON);
                    if self.players[0].lives(0) < 11 {
                        self.players[0].items.weapon[0].power += 1;
                    } else {
                        self.players[0].cash += 1000;
                    }
                    self.mode.galaga_life =
                        if self.mode.galaga_life == 10000 { 20000 } else { self.mode.galaga_life + 25000 };
                }
            } else if self.two_player_mode {
                self.energy.shield_wait = self.energy.shield_wait.wrapping_sub(1);
                if self.energy.shield_wait == 0 {
                    self.energy.shield_wait = 15;
                    for p in self.players.iter_mut() {
                        if p.shield < p.shield_max && p.is_alive {
                            p.shield += 1;
                        }
                    }
                    self.hud_bars(false, true, false);
                }
            } else if self.players[0].is_alive
                && self.players[0].shield < self.players[0].shield_max
                && self.energy.power > self.energy.shield_t
            {
                self.energy.shield_wait = self.energy.shield_wait.wrapping_sub(1);
                if self.energy.shield_wait == 0 {
                    self.energy.shield_wait = 15;
                    self.energy.power -= self.energy.shield_t;
                    self.players[0].shield += 1;
                    if self.players[1].shield < self.players[0].shield_max {
                        self.players[1].shield += 1;
                    }
                    self.hud_bars(false, true, false);
                }
            }
            if self.two_player_mode || self.one_player_action {
                self.energy.power = 900;
            } else {
                self.energy.power += self.energy.power_add;
                if self.energy.power > 900 {
                    self.energy.power = 900;
                }
            }
            self.render.hud_power = self.energy.power;
            self.background.old_map_x3_ofs = self.background.map_x3_ofs;
            self.enemy_on_screen = 0;
        }
        // Work limit of the interpreter, not of the destinations: a conditional
        // loop can revisit an event and then finish. The guard
        // consumes no RNG draw and neither reorders nor defers events.
        const EVENT_WORK_LIMIT: usize = 1_000_000;
        let mut event_work = 0;
        while self.ev(self.cur_ev_index()).eventtime <= self.script.cur_loc
            && self.script.event_loc <= self.script.max_event
        {
            if event_work == EVENT_WORK_LIMIT {
                self.stop_event_script("per-tick event work limit exceeded (possible cycle)");
                return;
            }
            event_work += 1;
            self.event_system();
        }
        // Layer 1.
        if self.script.force_events && self.background.back_move[0] == 0 {
            self.script.cur_loc = self.script.cur_loc.wrapping_add(1);
        }
        if self.background.map1_y_delay_max > 1 && self.background.back_move[0] < 2 {
            self.background.back_move[0] = if self.background.map1_y_delay == 1 { 1 } else { 0 };
        }
        if self.special_weapon.astral_duration == 0 && self.background.map_y_pos[0] < LAYER_WIDTH[0] {
            // backgrnd.c:draw_background_1 — BG_CLAMP_TOP.
            self.background.map_y_pos[0] = LAYER_WIDTH[0];
        }
        self.render.drawn_layer[0] = Some((self.background.map_y_pos[0], self.background.back_pos[0]));
        self.background.map1_y_delay = self.background.map1_y_delay.wrapping_sub(1);
        if self.background.map1_y_delay == 0 {
            self.background.map1_y_delay = self.background.map1_y_delay_max;
            self.script.cur_loc = self.script.cur_loc.wrapping_add(self.background.back_move[0]);
            self.background.back_pos[0] = self.background.back_pos[0].wrapping_add(self.background.back_move[0]);
            if self.background.back_pos[0] > 27 {
                self.background.back_pos[0] -= 28;
                self.background.map_y[0] = self.background.map_y[0].wrapping_sub(1);
                self.background.map_y_pos[0] -= 14;
            }
        }
        // tyrian2.c: update_and_draw_starfield (rendering only).
        self.render.starfield_drawn = self.background.star_active || self.special_weapon.astral_duration > 0;
        if self.processor_type == 6 {
            // tyrian2.c: neat += 3 then neat++ (superWild), one darkening after each.
            let first = self.render.superwild_neat.wrapping_add(3);
            self.render.superwild_neat = first.wrapping_add(1);
            self.render.drawn_neat = (first, self.render.superwild_neat);
        }
        if self.render.starfield_drawn {
            self.advance_starfield();
        }
        // Layer 2 (first slot).
        if self.background.background2over == 3 {
            self.background_2_move();
        }
        if self.background.background2over == 0 && self.background_2_allowed() {
            self.background_2_move();
        }
        // Ground enemies.
        let mut last_enemy_on_screen = self.enemy_on_screen;
        self.background.temp_map_x_ofs = self.background.map_x_ofs;
        self.background.temp_back_move = self.background.back_move[0];
        self.draw_enemy(50);
        self.draw_enemy(100);
        if (self.enemy_on_screen == 0 || self.enemy_on_screen == last_enemy_on_screen)
            && self.background.stop_background_num == 1
        {
            self.background.stop_background_num = 9;
        }
        // Layer 2 (second slot).
        if self.background_2_allowed() && self.background.background2over == 1 {
            self.background_2_move();
        }
        if self.background.background3over == 2 {
            self.background_3_move();
        }
        // New random enemy.
        if self.script.enemies_active && self.rng.next_u32() % 100 > self.script.level_enemy_frequency as u32 {
            let r = self.rng.next_u32();
            // rng_next() % levelEnemyMax: division by zero in C if the list is empty.
            if self.script.level_enemy_max > 0 {
                let temp_w = self.script.level_enemy[(r % self.script.level_enemy_max as u32) as usize];
                if temp_w == 2 {
                    self.queue_sound(3, crate::sound::S_WEAPON_7);
                }
                self.new_enemy(0, temp_w, 0);
            }
        }
        // Sky enemies.
        if !self.background.sky_enemy_over_all {
            last_enemy_on_screen = self.enemy_on_screen;
            self.background.temp_map_x_ofs = self.background.map_x2_ofs;
            self.background.temp_back_move = 0;
            self.draw_enemy(25);
            if self.enemy_on_screen == last_enemy_on_screen && self.background.stop_background_num == 2 {
                self.background.stop_background_num = 9;
            }
        }
        if self.background.background3over == 0 {
            self.background_3_move();
        }
        // Top enemies.
        if !self.background.top_enemy_over {
            self.background.temp_map_x_ofs =
                if !self.background.background3x1 { self.background.old_map_x3_ofs } else { self.background.map_x_ofs };
            self.background.temp_back_move = self.background.back_move[2];
            self.draw_enemy(75);
        }
        self.update_player_shots();
        for p in self.players.iter_mut() {
            p.last_x_shot_move = p.x;
            p.last_y_shot_move = p.y;
        }
        let n = if self.two_player_mode { 2 } else { 1 };
        for i in 0..n {
            if self.players[i].is_alive && !self.ending.end_level {
                self.player_collide(i);
            }
        }
    }

    /// End of the iteration, from `JE_mainGamePlayerFunctions` to `goto level_loop`.
    pub fn loop_bottom(&mut self, inputs: [PlayerInput; 2]) {
        // Rendering only: drawings from the head of this iteration (see `hud_ops_next`).
        let earlier = std::mem::take(&mut self.render.hud_ops_next);
        self.render.hud_ops.extend(earlier);
        if self.ending.first_game_over {
            self.main_game_player_functions(inputs);
        }
        if !self.ending.end_level {
            self.update_enemy_shots();
        } else {
            // tyrian2.c:1251: level end, enemy shots no longer drawn.
            self.render.drawn_enemy_shots.clear();
        }
        if self.background.background3over == 1 {
            self.background_3_move();
        }
        if self.background.top_enemy_over {
            self.background.temp_map_x_ofs = if !self.background.background3x1 {
                self.background.old_map_x3_ofs
            } else {
                self.background.old_map_x_ofs
            };
            self.background.temp_back_move = self.background.back_move[2];
            self.draw_enemy(75);
        }
        if self.background.sky_enemy_over_all {
            let last = self.enemy_on_screen;
            self.background.temp_map_x_ofs = self.background.map_x2_ofs;
            self.background.temp_back_move = 0;
            self.draw_enemy(25);
            if self.enemy_on_screen == last && self.background.stop_background_num == 2 {
                self.background.stop_background_num = 9;
            }
        }
        self.update_repeating_explosions();
        self.update_explosions();
        // tyrian2.c:JE_drawSP (further down the loop; no JE_doSP between
        // the two): superpixel counters, rendering only.
        self.draw_sp_tick();
        if !self.controls.port_config_change {
            self.controls.port_config_done = true;
        }
        if self.background_2_allowed() && self.background.background2over == 2 {
            self.background_2_move();
        }
        // Low armor warning: spare ship (tyrian2.c).
        if (self.players[0].is_alive && self.players[0].armor < 6)
            || (self.two_player_mode && self.players[1].is_alive && self.players[1].armor < 6)
        {
            if self.armor_ship_delay > 0 {
                self.armor_ship_delay -= 1;
            } else {
                if let Some(bi) = self.new_enemy(50, 560, 0) {
                    let r = self.rng.next_u32() % 3;
                    self.enemy[bi].enemydie = 560 + r as u16 + 1;
                    self.enemy[bi].eyc = (self.enemy[bi].eyc as i32 - self.background.back_move[2] as i32) as i8;
                    self.enemy[bi].armorleft = 4;
                }
                self.armor_ship_delay = 500;
            }
        }
        // Rendering: bars and WARNING word (tyrian2.c, "thisPlayerNum" ignored outside network play).
        self.render.drawn_texts.warning = false;
        if (self.players[0].is_alive && self.players[0].armor < 6)
            || (self.two_player_mode && self.players[1].is_alive && self.players[1].armor < 6)
        {
            let armor = if self.players[0].is_alive && self.players[0].armor < 6 {
                self.players[0].armor
            } else {
                self.players[1].armor
            };
            // tyrian2.c: low armor alarm (warningSoundDelay, faster as armor drops).
            let temp_w = (armor * 4 + 8) as u16;
            let s = &mut self.sound;
            if s.warning_sound_delay > temp_w {
                s.warning_sound_delay = temp_w;
            }
            if s.warning_sound_delay > 1 {
                s.warning_sound_delay -= 1;
            } else {
                s.sample[7] = crate::sound::S_WARNING;
                s.warning_sound_delay = temp_w;
            }
            let t = &mut self.render.drawn_texts;
            t.warning_col = t.warning_col.wrapping_add(t.warning_col_change as u8);
            let top = (113 + (14 - armor as i32 * 2)) as u8;
            if t.warning_col > top {
                t.warning_col_change = -t.warning_col_change;
                t.warning_col = top;
            } else if t.warning_col < 113 {
                t.warning_col_change = -t.warning_col_change;
            }
            t.warning = true;
        }
        if self.script.random_explosions && self.rng.next_u32() % 10 == 1 {
            // `JE_setupExplosionLarge(false, 20, rng_next() % 280, rng_next() % 180)`:
            // GCC x86-64 evaluates the arguments from right to left (y drawn before x),
            // as proven for `varz.c:JE_doSpecialShot` (e1-special-blade-autopilot).
            let y = (self.rng.next_u32() % 180) as i16;
            let x = (self.rng.next_u32() % 280) as i16;
            self.setup_explosion_large(false, 20, x, y);
        }
        self.flush_sound_queue();
        if self.script.return_active && self.enemy_on_screen == 0 {
            self.event_jump(65535);
            self.script.return_active = false;
        }
        self.render.drawn_texts.secret = false;
        if self.ending.display_time > 0 {
            self.ending.display_time -= 1;
            // Rendering: JE_outTextAndDarken(.., (uint8_t)flash - 8, ..) then flash += flashChange.
            let t = &mut self.render.drawn_texts;
            t.secret = true;
            t.secret_flash = t.flash;
            t.flash = t.flash.wrapping_add(t.flash_change as u8);
            if t.flash > 4 || t.flash == 0 {
                t.flash_change = -t.flash_change;
            }
        }
        self.render.drawn_texts.timer = None;
        if self.script.level_timer && self.script.level_timer_countdown > 0 {
            self.script.level_timer_countdown -= 1;
            self.render.drawn_texts.timer = Some(self.script.level_timer_countdown);
            if self.script.level_timer_countdown == 0 {
                self.event_jump(self.script.level_timer_jump_to);
            }
            // tyrian2.c: stopwatch ticking (no sound in Timed Battle).
            let c = self.script.level_timer_countdown;
            if self.mode.timed != 0 {
            } else if c > 200 {
                if c.is_multiple_of(100) {
                    self.queue_sound(7, crate::sound::S_WARNING);
                }
                if c.is_multiple_of(10) {
                    self.queue_sound(6, crate::sound::S_CLICK);
                }
            } else if c.is_multiple_of(20) {
                self.queue_sound(7, crate::sound::S_WARNING);
            }
        }
        self.render.drawn_texts.game_over = self.demo.is_none()
            && !self.bonus_level_current
            && self.ending.all_players_gone
            && self.players[0].exploding_ticks == 0
            && self.players[1].exploding_ticks == 0;
        if self.ending.all_players_gone {
            if self.players[0].exploding_ticks > 0 || self.players[1].exploding_ticks > 0 {
                // tyrian2.c, GAME OVER block: musicFade = true.
                self.music.music_fade = true;
            } else {
                // tyrian2.c: "if (play_demo || …) reallyEndLevel = true; else
                // JE_dString(… game over)"; in a demo, neither music nor waiting.
                // Bonus level ("$"): same immediate exit (tyrian2.c:1481), but the
                // game over song is played (only `play_demo` suppresses it, :1487).
                let demo = self.demo.is_some();
                if demo || self.bonus_level_current {
                    self.ending.really_end_level = true;
                }
                if self.ending.first_game_over {
                    // tyrian2.c: play_song(SONG_GAMEOVER) then set_volume(tyrMusicVolume).
                    if !demo {
                        self.push_music_cue(MusicCue::PlaySong(10));
                        self.push_music_cue(MusicCue::SetVolume(self.music.tyr_music_volume));
                    }
                    self.ending.first_game_over = false;
                }
                // tyrian2.c: "newkey || button[0..2] || newmouse"; the mouse
                // click arrives through button[] (MouseInput), the key through new_key.
                if !demo
                    && (self.mode.new_key
                        || self.controls.button[0]
                        || self.controls.button[1]
                        || self.controls.button[2])
                {
                    self.ending.really_end_level = true;
                }
            }
        }
        // tyrian2.c "Filtration": JE_filterScreen(levelFilter, levelBrightness)
        // if filterActive. The arguments are read before the call's fade.
        self.render.drawn_filter = None;
        if self.filter.filter_active {
            // backgrnd.c:JE_filterScreen: the tint requires filtrationAvail, the
            // brightness explosionTransparent (processorType, config.c).
            let col =
                if crate::world::pt_filtration_avail(self.processor_type) { self.filter.level_filter } else { -99 };
            let int_ = if crate::world::pt_explosion_transparent(self.processor_type) {
                self.filter.level_brightness
            } else {
                -99
            };
            self.render.drawn_filter = Some((col, int_));
            if self.filter.filter_fade {
                self.filter.level_brightness =
                    self.filter.level_brightness.wrapping_add(self.filter.level_brightness_chg);
                if (self.filter.filter_fade_start && self.filter.level_brightness < -14)
                    || self.filter.level_brightness > 14
                {
                    self.filter.level_brightness_chg = self.filter.level_brightness_chg.wrapping_neg();
                    self.filter.filter_fade_start = false;
                    self.filter.level_filter = self.filter.level_filter_new;
                }
                if !self.filter.filter_fade_start && self.filter.level_brightness == 0 {
                    self.filter.filter_fade = false;
                    self.filter.level_brightness = -99;
                }
            }
        }
        // level_helpers.c:draw_boss_bar — updates the boss bars.
        for bb in 0..2 {
            if self.script.boss_bar[bb].link_num == 0 {
                continue;
            }
            let mut armor: u32 = 256;
            for e in 0..ENEMY_MAX {
                if self.enemy_avail[e] != 1
                    && self.enemy[e].linknum == self.script.boss_bar[bb].link_num
                    && (self.enemy[e].armorleft as u32) < armor
                {
                    armor = self.enemy[e].armorleft as u32;
                }
            }
            if armor > 255 || armor == 0 {
                self.script.boss_bar[bb].link_num = 0;
            } else {
                self.script.boss_bar[bb].armor = if armor == 255 { 254 } else { armor };
            }
        }
        let bars = (self.script.boss_bar[0].link_num != 0) as u32 + (self.script.boss_bar[1].link_num != 0) as u32;
        if bars == 1 && self.script.boss_bar[0].link_num == 0 {
            self.script.boss_bar[0] = self.script.boss_bar[1];
            self.script.boss_bar[1].link_num = 0;
        }
        self.render.drawn_texts.boss_bars = bars as u8;
        for b in 0..bars as usize {
            self.render.drawn_texts.boss[b] = (self.script.boss_bar[b].armor, self.script.boss_bar[b].color);
            if self.script.boss_bar[b].color > 0 {
                // level_helpers.c:draw_boss_bar: the flash color decays at display time.
                self.script.boss_bar[b].color -= 1;
            }
        }
        // tyrian2.c:JE_main, after JE_inGameDisplays: combo_update(delta).
        self.combo_update();
        // tyrian2.c, end of loop: hitstop_update(), kill_slowmo_update()
        // (display speed, rendering) and secondary_explosions_update().
        self.hitstop_end_of_loop();
        // Level end on scrolling stop.
        if self.background.stop_background_num == 9 && self.background.back_move[0] == 0 && !self.enemy_still_exploding
        {
            self.background.back_move = [1, 2, 3];
            self.background.explode_move = 2;
            self.background.stop_background_num = 0;
            self.background.stop_backgrounds = false;
            if self.ending.wait_to_end_level {
                self.ending.end_level = true;
                self.ending.level_end = 40;
            }
            if self.ending.all_players_gone {
                self.ending.really_end_level = true;
            }
        }
        if !self.ending.end_level && self.enemy_on_screen == 0 {
            if self.ending.ready_to_end_level && !self.enemy_still_exploding {
                if self.script.level_timer_countdown > 0 {
                    self.script.level_timer = false;
                }
                self.ending.ready_to_end_level = false;
                self.ending.end_level = true;
                self.ending.level_end = 40;
                if self.ending.all_players_gone {
                    self.ending.really_end_level = true;
                }
            }
            if self.background.stop_backgrounds {
                self.background.stop_backgrounds = false;
                self.background.back_move = [1, 2, 3];
                self.background.explode_move = 2;
            }
        }
        // tyrian2.c: achievements_check at the end of the loop.
        self.ach_events_at_check = self.ach_events;
    }
}
