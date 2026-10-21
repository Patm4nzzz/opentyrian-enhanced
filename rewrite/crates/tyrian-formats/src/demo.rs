//! Demos `demo.1` … `demo.5` (`mainint.c:load_next_demo`, `replay_demo_keys`).
//! 30-byte header, initial delay `u16` big-endian, then triplets
//! `(u8 keys, u16 BE delay)` until the end; an incomplete final triplet
//! is kept in `tail`.

use crate::io::{FormatError, Result};
use serde::Serialize;

pub const DEMO_HEADER_SIZE: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DemoHeader {
    pub episode: u8,
    /// 10 raw bytes of `levelName`.
    pub level_name: [u8; 10],
    pub lvl_file_num: u8,
    pub front_weapon: u8,
    pub rear_weapon: u8,
    pub super_arcade_mode: u8,
    pub left_sidekick: u8,
    pub right_sidekick: u8,
    pub generator: u8,
    pub sidekick_level: u8,
    pub sidekick_series: u8,
    pub initial_episode_num: u8,
    pub shield: u8,
    pub special: u8,
    pub ship: u8,
    pub front_power: u8,
    pub rear_power: u8,
    pub unused: [u8; 3],
    pub level_song: u8,
}

/// A keyboard entry: bits 0 up, 1 down, 2 left, 3 right, 4 button0,
/// 5 button3, 6 button1, 7 button2; held for `wait` frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DemoKey {
    pub keys: u8,
    pub wait: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Demo {
    pub header: DemoHeader,
    pub initial_wait: u16,
    pub keys: Vec<DemoKey>,
    #[serde(skip)]
    pub tail: Vec<u8>,
}

impl Demo {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < DEMO_HEADER_SIZE + 2 {
            return Err(FormatError::new("demo too short"));
        }
        let d = data;
        let mut level_name = [0u8; 10];
        level_name.copy_from_slice(&d[1..11]);
        let header = DemoHeader {
            episode: d[0],
            level_name,
            lvl_file_num: d[11],
            front_weapon: d[12],
            rear_weapon: d[13],
            super_arcade_mode: d[14],
            left_sidekick: d[15],
            right_sidekick: d[16],
            generator: d[17],
            sidekick_level: d[18],
            sidekick_series: d[19],
            initial_episode_num: d[20],
            shield: d[21],
            special: d[22],
            ship: d[23],
            front_power: d[24],
            rear_power: d[25],
            unused: [d[26], d[27], d[28]],
            level_song: d[29],
        };
        let initial_wait = u16::from_be_bytes([d[30], d[31]]);
        let body = &d[32..];
        let full = body.len() / 3 * 3;
        let keys = body[..full]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| DemoKey { keys: c[0], wait: u16::from_be_bytes([c[1], c[2]]) })
            .collect();
        Ok(Self { header, initial_wait, keys, tail: body[full..].to_vec() })
    }

    pub fn write(&self) -> Vec<u8> {
        let h = &self.header;
        let mut out = vec![h.episode];
        out.extend_from_slice(&h.level_name);
        out.extend_from_slice(&[
            h.lvl_file_num,
            h.front_weapon,
            h.rear_weapon,
            h.super_arcade_mode,
            h.left_sidekick,
            h.right_sidekick,
            h.generator,
            h.sidekick_level,
            h.sidekick_series,
            h.initial_episode_num,
            h.shield,
            h.special,
            h.ship,
            h.front_power,
            h.rear_power,
        ]);
        out.extend_from_slice(&h.unused);
        out.push(h.level_song);
        out.extend_from_slice(&self.initial_wait.to_be_bytes());
        for k in &self.keys {
            out.push(k.keys);
            out.extend_from_slice(&k.wait.to_be_bytes());
        }
        out.extend_from_slice(&self.tail);
        out
    }
}
