//! Round 4, Endless path: frozen starting difficulty (endless.c:800-801) and player 2's
//! sidekick level carried over from wave to wave (endless.c:849).

use std::path::PathBuf;
use tyrian_sim::endless::{mode_setup, start_loadout, start_loadouts_two_player, START_DIFFICULTY};
use tyrian_sim::{setup::load_level, LevelCarry, PlayerCarry, Sim};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn has_data() -> bool {
    data_dir().join("tyrian1.lvl").exists()
}

/// The current difficulty goes up, the starting difficulty (Hard / Impossible, Event 66) stays.
#[test]
fn one_player_endless_keeps_start_difficulty_when_ramp_rises() {
    if !has_data() {
        return;
    }
    let mut setup = load_level(&data_dir(), 1, 2, 9, 5, start_loadout(), 7).unwrap();
    setup.modes = mode_setup(false, [3, 3]);
    let sim = Sim::new(setup);
    assert_eq!(sim.world().difficulty, 5, "current difficulty = the wave's difficulty");
    assert_eq!(sim.world().initial_difficulty, START_DIFFICULTY as i8, "starting difficulty frozen");
}

/// Player 2: the sidekick level acquired during a wave is picked up again in the next wave.
#[test]
fn two_player_endless_keeps_player_two_sidekick_level() {
    if !has_data() {
        return;
    }
    let mut setup = load_level(&data_dir(), 1, 2, 9, 3, start_loadout(), 7).unwrap();
    setup.modes = mode_setup(true, [3, 3]);
    let first = Sim::new(setup.clone());
    assert_eq!(first.world().players[1].items.sidekick_level, 0, "run start: 0");
    let mut next = setup;
    let mut carry = LevelCarry::default();
    carry.players[1] = PlayerCarry { sidekick_level: 102, sidekick_series: 3, ..Default::default() };
    next.carry_over = Some(carry);
    let second = Sim::new(next);
    assert_eq!(second.world().players[1].items.sidekick_level, 102);
    assert_eq!(second.world().players[1].items.sidekick_series, 3);
    let _ = start_loadouts_two_player();
}
