//! Data cubes (`cubetxtN.dat`): records and line layout,
//! like `game_menu.c:load_cube`.
//!
//! The file is a sequence of encrypted strings (`crypt::EncryptedStringFile`).
//! Each cube starts with a `*NN FF…` line (FF = face, 1-based), then
//! its title, its header and the text lines up to the next `*`.
//! The text is re-wrapped into lines of at most [`LINE_CHARS`] characters and
//! [`LINE_WIDTH`] pixels (TINY_FONT font), at most [`MAX_LINES`] lines.
//! The width of a word is supplied by the caller (it depends on the font).

use crate::crypt::EncryptedStringFile;

/// `sizeof(*cube->text) - 1` (`char text[90][36]`).
pub const LINE_CHARS: usize = 35;
/// `game_menu.c:cube_line_width`.
pub const LINE_WIDTH: i32 = 150;
/// `COUNTOF(cube->text)`.
pub const MAX_LINES: usize = 90;

/// A cube as read from `cubetxtN.dat`, text still in source lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubeRecord {
    /// Face (`face_sprite`, 0-based; -1: none).
    pub face_sprite: i32,
    pub title: String,
    pub header: String,
    /// Source lines; an empty line separates two paragraphs.
    pub body: Vec<String>,
}

/// Text of a cube split into display lines (`cube_struct.text`,
/// `last_line` = `lines.len()`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CubeText {
    pub lines: Vec<String>,
}

impl CubeText {
    fn push_word(&mut self, line: usize, word: &str, space: bool) {
        if line >= MAX_LINES {
            return;
        }
        while self.lines.len() <= line {
            self.lines.push(String::new());
        }
        let l = &mut self.lines[line];
        if space {
            l.push(' ');
        }
        l.push_str(word);
    }
}

/// `load_cube`, lookup: cube `index` (1-based, value of `cubeList`)
/// is the one that follows the `index`-th line starting with `*`.
/// The file is in CP437: byte 0xF8 is the degree sign, absent from the
/// game font (no glyph, zero width). Read as Latin-1 it became
/// "ø", folded to "o": one extra word on the line, hence a different
/// break from the C code (cubes 4-2 and 4-3). "°" is removed by `to_cp437`.
fn degree_sign(line: &str) -> String {
    line.replace('\u{f8}', "\u{b0}")
}

pub fn record(file: &EncryptedStringFile, index: usize) -> Option<CubeRecord> {
    let lines = file.lines();
    let mut stars = 0;
    let start = lines.iter().position(|l| {
        if l.starts_with('*') {
            stars += 1;
        }
        stars == index
    })?;
    // str_pop_int(&buf[4], &face_sprite) then --face_sprite.
    let face = lines[start].get(4..).map_or(0, |s| crate::crypt::atoi_at(s.as_bytes(), 0));
    let title = degree_sign(lines.get(start + 1)?);
    let header = degree_sign(lines.get(start + 2)?);
    let body = lines[start + 3..].iter().take_while(|l| !l.starts_with('*')).map(|l| degree_sign(l)).collect();
    Some(CubeRecord { face_sprite: face - 1, title, header, body })
}

/// `load_cube`, file text: each word is preceded by a space (even
/// the first of a paragraph, like the C code), an empty line ends the current
/// line, the following ones leave four blank lines. A word too long
/// for a line abandons the rest of its source line.
pub fn wrap_record(body: &[String], width: impl Fn(&str) -> i32) -> CubeText {
    let mut out = CubeText::default();
    let (mut line, mut chars, mut w) = (0usize, 0usize, 0i32);
    for src in body {
        if src.is_empty() {
            if chars == 0 {
                line += 4;
            } else {
                line += 1;
            }
            chars = 0;
            w = 0;
            continue;
        }
        for word in src.split(' ') {
            let (wc, ww) = (word.chars().count(), width(word));
            if wc > LINE_CHARS || ww > LINE_WIDTH {
                break;
            }
            let mut space = true;
            chars += wc + 1;
            w += ww + 6;
            if chars > LINE_CHARS || w > LINE_WIDTH {
                line += 1;
                chars = wc;
                w = ww;
                space = false;
            }
            out.push_word(line, word, space);
        }
    }
    out
}

