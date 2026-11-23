//! Two-player game: what player 2 keeps from one level to the next.
//!
//! The C sets `player[1].items` once, at the start of the game
//! (`mainint.c:JE_initPlayerData`: sidekick level 101, series 0) and never between two
//! levels: a sidekick picked up twice in a row is upgraded even across a level change.

use std::path::PathBuf;

use tyrian_sim::{
    setup, GeneratorId, LevelCarry, Loadout, PlayerCarry, ShieldId, ShipId, SidekickId, Sim, SpecialId, WeaponId,
};

fn loadout(ship: u8) -> Loadout {
    Loadout {
        ship: ShipId(ship),
        front_weapon: WeaponId(1),
        front_power: 3,
        rear_weapon: WeaponId(15),
        rear_power: 3,
        shield: ShieldId(4),
        generator: GeneratorId(2),
        left_sidekick: SidekickId(0),
        right_sidekick: SidekickId(0),
        special: SpecialId(0),
        cash: 0,
    }
}

/// TYRIAN (episode 1, level file 9) for two players, with the state of the previous level.
fn two_player_level(carry: Option<LevelCarry>) -> Option<Sim> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    if !dir.join("tyrian1.lvl").exists() {
        return None;
    }
    let mut setup = setup::load_level(&dir, 1, 3, 9, 3, loadout(11), 7).unwrap();
    setup.modes.two_player = true;
    setup.modes.loadout2 = Some(loadout(11));
    setup.modes.devices = (1, 2);
    setup.second_player_loadout = Some(loadout(1));
    setup.carry_over = carry;
    Some(Sim::new(setup))
}

#[test]
fn first_level_starts_player_two_at_sidekick_level_101() {
    let Some(sim) = two_player_level(None) else { return };
    let items = sim.world().players[1].items;
    assert_eq!((items.sidekick_level, items.sidekick_series), (101, 0));
}

#[test]
fn player_two_keeps_sidekick_series_and_level_across_levels() {
    let carry = LevelCarry {
        players: [
            PlayerCarry::default(),
            PlayerCarry { sidekick_level: 102, sidekick_series: 7, ..Default::default() },
        ],
        special_fire_held: false,
    };
    // The state goes through the Godot bridge as a list of values.
    let carry = LevelCarry::from_values(&carry.to_values()).unwrap();
    let Some(sim) = two_player_level(Some(carry)) else { return };
    let items = sim.world().players[1].items;
    assert_eq!((items.sidekick_level, items.sidekick_series), (102, 7));

    // And the level hands the same values over to the next one.
    let next = LevelCarry::from_sim(&sim);
    assert_eq!((next.players[1].sidekick_level, next.players[1].sidekick_series), (102, 7));
}

#[test]
fn carry_without_sidekick_state_keeps_the_start_values() {
    // First level of a game, or a capture older than these two values.
    let Some(sim) = two_player_level(Some(LevelCarry::default())) else { return };
    let items = sim.world().players[1].items;
    assert_eq!((items.sidekick_level, items.sidekick_series), (101, 0));
}
