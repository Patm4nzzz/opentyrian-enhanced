//! `tyrian-robustness`: plays seeded random games on the Rust core.
//!
//! Examples (from `rewrite/`):
//!   short version (CI)  : cargo run --release -p tyrian-robustness -- --games 200 --max-ticks 2000
//!   long version        : cargo run --release -p tyrian-robustness -- --games 20000 --max-ticks 20000
//!   replay a game       : cargo run --release -p tyrian-robustness -- --only 1234
//!
//! Each game is played twice (passes 1 and 2, reverse order and
//! different distribution among the threads); any trace difference is
//! reported as non-reproducibility. Exit code 1 if a defect is found.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use serde::Serialize;
use tyrian_robustness::{install_panic_hook, play, Catalogue, GameOutcome};

#[derive(Parser)]
#[command(about = "Seeded random games on tyrian-sim: panics, non-reproducibility, invariants")]
struct Args {
    /// `data/` folder of the original game.
    #[arg(long, default_value = "../data")]
    data: PathBuf,
    /// Number of games.
    #[arg(long, default_value_t = 200)]
    games: u64,
    /// First game identifier.
    #[arg(long, default_value_t = 0)]
    first: u64,
    /// Tick cap per game (a level lasts 3,000 to 8,000 ticks).
    #[arg(long, default_value_t = 2000)]
    max_ticks: u64,
    /// Worker threads (0 = all cores).
    #[arg(long, default_value_t = 0)]
    threads: usize,
    /// Episodes drawn (comma-separated list).
    #[arg(long, default_value = "1,2,3,4,5", value_delimiter = ',')]
    episodes: Vec<u8>,
    /// Replays a single game and prints its detail.
    #[arg(long)]
    only: Option<u64>,
    /// Equipment drawn over 0..=255 per field, including values outside the tables.
    #[arg(long)]
    wild_loadout: bool,
    /// JSON report (defects and statistics).
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(Serialize)]
struct Report {
    games: u64,
    first_id: u64,
    max_ticks: u64,
    wild_loadout: bool,
    ticks_per_pass: u64,
    ticks_total: u64,
    levels_ended: u64,
    seconds: f64,
    panics: Vec<GameOutcome>,
    invariants: Vec<GameOutcome>,
    non_reproducible: Vec<(GameOutcome, GameOutcome)>,
    per_level_games: BTreeMap<String, u64>,
}

fn run_pass(ids: &[u64], cat: &Catalogue, max_ticks: u64, wild: bool, threads: usize) -> Vec<GameOutcome> {
    let next = AtomicUsize::new(0);
    let mut results: Vec<Option<GameOutcome>> = vec![None; ids.len()];
    let slots: Vec<std::sync::Mutex<&mut Option<GameOutcome>>> =
        results.iter_mut().map(std::sync::Mutex::new).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= ids.len() {
                    break;
                }
                let o = play(ids[i], cat, max_ticks, wild);
                **slots[i].lock().unwrap() = Some(o);
            });
        }
    });
    drop(slots);
    results.into_iter().map(|o| o.expect("game played")).collect()
}

fn main() -> Result<()> {
    let args = Args::parse();
    install_panic_hook();
    let cat = Catalogue::load(&args.data, &args.episodes)?;
    let threads = if args.threads == 0 {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
    } else {
        args.threads
    };

    if let Some(id) = args.only {
        let a = play(id, &cat, args.max_ticks, args.wild_loadout);
        let b = play(id, &cat, args.max_ticks, args.wild_loadout);
        println!("{}", serde_json::to_string_pretty(&a)?);
        println!("reproducible: {}", if a == b { "yes" } else { "NO" });
        let bad = a.panic.is_some() || a.invariant.is_some() || a != b;
        std::process::exit(if bad { 1 } else { 0 });
    }

    let t0 = Instant::now();
    let ids: Vec<u64> = (args.first..args.first + args.games).collect();
    println!(
        "{} levels available, {} games, {} ticks max, {} threads",
        cat.levels.len(),
        args.games,
        args.max_ticks,
        threads
    );
    let pass1 = run_pass(&ids, &cat, args.max_ticks, args.wild_loadout, threads);
    // Pass 2: reverse order and different number of threads, so that each
    // game is replayed in a different execution context.
    let rev: Vec<u64> = ids.iter().rev().copied().collect();
    let mut pass2 = run_pass(&rev, &cat, args.max_ticks, args.wild_loadout, (threads / 2).max(1));
    pass2.reverse();
    let seconds = t0.elapsed().as_secs_f64();

    let mut report = Report {
        games: args.games,
        first_id: args.first,
        max_ticks: args.max_ticks,
        wild_loadout: args.wild_loadout,
        ticks_per_pass: pass1.iter().map(|o| o.ticks).sum(),
        ticks_total: pass1.iter().chain(pass2.iter()).map(|o| o.ticks).sum(),
        levels_ended: pass1.iter().filter(|o| o.level_ended).count() as u64,
        seconds,
        panics: Vec::new(),
        invariants: Vec::new(),
        non_reproducible: Vec::new(),
        per_level_games: BTreeMap::new(),
    };
    for (a, b) in pass1.iter().zip(pass2.iter()) {
        *report.per_level_games.entry(format!("ep{}_{:02}", a.spec.episode, a.spec.lvl_file)).or_default() += 1;
        if a.panic.is_some() {
            report.panics.push(a.clone());
        }
        if a.invariant.is_some() {
            report.invariants.push(a.clone());
        }
        if a != b {
            report.non_reproducible.push((a.clone(), b.clone()));
        }
    }
    println!(
        "{} games, {} ticks per pass ({} in total, 2 passes) in {:.1} s; {} levels completed",
        report.games, report.ticks_per_pass, report.ticks_total, seconds, report.levels_ended
    );
    println!(
        "{} distinct levels drawn; panics: {}; invariants: {}; non-reproducible: {}",
        report.per_level_games.len(),
        report.panics.len(),
        report.invariants.len(),
        report.non_reproducible.len()
    );
    // Groups panics by location and message, digits masked
    // ("index 37", "index 112"... = a single defect).
    let mut by_msg: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for o in &report.panics {
        let (_, m) = o.panic.as_ref().unwrap();
        // Digits masked only after "file.rs:line: ".
        let cut = m.find(".rs:").and_then(|p| m[p..].find(": ").map(|q| p + q)).unwrap_or(0);
        let tail: String = m[cut..].chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
        let mut key = format!("{}{tail}", &m[..cut]);
        while key.contains("##") {
            key = key.replace("##", "#");
        }
        by_msg.entry(key).or_default().push(o.spec.id);
    }
    for (m, ids) in &by_msg {
        println!("  panic ×{} (e.g. --only {}): {m}", ids.len(), ids[0]);
    }
    for o in report.invariants.iter().take(10) {
        println!("  invariant (--only {}): {:?}", o.spec.id, o.invariant);
    }
    for (a, _) in report.non_reproducible.iter().take(10) {
        println!("  non-reproducible: --only {}", a.spec.id);
    }
    if let Some(p) = &args.json {
        std::fs::write(p, serde_json::to_string_pretty(&report)? + "\n")?;
    }
    let bad = !report.panics.is_empty() || !report.invariants.is_empty() || !report.non_reproducible.is_empty();
    std::process::exit(if bad { 1 } else { 0 });
}
