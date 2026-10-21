//! Chaining of the sections of an episode (`levelsN.dat`): the "script"
//! part of `level_load.c:JE_loadMap`, without display.
//!
//! The C code reads the file line by line from the `mainLevel` section
//! (without stopping at the following `*` headers) until it finds a level
//! (`]L[`) or a section jump (`]J`, `]H`, shop `]I[`, episode end
//! `]Q[`...). [`run_script`] reproduces this walk and hands control back to the
//! caller at the points where the C code opens a screen: shop, episode end.
//! The screens themselves (text, animations, shop) are on the Godot side.

use tyrian_formats::crypt::atoi_at;
use tyrian_formats::episode::{EpisodeScript, LevelDirective};

/// `opentyr.h`: `DIFFICULTY_NORMAL`, `DIFFICULTY_HARD`.
const DIFFICULTY_NORMAL: u8 = 2;
const DIFFICULTY_HARD: u8 = 3;
/// `episodes.h:EPISODE_MAX`.
pub const EPISODE_MAX: u8 = 5;

/// C variables read by the conditional directives.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptContext {
    /// `initialDifficulty` (`]h[`, `]H`).
    pub initial_difficulty: u8,
    /// `twoPlayerMode || onePlayerAction` (`]2`).
    pub two_player_or_action: bool,
    /// `player[0].items.ship` (`]w`: Stalker 21.126 = 13).
    pub ship: u8,
    /// `levelTimer && levelTimerCountdown == 0` (`]t`).
    pub level_timer_expired: bool,
    /// `all_players_alive()` (`]l`).
    pub all_players_alive: bool,
    /// `timedBattleMode` (`]T`, `]q`).
    pub timed_battle: bool,
    /// `timeBattleSelection` (`]T`).
    pub time_battle_selection: usize,
    /// `cubeMax` on entry: cubes collected during the level that just
    /// ended (`collision.c`, `cubeMax++`); `]?` caps it, `]+` adds to it.
    pub cube_max: u8,
}

/// What the script asked for in passing (effects to be applied by the caller).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptEffects {
    /// `]s`: `saveLevel = mainLevel`.
    pub save_level: Option<u16>,
    /// `]b`: `JE_saveGame(11 or 22)` (autosave of the last level).
    pub autosave: bool,
    /// `]M`: `play_song(n - 1)`, in order.
    pub songs: Vec<i32>,
    /// `]i`: `songBuy = n - 1`.
    pub song_buy: Option<i32>,
    /// `]G`: planets and sections of the "next level" menu (`mapPlanet`, `mapSection`).
    pub map: Vec<(i32, i32)>,
    /// `]G`: starting planet of the map (`mapOrigin`, 1-based).
    pub map_origin: i32,
    /// `]?`: list of data cubes (`cubeList`, cube numbers of
    /// `cubetxtN.dat`); empty if the walk met none.
    pub cube_list: Vec<i32>,
    /// `cubeMax` after `]?` (cap), `]!` (value) and `]+` (add), starting
    /// from [`ScriptContext::cube_max`]; `None` if none of these
    /// directives was met (the input value stays).
    pub cube_max: Option<u8>,
    /// `]e` (ENGAGE), `]g` (Galaga), `]x` (extraGame) encountered.
    pub engage: bool,
    pub galaga: bool,
    pub extra_game: bool,
    /// Screen directives encountered, in file order (`]A`, `]P`,
    /// `]U`, `]V`, `]R`, `]C`, `]B`, `]F`, `]W`, `]@`, and `]M` so that the
    /// music changes at the same moment as in the C code), for display.
    pub screens: Vec<ScriptScreen>,
    /// `]Q[`: the text blocks that follow the directive (secret
    /// hints, `level_load.c`, `case 'Q'`: `SECRET_HINT_BLOCKS` blocks
    /// terminated by `#`), without the `#` line.
    pub hint_blocks: Vec<Vec<String>>,
}

/// A screen directive and its text (`]W`: the lines up to the `#`,
/// `level_load.c`, `case 'W'`, at most [`WARNING_TEXT_LINES`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptScreen {
    pub directive: String,
    pub text: Vec<String>,
}

/// `fonthand.h:levelWarningText[14]`.
pub const WARNING_TEXT_LINES: usize = 14;
/// `level_load.c:SECRET_HINT_BLOCKS`.
pub const SECRET_HINT_BLOCKS: usize = 9;

