//! Sounds `tyrian.snd`, `voices.snd`, `voicesc.snd` (`nortsong.c:loadSndFile`):
//! offset container, signed 8-bit mono PCM at 11,025 Hz without a header.

use crate::container::OffsetContainer;
use crate::io::Result;

pub const SAMPLE_RATE: u32 = 11025;
/// `SFX_COUNT`.
pub const SFX_COUNT: usize = 31;
/// `VOICE_COUNT`.
pub const VOICE_COUNT: usize = 9;
/// Bytes ignored at the end of each voice ("bad data", nortsong.c).
pub const VOICE_TRAILING_BAD_BYTES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundFile {
    pub count: u16,
    /// Raw samples (signed bytes), including the bytes ignored by the C code.
    pub samples: Vec<Vec<u8>>,
}

impl SoundFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let c = OffsetContainer::parse(data)?;
        Ok(Self { count: c.count, samples: c.entries })
    }

    pub fn write(&self) -> Vec<u8> {
        OffsetContainer { count: self.count, entries: self.samples.clone() }.write()
    }

    /// Sample as played by the C code (voices: last 100 bytes removed).
    pub fn played_sample(&self, index: usize, is_voice: bool) -> Option<&[u8]> {
        let s = self.samples.get(index)?;
        if is_voice {
            Some(&s[..s.len().saturating_sub(VOICE_TRAILING_BAD_BYTES)])
        } else {
            Some(&s[..s.len().min(65535)])
        }
    }
}

/// Signed 8-bit PCM → unsigned 8-bit mono PCM WAV file.
pub fn pcm_s8_to_wav(samples: &[u8], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() as u32;
    let mut out = Vec::with_capacity(44 + samples.len() + 1);
    out.extend_from_slice(b"RIFF");
    let pad = data_len & 1;
    out.extend_from_slice(&(36 + data_len + pad).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes()); // bytes/s
    out.extend_from_slice(&1u16.to_le_bytes()); // alignment
    out.extend_from_slice(&8u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend(samples.iter().map(|&s| s ^ 0x80));
    if pad == 1 {
        out.push(0);
    }
    out
}