/// `load_cube`, locale text (`dc_<episode>_<cube>_body`): a lone `\n`
/// separates two source lines of the same paragraph, an empty source
/// line is a paragraph break (first: end of line; following:
/// four blank lines); no space before the first word of a line.
/// A word that is too long is skipped.
pub fn wrap_locale(body: &str, width: impl Fn(&str) -> i32) -> CubeText {
    let mut out = CubeText::default();
    let (mut line, mut chars, mut w) = (0usize, 0usize, 0i32);
    let src: Vec<char> = body.chars().collect();
    let mut at_line_start = true;
    let mut i = 0;
    while i < src.len() {
        match src[i] {
            '\n' => {
                if at_line_start {
                    if chars == 0 {
                        line += 4;
                    } else {
                        line += 1;
                    }
                    chars = 0;
                    w = 0;
                }
                at_line_start = true;
                i += 1;
                continue;
            }
            ' ' => {
                i += 1;
                continue;
            }
            _ => {}
        }
        at_line_start = false;
        // char word[80]: 79 characters at most, the rest makes another word.
        let start = i;
        while i < src.len() && src[i] != ' ' && src[i] != '\n' && i - start < 79 {
            i += 1;
        }
        let word: String = src[start..i].iter().collect();
        let (wc, ww) = (i - start, width(&word));
        if wc > LINE_CHARS || ww > LINE_WIDTH {
            continue;
        }
        let mut space = chars > 0;
        chars += wc + usize::from(space);
        w += ww + if space { 6 } else { 0 };
        if chars > LINE_CHARS || w > LINE_WIDTH {
            line += 1;
            chars = wc;
            w = ww;
            space = false;
        }
        out.push_word(line, &word, space);
    }
    out
}

/// `struct cube_struct`: `title[81]`, `header[13]` (text truncated to the
/// array size minus the trailing zero, like `read_encrypted_pascal_string`
/// and `strncpy`).
pub const TITLE_CHARS: usize = 80;
pub const HEADER_CHARS: usize = 12;

/// A cube ready to display (`cube[cube_slot]` after `load_cube`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cube {
    pub face_sprite: i32,
    pub title: String,
    pub header: String,
    pub text: CubeText,
}

/// `key=value` entries of a locale file (`locale.c:load_file`):
/// a value that starts with `` ` `` continues on the following lines
/// up to the one that ends with `` ` `` (lines joined by `\n`, without a
/// leading `\n` if the first line is empty). Lines starting with `#` and lines without `=`
/// are ignored.
pub fn locale_entries(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = match value.strip_prefix('`') {
            Some(first) if first.ends_with('`') && !first.is_empty() => first[..first.len() - 1].to_string(),
            Some(first) => {
                let mut v = first.to_string();
                for l in lines.by_ref() {
                    let (l, end) = match l.strip_suffix('`') {
                        Some(l) => (l, true),
                        None => (l, false),
                    };
                    if !v.is_empty() {
                        v.push('\n');
                    }
                    v.push_str(l);
                    if end {
                        break;
                    }
                }
                v
            }
            None => value.to_string(),
        };
        // Non-breaking space (French typography): the C code turns it into a space
        // at load time (locale.c:utf8_to_cp437), line breaking included.
        out.push((key.to_string(), value.replace('\u{a0}', " ")));
    }
    out
}

