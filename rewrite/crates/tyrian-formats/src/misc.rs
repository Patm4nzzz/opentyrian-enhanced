//! Miscellaneous: `tyrend.anm` (`animlib.c`, Deluxe Paint "LPF") seen
//! structurally (header, palette, page headers) and `exitmsg.bin`
//! (80×25 text screen, unused). Both are kept raw: the
//! RunSkipDump decoding is not implemented here.

use crate::io::{FormatError, Reader, Result};

pub const ANM_PALETTE_OFFSET: usize = 0x100;
pub const ANM_PAGE_HEADER_OFFSET: usize = 0x500;
pub const ANM_PAGES_OFFSET: usize = 0xB00;
pub const ANM_PAGE_SIZE: usize = 0x10000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnmPageHeader {
    pub base_record: u16,
    pub n_records: u16,
    pub n_bytes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnmFile {
    pub raw: Vec<u8>,
    pub nlps: u16,
    pub n_records: u32,
    pub pages: Vec<AnmPageHeader>,
}

impl AnmFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        if r.bytes(4)? != b"LPF " {
            return Err(FormatError::new("tyrend.anm: missing LPF signature"));
        }
        r.u16()?;
        let nlps = r.u16()?;
        let n_records = r.u32()?;
        r.seek(ANM_PAGE_HEADER_OFFSET)?;
        let mut pages = Vec::with_capacity(nlps as usize);
        for _ in 0..nlps {
            pages.push(AnmPageHeader { base_record: r.u16()?, n_records: r.u16()?, n_bytes: r.u16()? });
        }
        Ok(Self { raw: data.to_vec(), nlps, n_records, pages })
    }

    pub fn write(&self) -> Vec<u8> {
        self.raw.clone()
    }

    /// 8-bit palette (stored BGRx in the file).
    pub fn palette_rgb8(&self) -> Vec<[u8; 3]> {
        self.raw[ANM_PALETTE_OFFSET..ANM_PALETTE_OFFSET + 1024]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| [c[2], c[1], c[0]])
            .collect()
    }
}

/// `exitmsg.bin`: 80×25 pairs (character, attribute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextScreen {
    pub cells: Vec<(u8, u8)>,
}

impl TextScreen {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() != 4000 {
            return Err(FormatError::new("exitmsg.bin: size ≠ 4000"));
        }
        Ok(Self { cells: data.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect() })
    }

    pub fn write(&self) -> Vec<u8> {
        self.cells.iter().flat_map(|&(c, a)| [c, a]).collect()
    }
}
