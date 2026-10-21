//! Saves of the C version: `opentyrian.dat` container and `GAME` section
//! (the old `opentyrian.sav` / `tyrian.sav`).
//!
//! # `opentyrian.dat` container (`src/opentyrian_dat.c`)
//!
//! Little-endian: `magic u32 = "OT2K"`, `version u32 = 1`, `count u32`, then
//! for each section `tag[4]`, `flags u32` (bit 0 = signed), `size u32`,
//! `digest u32` (salted FNV-1a of the contents, 0 if unsigned), the contents; finally
//! a salted FNV-1a digest of everything before it (`ot_dat_flush`).
//!
//! # `GAME` section (`src/config.c`)
//!
//! - `saveTemp` (2502 bytes, `SIZEOF_SAVEGAMETEMP`): 22 slots of 109
//!   bytes (`JE_saveConfiguration`), then 100 bytes (ship editor rights
//!   `saveTemp[SAVE_FILES_SIZE + n]` and `editorLevel` in
//!   `SIZEOF_SAVEGAMETEMP - 6/-5`), then 4 checksums. The first 2498
//!   bytes are encrypted (`JE_encryptSaveTemp`).
//! - T2K high scores in clear: 10 "Timed Battle" tables of 3 entries
//!   (`score i32`, `len u8`, `name[29]`, `difficulty u8`) then 10 campaign
//!   tables of 3 entries with one more unknown `i32` (written as `0x12345678`).
//! - Enhanced Edition extensions (`"OTYE"`, statistics, achievements, `"OTY2"` /
//!   `"OTY3"` blocks): kept as they are, only the basic statistics
//!   are decoded.
//!
//! Round-trip: each field keeps its original bytes (including those that
//! follow the end of a Pascal string), and writing recomputes encryption,
//! checksums and digests like the C code. `write(parse(b)) == b` for any file
//! that the C code accepts.

use crate::io::{FormatError, Reader, Result, Writer};

/// `config.h:SAVE_FILES_NUM`: 11 slots per player count.
pub const SAVE_FILES_NUM: usize = 22;
/// Size of a slot on disk (`JE_saveConfiguration`).
pub const SAVE_RECORD_SIZE: usize = 109;
/// `config.h:SAVE_FILES_SIZE`.
pub const SAVE_FILES_SIZE: usize = 2398;
/// `config.h:SIZEOF_SAVEGAMETEMP`.
pub const SAVE_TEMP_SIZE: usize = SAVE_FILES_SIZE + 4 + 100;
/// `config.h:SAVE_FILE_SIZE`: bytes that are encrypted and covered by the checksums.
pub const SAVE_FILE_SIZE: usize = SAVE_TEMP_SIZE - 4;
/// `config.c:cryptKey`.
pub const CRYPT_KEY: [u8; 10] = [15, 50, 89, 240, 147, 34, 86, 9, 32, 208];

/// Value written by the C code in the unknown field of campaign scores.
pub const T2K_UNKNOWN: i32 = 0x1234_5678;

const OT_DAT_MAGIC: u32 = 0x4B32_544F;
const OT_DAT_SALT: u32 = 0x9E37_79B1;
const OT_FLAG_SIGNED: u32 = 1;
const OTYE_MAGIC: u32 = 0x4F54_5945;

/// `opentyrian_dat.c:fnv1a`.
pub fn fnv1a(mut h: u32, data: &[u8]) -> u32 {
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// `opentyrian_dat.c:digest_payload`.
pub fn digest(data: &[u8]) -> u32 {
    fnv1a(OT_DAT_SALT, data)
}

// --- Container ---------------------------------------------------------------

/// A section of the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtSection {
    pub tag: [u8; 4],
    pub flags: u32,
    pub payload: Vec<u8>,
    /// Digest read from disk.
    pub stored_digest: u32,
}

impl OtSection {
    pub fn is_signed(&self) -> bool {
        self.flags & OT_FLAG_SIGNED != 0
    }

    /// False if the section is signed and its digest does not match:
    /// the C code then rejects it (`load_container`) and does not rewrite it.
    pub fn is_valid(&self) -> bool {
        !self.is_signed() || self.stored_digest == digest(&self.payload)
    }
}

/// `opentyrian.dat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtDat {
    pub version: u32,
    pub sections: Vec<OtSection>,
}

impl OtDat {
    pub fn new() -> Self {
        Self { version: 1, sections: Vec::new() }
    }

