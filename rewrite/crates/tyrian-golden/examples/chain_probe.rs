//! Campaign diagnostic: player state at the first tick of a chained golden
//! master, and search for the C state that reproduces `h_player`.
//! Usage: chain_probe FILE.gm
use std::path::Path;
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn mix(h: u64, v: i64) -> u64 {
    let mut h = h;
    for b in v.to_le_bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let gm = GoldenMaster::load_chained(Path::new(&args[1]), Path::new("../data"))?;
    let mut sim = gm.make_sim(Path::new("../data"))?;
    let n: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(1);
    for t in gm.ticks.iter().take(n) {
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
        let got = sim.frame_record();
        println!(
            "expected {} {} {} {} {} {} {} ps{} hp{:x}",
            t.tick, t.mask, t.px, t.py, t.armor, t.shield, t.cash, t.player_shots, t.h_player
        );
        println!(
            "got      {} {} {} {} {} {} {} ps{} hp{:x}",
            got.tick, got.mask, got.px, got.py, got.armor, got.shield, got.cash, got.player_shots, got.h_player
        );
    }
    let t0 = &gm.ticks[n - 1];
    let ps = &sim.world().players;
    for (i, p) in ps.iter().enumerate() {
        println!(
            "p{i} x {} y {} armor {} shield {} alive {} inv {} expl {} cash {} w {:?}",
            p.x, p.y, p.armor, p.shield, p.is_alive, p.invulnerable_ticks, p.exploding_ticks, p.cash, p.items.weapon
        );
    }
    // Search: p1 (Dragonwing) powers/cash and p0 powers.
    let field = |i: usize, p: &tyrian_sim::world::Player, w0: i64, w1: i64, cash: i64, armor: i64| {
        let _ = i;
        [
            p.x as i64,
            p.y as i64,
            armor,
            p.shield as i64,
            p.is_alive as i64,
            p.invulnerable_ticks as i64,
            p.exploding_ticks as i64,
            cash,
            w0,
            w1,
        ]
    };
    let p0 = ps[0];
    let p1 = ps[1];
    for w00 in 0..12i64 {
        for w01 in 0..12i64 {
            for w10 in 0..12i64 {
                for w11 in 0..12i64 {
                    let mut h = 0xcbf2_9ce4_8422_2325u64;
                    for v in field(0, &p0, w00, w01, p0.cash as i64, p0.armor as i64) {
                        h = mix(h, v);
                    }
                    for v in field(1, &p1, w10, w11, p1.cash as i64, p1.armor as i64) {
                        h = mix(h, v);
                    }
                    if h == t0.h_player {
                        println!("found: p0 powers {w00}:{w01} p1 powers {w10}:{w11}");
                    }
                }
            }
        }
    }
    Ok(())
}
