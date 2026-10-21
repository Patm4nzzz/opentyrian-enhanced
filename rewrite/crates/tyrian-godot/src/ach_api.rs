//! Unlocked achievements read from an `opentyrian.dat` (container `opentyrian_dat.c`,
//! GAME section, OTYE block of `config.c:JE_saveConfiguration`) for the achievements
//! screen (`achievements.c:achievements_display`), tracking of unlocks
//! in game (`achievements.c:achievements_check` via `AchievementTracker`) for
//! the "Achievement: ..." banner and their saving into `opentyrian.dat`
//! (`achievements_unlock` -> `JE_saveConfiguration`).

use std::cell::RefCell;

use godot::prelude::*;
use tyrian_formats::achievements::{AchievementBlock, LifetimeCounters, ACH_MAX};
use tyrian_formats::ot_dat::OtDat;
use tyrian_sim::achievements::{AchievementTracker, CheckView};

use crate::vga::TyrianVga;
use crate::TyrianSim;

thread_local! {
    /// Achievement tracking for the session: lifetime counters and unlocks
    /// persist from one level to the next like the C globals.
    static TRACKER: RefCell<Option<AchievementTracker>> = const { RefCell::new(None) };
}

/// Unlocks and lifetime counters of the GAME section (`JE_loadConfiguration`,
/// OTYE block then OTY2 / OTY3 extensions); `None` if the file has none.
fn block_from_dat(path: &str) -> Option<AchievementBlock> {
    std::fs::read(path)
        .ok()
        .and_then(|b| OtDat::parse(&b).ok())
        .and_then(|d| d.section(b"GAME").and_then(AchievementBlock::at_exact_position))
}

fn new_tracker(dat_path: &str, spectator: bool) -> AchievementTracker {
    let mut tr = match if dat_path.is_empty() { None } else { block_from_dat(dat_path) } {
        Some(b) => {
            let mut tr = AchievementTracker::new(b.unlocked, spectator);
            tr.set_lifetime(&b.counters());
            tr
        }
        None => AchievementTracker::new([false; ACH_MAX], spectator),
    };
    // Real time for a player (R-06); the tools (capture, replay, bot) pass an
    // empty path (Game.ach_dat_path) and keep the C golden virtual clock
    // (GOLDEN_ACH_CLOCK), otherwise the achievement banners no longer fall on the same tick.
    tr.real_clock = !dat_path.is_empty();
    tr
}

/// Real `SDL_GetTicks()`: session duration for Night Owl.
fn wall_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

fn ids(v: Vec<usize>) -> PackedInt32Array {
    v.into_iter().map(|i| i as i32).collect()
}

#[godot_api(secondary)]
impl TyrianSim {
    /// Level start: creates the tracking on the first call (unlocks read from
    /// `dat_path`, all locked if empty), then `level_start`.
    /// `spectator`: replay playback (`ach_spectator_mode`), no unlocks.
    #[func]
    fn ach_level_start(&self, dat_path: GString, spectator: bool) {
        let Some(sim) = self.sim.as_ref() else { return };
        TRACKER.with(|t| {
            let mut t = t.borrow_mut();
            let tr = t.get_or_insert_with(|| new_tracker(&dat_path.to_string(), spectator));
            tr.spectator = spectator;
            tr.wall_ms = wall_ms();
            tr.level_start(sim);
        });
    }

    /// After each tick: achievements unlocked during this tick, in C order.
    #[func]
    fn ach_after_tick(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        TRACKER.with(|t| {
            let mut t = t.borrow_mut();
            let Some(tr) = t.as_mut() else {
                return PackedInt32Array::new();
            };
            let n = tr.log.len();
            tr.wall_ms = wall_ms();
            tr.after_tick(sim);
            tr.log[n..].iter().map(|&i| i as i32).collect()
        })
    }

    /// Level end (mainint.c, EVENT_LEVEL_COMPLETE): achievements unlocked.
    #[func]
    fn ach_level_complete(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        TRACKER.with(|t| {
            let mut t = t.borrow_mut();
            let Some(tr) = t.as_mut() else {
                return PackedInt32Array::new();
            };
            tr.wall_ms = wall_ms();
            tr.level_complete(sim).into_iter().map(|i| i as i32).collect()
        })
    }

