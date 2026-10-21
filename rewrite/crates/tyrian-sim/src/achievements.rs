//! Achievements (`achievements.c`) for the part that depends only on the simulation.
//!
//! Level end (`achievements_check_level`, on `EVENT_LEVEL_COMPLETE`):
//! - Survivor / Untouchable / Ironman / Immortal / Adamantium: no death
//!   during the level (`deaths_this_level == 0`, `stats.c:stats_count_death` on
//!   `EVENT_PLAYER_DIED` from `varz.c`), `deathless_streak` ≥ 5/10/20/50;
//! - Phoenix, Close Call, Max Power, Hard Mode / Impossible (`initialDifficulty`);
//! - mutators (Glass Cannon, Mirror Master, Modifier, Daredevil, Insanity,
//!   Speedster, Glass House, Mut Survivor, Mutator Collector, Glass Master);
//! - True Pacifist, Zen Master, Perfect Score (level counters, `stats.c`);
//! - Speed Demon (< 90 s), Bulletproof / Last Stand / Iron Curtain (EVENT_PLAYER_HIT).
//!
//! At each frame, the event listeners then `achievements_check`:
//! First Blood / Centurion... (`achievements_check_kills`, lifetime counter
//! `game_stats.enemies_killed`), Chain Reaction (5 kills in the frame), Quick
//! Draw (kill < 3 s after level start), Combo King / Execution Chain
//! (`stats.c:stats_count_kill`, 2 s window), Firepower I/II / Overkill
//! (`dps.c`, 1 s window), Sharpshooter / Sniper (hits / shots of the run
//! beyond 100 shots), Boss Slayer / Boss Shredder (< 10 s) / Perfect Boss,
//! Mirror Boss / Bullet Hell Boss / Boss Rush.
//!
//! Time: the C code reads `SDL_GetTicks()`; the rewrite counts a nominal game time
//! of 28.75 ms per frame (ADR 0091), which the GOLDEN binary reproduces
//! with GOLDEN_ACH_CLOCK=1. Events come from `World::ach_events_at_check`.
//!
//! `stats.c` counters (`game_stats`, `run_stats`) are kept here; screenshots
//! and slowed time via `note_screenshot` / `note_bullet_time`.
//!
//! The C code's 100 achievements are ported. `check` follows `achievements_check` in
//! the C order (lifetime milestones, Pacifist, collections, Endless, Dodge Artist);
//! `episode_complete` follows `achievements_on_episode_complete`; `note_shop`,
//! `count_game_start`, `unlock_named` receive the events that do not come
//! from the simulation (shop, modes menu, Endless leaderboard).
//!
//! `ach_spectator_mode` guard: nothing is unlocked while a replay or a
//! demo is playing back.

use crate::anomalies::mutators_score_multiplier;
use crate::f32ops::F32;
use crate::modes::{MUT_BULLET_HELL, MUT_FAST_ENEMIES, MUT_GLASS_CANNON, MUT_MIRROR, MUT_ONE_LIFE, MUT_RISK_REWARD};
use crate::world::AchEvents;
use crate::Sim;
pub use tyrian_formats::achievements::{achievement_index, AchievementBlock, LifetimeCounters, ACHIEVEMENTS, ACH_MAX};

const DIFFICULTY_HARD: i8 = 3;
const DIFFICULTY_IMPOSSIBLE: i8 = 4;

/// Nominal duration of a frame in hundredths of ms: `setDelaySpeed(0x4300)` ×
/// `frameCountMax` 2 × `pitPeriod` 12/14318180 s = 28.75 ms (fastPlay 0, config.c).
const FRAME_CMS: u64 = 2875;

/// Game clock in ms (`SDL_GetTicks()` under GOLDEN_ACH_CLOCK) after `frames` frames.
pub fn clock_ms(frames: u64) -> u32 {
    (1_000_000 + frames * FRAME_CMS / 100) as u32
}

/// `stats.h:GameStats`: lifetime counters (`game_stats`) or run counters (`run_stats`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameStats {
    pub enemies_killed: u64,
    pub shots_fired: u64,
    pub games_played: u64,
    pub total_score: u64,
    pub time_played_ms: u64,
    pub levels_completed: u64,
    pub deaths: u64,
}

impl GameStats {
    /// In the order of the OTYE block (`config.c:JE_saveConfiguration`).
    pub fn to_array(&self) -> [u64; 7] {
        [
            self.enemies_killed,
            self.shots_fired,
            self.games_played,
            self.total_score,
            self.time_played_ms,
            self.levels_completed,
            self.deaths,
        ]
    }

    pub fn from_array(a: [u64; 7]) -> Self {
        Self {
            enemies_killed: a[0],
            shots_fired: a[1],
            games_played: a[2],
            total_score: a[3],
            time_played_ms: a[4],
            levels_completed: a[5],
            deaths: a[6],
        }
    }
}

/// What `achievements_check` reads from the player globals (`player[0]`,
/// `allPlayersGone`, mutators). Separate from `Sim` to test each rule.
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckView {
    /// `player[0].cash`.
    pub cash: u64,
    /// `player[0].items`: ship, front and rear weapons, sidekicks.
    pub ship: u8,
    pub weapons: [u8; 2],
    pub sidekicks: [u8; 2],
    /// `player[0].armor`, `initial_armor`, `is_alive`.
    pub armor: u32,
    pub initial_armor: u32,
    pub alive: bool,
    /// One of the players is alive, and `allPlayersGone`.
    pub any_alive: bool,
    pub all_players_gone: bool,
    /// `mut_risk_reward`.
    pub risk_reward: bool,
}

