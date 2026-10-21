//! Title demo (`demo.1` … `demo.5`): `mainint.c:load_next_demo`
//! (header: episode, level, equipment, music) and
//! `mainint.c:replay_demo_keys` (player 1 keys, tick by tick).
//!
//! The C code injects the demo keys in `player_movement.c:JE_playerMovement`,
//! in place of the keyboard (`inputDevice = 1`, keyboard ignored during
//! `play_demo`), only when the player is alive and `endLevel` is
//! false; file exhausted: `endLevel = true; levelEnd = 40`. [`DemoInput`]
//! carries this cursor in `World` ([`crate::World::demo`]).

use crate::{GeneratorId, Loadout, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};
use tyrian_formats::demo::{Demo, DemoKey};

/// `opentyr.h`: `DIFFICULTY_NORMAL` (`load_next_demo`: `difficultyLevel = DIFFICULTY_NORMAL`).
pub const DEMO_DIFFICULTY: u8 = 2;

/// Cursor of `replay_demo_keys`: `demo_keys` and `demo_keys_wait`, then the
/// remaining complete triplets. A final incomplete triplet (`Demo::tail`) is
/// never played: reading it reaches the end of the file (`feof`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoInput {
    keys: Vec<DemoKey>,
    pos: usize,
    current: u8,
    wait: u16,
    exhausted: bool,
}

impl DemoInput {
    /// `load_next_demo`: `demo_keys = 0`, `demo_keys_wait` = initial delay.
    pub fn new(demo: &Demo) -> Self {
        Self { keys: demo.keys.clone(), pos: 0, current: 0, wait: demo.initial_wait, exhausted: false }
    }

    /// `replay_demo_keys`: keys of the tick (bits 0 up, 1 down, 2 left,
    /// 3 right, 4 fire, 5 fire mode change, 6 / 7 sidekicks — the core's
    /// input mask), or `None` when the file is exhausted.
    pub fn next_keys(&mut self) -> Option<u8> {
        while self.wait == 0 {
            // "demo_keys = 0; fread_u8(&demo_keys, 1, …); fread_u8(wait_bytes, 2, …);
            //   if (feof(demo_file)) return false;"
            let Some(k) = self.keys.get(self.pos) else {
                self.exhausted = true;
                return None;
            };
            self.pos += 1;
            self.current = k.keys;
            self.wait = k.wait;
        }
        self.wait -= 1;
        Some(self.current)
    }

    /// The file has been read to the end (`replay_demo_keys` returned false).
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// Triplets already read.
    pub fn position(&self) -> usize {
        self.pos
    }
}

/// Equipment of `player[0]` read by `load_next_demo` (power levels included;
/// `sidekick_level`, `sidekick_series` and `super_arcade_mode` are not used
/// in single player). `cash`: money of `player[0]`, which the demo does not touch.
pub fn demo_loadout(demo: &Demo, cash: u32) -> Loadout {
    let h = &demo.header;
    Loadout {
        ship: ShipId(h.ship),
        front_weapon: WeaponId(h.front_weapon),
        front_power: h.front_power,
        rear_weapon: WeaponId(h.rear_weapon),
        rear_power: h.rear_power,
        shield: ShieldId(h.shield),
        generator: GeneratorId(h.generator),
        left_sidekick: SidekickId(h.left_sidekick),
        right_sidekick: SidekickId(h.right_sidekick),
        special: SpecialId(h.special),
        cash,
    }
}

/// Level name from the header (`levelName`, 10 bytes, ends at the first NUL),
/// without the padding spaces.
pub fn demo_level_name(demo: &Demo) -> String {
    let n = &demo.header.level_name;
    let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
    String::from_utf8_lossy(&n[..end]).trim_end().to_string()
}

/// C state kept from one game to the next and read by the demo: money of
/// `player[0]`, `initialDifficulty` (0 at program start) and the
/// state carried from level to level (see [`crate::LevelCarry`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DemoContext {
    pub cash: u32,
    pub initial_difficulty: u8,
    pub carry: Option<crate::LevelCarry>,
}

