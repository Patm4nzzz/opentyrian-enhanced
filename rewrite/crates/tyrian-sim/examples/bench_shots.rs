//! Measures the time of one simulation tick with N enemy shots in flight.
//!
//!   cargo run --release -p tyrian-sim --example bench_shots -- [data] [ticks]
//!
//! For each N, level 1 of episode 1 (tyrian1.lvl no. 9, TYRIAN) is
//! played for `ticks` ticks with continuous fire and pilot aids enabled; shots are re-injected every tick
//! to stay at N. Prints the median, the 99th percentile and the maximum of the time
//! per tick (step + re-injection), and the matching 35 Hz budget.

use std::path::PathBuf;
use std::time::Instant;
use tyrian_sim::{setup::load_level, Loadout, PlayerInput, Sim};
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "../data".into()));
    let ticks: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let loadout = Loadout {
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
    println!("{:>7} {:>10} {:>10} {:>10} {:>10} {:>9}", "shots", "med. µs", "p99 µs", "max µs", "mean µs", "% 28.6 ms");
    for n in [0usize, 60, 500, 1000, 2000, 5000, 10000, 20000] {
        let mut setup = load_level(&data, 1, 0, 9, 2, loadout, 1).expect("level");
        // Pilot aids (hull and shield kept up): the run lasts for the whole
        // bench instead of stopping when the ship dies.
        setup.assist = true;
        let mut sim = Sim::new(setup);
        sim.bench_set_enemy_shot_capacity(n.max(60));
        let mut seed = 42u64;
        let mut times = Vec::with_capacity(ticks);
        let mut live = 0usize;
        for t in 0..ticks {
            let t0 = Instant::now();
            sim.bench_fill_enemy_shots(n, &mut seed);
            // Alternates left/right every 30 ticks, continuous fire.
            let dir = if (t / 30) % 2 == 0 { PlayerInput::LEFT } else { PlayerInput::RIGHT };
            sim.step([PlayerInput(dir | PlayerInput::FIRE), PlayerInput(0)]);
            times.push(t0.elapsed().as_nanos() as u64);
            live += sim.state().enemy_shots.len();
            if sim.state().level_ended {
                break;
            }
        }
        let count = times.len();
        let mean = times.iter().sum::<u64>() as f64 / count as f64 / 1000.0;
        times.sort_unstable();
        let q = |p: f64| times[((count as f64 * p) as usize).min(count - 1)] as f64 / 1000.0;
        println!(
            "{:>7} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>8.2}%   (average visible shots: {:.0}, {} ticks)",
            n,
            q(0.5),
            q(0.99),
            q(1.0),
            mean,
            q(0.99) / 28_571.4 * 100.0,
            live as f64 / count as f64,
            count
        );
    }
}
