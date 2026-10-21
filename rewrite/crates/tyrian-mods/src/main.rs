//! Command-line tool for mods.
//!
//!   tyrian-mods check [--data DIR] [--mods DIR] [--episode N] [--enable-all]
//!       (--mods defaults to `mods`, i.e. rewrite/mods from rewrite/)
//!       Loads the mods like the game, prints the summary and diagnostics.
//!       Exit code 1 if there is at least one error.
//!   tyrian-mods catalog [--data DIR] [--mods DIR] [--sheets DIR]
//!       Writes `<mods>/original-names.json`: names of the original enemies and
//!       enemy weapons that a mod can cite (`originals.rs`); with
//!       --sheets, one PNG sheet per image bank (docs/original-enemies).
//!   tyrian-mods run [--data DIR] [--mods DIR] --level epN_NN [--ticks N] [--enable-all] [--seed S]
//!       Plays the level headless with the mods and counts, per enemy
//!       type, the spawns and enemy shots (proof that a mod runs).

use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};
mod catalog_sheets;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use tyrian_mods::{episode_items, ModSet, Severity};
use tyrian_sim::{Loadout, PlayerInput, Sim};

struct Args {
    cmd: String,
    data: PathBuf,
    mods: PathBuf,
    episode: u8,
    level: Option<(u8, usize)>,
    ticks: u64,
    seed: u64,
    enable_all: bool,
    only: Vec<String>,
    sheets: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or("command expected: check | catalog | run")?;
    let mut a = Args {
        cmd,
        data: PathBuf::from("../data"),
        mods: PathBuf::from("mods"),
        episode: 1,
        level: None,
        ticks: 3000,
        seed: 1,
        enable_all: false,
        only: Vec::new(),
        sheets: None,
    };
    while let Some(k) = it.next() {
        let mut val = || it.next().ok_or(format!("missing value after {k}"));
        match k.as_str() {
            "--data" => a.data = PathBuf::from(val()?),
            "--mods" => a.mods = PathBuf::from(val()?),
            "--episode" => a.episode = val()?.parse().map_err(|_| "invalid episode")?,
            "--ticks" => a.ticks = val()?.parse().map_err(|_| "invalid ticks")?,
            "--seed" => a.seed = val()?.parse().map_err(|_| "invalid seed")?,
            "--enable-all" => a.enable_all = true,
            "--only" => a.only.push(val()?),
            "--sheets" => a.sheets = Some(PathBuf::from(val()?)),
            "--level" => {
                let v = val()?;
                let p: Vec<&str> = v.trim_start_matches("ep").split('_').collect();
                if p.len() != 2 {
                    return Err("--level expects epN_NN".into());
                }
                let e: u8 = p[0].parse().map_err(|_| "invalid episode")?;
                let n: usize = p[1].parse().map_err(|_| "invalid level")?;
                a.level = Some((e, n));
                a.episode = e;
            }
            _ => return Err(format!("unknown option {k}")),
        }
    }
    Ok(a)
}

fn load_set(a: &Args) -> ModSet {
    let mut set = ModSet::load_from(&a.data, &a.mods);
    let names: Vec<String> = set.folders.iter().map(|f| f.folder.clone()).collect();
    for n in names {
        if a.enable_all {
            set.set_enabled(&n, true);
        }
        if !a.only.is_empty() {
            set.set_enabled(&n, a.only.contains(&n));
        }
    }
    set
}

