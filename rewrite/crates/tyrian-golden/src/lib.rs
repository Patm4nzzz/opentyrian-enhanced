//! Reading of golden masters (`src/golden_master.c`, ADR 0006) and
//! tick-by-tick comparison with `tyrian-sim`.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tyrian_sim::modes::{ModeRecord, ModeSetup};
use tyrian_sim::{FrameRecord, LevelCarry, Loadout, PlayerInput, Sim};
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

pub mod writer;

/// Header of a `.gm` file.
#[derive(Debug, Clone)]
pub struct GmHeader {
    pub episode: u8,
    pub section: u8,
    pub level_name: String,
    pub lvl_file: usize,
    pub difficulty: u8,
    pub seed: u64,
    pub assist: bool,
    pub loadout: Loadout,
    /// `campaign` line (chained golden masters, `GOLDEN_CONTINUE=2`):
    /// initial cash and difficulty cannot be deduced from the episode.
    pub campaign: Option<CampaignHeader>,
    /// Format v2 (side modes): "mode" and "items2" lines.
    pub modes: Option<ModeSetup>,
    /// "endless wave W lives L0:L1 armor_mult BITS" line (`--goto=endless`).
    pub endless: Option<EndlessHeader>,
    /// "combo frame_ms N count C multiplier M" line (`GOLDEN_COMBO_MS`):
    /// active combos, `combos.c:combo_update(N)` on every loop.
    pub combo_frame_ms: Option<u32>,
    /// "demo N" line: title demo (`play_demo`), keys read
    /// from `data/demo.N` (`mainint.c:replay_demo_keys`).
    pub demo: Option<u8>,
    /// "detail N" line (`GOLDEN_DETAIL`): `config.c:processorType` of the
    /// C launch (Visual FX). Absent at the default 4.
    pub detail: u8,
}

/// "endless_end score S lives L0:L1" line written by `golden_master.c:finish`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndlessEnd {
    /// `endless.c:endless_score` (multiplied by the combos if `GOLDEN_COMBO_MS`).
    pub score: u64,
    pub lives: [i32; 2],
}

/// Header of an Endless level golden master (`golden_master.c:golden_master_level_start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndlessHeader {
    /// `endless.c:endless_wave` (completed levels + 1).
    pub wave: i32,
    /// `endless.c:endless_lives_of(0..2)`.
    pub lives: [i32; 2],
    /// IEEE 754 bits of `endless_armor_multiplier()`.
    pub armor_mult_bits: u32,
}

/// `campaign cash N initial_difficulty D episode_start E`
/// (`golden_master.c:golden_master_level_start`, multi-level mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignHeader {
    pub cash: u32,
    pub initial_difficulty: u8,
    pub episode_start: u8,
}

#[derive(Debug, Clone)]
pub struct GoldenMaster {
    pub header: GmHeader,
    /// Chained campaign (`NAME-K.gm`, K > 1): state inherited from the end of
    /// level `NAME-(K-1).gm`, obtained by replaying it (see
    /// [`PlayerCarry`]). `None` for a standalone level.
    pub carry: Option<LevelCarry>,
    /// Difficulty computed for this level from the end of the previous one
    /// (`mainint.c:adjust_difficulty` in `JE_endLevelAni`), to compare with
    /// the header written by the C code.
    pub derived_difficulty: Option<u8>,
    pub ticks: Vec<FrameRecord>,
    /// Extended fields of the v2 lines (empty in v1).
    pub mode_ticks: Vec<ModeRecord>,
    /// `level` or `script`.
    pub end_reason: String,
    /// Endless score and lives at the end of the recording (`--goto=endless`).
    pub endless_end: Option<EndlessEnd>,
}

