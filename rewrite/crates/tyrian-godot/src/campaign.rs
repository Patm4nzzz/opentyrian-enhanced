//! Godot bridge for campaign sequencing (`tyrian_sim::campaign`):
//! episode script `levelsN.dat` (`level_load.c:JE_loadMap`) and
//! end-of-level difficulty (`mainint.c:adjust_difficulty`).
//!
//! `run_script` returns a dictionary:
//! - `stop`: "level", "shop", "episode_end", "title" or "eof";
//! - `section`, and depending on the case `next_level`, `level_name`, `lvl_file_num`,
//!   `level_song`, `bonus`, `normal_bonus` (level), `items_available`
//!   (shop: 9 arrays), `next_episode`, `jump_back_to_episode1`;
//! - effects: `map` (flattened planet, section pairs), `songs`, `song_buy`
//!   (-1 if absent), `save_level` (-1), `autosave`, `cube_max` (-1),
//!   `cube_list`, `screens` (dictionaries `directive`, `text`: story
//!   screens `]P` `]W` `]C` `]F` `]A`... in file order),
//!   `hint_blocks` (`]Q[`: secret hint blocks), `extra_game` (`]x[`:
//!   the shop offers a mini-game, "Bonus Game" on the map);
//! - `mini_game` (level preceded by `]e[` ENGAGE, possibly `]g[`
//!   GALAGA: `** ALE **`, `SQUADRON`, `TIME WAR`): dictionary `modes`
//!   (10 values of `TyrianSim.set_modes`), `loadout` (11 values of
//!   `set_loadout`, imposed equipment), `loadout2` (10 values, player 2 of
//!   Galaga, empty otherwise), `initial_difficulty`; absent outside ENGAGE.

use std::path::{Path, PathBuf};

use godot::prelude::*;
use tyrian_formats::campaign_save::{self, ItemNames};
use tyrian_formats::episode::EpisodeScript;
use tyrian_sim::campaign::{
    adjust_difficulty, engage_loadout, mini_game_modes, run_script, ScriptContext, ScriptStop, EPISODE_MAX,
};
use tyrian_sim::Loadout;

#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct TyrianCampaign {
    scripts: [Option<EpisodeScript>; EPISODE_MAX as usize],
    data_dir: String,
    /// Item names of each episode (native save), read once.
    item_names: Vec<Option<ItemNames>>,
    /// Mods attached by `set_mods`: their items are written by name in the saves.
    mods: Option<Gd<crate::TyrianMods>>,
    #[var]
    last_error: GString,
}

#[godot_api]
impl TyrianCampaign {
    /// Mods whose items are known to the saves (null = none).
    #[func]
    fn set_mods(&mut self, mods: Option<Gd<crate::TyrianMods>>) {
        self.mods = mods;
    }

    /// Loads `levels1.dat` ... `levels5.dat` present in `data_dir`
    /// (`episodes.c:JE_initEpisode`, `episodeAvail`). Returns the number loaded.
    #[func]
    fn load(&mut self, data_dir: GString) -> i32 {
        let dir = data_dir.to_string();
        self.data_dir = dir.clone();
        self.item_names.clear();
        let mut n = 0;
        for ep in 1..=EPISODE_MAX {
            let p = Path::new(&dir).join(format!("levels{ep}.dat"));
            self.scripts[(ep - 1) as usize] = std::fs::read(&p).ok().and_then(|b| match EpisodeScript::parse(&b) {
                Ok(s) => Some(s),
                Err(e) => {
                    self.last_error = GString::from(format!("{}: {e}", p.display()).as_str());
                    None
                }
            });
            n += self.scripts[(ep - 1) as usize].is_some() as i32;
        }
        n
    }

    /// `episodeAvail[episode - 1]`.
    #[func]
    fn episode_available(&self, episode: i32) -> bool {
        (1..=EPISODE_MAX as i32).contains(&episode) && self.scripts[(episode - 1) as usize].is_some()
    }

