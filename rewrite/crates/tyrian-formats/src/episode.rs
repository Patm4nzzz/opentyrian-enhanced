//! Episode script `levelsN.dat` (`level_load.c:JE_loadMap`, script part).
//!
//! The file is a sequence of encrypted Pascal strings; they are kept
//! raw (exact round-trip) and a parsed view is exposed: sections (one
//! per line starting with `*`), `]L[` directives (level) and `]I[`
//! (shop, 9 following lines).

use crate::crypt::{atoi_at, bytes_to_text, pop_ints, EncryptedStringFile};
use crate::io::Result;
use serde::Serialize;

/// Decoded `]L[` directive (`level_load.c`, `case 'L'`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelDirective {
    /// Index of the line in the file.
    pub line_index: usize,
    /// `atoi(s + 9)`: next section (0 → `mainLevel + 1` on the C side).
    pub next_level: i32,
    /// `s[13..22]` (9 characters max, `SDL_strlcpy(levelName, s+13, 10)`).
    pub level_name: String,
    /// `atoi(s + 22)`: piece of `music.mus` (1-based).
    pub level_song: i32,
    /// `atoi(s + 25)`: level (1-based) in `tyrianN.lvl`.
    pub lvl_file_num: i32,
    /// `s[28] == '$'` → `bonusLevelCurrent`.
    pub bonus: bool,
    /// `s[27] == '$'` → `normalBonusLevelCurrent`.
    pub normal_bonus: bool,
    pub raw: String,
}

impl LevelDirective {
    pub fn parse(line_index: usize, s: &[u8]) -> Self {
        let name_end = s.len().min(22);
        let name_bytes = if s.len() > 13 { &s[13..name_end] } else { &[][..] };
        let name_bytes = match name_bytes.iter().position(|&b| b == 0) {
            Some(z) => &name_bytes[..z],
            None => name_bytes,
        };
        Self {
            line_index,
            next_level: atoi_at(s, 9),
            level_name: bytes_to_text(name_bytes),
            level_song: atoi_at(s, 22),
            lvl_file_num: atoi_at(s, 25),
            bonus: s.len() > 28 && s[28] == b'$',
            normal_bonus: s.len() > 27 && s[27] == b'$',
            raw: bytes_to_text(s),
        }
    }
}

/// A shop category (`]I[`, one of the 9 following lines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShopCategory {
    /// Label (columns 0..8, informational).
    pub label: String,
    /// Integers read after column 8 (`str_pop_int`); the C code keeps 10 at most.
    pub items: Vec<i32>,
}

/// `]I[` block: 9 categories (ship, front weapon, rear, generator,
/// engine, left sidekick, right sidekick, armor, shield).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShopDirective {
    pub line_index: usize,
    pub categories: Vec<ShopCategory>,
}

/// A section (`*` then its lines up to the next `*`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// 1-based number (`mainLevel`).
    pub number: usize,
    /// Index of the `*` line in the file.
    pub line_index: usize,
    pub title: String,
    /// All (decrypted) lines of the section, `*` line excluded.
    pub lines: Vec<String>,
    pub levels: Vec<LevelDirective>,
    pub shops: Vec<ShopDirective>,
}

/// `levelsN.dat` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeScript {
    pub strings: EncryptedStringFile,
}

impl EpisodeScript {
    pub fn parse(data: &[u8]) -> Result<Self> {
        Ok(Self { strings: EncryptedStringFile::parse(data)? })
    }

    pub fn write(&self) -> Vec<u8> {
        self.strings.write()
    }

    /// Decrypted lines (raw bytes).
    pub fn plain_lines(&self) -> Vec<Vec<u8>> {
        self.strings.strings.iter().map(|s| s.plain()).collect()
    }

    /// Split into sections with decoded `]L[` and `]I[` directives.
    pub fn sections(&self) -> Vec<Section> {
        let lines = self.plain_lines();
        let mut sections: Vec<Section> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let s = &lines[i];
            if s.first() == Some(&b'*') {
                sections.push(Section {
                    number: sections.len() + 1,
                    line_index: i,
                    title: bytes_to_text(s),
                    lines: Vec::new(),
                    levels: Vec::new(),
                    shops: Vec::new(),
                });
                i += 1;
                continue;
            }
            if let Some(sec) = sections.last_mut() {
                sec.lines.push(bytes_to_text(s));
                if s.len() >= 2 && s[0] == b']' {
                    match s[1] {
                        b'L' => sec.levels.push(LevelDirective::parse(i, s)),
                        b'I' => {
                            let mut categories = Vec::new();
                            for k in 1..=9 {
                                let Some(c) = lines.get(i + k) else { break };
                                let label = bytes_to_text(&c[..c.len().min(8)]);
                                let items = if c.len() > 8 { pop_ints(&c[8..]) } else { Vec::new() };
                                categories.push(ShopCategory { label, items });
                            }
                            sec.shops.push(ShopDirective { line_index: i, categories });
                        }
                        _ => {}
                    }
                }
            }
            i += 1;
        }
        sections
    }
}