    /// `opentyrian_dat.c:load_container`, except that invalid sections are
    /// kept (see [`OtSection::is_valid`]) instead of being emptied.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(FormatError::new("opentyrian.dat: file too short"));
        }
        let mut r = Reader::new(data);
        let magic = r.u32()?;
        let version = r.u32()?;
        let count = r.u32()?;
        if magic != OT_DAT_MAGIC || version != 1 || count > 16 {
            return Err(FormatError::new(format!(
                "opentyrian.dat: invalid header (magic {magic:#x}, version {version}, {count} sections)"
            )));
        }
        let body = &data[..data.len() - 4];
        let stored = u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap());
        if stored != fnv1a(OT_DAT_SALT, body) {
            return Err(FormatError::new("opentyrian.dat: wrong container digest (file truncated or modified)"));
        }
        let mut r = Reader::new(body);
        r.seek(12)?;
        let mut sections = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let tag = r.array::<4>()?;
            let flags = r.u32()?;
            let size = r.u32()? as usize;
            let stored_digest = r.u32()?;
            let payload = r.bytes(size)?.to_vec();
            sections.push(OtSection { tag, flags, payload, stored_digest });
        }
        if r.remaining() != 0 {
            return Err(FormatError::new("opentyrian.dat: extra bytes after the last section"));
        }
        Ok(Self { version, sections })
    }

    /// `opentyrian_dat.c:ot_dat_flush`: valid sections only, digests
    /// recomputed.
    pub fn write(&self) -> Vec<u8> {
        let valid: Vec<&OtSection> = self.sections.iter().filter(|s| s.is_valid()).collect();
        let mut w = Writer::new();
        w.u32(OT_DAT_MAGIC);
        w.u32(self.version);
        w.u32(valid.len() as u32);
        for s in valid {
            w.bytes(&s.tag);
            w.u32(s.flags);
            w.u32(s.payload.len() as u32);
            w.u32(if s.is_signed() { digest(&s.payload) } else { 0 });
            w.bytes(&s.payload);
        }
        let mut out = w.into_inner();
        let d = fnv1a(OT_DAT_SALT, &out);
        out.extend_from_slice(&d.to_le_bytes());
        out
    }

    /// `ot_dat_get_section`: contents of a valid section.
    pub fn section(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        self.sections.iter().find(|s| &s.tag == tag && s.is_valid()).map(|s| s.payload.as_slice())
    }

    /// `ot_dat_set_section`.
    pub fn set_section(&mut self, tag: &[u8; 4], payload: Vec<u8>, signed: bool) {
        let flags = if signed { OT_FLAG_SIGNED } else { 0 };
        let stored_digest = if signed { digest(&payload) } else { 0 };
        match self.sections.iter_mut().find(|s| &s.tag == tag) {
            Some(s) => {
                s.flags = flags;
                s.payload = payload;
                s.stored_digest = stored_digest;
            }
            None => self.sections.push(OtSection { tag: *tag, flags, payload, stored_digest }),
        }
    }
}

impl Default for OtDat {
    fn default() -> Self {
        Self::new()
    }
}

// --- saveTemp encryption ------------------------------------------------------

/// `config.c:JE_decryptSaveTemp`: returns the 2498 plaintext bytes, or an
/// error if one of the four checksums is wrong (the C code then starts
/// again from a fresh state).
pub fn decrypt_save_temp(cipher: &[u8]) -> Result<Vec<u8>> {
    if cipher.len() < SAVE_TEMP_SIZE {
        return Err(FormatError::new("saveTemp truncated"));
    }
    let mut plain = vec![0u8; SAVE_FILE_SIZE];
    for x in (0..SAVE_FILE_SIZE).rev() {
        plain[x] = cipher[x] ^ CRYPT_KEY[(x + 1) % 10];
        if x > 0 {
            plain[x] ^= cipher[x - 1];
        }
    }
    let sums = checksums(&plain);
    let stored = &cipher[SAVE_FILE_SIZE..SAVE_TEMP_SIZE];
    for (i, name) in ["additive", "subtractive", "multiplicative", "XOR"].iter().enumerate() {
        if stored[i] != sums[i] {
            return Err(FormatError::new(format!(
                "saveTemp: wrong {name} checksum ({} instead of {})",
                stored[i], sums[i]
            )));
        }
    }
    Ok(plain)
}

/// The four sums of `JE_encryptSaveTemp` (over the plaintext).
pub fn checksums(plain: &[u8]) -> [u8; 4] {
    let (mut add, mut sub, mut mul, mut xor) = (0u8, 0u8, 1u8, 0u8);
    for &b in &plain[..SAVE_FILE_SIZE] {
        add = add.wrapping_add(b);
        sub = sub.wrapping_sub(b);
        mul = mul.wrapping_mul(b).wrapping_add(1);
        xor ^= b;
    }
    [add, sub, mul, xor]
}