    /// Walks the script of `episode` from section `main_level`.
    #[func]
    ///
    /// `two_player_or_action`: `twoPlayerMode || onePlayerAction` (`]2`);
    /// `time_battle_selection`: `timeBattleSelection` in Timed Battle, 0 otherwise.
    ///
    /// `cube_max`: data cubes collected in the previous level (`cubeMax`
    /// of the C before the `]?` `]!` `]+` directives); the `cube_max` result
    /// is the value after those directives (-1: none encountered).
    #[allow(clippy::too_many_arguments)]
    fn run_script(
        &self,
        episode: i32,
        main_level: i32,
        initial_difficulty: i32,
        ship: i32,
        all_players_alive: bool,
        #[opt(default = 0)] cube_max: i32,
        #[opt(default = false)] two_player_or_action: bool,
        #[opt(default = 0)] time_battle_selection: i32,
    ) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(script) = (1..=EPISODE_MAX as i32)
            .contains(&episode)
            .then(|| self.scripts[(episode - 1) as usize].as_ref())
            .flatten()
        else {
            d.set("stop", "eof");
            return d;
        };
        let available = std::array::from_fn(|i| self.scripts[i].is_some());
        let ctx = ScriptContext {
            initial_difficulty: initial_difficulty.clamp(0, 10) as u8,
            ship: ship.clamp(0, 255) as u8,
            all_players_alive,
            cube_max: cube_max.clamp(0, 4) as u8,
            // "New Game" menu modes (menus.c:gameplaySelect): `]2` jumps
            // to two players or arcade, `]T` / `]q` to Timed Battle
            // (timeBattleSelection 1..3, 0 = outside Timed Battle).
            two_player_or_action,
            timed_battle: time_battle_selection > 0,
            time_battle_selection: time_battle_selection.clamp(0, 5) as usize,
            ..Default::default()
        };
        let (stop, fx) =
            run_script(script, episode as u8, main_level.clamp(0, u16::MAX as i32) as u16, &ctx, available);
        match stop {
            ScriptStop::Level { section, next_level, directive } => {
                d.set("stop", "level");
                d.set("section", section as i32);
                d.set("next_level", next_level as i32);
                d.set("level_name", directive.level_name.trim_end());
                d.set("lvl_file_num", directive.lvl_file_num);
                d.set("level_song", directive.level_song);
                d.set("bonus", directive.bonus);
                d.set("normal_bonus", directive.normal_bonus);
                if let Some(m) = mini_game_modes(&fx, ctx.initial_difficulty) {
                    let flags = [
                        m.two_player as i32,
                        m.galaga as i32,
                        m.one_player_action as i32,
                        m.super_arcade as i32,
                        m.super_tyrian as i32,
                        m.timed as i32,
                        m.mutators as i32,
                        m.anomalies as i32,
                        // One player: "all devices" (0), for lack of a setting in the interface
                        // (docs/divergences.md); the C sets 1 (keyboard) by default.
                        if m.two_player { m.devices.0 as i32 } else { 0 },
                        if m.two_player { m.devices.1 as i32 } else { 0 },
                    ];
                    let mut lo = loadout_values(&engage_loadout()).to_vec();
                    lo.push(engage_loadout().cash as i32);
                    let lo2 = m.loadout2.map(|l| loadout_values(&l).to_vec()).unwrap_or_default();
                    let mut mg = VarDictionary::new();
                    mg.set("modes", &PackedInt32Array::from(&flags[..]));
                    mg.set("loadout", &PackedInt32Array::from(lo.as_slice()));
                    mg.set("loadout2", &PackedInt32Array::from(lo2.as_slice()));
                    mg.set("initial_difficulty", m.initial_difficulty as i32);
                    d.set("mini_game", &mg);
                }
            }
            ScriptStop::Shop { section, items_available } => {
                d.set("stop", "shop");
                d.set("section", section as i32);
                let mut arr = VarArray::new();
                for row in items_available {
                    arr.push(&PackedInt32Array::from(row.as_slice()).to_variant());
                }
                d.set("items_available", &arr);
            }
            ScriptStop::EpisodeEnd { section, next_episode, jump_back_to_episode1 } => {
                d.set("stop", "episode_end");
                d.set("section", section as i32);
                d.set("next_episode", next_episode as i32);
                d.set("jump_back_to_episode1", jump_back_to_episode1);
            }
            ScriptStop::TitleScreen => d.set("stop", "title"),
            ScriptStop::EndOfFile => d.set("stop", "eof"),
        }
        let map: Vec<i32> = fx.map.iter().flat_map(|&(p, s)| [p, s]).collect();
        d.set("map", &PackedInt32Array::from(map.as_slice()));
        d.set("map_origin", fx.map_origin);
        d.set("songs", &PackedInt32Array::from(fx.songs.as_slice()));
        d.set("song_buy", fx.song_buy.unwrap_or(-1));
        d.set("save_level", fx.save_level.map_or(-1, |v| v as i32));
        d.set("autosave", fx.autosave);
        d.set("cube_max", fx.cube_max.map_or(-1, |v| v as i32));
        d.set("cube_list", &PackedInt32Array::from(fx.cube_list.as_slice()));
        d.set("extra_game", fx.extra_game);
        // Script screens in file order: raw directive (`]P 005[`)
        // and text of `]W` (lines up to the `#`).
        let mut screens = VarArray::new();
        for s in &fx.screens {
            let mut e = VarDictionary::new();
            e.set("directive", s.directive.as_str());
            let mut text = PackedStringArray::new();
            for t in &s.text {
                text.push(t.as_str());
            }
            e.set("text", &text);
            screens.push(&e.to_variant());
        }
        d.set("screens", &screens);
        // `]Q[`: secret hint blocks (1..=9).
        let mut hints = VarArray::new();
        for b in &fx.hint_blocks {
            let mut text = PackedStringArray::new();
            for t in b {
                text.push(t.as_str());
            }
            hints.push(&text.to_variant());
        }
        d.set("hint_blocks", &hints);
        d
    }

    /// Slots of the native save (ADR 0129) in `dir`
    /// (`user://saves` globalized): array of 11 values, `null` for an
    /// empty slot, otherwise a dictionary `slot`, `name`, `saved_at`,
    /// `episode`, `section`, `last_level`, `difficulty`,
    /// `initial_difficulty`, `cubes`, `loadout` (11 values, order of
    /// `TyrianSim.set_loadout`) and `error` (empty if the slot is
    /// readable; otherwise the message, and the slot must not be loaded).
    #[func]
    fn load_campaign_slots(&mut self, dir: GString) -> VarArray {
        let dir = PathBuf::from(dir.to_string());
        self.ensure_item_names();
        let names: Vec<Option<ItemNames>> = (1..=EPISODE_MAX).map(|ep| self.names_for(ep)).collect();
        let lookup = |ep: u8| names.get(ep.wrapping_sub(1) as usize).cloned().flatten();
        let mut out = VarArray::new();
        for slot in 1..=campaign_save::SLOT_COUNT {
            let Some(res) = campaign_save::load(&dir, slot, &lookup) else {
                out.push(&Variant::nil());
                continue;
            };
            let mut d = VarDictionary::new();
            d.set("slot", slot as i32);
            match res {
                Ok(s) => {
                    d.set("name", s.name.as_str());
                    d.set("saved_at", s.saved_at.as_str());
                    d.set("episode", s.episode as i32);
                    d.set("section", s.section as i32);
                    d.set("last_level", s.last_level.as_str());
                    d.set("difficulty", s.difficulty as i32);
                    d.set("initial_difficulty", s.initial_difficulty as i32);
                    d.set("cubes", s.data_cubes as i32);
                    let lo: Vec<i32> = s.loadout.iter().map(|&v| v.min(i32::MAX as u64) as i32).collect();
                    d.set("loadout", &PackedInt32Array::from(lo.as_slice()));
                    // config.c:JE_loadGame: lastItems (player 2) and score2.
                    let lo2: Vec<i32> = s.loadout2.iter().map(|&v| v.min(i32::MAX as u64) as i32).collect();
                    d.set("loadout2", &PackedInt32Array::from(lo2.as_slice()));
                    d.set(
                        "input_devices",
                        &PackedInt32Array::from(&[s.input_devices[0] as i32, s.input_devices[1] as i32][..]),
                    );
                    d.set("players", if s.loadout2 != [0u64; 11] { 2i32 } else { 1i32 });
                    // config.c:JE_loadGame : initial_episode_num, gameHasRepeated.
                    d.set(
                        "initial_episode",
                        if s.initial_episode > 0 { s.initial_episode as i32 } else { s.episode as i32 },
                    );
                    d.set("game_has_repeated", s.game_has_repeated);
                    d.set("secret_hint", s.secret_hint as i32);
                    d.set("super_arcade_mode", s.super_arcade_mode as i32);
                    d.set("error", "");
                }
                Err(e) => d.set("error", e.to_string().as_str()),
            }
            out.push(&d.to_variant());
        }
        out
    }

    /// Writes slot `state.slot` (1..=11) of the native save in
    /// `dir`, atomically. `state`: `slot`, `name`, `saved_at`,
    /// `episode`, `section`, `last_level`, `difficulty`,
    /// `initial_difficulty`, `cubes`, `loadout` (11 values). False on
    /// failure (`last_error`).
    #[func]
    fn save_campaign_slot(&mut self, dir: GString, state: VarDictionary) -> bool {
        let get = |k: &str| state.get(k).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0);
        let gets = |k: &str| state.get(k).map(|v| v.to_string()).unwrap_or_default();
        let lo: Vec<i32> = state
            .get("loadout")
            .and_then(|v| v.try_to::<PackedInt32Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default();
        // config.c:JE_saveGame : lastItems (j2), score2, input1/input2.
        let lo2: Vec<i32> = state
            .get("loadout2")
            .and_then(|v| v.try_to::<PackedInt32Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default();
        let inp: Vec<i32> = state
            .get("input_devices")
            .and_then(|v| v.try_to::<PackedInt32Array>().ok())
            .map(|a| a.to_vec())
            .unwrap_or_default();
        let res = (|| -> Result<(), String> {
            if lo.len() < 11 || lo.iter().any(|&v| v < 0) {
                return Err("loadout: 11 non-negative values expected".into());
            }
            let s = campaign_save::CampaignState {
                slot: get("slot").clamp(0, 255) as u8,
                name: gets("name"),
                saved_at: gets("saved_at"),
                episode: get("episode").clamp(0, 255) as u8,
                section: get("section").clamp(0, u16::MAX as i64) as u16,
                last_level: gets("last_level"),
                difficulty: get("difficulty").clamp(0, 255) as u8,
                initial_difficulty: get("initial_difficulty").clamp(0, 255) as u8,
                data_cubes: get("cubes").clamp(0, 255) as u8,
                loadout: std::array::from_fn(|i| lo[i] as u64),
                loadout2: if lo2.len() >= 11 { std::array::from_fn(|i| lo2[i].max(0) as u64) } else { [0u64; 11] },
                input_devices: if inp.len() >= 2 {
                    [inp[0].clamp(0, 255) as u8, inp[1].clamp(0, 255) as u8]
                } else {
                    [0u8; 2]
                },
                initial_episode: get("initial_episode").clamp(0, 255) as u8,
                game_has_repeated: state
                    .get("game_has_repeated")
                    .and_then(|v| v.try_to::<bool>().ok())
                    .unwrap_or(false),
                secret_hint: get("secret_hint").clamp(0, 3) as u8,
                super_arcade_mode: get("super_arcade_mode").clamp(0, 255) as u8,
            };
            self.ensure_item_names();
            let names =
                self.names_for(s.episode).ok_or_else(|| format!("item tables of episode {} not found", s.episode))?;
            campaign_save::save(Path::new(&dir.to_string()), &s, &names).map(|_| ()).map_err(|e| e.to_string())
        })();
        match res {
            Ok(()) => true,
            Err(e) => {
                self.last_error = GString::from(e.as_str());
                false
            }
        }
    }

    /// One-time import of a save from the C version (`opentyrian.dat`,
    /// GAME section, or old `opentyrian.sav`): each used one-player
    /// slot becomes a native slot, unless the latter already
    /// exists. Decoding of `config.c:JE_loadGame` ("Completed" rule ->
    /// next episode). Returns the number of slots imported, -1 if the
    /// C file is unreadable (`last_error`).
    #[func]
    fn import_c_save(&mut self, c_path: GString, dir: GString) -> i32 {
        let p = c_path.to_string();
        let save = match std::fs::read(&p)
            .map_err(|e| e.to_string())
            .and_then(|b| tyrian_formats::save::load_any(&b).map_err(|e| e.to_string()))
        {
            Ok(s) => s,
            Err(e) => {
                self.last_error = GString::from(format!("{p}: {e}").as_str());
                return -1;
            }
        };
        self.ensure_item_names();
        let dir = PathBuf::from(dir.to_string());
        let mut n = 0;
        for (i, s) in save.slots.iter().enumerate().take(campaign_save::SLOT_COUNT as usize) {
            let slot = (i + 1) as u8;
            if !s.is_used() || campaign_save::slot_path(&dir, slot).exists() {
                continue;
            }
            let it = &s.items;
            let level_name = s.level_name().trim_end().to_string();
            let mut episode = s.episode;
            if level_name == "Completed" {
                episode = if episode >= EPISODE_MAX { 1 } else { episode + 1 };
            }
            // pitems: 0 front, 1 rear, 3/4 sidekicks, 5 generator, 9 shield, 10 special, 11 ship.
            let state = campaign_save::CampaignState {
                slot,
                name: s.name().trim_end().to_string(),
                saved_at: String::new(),
                episode,
                section: s.level,
                last_level: level_name,
                difficulty: s.difficulty,
                initial_difficulty: s.initial_difficulty,
                data_cubes: s.cubes,
                loadout: [
                    it[11] as u64,
                    it[0] as u64,
                    s.power[0].clamp(1, campaign_save::POWER_MAX) as u64,
                    it[1] as u64,
                    s.power[1].clamp(1, campaign_save::POWER_MAX) as u64,
                    it[9] as u64,
                    it[5] as u64,
                    it[3] as u64,
                    it[4] as u64,
                    it[10] as u64,
                    s.score.max(0) as u64,
                ],
                // import_c_save: one-player saves only (twoPlayerMode ignored).
                loadout2: [0u64; 11],
                input_devices: [0u8; 2],
                // config.c:playeritems_to_pitems : pItems[8] = initial_episode_num.
                initial_episode: it[8],
                game_has_repeated: s.game_has_repeated != 0,
                secret_hint: if (1..=3).contains(&s.secret_hint) { s.secret_hint } else { 0 },
                // config.c:playeritems_to_pitems : pItems[2] = super_arcade_mode.
                super_arcade_mode: it[2],
            };
            let Some(names) = self.item_names.get(episode.wrapping_sub(1) as usize).cloned().flatten() else {
                continue;
            };
            match campaign_save::save(&dir, &state, &names) {
                Ok(_) => n += 1,
                Err(e) => self.last_error = GString::from(format!("emplacement {slot} : {e}").as_str()),
            }
        }
        n
    }

    /// Profile of a C version (`opentyrian.dat`): settings (JSON of the `SETT`
    /// section, empty string if absent) and 20 arrays of high scores
    /// (arrays of three `{score, name, difficulty}`). Empty dictionary if the
    /// file is unreadable (`last_error`).
    #[func]
    fn read_c_profile(&mut self, c_path: GString) -> VarDictionary {
        let mut d = VarDictionary::new();
        let p = c_path.to_string();
        let data = match std::fs::read(&p) {
            Ok(b) => b,
            Err(e) => {
                self.last_error = GString::from(format!("{p}: {e}").as_str());
                return d;
            }
        };
        let save = match tyrian_formats::save::load_any(&data) {
            Ok(s) => s,
            Err(e) => {
                self.last_error = GString::from(format!("{p}: {e}").as_str());
                return d;
            }
        };
        let mut tables = VarArray::new();
        for table in &save.high_scores {
            let mut rows = VarArray::new();
            for e in table {
                let mut r = VarDictionary::new();
                r.set("score", e.score);
                r.set("name", e.name().trim_end().to_string().as_str());
                r.set("difficulty", e.difficulty as i32);
                rows.push(&r.to_variant());
            }
            tables.push(&rows.to_variant());
        }
        let settings = tyrian_formats::ot_dat::OtDat::parse(&data)
            .ok()
            .and_then(|dat| dat.section(b"SETT").map(|b| String::from_utf8_lossy(b).trim_end_matches(' ').to_string()))
            .unwrap_or_default();
        d.set("high_scores", &tables);
        d.set("settings_json", settings.as_str());
        d
    }

    /// `mainint.c:JE_totalScore` of an equipment set (11 values, order of
    /// `TyrianSim.set_loadout`, money last) with the item tables of
    /// `episode`: money plus value of the ship, weapons (power
    /// included), shield, generator and sidekicks. -1 if the
    /// episode tables are unreadable.
    #[func]
    fn total_score(&self, episode: i32, loadout: PackedInt32Array) -> i64 {
        use tyrian_sim::shop::{item_value, ShopCategory as C};
        let lo = loadout.as_slice();
        if lo.len() < 11 {
            return -1;
        }
        let Ok(t) = crate::shop::load_tables(Path::new(&self.data_dir), episode) else {
            return -1;
        };
        let v = |c, i: usize, p: i32| item_value(&t, c, lo[i].max(0) as u16, p.clamp(0, 255) as u8).max(0);
        lo[10].max(0) as i64
            + v(C::Ship, 0, 0)
            + v(C::FrontWeapon, 1, lo[2])
            + v(C::RearWeapon, 3, lo[4])
            + v(C::Shield, 5, 0)
            + v(C::Generator, 6, 0)
            + v(C::LeftSidekick, 7, 0)
            + v(C::RightSidekick, 8, 0)
    }

    /// `mainint.c:adjust_difficulty` (one player), `score` = `JE_totalScore`.
    #[func]
    fn adjust_difficulty(difficulty: i32, initial_difficulty: i32, score: i64) -> i32 {
        adjust_difficulty(difficulty.clamp(0, 10) as u8, initial_difficulty.clamp(0, 10) as u8, score.max(0) as u64)
            as i32
    }
}

impl TyrianCampaign {
    /// Item names of episode `ep` (1..=5), enabled mods included.
    fn names_for(&mut self, ep: u8) -> Option<ItemNames> {
        let base = self.item_names.get(ep.wrapping_sub(1) as usize).cloned().flatten()?;
        match self.mods.as_mut() {
            Some(m) => Some(m.bind_mut().save_item_names(ep as i32, &base)),
            None => Some(base),
        }
    }

    fn ensure_item_names(&mut self) {
        if self.item_names.is_empty() {
            let dir = PathBuf::from(&self.data_dir);
            self.item_names = (1..=EPISODE_MAX).map(|ep| ItemNames::load(&dir, ep).ok()).collect();
        }
    }
}

/// Equipment in the order of `TyrianSim.set_loadout` (without money).
fn loadout_values(l: &Loadout) -> [i32; 10] {
    [
        l.ship.0 as i32,
        l.front_weapon.0 as i32,
        l.front_power as i32,
        l.rear_weapon.0 as i32,
        l.rear_power as i32,
        l.shield.0 as i32,
        l.generator.0 as i32,
        l.left_sidekick.0 as i32,
        l.right_sidekick.0 as i32,
        l.special.0 as i32,
    ]
}
