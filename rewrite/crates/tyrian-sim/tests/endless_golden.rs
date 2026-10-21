//! Parity of Endless mode with the C golden master (`rewrite/golden/endless/*.egm`,
//! produced by `c-final:utils/golden/record_endless.sh` on `endless.c:endless_golden_dump`).

use std::path::PathBuf;
use tyrian_sim::endless::{armor_mult_for_levels, build_level_pool, f32_bits, scale_armor, Rotation, EPISODE_MAX};
use tyrian_sim::rng::Xoshiro256;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn endless_matches_c_golden() {
    let dir = root().join("rewrite/golden/endless");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "egm"))
        .collect();
    files.sort();
    assert!(files.len() >= 4, "Endless golden masters missing");
    let data = root().join("data");
    let pools: [Option<Vec<i32>>; EPISODE_MAX] = std::array::from_fn(|i| {
        let ep = i + 1;
        if !data.join(format!("tyrian{ep}.lvl")).exists() {
            return None;
        }
        Some(build_level_pool(&std::fs::read(data.join(format!("levels{ep}.dat"))).unwrap()))
    });
    let mut checked = 0usize;
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(' ').collect();
        let seed: u64 = head[2].parse().unwrap();
        let mut rng = Xoshiro256::new(seed);
        let mut rot = Rotation::new(pools.clone());
        rot.fill_block_queue(&mut rng);
        let mut k = 0;
        for line in lines {
            let w: Vec<&str> = line.split(' ').collect();
            match w[0] {
                "pool" => {
                    let ep: usize = w[1].parse().unwrap();
                    let want: Vec<i32> = w[3..].iter().map(|x| x.parse().unwrap()).collect();
                    assert_eq!(pools[ep - 1].clone().unwrap_or_default(), want, "{f:?} pool ep{ep}");
                }
                "pick" => {
                    let got = rot.pick_next_level(&mut rng);
                    let want = if w[2] == "none" { None } else { Some((w[2].parse().unwrap(), w[3].parse().unwrap())) };
                    assert_eq!(got, want, "{f:?} draw {k}");
                    k += 1;
                }
                "mult" => {
                    let lv: i32 = w[1].parse().unwrap();
                    let m = armor_mult_for_levels(lv);
                    assert_eq!(format!("{:08x}", f32_bits(m)), w[2], "{f:?} multiplier {lv}");
                    for (a, x) in w[3..].iter().enumerate() {
                        assert_eq!(scale_armor(a as u8 + 1, m).to_string(), *x, "{f:?} armor {} level {lv}", a + 1);
                    }
                }
                _ => {}
            }
            checked += 1;
        }
    }
    eprintln!("endless: {checked} identical lines");
}

/// The Endless multiplier does go through enemy creation
/// (enemy_spawn.c:JE_makeEnemy): without firing, same enemies at the same ticks, and
/// armor of each enemy = scale_armor(non-Endless armor) (armor < 255).
#[test]
fn armor_mult_reaches_spawned_enemies() {
    use tyrian_sim::{endless::start_loadout, setup::load_level, PlayerInput, Sim};
    let data = root().join("data");
    let mk = |mult| {
        let mut s = load_level(&data, 1, 2, 9, 3, start_loadout(), 5).unwrap();
        s.endless_armor_mult = mult;
        Sim::new(s)
    };
    let m = armor_mult_for_levels(20);
    let (mut a, mut b) = (mk(None), mk(Some(m)));
    let mut compared = 0;
    let mut prev: Vec<u16> = Vec::new();
    for _ in 0..1500 {
        a.step([PlayerInput(0), PlayerInput(0)]);
        b.step([PlayerInput(0), PlayerInput(0)]);
        let (ea, eb) = (&a.state().enemies, &b.state().enemies);
        // Tougher enemies survive longer: the two
        // games diverge sooner or later, we stop there.
        if ea.len() != eb.len() || ea.iter().zip(eb.iter()).any(|(x, y)| x.slot != y.slot) {
            break;
        }
        // Only at the spawn tick: afterwards collisions with the
        // ship remove points from both games.
        let fresh: Vec<u16> = ea.iter().map(|e| e.slot).filter(|s| !prev.contains(s)).collect();
        prev = ea.iter().map(|e| e.slot).collect();
        for (x, y) in ea.iter().zip(eb.iter()) {
            if fresh.contains(&x.slot) && x.armor < 255 && x.armor > 0 {
                assert_eq!(y.armor, scale_armor(x.armor as u8, m) as i32, "slot {}", x.slot);
                compared += 1;
            }
        }
    }
    assert!(compared > 0, "no enemy compared");
}

/// `EndlessRun` (path used by Godot) follows the C draws and each
/// drawn section designates a `tyrianN.lvl` level that loads.
#[test]
fn endless_run_follows_c_picks_and_levels_load() {
    use tyrian_sim::endless::{start_loadout, EndlessRun, START_DIFFICULTY};
    use tyrian_sim::setup::load_level;
    let data = root().join("data");
    for seed in [1u64, 7, 42, 1234567] {
        let text =
            std::fs::read_to_string(root().join(format!("rewrite/golden/endless/endless-seed{seed}.egm"))).unwrap();
        let mut run = EndlessRun::start(&data, seed, START_DIFFICULTY);
        for line in text.lines().filter(|l| l.starts_with("pick ")) {
            let w: Vec<&str> = line.split(' ').collect();
            let l = run.next_level().expect("draw");
            assert_eq!(
                (l.episode.to_string(), l.section.to_string()),
                (w[2].to_string(), w[3].to_string()),
                "seed {seed} {line}"
            );
            load_level(&data, l.episode, l.section, l.lvl_file_num, START_DIFFICULTY, start_loadout(), seed)
                .unwrap_or_else(|e| panic!("seed {seed} {line}: {e}"));
        }
    }
}
