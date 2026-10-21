//! `TyrianSim.set_processor_type` (Setup > Graphics > Visual FX,
//! `config.c:processorType`) really changes the simulation: stars of
//! `backgrnd.c:initialize_starfield` (Classic branch) and layer 2 blocked by
//! water (`tyrian2.c:background_2_allowed`, `smoothies[1] && processorType < 4`).

use std::path::PathBuf;
use tyrian_golden::new_game_loadout;
use tyrian_sim::{PlayerInput, Sim};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load(data: &std::path::Path, processor_type: u8) -> Option<Sim> {
    // Endless arena (SAVARA V, lvlfile 17, difficulty 3): water active early
    // (golden master endless-arcade-seed7).
    let mut setup = tyrian_sim::setup::load_level(data, 1, 38, 17, 3, new_game_loadout(), 7).ok()?;
    setup.processor_type = processor_type;
    Some(Sim::new(setup))
}

#[test]
fn processor_type_changes_starfield_and_layer2() {
    let data = root().join("data");
    if !data.join("tyrian.hdt").exists() {
        eprintln!("data missing: test skipped");
        return;
    }
    // 1. Stars: Classic (pt < 2) has only one layer (backgrnd.c:513-518);
    //    Wild (4) splits 50/30/20.
    let classic = load(&data, 1).unwrap();
    let layers: Vec<u8> = classic.world().render.starfield.iter().map(|s| s.layer).collect();
    assert!(layers.iter().all(|&l| l == 1), "Classic: a single layer (layer 1)");
    let wild = load(&data, 4).unwrap();
    let l4: Vec<u8> = wild.world().render.starfield.iter().map(|s| s.layer).collect();
    assert!(l4.contains(&2), "Wild: near-layer stars (2)");
    assert_ne!(layers, l4, "identical stars between Classic and Wild");

    // 2. Layer 2: water (smoothies[1]) and processorType < 4 block it
    //    (tyrian2.c:1096,1145,1298), the layer advances anyway at 4.
    let mut a = load(&data, 3).unwrap();
    let mut b = load(&data, 4).unwrap();
    for _ in 0..30 {
        a.step([PlayerInput::default(), PlayerInput::default()]);
        b.step([PlayerInput::default(), PlayerInput::default()]);
    }
    let wa = a.world();
    let wb = b.world();
    assert!(wa.background.smoothies[1], "level with water expected (smoothies[1])");
    let (a_pos, b_pos) = (wa.background.back_pos[1], wb.background.back_pos[1]);
    assert_ne!(
        (a_pos, wa.background.map_y_pos[1]),
        (b_pos, wb.background.map_y_pos[1]),
        "layer 2 identical at processorType 3 and 4 (expected: capped at 3)"
    );
}

/// `config.c:JE_initProcessorType`: `wild` and `filtrationAvail` only at
/// levels 4 and 6, `explosionTransparent` everywhere except at level 1. At level 3
/// (the maintainer's profile), layer 2 of TIME WAR is therefore opaque and not blended.
#[test]
fn processor_type_derived_flags_match_c() {
    use tyrian_sim::world::{pt_explosion_transparent, pt_filtration_avail, pt_wild};
    let wild: Vec<bool> = (1..=6).map(pt_wild).collect();
    assert_eq!(wild, [false, false, false, true, false, true]);
    let filtration: Vec<bool> = (1..=6).map(pt_filtration_avail).collect();
    assert_eq!(filtration, [false, false, false, true, false, true]);
    let transparent: Vec<bool> = (1..=6).map(pt_explosion_transparent).collect();
    assert_eq!(transparent, [false, true, true, true, true, true]);
}