/// Stopping point of the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptStop {
    /// `]L[`: level to play. `next_level` is already `mainLevel + 1` if the
    /// column is 0 (`level_load.c`, `case 'L'`).
    Level { section: u16, next_level: u16, directive: LevelDirective },
    /// `]I[`: shop (`JE_itemScreen`). The caller displays the shop
    /// then restarts [`run_script`] on the section chosen in
    /// `effects.map` ("next level" menu, `game_menu.c`,
    /// `MENU_PLAY_NEXT_LEVEL`: `mainLevel = mapSection[choice]`).
    Shop { section: u16, items_available: Vec<Vec<i32>> },
    /// `]Q[`: episode end (ending text, `JE_nextEpisode`); the game
    /// resumes at section 1 of `next_episode`.
    EpisodeEnd { section: u16, next_episode: u8, jump_back_to_episode1: bool },
    /// `]q` in Timed Battle: back to the title screen.
    TitleScreen,
    /// End of file without a level (the C code would stop on `fread_die`).
    EndOfFile,
}

/// `episodes.c:JE_findNextEpisode`: next available episode, back to 1
/// after the last one (`jumpBackToEpisode1`).
pub fn find_next_episode(episode: u8, available: [bool; EPISODE_MAX as usize]) -> (u8, bool) {
    let mut e = episode;
    let mut back = false;
    loop {
        e += 1;
        if e > EPISODE_MAX {
            e = 1;
            back = true;
        }
        if available[(e - 1) as usize] || e == episode {
            return (e, back);
        }
    }
}