/// `load_cube(cube_slot, cube_index)`: record `index` of the episode
/// file, replaced field by field by the locale keys
/// `dc_<episode>_<index>_title|header|body` when they
/// exist. `width`: `JE_textWidth(word, TINY_FONT)`. `None` if the
/// file does not have that many cubes.
pub fn load(
    file: &EncryptedStringFile,
    locale: &dyn Fn(&str) -> Option<String>,
    episode: u8,
    index: usize,
    width: impl Fn(&str) -> i32,
) -> Option<Cube> {
    let rec = record(file, index)?;
    let key = |field: &str| locale(&format!("dc_{episode}_{index}_{field}"));
    let cut = |s: String, n: usize| s.chars().take(n).collect::<String>();
    let title = cut(key("title").unwrap_or(rec.title), TITLE_CHARS);
    let header = cut(key("header").unwrap_or(rec.header), HEADER_CHARS);
    let text = match key("body") {
        Some(body) => wrap_locale(&body, &width),
        None => wrap_record(&rec.body, &width),
    };
    Some(Cube { face_sprite: rec.face_sprite, title, header, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test font: 5 px per character (+1 spacing, like
    /// `JE_textWidth`).
    fn w(s: &str) -> i32 {
        s.chars().count() as i32 * 6
    }

    #[test]
    fn degree_byte_is_not_read_as_o_slash() {
        let plain = [b"*01".to_vec(), b"T".to_vec(), b"H".to_vec(), b"is 103\xF8 here".to_vec()];
        let file = EncryptedStringFile {
            strings: plain.iter().map(|s| crate::crypt::EncryptedString::from_plain(s).unwrap()).collect(),
        };
        let r = record(&file, 1).unwrap();
        assert_eq!(r.body, vec!["is 103\u{b0} here"]);
    }

    #[test]
    fn record_finds_the_nth_star() {
        let plain =
            ["*01 04 (before)", "Title A", "Head A", "line a", "*02 02", "Title B", "Head B", "b1", "", "b2", "*03"];
        let file = EncryptedStringFile {
            strings: plain.iter().map(|s| crate::crypt::EncryptedString::from_plain(s.as_bytes()).unwrap()).collect(),
        };
        let a = record(&file, 1).unwrap();
        assert_eq!((a.face_sprite, a.title.as_str(), a.header.as_str()), (3, "Title A", "Head A"));
        assert_eq!(a.body, vec!["line a"]);
        let b = record(&file, 2).unwrap();
        assert_eq!(b.face_sprite, 1);
        assert_eq!(b.body, vec!["b1", "", "b2"]);
        assert!(record(&file, 5).is_none());
    }

    #[test]
    fn binary_wrap_prepends_spaces_and_breaks_paragraphs() {
        let body: Vec<String> = ["aaa bbb", "", "", "ccc"].iter().map(|s| s.to_string()).collect();
        let t = wrap_record(&body, w);
        // Empty line 1: end of line; empty line 2 (nothing written): +4.
        assert_eq!(t.lines[0], " aaa bbb");
        assert_eq!(t.lines.len(), 6);
        assert_eq!(t.lines[5], " ccc");
    }

    #[test]
    fn locale_wrap_cuts_at_line_width() {
        // 150 px = 25 characters of 6 px: "word" ×6 = 29 > 25 characters? no:
        // 6 words of 4 + 5 spaces = 29 > 35? no, but 29·6 = 174 px > 150.
        let t = wrap_locale("word word word word word word", w);
        assert_eq!(t.lines, vec!["word word word word word", "word"]);
        let t = wrap_locale("a\nb\n\nc", w);
        assert_eq!(t.lines, vec!["a b", "c"]);
        let t = wrap_locale("a\n\n\nc", w);
        assert_eq!(t.lines.len(), 6, "second empty line: four blank lines");
        assert_eq!(t.lines[5], "c");
    }

    #[test]
    fn locale_entries_join_backtick_lines() {
        let e = locale_entries("# x\ndc_1_1_title=T\ndc_1_1_body=`a b\n\nc`\nk=`one`\n");
        assert_eq!(e[0], ("dc_1_1_title".into(), "T".into()));
        assert_eq!(e[1], ("dc_1_1_body".into(), "a b\n\nc".into()));
        assert_eq!(e[2], ("k".into(), "one".into()));
        // French non-breaking space: ordinary space, like the C code.
        let e = locale_entries("dc_1_2_body=`Note\u{a0}: «\u{a0}x\u{a0}»`\n");
        assert_eq!(e[0].1, "Note : « x »");
    }

    /// First cube of episode 1 read from the game data: the text of the
    /// English locale (reference generated from `cubetxt1.dat`) and that
    /// of the file give the same lines.
    #[test]
    fn episode1_first_cube_locale_matches_file() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        let (Ok(bin), Ok(loc)) = (
            std::fs::read(root.join("cubetxt1.dat")),
            std::fs::read_to_string(root.join("locales/locale_en_cubes.txt")),
        ) else {
            return;
        };
        let file = EncryptedStringFile::parse(&bin).unwrap();
        let entries = locale_entries(&loc);
        let from_locale = |k: &str| entries.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        let a = load(&file, &from_locale, 1, 1, w).unwrap();
        let b = load(&file, &|_| None, 1, 1, w).unwrap();
        assert_eq!(a.title, "~TYRIAN~ DATAFLUX 040-54-871");
        assert_eq!((a.header.as_str(), a.face_sprite), ("Data", 3));
        assert_eq!(a.title, b.title);
        // The file keeps the space of an empty word (trailing space of a source
        // line) and a space before each word: only the borders differ.
        let trim = |t: &CubeText| t.lines.iter().map(|l| l.trim().to_string()).collect::<Vec<_>>();
        assert_eq!(trim(&a.text), trim(&b.text));
        assert!(a.text.lines.len() > 10);
    }
}
