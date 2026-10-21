//! A lone player reads the chosen device (`mainint.c:3365-3370` at tag c-final:
//! `inputDevice[0]`, and no longer a hard-coded 0; 0 = all devices, 1 keyboard,
//! 2 mouse). With no choice, the one-player game takes "all": the mouse steers.

use std::path::PathBuf;
use tyrian_sim::modes::MouseInput;
use tyrian_sim::{
    setup::load_level, GeneratorId, Loadout, PlayerInput, ShieldId, ShipId, SidekickId, Sim, SpecialId, WeaponId,
};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn loadout() -> Loadout {
    Loadout {
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
        cash: 10000,
    }
}

#[test]
fn mouse_moves_ship_without_chosen_device() {
    if !data_dir().join("tyrian1.lvl").exists() {
        eprintln!("data/ missing: test skipped");
        return;
    }
    let mut sim = Sim::new(load_level(&data_dir(), 1, 2, 9, 2, loadout(), 7).unwrap());
    // Lets the ship settle in (level entrance).
    for _ in 0..200 {
        sim.step([PlayerInput(0), PlayerInput(0)]);
    }
    let start = sim.world().players[0].x;
    for _ in 0..30 {
        sim.set_mouse(MouseInput { mask: 0, dx: 8, dy: 0 });
        sim.step([PlayerInput(0), PlayerInput(0)]);
    }
    let after = sim.world().players[0].x;
    assert!(after > start + 20, "the ship must follow the mouse: {start} -> {after}");
}

/// Ship position after 30 mouse ticks (dx 8) then 30 right-arrow ticks,
/// with the given device; returns (mouse movement, keyboard movement).
fn moves_with_device(device: u8, mouse_dx: i32) -> (i32, i32) {
    let mut setup = load_level(&data_dir(), 1, 2, 9, 2, loadout(), 7).unwrap();
    setup.modes.devices = (device, device);
    let mut sim = Sim::new(setup);
    for _ in 0..200 {
        sim.step([PlayerInput(0), PlayerInput(0)]);
    }
    let start = sim.world().players[0].x;
    for _ in 0..30 {
        sim.set_mouse(MouseInput { mask: 0, dx: mouse_dx, dy: 0 });
        sim.step([PlayerInput(0), PlayerInput(0)]);
    }
    let after_mouse = sim.world().players[0].x;
    sim.set_mouse(MouseInput { mask: 0, dx: 0, dy: 0 });
    for _ in 0..30 {
        sim.step([PlayerInput(PlayerInput::LEFT), PlayerInput(0)]);
    }
    (after_mouse - start, sim.world().players[0].x - after_mouse)
}

/// R-09.1: the device choice is honored in single player.
#[test]
fn single_player_honors_chosen_input_device() {
    if !data_dir().join("tyrian1.lvl").exists() {
        eprintln!("data/ missing: test skipped");
        return;
    }
    // The level makes the ship drift even without input: the idle movement is subtracted.
    let (idle, _) = moves_with_device(0, 0);
    let (mouse, key) = moves_with_device(1, 8);
    assert!(mouse == idle && key < 0, "keyboard only: mouse {mouse} (idle {idle}), keyboard {key}");
    let (mouse, key) = moves_with_device(2, 8);
    assert!(mouse > idle + 20 && key == 0, "mouse only: mouse {mouse} (idle {idle}), keyboard {key}");
    let (mouse, key) = moves_with_device(0, 8);
    assert!(mouse > idle + 20 && key < 0, "all: mouse {mouse}, keyboard {key}");
}
