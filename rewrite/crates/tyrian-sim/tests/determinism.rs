//! Deterministic properties of the core: same seed and inputs ⇒ same state;
//! fixed-size snapshot; consistent Godot views.

use std::path::PathBuf;
use tyrian_sim::{setup::load_level, Loadout, PlayerInput, Sim};
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn loadout() -> Loadout {
    // mainint.c:JE_initPlayerData + opentyr.c:apply_goto_spec (episode 1).
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

fn make(seed: u64) -> Option<Sim> {
    if !data_dir().join("tyrian1.lvl").exists() {
        eprintln!("data/ missing: test skipped");
        return None;
    }
    Some(Sim::new(load_level(&data_dir(), 1, 2, 9, 2, loadout(), seed).unwrap()))
}

fn script(t: u64) -> PlayerInput {
    // Left/right sweep with fire, deterministic.
    let dir = if (t / 90).is_multiple_of(2) { PlayerInput::LEFT } else { PlayerInput::RIGHT };
    PlayerInput(PlayerInput::FIRE | dir | if t % 200 < 40 { PlayerInput::UP } else { 0 })
}

#[test]
fn same_seed_and_inputs_give_same_state() {
    let (Some(mut a), Some(mut b)) = (make(42), make(42)) else {
        return;
    };
    for t in 0..1500 {
        a.step([script(t), PlayerInput(0)]);
        b.step([script(t), PlayerInput(0)]);
        assert_eq!(a.state_hash(), b.state_hash(), "tick {t}");
        assert_eq!(a.frame_record(), b.frame_record(), "tick {t}");
    }
}

#[test]
fn different_seed_changes_rng_state() {
    let (Some(mut a), Some(mut b)) = (make(1), make(2)) else {
        return;
    };
    for t in 0..10 {
        a.step([script(t), PlayerInput(0)]);
        b.step([script(t), PlayerInput(0)]);
    }
    assert_ne!(a.frame_record().h_rng, b.frame_record().h_rng);
}

#[test]
fn snapshot_length_is_stable() {
    let Some(mut a) = make(7) else { return };
    let len = a.snapshot().len();
    for t in 0..2000 {
        a.step([script(t), PlayerInput(0)]);
        assert_eq!(a.snapshot().len(), len, "tick {t}");
    }
}

#[test]
fn views_are_filled() {
    let Some(mut a) = make(7) else { return };
    let mut saw_enemy = false;
    let mut saw_shot = false;
    for t in 0..1500 {
        a.step([script(t), PlayerInput(0)]);
        let s = a.state();
        if s.level_ended {
            // Ship destroyed without aids: level end (tyrian2.c, allPlayersGone).
            break;
        }
        assert_eq!(s.tick, t + 1);
        for e in &s.enemies {
            saw_enemy = true;
            assert!(e.size == 1 || e.size == 2);
            assert!(e.layer <= 3);
        }
        saw_shot |= !s.player_shots.is_empty();
        // Layer 1: y_px advances with scrolling, stays within the map.
        for l in &s.layers {
            assert!(l.y_px >= -28 && l.y_px < 600 * 28, "{:?}", l);
        }
    }
    assert!(saw_enemy, "no enemy seen");
    assert!(saw_shot, "no shot seen");
}

/// Analog stick (player_movement.c, `joystick[j].analog`): the reduced
/// value is added to mouseXC like the mouse; a stronger tilt
/// moves faster, and two games with the same inputs stay identical.
#[test]
fn analog_stick_moves_proportionally_and_deterministically() {
    let (Some(mut slow), Some(mut fast), Some(mut again)) = (make(7), make(7), make(7)) else {
        return;
    };
    let x0 = slow.world().players[0].x;
    slow.set_analog([(2, 0), (0, 0)]);
    fast.set_analog([(13, 0), (0, 0)]);
    again.set_analog([(13, 0), (0, 0)]);
    for _ in 0..4 {
        slow.step([PlayerInput(0), PlayerInput(0)]);
        fast.step([PlayerInput(0), PlayerInput(0)]);
        again.step([PlayerInput(0), PlayerInput(0)]);
        assert_eq!(fast.state_hash(), again.state_hash());
    }
    let (ds, df) = (slow.world().players[0].x - x0, fast.world().players[0].x - x0);
    assert!(ds > 0 && df > ds, "slow {ds}, fast {df}");
    // Stick released: no more analog thrust.
    fast.set_analog([(0, 0), (0, 0)]);
    let before = fast.world().players[0].x;
    for _ in 0..20 {
        fast.step([PlayerInput(0), PlayerInput(0)]);
    }
    assert!(fast.world().players[0].x - before <= 8, "inertia only after release");
}