/// Walks the script from section `main_level` (1-based) to the
/// next stopping point. `available` is used for `]Q[` (`episodeAvail`).
pub fn run_script(
    script: &EpisodeScript,
    episode: u8,
    main_level: u16,
    ctx: &ScriptContext,
    available: [bool; EPISODE_MAX as usize],
) -> (ScriptStop, ScriptEffects) {
    let lines = script.plain_lines();
    let mut fx = ScriptEffects::default();
    let mut main_level = main_level;
    // Outer loop: each section jump re-reads the file from the start.
    'outer: for _ in 0..1000 {
        // "Seek Section # Mainlevel": consume `main_level` `*` lines.
        let mut i = 0usize;
        let mut x = 0u16;
        while x < main_level {
            let Some(s) = lines.get(i) else {
                return (ScriptStop::EndOfFile, fx);
            };
            if s.first() == Some(&b'*') {
                x += 1;
            }
            i += 1;
        }
        loop {
            let Some(s) = lines.get(i) else {
                return (ScriptStop::EndOfFile, fx);
            };
            i += 1;
            if s.len() < 2 || s[0] != b']' {
                continue;
            }
            let jump = |to: i32| to.clamp(0, u16::MAX as i32) as u16;
            match s[1] {
                b'G' => {
                    let n = atoi_at(s, 7).clamp(1, 5);
                    fx.map_origin = atoi_at(s, 4);
                    fx.map =
                        (0..n as usize).map(|k| (atoi_at(s, 1 + (k + 1) * 8), atoi_at(s, 4 + (k + 1) * 8))).collect();
                }
                b'?' => {
                    let n = atoi_at(s, 4).clamp(0, 4);
                    fx.cube_list = (0..n as usize).map(|k| atoi_at(s, 3 + (k + 1) * 4)).collect();
                    // level_load.c: "if (cubeMax > temp) cubeMax = temp".
                    let m = fx.cube_max.unwrap_or(ctx.cube_max);
                    if m > n as u8 {
                        fx.cube_max = Some(n as u8);
                    }
                }
                b'!' => fx.cube_max = Some(atoi_at(s, 4).clamp(0, 4) as u8),
                b'+' => {
                    fx.cube_max = Some((fx.cube_max.unwrap_or(ctx.cube_max) as i32 + atoi_at(s, 4)).clamp(0, 4) as u8)
                }
                b'g' => fx.galaga = true,
                b'x' => fx.extra_game = true,
                b'e' => fx.engage = true,
                b'J' => {
                    main_level = jump(atoi_at(s, 3));
                    continue 'outer;
                }
                b'2' if ctx.two_player_or_action => {
                    main_level = jump(atoi_at(s, 3));
                    continue 'outer;
                }
                b'w' if ctx.ship == 13 => {
                    main_level = jump(atoi_at(s, 3));
                    continue 'outer;
                }
                b't' if ctx.level_timer_expired => {
                    main_level = jump(atoi_at(s, 3));
                    continue 'outer;
                }
                b'l' if !ctx.all_players_alive => {
                    main_level = jump(atoi_at(s, 3));
                    continue 'outer;
                }
                b's' => fx.save_level = Some(main_level),
                b'b' => fx.autosave = true,
                b'i' => fx.song_buy = Some(atoi_at(s, 3) - 1),
                b'M' => {
                    fx.songs.push(atoi_at(s, 3) - 1);
                    fx.screens
                        .push(ScriptScreen { directive: String::from_utf8_lossy(s).into_owned(), text: Vec::new() });
                }
                b'I' => {
                    // The next 9 lines: available items (column 8+).
                    let mut avail = Vec::new();
                    for _ in 0..9 {
                        let Some(c) = lines.get(i) else { break };
                        i += 1;
                        let items = if c.len() > 8 { tyrian_formats::crypt::pop_ints(&c[8..]) } else { Vec::new() };
                        avail.push(items.into_iter().take(10).collect());
                    }
                    return (ScriptStop::Shop { section: main_level, items_available: avail }, fx);
                }
                b'L' => {
                    let mut directive = LevelDirective::parse(i - 1, s);
                    if directive.next_level == 0 {
                        directive.next_level = main_level as i32 + 1;
                    }
                    let next_level = jump(directive.next_level);
                    return (ScriptStop::Level { section: main_level, next_level, directive }, fx);
                }
                b'Q' => {
                    // Hint blocks: each ends with a `#` line.
                    for _ in 0..SECRET_HINT_BLOCKS {
                        let mut block = Vec::new();
                        while let Some(t) = lines.get(i) {
                            i += 1;
                            if t.first() == Some(&b'#') {
                                break;
                            }
                            block.push(String::from_utf8_lossy(t).into_owned());
                        }
                        fx.hint_blocks.push(block);
                        if i >= lines.len() {
                            break;
                        }
                    }
                    let (next_episode, back) = find_next_episode(episode, available);
                    return (
                        ScriptStop::EpisodeEnd { section: main_level, next_episode, jump_back_to_episode1: back },
                        fx,
                    );
                }
                b'H' if ctx.initial_difficulty < DIFFICULTY_HARD => {
                    main_level = jump(atoi_at(s, 4));
                    continue 'outer;
                }
                b'h' if ctx.initial_difficulty > DIFFICULTY_NORMAL => {
                    // Next line ignored (level reserved for low difficulties).
                    i += 1;
                }
                b'T' if ctx.timed_battle => {
                    main_level = jump(atoi_at(s, ctx.time_battle_selection * 3));
                    continue 'outer;
                }
                b'q' if ctx.timed_battle => return (ScriptStop::TitleScreen, fx),
                b'W' => {
                    // level_load.c, case 'W': lines read up to the `#`.
                    let mut text = Vec::new();
                    while let Some(t) = lines.get(i) {
                        i += 1;
                        if t.first() == Some(&b'#') {
                            break;
                        }
                        if text.len() < WARNING_TEXT_LINES {
                            text.push(String::from_utf8_lossy(t).into_owned());
                        }
                    }
                    fx.screens.push(ScriptScreen { directive: String::from_utf8_lossy(s).into_owned(), text });
                }
                b'A' | b'P' | b'U' | b'V' | b'R' | b'C' | b'B' | b'F' | b'@' | b'n' => {
                    fx.screens
                        .push(ScriptScreen { directive: String::from_utf8_lossy(s).into_owned(), text: Vec::new() });
                }
                _ => {}
            }
        }
    }
    (ScriptStop::EndOfFile, fx)
}

/// Equipment imposed by `]e[` (ENGAGE, mini-games: `level_load.c:JE_loadMap`,
/// `case 'e'`): Stalker 21.126, Atomic RailGun power 3 (hence 3 lives:
/// `*player[0].lives` is the front weapon power), no rear weapon or
/// sidekick, Advanced MR-12, Advanced Integrity Field, money reset to zero.
pub fn engage_loadout() -> crate::Loadout {
    use crate::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};
    crate::Loadout {
        ship: ShipId(13),
        front_weapon: WeaponId(39),
        front_power: 3,
        rear_weapon: WeaponId(0),
        rear_power: 1,
        shield: ShieldId(4),
        generator: GeneratorId(2),
        left_sidekick: SidekickId(0),
        right_sidekick: SidekickId(0),
        special: SpecialId(0),
        cash: 0,
    }
}