    /// `stats.c`: `game_stats` (`game_*`, lifetime) and `run_stats` (`run_*`),
    /// `run_active`, `dps` (`dps.c:dps_current`). All as 64-bit integers.
    #[func]
    fn ach_stats(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        TRACKER.with(|t| {
            let t = t.borrow();
            let Some(tr) = t.as_ref() else { return };
            let (g, r) = (&tr.game_stats, &tr.run_stats);
            for (k, v) in [
                ("run_kills", r.enemies_killed),
                ("run_shots", r.shots_fired),
                ("run_deaths", r.deaths),
                ("run_levels", r.levels_completed),
                ("run_score", r.total_score),
                ("run_time_ms", r.time_played_ms),
                ("game_kills", g.enemies_killed),
                ("game_shots", g.shots_fired),
                ("game_deaths", g.deaths),
                ("game_levels", g.levels_completed),
                ("game_score", g.total_score),
                ("game_time_ms", g.time_played_ms),
                ("game_played", g.games_played),
                ("dps", tr.dps() as u64),
            ] {
                d.set(k, v as i64);
            }
            d.set("run_active", tr.run_active);
        });
        d
    }

    /// `stats.c:stats_run_start`: resets `run_stats` to zero. Already called by
    /// `ach_level_start` in the campaign; in Endless the GDScript calls it at
    /// the start of the run.
    #[func]
    fn ach_run_start(&self) {
        // stats.c:stats_run_start: in Endless, this call IS the start of the run, so the
        // "game" scope (Risk Taker...) restarts too.
        let endless = self.sim.as_ref().is_some_and(|s| s.world().endless_lives.is_some());
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.run_start();
                if endless {
                    tr.run_scope_reset();
                }
            }
        });
    }

    /// `stats.c:stats_run_end`: `run_active` goes back to false.
    #[func]
    fn ach_run_end(&self) {
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.run_end();
            }
        });
    }

    /// `stats.c:stats_count_game_start`: one more game (modes menu).
    #[func]
    fn ach_count_game_start(&self) {
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.count_game_start();
            }
        });
    }

    /// `input_map.c:input_action_screenshot` (successful capture): achievements
    /// unlocked (Photographer, Screenshot Pro), index in C order.
    #[func]
    fn ach_note_screenshot(&self) -> PackedInt32Array {
        TRACKER.with(|t| t.borrow_mut().as_mut().map(|tr| ids(tr.note_screenshot())).unwrap_or_default())
    }

    /// `bullet_time.c:bullet_time_uses` kept on the GDScript side: copies the total.
    #[func]
    fn ach_set_bullet_time_uses(&self, n: i64) {
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.bullet_time_uses = n.clamp(0, u32::MAX as i64) as u32;
            }
        });
    }

    /// Local hour (0..23) for Night Player (`achievements_check`: `localtime()`).
    #[func]
    fn ach_set_local_hour(&self, hour: i32) {
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.local_hour = (0..24).contains(&hour).then_some(hour as u8);
            }
        });
    }

    /// `mainint.c` (F10): a recording stopped by hand and saved (`replays_saved++`).
    #[func]
    fn ach_note_replay_saved(&self) -> PackedInt32Array {
        TRACKER.with(|t| t.borrow_mut().as_mut().map(|tr| ids(tr.note_replay_saved())).unwrap_or_default())
    }

    /// `input_map.c:input_action_bullet_time` (successful activation): achievements
    /// unlocked (Bullet Time, Slow Motion Pro at 50 uses).
    #[func]
    fn ach_note_bullet_time(&self) -> PackedInt32Array {
        TRACKER.with(|t| t.borrow_mut().as_mut().map(|tr| ids(tr.note_bullet_time())).unwrap_or_default())
    }

    /// 100 bytes 0/1: `achievements[i].unlocked` in memory (including the
    /// session's unlocks). Empty without tracking.
    #[func]
    fn ach_unlocked_flags(&self) -> PackedByteArray {
        TRACKER
            .with(|t| t.borrow().as_ref().map(|tr| tr.unlocked.iter().map(|&u| u as u8).collect()).unwrap_or_default())
    }

    /// In-memory lifetime counters for the tests: `bullet_time_uses`, `screenshots_taken`.
    #[func]
    fn ach_counter(&self, name: GString) -> i64 {
        TRACKER.with(|t| {
            t.borrow().as_ref().map_or(0, |tr| match name.to_string().as_str() {
                "bullet_time_uses" => tr.bullet_time_uses as i64,
                "screenshots_taken" => tr.screenshots_taken as i64,
                "replays_saved" => tr.replays_saved as i64,
                "games_played" => tr.game_stats.games_played as i64,
                _ => 0,
            })
        })
    }

    /// Context read by `achievements_check`: current language (0..5),
    /// Endless wave (negative outside Endless), active mods.
    #[func]
    fn ach_set_context(&self, language: i32, endless_wave: i32, enabled_mods: i32) {
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.language = language;
                tr.endless_wave = (endless_wave >= 0).then_some(endless_wave);
                tr.enabled_mod_count = enabled_mods.max(0) as u32;
            }
        });
    }

    /// F3 in a level (`input_action_toggle_achievements` -> `achievements_check`).
    #[func]
    fn ach_check_now(&self) -> PackedInt32Array {
        let Some(sim) = self.sim.as_ref() else {
            return PackedInt32Array::new();
        };
        TRACKER.with(|t| {
            let mut t = t.borrow_mut();
            let Some(tr) = t.as_mut() else {
                return PackedInt32Array::new();
            };
            tr.wall_ms = wall_ms();
            ids(tr.check_now(&CheckView::of(sim.world())))
        })
    }

    /// End of episode (`mainint.c:JE_nextEpisode`): episode achievements.
    #[func]
    fn ach_episode_complete(&self, episode: i32, mutators: i32) -> PackedInt32Array {
        TRACKER.with(|t| {
            t.borrow_mut()
                .as_mut()
                .map(|tr| ids(tr.episode_complete(episode.clamp(0, 255) as u8, mutators.clamp(0, 255) as u8)))
                .unwrap_or_default()
        })
    }

    /// Achievement without its own rule, unlocked directly (Arcade Entry).
    #[func]
    fn ach_unlock_named(&self, id: GString) -> PackedInt32Array {
        TRACKER.with(|t| t.borrow_mut().as_mut().map(|tr| ids(tr.unlock_named(&id.to_string()))).unwrap_or_default())
    }

    /// `game_menu.c:shop_commit_done`: purchase confirmed (credits spent, weapon port
    /// bought, 0 if another category).
    #[func]
    fn ach_note_purchase(&self, spent: i64, weapon_id: i32) -> PackedInt32Array {
        TRACKER.with(|t| {
            let mut t = t.borrow_mut();
            let Some(tr) = t.as_mut() else {
                return PackedInt32Array::new();
            };
            tr.note_purchase(spent.max(0) as u64, weapon_id.clamp(0, 255) as u8);
            PackedInt32Array::new()
        })
    }

    /// `JE_itemScreen`: opening of the shop.
    #[func]
    fn ach_shop_opened(&self) {
        TRACKER.with(|t| {
            if let Some(tr) = t.borrow_mut().as_mut() {
                tr.shop_opened();
            }
        });
    }

    /// Achievement clock in ms (`SDL_GetTicks`, real time).
    #[func]
    fn ach_clock_ms(&self) -> i64 {
        TRACKER.with(|t| t.borrow().as_ref().map_or(0, |tr| tr.now_ms() as i64))
    }
}

