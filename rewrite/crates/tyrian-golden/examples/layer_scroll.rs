//! Diagnostic: views published by the simulation after N steps of a golden master
//! (used to check the Godot rendering). Usage: layer_scroll FILE.gm N
use std::path::Path;
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let gm = GoldenMaster::load(Path::new(&args[1]))?;
    let steps: usize = args[2].parse()?;
    let mut sim = gm.make_sim(Path::new("../data"))?;
    for t in gm.ticks.iter().take(steps) {
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
    }
    let s = sim.state();
    println!("tick {} cur_loc {} layers {:?}", s.tick, s.cur_loc, s.layers);
    for e in s.enemies.iter().take(4) {
        println!("enemy {:?}", e);
    }
    Ok(())
}
