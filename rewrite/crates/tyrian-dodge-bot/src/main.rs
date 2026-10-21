//! `tyrian-dodge-bot`: headless bench for unassisted driving.
//!
//! Plays levels on the `tyrian-sim` core (without Godot, assists off)
//! with [`tyrian_dodge_bot::DodgeBot`], and records per game: end of level
//! or death, tick of death, hits taken, kills. Used to tune the
//! driver quickly (thousands of ticks per millisecond, one thread per game);
//! the real campaign (shop, game over, sequencing) is played in Godot
//! (`tools/campaign_bot.gd --campaign-bot-drive=dodge`).
//!
//! Examples (from `rewrite/`):
//!   cargo run --release -p tyrian-dodge-bot -- --levels 1:9,2:1,3:3,4:4,5:5 --seeds 8
//!   cargo run --release -p tyrian-dodge-bot -- --levels 1:9 --seeds 1 --trace /tmp/t.jsonl

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;
use tyrian_dodge_bot::{enemy_ref, is_pickup, DodgeBot, Tuning};
use tyrian_sim::{GeneratorId, Loadout, PlayerInput, ShieldId, ShipId, SidekickId, Sim, SpecialId, WeaponId};

#[derive(Parser)]
#[command(about = "Headless bench for the unassisted (dodging) driver on tyrian-sim")]
struct Args {
    #[arg(long, default_value = "../data")]
    data: PathBuf,
    /// Levels `episode:file` (lvl column of levelsN.dat), separated by commas.
    #[arg(long, value_delimiter = ',', default_value = "1:9,2:1,3:3,4:4,5:5")]
    levels: Vec<String>,
    /// Games per level (seeds 1..=n).
    #[arg(long, default_value_t = 4)]
    seeds: u64,
    #[arg(long, default_value_t = 2)]
    difficulty: u8,
    /// Equipment: ship, front, power, rear, power, shield, generator.
    #[arg(long, value_delimiter = ',', default_value = "1,1,1,0,1,4,2")]
    loadout: Vec<u8>,
    #[arg(long, default_value_t = 30000)]
    max_ticks: u64,
    #[arg(long, default_value_t = 130)]
    cruise_y: i32,
    #[arg(long, default_value_t = 3)]
    margin: i32,
    #[arg(long, default_value_t = 4)]
    crowd: i64,
    #[arg(long, default_value_t = 8)]
    aim: i64,
    #[arg(long, default_value_t = 12)]
    edge: i64,
    #[arg(long)]
    accel: bool,
    #[arg(long, default_value_t = 6)]
    close: i64,
    #[arg(long, default_value_t = 4)]
    growth: i32,
    #[arg(long, default_value_t = 18)]
    horizon: i32,
    #[arg(long, default_value_t = 4)]
    plan_first: i32,
    /// Do not pick up bonuses.
    #[arg(long)]
    no_collect: bool,
    /// Audit (diagnostic, reads the internal world, never the driver): for
    /// each hit taken, the enemy or shot responsible and what the driver saw of it.
    #[arg(long)]
    audit: bool,
    /// JSON trace (one line per hit taken and per end of game).
    #[arg(long)]
    trace: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    threads: usize,
    /// Attempts per level (game over -> same level, next tuning of
    /// `Tuning::attempt`, contact memory kept), as in the campaign.
    /// Replaces the command-line settings.
    #[arg(long, default_value_t = 0)]
    attempts: u32,
    /// Diagnostic: one status line every N ticks (0 = none).
    #[arg(long, default_value_t = 0)]
    log_every: u64,
    /// Level loaded as in the campaign (TyrianSim.set_campaign_state):
    /// initial menu difficulty, player 2 with starting equipment.
    #[arg(long)]
    campaign: Option<u8>,
}

#[derive(Serialize, Clone)]
struct Outcome {
    episode: u8,
    lvl: usize,
    seed: u64,
    completed: bool,
    dead: bool,
    ticks: u64,
    hits: u32,
    damage: i32,
    kills: u32,
    total_enemy: u32,
    hit_causes: Vec<String>,
    attempts: u32,
}