/// Shop purchases (`shop_commit_done`) and opening (`JE_itemScreen`).
pub(crate) fn shop_opened() {
    TRACKER.with(|t| {
        if let Some(tr) = t.borrow_mut().as_mut() {
            tr.shop_opened();
        }
    });
}

pub(crate) fn note_purchases(purchases: &[(u64, u8)]) {
    TRACKER.with(|t| {
        if let Some(tr) = t.borrow_mut().as_mut() {
            for &(spent, weapon) in purchases {
                tr.note_purchase(spent, weapon);
            }
        }
    });
}

/// `dev_console.c` Ctrl+Shift+U: "achievements[i].unlocked = true" for
/// all; creates the tracking if it does not exist yet (outside a level).
pub(crate) fn unlock_all() {
    TRACKER.with(|t| {
        let mut t = t.borrow_mut();
        let tr = t.get_or_insert_with(|| AchievementTracker::new([false; ACH_MAX], false));
        tr.unlocked = [true; ACH_MAX];
    });
}

#[godot_api(secondary)]
impl TyrianSim {
    /// Resets the achievement tracking (integration tests: each independent
    /// suite must start from zero, like a fresh launch of the game).
    #[func]
    fn ach_tracker_reset(&self) {
        TRACKER.with(|t| *t.borrow_mut() = None);
    }
}

