//! "Offset table" container (spec § 0.4): `u16 count` + `u32 offset[count]`
//! (absolute offsets) then the blobs. Used by `.lvl`, `.pic`, `tyrian.shp`,
//! `.snd`, `music.mus`.
//!
//! The size of entry `i` is `offset[i+1] - offset[i]`, the last one
//! stops at the end of the file. Writing recomputes the offsets: the
//! round-trip is exact as long as the original offsets are contiguous and
//! increasing from `2 + 4*count`, which `parse` checks.

use crate::io::{FormatError, Reader, Result, Writer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffsetContainer {
    /// Header word (number of entries; ignored by the C code for `tyrian.pic`).
    pub count: u16,
    pub entries: Vec<Vec<u8>>,
}

impl OffsetContainer {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let count = r.u16()?;
        let mut offsets = Vec::with_capacity(count as usize + 1);
        for _ in 0..count {
            offsets.push(r.u32()? as usize);
        }
        offsets.push(data.len());
        let header_end = 2 + 4 * count as usize;
        if count > 0 && offsets[0] != header_end {
            return Err(FormatError::new(format!(
                "container: first offset {} ≠ end of header {header_end}",
                offsets[0]
            )));
        }
        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let (a, b) = (offsets[i], offsets[i + 1]);
            if a > b || b > data.len() {
                return Err(FormatError::new(format!("container: offsets not increasing at entry {i}")));
            }
            entries.push(data[a..b].to_vec());
        }
        Ok(Self { count, entries })
    }

    /// Absolute offsets computed for writing.
    pub fn offsets(&self) -> Vec<u32> {
        let mut pos = 2 + 4 * self.entries.len();
        self.entries
            .iter()
            .map(|e| {
                let p = pos as u32;
                pos += e.len();
                p
            })
            .collect()
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u16(self.count);
        for o in self.offsets() {
            w.u32(o);
        }
        for e in &self.entries {
            w.bytes(e);
        }
        w.into_inner()
    }
}