fn main() -> ExitCode {
    let a = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("tyrian-mods: {e}");
            return ExitCode::from(2);
        }
    };
    let set = load_set(&a);
    if a.cmd == "catalog" {
        if let Some(dir) = &a.sheets {
            match catalog_sheets::write_sheets(&a.data, &set.originals, dir) {
                Ok(n) => println!("{}: {n} sheets", dir.display()),
                Err(e) => {
                    eprintln!("tyrian-mods catalog: {e}");
                    return ExitCode::from(2);
                }
            }
        }
        let path = a.mods.join("original-names.json");
        return match std::fs::write(&path, set.originals.to_json()) {
            Ok(()) => {
                println!(
                    "{}: {} enemies, {} enemy weapons",
                    path.display(),
                    set.originals.enemies.len(),
                    set.originals.weapons.len()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("tyrian-mods catalog: {}: {e}", path.display());
                ExitCode::from(2)
            }
        };
    }
    let (lvl, items) = match episode_items(&a.data, a.episode) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("tyrian-mods: {e}");
            return ExitCode::from(2);
        }
    };
    let res = set.resolve(a.episode, &items);
    for r in &res.reports {
        let state = if !r.error.is_empty() {
            "ERROR"
        } else if r.enabled {
            "enabled"
        } else {
            "disabled"
        };
        println!(
            "{:<24} {:<9} weapons {} ships {} sidekicks {} shields {} specials {} enemies {} sprites {} enemy-weapons {} new-enemies {} levels {}",
            r.folder, state, r.weapons, r.ships, r.sidekicks, r.shields, r.specials, r.enemies, r.sprites, r.enemy_weapons, r.new_enemies, r.levels
        );
    }
    for d in &res.diagnostics.items {
        let tag = match d.severity {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "ERROR",
        };
        println!(
            "  [{tag}] {}{}",
            if d.folder.is_empty() { String::new() } else { format!("{}:", d.folder) },
            d.message
        );
    }
    let errors = res.diagnostics.count(Severity::Error);
    println!(
        "summary: {} mod(s), {} enabled, {} error(s), {} warning(s)",
        set.folders.len(),
        set.enabled_count(),
        errors,
        res.diagnostics.count(Severity::Warning)
    );
    if a.cmd == "check" {
        return if errors > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS };
    }
    if a.cmd != "run" {
        eprintln!("tyrian-mods: unknown command {}", a.cmd);
        return ExitCode::from(2);
    }
    let Some((episode, num)) = a.level else {
        eprintln!("tyrian-mods run: --level epN_NN required");
        return ExitCode::from(2);
    };
    let Some(level) = lvl.level(num).cloned() else {
        eprintln!("tyrian-mods run: no level {num}");
        return ExitCode::from(2);
    };
    let mut loadout = Loadout {
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
    };
    res.starter.apply(&mut loadout);
    let mut setup = tyrian_sim::LevelSetup {
        episode,
        section: 0,
        difficulty: 2,
        level,
        items,
        loadout,
        seed: a.seed,
        assist: true,
        initial_difficulty: None,
        second_player_loadout: None,
        carry_over: None,
        modes: Default::default(),
        endless_armor_mult: None,
        patch: Default::default(),
        processor_type: 4,
    };
    let replaced = res.apply_to(&mut setup, num as u16);
    println!(
        "level ep{episode}_{num:02} {}, {} behavior(s), {} modified enemy(ies)",
        if replaced { "replaced by a mod" } else { "original" },
        setup.patch.behaviors.len(),
        setup.patch.enemies.len()
    );
    let mut sim = Sim::new(setup);
    let mut seen: BTreeMap<u16, (u32, u32)> = BTreeMap::new();
    let mut alive_prev = [false; 100];
    let mut max_enemy_shots = 0usize;
    for t in 0..a.ticks {
        sim.step([
            PlayerInput(PlayerInput::FIRE | if (t / 60) % 2 == 0 { PlayerInput::LEFT } else { PlayerInput::RIGHT }),
            PlayerInput(0),
        ]);
        let w = sim.world();
        let mut alive = [false; 100];
        for i in 0..100 {
            if w.enemy_avail[i] != 1 {
                alive[i] = true;
                if !alive_prev[i] {
                    seen.entry(w.enemy[i].enemytype).or_default().0 += 1;
                }
            }
        }
        alive_prev = alive;
        max_enemy_shots = max_enemy_shots.max(sim.state().enemy_shots.len());
        if sim.state().level_ended {
            println!("level end at tick {t}");
            break;
        }
    }
    let state = sim.state();
    println!("tick {} cur_loc {} max simultaneous enemy shots {}", state.tick, state.cur_loc, max_enemy_shots);
    for (ty, (n, _)) in &seen {
        let name = match res.new_enemies.iter().find(|(_, &v)| v == *ty) {
            Some((k, _)) => format!(" ('{k}', mod)"),
            None => set
                .originals
                .enemy_name(*ty, tyrian_mods::originals::table_group(episode))
                .map(|k| format!(" ('{k}')"))
                .unwrap_or_default(),
        };
        println!("  enemy {ty}{name}: {n} spawn(s)");
    }
    println!("hash {:016x}", sim.state_hash());
    ExitCode::SUCCESS
}