fn kv<'a>(tokens: &'a [&'a str], key: &str) -> Result<&'a str> {
    let i = tokens.iter().position(|t| *t == key).ok_or_else(|| anyhow!("missing key '{key}'"))?;
    tokens.get(i + 1).copied().ok_or_else(|| anyhow!("missing value for '{key}'"))
}

fn pair(s: &str) -> Result<(u8, u8)> {
    let (a, b) = s.split_once(':').ok_or_else(|| anyhow!("pair expected: {s}"))?;
    Ok((a.parse()?, b.parse()?))
}

fn parse_items(t: &[&str]) -> Result<Loadout> {
    let (fw, fp) = pair(kv(t, "front")?)?;
    let (rw, rp) = pair(kv(t, "rear")?)?;
    let (ls, rs) = pair(kv(t, "sidekicks")?)?;
    Ok(Loadout {
        ship: ShipId(kv(t, "ship")?.parse()?),
        front_weapon: WeaponId(fw),
        front_power: fp,
        rear_weapon: WeaponId(rw),
        rear_power: rp,
        shield: ShieldId(kv(t, "shield")?.parse()?),
        generator: GeneratorId(kv(t, "generator")?.parse()?),
        left_sidekick: SidekickId(ls),
        right_sidekick: SidekickId(rs),
        special: SpecialId(kv(t, "special")?.parse()?),
        cash: 0,
    })
}

fn hex(s: &str) -> Result<u64> {
    u64::from_str_radix(s, 16).with_context(|| format!("invalid hexadecimal: {s}"))
}

/// `opentyr.c:apply_goto_spec`: starting cash per episode.
pub fn initial_cash(episode: u8) -> u32 {
    const T: [u32; 5] = [10000, 15000, 20000, 30000, 20000];
    T[(episode.clamp(1, 5) - 1) as usize]
}

/// Equipment of a new game (`mainint.c:JE_initPlayerData`, without
/// `mod.json`): this is what `player[1]` keeps throughout the campaign.
pub fn new_game_loadout() -> Loadout {
    Loadout {
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
    }
}

impl GoldenMaster {
    pub fn parse(text: &str) -> Result<Self> {
        let mut episode = None;
        let mut assist = false;
        let mut loadout = None;
        let mut campaign = None;
        let mut ticks = Vec::new();
        let mut mode_ticks = Vec::new();
        let mut loadout2 = None;
        let mut modes: Option<ModeSetup> = None;
        let mut end_reason = String::new();
        let mut endless = None;
        let mut endless_end = None;
        let mut combo_frame_ms = None;
        let mut demo = None;
        let mut detail = 4u8;
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let t: Vec<&str> = line.split_whitespace().collect();
            match t[0] {
                "episode" => {
                    // The level name may contain padding spaces:
                    // "level NAME   lvlfile".
                    let li = line.find(" level ").ok_or_else(|| anyhow!("missing level"))? + 7;
                    let lf = line.find(" lvlfile ").ok_or_else(|| anyhow!("missing lvlfile"))?;
                    let name = line[li..lf].trim().to_string();
                    episode = Some((
                        kv(&t, "episode")?.parse::<u8>()?,
                        kv(&t, "section")?.parse::<u8>()?,
                        name,
                        kv(&t, "lvlfile")?.parse::<usize>()?,
                        kv(&t, "difficulty")?.parse::<u8>()?,
                        kv(&t, "seed")?.parse::<u64>()?,
                    ));
                }
                "campaign" => {
                    campaign = Some(CampaignHeader {
                        cash: kv(&t, "cash")?.parse()?,
                        initial_difficulty: kv(&t, "initial_difficulty")?.parse()?,
                        episode_start: kv(&t, "episode_start")?.parse()?,
                    });
                }
                "assist" => assist = t.get(1) == Some(&"1"),
                // "replay rec|play": golden master written during a replay (ADR 0090).
                "replay" => {}
                "items" => loadout = Some(parse_items(&t)?),
                "items2" => loadout2 = Some(parse_items(&t)?),
                "mode" => {
                    let (c0, c1) = kv(&t, "cash")?.split_once(':').ok_or_else(|| anyhow!("cash"))?;
                    let (d0, d1) = pair(kv(&t, "devices")?)?;
                    modes = Some(ModeSetup {
                        two_player: kv(&t, "twoplayer")? == "1",
                        galaga: kv(&t, "galaga")? == "1",
                        one_player_action: kv(&t, "oneplayer")? == "1",
                        super_arcade: kv(&t, "superarcade")?.parse()?,
                        super_tyrian: kv(&t, "supertyrian")? == "1",
                        timed: kv(&t, "timed")?.parse()?,
                        mutators: u8::from_str_radix(kv(&t, "mutators")?, 16)?,
                        anomalies: u16::from_str_radix(kv(&t, "anomalies")?, 16)?,
                        devices: (d0, d1),
                        cash: (c0.parse()?, c1.parse()?),
                        initial_difficulty: kv(&t, "initdiff")?.parse()?,
                        loadout2: None,
                        endless_lives: None,
                    });
                }
                "endless" => {
                    let (l0, l1) = kv(&t, "lives")?.split_once(':').ok_or_else(|| anyhow!("lives"))?;
                    endless = Some(EndlessHeader {
                        wave: kv(&t, "wave")?.parse()?,
                        lives: [l0.parse()?, l1.parse()?],
                        armor_mult_bits: u32::from_str_radix(kv(&t, "armor_mult")?, 16)?,
                    });
                }
                "combo" => combo_frame_ms = Some(kv(&t, "frame_ms")?.parse()?),
                "demo" => demo = Some(t.get(1).ok_or_else(|| anyhow!("missing demo number"))?.parse()?),
                "detail" => detail = t.get(1).ok_or_else(|| anyhow!("detail"))?.parse()?,
                "endless_end" => {
                    let (l0, l1) = kv(&t, "lives")?.split_once(':').ok_or_else(|| anyhow!("lives"))?;
                    endless_end =
                        Some(EndlessEnd { score: kv(&t, "score")?.parse()?, lives: [l0.parse()?, l1.parse()?] });
                }
                "end" => end_reason = t.get(1).unwrap_or(&"").to_string(),
                _ => {
                    if t.len() == 24 {
                        mode_ticks.push(ModeRecord {
                            mouse_mask: u8::from_str_radix(t[18], 16)?,
                            mouse_dx: t[19].parse()?,
                            mouse_dy: t[20].parse()?,
                            p2x: t[21].parse()?,
                            p2y: t[22].parse()?,
                            h_mode: hex(t[23])?,
                        });
                    } else if t.len() != 18 {
                        bail!("invalid tick line: {line}");
                    }
                    ticks.push(FrameRecord {
                        tick: t[0].parse()?,
                        mask: u8::from_str_radix(t[1], 16)?,
                        px: t[2].parse()?,
                        py: t[3].parse()?,
                        armor: t[4].parse()?,
                        shield: t[5].parse()?,
                        cash: t[6].parse()?,
                        cur_loc: t[7].parse()?,
                        enemies: t[8].parse()?,
                        player_shots: t[9].parse()?,
                        enemy_shots: t[10].parse()?,
                        h_player: hex(t[11])?,
                        h_scroll: hex(t[12])?,
                        h_enemies: hex(t[13])?,
                        h_player_shots: hex(t[14])?,
                        h_enemy_shots: hex(t[15])?,
                        h_mt: hex(t[16])?,
                        h_rng: hex(t[17])?,
                    });
                }
            }
        }
        let (episode, section, level_name, lvl_file, difficulty, seed) =
            episode.ok_or_else(|| anyhow!("missing 'episode' line"))?;
        let mut loadout = loadout.ok_or_else(|| anyhow!("missing 'items' line"))?;
        loadout.cash = match campaign {
            Some(c) => c.cash,
            None => initial_cash(episode),
        };
        if let Some(m) = modes.as_mut() {
            // v2: the starting cash is recorded as is (apply_goto_spec or arcade modes).
            loadout.cash = m.cash.0 as u32;
            m.loadout2 = loadout2;
        }
        if modes.is_some() && mode_ticks.len() != ticks.len() {
            bail!("format v2: tick lines without extended fields");
        }
        Ok(Self {
            header: GmHeader {
                episode,
                section,
                level_name,
                lvl_file,
                difficulty,
                seed,
                assist,
                loadout,
                campaign,
                modes,
                endless,
                combo_frame_ms,
                demo,
                detail,
            },
            carry: None,
            derived_difficulty: None,
            ticks,
            mode_ticks,
            end_reason,
            endless_end,
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("{}", path.display()))?;
        Self::parse(&text).with_context(|| format!("{}", path.display()))
    }

    /// Previous file of a chained campaign (`NAME-K.gm` → `NAME-(K-1).gm`),
    /// as written by `c-final:utils/golden/record_campaign_chain.sh`.
    pub fn chain_predecessor(path: &Path) -> Option<std::path::PathBuf> {
        let stem = path.file_stem()?.to_str()?;
        let (prefix, k) = stem.rsplit_once('-')?;
        let k: u32 = k.parse().ok()?;
        (k > 1).then(|| path.with_file_name(format!("{prefix}-{}.gm", k - 1)))
    }

    /// Like [`load`](Self::load), then, for a chained level, replays the
    /// previous levels of the campaign to obtain the inherited state.
    pub fn load_chained(path: &Path, data_dir: &Path) -> Result<Self> {
        let mut gm = Self::load(path)?;
        if gm.header.campaign.is_some() {
            if let Some(prev) = Self::chain_predecessor(path).filter(|p| p.exists()) {
                let prev_gm = Self::load_chained(&prev, data_dir)?;
                let mut sim = prev_gm.make_sim(data_dir)?;
                for t in &prev_gm.ticks {
                    if sim.state().level_ended {
                        break;
                    }
                    sim.step([PlayerInput(t.mask), PlayerInput(0)]);
                }
                gm.carry = Some(LevelCarry::from_sim(&sim));
                let c = prev_gm.header.campaign.unwrap();
                let p0 = &sim.world().players[0];
                let score = tyrian_sim::campaign::total_score(&sim.world().tables, &p0.items, p0.cash);
                // tyrian2.c:JE_main: episode 5 removes one notch during the
                // level, rendered by "difficultyLevel = oldDifficultyLevel".
                let before = prev_gm.header.difficulty + (prev_gm.header.episode == 5) as u8;
                let mut d = tyrian_sim::campaign::adjust_difficulty(before, c.initial_difficulty, score);
                if gm.header.episode == 5 {
                    d = d.saturating_sub(1).max(1);
                }
                gm.derived_difficulty = Some(d);
            }
        }
        Ok(gm)
    }

    pub fn make_sim(&self, data_dir: &Path) -> Result<Sim> {
        let h = &self.header;
        let mut setup =
            tyrian_sim::setup::load_level(data_dir, h.episode, h.section, h.lvl_file, h.difficulty, h.loadout, h.seed)
                .map_err(|e| anyhow!(e))?;
        setup.assist = h.assist;
        setup.initial_difficulty = h.campaign.map(|c| c.initial_difficulty);
        if h.campaign.is_some() {
            setup.second_player_loadout = Some(new_game_loadout());
        }
        setup.carry_over = self.carry;
        if let Some(m) = h.modes {
            setup.modes = m;
        }
        if let Some(e) = h.endless {
            // Multiplier recomputed by the Sim (endless.c:endless_armor_mult_for_levels)
            // and checked against the bits recorded by the C code.
            let mult = tyrian_sim::endless::armor_mult_for_levels(e.wave - 1);
            if tyrian_sim::endless::f32_bits(mult) != e.armor_mult_bits {
                bail!(
                    "Endless armor multiplier {:08x} ≠ {:08x} (wave {})",
                    tyrian_sim::endless::f32_bits(mult),
                    e.armor_mult_bits,
                    e.wave
                );
            }
            setup.endless_armor_mult = Some(mult);
            setup.modes.endless_lives = Some(e.lives);
        }
        // GOLDEN_DETAIL: effects level of the C launch (config.c:processorType).
        // Injected before `Sim::new`, which draws the stars of `initialize_starfield`
        // with this setting.
        setup.processor_type = h.detail;
        let mut sim = Sim::new(setup);
        if let Some(n) = h.demo {
            let path = data_dir.join(format!("demo.{n}"));
            let bytes = std::fs::read(&path).with_context(|| format!("{}", path.display()))?;
            let demo = tyrian_formats::demo::Demo::parse(&bytes).map_err(|e| anyhow!("{}: {e}", path.display()))?;
            sim.set_demo(tyrian_sim::demo::DemoInput::new(&demo));
        }
        if let Some(ms) = h.combo_frame_ms {
            sim.enable_combos(ms);
        }
        Ok(sim)
    }
}

/// Groups compared, in report order.
/// `mode`: extended v2 fields (player 2, hash_mode); always identical in v1.
pub const GROUPS: [&str; 8] = ["player", "scroll", "enemies", "pshots", "eshots", "rng", "mt", "mode"];

/// Result of the comparison of a file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileReport {
    pub file: String,
    pub ticks: usize,
    /// Number of consecutive identical ticks from the start (all fields).
    pub matched_prefix: usize,
    /// First divergent tick per group (`None` = identical over the whole file).
    pub first_divergence: std::collections::BTreeMap<String, Option<u64>>,
    /// The level ended on the Rust side before the end of the file.
    pub ended_early_at: Option<u64>,
    /// The Rust level end coincides with the C one (`end level`: level
    /// completed just after the last tick; `end script`: level in progress).
    pub end_matches: bool,
}

