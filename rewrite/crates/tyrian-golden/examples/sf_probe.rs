//! Diagnostic: replays an input script (GOLDEN_INPUT format: "ticks mask")
//! on the header of a golden master and prints the ticks where a
//! `mainint.c:JE_SFCodes` combination succeeded (`SFExecuted` non-zero).
//! Usage: sf_probe HEADER.gm SCRIPT.in
use std::path::Path;
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let gm = GoldenMaster::load(Path::new(&args[1]))?;
    let mut sim = gm.make_sim(Path::new("../data"))?;
    let mut tick = 0u64;
    let mut last: Vec<u8> = vec![0; 3];
    for line in std::fs::read_to_string(&args[2])?.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let n: u64 = it.next().unwrap().parse()?;
        let mask = u8::from_str_radix(it.next().unwrap(), 16)?;
        for _ in 0..n {
            sim.step([PlayerInput(mask), PlayerInput(0)]);
            let w = sim.world();
            let code = &w.controls.sf_current_code[0][..3];
            if code != last.as_slice() || w.controls.sf_executed[0] != 0 {
                println!(
                    "tick {tick} mask {mask:02x} code {code:?} sf {} invuln {}",
                    w.controls.sf_executed[0], w.players[0].invulnerable_ticks
                );
                last = code.to_vec();
            }
            if sim.state().level_ended {
                return Ok(());
            }
            tick += 1;
        }
    }
    Ok(())
}
