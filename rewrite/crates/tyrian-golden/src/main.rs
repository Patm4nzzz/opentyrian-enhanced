//! `tyrian-golden --data ../data file.gm...`: replays each golden master
//! with `tyrian-sim` and reports, per group, the first divergent tick.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tyrian_golden::{compare, FileReport, GoldenMaster, GROUPS};

#[derive(Parser)]
#[command(about = "Compares the C golden masters against the tyrian-sim Rust core")]
struct Args {
    /// `data/` folder of the original game.
    #[arg(long, default_value = "../data")]
    data: PathBuf,
    /// JSON summary file.
    #[arg(long, default_value = "golden/parity.json")]
    json: PathBuf,
    /// Prints the expected/obtained fields around the first divergence of the given group.
    #[arg(long)]
    trace: Option<String>,
    /// Number of ticks displayed by --trace.
    #[arg(long, default_value_t = 3)]
    trace_len: u64,
    /// Non-zero exit code if a single tick diverges, if a level end
    /// differs or if no file is provided (continuous integration).
    #[arg(long)]
    strict: bool,
    files: Vec<PathBuf>,
}

/// A recording is identical if it is so at every tick, for all
/// groups, and the level end coincides.
fn fully_identical(r: &FileReport) -> bool {
    r.matched_prefix == r.ticks
        && GROUPS.iter().all(|g| r.first_divergence[*g].is_none())
        && r.ended_early_at.is_none()
        && r.end_matches
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut reports: Vec<FileReport> = Vec::new();
    for f in &args.files {
        let gm = GoldenMaster::load_chained(f, &args.data)?;
        let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut shown = 0u64;
        let mut tracing = false;
        let trace = args.trace.clone();
        let report = compare(&gm, &args.data, &name, |exp, got, sim| {
            if let Some(g) = &trace {
                let diverged = match g.as_str() {
                    "player" => exp.h_player != got.h_player || exp.px != got.px || exp.py != got.py,
                    "scroll" => exp.h_scroll != got.h_scroll || exp.cur_loc != got.cur_loc,
                    "enemies" => exp.h_enemies != got.h_enemies || exp.enemies != got.enemies,
                    "pshots" => exp.h_player_shots != got.h_player_shots || exp.player_shots != got.player_shots,
                    "eshots" => exp.h_enemy_shots != got.h_enemy_shots || exp.enemy_shots != got.enemy_shots,
                    "rng" => exp.h_rng != got.h_rng,
                    _ => false,
                };
                if diverged {
                    tracing = true;
                }
                if tracing && shown < args.trace_len {
                    shown += 1;
                    println!("  tick {} expected: {:?}", exp.tick, exp);
                    println!("  tick {} got:      {:?}", got.tick, got);
                    let w = sim.world();
                    if g == "pshots" {
                        println!(
                            "    pshots={:?} shot_repeat={:?} special={} sp={:?} fire_held={} power={} flare={} wpn={} front={:?}",
                            (0..w.shot_avail.len()).filter(|&i| w.shot_avail[i] != 0).map(|i| (i, w.player_shots[i].shot_x, w.player_shots[i].shot_y, w.player_shots[i].shot_gr)).collect::<Vec<_>>(),
                            w.shot_repeat,
                            w.players[0].items.special,
                            w.tables.specials[w.players[0].items.special.index()],
                            w.special_weapon.fire_button_held,
                            w.energy.power, w.special_weapon.flare_duration, w.special_weapon.special_weapon_wpn,
                            w.tables.weapon(w.special_weapon.special_weapon_wpn as usize)
                        );
                    }
                    println!(
                        "    cur_loc={} event_loc={} map_y={:?} back_move={:?} enemies={:?}",
                        w.script.cur_loc,
                        w.script.event_loc,
                        w.background.map_y,
                        w.background.back_move,
                        (0..100)
                            .filter(|&i| w.enemy_avail[i] != 1)
                            .map(|i| (i, w.enemy[i].enemytype, w.enemy[i].ex, w.enemy[i].ey, w.enemy[i].armorleft))
                            .collect::<Vec<_>>()
                    );
                }
            }
        })?;
        println!("{}: {} ticks, {} identical from the start", report.file, report.ticks, report.matched_prefix);
        for g in GROUPS {
            match report.first_divergence[g] {
                None => println!("  {g:8} OK over the {} ticks", report.ticks),
                Some(t) => println!("  {g:8} first divergence at tick {t}"),
            }
        }
        if let Some(t) = report.ended_early_at {
            println!("  premature Rust level end at tick {t}");
        }
        println!("  level end {}", if report.end_matches { "identical" } else { "DIFFERENT" });
        reports.push(report);
    }
    if let Some(dir) = args.json.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(&args.json, serde_json::to_string_pretty(&reports)? + "\n")?;
    println!("summary written to {}", args.json.display());
    let total: usize = reports.iter().map(|r| r.ticks).sum();
    let identical: Vec<&FileReport> = reports.iter().filter(|r| fully_identical(r)).collect();
    let identical_ticks: usize = identical.iter().map(|r| r.ticks).sum();
    println!("total: {}/{} identical recordings, {identical_ticks}/{total} ticks", identical.len(), reports.len());
    if args.strict {
        if reports.is_empty() {
            anyhow::bail!("--strict: no golden master given");
        }
        let bad: Vec<&str> = reports.iter().filter(|r| !fully_identical(r)).map(|r| r.file.as_str()).collect();
        if !bad.is_empty() {
            anyhow::bail!("--strict: parity broken on {}", bad.join(", "));
        }
    }
    Ok(())
}