/// Game mode of a level preceded by `]e[` (and possibly `]g[`):
/// `doNotSaveBackup`, `onePlayerAction`, `superTyrian`; `]g[` adds
/// `galagaMode` and gives player 2 (dragonwing) a copy of the equipment
/// with a Vulcan Cannon (15) at the rear and no sidekick. The campaign
/// mini-games are `** ALE **` (episode 1, section 41) and `SQUADRON`
/// (episode 4, section 45), both `]e[` + `]g[`, and `TIME WAR` (episode
/// 4, section 40, `]e[` alone). `None` outside ENGAGE.
pub fn mini_game_modes(fx: &ScriptEffects, initial_difficulty: u8) -> Option<crate::modes::ModeSetup> {
    if !fx.engage {
        return None;
    }
    let loadout2 = fx.galaga.then(|| crate::Loadout {
        rear_weapon: crate::WeaponId(15),
        left_sidekick: crate::SidekickId(0),
        right_sidekick: crate::SidekickId(0),
        ..engage_loadout()
    });
    Some(crate::modes::ModeSetup {
        galaga: fx.galaga,
        one_player_action: true,
        super_tyrian: true,
        // config.c: inputDevice = { 1 keyboard, 2 mouse } by default.
        devices: (1, 2),
        initial_difficulty,
        loadout2,
        ..Default::default()
    })
}

/// Continuation of the campaign after an ENGAGE level (`tyrian2.c:JE_main`, level
/// exit, `doNotSaveBackup` set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniGameExit {
    /// All players dead (the only possible ending of `** ALE **` and of
    /// `SQUADRON`, which have no level-end event): no
    /// debriefing, `JE_loadGame(11)` restores the equipment, money and section
    /// saved by `]b[` (the shop that offered the mini-game), and the
    /// `superTyrian` / `onePlayerAction` flags fall back.
    ReturnToBackup,
    /// Level finished alive (`TIME WAR`): debriefing then `mainLevel =
    /// nextLevel`. The Enhanced C version also reloads the save so as not to
    /// keep the arcade Stalker (see docs/divergences.md).
    NextLevelWithBackupItems,
}

/// Exit rule of an ENGAGE level; `player_end_level` = abandon through the
/// game menu (`playerEndLevel`), which also returns to the save.
pub fn mini_game_exit(all_players_dead: bool, player_end_level: bool) -> MiniGameExit {
    if !all_players_dead && !player_end_level {
        MiniGameExit::NextLevelWithBackupItems
    } else {
        MiniGameExit::ReturnToBackup
    }
}

/// `mainint.c:JE_totalScore`: money plus equipment value
/// (`JE_getValue`, weapon power included).
pub fn total_score(t: &crate::tables::Tables, items: &crate::world::Items, cash: u64) -> u64 {
    use crate::shop::{item_value, ShopCategory as C};
    let v = |c, id: u8, p: u8| item_value(t, c, id as u16, p).max(0) as u64;
    cash + v(C::Ship, items.ship.0, 0)
        + v(C::FrontWeapon, items.weapon[0].id.0, items.weapon[0].power)
        + v(C::RearWeapon, items.weapon[1].id.0, items.weapon[1].power)
        + v(C::Shield, items.shield.0, 0)
        + v(C::Generator, items.generator.0, 0)
        + v(C::LeftSidekick, items.sidekick[0].0, 0)
        + v(C::RightSidekick, items.sidekick[1].0, 0)
}

/// `mainint.c:adjust_difficulty` (one player): difficulty rises with the
/// score, never goes down. `score * score_multiplier[initialDifficulty]` is
/// a `float` product rounded by `roundf`, reproduced by [`crate::fmath`].
pub fn adjust_difficulty(difficulty: u8, initial_difficulty: u8, score: u64) -> u8 {
    adjust_difficulty_mode(difficulty, initial_difficulty, score, false)
}

