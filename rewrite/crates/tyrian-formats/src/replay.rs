//! `.rep` replays of the Enhanced version (`src/replay.c`), versions 1 to 4.
//!
//! Layout (little-endian), based on `replay_stop_recording` / `replay_load`:
//!
//! | field            | size   | since  |
//! |------------------|--------|--------|
//! | `"TYRP"`         | 4      | v1     |
//! | version (u16)    | 2      | v1     |
//! | episode, level, difficulty | 3 | v1 |
//! | seed (u64)       | 8      | v1     |
//! | speed ×10        | 1      | v1     |
//! | game mode        | 1      | v2     |
//! | P1 equipment (`PlayerItems`) | 13 | v2 |
//! | cash (u32)       | 4      | v2     |
//! | mutators         | 8      | v2     |
//! | P2 equipment     | 13     | v3     |
//! | frame count (u32) | 4 | v1   |
//! | frames           | 5 (v1-v3) or 10 (v4) each | |
//!
//! `PlayerItems` (`player.h`) is a sequence of 13 `Uint8` written by raw
//! `fwrite` (the comment in `replay.c` says 12: the `super_arcade_mode`
//! field is forgotten there; a real file has 13).
//!
//! A frame (`replay_capture_frame`): `x`, `y` of player 1 (i16), button
//! mask (bit 0 fire, 1 fire switch, 2 left sidekick, 3 right
//! sidekick), then in v4 the same for player 2.

use crate::io::{FormatError, Reader, Result, Writer};

pub const REPLAY_MAGIC: &[u8; 4] = b"TYRP";
/// `replay.c:REPLAY_VERSION`.
pub const REPLAY_VERSION: u16 = 4;
/// `replay.c:REPLAY_MAX_FRAMES`.
pub const REPLAY_MAX_FRAMES: u32 = 1_500_000;
/// `episodes.h:EPISODE_MAX`.
const EPISODE_MAX: u8 = 5;

/// Bits of a frame's button mask (`replay_capture_frame`).
pub const REPLAY_FIRE: u8 = 0x01;
pub const REPLAY_CHANGE_FIRE: u8 = 0x02;
pub const REPLAY_LEFT_SIDEKICK: u8 = 0x04;
pub const REPLAY_RIGHT_SIDEKICK: u8 = 0x08;

/// `player.h:PlayerItems`, in field order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayItems {
    pub ship: u8,
    pub generator: u8,
    pub shield: u8,
    /// (id, power) front then rear.
    pub weapon: [(u8, u8); 2],
    pub sidekick: [u8; 2],
    pub special: u8,
    pub sidekick_series: u8,
    pub sidekick_level: u8,
    pub super_arcade_mode: u8,
}

pub const PLAYER_ITEMS_SIZE: usize = 13;

impl ReplayItems {
    fn read(r: &mut Reader) -> Result<Self> {
        let b = r.bytes(PLAYER_ITEMS_SIZE)?;
        Ok(Self {
            ship: b[0],
            generator: b[1],
            shield: b[2],
            weapon: [(b[3], b[4]), (b[5], b[6])],
            sidekick: [b[7], b[8]],
            special: b[9],
            sidekick_series: b[10],
            sidekick_level: b[11],
            super_arcade_mode: b[12],
        })
    }

    fn write(&self, w: &mut Writer) {
        w.bytes(&[
            self.ship,
            self.generator,
            self.shield,
            self.weapon[0].0,
            self.weapon[0].1,
            self.weapon[1].0,
            self.weapon[1].1,
            self.sidekick[0],
            self.sidekick[1],
            self.special,
            self.sidekick_series,
            self.sidekick_level,
            self.super_arcade_mode,
        ]);
    }

    /// `replay.c:clamp_player_items`: bounds from `lvlmast.h` (SHIP_NUM 32,
    /// PORT_NUM 80, OPTION_NUM 50, SHIELD_NUM 20, SPECIAL_NUM 70) and `varz.h`
    /// (SA 9). Editor ships 91..100 remain valid.
    pub fn clamp(&mut self) {
        if self.ship < 1 || (self.ship > 32 && !(91..=100).contains(&self.ship)) {
            self.ship = 1;
        }
        if !(1..=6).contains(&self.generator) {
            self.generator = 1;
        }
        if !(1..=20).contains(&self.shield) {
            self.shield = 1;
        }
        if self.special > 70 {
            self.special = 0;
        }
        if !(1..=80).contains(&self.weapon[0].0) {
            self.weapon[0].0 = 1;
        }
        if self.weapon[1].0 > 80 {
            self.weapon[1].0 = 0;
        }
        for s in self.sidekick.iter_mut() {
            if *s > 50 {
                *s = 0;
            }
        }
        if self.sidekick_series >= 16 {
            self.sidekick_series = 0;
        }
        if self.super_arcade_mode > 9 {
            self.super_arcade_mode = 0;
        }
        if !(101..=103).contains(&self.sidekick_level) {
            self.sidekick_level = 101;
        }
    }
}

