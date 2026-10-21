//! Parity of triggered sounds: `rewrite/golden/sound/<name>.snd` is the list of sounds that the
//! C code (`loudness.c:JE_flushSoundQueue`) sends to the mixer, one per line "tick channel sample x
//! amplitude", recorded during the replay of the golden master `<name>.gm` (measurement hook in a
//! copy of the C code, `artifacts/round3/leftovers/scripts/c_sound_log.sh`). The same golden master is replayed
//! in `tyrian-sim`: same sounds, same ticks.

use std::path::PathBuf;
use tyrian_golden::GoldenMaster;
use tyrian_sim::PlayerInput;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Sound played: (tick, channel, sample, volume, pan).
type PlayedSound = (u64, u8, u8, u16, u16);

fn rust_sounds(name: &str) -> Option<Vec<PlayedSound>> {
    let data = root().join("data");
    let gm_path = root().join(format!("rewrite/golden/{name}.gm"));
    if !data.join("tyrian.hdt").exists() || !gm_path.exists() {
        eprintln!("data or golden master missing: test skipped");
        return None;
    }
    let gm = GoldenMaster::load(&gm_path).unwrap();
    let mut sim = gm.make_sim(&data).unwrap();
    let mut out = Vec::new();
    for (k, exp) in gm.ticks.iter().enumerate() {
        let script_tail = gm.end_reason == "script" && k + 1 == gm.ticks.len() && k > 0;
        if script_tail {
            break;
        }
        sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        for s in sim.take_sounds() {
            out.push((exp.tick, s.channel, s.sample, s.x, s.dmg));
        }
    }
    Some(out)
}

fn c_sounds(name: &str) -> Vec<(u64, u8, u8, u16, u16)> {
    let text = std::fs::read_to_string(root().join(format!("rewrite/golden/sound/{name}.snd"))).unwrap();
    text.lines()
        .map(|l| {
            let v: Vec<u64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            (v[0], v[1] as u8, v[2] as u8, v[3] as u16, v[4] as u16)
        })
        .collect()
}

/// Channel 3 = scripted events (`level_events.c:voice_dedup_allow`): the C code limits each sound to one
/// every 3 s of wall-clock time (`SDL_GetTicks`), the simulation to 3 s of ticks. C recording
/// at full speed: the wall clock advances little, so the C code sometimes holds back a sound that the
/// simulation plays (intended difference, reproducible tick count). Rule: outside channel 3,
/// strict identity; on channel 3, every C sound appears in the simulation, at the same tick.
fn check(name: &str) {
    let Some(rust) = rust_sounds(name) else { return };
    let c = c_sounds(name);
    let strict = |v: &[(u64, u8, u8, u16, u16)]| v.iter().copied().filter(|s| s.1 != 3).collect::<Vec<_>>();
    let (rs, cs) = (strict(&rust), strict(&c));
    let first_diff = rs.iter().zip(cs.iter()).position(|(a, b)| a != b);
    assert!(
        first_diff.is_none() && rs.len() == cs.len(),
        "{name}: {} sounds on the Rust side, {} on the C side (excluding channel 3); first mismatch {:?} (Rust {:?}, C {:?})",
        rs.len(),
        cs.len(),
        first_diff,
        first_diff.map(|i| rs[i]),
        first_diff.map(|i| cs[i]),
    );
    for s in c.iter().filter(|s| s.1 == 3) {
        assert!(rust.contains(s), "{name}: C sound missing from the simulation: {s:?}");
    }
}

#[test]
fn sounds_match_c_on_e1_tyrian_script() {
    check("e1-tyrian-script");
}

#[test]
fn sounds_match_c_on_e1_tyrian_autopilot() {
    check("e1-tyrian-autopilot");
}

#[test]
fn sounds_match_c_on_e1_tyrian_hard_autopilot() {
    check("e1-tyrian-hard-autopilot");
}

#[test]
fn sounds_match_c_on_e2_first_autopilot() {
    check("e2-first-autopilot");
}

#[test]
fn sounds_match_c_on_e1_asteroid1_autopilot() {
    check("e1-asteroid1-autopilot");
}

#[test]
fn sounds_match_c_on_e4_first_autopilot() {
    check("e4-first-autopilot");
}