/// `JE_main` with `play_demo`: `JE_loadMap` → `load_next_demo`, level
/// `lvlFileNum` of the header's episode at Normal difficulty, then
/// `level_loop` with the demo keys.
pub fn start_demo(data_dir: &std::path::Path, demo: &Demo, seed: u64, ctx: DemoContext) -> Result<crate::Sim, String> {
    let ep = demo.header.episode.clamp(1, 5);
    let mut setup = crate::setup::load_level(
        data_dir,
        ep,
        0,
        demo.header.lvl_file_num.max(1) as usize,
        DEMO_DIFFICULTY,
        demo_loadout(demo, ctx.cash),
        seed,
    )?;
    setup.initial_difficulty = Some(ctx.initial_difficulty);
    setup.carry_over = ctx.carry;
    let mut sim = crate::Sim::new(setup);
    sim.set_demo(DemoInput::new(demo));
    Ok(sim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tyrian_formats::demo::{DemoHeader, DEMO_HEADER_SIZE};

    fn demo(initial_wait: u16, keys: &[(u8, u16)], tail: &[u8]) -> Demo {
        let mut bytes = vec![0u8; DEMO_HEADER_SIZE];
        bytes[0] = 1;
        bytes.extend_from_slice(&initial_wait.to_be_bytes());
        for &(k, w) in keys {
            bytes.push(k);
            bytes.extend_from_slice(&w.to_be_bytes());
        }
        bytes.extend_from_slice(tail);
        Demo::parse(&bytes).unwrap()
    }

    fn drain(d: &Demo) -> Vec<u8> {
        let mut c = DemoInput::new(d);
        let mut out = Vec::new();
        while let Some(k) = c.next_keys() {
            out.push(k);
            assert!(out.len() < 100_000);
        }
        assert!(c.exhausted());
        out
    }

    #[test]
    fn initial_wait_plays_no_key_then_each_triplet_for_its_wait() {
        let d = demo(2, &[(0x11, 3), (0x04, 1)], &[]);
        assert_eq!(drain(&d), vec![0, 0, 0x11, 0x11, 0x11, 0x04]);
    }

    #[test]
    fn zero_waits_are_skipped_and_incomplete_tail_is_never_played() {
        // replay_demo_keys keeps reading while demo_keys_wait is 0; a truncated
        // triplet (1 or 2 bytes) hits feof and ends the demo.
        let d = demo(0, &[(0x01, 0), (0x02, 2)], &[0x08]);
        assert_eq!(d.tail, vec![0x08]);
        assert_eq!(drain(&d), vec![0x02, 0x02]);
        let d = demo(1, &[], &[0x08, 0x00]);
        assert_eq!(drain(&d), vec![0]);
    }

    #[test]
    fn exhausted_cursor_stays_exhausted() {
        let d = demo(1, &[], &[]);
        let mut c = DemoInput::new(&d);
        assert_eq!(c.next_keys(), Some(0));
        assert_eq!(c.next_keys(), None);
        assert_eq!(c.next_keys(), None);
        assert!(c.exhausted());
    }

    #[test]
    fn header_gives_level_and_loadout() {
        let mut h = DemoHeader {
            episode: 1,
            level_name: *b"SAVARA   \0",
            lvl_file_num: 12,
            front_weapon: 5,
            rear_weapon: 29,
            super_arcade_mode: 0,
            left_sidekick: 21,
            right_sidekick: 21,
            generator: 5,
            sidekick_level: 2,
            sidekick_series: 1,
            initial_episode_num: 1,
            shield: 8,
            special: 2,
            ship: 6,
            front_power: 10,
            rear_power: 11,
            unused: [0; 3],
            level_song: 33,
        };
        let d = Demo { header: h.clone(), initial_wait: 0, keys: vec![], tail: vec![] };
        assert_eq!(demo_level_name(&d), "SAVARA");
        let l = demo_loadout(&d, 4527);
        assert_eq!(
            (l.ship, l.front_weapon, l.front_power, l.rear_weapon, l.rear_power),
            (ShipId(6), WeaponId(5), 10, WeaponId(29), 11)
        );
        assert_eq!(
            (l.shield, l.generator, l.left_sidekick, l.right_sidekick, l.special, l.cash),
            (ShieldId(8), GeneratorId(5), SidekickId(21), SidekickId(21), SpecialId(2), 4527)
        );
        h.level_name = *b"0123456789";
        let d = Demo { header: h, initial_wait: 0, keys: vec![], tail: vec![] };
        assert_eq!(demo_level_name(&d), "0123456789");
    }
}