/// A frame: positions and buttons sampled at the top of the iteration of
/// `tyrian2.c:level_loop`, replayed by `replay_inject_frame`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayFrame {
    pub x: i16,
    pub y: i16,
    pub buttons: u8,
    /// Player 2 (v4 only; zeros in solo).
    pub x2: i16,
    pub y2: i16,
    pub buttons2: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub version: u16,
    pub episode: u8,
    /// `mainLevel` (section of `levelsN.dat`).
    pub level: u8,
    pub difficulty: u8,
    pub seed: u64,
    pub game_speed_x10: u8,
    /// 0 campaign, 1 one-player arcade, 2 two-player arcade.
    pub game_mode: u8,
    pub items: ReplayItems,
    pub cash: u32,
    pub mutators: [u8; 8],
    pub items_p2: ReplayItems,
    pub frames: Vec<ReplayFrame>,
}

impl Replay {
    /// Raw read, without the corrections of `replay_load` (see [`Replay::load`]).
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        if r.bytes(4)? != REPLAY_MAGIC {
            return Err(FormatError::new("replay: missing TYRP signature"));
        }
        let version = r.u16()?;
        if !(1..=REPLAY_VERSION).contains(&version) {
            return Err(FormatError::new(format!("replay: unknown version {version}")));
        }
        let episode = r.u8()?;
        let level = r.u8()?;
        let difficulty = r.u8()?;
        let seed = r.u64()?;
        let game_speed_x10 = r.u8()?;
        let (mut game_mode, mut items, mut cash, mut mutators) = (0, ReplayItems::default(), 10_000, [0u8; 8]);
        if version >= 2 {
            game_mode = r.u8()?;
            items = ReplayItems::read(&mut r)?;
            cash = r.u32()?;
            mutators.copy_from_slice(r.bytes(8)?);
        }
        let items_p2 = if version >= 3 {
            ReplayItems::read(&mut r)?
        } else {
            // replay.c:replay_load: P2 rebuilt like JE_initPlayerData.
            let mut p2 = items;
            p2.weapon[1].0 = 15;
            p2.sidekick_series = 0;
            p2.sidekick_level = 101;
            p2
        };
        let count = r.u32()?;
        if count > REPLAY_MAX_FRAMES {
            return Err(FormatError::new(format!("replay: {count} frames, beyond the C limit")));
        }
        let v4 = version >= 4;
        let mut frames = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let mut f = ReplayFrame { x: r.i16()?, y: r.i16()?, buttons: r.u8()?, ..Default::default() };
            if v4 {
                f.x2 = r.i16()?;
                f.y2 = r.i16()?;
                f.buttons2 = r.u8()?;
            }
            frames.push(f);
        }
        if r.remaining() != 0 {
            return Err(FormatError::new(format!("replay: {} extra bytes", r.remaining())));
        }
        Ok(Self {
            version,
            episode,
            level,
            difficulty,
            seed,
            game_speed_x10,
            game_mode,
            items,
            cash,
            mutators,
            items_p2,
            frames,
        })
    }

    /// `replay.c:replay_load`: read, rejection of out-of-bounds headers and
    /// corrections applied by the C code (difficulty floor 1, equipment
    /// clamped by `clamp_player_items`).
    pub fn load(data: &[u8]) -> Result<Self> {
        let mut rep = Self::parse(data)?;
        if !(1..=EPISODE_MAX).contains(&rep.episode) {
            return Err(FormatError::new(format!("replay: episode {} out of range", rep.episode)));
        }
        if rep.difficulty < 1 {
            rep.difficulty = 1;
        }
        if rep.level < 1 || rep.difficulty > 10 {
            return Err(FormatError::new("replay: level or difficulty out of range"));
        }
        if rep.game_mode > 2 {
            return Err(FormatError::new("replay: unknown game mode"));
        }
        rep.items.clamp();
        rep.items_p2.clamp();
        Ok(rep)
    }

    /// Byte-exact write (same layout as `replay_stop_recording`
    /// for the version carried by `self.version`).
    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.bytes(REPLAY_MAGIC);
        w.u16(self.version);
        w.u8(self.episode);
        w.u8(self.level);
        w.u8(self.difficulty);
        w.u64(self.seed);
        w.u8(self.game_speed_x10);
        if self.version >= 2 {
            w.u8(self.game_mode);
            self.items.write(&mut w);
            w.u32(self.cash);
            w.bytes(&self.mutators);
        }
        if self.version >= 3 {
            self.items_p2.write(&mut w);
        }
        w.u32(self.frames.len() as u32);
        for f in &self.frames {
            w.i16(f.x);
            w.i16(f.y);
            w.u8(f.buttons);
            if self.version >= 4 {
                w.i16(f.x2);
                w.i16(f.y2);
                w.u8(f.buttons2);
            }
        }
        w.into_inner()
    }

    /// `replay.c:level_seed`: level start seed, mix of the header
    /// seed and (episode, level); never 0.
    pub fn level_seed(&self, episode: u8, level: u8) -> u64 {
        let mix = (((episode as u64) << 8) | level as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let s = self.seed ^ mix;
        if s != 0 {
            s
        } else {
            0x5_DEEC_E66D
        }
    }

    /// `replay.c:reseed_for_level`: `mt_srand` seed derived from the
    /// level seed (`unsigned long`, truncated to 32 bits by MT19937).
    pub fn mt_seed(level_seed: u64) -> u32 {
        ((level_seed & 0xFFFF_FFFF) ^ (level_seed >> 32)) as u32
    }
}