/// 24-bit BMP (14 + 40 byte headers, rows bottom-up aligned on
/// 4 bytes) of an RGBA image: the `SDL_SaveBMP` format.
fn encode_bmp(w: usize, h: usize, rgba: &[u8]) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return None;
    }
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let i = (y * w + x) * 4;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i]]);
        }
        out.resize(out.len() + row - w * 3, 0);
    }
    Some(out)
}

#[cfg(test)]
mod bmp_tests {
    use super::encode_bmp;

    #[test]
    fn bmp_rows_are_bottom_up_bgr_padded_to_four_bytes() {
        // 2 x 2: top = red, green; bottom = blue, white.
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        let b = encode_bmp(2, 2, &rgba).unwrap();
        assert_eq!(&b[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize, b.len());
        assert_eq!(b.len(), 54 + 8 * 2);
        // first row written = bottom row (blue, white), in BGR, 2 padding bytes.
        assert_eq!(&b[54..62], &[255, 0, 0, 255, 255, 255, 0, 0]);
        assert_eq!(&b[62..70], &[0, 0, 255, 0, 255, 0, 0, 0]);
        assert!(encode_bmp(0, 2, &rgba).is_none());
        assert!(encode_bmp(2, 2, &rgba[..8]).is_none());
    }
}

/// Tracking created as needed (outside a level: game start, shop, title).
fn with_tracker<R>(dat_path: &str, f: impl FnOnce(&mut AchievementTracker) -> R) -> R {
    TRACKER.with(|t| {
        let mut t = t.borrow_mut();
        let tr = t.get_or_insert_with(|| new_tracker(dat_path, false));
        tr.wall_ms = wall_ms();
        f(tr)
    })
}

#[godot_api(secondary)]
impl TyrianVga {
    /// Creates the tracking if needed (reads `opentyrian.dat`), with no other effect.
    #[func]
    fn ach_ensure(dat_path: GString) {
        with_tracker(&dat_path.to_string(), |_| ());
    }

    /// `achievements_check` outside a level (achievements screen of the title): current
    /// language, local hour and active mods set, same rules as in game,
    /// then a write if something was unlocked. Returns the 100 bytes 0/1.
    #[func]
    fn ach_title_check(dat_path: GString, language: i32, local_hour: i32, enabled_mods: i32) -> PackedByteArray {
        let path = dat_path.to_string();
        let (new, flags) = with_tracker(&path, |tr| {
            tr.language = language;
            tr.local_hour = (0..24).contains(&local_hour).then_some(local_hour as u8);
            tr.enabled_mod_count = enabled_mods.max(0) as u32;
            let new = tr.check_now(&CheckView::default());
            (new, tr.unlocked.iter().map(|&u| u as u8).collect::<PackedByteArray>())
        });
        if !new.is_empty() && !path.is_empty() {
            save_tracker(&path);
        }
        flags
    }

    /// `mainint.c:JE_nextEpisode`: end-of-episode achievements.
    #[func]
    fn ach_episode_done(dat_path: GString, episode: i32, mutators: i32) -> PackedInt32Array {
        with_tracker(&dat_path.to_string(), |tr| {
            ids(tr.episode_complete(episode.clamp(0, 255) as u8, mutators.clamp(0, 255) as u8))
        })
    }

    /// `achievements_unlock` directly (Arcade Entry).
    #[func]
    fn ach_unlock_by_name(dat_path: GString, id: GString) -> PackedInt32Array {
        with_tracker(&dat_path.to_string(), |tr| ids(tr.unlock_named(&id.to_string())))
    }