fn main() -> Result<()> {
    let a = Args::parse();
    let lo = &a.loadout;
    anyhow::ensure!(lo.len() == 7, "--loadout: 7 values expected");
    let loadout = Loadout {
        ship: ShipId(lo[0]),
        front_weapon: WeaponId(lo[1]),
        front_power: lo[2],
        rear_weapon: WeaponId(lo[3]),
        rear_power: lo[4],
        shield: ShieldId(lo[5]),
        generator: GeneratorId(lo[6]),
        left_sidekick: SidekickId(0),
        right_sidekick: SidekickId(0),
        special: SpecialId(0),
        cash: 0,
    };
    let mut jobs = Vec::new();
    for l in &a.levels {
        let (e, f) = l.split_once(':').context("level expected in the form episode:file")?;
        for s in 1..=a.seeds {
            jobs.push((e.parse::<u8>()?, f.parse::<usize>()?, s));
        }
    }
    let tuning = Tuning {
        cruise_y: a.cruise_y,
        margin: a.margin,
        margin_growth_8th: a.growth,
        collect: !a.no_collect,
        crowd: a.crowd,
        accel: a.accel,
        close: a.close,
        horizon: a.horizon,
        plan_first: a.plan_first,
        aim: a.aim,
        edge: a.edge,
    };
    let threads = if a.threads == 0 { std::thread::available_parallelism().map_or(4, |n| n.get()) } else { a.threads };
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.min(jobs.len()) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(ep, lvl, seed)) = jobs.get(i) else { break };
                let r = if a.attempts == 0 {
                    play(&a, ep, lvl, seed, loadout, &mut DodgeBot::new(tuning))
                } else {
                    let mut bot = DodgeBot::new(Tuning::attempt(0));
                    let mut r = play(&a, ep, lvl, seed, loadout, &mut bot);
                    let mut n = 1;
                    while !r.completed && n < a.attempts {
                        bot.tuning = Tuning::attempt(n);
                        r = play(&a, ep, lvl, seed, loadout, &mut bot);
                        n += 1;
                    }
                    r.attempts = n;
                    r
                };
                results.lock().unwrap().push(r);
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| (r.episode, r.lvl, r.seed));
    let mut trace = a.trace.as_ref().map(std::fs::File::create).transpose()?;
    let mut done = 0;
    for r in &results {
        println!(
            "ep. {} lvl {:2} seed {:2}: {:8} attempt {} tick {:6}  hits {:3} (damage {:3})  kills {:3}/{:3}  {}",
            r.episode,
            r.lvl,
            r.seed,
            if r.completed {
                "FINISHED"
            } else if r.dead {
                "dead"
            } else {
                "tick cap"
            },
            r.attempts,
            r.ticks,
            r.hits,
            r.damage,
            r.kills,
            r.total_enemy,
            if a.audit { r.hit_causes.join(" ; ") } else { String::new() }
        );
        done += r.completed as u32;
        if let Some(f) = trace.as_mut() {
            writeln!(f, "{}", serde_json::to_string(r)?)?;
        }
    }
    println!("total: {done}/{} levels finished", results.len());
    Ok(())
}

fn play(a: &Args, episode: u8, lvl: usize, seed: u64, loadout: Loadout, bot: &mut DodgeBot) -> Outcome {
    let mut setup =
        tyrian_sim::setup::load_level(&a.data, episode, 0, lvl, a.difficulty, loadout, seed).expect("level");
    if let Some(d) = a.campaign {
        setup.initial_difficulty = Some(d);
        setup.second_player_loadout = Some(Loadout {
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
        });
    }
    let mut sim = Sim::new(setup);
    let mut out = Outcome {
        episode,
        lvl,
        seed,
        completed: false,
        dead: false,
        ticks: 0,
        hits: 0,
        damage: 0,
        kills: 0,
        total_enemy: 0,
        hit_causes: Vec::new(),
        attempts: 1,
    };
    let mut hp = sim.state().players[0].armor + sim.state().players[0].shield;
    while out.ticks < a.max_ticks && !sim.state().level_ended {
        let input = bot.next(sim.state());
        let before = a.audit.then(|| sim.state().clone());
        sim.step([input, PlayerInput(0)]);
        out.ticks += 1;
        let p = sim.state().players[0];
        let now = p.armor + p.shield;
        if now < hp {
            out.hits += 1;
            out.damage += hp - now;
            if let Some(b) = before {
                let w = sim.world();
                let mut culprit = String::new();
                for i in 0..w.enemy.len() {
                    let e = &w.enemy[i];
                    let sx = e.ex as i32 + e.mapoffset as i32;
                    if w.enemy_avail[i] != 1 && (p.x - sx).abs() < 16 && (p.y - e.ey as i32).abs() < 18 {
                        let view = sim
                            .state()
                            .enemies
                            .iter()
                            .find(|v| v.slot as usize == i)
                            .map(|v| format!("{:?}", enemy_ref(v)));
                        culprit +=
                            &format!(" [world slot {i} ({sx},{}) exc {} eyc {} view {:?}]", e.ey, e.exc, e.eyc, view);
                    }
                }
                out.hit_causes.push(cause(&b, sim.state()) + &culprit);
            }
        }
        hp = now;
        if a.log_every > 0 && out.ticks.is_multiple_of(a.log_every) {
            let st = sim.state();
            let solid: Vec<String> = st
                .enemies
                .iter()
                .filter(|e| !is_pickup(e))
                .map(|e| format!("{},{}:a{}l{}f{}s{}", e.x, e.y, e.armor, e.layer, e.sheet, e.size))
                .collect();
            eprintln!(
                "{}:{} t{} cur_loc {} p({},{}) hp {} shots {} enemies {}",
                episode,
                lvl,
                out.ticks,
                st.cur_loc,
                p.x,
                p.y,
                now,
                st.enemy_shots.len(),
                solid.join(" ")
            );
        }
        if !p.alive && !out.dead {
            out.dead = true;
        }
    }
    let w = sim.world();
    out.kills = w.enemy_killed as u32;
    out.total_enemy = w.total_enemy as u32;
    out.completed = sim.state().level_ended && !out.dead;
    out
}

/// Diagnostic of a hit: the threat closest to the ship before the tick.
fn cause(before: &tyrian_sim::SimState, after: &tyrian_sim::SimState) -> String {
    let p = after.players[0];
    let b = before.players[0];
    let mut best = (i32::MAX, String::new());
    for e in &before.enemies {
        let (x, y) = enemy_ref(e);
        let d = (x - p.x).abs().max((y - p.y).abs());
        if d < best.0 {
            best = (
                d,
                format!("enemy l{} f{} a{} {}", e.layer, e.sheet, e.armor, if is_pickup(e) { "bonus" } else { "" }),
            );
        }
    }
    for s in &before.enemy_shots {
        let d = (s.x - p.x).abs().max((s.y - p.y).abs());
        if d < best.0 {
            best = (d, format!("shot s{}", s.sprite));
        }
    }
    format!("t{} ({},{})->({},{}) {} at {}", after.tick, b.x, b.y, p.x, p.y, best.1, best.0)
}
