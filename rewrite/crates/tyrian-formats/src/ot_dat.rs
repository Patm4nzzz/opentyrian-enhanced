//! `opentyrian.dat` container (`opentyrian_dat.c:load_container` / `ot_dat_flush`).
//!
//! Little-endian: u32 magic "OT2K" (0x4B32544F), u32 version (1), u32 number
//! of sections; per section: tag[4], u32 flags (bit 0 = signed), u32 size,
//! u32 digest (salted FNV-1a of the payload if signed, 0 otherwise — `ot_dat_flush`),
//! payload; then a u32 salted FNV-1a digest of everything before it.
//!
//! `parse` rejects what `load_container` rejects (magic, version, more than 16
//! sections, container digest). A signed section whose digest is
//! wrong is kept with `valid = false`: the C code empties it and does not rewrite it.
//! The round-trip `write(parse(f)) == f` is exact for a file written by the C code,
//! since `ot_dat_flush` recomputes the digests with the same formula.

use crate::io::{FormatError, Result};

pub const OT_DAT_MAGIC: u32 = 0x4B32_544F;
pub const OT_DAT_VERSION: u32 = 1;
const OT_DAT_SALT: u32 = 0x9E37_79B1;
pub const OT_FLAG_SIGNED: u32 = 1;
const OT_MAX_SECTIONS: u32 = 16;
const OT_MAX_SECTION_SIZE: u32 = 8 * 1024 * 1024;

/// `opentyrian_dat.c:fnv1a`.
pub fn fnv1a(mut h: u32, buf: &[u8]) -> u32 {
    for &b in buf {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn digest(buf: &[u8]) -> u32 {
    fnv1a(OT_DAT_SALT, buf)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatSection {
    pub tag: [u8; 4],
    pub flags: u32,
    pub data: Vec<u8>,
    /// False if the digest of a signed section does not match (the C code discards it).
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OtDat {
    pub sections: Vec<DatSection>,
}

fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

impl OtDat {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let bad = |m: &str| FormatError::new(format!("opentyrian.dat: {m}"));
        if b.len() < 16 {
            return Err(bad("too short"));
        }
        let (magic, version, count) = (u32_at(b, 0), u32_at(b, 4), u32_at(b, 8));
        if magic != OT_DAT_MAGIC || version != OT_DAT_VERSION || count > OT_MAX_SECTIONS {
            return Err(bad("magic/version/section count"));
        }
        let end = b.len() - 4;
        if u32_at(b, end) != fnv1a(OT_DAT_SALT, &b[..end]) {
            return Err(bad("container hash"));
        }
        let mut p = 12;
        let mut sections = Vec::new();
        for _ in 0..count {
            // load_container: truncated header or invalid size → silent stop.
            if end - p < 16 {
                break;
            }
            let tag = [b[p], b[p + 1], b[p + 2], b[p + 3]];
            let (flags, size, dg) = (u32_at(b, p + 4), u32_at(b, p + 8), u32_at(b, p + 12));
            p += 16;
            if size > OT_MAX_SECTION_SIZE || end - p < size as usize {
                break;
            }
            let data = b[p..p + size as usize].to_vec();
            let valid = flags & OT_FLAG_SIGNED == 0 || dg == digest(&data);
            sections.push(DatSection { tag, flags, data: if valid { data } else { Vec::new() }, valid });
            p += size as usize;
        }
        Ok(Self { sections })
    }

    /// `ot_dat_flush`: only valid sections are written.
    pub fn write(&self) -> Vec<u8> {
        let valid: Vec<&DatSection> = self.sections.iter().filter(|s| s.valid).collect();
        let mut out = Vec::new();
        out.extend_from_slice(&OT_DAT_MAGIC.to_le_bytes());
        out.extend_from_slice(&OT_DAT_VERSION.to_le_bytes());
        out.extend_from_slice(&(valid.len() as u32).to_le_bytes());
        for s in valid {
            out.extend_from_slice(&s.tag);
            out.extend_from_slice(&s.flags.to_le_bytes());
            out.extend_from_slice(&(s.data.len() as u32).to_le_bytes());
            let dg = if s.flags & OT_FLAG_SIGNED != 0 { digest(&s.data) } else { 0 };
            out.extend_from_slice(&dg.to_le_bytes());
            out.extend_from_slice(&s.data);
        }
        let c = fnv1a(OT_DAT_SALT, &out);
        out.extend_from_slice(&c.to_le_bytes());
        out
    }

    /// `ot_dat_get_section`: payload of a valid section.
    pub fn section(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        self.sections.iter().find(|s| &s.tag == tag && s.valid).map(|s| s.data.as_slice())
    }

    /// `ot_dat_set_section`: replaces or appends (at the end) the section.
    pub fn set_section(&mut self, tag: &[u8; 4], data: &[u8], signed: bool) {
        let flags = if signed { OT_FLAG_SIGNED } else { 0 };
        if let Some(s) = self.sections.iter_mut().find(|s| &s.tag == tag) {
            s.data = data.to_vec();
            s.flags = flags;
            s.valid = true;
        } else {
            self.sections.push(DatSection { tag: *tag, flags, data: data.to_vec(), valid: true });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let mut d = OtDat::default();
        d.set_section(b"GAME", &[1, 2, 3, 4, 5], true);
        d.set_section(b"JSON", b"{}", false);
        let bytes = d.write();
        let back = OtDat::parse(&bytes).unwrap();
        assert_eq!(back, d);
        assert_eq!(back.write(), bytes);
        // Modified payload, container digest recomputed: section rejected.
        let mut t = bytes.clone();
        t[12 + 16] ^= 0xFF;
        let n = t.len() - 4;
        let c = fnv1a(OT_DAT_SALT, &t[..n]);
        t[n..].copy_from_slice(&c.to_le_bytes());
        let tb = OtDat::parse(&t).unwrap();
        assert!(tb.section(b"GAME").is_none());
        assert_eq!(tb.section(b"JSON"), Some(&b"{}"[..]));
        // Truncated container: rejected.
        assert!(OtDat::parse(&bytes[..bytes.len() - 1]).is_err());
    }
}
