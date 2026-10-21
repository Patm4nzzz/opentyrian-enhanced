//! Music `music.mus` (`loudness.c:load_music`, `lds_play.c:lds_load`):
//! container of 41 tunes in the LOUDNESS (LDS) format.

use crate::container::OffsetContainer;
use crate::io::{FormatError, Reader, Result, Writer};
use serde::Serialize;

/// LDS instrument (46 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LdsPatch {
    pub mod_misc: u8,
    pub mod_vol: u8,
    pub mod_ad: u8,
    pub mod_sr: u8,
    pub mod_wave: u8,
    pub car_misc: u8,
    pub car_vol: u8,
    pub car_ad: u8,
    pub car_sr: u8,
    pub car_wave: u8,
    pub feedback: u8,
    pub keyoff: u8,
    pub portamento: u8,
    pub glide: u8,
    pub finetune: u8,
    pub vibrato: u8,
    pub vibdelay: u8,
    pub mod_trem: u8,
    pub car_trem: u8,
    pub tremwait: u8,
    pub arpeggio: u8,
    pub arp_tab: [u8; 12],
    pub start: u16,
    pub size: u16,
    pub fms: u8,
    pub transp: u16,
    pub midinst: u8,
    pub midvelo: u8,
    pub midkey: u8,
    pub midtrans: u8,
    pub middum1: u8,
    pub middum2: u8,
}

impl LdsPatch {
    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            mod_misc: r.u8()?,
            mod_vol: r.u8()?,
            mod_ad: r.u8()?,
            mod_sr: r.u8()?,
            mod_wave: r.u8()?,
            car_misc: r.u8()?,
            car_vol: r.u8()?,
            car_ad: r.u8()?,
            car_sr: r.u8()?,
            car_wave: r.u8()?,
            feedback: r.u8()?,
            keyoff: r.u8()?,
            portamento: r.u8()?,
            glide: r.u8()?,
            finetune: r.u8()?,
            vibrato: r.u8()?,
            vibdelay: r.u8()?,
            mod_trem: r.u8()?,
            car_trem: r.u8()?,
            tremwait: r.u8()?,
            arpeggio: r.u8()?,
            arp_tab: r.u8_array()?,
            start: r.u16()?,
            size: r.u16()?,
            fms: r.u8()?,
            transp: r.u16()?,
            midinst: r.u8()?,
            midvelo: r.u8()?,
            midkey: r.u8()?,
            midtrans: r.u8()?,
            middum1: r.u8()?,
            middum2: r.u8()?,
        })
    }

    fn write(&self, w: &mut Writer) {
        for b in [
            self.mod_misc,
            self.mod_vol,
            self.mod_ad,
            self.mod_sr,
            self.mod_wave,
            self.car_misc,
            self.car_vol,
            self.car_ad,
            self.car_sr,
            self.car_wave,
            self.feedback,
            self.keyoff,
            self.portamento,
            self.glide,
            self.finetune,
            self.vibrato,
            self.vibdelay,
            self.mod_trem,
            self.car_trem,
            self.tremwait,
            self.arpeggio,
        ] {
            w.u8(b);
        }
        w.bytes(&self.arp_tab);
        w.u16(self.start);
        w.u16(self.size);
        w.u8(self.fms);
        w.u16(self.transp);
        for b in [self.midinst, self.midvelo, self.midkey, self.midtrans, self.middum1, self.middum2] {
            w.u8(b);
        }
    }
}

/// Position: 9 channels × (`patnum` in bytes, `transpose`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LdsChannelPosition {
    /// Offset in bytes in the pattern space (the C code divides by 2).
    pub patnum: u16,
    pub transpose: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LdsSong {
    pub mode: u8,
    pub speed: u16,
    pub tempo: u8,
    pub pattlen: u8,
    pub chandelay: [u8; 9],
    pub regbd: u8,
    pub patches: Vec<LdsPatch>,
    pub positions: Vec<[LdsChannelPosition; 9]>,
    /// "Number of digital sounds" word ignored by the C code (`fseek +2`).
    pub digital_sound_count: u16,
    pub patterns: Vec<u16>,
    /// Possible trailing odd byte (not read by the C code).
    #[serde(skip)]
    pub tail: Vec<u8>,
}

impl LdsSong {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mode = r.u8()?;
        if mode > 2 {
            return Err(FormatError::new(format!("LDS: mode {mode} > 2")));
        }
        let speed = r.u16()?;
        let tempo = r.u8()?;
        let pattlen = r.u8()?;
        let chandelay = r.u8_array()?;
        let regbd = r.u8()?;
        let numpatch = r.u16()?;
        let patches = (0..numpatch).map(|_| LdsPatch::read(&mut r)).collect::<Result<Vec<_>>>()?;
        let numposi = r.u16()?;
        let mut positions = Vec::with_capacity(numposi as usize);
        for _ in 0..numposi {
            let mut p = [LdsChannelPosition { patnum: 0, transpose: 0 }; 9];
            for c in p.iter_mut() {
                c.patnum = r.u16()?;
                c.transpose = r.u8()?;
            }
            positions.push(p);
        }
        let digital_sound_count = r.u16()?;
        let n = r.remaining() / 2;
        let patterns = (0..n).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
        let tail = r.rest();
        Ok(Self {
            mode,
            speed,
            tempo,
            pattlen,
            chandelay,
            regbd,
            patches,
            positions,
            digital_sound_count,
            patterns,
            tail,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u8(self.mode);
        w.u16(self.speed);
        w.u8(self.tempo);
        w.u8(self.pattlen);
        w.bytes(&self.chandelay);
        w.u8(self.regbd);
        w.u16(self.patches.len() as u16);
        self.patches.iter().for_each(|p| p.write(&mut w));
        w.u16(self.positions.len() as u16);
        for p in &self.positions {
            for c in p {
                w.u16(c.patnum);
                w.u8(c.transpose);
            }
        }
        w.u16(self.digital_sound_count);
        w.u16_slice(&self.patterns);
        w.bytes(&self.tail);
        w.into_inner()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicFile {
    pub count: u16,
    pub songs: Vec<LdsSong>,
}

impl MusicFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let c = OffsetContainer::parse(data)?;
        let songs = c
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| LdsSong::parse(e).map_err(|err| FormatError::new(format!("song {i}: {err}"))))
            .collect::<Result<_>>()?;
        Ok(Self { count: c.count, songs })
    }

    pub fn write(&self) -> Vec<u8> {
        OffsetContainer { count: self.count, entries: self.songs.iter().map(|s| s.write()).collect() }.write()
    }
}
