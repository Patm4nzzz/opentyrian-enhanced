//! Message of an event 16 (level_events.c:414, JE_drawTextWindow): the
//! simulation must publish the message index so that the bottom bar
//! displays it. C | Godot pair at tick 2420 of e1-tyrian-autopilot: 287 pixels
//! of difference in the bottom band when the text was missing.

use std::path::{Path, PathBuf};
use tyrian_golden::GoldenMaster;
use tyrian_sim::world::HudOp;
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn level_event_message_is_published() {
    let data = root().join("data");
    let f = root().join("rewrite/golden/e1-tyrian-autopilot.gm");
    if !data.join("tyrian.hdt").exists() || !f.exists() {
        eprintln!("data or golden master missing: test skipped");
        return;
    }
    let gm = GoldenMaster::load(&f).expect("golden master");
    let mut sim = gm.make_sim(&data).expect("sim");
    let mut messages = Vec::new();
    for t in &gm.ticks {
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
        for op in sim.take_hud_ops() {
            if let HudOp::EventText { index } = op {
                messages.push((t.tick, index));
            }
        }
        if t.tick >= 2500 {
            break;
        }
    }
    assert!(
        messages.iter().any(|&(tick, _)| (2370..=2420).contains(&tick)),
        "no event message published before tick 2420 (shown for 100 ticks): {messages:?}"
    );
}