/// `config.c:JE_encryptSaveTemp`: 2502 bytes (encrypted + sums).
pub fn encrypt_save_temp(plain: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; SAVE_TEMP_SIZE];
    out[..SAVE_FILE_SIZE].copy_from_slice(&plain[..SAVE_FILE_SIZE]);
    out[SAVE_FILE_SIZE..].copy_from_slice(&checksums(plain));
    for x in 0..SAVE_FILE_SIZE {
        out[x] ^= CRYPT_KEY[(x + 1) % 10];
        if x > 0 {
            out[x] ^= out[x - 1];
        }
    }
    out
}

// --- Slots --------------------------------------------------------------------

/// A slot (`config.h:JE_SaveFileType`), original bytes kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveSlot {
    pub encode: u16,
    /// Section of `levelsN.dat` (0 = empty slot, `JE_operation`).
    pub level: u16,
    /// `JE_PItemsType` (`config.c:playeritems_to_pitems`): front, rear,
    /// super arcade mode, left sidekick, right sidekick, generator, level and series
    /// of the player 2 sidekick, initial episode, shield, special, ship.
    pub items: [u8; 12],
    pub score: i32,
    pub score2: i32,
    /// Pascal string: length then 9 bytes (`levelName`).
    pub level_name_raw: [u8; 10],
    /// 14 bytes ("BYTE array, not a STRING").
    pub name_raw: [u8; 14],
    pub cubes: u8,
    pub power: [u8; 2],
    pub episode: u8,
    pub last_items: [u8; 12],
    pub difficulty: u8,
    pub secret_hint: u8,
    pub input1: u8,
    pub input2: u8,
    pub game_has_repeated: u8,
    pub initial_difficulty: u8,
    pub high_score1: i32,
    pub high_score2: i32,
    /// Pascal string: length then 29 bytes.
    pub high_score_name_raw: [u8; 30],
    pub high_score_diff: u8,
}

fn pascal(raw: &[u8], max: usize) -> String {
    let len = (raw[0] as usize).min(max);
    let bytes = &raw[1..1 + len];
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&b| b as char).collect()
}

fn set_pascal(raw: &mut [u8], s: &str) {
    raw.fill(0);
    let bytes: Vec<u8> = s.bytes().take(raw.len() - 1).collect();
    raw[0] = bytes.len() as u8;
    raw[1..1 + bytes.len()].copy_from_slice(&bytes);
}

impl SaveSlot {
    fn read(r: &mut Reader) -> Result<Self> {
        Ok(Self {
            encode: r.u16()?,
            level: r.u16()?,
            items: r.array()?,
            score: r.i32()?,
            score2: r.i32()?,
            level_name_raw: r.array()?,
            name_raw: r.array()?,
            cubes: r.u8()?,
            power: r.array()?,
            episode: r.u8()?,
            last_items: r.array()?,
            difficulty: r.u8()?,
            secret_hint: r.u8()?,
            input1: r.u8()?,
            input2: r.u8()?,
            game_has_repeated: r.u8()?,
            initial_difficulty: r.u8()?,
            high_score1: r.i32()?,
            high_score2: r.i32()?,
            high_score_name_raw: r.array()?,
            high_score_diff: r.u8()?,
        })
    }

    fn write(&self, w: &mut Writer) {
        w.u16(self.encode);
        w.u16(self.level);
        w.bytes(&self.items);
        w.i32(self.score);
        w.i32(self.score2);
        w.bytes(&self.level_name_raw);
        w.bytes(&self.name_raw);
        w.u8(self.cubes);
        w.bytes(&self.power);
        w.u8(self.episode);
        w.bytes(&self.last_items);
        w.u8(self.difficulty);
        w.u8(self.secret_hint);
        w.u8(self.input1);
        w.u8(self.input2);
        w.u8(self.game_has_repeated);
        w.u8(self.initial_difficulty);
        w.i32(self.high_score1);
        w.i32(self.high_score2);
        w.bytes(&self.high_score_name_raw);
        w.u8(self.high_score_diff);
    }

    /// Empty slot like `JE_loadConfiguration` without a save (names as
    /// spaces, the rest zero; the random dummy scores are not
    /// reproduced).
    pub fn empty() -> Self {
        Self {
            encode: 0,
            level: 0,
            items: [0; 12],
            score: 0,
            score2: 0,
            level_name_raw: [0; 10],
            name_raw: [b' '; 14],
            cubes: 0,
            power: [0; 2],
            episode: 0,
            last_items: [0; 12],
            difficulty: 0,
            secret_hint: 0,
            input1: 0,
            input2: 0,
            game_has_repeated: 0,
            initial_difficulty: 0,
            high_score1: 0,
            high_score2: 0,
            high_score_name_raw: [0; 30],
            high_score_diff: 0,
        }
    }

