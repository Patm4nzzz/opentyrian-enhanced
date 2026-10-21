//! Development keys (`dev_console.c`, `--devmode`): each effect
//! acts on the simulation as in the C code, and a simulation that does not
//! use them stays identical tick for tick (golden master parity).

use std::path::PathBuf;
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

fn make() -> Option<Sim> {
    if !data_dir().join("tyrian1.lvl").exists() {
        eprintln!("data/ missing: test skipped");
        return None;
    }
    Some(Sim::new(load_level(&data_dir(), 1, 2, 9, 2, loadout(), 7).unwrap()))
}

#[test]
fn unused_dev_api_changes_nothing() {
    let (Some(mut a), Some(mut b)) = (make(), make()) else {
        return;
    };
    // Round-trip toggle before the first tick: no effect.
    b.dev_toggle_constant_play();
    b.dev_toggle_constant_play();
    for t in 0..600u64 {
        let i = PlayerInput(if t % 50 < 25 { PlayerInput::LEFT | PlayerInput::FIRE } else { PlayerInput::RIGHT });
        a.step([i, PlayerInput(0)]);
        b.step([i, PlayerInput(0)]);
        assert_eq!(a.state_hash(), b.state_hash(), "tick {t}");
    }
}

#[test]
fn restore_armor_and_add_cash() {
    let Some(mut s) = make() else { return };
    s.step([PlayerInput(0), PlayerInput(0)]);
    let cash = s.world().players[0].cash;
    s.dev_add_cash(10000);
    s.dev_restore_armor();
    // Effects played at the next tick, in place of dev_console_poll.
    assert_eq!(s.world().players[0].cash, cash);
    s.step([PlayerInput(0), PlayerInput(0)]);
    assert_eq!(s.world().players[0].cash, cash + 10000);
    assert_eq!(s.world().players[0].armor, 28);
}

#[test]
fn complete_level_ends_the_level_as_won() {
    let Some(mut s) = make() else { return };
    for _ in 0..10 {
        s.step([PlayerInput(0), PlayerInput(0)]);
    }
    s.dev_complete_level();
    s.step([PlayerInput(0), PlayerInput(0)]);
    assert!(s.world().ending.end_level);
    assert_eq!(s.world().script.level_timer_countdown, 0);
    let mut n = 0;
    while !s.state().level_ended && n < 2000 {
        s.step([PlayerInput(0), PlayerInput(0)]);
        n += 1;
    }
    assert!(s.state().level_ended, "level not ended after Ctrl+Shift+N");
    assert!(s.world().players[0].is_alive, "level won: player alive");
}

#[test]
fn god_mode_fires_and_drifts() {
    let Some(mut s) = make() else { return };
    for _ in 0..5 {
        s.step([PlayerInput(0), PlayerInput(0)]);
    }
    assert!(s.dev_toggle_constant_play());
    let x0 = s.world().players[0].x;
    // constantPlay: all buttons held, x += constantLastX (-1) per tick.
    for _ in 0..10 {
        s.step([PlayerInput(0), PlayerInput(0)]);
    }
    let x1 = s.world().players[0].x;
    assert!(x1 < x0, "leftward drift expected ({x0} -> {x1})");
    assert!(s.world().shot_avail.iter().any(|&a| a != 0), "auto-fire expected");
    assert!(!s.dev_toggle_constant_play());
}