    /// Only write path of the tracking into `dat_path` (OTYE block of the signed
    /// GAME section, created if absent, the rest of the file kept): game
    /// exit, level end, unlock, shop.
    #[func]
    fn ach_save_current(dat_path: GString) -> bool {
        save_tracker(&dat_path.to_string())
    }
}

/// Import of the achievements and statistics of the C profile (`c_dat`, OTYE block of the
/// GAME section) into the Godot file (`dest`, `None` if it does not exist):
/// without a Godot block, the C one is taken as is; with a Godot block already
/// played, the C achievements are added (union) and the Godot counters stay.
/// Returns the new contents of `dest`, or `None` if the C has nothing to give
/// (unreadable file, no OTYE block) or if nothing changes.
pub(crate) fn merge_c_achievements(c_dat: &[u8], dest: Option<&[u8]>) -> Option<Vec<u8>> {
    use tyrian_formats::save::{GameSave, OtDat as SaveDat};
    let c_block = OtDat::parse(c_dat)
        .ok()
        .and_then(|d| d.section(b"GAME").and_then(AchievementBlock::find_in_game_section).map(|(_, b)| b))?;
    let mut dat = dest.and_then(|b| SaveDat::parse(b).ok()).unwrap_or_default();
    let mut save = match dat.section(b"GAME") {
        Some(g) => GameSave::parse(g).unwrap_or_else(|_| GameSave::new_empty(&[0; 100])),
        None => GameSave::new_empty(&[0; 100]),
    };
    let block = match AchievementBlock::parse(&save.tail) {
        Ok(mut own) => {
            let before = own.unlocked;
            for i in 0..ACH_MAX {
                own.unlocked[i] |= c_block.unlocked[i];
            }
            if own.unlocked == before {
                return None;
            }
            own
        }
        Err(_) => c_block,
    };
    save.tail = block.write();
    dat.set_section(b"GAME", save.write(), true);
    Some(dat.write())
}

#[godot_api(secondary)]
impl TyrianVga {
    /// One-time import of the C profile's achievements: see `merge_c_achievements`.
    /// Never modifies `c_path`. Returns 1 if `dest_path` was written, 0 if there
    /// was nothing to add, -1 if the import failed (read or write):
    /// only a failure should be retried at the next launch. The in-memory tracking
    /// is reloaded from the written file, otherwise the next save
    /// would overwrite the imported union.
    #[func]
    fn import_c_achievements(c_path: GString, dest_path: GString) -> i32 {
        let Ok(c) = std::fs::read(c_path.to_string()) else {
            return -1;
        };
        let dest = std::fs::read(dest_path.to_string()).ok();
        match merge_c_achievements(&c, dest.as_deref()) {
            Some(out) => {
                if std::fs::write(dest_path.to_string(), out).is_err() {
                    return -1;
                }
                reload_tracker_from(&dest_path.to_string());
                1
            }
            None => 0,
        }
    }
}

/// Reloads unlocks and counters of the in-memory tracking (if it exists) from `p`.
fn reload_tracker_from(p: &str) {
    let Some(b) = block_from_dat(p) else { return };
    TRACKER.with(|t| {
        if let Some(tr) = t.borrow_mut().as_mut() {
            tr.unlocked = b.unlocked;
            tr.set_lifetime(&b.counters());
        }
    });
}

/// New contents of `opentyrian.dat`: the unlocks and counters set in
/// the OTYE block of the GAME section. An existing unreadable file (wrong hash,
/// unparseable GAME section) is discarded and rewritten cleanly, like
/// `opentyrian_dat.c:236-246` ("discarding"): otherwise nothing would ever
/// be saved again. The second result says whether the old contents were discarded.
fn rebuild_dat(existing: Option<&[u8]>, unlocked: [bool; ACH_MAX], counters: &LifetimeCounters) -> (Vec<u8>, bool) {
    use tyrian_formats::save::{GameSave, OtDat};
    let mut discarded = false;
    let mut dat = match existing {
        Some(b) => OtDat::parse(b).unwrap_or_else(|_| {
            discarded = true;
            OtDat::new()
        }),
        None => OtDat::new(),
    };
    let mut save = match dat.section(b"GAME") {
        Some(g) => GameSave::parse(g).unwrap_or_else(|_| {
            discarded = true;
            GameSave::new_empty(&[0; 100])
        }),
        None => GameSave::new_empty(&[0; 100]),
    };
    let mut block = AchievementBlock::parse(&save.tail).unwrap_or_default();
    block.unlocked = unlocked;
    block.set_counters(counters);
    save.tail = block.write();
    dat.set_section(b"GAME", save.write(), true);
    (dat.write(), discarded)
}

pub(crate) fn save_tracker(p: &str) -> bool {
    let Some((unlocked, counters)) = TRACKER.with(|t| t.borrow().as_ref().map(|tr| (tr.unlocked, tr.lifetime())))
    else {
        return false;
    };
    let existing = std::fs::read(p).ok();
    let (out, discarded) = rebuild_dat(existing.as_deref(), unlocked, &counters);
    if discarded {
        // The damaged old file is kept alongside, only once.
        let bak = format!("{p}.bak");
        if !std::path::Path::new(&bak).exists() {
            let _ = std::fs::copy(p, &bak);
        }
        godot_warn!("opentyrian.dat unreadable: discarded and rewritten ({p})");
    }
    match std::fs::write(p, out) {
        Ok(()) => true,
        Err(e) => {
            godot_error!("opentyrian.dat not written ({p}): {e}");
            false
        }
    }
}

#[godot_api(secondary)]
impl TyrianVga {
    /// `video.c:JE_saveScreenshot`: writes `rgba` (width x height, 4 bytes
    /// per pixel) as a 24-bit BMP, top to bottom inverted, like `SDL_SaveBMP`
    /// (14 + 40 byte headers, rows aligned on 4 bytes).
    #[func]
    fn save_bmp(path: GString, width: i32, height: i32, rgba: PackedByteArray) -> bool {
        match encode_bmp(width.max(0) as usize, height.max(0) as usize, rgba.as_slice()) {
            Some(bytes) => std::fs::write(path.to_string(), bytes).is_ok(),
            None => false,
        }
    }