    /// `levelName` (limited to 9 characters like `JE_loadConfiguration`).
    pub fn level_name(&self) -> String {
        pascal(&self.level_name_raw, 9)
    }

    /// Write like `JE_saveConfiguration` (zeros then length + text).
    pub fn set_level_name(&mut self, s: &str) {
        set_pascal(&mut self.level_name_raw, s);
    }

    /// Slot name (up to the first NUL).
    pub fn name(&self) -> String {
        let end = self.name_raw.iter().position(|&b| b == 0).unwrap_or(14);
        self.name_raw[..end].iter().map(|&b| b as char).collect()
    }

    /// `snprintf(saveFiles[].name, 15, "%s", name)` then copy of 14 bytes.
    pub fn set_name(&mut self, s: &str) {
        self.name_raw = [0; 14];
        for (d, b) in self.name_raw.iter_mut().zip(s.bytes()) {
            *d = b;
        }
    }

    /// `JE_operation`: a slot is loadable if `level > 0`.
    pub fn is_used(&self) -> bool {
        self.level > 0
    }
}

/// T2K high score entry (`T2KHighScoreType`), bytes kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T2kHighScore {
    pub score: i32,
    /// Unknown field of the campaign tables (`None` for Timed Battle).
    pub unknown: Option<i32>,
    pub name_len: u8,
    pub name_raw: [u8; 29],
    pub difficulty: u8,
}

impl T2kHighScore {
    /// Name limited to 29 characters (`config.c`, clamp of `len`).
    pub fn name(&self) -> String {
        let len = (self.name_len as usize).min(29);
        let bytes = &self.name_raw[..len];
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
        bytes[..end].iter().map(|&b| b as char).collect()
    }

    /// Write like `JE_saveConfiguration`: `len = strlen(name)`, 29 bytes
    /// of the buffer (zeros after the name here).
    pub fn new(score: i32, name: &str, difficulty: u8, campaign: bool) -> Self {
        let mut name_raw = [0u8; 29];
        let bytes: Vec<u8> = name.bytes().take(29).collect();
        name_raw[..bytes.len()].copy_from_slice(&bytes);
        Self { score, unknown: campaign.then_some(T2K_UNKNOWN), name_len: bytes.len() as u8, name_raw, difficulty }
    }
}

/// Basic statistics of the `"OTYE"` extension (`config.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnhancedStats {
    pub version: u16,
    pub enemies_killed: u32,
    pub shots_fired: u32,
    pub games_played: u32,
    pub total_score: u32,
    pub time_played_ms: u32,
    pub levels_completed: u32,
    pub deaths: u32,
}

/// Contents of the `GAME` section (the old `opentyrian.sav`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSave {
    pub slots: Vec<SaveSlot>,
    /// `saveTemp[SAVE_FILES_SIZE .. SAVE_FILE_SIZE]`: ship editor rights
    /// (`JE_endLevelAni`) and `editorLevel` (bytes 98-99).
    pub extra: [u8; 100],
    /// 20 tables of 3 entries: 0..10 Timed Battle, 10..20 campaign
    /// (`10 + (episode - 1) * 2`, +1 in two-player: `JE_highScoreCheck`).
    pub high_scores: Vec<[T2kHighScore; 3]>,
    /// Enhanced Edition extensions, kept raw.
    pub tail: Vec<u8>,
}