/// Header read by `replay.c:replay_read_header` for the title browser:
/// episode, level, difficulty, mode, frame count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayHeader {
    pub episode: u8,
    pub level: u8,
    pub difficulty: u8,
    pub game_mode: u8,
    pub frame_count: u32,
}

/// `replay.c:replay_read_header`: partial read, same rejections (magic,
/// version 1..=REPLAY_VERSION, episode 1..=EPISODE_MAX, level ≥ 1,
/// difficulty ≤ 10, mode ≤ 2, frames ≤ REPLAY_MAX_FRAMES). A file with 0
/// frames is accepted (it is the launch that rejects it).
pub fn read_header(b: &[u8]) -> Option<ReplayHeader> {
    if b.len() < 9 || &b[0..4] != REPLAY_MAGIC {
        return None;
    }
    let version = u16::from_le_bytes([b[4], b[5]]);
    if !(1..=REPLAY_VERSION).contains(&version) {
        return None;
    }
    let (episode, level, difficulty) = (b[6], b[7], b[8]);
    // seed (8) + game_speed_x10 (1)
    let mut o = 9 + 9;
    let mut game_mode = 0;
    if version >= 2 {
        game_mode = *b.get(o)?;
        o += 1 + PLAYER_ITEMS_SIZE + 4 + 8;
        if version >= 3 {
            o += PLAYER_ITEMS_SIZE;
        }
    }
    let fc = b.get(o..o + 4)?;
    let frame_count = u32::from_le_bytes([fc[0], fc[1], fc[2], fc[3]]);
    if !(1..=EPISODE_MAX).contains(&episode)
        || level < 1
        || difficulty > 10
        || game_mode > 2
        || frame_count > REPLAY_MAX_FRAMES
    {
        return None;
    }
    Some(ReplayHeader { episode, level, difficulty, game_mode, frame_count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_v4_and_v3() {
        let mut rep = Replay {
            version: 4,
            episode: 1,
            level: 2,
            difficulty: 2,
            seed: 0xa186_33ae_2435_ba2a,
            game_speed_x10: 40,
            game_mode: 0,
            items: ReplayItems { ship: 1, generator: 2, shield: 4, weapon: [(1, 1), (0, 1)], ..Default::default() },
            cash: 10_000,
            mutators: [0; 8],
            items_p2: ReplayItems::default(),
            frames: vec![ReplayFrame { x: 100, y: 180, buttons: 1, x2: -3, y2: 7, buttons2: 1 }],
        };
        let bytes = rep.write();
        // v4 header: 4 + 2 + 3 + 8 + 1 + 1 + 13 + 4 + 8 + 13 + 4 = 61 bytes.
        assert_eq!(bytes.len(), 61 + 10);
        assert_eq!(Replay::parse(&bytes).unwrap(), rep);
        rep.version = 3;
        rep.frames[0].x2 = 0;
        rep.frames[0].y2 = 0;
        rep.frames[0].buttons2 = 0;
        let bytes = rep.write();
        assert_eq!(bytes.len(), 61 + 5);
        assert_eq!(Replay::parse(&bytes).unwrap(), rep);
    }

    #[test]
    fn load_clamps_like_c() {
        let rep = Replay {
            version: 1,
            episode: 1,
            level: 2,
            difficulty: 0,
            seed: 1,
            game_speed_x10: 40,
            game_mode: 0,
            items: ReplayItems::default(),
            cash: 10_000,
            mutators: [0; 8],
            items_p2: ReplayItems::default(),
            frames: vec![],
        };
        let r = Replay::load(&rep.write()).unwrap();
        assert_eq!(r.difficulty, 1);
        // v1: null equipment -> minimal starting equipment (clamp_player_items).
        assert_eq!((r.items.ship, r.items.generator, r.items.shield, r.items.weapon[0].0), (1, 1, 1, 1));
        assert_eq!(r.items.sidekick_level, 101);
        assert_eq!(r.items_p2.weapon[1].0, 15);
    }

    #[test]
    fn level_seed_never_zero() {
        let mut r = Replay::parse(
            &Replay {
                version: 4,
                episode: 1,
                level: 2,
                difficulty: 2,
                seed: 0,
                game_speed_x10: 40,
                game_mode: 0,
                items: ReplayItems::default(),
                cash: 0,
                mutators: [0; 8],
                items_p2: ReplayItems::default(),
                frames: vec![],
            }
            .write(),
        )
        .unwrap();
        r.seed = ((1u64 << 8) | 2).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        assert_eq!(r.level_seed(1, 2), 0x5_DEEC_E66D);
    }
}
