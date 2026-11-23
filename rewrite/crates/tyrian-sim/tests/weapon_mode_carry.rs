//! Rear weapon mode (`player[].weapon_mode`) from one level to the next.
//!
//! The C sets it to 1 at the start of a game (`mainint.c:JE_initPlayerData`) and
//! never at the start of a level (`tyrian2.c:JE_main`) nor on entering the shop
//! (`game_menu.c:JE_itemScreen`): the mode chosen in the shop ("/" in the rear
//! gun list) or during a level is the one the next level starts with.

use std::path::PathBuf;

use tyrian_sim::shop::ShopSession;
use tyrian_sim::{
    setup, GeneratorId, LevelCarry, Loadout, PlayerCarry, PlayerInput, ShieldId, ShipId, SidekickId, Sim, SpecialId,
    WeaponId,
};

/// Sonic Wave (11): a rear weapon with two modes; Protron Z (15) has one.
const TWO_MODE_REAR: u8 = 11;

fn loadout(rear: u8) -> Loadout {
    Loadout {
        ship: ShipId(1),
        front_weapon: WeaponId(1),
        front_power: 1,
        rear_weapon: WeaponId(rear),
        rear_power: 1,
        shield: ShieldId(4),
        generator: GeneratorId(2),
        left_sidekick: SidekickId(0),
        right_sidekick: SidekickId(0),
        special: SpecialId(0),
        cash: 0,
    }
}

/// TYRIAN (episode 1, level file 9) with the state of the previous level.
fn level(rear: u8, carry: Option<LevelCarry>) -> Option<Sim> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    if !dir.join("tyrian1.lvl").exists() {
        return None;
    }
    let mut setup = setup::load_level(&dir, 1, 3, 9, 2, loadout(rear), 7).unwrap();
    setup.carry_over = carry;
    Some(Sim::new(setup))
}

fn carry_with_mode(mode: u8) -> LevelCarry {
    let carry = LevelCarry {
        players: [PlayerCarry { weapon_mode: mode, ..Default::default() }, PlayerCarry::default()],
        special_fire_held: false,
    };
    // The state goes through the Godot bridge as a list of values.
    LevelCarry::from_values(&carry.to_values()).unwrap()
}

#[test]
fn first_level_starts_in_mode_one() {
    let Some(sim) = level(TWO_MODE_REAR, None) else { return };
    assert_eq!(sim.world().players[0].weapon_mode, 1);
}

#[test]
fn mode_two_is_kept_at_the_next_level_and_handed_on() {
    let Some(mut sim) = level(TWO_MODE_REAR, Some(carry_with_mode(2))) else { return };
    assert_eq!(sim.world().players[0].weapon_mode, 2);
    for _ in 0..30 {
        sim.step([PlayerInput::default(); 2]);
    }
    assert_eq!(sim.world().players[0].weapon_mode, 2, "a weapon with two modes keeps mode 2");
    assert_eq!(LevelCarry::from_sim(&sim).players[0].weapon_mode, 2);
}

#[test]
fn mode_the_weapon_does_not_have_falls_back_to_one() {
    // Protron Z has a single mode: player_movement.c:77.
    let Some(mut sim) = level(15, Some(carry_with_mode(2))) else { return };
    for _ in 0..5 {
        sim.step([PlayerInput::default(); 2]);
    }
    assert_eq!(sim.world().players[0].weapon_mode, 1);
}

#[test]
fn older_carry_lists_start_in_mode_one() {
    // 7 values: lists written before the rear weapon mode was carried.
    let carry = LevelCarry::from_values(&[0, 0, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(carry.players[0].weapon_mode, 0);
    let Some(sim) = level(TWO_MODE_REAR, Some(carry)) else { return };
    assert_eq!(sim.world().players[0].weapon_mode, 1);
}

#[test]
fn shop_starts_with_the_mode_of_the_level() {
    let Some(sim) = level(TWO_MODE_REAR, None) else { return };
    let t = &sim.world().tables;
    let items = sim.world().players[0].items;
    let mut shop = ShopSession::new(t, items, 0, &[], 1);
    assert_eq!(shop.weapon_mode, 1);
    shop.set_weapon_mode(t, 2);
    assert_eq!(shop.weapon_mode, 2);
    shop.set_weapon_mode(t, 3);
    assert_eq!(shop.weapon_mode, 1, "a mode the weapon does not have falls back to 1");
}