impl GameSave {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let plain = decrypt_save_temp(data)?;
        let mut r = Reader::new(&plain);
        let mut slots = Vec::with_capacity(SAVE_FILES_NUM);
        for _ in 0..SAVE_FILES_NUM {
            slots.push(SaveSlot::read(&mut r)?);
        }
        let extra = r.array::<100>()?;
        let mut r = Reader::new(data);
        r.seek(SAVE_TEMP_SIZE)?;
        let mut high_scores = Vec::with_capacity(20);
        for table in 0..20 {
            let mut entry = || -> Result<T2kHighScore> {
                let score = r.i32()?;
                let unknown = if table >= 10 { Some(r.i32()?) } else { None };
                Ok(T2kHighScore { score, unknown, name_len: r.u8()?, name_raw: r.array()?, difficulty: r.u8()? })
            };
            high_scores.push([entry()?, entry()?, entry()?]);
        }
        let tail = r.rest();
        Ok(Self { slots, extra, high_scores, tail })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        for s in &self.slots {
            s.write(&mut w);
        }
        w.bytes(&self.extra);
        let mut out = encrypt_save_temp(&w.into_inner());
        let mut w = Writer::new();
        for table in &self.high_scores {
            for e in table {
                w.i32(e.score);
                if let Some(u) = e.unknown {
                    w.i32(u);
                }
                w.u8(e.name_len);
                w.bytes(&e.name_raw);
                w.u8(e.difficulty);
            }
        }
        out.extend_from_slice(&w.into_inner());
        out.extend_from_slice(&self.tail);
        out
    }

    /// Fresh save ("no save" branch of
    /// `JE_loadConfiguration`, without the randomly drawn names and scores):
    /// `initial_item_avail` = `initialItemAvail`, `editorLevel = 800`.
    pub fn new_empty(initial_item_avail: &[u8; 100]) -> Self {
        let mut extra = *initial_item_avail;
        extra[98] = (800u16 & 0xff) as u8;
        extra[99] = (800u16 >> 8) as u8;
        let entry = |campaign| T2kHighScore::new(0, "", 0, campaign);
        let high_scores = (0..20).map(|t| [entry(t >= 10), entry(t >= 10), entry(t >= 10)]).collect();
        Self { slots: vec![SaveSlot::empty(); SAVE_FILES_NUM], extra, high_scores, tail: Vec::new() }
    }

    /// `editorLevel` (`saveTemp[SIZEOF_SAVEGAMETEMP - 6/-5]`, little-endian).
    pub fn editor_level(&self) -> u16 {
        u16::from_le_bytes([self.extra[98], self.extra[99]])
    }

    /// `"OTYE"` statistics if present.
    pub fn enhanced_stats(&self) -> Option<EnhancedStats> {
        let mut r = Reader::new(&self.tail);
        if r.u32().ok()? != OTYE_MAGIC {
            return None;
        }
        let version = r.u16().ok()?;
        let mut v = [0u32; 7];
        for x in &mut v {
            *x = r.u32().ok()?;
        }
        Some(EnhancedStats {
            version,
            enemies_killed: v[0],
            shots_fired: v[1],
            games_played: v[2],
            total_score: v[3],
            time_played_ms: v[4],
            levels_completed: v[5],
            deaths: v[6],
        })
    }
}

/// Reads a C save: `opentyrian.dat` (`GAME` section) or an old raw
/// `opentyrian.sav` / `tyrian.sav` (order of `JE_loadConfiguration`).
pub fn load_any(data: &[u8]) -> Result<GameSave> {
    if data.len() >= 4 && u32::from_le_bytes(data[..4].try_into().unwrap()) == OT_DAT_MAGIC {
        let dat = OtDat::parse(data)?;
        let game = dat.section(b"GAME").ok_or_else(|| FormatError::new("opentyrian.dat: no valid GAME section"))?;
        GameSave::parse(game)
    } else {
        GameSave::parse(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_layout_sums_to_c_constant() {
        assert_eq!(SAVE_RECORD_SIZE * SAVE_FILES_NUM, SAVE_FILES_SIZE);
        let mut w = Writer::new();
        SaveSlot::empty().write(&mut w);
        assert_eq!(w.len(), SAVE_RECORD_SIZE);
    }

    #[test]
    fn encrypt_then_decrypt_is_identity() {
        let plain: Vec<u8> = (0..SAVE_FILE_SIZE).map(|i| (i * 7 + 3) as u8).collect();
        let c = encrypt_save_temp(&plain);
        assert_eq!(decrypt_save_temp(&c).unwrap(), plain);
        // A tampered sum is rejected. (Flipping one bit of the ciphertext is
        // not always enough: the chaining changes two plaintext bytes whose
        // effects may cancel out in the four C sums.)
        let mut bad = c.clone();
        bad[SAVE_FILE_SIZE + 2] ^= 1;
        assert!(decrypt_save_temp(&bad).is_err());
    }

    #[test]
    fn new_save_round_trips() {
        let mut s = GameSave::new_empty(&[1; 100]);
        s.slots[10].level = 3;
        s.slots[10].set_level_name("TYRIAN");
        s.slots[10].set_name("LAST LEVEL");
        let bytes = s.write();
        let back = GameSave::parse(&bytes).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.write(), bytes);
        assert_eq!(back.slots[10].level_name(), "TYRIAN");
        assert_eq!(back.editor_level(), 800);
        let mut dat = OtDat::new();
        dat.set_section(b"GAME", bytes.clone(), true);
        let dat_bytes = dat.write();
        assert_eq!(OtDat::parse(&dat_bytes).unwrap().write(), dat_bytes);
        assert_eq!(load_any(&dat_bytes).unwrap(), s);
    }
}