fn group_eq(group: &str, a: &FrameRecord, b: &FrameRecord) -> bool {
    match group {
        "player" => {
            a.px == b.px
                && a.py == b.py
                && a.armor == b.armor
                && a.shield == b.shield
                && a.cash == b.cash
                && a.h_player == b.h_player
        }
        "scroll" => a.cur_loc == b.cur_loc && a.h_scroll == b.h_scroll,
        "enemies" => a.enemies == b.enemies && a.h_enemies == b.h_enemies,
        "pshots" => a.player_shots == b.player_shots && a.h_player_shots == b.h_player_shots,
        "eshots" => a.enemy_shots == b.enemy_shots && a.h_enemy_shots == b.h_enemy_shots,
        "rng" => a.h_rng == b.h_rng,
        "mt" => a.h_mt == b.h_mt,
        _ => true,
    }
}

/// Replays the inputs of a golden master and compares every tick.
/// `on_tick` receives (expected, obtained) for diagnostic tooling.
pub fn compare(
    gm: &GoldenMaster,
    data_dir: &Path,
    name: &str,
    mut on_tick: impl FnMut(&FrameRecord, &FrameRecord, &Sim),
) -> Result<FileReport> {
    let mut sim = gm.make_sim(data_dir)?;
    let mut first: std::collections::BTreeMap<String, Option<u64>> =
        GROUPS.iter().map(|g| (g.to_string(), None)).collect();
    let mut matched_prefix = 0usize;
    let mut prefix_open = true;
    let mut ended_early_at = None;
    for (k, exp) in gm.ticks.iter().enumerate() {
        if sim.state().level_ended {
            ended_early_at = Some(exp.tick);
            for g in GROUPS {
                first.get_mut(g).unwrap().get_or_insert(exp.tick);
            }
            break;
        }
        // golden_master.c:golden_master_inject: when the input script is exhausted,
        // finish("script") rewrites the open tick without a new loop iteration;
        // the last line of an "end script" recording therefore repeats the previous state.
        let script_tail = gm.end_reason == "script" && k + 1 == gm.ticks.len() && k > 0;
        if !script_tail {
            if let Some(m) = gm.mode_ticks.get(k) {
                sim.set_mouse(tyrian_sim::modes::MouseInput { mask: m.mouse_mask, dx: m.mouse_dx, dy: m.mouse_dy });
            }
            sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        }
        let mut got = sim.frame_record();
        if script_tail {
            got.tick = exp.tick;
            got.mask = exp.mask;
        }
        on_tick(exp, &got, &sim);
        let mode_ok = gm.mode_ticks.get(k).is_none_or(|m| {
            let r = sim.mode_record();
            m.p2x == r.p2x && m.p2y == r.p2y && m.h_mode == r.h_mode
        });
        let mut all = true;
        for g in GROUPS {
            let eq = if g == "mode" { mode_ok } else { group_eq(g, exp, &got) };
            if !eq {
                all = false;
                let slot = first.get_mut(g).unwrap();
                if slot.is_none() {
                    *slot = Some(exp.tick);
                }
            }
        }
        if all && prefix_open {
            matched_prefix += 1;
        } else {
            prefix_open = false;
        }
    }
    let end_matches = ended_early_at.is_none() && sim.state().level_ended == (gm.end_reason == "level");
    Ok(FileReport {
        file: name.to_string(),
        ticks: gm.ticks.len(),
        matched_prefix,
        first_divergence: first,
        ended_early_at,
        end_matches,
    })
}