impl CheckView {
    pub fn of(w: &crate::world::World) -> Self {
        let p = &w.players[0];
        Self {
            cash: p.cash,
            ship: p.items.ship.0,
            weapons: [p.items.weapon[0].id.0, p.items.weapon[1].id.0],
            sidekicks: [p.items.sidekick[0].0, p.items.sidekick[1].0],
            armor: p.armor,
            initial_armor: p.initial_armor,
            alive: p.is_alive,
            any_alive: w.players.iter().any(|q| q.is_alive),
            all_players_gone: w.ending.all_players_gone,
            risk_reward: w.mode.mutator(MUT_RISK_REWARD),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AchievementTracker {
    pub unlocked: [bool; ACH_MAX],
    /// `achievements.c:deathless_streak`.
    pub deathless_streak: u32,
    /// `ach_spectator_mode()`: replay or demo playback.
    pub spectator: bool,
    /// `stats.c:game_stats`: lifetime counters (loaded from `opentyrian.dat`).
    pub game_stats: GameStats,
    /// `stats.c:run_stats`.
    pub run_stats: GameStats,
    /// `stats.c:run_active`: true after `run_start`, false after `run_end`.
    pub run_active: bool,
    /// `stats.c:screenshots_taken` (OTY3).
    pub screenshots_taken: u32,
    /// `bullet_time.c:bullet_time_uses` (OTY3).
    pub bullet_time_uses: u32,
    /// `stats.c:replays_saved` (OTY3): recordings stopped by hand (F10), lifetime.
    pub replays_saved: u32,
    /// Local hour (0..23) for ACH_NIGHT_PLAYER (`achievements_check`, `localtime()`);
    /// `None`: rule inactive (tools, captures without a clock).
    pub local_hour: Option<u8>,
    /// True if the counters come from a file that was read (otherwise `ach_save`
    /// must not overwrite those on disk with zeros).
    pub counters_loaded: bool,
    /// Context set by the caller before the checks (the C code reads globals).
    pub language: i32,
    pub endless_wave: Option<i32>,
    pub enabled_mod_count: u32,
    /// Wall clock in ms (real `SDL_GetTicks()`): session duration (Night Owl).
    pub wall_ms: u64,
    /// True in play: the achievement timers (Dodge Artist, Marathon, Speed Demon...) follow
    /// real time like `SDL_GetTicks()` (stats.c:337), and not the nominal
    /// 28.75 ms frames. False under golden tests (virtual C clock, ADR 0091).
    pub real_clock: bool,
    session_start_wall_ms: Option<u64>,
    /// Collection and progression counters (`stats.c`), persisted (OTYE / OTY3).
    pub weapons_used_mask: u64,
    pub ships_tried_mask: u64,
    pub sidekicks_used_mask: u64,
    pub weapons_bought_mask: u64,
    pub weapons_bought_mask_hi: u64,
    pub total_credits_spent: u32,
    pub episodes_completed_mask: u64,
    pub languages_played_mask: u64,
    pub lifetime_credits: u64,
    pub chaos_episodes_mask: u64,
    pub episode_death_count: u32,
    pub episode_start_episode: u32,
    /// "Run" scope (`stats_run_scope_reset`), not persisted.
    pub run_had_mutator: bool,
    pub run_risk_tiers_crossed: u32,
    pub run_score_total: u64,
    /// `items_bought_this_shop`, `half_level_low_hp`, `last_shot_time`.
    pub items_bought_this_shop: u32,
    half_level_low_hp: bool,
    last_shot_ms: u32,
    /// `stats.c:level_stats.credits_start` and `score_banked_this_level`.
    credits_start: u64,
    score_banked_this_level: u64,
    /// `stats.c:total_bosses_defeated`, `mut_one_life_levels`,
    /// `mutator_levels_completed`, `zero_damage_level_streak`, `mirror_levels_completed`.
    pub total_bosses_defeated: u32,
    pub one_life_levels: u32,
    pub mutator_levels_completed: u32,
    pub zero_damage_level_streak: u32,
    pub mirror_levels_completed: u32,
    /// Game frames elapsed since creation (clock, see `clock_ms`).
    pub frames: u64,
    /// Indices unlocked since the last `level_start`, in C order.
    pub log: Vec<usize>,
    deaths_this_level: u32,
    /// One per player: `stats_on_player_died` counts the death of either one (varz.c:1204-1216).
    was_alive: [bool; 2],
    level_start_ms: u32,
    last_hit_ms: u32,
    boss_spawn_ms: u32,
    kills_window_start: u32,
    kills_in_window: u32,
    run_shots_fired: u64,
    run_shots_hit: u64,
    shots_fired_this_level: u64,
    kills_this_level: u64,
    level_damage_taken: u32,
    level_start_hp: u32,
    dps_current: u32,
    dps_acc: u32,
    dps_window_ms: u32,
    last_frame_ms: u32,
    prev: AchEvents,
}

impl AchievementTracker {
    pub fn new(unlocked: [bool; ACH_MAX], spectator: bool) -> Self {
        Self {
            unlocked,
            deathless_streak: 0,
            spectator,
            game_stats: GameStats::default(),
            run_stats: GameStats::default(),
            run_active: false,
            screenshots_taken: 0,
            bullet_time_uses: 0,
            replays_saved: 0,
            local_hour: None,
            counters_loaded: false,
            language: -1,
            endless_wave: None,
            enabled_mod_count: 0,
            wall_ms: 0,
            real_clock: false,
            session_start_wall_ms: None,
            weapons_used_mask: 0,
            ships_tried_mask: 0,
            sidekicks_used_mask: 0,
            weapons_bought_mask: 0,
            weapons_bought_mask_hi: 0,
            total_credits_spent: 0,
            episodes_completed_mask: 0,
            languages_played_mask: 0,
            lifetime_credits: 0,
            chaos_episodes_mask: 0,
            episode_death_count: 0,
            episode_start_episode: 0,
            run_had_mutator: false,
            run_risk_tiers_crossed: 0,
            run_score_total: 0,
            items_bought_this_shop: 0,
            half_level_low_hp: false,
            last_shot_ms: 0,
            credits_start: 0,
            score_banked_this_level: 0,
            total_bosses_defeated: 0,
            one_life_levels: 0,
            mutator_levels_completed: 0,
            zero_damage_level_streak: 0,
            mirror_levels_completed: 0,
            frames: 0,
            log: Vec::new(),
            deaths_this_level: 0,
            was_alive: [true; 2],
            level_start_ms: 0,
            last_hit_ms: 0,
            boss_spawn_ms: 0,
            kills_window_start: 0,
            kills_in_window: 0,
            run_shots_fired: 0,
            run_shots_hit: 0,
            shots_fired_this_level: 0,
            kills_this_level: 0,
            level_damage_taken: 0,
            level_start_hp: 0,
            dps_current: 0,
            dps_acc: 0,
            dps_window_ms: 0,
            last_frame_ms: 0,
            prev: AchEvents::default(),
        }
    }

    /// `dps.c:dps_current`.
    pub fn dps(&self) -> u32 {
        self.dps_current
    }

    /// Lifetime counters read from the file (`config.c:JE_loadConfiguration`).
    pub fn set_lifetime(&mut self, c: &LifetimeCounters) {
        self.game_stats = GameStats::from_array(c.stats);
        self.screenshots_taken = c.screenshots_taken;
        self.bullet_time_uses = c.bullet_time_uses;
        self.replays_saved = c.replays_saved;
        self.total_bosses_defeated = c.total_bosses_defeated;
        self.mutator_levels_completed = c.mutator_levels_completed;
        self.weapons_used_mask = c.weapons_used_mask;
        self.ships_tried_mask = c.ships_tried_mask;
        self.sidekicks_used_mask = c.sidekicks_used_mask;
        self.weapons_bought_mask = c.weapons_bought_mask;
        self.weapons_bought_mask_hi = c.weapons_bought_mask_hi;
        self.mirror_levels_completed = c.mirror_levels_completed;
        self.total_credits_spent = c.total_credits_spent;
        self.one_life_levels = c.mut_one_life_levels;
        self.zero_damage_level_streak = c.zero_damage_level_streak;
        self.deathless_streak = c.deathless_streak;
        self.episodes_completed_mask = c.episodes_completed_mask;
        self.languages_played_mask = c.languages_played_mask;
        self.lifetime_credits = c.lifetime_credits;
        self.chaos_episodes_mask = c.chaos_episodes_mask;
        self.episode_death_count = c.episode_death_count;
        self.episode_start_episode = c.episode_start_episode;
        self.counters_loaded = true;
    }

    /// Counters to write to the file (`config.c:JE_saveConfiguration`).
    pub fn lifetime(&self) -> LifetimeCounters {
        LifetimeCounters {
            stats: self.game_stats.to_array(),
            screenshots_taken: self.screenshots_taken,
            bullet_time_uses: self.bullet_time_uses,
            replays_saved: self.replays_saved,
            total_bosses_defeated: self.total_bosses_defeated,
            mutator_levels_completed: self.mutator_levels_completed,
            weapons_used_mask: self.weapons_used_mask,
            ships_tried_mask: self.ships_tried_mask,
            sidekicks_used_mask: self.sidekicks_used_mask,
            weapons_bought_mask: self.weapons_bought_mask,
            weapons_bought_mask_hi: self.weapons_bought_mask_hi,
            mirror_levels_completed: self.mirror_levels_completed,
            total_credits_spent: self.total_credits_spent,
            mut_one_life_levels: self.one_life_levels,
            zero_damage_level_streak: self.zero_damage_level_streak,
            deathless_streak: self.deathless_streak,
            episodes_completed_mask: self.episodes_completed_mask,
            languages_played_mask: self.languages_played_mask,
            lifetime_credits: self.lifetime_credits,
            chaos_episodes_mask: self.chaos_episodes_mask,
            episode_death_count: self.episode_death_count,
            episode_start_episode: self.episode_start_episode,
        }
    }

    /// `stats.c:stats_run_start`: `run_stats` reset to zero, `run_active` true.
    /// The C code calls it at each level in the campaign (level_load.c) and only once
    /// at the start of an Endless run (endless.c:endless_start, two sites:
    /// start and resume); in Endless the achievements' "run" scope
    /// (`stats_run_scope_reset`) is not ported here.
    pub fn run_start(&mut self) {
        self.run_stats = GameStats::default();
        self.run_shots_hit = 0;
        self.run_active = true;
    }

    /// `stats.c:stats_run_end` (tyrian2.c: end of game / return to the menu).
    pub fn run_end(&mut self) {
        self.run_active = false;
        self.run_scope_reset();
    }

    /// `stats.c:stats_run_scope_reset`: the "run"-scoped state.
    pub fn run_scope_reset(&mut self) {
        self.run_had_mutator = false;
        self.run_risk_tiers_crossed = 0;
        self.run_score_total = 0;
    }

    /// `stats.c:stats_count_game_start`. No spectator guard in the C code.
    pub fn count_game_start(&mut self) {
        self.game_stats.games_played += 1;
        self.run_stats.games_played += 1;
        self.run_scope_reset();
    }

    /// `stats.c:stats_add_score`: `(unsigned long)(score * mutators_score_multiplier())`,
    /// the product being computed in float (`unsigned long` converted to float).
    fn add_score(&mut self, score: u64, mutators: u8) {
        let adjusted = F32::from_int(score as i64).mul(mutators_score_multiplier(mutators)).trunc().max(0) as u64;
        self.game_stats.total_score += adjusted;
        self.run_stats.total_score += adjusted;
        self.run_score_total += adjusted;
    }

    /// `stats.c:stats_bank_level_earnings`: adds to the total score what the
    /// level earned (`player[0].cash - credits_start`) since the last
    /// deposit.
    fn bank_level_earnings(&mut self, cash: u64, mutators: u8) {
        let earned = cash as i64 - self.credits_start as i64 - self.score_banked_this_level as i64;
        if earned > 0 {
            self.add_score(earned as u64, mutators);
            self.score_banked_this_level += earned as u64;
        }
    }

    /// `input_map.c:input_action_screenshot` (achievement): `screenshots_taken++`,
    /// ACH_PHOTOGRAPHER, then the `achievements.c:achievements_check` test
    /// (ACH_SCREENSHOT_PRO at 25 captures). Returns the unlocked indices.
    pub fn note_screenshot(&mut self) -> Vec<usize> {
        let mut out = Vec::new();
        self.screenshots_taken = self.screenshots_taken.saturating_add(1);
        if !self.spectator {
            self.unlock("PHOTOGRAPHER", &mut out);
            if self.screenshots_taken >= 25 {
                self.unlock("SCREENSHOT_PRO", &mut out);
            }
        }
        self.log.extend_from_slice(&out);
        out
    }

    /// `input_map.c:input_action_bullet_time` (successful activation):
    /// ACH_BULLET_TIME, then the `achievements_check` test (ACH_SLOW_MOTION_PRO
    /// at 50 uses, `bullet_time_uses` being incremented by the caller).
    pub fn note_bullet_time(&mut self) -> Vec<usize> {
        let mut out = Vec::new();
        if !self.spectator {
            self.unlock("BULLET_TIME", &mut out);
            if self.bullet_time_uses >= 50 {
                self.unlock("SLOW_MOTION_PRO", &mut out);
            }
        }
        self.log.extend_from_slice(&out);
        out
    }

    /// Lifetime thresholds of `achievements.c:achievements_check`, evaluated at each frame (and when
    /// loading a profile): Slow Motion Pro (`bullet_time_uses >= 50`), Filmmaker
    /// (`replays_saved >= 5`) and Night Player (local time 0:00 - 4:59).
    fn check_lifetime(&mut self, out: &mut Vec<usize>) {
        if self.bullet_time_uses >= 50 {
            self.unlock("SLOW_MOTION_PRO", out);
        }
        if self.replays_saved >= 5 {
            self.unlock("FILMMAKER", out);
        }
        if self.local_hour.is_some_and(|h| h < 5) {
            self.unlock("NIGHT_PLAYER", out);
        }
    }

    /// `mainint.c` (F10 key): a recording stopped by hand and written,
    /// `replays_saved++` then Filmmaker test. Returns the unlocked indices.
    pub fn note_replay_saved(&mut self) -> Vec<usize> {
        let mut out = Vec::new();
        self.replays_saved = self.replays_saved.saturating_add(1);
        if !self.spectator {
            self.check_lifetime(&mut out);
        }
        self.log.extend_from_slice(&out);
        out
    }

    /// Level start, on the `Sim` of the fresh level: `stats.c:level_stats_reset`
    /// (timers, level counters, `level_start_hp`), `stats.c:stats_run_start`
    /// (called at each level by level_load.c in the campaign), `dps.c:dps_reset`
    /// (tyrian2.c) and the reset of `deaths_this_level`.
    pub fn level_start(&mut self, sim: &Sim) {
        let now = self.now_ms();
        // level_load.c: stats_run_start() at each level in the campaign, outside
        // a demo; in Endless it is endless_start that calls it (here only
        // if no run is active, in case the caller forgot).
        let endless = sim.world().endless_lives.is_some();
        if !self.spectator && (!endless || !self.run_active) {
            self.run_start();
            // stats_run_start: in Endless only, the "run" scope starts over.
            if endless {
                self.run_scope_reset();
            }
        }
        // level_load.c: session start (Night Owl), only once.
        if !self.spectator && self.session_start_wall_ms.is_none() {
            self.session_start_wall_ms = Some(self.wall_ms);
        }
        self.credits_start = sim.world().players[0].cash;
        self.score_banked_this_level = 0;
        self.deaths_this_level = 0;
        self.was_alive = [true; 2];
        self.level_start_ms = now;
        self.last_hit_ms = now;
        self.boss_spawn_ms = 0;
        self.run_shots_fired = 0;
        self.run_shots_hit = 0;
        self.shots_fired_this_level = 0;
        self.kills_this_level = 0;
        self.level_damage_taken = 0;
        self.level_start_hp = sim.world().players[0].armor;
        self.dps_current = 0;
        self.dps_acc = 0;
        self.dps_window_ms = 0;
        self.half_level_low_hp = false;
        // tyrian2.c: lastFrameTicks re-read after loading under the
        // virtual clock (golden_master_level_clock_start, ADR 0091): the first
        // frame of the level counts as a nominal frame in the DPS window.
        self.last_frame_ms = now;
        self.prev = sim.world().ach_events_at_check;
        self.log.clear();
    }

    /// `SDL_GetTicks()` of the achievement timers: real time (`wall_ms`) in play, nominal
    /// frames under the golden masters.
    pub fn now_ms(&self) -> u32 {
        if self.real_clock {
            (1_000_000 + self.wall_ms) as u32
        } else {
            clock_ms(self.frames)
        }
    }

    fn unlock(&mut self, id: &str, out: &mut Vec<usize>) {
        let i = achievement_index(id).expect("unknown achievement id");
        // `achievements_unlock`: already unlocked → silent.
        if !self.unlocked[i] {
            self.unlocked[i] = true;
            out.push(i);
        }
    }

    /// To be called after each tick. Replays, in C order, the listeners of the
    /// events emitted during the tick then `achievements_check` (end of frame,
    /// tyrian2.c).
    pub fn after_tick(&mut self, sim: &Sim) {
        let w = sim.world();
        self.frames += 1;
        let now = self.now_ms();
        let player_count = if w.two_player_mode { 2 } else { 1 };
        for i in 0..player_count {
            let alive = w.players[i].is_alive;
            if self.was_alive[i] && !alive && !self.spectator {
                self.deaths_this_level += 1;
                // stats.c:stats_count_death (EVENT_PLAYER_DIED).
                self.game_stats.deaths += 1;
                self.run_stats.deaths += 1;
                // In Endless, death banks the earnings gained so far.
                if w.endless_lives.is_some() {
                    self.bank_level_earnings(w.players[0].cash, w.mode.mutators);
                }
            }
            self.was_alive[i] = alive;
        }
        let ev = w.ach_events_at_check;
        let prev = std::mem::replace(&mut self.prev, ev);
        if self.spectator {
            return;
        }
        let mut out = std::mem::take(&mut self.log);
        // stats.c:stats_count_shot, stats_on_shot_hit; dps.c:stats_add_damage.
        let shots = (ev.shots_fired - prev.shots_fired) as u64;
        self.run_shots_fired += shots;
        // stats.c:stats_count_shot, once per shot.
        self.game_stats.shots_fired += shots;
        self.run_stats.shots_fired += shots;
        self.shots_fired_this_level += shots;
        if shots > 0 {
            self.last_shot_ms = now;
        }
        self.run_shots_hit += (ev.shots_hit - prev.shots_hit) as u64;
        self.dps_acc = self.dps_acc.wrapping_add(ev.damage_dealt - prev.damage_dealt);
        // achievements_on_player_hit.
        if ev.player_hits != prev.player_hits {
            self.last_hit_ms = now;
            self.level_damage_taken += ev.player_hits - prev.player_hits;
        }
        // achievements_on_boss_spawned (event 79).
        if ev.boss_spawns != prev.boss_spawns {
            self.boss_spawn_ms = now;
        }
        // stats_on_enemy_killed: stats_add_credits(enemy_killed.value) if > 0.
        self.lifetime_credits += ev.kill_credits - prev.kill_credits;
        // EVENT_ENEMY_KILLED: stats_on_enemy_killed (stats_count_kill) then
        // achievements_on_enemy_killed. kills_this_frame restarts from zero at each
        // achievements_check: it equals the rank of the kill in the frame.
        for k in 1..=(ev.kills - prev.kills) {
            self.game_stats.enemies_killed += 1;
            self.run_stats.enemies_killed += 1;
            if now.wrapping_sub(self.kills_window_start) > 2000 {
                self.kills_in_window = 0;
                self.kills_window_start = now;
            }
            self.kills_in_window += 1;
            self.kills_this_level += 1;
            if k >= 5 {
                self.unlock("CHAIN_REACTION", &mut out);
            }
            self.check_kills(&mut out);
            if self.level_start_ms > 0 && now.wrapping_sub(self.level_start_ms) < 3000 {
                self.unlock("QUICK_DRAW", &mut out);
            }
        }
        // achievements_on_boss_defeated (boss_track_flush, after the chain's kills).
        for _ in 0..(ev.bosses_defeated - prev.bosses_defeated) {
            self.unlock("BOSS_SLAYER", &mut out);
            if self.boss_spawn_ms > 0 && now.wrapping_sub(self.boss_spawn_ms) < 10_000 {
                self.unlock("BOSS_SHREDDER", &mut out);
            }
            self.total_bosses_defeated += 1;
            if self.total_bosses_defeated >= 10 {
                self.unlock("BOSS_RUSH", &mut out);
            }
            if w.mode.mutator(MUT_MIRROR) {
                self.unlock("MIRROR_BOSS", &mut out);
            }
            if w.mode.mutator(MUT_BULLET_HELL) {
                self.unlock("BULLET_HELL_BOSS", &mut out);
            }
            if self.boss_spawn_ms > 0 && self.last_hit_ms < self.boss_spawn_ms {
                self.unlock("PERFECT_BOSS", &mut out);
            }
        }
        // tyrian2.c: stats_dps_update(delta) if delta < 1000.
        let delta = now.wrapping_sub(self.last_frame_ms);
        if delta < 1000 {
            // stats.c:stats_update_time(delta), same condition (tyrian2.c).
            self.game_stats.time_played_ms += delta as u64;
            self.run_stats.time_played_ms += delta as u64;
            self.dps_window_ms += delta;
            if self.dps_window_ms >= 1000 {
                self.dps_current = self.dps_acc;
                self.dps_acc = 0;
                self.dps_window_ms = 0;
            }
        }
        self.last_frame_ms = now;
        // tyrian2.c: shots on screen (Bullet Hell / Bullet Blizzard), before achievements_check.
        let active_shots =
            w.shot_avail.iter().filter(|&&a| a != 0).count() + w.enemy_shot_avail.iter().filter(|&&a| !a).count();
        self.on_screen_shots(active_shots, &mut out);
        self.check(now, &CheckView::of(w), &mut out);
        self.log = out;
    }

    /// tyrian2.c: ≥ 50 shots on screen (player and enemies) = Bullet Hell, ≥ 100 = Bullet Blizzard.
    pub fn on_screen_shots(&mut self, active_shots: usize, out: &mut Vec<usize>) {
        if self.spectator {
            return;
        }
        if active_shots >= 50 {
            self.unlock("BULLET_HELL", out);
        }
        if active_shots >= 100 {
            self.unlock("BULLET_BLIZZARD", out);
        }
    }

    /// `achievements.c:achievements_check`, in C order. Called at each
    /// level frame, when the list is opened (F3) and before the title
    /// achievements screen (`now`: game clock).
    pub fn check(&mut self, now: u32, v: &CheckView, out: &mut Vec<usize>) {
        if self.spectator {
            return;
        }
        self.check_kills(out);
        let g = self.game_stats;
        for (n, id) in [(10_000, "TRIGGER_HAPPY"), (100_000, "BULLET_STORM")] {
            if g.shots_fired >= n {
                self.unlock(id, out);
            }
        }
        if self.run_shots_fired > 100 {
            let ratio = self.run_shots_hit * 100 / self.run_shots_fired;
            if ratio >= 50 {
                self.unlock("SHARPSHOOTER", out);
            }
            if ratio >= 75 {
                self.unlock("SNIPER", out);
            }
        }
        for (n, id) in [(5, "LEVEL_5"), (20, "LEVEL_20"), (50, "LEVEL_50")] {
            if g.levels_completed >= n {
                self.unlock(id, out);
            }
        }
        for (n, id) in [(3_600_000u64, "MARATHON"), (36_000_000, "DEDICATED"), (180_000_000, "OBSESSED")] {
            if g.time_played_ms >= n {
                self.unlock(id, out);
            }
        }
        for (n, id) in [(100_000, "RICH"), (1_000_000, "MILLIONAIRE")] {
            if g.total_score >= n {
                self.unlock(id, out);
            }
        }
        for (n, id) in [(100, "DIED_100"), (250, "PERSEVERANCE"), (500, "DIE_HARD")] {
            if g.deaths >= n {
                self.unlock(id, out);
            }
        }
        for (n, id) in [(10, "GAMES_10"), (50, "GAMES_50")] {
            if g.games_played >= n {
                self.unlock(id, out);
            }
        }
        for (n, id) in [(50, "DPS_50"), (100, "DPS_100"), (500, "OVERKILL")] {
            if self.dps_current >= n {
                self.unlock(id, out);
            }
        }
        if self.kills_in_window >= 10 {
            self.unlock("COMBO_KING", out);
        }
        if self.kills_in_window >= 25 {
            self.unlock("EXECUTION_CHAIN", out);
        }
        // Pacifist: 30 s without firing, measured over the current level.
        if self.last_shot_ms > 0
            && self.level_start_ms > 0
            && self.last_shot_ms >= self.level_start_ms
            && now.wrapping_sub(self.last_shot_ms) >= 30_000
        {
            self.unlock("PACIFIST", out);
        }
        if self.bullet_time_uses >= 50 {
            self.unlock("SLOW_MOTION_PRO", out);
        }
        if self.screenshots_taken >= 25 {
            self.unlock("SCREENSHOT_PRO", out);
        }
        if self.mirror_levels_completed >= 10 {
            self.unlock("MIRROR_VETERAN", out);
        }
        // Night Owl: 2 h of session, in wall time.
        if self.session_start_wall_ms.is_some_and(|t| self.wall_ms.wrapping_sub(t) >= 7_200_000) {
            self.unlock("NIGHT_OWL", out);
        }
        if self.total_credits_spent >= 50_000 {
            self.unlock("SHOPAHOLIC", out);
        }
        if v.risk_reward && g.total_score >= 500_000 {
            self.unlock("HIGH_ROLLER", out);
        }
        // Polyglot: the current language enters the mask, all six are enough.
        if (0..6).contains(&self.language) {
            self.languages_played_mask |= 1u64 << self.language;
            if self.languages_played_mask & 0x3F == 0x3F {
                self.unlock("POLYGLOT", out);
            }
        }
        if self.episodes_completed_mask & 0xF == 0xF {
            self.unlock("ALL_EPISODES", out);
        }
        if self.local_hour.is_some_and(|h| h < 5) {
            self.unlock("NIGHT_PLAYER", out);
        }
        if self.lifetime_credits >= 100_000 {
            self.unlock("MEGA_RICH", out);
        }
        if self.replays_saved >= 5 {
            self.unlock("FILMMAKER", out);
        }
        if g.levels_completed >= 100 {
            self.unlock("LEVEL_100", out);
        }
        if g.total_score >= 10_000_000 {
            self.unlock("BILLIONAIRE", out);
        }
        if self.dps_current >= 1000 {
            self.unlock("OVERCLOCK", out);
        }
        if v.cash >= 500_000 {
            self.unlock("PATIENT_INVESTOR", out);
        }
        let count = self.unlocked.iter().filter(|&&u| u).count();
        if count >= 50 {
            self.unlock("COMPLETIONIST", out);
        }
        if count >= 75 {
            self.unlock("ACHIEVEMENT_HUNTER", out);
        }
        if let Some(wave) = self.endless_wave {
            if wave >= 10 {
                self.unlock("ENDLESS_WARRIOR", out);
            }
            if wave >= 25 {
                self.unlock("ENDLESS_LEGEND", out);
            }
        }
        // Dodge Artist: 120 s without being hit, run active, one player alive.
        if self.run_active
            && self.last_hit_ms > 0
            && !v.all_players_gone
            && now.wrapping_sub(self.last_hit_ms) >= 120_000
            && v.any_alive
        {
            self.unlock("DODGE_ARTIST", out);
        }
        // Equipment worn, folded to 64 bits; identifier 0 is not an item.
        if self.run_active {
            if v.ship != 0 {
                self.ships_tried_mask |= 1u64 << (v.ship % 64);
            }
            for w in v.weapons {
                if w != 0 {
                    self.weapons_used_mask |= 1u64 << (w % 64);
                }
            }
            for s in v.sidekicks {
                if s != 0 {
                    self.sidekicks_used_mask |= 1u64 << (s % 64);
                }
            }
        }
        if self.total_bosses_defeated >= 10 {
            self.unlock("BOSS_RUSH", out);
        }
        if self.mutator_levels_completed >= 3 {
            self.unlock("MUTATOR_COLLECTOR", out);
        }
        let bought = self.weapons_bought_mask.count_ones() + self.weapons_bought_mask_hi.count_ones();
        if self.weapons_used_mask.count_ones() >= 30 {
            self.unlock("ARSENAL_MASTER", out);
        }
        if self.ships_tried_mask.count_ones() >= 10 {
            self.unlock("SHIP_COLLECTOR", out);
        }
        if self.sidekicks_used_mask.count_ones() >= 10 {
            self.unlock("SIDEKICK_SWAP", out);
        }
        if bought >= 30 {
            self.unlock("WEAPON_COLLECTOR", out);
        }
        if self.items_bought_this_shop >= 10 {
            self.unlock("FULL_HOUSE", out);
        }
        // Comeback Kid: at most 10% armor after 60 s of level.
        if self.level_start_ms > 0 && now.wrapping_sub(self.level_start_ms) >= 60_000 {
            let initial = if v.initial_armor != 0 { v.initial_armor } else { 28 };
            if v.armor * 10 <= initial && v.alive {
                self.half_level_low_hp = true;
            }
        }
    }

    /// Check outside a level (F3, title achievements screen): same rules,
    /// on the player's last known state.
    pub fn check_now(&mut self, v: &CheckView) -> Vec<usize> {
        let mut out = Vec::new();
        self.check(self.now_ms(), v, &mut out);
        self.log.extend_from_slice(&out);
        out
    }

    /// An achievement with no rule of its own (`achievements_unlock` called directly:
    /// Arcade Entry, ...). Returns the newly unlocked indices.
    pub fn unlock_named(&mut self, id: &str) -> Vec<usize> {
        let mut out = Vec::new();
        if !self.spectator {
            self.unlock(id, &mut out);
        }
        self.log.extend_from_slice(&out);
        out
    }

    /// `game_menu.c:shop_commit_done`: a confirmed purchase.
    pub fn note_purchase(&mut self, spent: u64, weapon_id: u8) {
        if self.spectator {
            return;
        }
        self.total_credits_spent = self.total_credits_spent.wrapping_add(spent as u32);
        self.items_bought_this_shop += 1;
        // stats.c:stats_mark_weapon_bought: ports 1..64 then 65..128.
        if (1..=64).contains(&weapon_id) {
            self.weapons_bought_mask |= 1u64 << (weapon_id - 1);
        } else if (65..=128).contains(&weapon_id) {
            self.weapons_bought_mask_hi |= 1u64 << (weapon_id - 65);
        }
    }

    /// `game_menu.c:JE_itemScreen`: shop opening.
    pub fn shop_opened(&mut self) {
        self.items_bought_this_shop = 0;
    }

    /// `achievements.c:achievements_on_episode_complete` (JE_nextEpisode).
    pub fn episode_complete(&mut self, episode: u8, mutators: u8) -> Vec<usize> {
        let mut out = Vec::new();
        if self.spectator || !(1..=5).contains(&episode) {
            return out;
        }
        self.unlock("EPISODE_CLEAR", &mut out);
        self.episodes_completed_mask |= 1u64 << (episode - 1);
        if self.episodes_completed_mask & 0xF == 0xF {
            self.unlock("ALL_EPISODES", &mut out);
        }
        if self.episode_start_episode == episode as u32 && self.episode_death_count == 0 {
            self.unlock("FLAWLESS_EPISODE", &mut out);
        }
        if mutators & MUT_ONE_LIFE != 0 {
            self.unlock("ONE_LIFE_EPISODE", &mut out);
        }
        if self.run_had_mutator {
            self.chaos_episodes_mask |= 1u64 << (episode - 1);
            if self.chaos_episodes_mask & 0xF == 0xF {
                self.unlock("CHAOS_THEORY", &mut out);
            }
        }
        self.episode_death_count = 0;
        self.episode_start_episode = 0;
        self.run_had_mutator = false;
        self.log.extend_from_slice(&out);
        out
    }

    /// Risk Taker: with Risk/Reward, 4 score tiers (100k, 250k,
    /// 500k, 1M) crossed in the run (`run_score_total`, not the lifetime score).
    pub fn check_risk_taker(&mut self, risk_reward: bool, out: &mut Vec<usize>) {
        if !risk_reward {
            return;
        }
        for (i, tier) in [100_000u64, 250_000, 500_000, 1_000_000].into_iter().enumerate() {
            if self.run_score_total >= tier {
                self.run_risk_tiers_crossed |= 1 << i;
            }
        }
        if self.run_risk_tiers_crossed & 0xF == 0xF {
            self.unlock("RISK_TAKER", out);
        }
    }

    /// `achievements.c:achievements_check_kills`.
    fn check_kills(&mut self, out: &mut Vec<usize>) {
        for (n, id) in
            [(1, "FIRST_BLOOD"), (100, "CENTURION"), (1000, "DESTROYER"), (10_000, "ANNIHILATOR"), (50_000, "GENOCIDE")]
        {
            if self.game_stats.enemies_killed >= n {
                self.unlock(id, out);
            }
        }
    }

    /// `achievements_check_level` (simulation part). Adds the newly
    /// unlocked indices to `log` and returns those of this call, in C order.
    pub fn level_complete(&mut self, sim: &Sim) -> Vec<usize> {
        let mut out = Vec::new();
        if self.spectator {
            return out;
        }
        let w = sim.world();
        let p = &w.players[0];
        // stats.c:stats_on_level_complete: stats_count_level_complete then
        // stats_bank_level_earnings.
        self.game_stats.levels_completed += 1;
        self.run_stats.levels_completed += 1;
        self.bank_level_earnings(p.cash, w.mode.mutators);
        if self.deaths_this_level == 0 {
            self.unlock("SURVIVOR", &mut out);
            self.deathless_streak += 1;
            for (n, id) in [(5, "UNTOUCHABLE"), (10, "IRONMAN"), (20, "IMMORTAL"), (50, "ADAMANTIUM")] {
                if self.deathless_streak >= n {
                    self.unlock(id, &mut out);
                }
            }
        } else {
            self.deathless_streak = 0;
            self.unlock("PHOENIX", &mut out);
        }
        // Deaths of the current episode (Flawless Episode).
        if self.episode_start_episode != w.episode as u32 {
            self.episode_start_episode = w.episode as u32;
            self.episode_death_count = 0;
        }
        self.episode_death_count += self.deaths_this_level;
        if p.armor <= 1 && p.is_alive {
            self.unlock("CLOSE_CALL", &mut out);
        }
        let m = &w.mode;
        if m.mutator(MUT_GLASS_CANNON) {
            self.unlock("GLASS_CANNON", &mut out);
        }
        if p.items.weapon[0].power >= 11 || p.items.weapon[1].power >= 11 {
            self.unlock("MAX_POWER", &mut out);
        }
        if w.initial_difficulty >= DIFFICULTY_HARD {
            self.unlock("HARD_MODE", &mut out);
        }
        if w.initial_difficulty >= DIFFICULTY_IMPOSSIBLE {
            self.unlock("IMPOSSIBLE", &mut out);
        }
        if m.mutator(MUT_MIRROR) {
            self.unlock("MIRROR_MASTER", &mut out);
            self.mirror_levels_completed += 1;
        }
        // mutators.c:mutators_active_count: the eight mutators.
        let active = m.mutators.count_ones();
        for (n, id) in [(1, "MODIFIER"), (3, "DAREDEVIL"), (8, "INSANITY")] {
            if active >= n {
                self.unlock(id, &mut out);
            }
        }
        if m.mutator(MUT_FAST_ENEMIES) {
            self.unlock("SPEEDSTER", &mut out);
        }
        if m.mutator(MUT_GLASS_CANNON) && w.episode == 1 {
            self.unlock("GLASS_HOUSE", &mut out);
        }
        if m.mutator(MUT_ONE_LIFE) {
            self.one_life_levels += 1;
            if self.one_life_levels >= 5 {
                self.unlock("MUT_SURVIVOR", &mut out);
            }
        }
        if self.shots_fired_this_level == 0 {
            self.unlock("TRUE_PACIFIST", &mut out);
        }
        if self.kills_this_level == 0 {
            self.unlock("ZEN_MASTER", &mut out);
        }
        if self.shots_fired_this_level > 0 && self.kills_this_level >= self.shots_fired_this_level {
            self.unlock("PERFECT_SCORE", &mut out);
        }
        let now = self.now_ms();
        if self.level_start_ms > 0 && now.wrapping_sub(self.level_start_ms) < 90_000 {
            self.unlock("SPEED_DEMON", &mut out);
        }
        if self.level_start_hp > 0 && self.level_damage_taken == 0 {
            self.unlock("BULLETPROOF", &mut out);
        }
        if self.level_start_hp == 1 && p.is_alive {
            self.unlock("LAST_STAND", &mut out);
        }
        if self.level_damage_taken == 0 {
            self.zero_damage_level_streak += 1;
            if self.zero_damage_level_streak >= 3 {
                self.unlock("IRON_CURTAIN", &mut out);
            }
        } else {
            self.zero_damage_level_streak = 0;
        }
        if active >= 1 {
            self.run_had_mutator = true;
            self.mutator_levels_completed += 1;
            if self.mutator_levels_completed >= 3 {
                self.unlock("MUTATOR_COLLECTOR", &mut out);
            }
        }
        self.check_risk_taker(m.mutator(MUT_RISK_REWARD), &mut out);
        if self.half_level_low_hp {
            self.unlock("COMEBACK_KID", &mut out);
        }
        if m.mutator(MUT_GLASS_CANNON) && m.mutator(MUT_ONE_LIFE) {
            self.unlock("GLASS_MASTER", &mut out);
        }
        if self.enabled_mod_count > 0 {
            self.unlock("MODDED_RUN", &mut out);
        }
        self.log.extend_from_slice(&out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> AchievementTracker {
        AchievementTracker::new([false; ACH_MAX], false)
    }

    #[test]
    fn screenshot_unlocks_photographer_then_pro_at_25() {
        let mut t = tracker();
        assert_eq!(t.note_screenshot(), vec![achievement_index("PHOTOGRAPHER").unwrap()]);
        for _ in 0..23 {
            assert!(t.note_screenshot().is_empty());
        }
        assert_eq!(t.note_screenshot(), vec![achievement_index("SCREENSHOT_PRO").unwrap()]);
        assert_eq!(t.screenshots_taken, 25);
        assert!(t.note_screenshot().is_empty());
    }

    #[test]
    fn bullet_time_unlocks_then_slow_motion_pro_at_50() {
        let mut t = tracker();
        t.bullet_time_uses = 1;
        assert_eq!(t.note_bullet_time(), vec![achievement_index("BULLET_TIME").unwrap()]);
        t.bullet_time_uses = 49;
        assert!(t.note_bullet_time().is_empty());
        t.bullet_time_uses = 50;
        assert_eq!(t.note_bullet_time(), vec![achievement_index("SLOW_MOTION_PRO").unwrap()]);
    }

    /// A-07 / A-11 / B-08: lifetime thresholds tested at each frame, not only on activation.
    #[test]
    fn lifetime_thresholds_checked_every_tick() {
        let mut t = tracker();
        t.bullet_time_uses = 50;
        t.replays_saved = 5;
        t.local_hour = Some(3);
        let mut out = Vec::new();
        t.check_lifetime(&mut out);
        let want: Vec<usize> =
            ["SLOW_MOTION_PRO", "FILMMAKER", "NIGHT_PLAYER"].iter().map(|n| achievement_index(n).unwrap()).collect();
        assert_eq!(out, want);
        let mut t = tracker();
        t.local_hour = Some(5);
        t.bullet_time_uses = 49;
        t.replays_saved = 4;
        let mut out = Vec::new();
        t.check_lifetime(&mut out);
        assert!(out.is_empty(), "5 h, 49 uses, 4 replays: nothing");
    }

    #[test]
    fn fifth_saved_replay_unlocks_filmmaker() {
        let mut t = tracker();
        for _ in 0..4 {
            assert!(t.note_replay_saved().is_empty());
        }
        assert_eq!(t.note_replay_saved(), vec![achievement_index("FILMMAKER").unwrap()]);
        assert_eq!(t.lifetime().replays_saved, 5);
    }

    #[test]
    fn spectator_unlocks_nothing() {
        let mut t = AchievementTracker::new([false; ACH_MAX], true);
        assert!(t.note_screenshot().is_empty() && t.note_bullet_time().is_empty());
    }

    #[test]
    fn banking_counts_each_credit_once() {
        let mut t = tracker();
        t.credits_start = 1000;
        t.bank_level_earnings(1500, 0);
        assert_eq!(t.game_stats.total_score, 500);
        t.bank_level_earnings(1500, 0); // already banked
        t.bank_level_earnings(1800, 0); // Endless death then level end
        assert_eq!((t.game_stats.total_score, t.run_stats.total_score), (800, 800));
        t.bank_level_earnings(900, 0); // the player spent: nothing
        assert_eq!(t.game_stats.total_score, 800);
    }

    #[test]
    fn run_start_resets_run_but_not_lifetime() {
        let mut t = tracker();
        t.game_stats.enemies_killed = 7;
        t.run_stats.enemies_killed = 7;
        t.count_game_start();
        t.run_start();
        assert!(t.run_active);
        assert_eq!((t.game_stats.enemies_killed, t.run_stats.enemies_killed), (7, 0));
        assert_eq!((t.game_stats.games_played, t.run_stats.games_played), (1, 0));
        t.run_end();
        assert!(!t.run_active);
    }

    #[test]
    fn lifetime_counters_roundtrip_through_block() {
        let mut t = tracker();
        t.game_stats = GameStats::from_array([1, 2, 3, 4, 5, 6, 7]);
        t.screenshots_taken = 8;
        t.bullet_time_uses = 9;
        let mut b = AchievementBlock::default();
        b.set_counters(&t.lifetime());
        let mut t2 = tracker();
        t2.set_lifetime(&AchievementBlock::parse(&b.write()).unwrap().counters());
        assert_eq!(t2.game_stats, t.game_stats);
        assert_eq!((t2.screenshots_taken, t2.bullet_time_uses), (8, 9));
    }

    // --- The 46 achievements that only the C code unlocked: one rule, one test. -------

    /// R-06: in play, the achievement clock is real time (`SDL_GetTicks`, stats.c:337),
    /// not 28.75 ms per frame; under the golden masters it stays virtual.
    #[test]
    fn real_clock_follows_wall_time_not_frames() {
        let mut t = tracker();
        t.frames = 1000;
        assert_eq!(t.now_ms(), clock_ms(1000));
        t.real_clock = true;
        t.wall_ms = 40_000;
        assert_eq!(t.now_ms(), 1_040_000);
        // Turbo speed: 1000 frames are worth less than 28.75 s; the timer does not count them.
        t.frames = 5000;
        assert_eq!(t.now_ms(), 1_040_000);
    }

    fn idx(id: &str) -> usize {
        achievement_index(id).unwrap()
    }

    /// Runs `check` on a fresh tracker prepared by `setup`; returns the unlocked indices.
    fn checked(setup: impl FnOnce(&mut AchievementTracker)) -> Vec<usize> {
        let mut t = tracker();
        t.run_active = true;
        setup(&mut t);
        let mut out = Vec::new();
        t.check(
            clock_ms(t.frames),
            &CheckView { alive: true, any_alive: true, armor: 28, initial_armor: 28, ..Default::default() },
            &mut out,
        );
        out
    }

    fn has(out: &[usize], id: &str) -> bool {
        out.contains(&idx(id))
    }

    macro_rules! threshold_test {
        ($name:ident, $id:literal, $set:expr, $below:expr, $at:expr) => {
            #[test]
            fn $name() {
                let set = $set;
                let below = checked(|t| set(t, $below));
                assert!(!has(&below, $id), "{} must not unlock below the threshold", $id);
                let at = checked(|t| set(t, $at));
                assert!(has(&at, $id), "{} must unlock at the threshold", $id);
            }
        };
    }

    threshold_test!(
        trigger_happy_at_10000_shots,
        "TRIGGER_HAPPY",
        |t: &mut AchievementTracker, n: u64| t.game_stats.shots_fired = n,
        9_999,
        10_000
    );
    threshold_test!(
        bullet_storm_at_100000_shots,
        "BULLET_STORM",
        |t: &mut AchievementTracker, n: u64| t.game_stats.shots_fired = n,
        99_999,
        100_000
    );
    threshold_test!(
        level_5_at_5_levels,
        "LEVEL_5",
        |t: &mut AchievementTracker, n: u64| t.game_stats.levels_completed = n,
        4,
        5
    );
    threshold_test!(
        level_20_at_20_levels,
        "LEVEL_20",
        |t: &mut AchievementTracker, n: u64| t.game_stats.levels_completed = n,
        19,
        20
    );
    threshold_test!(
        level_50_at_50_levels,
        "LEVEL_50",
        |t: &mut AchievementTracker, n: u64| t.game_stats.levels_completed = n,
        49,
        50
    );
    threshold_test!(
        level_100_at_100_levels,
        "LEVEL_100",
        |t: &mut AchievementTracker, n: u64| t.game_stats.levels_completed = n,
        99,
        100
    );
    threshold_test!(
        marathon_at_one_hour,
        "MARATHON",
        |t: &mut AchievementTracker, n: u64| t.game_stats.time_played_ms = n,
        3_599_999,
        3_600_000
    );
    threshold_test!(
        dedicated_at_ten_hours,
        "DEDICATED",
        |t: &mut AchievementTracker, n: u64| t.game_stats.time_played_ms = n,
        35_999_999,
        36_000_000
    );
    threshold_test!(
        obsessed_at_fifty_hours,
        "OBSESSED",
        |t: &mut AchievementTracker, n: u64| t.game_stats.time_played_ms = n,
        179_999_999,
        180_000_000
    );
    threshold_test!(
        rich_at_100000_score,
        "RICH",
        |t: &mut AchievementTracker, n: u64| t.game_stats.total_score = n,
        99_999,
        100_000
    );
    threshold_test!(
        millionaire_at_1000000_score,
        "MILLIONAIRE",
        |t: &mut AchievementTracker, n: u64| t.game_stats.total_score = n,
        999_999,
        1_000_000
    );
    threshold_test!(
        billionaire_at_10m_score,
        "BILLIONAIRE",
        |t: &mut AchievementTracker, n: u64| t.game_stats.total_score = n,
        9_999_999,
        10_000_000
    );
    threshold_test!(
        died_100_at_100_deaths,
        "DIED_100",
        |t: &mut AchievementTracker, n: u64| t.game_stats.deaths = n,
        99,
        100
    );
    threshold_test!(
        perseverance_at_250_deaths,
        "PERSEVERANCE",
        |t: &mut AchievementTracker, n: u64| t.game_stats.deaths = n,
        249,
        250
    );
    threshold_test!(
        die_hard_at_500_deaths,
        "DIE_HARD",
        |t: &mut AchievementTracker, n: u64| t.game_stats.deaths = n,
        499,
        500
    );
    threshold_test!(
        games_10_at_10_games,
        "GAMES_10",
        |t: &mut AchievementTracker, n: u64| t.game_stats.games_played = n,
        9,
        10
    );
    threshold_test!(
        games_50_at_50_games,
        "GAMES_50",
        |t: &mut AchievementTracker, n: u64| t.game_stats.games_played = n,
        49,
        50
    );
    threshold_test!(
        overclock_at_1000_dps,
        "OVERCLOCK",
        |t: &mut AchievementTracker, n: u64| t.dps_current = n as u32,
        999,
        1000
    );
    threshold_test!(
        mirror_veteran_at_10_levels,
        "MIRROR_VETERAN",
        |t: &mut AchievementTracker, n: u64| t.mirror_levels_completed = n as u32,
        9,
        10
    );
    threshold_test!(
        shopaholic_at_50000_spent,
        "SHOPAHOLIC",
        |t: &mut AchievementTracker, n: u64| t.total_credits_spent = n as u32,
        49_999,
        50_000
    );
    threshold_test!(
        mega_rich_at_100k_lifetime_credits,
        "MEGA_RICH",
        |t: &mut AchievementTracker, n: u64| t.lifetime_credits = n,
        99_999,
        100_000
    );
    threshold_test!(
        boss_rush_at_10_bosses,
        "BOSS_RUSH",
        |t: &mut AchievementTracker, n: u64| t.total_bosses_defeated = n as u32,
        9,
        10
    );
    threshold_test!(
        endless_warrior_at_wave_10,
        "ENDLESS_WARRIOR",
        |t: &mut AchievementTracker, n: u64| t.endless_wave = Some(n as i32),
        9,
        10
    );
    threshold_test!(
        endless_legend_at_wave_25,
        "ENDLESS_LEGEND",
        |t: &mut AchievementTracker, n: u64| t.endless_wave = Some(n as i32),
        24,
        25
    );
    threshold_test!(
        night_owl_at_two_hours_of_session,
        "NIGHT_OWL",
        |t: &mut AchievementTracker, n: u64| {
            t.session_start_wall_ms = Some(1000);
            t.wall_ms = 1000 + n;
        },
        7_199_999,
        7_200_000
    );

    #[test]
    fn patient_investor_needs_500000_credits_at_once() {
        let mut t = tracker();
        let mut out = Vec::new();
        t.check(0, &CheckView { cash: 499_999, ..Default::default() }, &mut out);
        assert!(!has(&out, "PATIENT_INVESTOR"));
        t.check(0, &CheckView { cash: 500_000, ..Default::default() }, &mut out);
        assert!(has(&out, "PATIENT_INVESTOR"));
    }

    #[test]
    fn high_roller_needs_risk_reward_and_500k_score() {
        let mut t = tracker();
        t.game_stats.total_score = 500_000;
        let mut out = Vec::new();
        t.check(0, &CheckView::default(), &mut out);
        assert!(!has(&out, "HIGH_ROLLER"), "without Risk/Reward");
        t.check(0, &CheckView { risk_reward: true, ..Default::default() }, &mut out);
        assert!(has(&out, "HIGH_ROLLER"));
    }

    #[test]
    fn pacifist_after_30_seconds_without_shooting_in_the_level() {
        let mut t = tracker();
        t.level_start_ms = 1_000_000;
        t.last_shot_ms = 1_000_500;
        let mut out = Vec::new();
        t.check(1_030_499, &CheckView::default(), &mut out);
        assert!(!has(&out, "PACIFIST"));
        t.check(1_030_500, &CheckView::default(), &mut out);
        assert!(has(&out, "PACIFIST"));
        // A last shot from before the level (menu left open) does not count.
        let mut t = tracker();
        t.level_start_ms = 1_000_000;
        t.last_shot_ms = 999_000;
        let mut out = Vec::new();
        t.check(1_100_000, &CheckView::default(), &mut out);
        assert!(!has(&out, "PACIFIST"));
    }

    #[test]
    fn polyglot_after_the_six_languages() {
        let mut t = tracker();
        let mut out = Vec::new();
        for lang in 0..5 {
            t.language = lang;
            t.check(0, &CheckView::default(), &mut out);
        }
        assert!(!has(&out, "POLYGLOT"), "five languages are not enough");
        t.language = 5;
        t.check(0, &CheckView::default(), &mut out);
        assert!(has(&out, "POLYGLOT"), "Italian (the sixth) counts");
        assert_eq!(t.languages_played_mask, 0x3F);
    }

    #[test]
    fn all_episodes_needs_the_four_first_episode_bits() {
        let mut t = tracker();
        let mut out = Vec::new();
        for ep in 1..=3u8 {
            out.extend(t.episode_complete(ep, 0));
        }
        assert!(!has(&out, "ALL_EPISODES"));
        out.extend(t.episode_complete(4, 0));
        assert!(has(&out, "ALL_EPISODES"));
        // Episode 5 triggers Episode Clear but not All Episodes on its own.
        let mut t = tracker();
        assert!(!has(&t.episode_complete(5, 0), "ALL_EPISODES"));
    }

    #[test]
    fn episode_clear_on_any_of_the_five_episodes_only() {
        for ep in 1..=5u8 {
            assert!(has(&tracker().episode_complete(ep, 0), "EPISODE_CLEAR"), "episode {ep}");
        }
        assert!(tracker().episode_complete(0, 0).is_empty());
        assert!(tracker().episode_complete(6, 0).is_empty());
    }

    #[test]
    fn flawless_episode_needs_no_death_over_the_episode() {
        let mut t = tracker();
        t.episode_start_episode = 2;
        t.episode_death_count = 0;
        assert!(has(&t.episode_complete(2, 0), "FLAWLESS_EPISODE"));
        let mut t = tracker();
        t.episode_start_episode = 2;
        t.episode_death_count = 1;
        assert!(!has(&t.episode_complete(2, 0), "FLAWLESS_EPISODE"));
        let mut t = tracker();
        t.episode_start_episode = 1; // the episode started elsewhere
        assert!(!has(&t.episode_complete(2, 0), "FLAWLESS_EPISODE"));
    }

    #[test]
    fn one_life_episode_needs_the_mutator() {
        assert!(!has(&tracker().episode_complete(1, 0), "ONE_LIFE_EPISODE"));
        assert!(has(&tracker().episode_complete(1, MUT_ONE_LIFE), "ONE_LIFE_EPISODE"));
    }

    #[test]
    fn chaos_theory_needs_a_mutator_run_in_each_of_the_four_episodes() {
        let mut t = tracker();
        let mut out = Vec::new();
        for ep in 1..=4u8 {
            t.run_had_mutator = true;
            out.extend(t.episode_complete(ep, 0));
        }
        assert!(has(&out, "CHAOS_THEORY"));
        assert!(!t.run_had_mutator, "the episode end resets the flag to false");
        let mut t = tracker();
        let mut out = Vec::new();
        for ep in 1..=4u8 {
            t.run_had_mutator = ep != 3;
            out.extend(t.episode_complete(ep, 0));
        }
        assert!(!has(&out, "CHAOS_THEORY"), "an episode without a mutator breaks the streak");
    }

    #[test]
    fn episode_progress_survives_a_save_roundtrip() {
        let mut t = tracker();
        t.episode_complete(3, 0);
        t.chaos_episodes_mask = 0b101;
        t.languages_played_mask = 0b11;
        t.lifetime_credits = 123_456;
        t.weapons_bought_mask = 0xF0F0;
        t.weapons_bought_mask_hi = 0x3;
        t.total_credits_spent = 42;
        t.episode_death_count = 2;
        t.episode_start_episode = 3;
        t.total_bosses_defeated = 7;
        t.mutator_levels_completed = 2;
        t.deathless_streak = 4;
        t.zero_damage_level_streak = 1;
        let mut b = AchievementBlock::default();
        b.set_counters(&t.lifetime());
        let mut t2 = tracker();
        t2.set_lifetime(&AchievementBlock::parse(&b.write()).unwrap().counters());
        assert_eq!(t2.lifetime(), t.lifetime());
        assert_eq!(t2.episodes_completed_mask, 0b100);
    }

    #[test]
    fn full_house_after_ten_purchases_in_one_shop_visit() {
        let mut t = tracker();
        t.shop_opened();
        for _ in 0..9 {
            t.note_purchase(100, 0);
        }
        let mut out = Vec::new();
        t.check(0, &CheckView::default(), &mut out);
        assert!(!has(&out, "FULL_HOUSE"));
        t.note_purchase(100, 0);
        t.check(0, &CheckView::default(), &mut out);
        assert!(has(&out, "FULL_HOUSE"));
        t.shop_opened();
        assert_eq!(t.items_bought_this_shop, 0, "the next visit starts from zero");
    }

    #[test]
    fn weapon_collector_counts_bought_weapons_across_both_masks() {
        let mut t = tracker();
        for port in 1..=29u8 {
            t.note_purchase(10, port);
        }
        let mut out = Vec::new();
        t.check(0, &CheckView::default(), &mut out);
        assert!(!has(&out, "WEAPON_COLLECTOR"));
        t.note_purchase(10, 70); // port 70: high mask
        t.check(0, &CheckView::default(), &mut out);
        assert!(has(&out, "WEAPON_COLLECTOR"));
        t.note_purchase(10, 0); // not a weapon
        assert_eq!(t.weapons_bought_mask.count_ones() + t.weapons_bought_mask_hi.count_ones(), 30);
    }

    #[test]
    fn arsenal_master_counts_30_distinct_weapons_and_ignores_id_zero() {
        let mut t = tracker();
        t.run_active = true;
        let mut out = Vec::new();
        for w in 0..30u8 {
            t.check(0, &CheckView { weapons: [w, 0], ..Default::default() }, &mut out);
        }
        // 0 is "no weapon": only 29 distinct weapons.
        assert!(!has(&out, "ARSENAL_MASTER"));
        t.check(0, &CheckView { weapons: [30, 0], ..Default::default() }, &mut out);
        assert!(has(&out, "ARSENAL_MASTER"));
    }

    #[test]
    fn ship_collector_counts_10_distinct_ships() {
        let mut t = tracker();
        t.run_active = true;
        let mut out = Vec::new();
        for ship in 1..10u8 {
            t.check(0, &CheckView { ship, ..Default::default() }, &mut out);
        }
        assert!(!has(&out, "SHIP_COLLECTOR"));
        t.check(0, &CheckView { ship: 10, ..Default::default() }, &mut out);
        assert!(has(&out, "SHIP_COLLECTOR"));
    }

    #[test]
    fn sidekick_swap_counts_10_distinct_sidekicks() {
        let mut t = tracker();
        t.run_active = true;
        let mut out = Vec::new();
        for s in 1..10u8 {
            t.check(0, &CheckView { sidekicks: [s, 0], ..Default::default() }, &mut out);
        }
        assert!(!has(&out, "SIDEKICK_SWAP"));
        t.check(0, &CheckView { sidekicks: [0, 10], ..Default::default() }, &mut out);
        assert!(has(&out, "SIDEKICK_SWAP"));
    }

    #[test]
    fn collections_are_only_recorded_while_a_run_is_active() {
        let mut t = tracker();
        let mut out = Vec::new();
        t.check(0, &CheckView { ship: 3, weapons: [4, 5], sidekicks: [6, 7], ..Default::default() }, &mut out);
        assert_eq!((t.ships_tried_mask, t.weapons_used_mask, t.sidekicks_used_mask), (0, 0, 0));
    }

    #[test]
    fn completionist_and_achievement_hunter_count_unlocked() {
        let mut t = tracker();
        for i in 0..49 {
            t.unlocked[i] = true;
        }
        let mut out = Vec::new();
        t.check(0, &CheckView::default(), &mut out);
        assert!(!has(&out, "COMPLETIONIST"));
        t.unlocked[49] = true;
        t.check(0, &CheckView::default(), &mut out);
        assert!(has(&out, "COMPLETIONIST") && !has(&out, "ACHIEVEMENT_HUNTER"));
        for i in 0..74 {
            t.unlocked[i] = true;
        }
        t.unlocked[achievement_index("COMPLETIONIST").unwrap()] = true;
        let n = t.unlocked.iter().filter(|&&u| u).count();
        assert!(n >= 74);
        t.unlocked[74] = true;
        t.unlocked[75] = true;
        t.check(0, &CheckView::default(), &mut out);
        assert!(has(&out, "ACHIEVEMENT_HUNTER"));
    }

    #[test]
    fn comeback_kid_after_a_minute_at_or_under_ten_percent_armor() {
        let mut t = tracker();
        t.level_start_ms = 1_000_000;
        let low = CheckView { armor: 2, initial_armor: 28, alive: true, ..Default::default() };
        let mut out = Vec::new();
        t.check(1_059_999, &low, &mut out);
        t.check(1_060_000, &CheckView { armor: 3, ..low }, &mut out);
        assert!(!t.half_level_low_hp, "3 × 10 > 28");
        t.check(1_060_000, &low, &mut out);
        assert!(t.half_level_low_hp);
    }

    #[test]
    fn arcade_entry_is_unlocked_by_name_and_silent_when_repeated() {
        let mut t = tracker();
        assert_eq!(t.unlock_named("ARCADE_ENTRY"), vec![idx("ARCADE_ENTRY")]);
        assert!(t.unlock_named("ARCADE_ENTRY").is_empty());
        let mut s = AchievementTracker::new([false; ACH_MAX], true);
        assert!(s.unlock_named("ARCADE_ENTRY").is_empty(), "spectator: nothing");
    }

    #[test]
    fn game_start_resets_the_run_scope_and_counts_a_game() {
        let mut t = tracker();
        t.run_had_mutator = true;
        t.run_score_total = 9;
        t.run_risk_tiers_crossed = 3;
        t.count_game_start();
        assert_eq!(
            (t.game_stats.games_played, t.run_had_mutator, t.run_score_total, t.run_risk_tiers_crossed),
            (1, false, 0, 0)
        );
    }

    #[test]
    fn bullet_hell_at_50_on_screen_shots_and_blizzard_at_100() {
        let mut t = tracker();
        let mut out = Vec::new();
        t.on_screen_shots(49, &mut out);
        assert!(out.is_empty());
        t.on_screen_shots(50, &mut out);
        assert_eq!(out, vec![idx("BULLET_HELL")]);
        t.on_screen_shots(99, &mut out);
        assert_eq!(out.len(), 1);
        t.on_screen_shots(100, &mut out);
        assert_eq!(out, vec![idx("BULLET_HELL"), idx("BULLET_BLIZZARD")]);
    }

    #[test]
    fn risk_taker_needs_risk_reward_and_the_four_run_score_tiers() {
        let mut t = tracker();
        t.game_stats.total_score = 9_000_000; // well-stocked profile: does not count
        let mut out = Vec::new();
        t.check_risk_taker(true, &mut out);
        assert!(out.is_empty(), "the lifetime score alone is not enough");
        t.run_score_total = 999_999;
        t.check_risk_taker(true, &mut out);
        assert!(out.is_empty(), "three tiers out of four");
        t.run_score_total = 1_000_000;
        t.check_risk_taker(false, &mut out);
        assert!(out.is_empty(), "without Risk/Reward");
        t.check_risk_taker(true, &mut out);
        assert_eq!(out, vec![idx("RISK_TAKER")]);
    }
}
