//! Story and episode-end screens (`level_load.c:JE_loadMap`,
//! directives `]A`, `]P 000`; `mainint.c:JE_playCredits`): data read from
//! `data/` for the GDScript scenes that display them.

use godot::prelude::*;
use tyrian_formats::crypt::EncryptedStringFile;
use tyrian_formats::pic::PcxFile;
use tyrian_formats::sprite::SpriteTable;

use crate::vga::{with_assets, TyrianVga};

/// `animlib.c`: offsets of the "LPF" header.
const ANM_PALETTE_OFFSET: usize = 0x100;
const ANM_PAGE_HEADER_OFFSET: usize = 0x500;
const ANM_OFFSET: usize = 0x0B00;
const ANM_PAGE_SIZE: usize = 0x10000;
const SCREEN_PIXELS: usize = 320 * 200;

#[godot_api(secondary)]
impl TyrianVga {
    /// `animlib.c:JE_playAnim` / `JE_loadAnim`: successive frames of
    /// animation `path` (dictionary `palette`: 768 bytes 8-bit RGB,
    /// `frames`: array of 320x200 images). Each frame is the result of the
    /// RunSkipDump delta applied to the previous one, starting from a black screen
    /// (`JE_clr256`), like the C which decodes into VGAScreen. The C does not play
    /// the last record ("delta of the first") and stops at the
    /// first invalid page or frame. Empty dictionary if the file is
    /// unreadable or rejected by the checks of `JE_loadAnim`.
    #[func]
    fn anm_frames(path: GString) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Ok(data) = std::fs::read(path.to_string()) else {
            return d;
        };
        let Some((palette, frames)) = decode_anm(&data) else {
            return d;
        };
        d.set("palette", &PackedByteArray::from(palette.as_slice()));
        let mut arr = VarArray::new();
        for f in &frames {
            arr.push(&PackedByteArray::from(f.as_slice()).to_variant());
        }
        d.set("frames", &arr);
        d
    }

    /// `colors` = given 8-bit palette (768 bytes RGB), without touching the
    /// hardware palette (`set_palette` / `fade_palette_init` afterwards).
    #[func]
    fn set_colors_rgb(&mut self, rgb: PackedByteArray) {
        for (i, c) in rgb.as_slice().as_chunks::<3>().0.iter().take(256).enumerate() {
            self.colors[i] = [c[0], c[1], c[2]];
        }
    }

    /// `pcxload.c:JE_loadPCX` (only `tshp2.pcx`, `]P 000[`): image in
    /// surface `surf` and its 8-bit palette in `colors`.
    #[func]
    fn load_pcx(&mut self, surf: i32, path: GString) -> bool {
        let Ok(data) = std::fs::read(path.to_string()) else {
            return false;
        };
        let Ok(pcx) = PcxFile::parse(&data) else { return false };
        let Ok(px) = pcx.decode() else { return false };
        if let Some(pal) = pcx.palette_rgb8() {
            for (i, c) in pal.iter().take(256).enumerate() {
                self.colors[i] = *c;
            }
        }
        let sf = &mut self.surfaces[surf.clamp(0, 2) as usize];
        let n = px.len().min(sf.pixels.len());
        sf.pixels[..n].copy_from_slice(&px[..n]);
        for i in 0..n {
            sf.forget(i);
        }
        true
    }

    /// `sprite.c:load_sprites_file(table, file)` (`EXTRA_SHAPES` = 7 and
    /// `estsc.shp` for the credits): number of sprites, -1 if unreadable.
    #[func]
    fn load_sprite_table(table: i32, path: GString) -> i32 {
        let Ok(data) = std::fs::read(path.to_string()) else {
            return -1;
        };
        let Ok(t) = SpriteTable::parse(&data) else { return -1 };
        let sprites: Vec<_> = t.entries.into_iter().map(|e| e.sprite).collect();
        let count = sprites.len() as i32;
        let idx = table.clamp(0, 15) as usize;
        with_assets(|a| {
            if a.tables.len() <= idx {
                a.tables.resize(idx + 1, Vec::new());
            }
            a.tables[idx] = sprites;
        })
        .map_or(-1, |_| count)
    }

    /// `mainint.c:JE_playCredits`: the 126 lines of `tyrian.cdt`
    /// (`read_encrypted_pascal_string`), empty if the file is unreadable.
    #[func]
    fn credits_lines(path: GString) -> PackedStringArray {
        let mut out = PackedStringArray::new();
        let Ok(data) = std::fs::read(path.to_string()) else {
            return out;
        };
        let Ok(f) = EncryptedStringFile::parse(&data) else {
            return out;
        };
        for l in f.lines().iter().take(CREDITS_LINES_MAX) {
            out.push(l.as_str());
        }
        out
    }
}

/// `mainint.c:JE_playCredits` : `lines_max`.
const CREDITS_LINES_MAX: usize = 126;

fn u16_at(d: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(p)?, *d.get(p + 1)?]))
}