/// Replays a C-version `.rep` in `tyrian-sim` (`tyrian_sim::replay`)
/// and compares every tick to the golden master written by the C binary during ITS
/// replay of the same file (`GOLDEN_REPLAY_PLAY`, c-final:utils/golden/record_replay.sh,
/// ADR 0090). Expected end: `end replay` (file exhausted) or `end level`.
pub fn compare_replay(rep_path: &Path, gm: &GoldenMaster, data_dir: &Path, name: &str) -> Result<(FileReport, u64)> {
    let mut run = tyrian_sim::replay::ReplayRun::load(data_dir, rep_path).map_err(|e| anyhow!(e))?;
    let mut first: std::collections::BTreeMap<String, Option<u64>> =
        GROUPS.iter().map(|g| (g.to_string(), None)).collect();
    let mut matched_prefix = 0usize;
    let mut prefix_open = true;
    let mut ended_early_at = None;
    for exp in &gm.ticks {
        if !run.step() {
            ended_early_at = Some(exp.tick);
            for g in GROUPS {
                first.get_mut(g).unwrap().get_or_insert(exp.tick);
            }
            break;
        }
        let got = run.sim.frame_record();
        let mut all = true;
        for g in GROUPS {
            if !group_eq(g, exp, &got) {
                all = false;
                first.get_mut(g).unwrap().get_or_insert(exp.tick);
            }
        }
        if all && prefix_open {
            matched_prefix += 1;
        } else {
            prefix_open = false;
        }
    }
    let end_matches =
        ended_early_at.is_none() && run.finished() && run.sim.state().level_ended == (gm.end_reason == "level");
    let hash = run.sim.state_hash();
    Ok((
        FileReport {
            file: name.to_string(),
            ticks: gm.ticks.len(),
            matched_prefix,
            first_divergence: first,
            ended_early_at,
            end_matches,
        },
        hash,
    ))
}