/// `mainint.c:adjust_difficulty` with the one/two player choice: with two, `score` is the sum
/// of the two purses (`player[0].cash + player[1].cash`) and the thresholds are lower
/// (10,000 ... 600,000 instead of 40,000 ... 3,000,000).
pub fn adjust_difficulty_mode(difficulty: u8, initial_difficulty: u8, score: u64, two_player: bool) -> u8 {
    use crate::fmath::round_rational;
    // { 0, 0.4f, 0.8f, 1.3f, 1.6f, 2, 2, 3, 3, 3 } as exact rationals before float rounding.
    const MULT: [(i128, i128); 10] = [(0, 1), (2, 5), (4, 5), (13, 10), (8, 5), (2, 1), (2, 1), (3, 1), (3, 1), (3, 1)];
    let (n, d) = MULT[(initial_difficulty as usize).clamp(1, 9)];
    let adjusted = round_rational(score as i128, 1).mul(round_rational(n, d)).roundf();
    const STEPS_1P: [(i64, u8); 8] =
        [(40000, 1), (70000, 2), (150000, 3), (300000, 4), (600000, 5), (1000000, 6), (2000000, 7), (3000000, 8)];
    const STEPS_2P: [(i64, u8); 8] =
        [(10000, 1), (20000, 2), (50000, 3), (80000, 4), (125000, 5), (200000, 6), (400000, 7), (600000, 8)];
    let steps = if two_player { &STEPS_2P } else { &STEPS_1P };
    let new = steps.iter().find(|(lim, _)| adjusted < *lim).map_or(9, |(_, d)| *d);
    difficulty.max(new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// ENH-02: two-player thresholds of `adjust_difficulty` (mainint.c:1738-1757).
    #[test]
    fn adjust_difficulty_two_player_thresholds() {
        // Initial difficulty 3 (Hard, ×1.3): 10,000 / 1.3 ≈ 7,693.
        assert_eq!(adjust_difficulty_mode(1, 3, 7000, true), 1);
        assert_eq!(adjust_difficulty_mode(1, 3, 8000, true), 2);
        assert_eq!(adjust_difficulty_mode(1, 3, 8000, false), 1);
        // Never goes down.
        assert_eq!(adjust_difficulty_mode(5, 3, 0, true), 5);
        // 600,000 adjusted and above: Nortaneous (9).
        assert_eq!(adjust_difficulty_mode(1, 3, 500_000, true), 9);
    }

    fn script(ep: u8) -> Option<EpisodeScript> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../../data/levels{ep}.dat"));
        Some(EpisodeScript::parse(&std::fs::read(p).ok()?).unwrap())
    }

    const ALL: [bool; 5] = [true; 5];

    fn ctx() -> ScriptContext {
        ScriptContext { initial_difficulty: 2, all_players_alive: true, ship: 1, ..Default::default() }
    }

    /// Path actually followed by the C code in `rewrite/golden/campaign/e1-chain-*.gm`:
    /// section 2 (shop, planet → 3), TYRIAN (level 9), section 4
    /// (shop, two planets 5 and 6), ASTEROID1 (level 1).
    #[test]
    fn episode1_chain_matches_c_recording() {
        let Some(s) = script(1) else { return };
        let (stop, fx) = run_script(&s, 1, 2, &ctx(), ALL);
        assert!(matches!(stop, ScriptStop::Shop { section: 2, .. }));
        assert_eq!(fx.map, vec![(1, 3)]);
        let (stop, _) = run_script(&s, 1, 3, &ctx(), ALL);
        let ScriptStop::Level { next_level, directive, .. } = stop else { panic!("{stop:?}") };
        assert_eq!((directive.level_name.trim(), directive.lvl_file_num, next_level), ("TYRIAN", 9, 4));
        // Initial difficulty > NORMAL: ]h[ skips the first ]L[ (lvl 9 → 15).
        let hard = ScriptContext { initial_difficulty: 3, ..ctx() };
        let (stop, _) = run_script(&s, 1, 3, &hard, ALL);
        let ScriptStop::Level { directive, .. } = stop else { panic!() };
        assert_eq!(directive.lvl_file_num, 15);
        let (stop, fx) = run_script(&s, 1, 4, &ctx(), ALL);
        assert!(matches!(stop, ScriptStop::Shop { section: 4, .. }));
        assert_eq!(fx.map, vec![(2, 5), (3, 6)]);
        assert_eq!(fx.map_origin, 1);
        let (stop, _) = run_script(&s, 1, 5, &ctx(), ALL);
        let ScriptStop::Level { directive, .. } = stop else { panic!() };
        assert_eq!((directive.level_name.trim(), directive.lvl_file_num), ("ASTEROID1", 1));
    }

    /// `e2-e3-chain-*.gm`: GRYPHON (episode 2, section 12) then episode
    /// end, episode 3 section 1 (shop → 2), GAUNTLET (level 3).
    #[test]
    fn episode2_to_3_transition_matches_c_recording() {
        let (Some(s2), Some(s3)) = (script(2), script(3)) else {
            return;
        };
        let (stop, _) = run_script(&s2, 2, 12, &ctx(), ALL);
        let ScriptStop::Level { next_level, directive, .. } = stop else { panic!() };
        assert_eq!((directive.level_name.trim(), directive.lvl_file_num, next_level), ("GRYPHON", 6, 13));
        let (stop, fx) = run_script(&s2, 2, 13, &ctx(), ALL);
        assert_eq!(stop, ScriptStop::EpisodeEnd { section: 13, next_episode: 3, jump_back_to_episode1: false });
        assert_eq!(fx.songs, vec![7]);
        let (stop, fx) = run_script(&s3, 3, 1, &ctx(), ALL);
        assert!(matches!(stop, ScriptStop::Shop { section: 1, .. }));
        assert_eq!(fx.map, vec![(20, 2)]);
        let (stop, _) = run_script(&s3, 3, 2, &ctx(), ALL);
        let ScriptStop::Level { directive, .. } = stop else { panic!() };
        assert_eq!((directive.level_name.trim(), directive.lvl_file_num), ("GAUNTLET", 3));
    }

    /// "Do you have the special ship?" shops: `]s[ ]b[ ]x[` then the
    /// map offers the mini-game (planet 5) or "Skip It" (planet 21); the
    /// mini-game is `]e[ ]g[` then `]L[` (`levels1.dat` section 41,
    /// `levels4.dat` section 45).
    #[test]
    fn mini_games_reached_from_their_shop() {
        for (ep, shop, mini, other, name) in [(1u8, 40u16, 41u16, 26u16, "** ALE **"), (4, 44, 45, 42, "SQUADRON")] {
            let Some(s) = script(ep) else { return };
            let (stop, fx) = run_script(&s, ep, shop, &ctx(), ALL);
            assert!(matches!(stop, ScriptStop::Shop { section, .. } if section == shop), "{stop:?}");
            assert!(fx.extra_game && fx.autosave && fx.save_level == Some(shop), "{fx:?}");
            assert_eq!(fx.map, vec![(5, mini as i32), (21, other as i32)]);
            assert!(mini_game_modes(&fx, 2).is_none(), "the shop is not a mini-game");
            let (stop, fx) = run_script(&s, ep, mini, &ctx(), ALL);
            let ScriptStop::Level { directive, next_level, .. } = stop else { panic!("{stop:?}") };
            assert_eq!(directive.level_name.trim(), name);
            assert_eq!(directive.lvl_file_num, 18);
            assert!(fx.engage && fx.galaga);
            // No "$": neither bonus level nor normal bonus, death does not lead to nextLevel.
            assert!(!directive.bonus && !directive.normal_bonus);
            let m = mini_game_modes(&fx, 2).unwrap();
            assert!(m.galaga && m.one_player_action && m.super_tyrian && !m.two_player);
            let l2 = m.loadout2.unwrap();
            assert_eq!((l2.ship.0, l2.front_weapon.0, l2.rear_weapon.0), (13, 39, 15));
            let _ = next_level;
        }
        // TIME WAR: ENGAGE without Galaga.
        let Some(s4) = script(4) else { return };
        let (_, fx) = run_script(&s4, 4, 40, &ctx(), ALL);
        let m = mini_game_modes(&fx, 2).unwrap();
        assert!(!m.galaga && m.loadout2.is_none());
    }

    #[test]
    fn mini_game_exit_rule() {
        assert_eq!(mini_game_exit(true, false), MiniGameExit::ReturnToBackup);
        assert_eq!(mini_game_exit(false, true), MiniGameExit::ReturnToBackup);
        assert_eq!(mini_game_exit(false, false), MiniGameExit::NextLevelWithBackupItems);
        let l = engage_loadout();
        assert_eq!(
            (l.ship.0, l.front_weapon.0, l.front_power, l.generator.0, l.shield.0, l.cash),
            (13, 39, 3, 2, 4, 0)
        );
    }

    /// Data cubes of the first two shops of episode 1: section
    /// 2 imposes four (`]![ 04`), section 4 keeps those of the
    /// level played, capped to its `]?` list.
    #[test]
    fn cube_directives_follow_cube_max() {
        let Some(s) = script(1) else { return };
        let (_, fx) = run_script(&s, 1, 2, &ctx(), ALL);
        assert_eq!((fx.cube_list.as_slice(), fx.cube_max), (&[1, 2, 23, 5][..], Some(4)));
        let collected = |n: u8| run_script(&s, 1, 4, &ScriptContext { cube_max: n, ..ctx() }, ALL).1;
        let fx = collected(2);
        assert_eq!((fx.cube_list.as_slice(), fx.cube_max.unwrap_or(2)), (&[6, 7, 12, 18][..], 2));
        assert_eq!(collected(0).cube_max.unwrap_or(0), 0);
        assert_eq!(collected(9).cube_max, Some(4), "cap from the ]? list");
    }

    /// End of episode 1 (section 26, `]F[ ]M 031[ ]P 005[ ]Q[`): the
    /// screens arrive in file order, then the 9 hint blocks
    /// ("Secret Hint 1-1-1:" ... "1-3-3:"), each without its `#` line.
    #[test]
    fn episode_end_screens_and_secret_hints() {
        let Some(s) = script(1) else { return };
        let (stop, fx) = run_script(&s, 1, 26, &ctx(), ALL);
        assert_eq!(stop, ScriptStop::EpisodeEnd { section: 26, next_episode: 2, jump_back_to_episode1: false });
        let dirs: Vec<&str> = fx.screens.iter().map(|e| e.directive.get(..2).unwrap_or("")).collect();
        assert_eq!(dirs, vec!["]F", "]M", "]P"]);
        assert_eq!(fx.hint_blocks.len(), SECRET_HINT_BLOCKS);
        assert_eq!(fx.hint_blocks[0][0].trim(), "Secret Hint 1-1-1:");
        assert_eq!(fx.hint_blocks[1][0].trim(), "Secret Hint 1-1-2:");
        assert_eq!(fx.hint_blocks[8][0].trim(), "Secret Hint 1-3-3:");
        assert!(fx.hint_blocks.iter().flatten().all(|l| !l.starts_with('#')));
        // Episode 4, section 42: a `]Wn 03[` page before `]Q[`.
        let Some(s4) = script(4) else { return };
        let (stop, fx) = run_script(&s4, 4, 42, &ctx(), ALL);
        assert!(matches!(stop, ScriptStop::EpisodeEnd { next_episode: 5, .. }), "{stop:?}");
        let pages: Vec<&ScriptScreen> = fx.screens.iter().filter(|e| e.directive.starts_with("]W")).collect();
        assert_eq!(pages[0].text[0], "The madness has stopped for now.");
        assert!(pages.iter().all(|p| !p.text.is_empty() && p.text.len() <= WARNING_TEXT_LINES));
        assert_eq!(fx.hint_blocks.len(), SECRET_HINT_BLOCKS);
    }

    /// After the BONUS level (section 25), the C code re-reads section 26 which
    /// chains to... section 11 ("Deliani from Savara") which displays
    /// the minefield `]Wy 01[` "Warning:" warning.
    #[test]
    fn warning_text_before_deliani() {
        let Some(s) = script(1) else { return };
        let (stop, fx) = run_script(&s, 1, 11, &ctx(), ALL);
        assert!(matches!(stop, ScriptStop::Shop { section: 12, .. }), "{stop:?}");
        let w: Vec<&ScriptScreen> = fx.screens.iter().filter(|e| e.directive.starts_with("]W")).collect();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].directive.get(..3), Some("]Wy"));
        assert_eq!(w[0].text[0].trim(), "Warning:");
        assert!(w[0].text.iter().any(|l| l.contains("minefield")));
    }

    #[test]
    fn next_episode_wraps_to_one() {
        assert_eq!(find_next_episode(5, ALL), (1, true));
        assert_eq!(find_next_episode(3, [true, true, true, false, false]), (1, true));
        assert_eq!(find_next_episode(1, [true, false, true, false, false]), (3, false));
    }
}