    /// One byte 0/1 per achievement, in the order of the `ACH_*` enumeration
    /// (= on-disk order). File missing, container rejected or OTYE block
    /// missing: empty array (the C then keeps everything locked,
    /// `JE_loadConfiguration` without a valid save).
    #[func]
    fn achievements_unlocked_from_dat(path: GString) -> PackedByteArray {
        let Ok(bytes) = std::fs::read(path.to_string()) else {
            return PackedByteArray::new();
        };
        let Ok(dat) = OtDat::parse(&bytes) else {
            return PackedByteArray::new();
        };
        let Some(game) = dat.section(b"GAME") else {
            return PackedByteArray::new();
        };
        let Some(block) = AchievementBlock::at_exact_position(game) else {
            return PackedByteArray::new();
        };
        (0..ACH_MAX).map(|i| block.unlocked[i] as u8).collect()
    }

    /// `replay.c:replay_read_header` on a file: [episode, level,
    /// difficulty, mode, frames], empty if the header is rejected.
    #[func]
    fn replay_header(path: GString) -> PackedInt64Array {
        let Ok(b) = std::fs::read(path.to_string()) else {
            return PackedInt64Array::new();
        };
        match tyrian_formats::replay::read_header(&b) {
            Some(h) => {
                [h.episode as i64, h.level as i64, h.difficulty as i64, h.game_mode as i64, h.frame_count as i64]
                    .into_iter()
                    .collect()
            }
            None => PackedInt64Array::new(),
        }
    }

    /// Launching a replay from the menu (title_screen.c:replay_browser,
    /// Enter): 0 = playable, 1 = refused by replay_load (unreadable file,
    /// section without a level), 2 = no frames ("replay_empty"). Same checks as
    /// TyrianSim.load_replay, without building the game or reporting an error.
    #[func]
    fn replay_check(data_dir: GString, path: GString) -> i32 {
        let Ok(rep) = tyrian_sim::replay::load_file(std::path::Path::new(&path.to_string())) else {
            return 1;
        };
        if rep.frames.is_empty() {
            return 2;
        }
        let dir = data_dir.to_string();
        match tyrian_sim::replay::lvl_file_of_section(
            std::path::Path::new(&dir),
            rep.episode,
            rep.level,
            rep.difficulty,
        ) {
            Ok(_) => 0,
            Err(_) => 1,
        }
    }
}

#[cfg(test)]
mod import_c_achievements_tests {
    use super::*;
    use tyrian_formats::save::{GameSave, OtDat as SaveDat};

