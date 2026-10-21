//! Full-screen images: `tyrian.pic` (`picload.c:JE_loadPic`) and standard
//! PCX `tshp2.pcx` / `shipedit.pcx` (`pcxload.c:JE_loadPCX`).

use crate::container::OffsetContainer;
use crate::io::{FormatError, Result};
use crate::palette::PCX_PALETTE;

pub const PIC_WIDTH: usize = 320;
pub const PIC_HEIGHT: usize = 200;
pub const PIC_PIXELS: usize = PIC_WIDTH * PIC_HEIGHT;

/// PCX RLE decoding as in the C code: stops as soon as 64,000 pixels have been
/// produced (a final repeat may overflow; the surplus is ignored).
/// Returns the pixels and the number of bytes consumed.
pub fn decode_rle(data: &[u8]) -> Result<(Vec<u8>, usize)> {
    let mut out = Vec::with_capacity(PIC_PIXELS + 64);
    let mut p = 0;
    while out.len() < PIC_PIXELS {
        let b = *data.get(p).ok_or_else(|| FormatError::new("PCX RLE: end of data"))?;
        if b & 0xC0 == 0xC0 {
            let v = *data.get(p + 1).ok_or_else(|| FormatError::new("PCX RLE: end of data"))?;
            out.extend(std::iter::repeat_n(v, (b & 0x3F) as usize));
            p += 2;
        } else {
            out.push(b);
            p += 1;
        }
    }
    out.truncate(PIC_PIXELS);
    Ok((out, p))
}

/// `tyrian.pic`: 14 RLE images of 320×200 without a header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PicFile {
    /// Header `u16` (read but ignored by the C code; 14).
    pub count: u16,
    /// Raw entries (RLE + unread trailing byte(s)).
    pub entries: Vec<Vec<u8>>,
}

impl PicFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let c = OffsetContainer::parse(data)?;
        Ok(Self { count: c.count, entries: c.entries })
    }

    pub fn write(&self) -> Vec<u8> {
        OffsetContainer { count: self.count, entries: self.entries.clone() }.write()
    }

    /// Image `index` (0-based): 320×200 pixels and number of unconsumed bytes.
    pub fn decode(&self, index: usize) -> Result<(Vec<u8>, usize)> {
        let e = self.entries.get(index).ok_or_else(|| FormatError::new("tyrian.pic: index outside the table"))?;
        let (px, used) = decode_rle(e)?;
        Ok((px, e.len() - used))
    }

    /// Palette of image `index` (0-based): `palettes[pcxpal[index]]`.
    pub fn palette_index(index: usize) -> Option<usize> {
        PCX_PALETTE.get(index).map(|&p| p as usize)
    }
}

/// ZSoft 8 bpp PCX file read like the C code: 128-byte header, RLE, optional
/// 8-bit palette (marker 12 at `size - 769`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcxFile {
    pub raw: Vec<u8>,
}

impl PcxFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 128 + 769 {
            return Err(FormatError::new("PCX too short"));
        }
        Ok(Self { raw: data.to_vec() })
    }

    pub fn write(&self) -> Vec<u8> {
        self.raw.clone()
    }

    pub fn header(&self) -> &[u8] {
        &self.raw[..128]
    }

    /// 8-bit palette (not rescaled) if marker 12 is present.
    pub fn palette_rgb8(&self) -> Option<Vec<[u8; 3]>> {
        let p = self.raw.len() - 769;
        if self.raw[p] != 12 {
            return None;
        }
        Some(self.raw[p + 1..].as_chunks::<3>().0.iter().map(|c| [c[0], c[1], c[2]]).collect())
    }

    pub fn decode(&self) -> Result<Vec<u8>> {
        Ok(decode_rle(&self.raw[128..])?.0)
    }
}
