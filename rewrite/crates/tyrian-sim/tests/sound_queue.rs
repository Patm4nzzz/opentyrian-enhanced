//! Sound effects queue (`varz.c:soundQueue`, `loudness.c:JE_flushSoundQueue`):
//! rendering only. Sounds come out with position and damage magnitude, and
//! draining them or not changes nothing in the simulated state.

use std::path::PathBuf;
use tyrian_sim::sound::{SoundEvent, SOUND_CENTER_X, S_EXPLOSION_8, S_EXPLOSION_9, V_GOOD_LUCK};
use tyrian_sim::{
    setup::load_level, GeneratorId, Loadout, PlayerInput, ShieldId, ShipId, SidekickId, Sim, SpecialId, WeaponId,
};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn make() -> Option<Sim> {
    if !data_dir().join("tyrian1.lvl").exists() {
        eprintln!("data/ missing: test skipped");
        return None;
    }
    let loadout = Loadout {
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
    };
    Some(Sim::new(load_level(&data_dir(), 1, 2, 9, 2, loadout, 42).unwrap()))
}

fn script(t: u64) -> PlayerInput {
    let dir = if (t / 90).is_multiple_of(2) { PlayerInput::LEFT } else { PlayerInput::RIGHT };
    PlayerInput(PlayerInput::FIRE | dir | if t % 200 < 40 { PlayerInput::UP } else { 0 })
}

#[test]
fn sounds_carry_position_and_damage_without_touching_state() {
    let (Some(mut a), Some(mut b)) = (make(), make()) else {
        return;
    };
    let mut all: Vec<SoundEvent> = Vec::new();
    for t in 0..1500 {
        a.step([script(t), PlayerInput(0)]);
        b.step([script(t), PlayerInput(0)]);
        all.extend(a.take_sounds());
        // `b` never drains its queue: same state as `a`.
        assert_eq!(a.state_hash(), b.state_hash(), "tick {t}");
        assert_eq!(a.frame_record(), b.frame_record(), "tick {t}");
    }
    // tyrian2.c: "Good luck" at level start, channel 3, centered.
    assert_eq!(all.first().map(|s| (s.channel, s.sample, s.x)), Some((3, V_GOOD_LUCK, SOUND_CENTER_X)));
    // Player shots: weapon 1 (sound > 0) on channel 0 (soundChannel[SHOT_FRONT]),
    // at the ship's position, which sweeps the screen.
    let shots: Vec<_> = all.iter().filter(|s| s.channel == 0).collect();
    assert!(shots.len() > 50, "shots: {}", shots.len());
    let (min_x, max_x) = shots.iter().fold((u16::MAX, 0), |(lo, hi), s| (lo.min(s.x), hi.max(s.x)));
    assert!(max_x - min_x > 100, "panning follows the ship: {min_x}..{max_x}");
    // Enemies shot down: explosion on channel 6 with the magnitude from collision.c.
    let kills: Vec<_> =
        all.iter().filter(|s| s.channel == 6 && (s.sample == S_EXPLOSION_8 || s.sample == S_EXPLOSION_9)).collect();
    // explosion_update.c (repeating explosions) also writes S_EXPLOSION_9 on
    // channel 6, with no position or magnitude: only the collision.c deaths have one.
    assert!(kills.iter().any(|s| s.dmg > 0 && s.x != SOUND_CENTER_X), "no enemy shot down: {kills:?}");
    assert!(kills.iter().all(|s| if s.sample == S_EXPLOSION_8 { s.dmg == 1 } else { s.dmg != 1 }), "{kills:?}");
    // Outside enemy explosions, no magnitude.
    assert!(all.iter().filter(|s| s.channel != 6 && s.channel != 5).all(|s| s.dmg == 0));
    // The undrained queue stays bounded.
    assert!(b.take_sounds().len() <= 256);
}
