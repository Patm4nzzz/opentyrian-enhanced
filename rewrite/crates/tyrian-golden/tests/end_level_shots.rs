//! End of level: tyrian2.c:1251 no longer calls JE_updateEnemyShots, no
//! enemy shot is drawn any more until the debriefing (the shots used to stay
//! displayed, frozen).

use std::path::{Path, PathBuf};
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn no_enemy_shot_is_drawn_once_the_level_has_ended() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/e1-tyrian-autopilot.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        eprintln!("data or golden master missing: test skipped");
        return;
    }
    let gm = GoldenMaster::load(&f).expect("golden master");
    let mut sim = gm.make_sim(&data).expect("sim");
    let mut ended_ticks = 0;
    for t in &gm.ticks {
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
        let w = sim.world();
        // The tick where end_level becomes true still shows the end of the previous
        // iteration (see hud_timing.rs): the following ticks are checked.
        if w.ending.end_level {
            ended_ticks += 1;
            if ended_ticks == 1 {
                continue;
            }
            assert!(
                w.render.drawn_enemy_shots.is_empty(),
                "tick {}: enemy shots drawn after the end of the level",
                t.tick
            );
        }
    }
    assert!(ended_ticks > 0, "the level never ended");
}