    /// File in the C format: GAME section whose tail starts with the OTYE block.
    fn dat_with(unlocked: &[usize], kills: u32) -> Vec<u8> {
        let mut save = GameSave::new_empty(&[0; 100]);
        let mut block = AchievementBlock::default();
        for &i in unlocked {
            block.unlocked[i] = true;
        }
        block.stats[0] = kills;
        save.tail = block.write();
        let mut dat = SaveDat::new();
        dat.set_section(b"GAME", save.write(), true);
        dat.write()
    }

    fn unlocked_of(dat: &[u8]) -> Vec<usize> {
        let d = OtDat::parse(dat).unwrap();
        let b = AchievementBlock::at_exact_position(d.section(b"GAME").unwrap()).unwrap();
        (0..ACH_MAX).filter(|&i| b.unlocked[i]).collect()
    }

    #[test]
    fn c_achievements_are_copied_when_godot_has_no_file() {
        let c = dat_with(&[0, 3, 7], 1234);
        let out = merge_c_achievements(&c, None).expect("import");
        assert_eq!(unlocked_of(&out), vec![0, 3, 7]);
        let d = OtDat::parse(&out).unwrap();
        let b = AchievementBlock::at_exact_position(d.section(b"GAME").unwrap()).unwrap();
        assert_eq!(b.stats[0], 1234);
    }

    #[test]
    fn c_achievements_are_added_to_godot_ones_without_losing_any() {
        let c = dat_with(&[0, 3], 10);
        let godot = dat_with(&[3, 9], 99);
        let out = merge_c_achievements(&c, Some(&godot)).expect("import");
        assert_eq!(unlocked_of(&out), vec![0, 3, 9]);
        // Godot counters stay
        let d = OtDat::parse(&out).unwrap();
        let b = AchievementBlock::at_exact_position(d.section(b"GAME").unwrap()).unwrap();
        assert_eq!(b.stats[0], 99);
    }

    #[test]
    fn nothing_is_written_when_c_adds_nothing_or_is_unreadable() {
        let c = dat_with(&[3], 1);
        let godot = dat_with(&[3, 9], 99);
        assert!(merge_c_achievements(&c, Some(&godot)).is_none());
        assert!(merge_c_achievements(b"not a profile", None).is_none());
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;

    fn counters() -> LifetimeCounters {
        AchievementBlock::default().counters()
    }

    fn unlocked_in(bytes: &[u8]) -> [bool; ACH_MAX] {
        use tyrian_formats::save::{GameSave, OtDat};
        let dat = OtDat::parse(bytes).expect("file readable again");
        let save = GameSave::parse(dat.section(b"GAME").unwrap()).expect("GAME readable again");
        AchievementBlock::parse(&save.tail).unwrap().unlocked
    }

    /// opentyrian_dat.c:236-246: a container with a wrong hash is discarded and
    /// rewritten; the unlocked achievement is indeed saved.
    #[test]
    fn corrupted_container_is_discarded_and_rewritten() {
        let mut unlocked = [false; ACH_MAX];
        unlocked[3] = true;
        let (good, discarded) = rebuild_dat(None, unlocked, &counters());
        assert!(!discarded);
        let mut bad = good.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0xFF;
        let mut more = unlocked;
        more[5] = true;
        let (out, discarded) = rebuild_dat(Some(&bad), more, &counters());
        assert!(discarded, "the damaged file must be reported as discarded");
        let read = unlocked_in(&out);
        assert!(read[3] && read[5]);
    }

    #[test]
    fn healthy_file_is_not_discarded() {
        let (good, _) = rebuild_dat(None, [false; ACH_MAX], &counters());
        let (_, discarded) = rebuild_dat(Some(&good), [false; ACH_MAX], &counters());
        assert!(!discarded);
    }
}
