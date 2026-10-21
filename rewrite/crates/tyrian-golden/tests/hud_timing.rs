//! Moment when the dashboard shows a hit taken. The C code draws the shield and armor bars
//! in `JE_playerCollide` / `JE_playerDamage`, at the top
//! of the iteration, and presents them at the end of that same iteration. `Sim::step`
//! plays the end of an iteration then the top of the next: a drawing made
//! in that top must only come out on the next call, otherwise the gauge drops
//! one frame before the C code's (Galaga pair at tick 1502: 162 pixels, at
//! tick 1692: 342 pixels of the armor bar).

use std::path::{Path, PathBuf};
use tyrian_golden::GoldenMaster;
use tyrian_sim::world::HudOp;
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn armor_bar_is_drawn_on_the_frame_the_c_presents_it() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/mode-galaga.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        eprintln!("data or Galaga golden master missing: test skipped");
        return;
    }
    let gm = GoldenMaster::load(&f).expect("golden master");
    let mut sim = gm.make_sim(&data).expect("sim");
    // Golden master armor: 28 until tick 1501, 19 at tick 1502.
    let armor_at = |t: u64| gm.ticks.iter().find(|r| r.tick == t).unwrap().armor;
    assert_eq!((armor_at(1501), armor_at(1502)), (28, 19));
    let mut drawn_armor = Vec::new();
    for t in &gm.ticks {
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
        for op in sim.take_hud_ops() {
            if let HudOp::Bars { armor: true, armors, .. } = op {
                drawn_armor.push((t.tick, armors[0]));
            }
        }
        if t.tick == 1504 {
            break;
        }
    }
    let first_19 = drawn_armor.iter().find(|&&(_, a)| a == 19).map(|&(t, _)| t);
    assert_eq!(first_19, Some(1503), "armor bar at 19: {drawn_armor:?}");
}
