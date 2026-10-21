//! Diagnostic: state of the special weapon and of player shots after N steps of a
//! golden master. Usage: special_trace FILE.gm N
use std::path::Path;
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let gm = GoldenMaster::load(Path::new(&args[1]))?;
    let steps: usize = args[2].parse()?;
    let mut sim = gm.make_sim(Path::new("../data"))?;
    for (i, t) in gm.ticks.iter().take(steps).enumerate() {
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
        let w = sim.world();
        let shots: Vec<_> = (0..w.shot_avail.len().min(w.player_shots.len()))
            .filter(|&k| w.shot_avail[k] != 0)
            .map(|k| (k, w.player_shots[k].shot_x, w.player_shots[k].shot_y, w.player_shots[k].shot_gr))
            .collect();
        println!(
            "tick {i} mask {:02x} special {} flare {} start {} wait {} rep {:?} power {} held {} shots {:?}",
            t.mask,
            w.players[0].items.special,
            w.special_weapon.flare_duration,
            w.special_weapon.flare_start,
            w.special_weapon.special_wait,
            w.shot_repeat,
            w.energy.power,
            w.special_weapon.fire_button_held,
            shots
        );
    }
    Ok(())
}