/// `animlib.c:JE_loadAnim` then the loop of `JE_playAnim`: palette and frames.
fn decode_anm(data: &[u8]) -> Option<(Vec<u8>, Vec<Vec<u8>>)> {
    if data.len() < ANM_OFFSET || data.get(..4)? != b"LPF " {
        return None;
    }
    let nlps = u16_at(data, 6)? as usize;
    let n_records = u32::from_le_bytes(data.get(8..12)?.try_into().ok()?) as usize;
    if nlps == 0 || n_records == 0 || nlps > 256 || n_records > 65535 {
        return None;
    }
    let pages: Vec<(usize, usize)> = (0..nlps)
        .map(|i| {
            Some((
                u16_at(data, ANM_PAGE_HEADER_OFFSET + i * 6)? as usize,
                u16_at(data, ANM_PAGE_HEADER_OFFSET + i * 6 + 2)? as usize,
            ))
        })
        .collect::<Option<_>>()?;
    // Palette stored B, G, R, unused.
    let mut palette = Vec::with_capacity(768);
    for i in 0..256 {
        let c = data.get(ANM_PALETTE_OFFSET + i * 4..ANM_PALETTE_OFFSET + i * 4 + 3)?;
        palette.extend_from_slice(&[c[2], c[1], c[0]]);
    }
    let mut screen = vec![0u8; SCREEN_PIXELS];
    let mut frames = Vec::new();
    for frame in 0..n_records - 1 {
        // JE_findPage.
        let Some(page) = pages.iter().position(|&(base, n)| base <= frame && base + n > frame) else {
            break;
        };
        let Some((base, sizes, buf)) = load_page(data, page) else {
            break;
        };
        let dest = frame - base;
        let Some(&size) = sizes.get(dest) else { break };
        let offset: usize = sizes[..dest].iter().sum();
        let Some(rec) = buf.get(offset + 4..offset + size) else {
            break;
        };
        if run_skip_dump(rec, &mut screen).is_none() {
            break;
        }
        frames.push(screen.clone());
    }
    Some((palette, frames))
}

/// `animlib.c:JE_loadPage`: page header, record sizes,
/// data; rejected if the sum of the sizes differs from `nBytes`.
fn load_page(data: &[u8], page: usize) -> Option<(usize, Vec<usize>, &[u8])> {
    let p = ANM_OFFSET + page * ANM_PAGE_SIZE;
    let base = u16_at(data, p)? as usize;
    let n = u16_at(data, p + 2)? as usize;
    let n_bytes = u16_at(data, p + 4)? as usize;
    let sizes: Vec<usize> = (0..n).map(|i| u16_at(data, p + 8 + i * 2).map(|v| v as usize)).collect::<Option<_>>()?;
    let start = p + 8 + n * 2;
    let buf = data.get(start..start + n_bytes)?;
    (sizes.iter().sum::<usize>() == n_bytes).then_some((base, sizes, buf))
}

/// `animlib.c:JE_playRunSkipDump`: run (memset), skip, dump (memcpy), in
/// short and long variants; `None` if a read or a write goes outside
/// the buffers (the C then stops reading the animation).
fn run_skip_dump(input: &[u8], out: &mut [u8]) -> Option<()> {
    let (mut i, mut o) = (0usize, 0usize);
    let byte = |i: &mut usize| -> Option<u8> {
        let b = *input.get(*i)?;
        *i += 1;
        Some(b)
    };
    loop {
        let op = byte(&mut i)?;
        if op == 0x80 {
            let word = u16::from_le_bytes([byte(&mut i)?, byte(&mut i)?]) as usize;
            if word == 0 {
                return Some(());
            } else if word & 0x8000 == 0 {
                o += word;
            } else {
                let w = word & !0x8000;
                if w & 0x4000 != 0 {
                    let count = w & !0x4000;
                    let v = byte(&mut i)?;
                    out.get_mut(o..o + count)?.fill(v);
                    o += count;
                } else {
                    out.get_mut(o..o + w)?.copy_from_slice(input.get(i..i + w)?);
                    i += w;
                    o += w;
                }
            }
        } else if op & 0x80 != 0 {
            o += (op & 0x7F) as usize;
        } else if op == 0 {
            let count = byte(&mut i)? as usize;
            let v = byte(&mut i)?;
            out.get_mut(o..o + count)?.fill(v);
            o += count;
        } else {
            let count = op as usize;
            out.get_mut(o..o + count)?.copy_from_slice(input.get(i..i + count)?);
            i += count;
            o += count;
        }
        if o > out.len() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shipped `tyrend.anm` (end of episode 3): 112 records, 111
    /// frames played, all decoded without error; the screen is not black.
    #[test]
    fn tyrend_anm_decodes_every_played_frame() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/tyrend.anm");
        let Ok(data) = std::fs::read(p) else { return };
        let (palette, frames) = decode_anm(&data).expect("header rejected");
        assert_eq!(palette.len(), 768);
        assert_eq!(frames.len(), 111);
        assert!(frames.last().unwrap().iter().any(|&v| v != 0));
    }

    #[test]
    fn run_skip_dump_operations() {
        let mut out = vec![9u8; 8];
        // skip 2, run 3×7, copy [1,2], stop.
        run_skip_dump(&[0x82, 0x00, 3, 7, 2, 1, 2, 0x80, 0, 0], &mut out).unwrap();
        assert_eq!(out, vec![9, 9, 7, 7, 7, 1, 2, 9]);
        // Write outside the screen: rejected.
        assert!(run_skip_dump(&[0x00, 20, 1], &mut out).is_none());
    }
}
