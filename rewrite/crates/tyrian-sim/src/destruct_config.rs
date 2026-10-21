//! Hidden Destruct configuration: `destruct.cfg` file
//! (`destruct.c:JE_destructGame` and `load_destruct_config`).
//!
//! The format is that of `config_file.c` (`section`, `item`,
//! `list` directives, fields between apostrophes or quotes); this module reproduces
//! its reading (`config_parse`), writing (`config_write`) and the
//! manipulations used by `load_destruct_config`, so that the rewritten
//! file is byte-for-byte identical to the C one.

use crate::destruct::{DestructConfig, MAX_MOVE, MAX_PLAYERS, UNIT_NAMES, UNIT_TANK};

/// Value of an option: `item` (one value) or `list` (zero or more).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Item(String),
    List(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Opt {
    key: String,
    value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Section {
    kind: String,
    name: Option<String>,
    options: Vec<Opt>,
}

/// `Config` de `config_file.h`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigFile {
    sections: Vec<Section>,
}

fn is_whitespace(c: u8) -> bool {
    c == b'\t' || c == b' '
}

fn is_end(c: u8) -> bool {
    c == 0 || c == b'\n' || c == b'\r'
}

fn is_whitespace_or_end(c: u8) -> bool {
    is_whitespace(c) || is_end(c)
}

#[derive(PartialEq, Eq)]
enum Directive {
    Invalid,
    Section,
    Item,
    List,
}

/// `config_file.c:match_directive`.
fn match_directive(b: &[u8], i: &mut usize) -> Directive {
    let mut j = *i;
    while is_whitespace(at(b, j)) {
        j += 1;
    }
    let d = if b[j..].starts_with(b"section") {
        j += 7;
        Directive::Section
    } else if b[j..].starts_with(b"item") {
        j += 4;
        Directive::Item
    } else if b[j..].starts_with(b"list") {
        j += 4;
        Directive::List
    } else {
        return Directive::Invalid;
    };
    if !is_whitespace_or_end(at(b, j)) {
        return Directive::Invalid;
    }
    *i = j;
    d
}

fn at(b: &[u8], i: usize) -> u8 {
    b.get(i).copied().unwrap_or(0)
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// `config_file.c:parse_field` (quoted or bare field).
fn parse_field(b: &[u8], i: &mut usize) -> Option<Vec<u8>> {
    let mut j = *i;
    while is_whitespace(at(b, j)) {
        j += 1;
    }
    let mut out = Vec::new();
    let q = at(b, j);
    if q == b'"' || q == b'\'' {
        loop {
            j += 1;
            let c = at(b, j);
            if c == q {
                j += 1;
                break;
            } else if c == b'\\' {
                j += 1;
                let c = at(b, j);
                if c == q {
                    out.push(q);
                } else {
                    match c {
                        b't' => out.push(b'\t'),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b'\\' => out.push(b'\\'),
                        b'x' => {
                            j += 1;
                            let h = hex(at(b, j))?;
                            j += 1;
                            let l = hex(at(b, j))?;
                            out.push((h << 4) | l);
                        }
                        _ => return None,
                    }
                }
            } else if (b' '..=b'~').contains(&c) {
                out.push(c);
            } else {
                return None;
            }
        }
    } else {
        let start = j;
        loop {
            let c = at(b, j);
            if is_whitespace_or_end(c) {
                break;
            } else if c <= b' ' || c > b'~' || c == b'#' || c == b'\'' || c == b'"' {
                return None;
            }
            j += 1;
        }
        if j == start {
            return None;
        }
        out.extend_from_slice(&b[start..j]);
    }
    if !is_whitespace_or_end(at(b, j)) {
        return None;
    }
    *i = j;
    Some(out)
}

fn text(v: Vec<u8>) -> String {
    v.into_iter().map(|c| c as char).collect()
}

impl ConfigFile {
    /// `config_file.c:config_parse` (invalid lines ignored).
    pub fn parse(data: &[u8]) -> ConfigFile {
        let mut cfg = ConfigFile::default();
        let mut section: Option<usize> = None;
        let mut option: Option<usize> = None;
        let mut start = 0;
        while start < data.len() {
            let mut end = start;
            while end < data.len() && data[end] != b'\n' && data[end] != b'\r' {
                end += 1;
            }
            let line = &data[start..end];
            start = (end + 1).min(data.len()).max(end + 1);
            let mut i = 0;
            match match_directive(line, &mut i) {
                Directive::Invalid => {}
                Directive::Section => {
                    let Some(kind) = parse_field(line, &mut i) else {
                        continue;
                    };
                    let name = parse_field(line, &mut i).map(text);
                    cfg.sections.push(Section { kind: text(kind), name, options: Vec::new() });
                    section = Some(cfg.sections.len() - 1);
                    option = None;
                }
                d @ (Directive::Item | Directive::List) => {
                    let Some(s) = section else { continue };
                    let Some(key) = parse_field(line, &mut i) else { continue };
                    let Some(value) = parse_field(line, &mut i) else {
                        continue;
                    };
                    let (key, value) = (text(key), text(value));
                    let sec = &mut cfg.sections[s];
                    if d == Directive::Item {
                        option = Some(set_option(sec, &key, Value::Item(value)));
                    } else {
                        let o = match option {
                            Some(o) if sec.options[o].key == key => o,
                            _ => get_or_set_option(sec, &key, Value::List(Vec::new())),
                        };
                        add_value(&mut sec.options[o], value);
                        option = Some(o);
                    }
                }
            }
        }
        cfg
    }

    /// `config_file.c:config_write`.
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for s in &self.sections {
            out.extend_from_slice(b"section ");
            write_field(&mut out, &s.kind);
            if let Some(n) = &s.name {
                out.push(b' ');
                write_field(&mut out, n);
            }
            out.push(b'\n');
            for o in &s.options {
                match &o.value {
                    Value::Item(v) => {
                        out.extend_from_slice(b"\titem ");
                        write_field(&mut out, &o.key);
                        out.push(b' ');
                        write_field(&mut out, v);
                        out.push(b'\n');
                    }
                    Value::List(vs) => {
                        for v in vs {
                            out.extend_from_slice(b"\tlist ");
                            write_field(&mut out, &o.key);
                            out.push(b' ');
                            write_field(&mut out, v);
                            out.push(b'\n');
                        }
                    }
                }
            }
            out.push(b'\n');
        }
        out
    }

    /// `config_find_section` (name `None`: unnamed section only).
    fn find_or_add_section(&mut self, kind: &str, name: Option<&str>) -> usize {
        if let Some(i) = self.sections.iter().position(|s| s.kind == kind && s.name.as_deref() == name) {
            return i;
        }
        self.sections.push(Section { kind: kind.into(), name: name.map(Into::into), options: Vec::new() });
        self.sections.len() - 1
    }
}

/// `config_write`: `write_field`.
fn write_field(out: &mut Vec<u8>, s: &str) {
    out.push(b'\'');
    for ch in s.chars() {
        let c = ch as u32 as u8;
        match c {
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\'' | b'\\' => {
                out.push(b'\\');
                out.push(c);
            }
            b' '..=b'~' => out.push(c),
            _ => {
                out.extend_from_slice(b"\\x");
                out.push(b"0123456789abcdef"[(c >> 4) as usize]);
                out.push(b"0123456789abcdef"[(c & 15) as usize]);
            }
        }
    }
    out.push(b'\'');
}

/// `config_set_option`: replaces the value or adds the option.
fn set_option(sec: &mut Section, key: &str, value: Value) -> usize {
    if let Some(i) = sec.options.iter().position(|o| o.key == key) {
        sec.options[i].value = value;
        return i;
    }
    sec.options.push(Opt { key: key.into(), value });
    sec.options.len() - 1
}

/// `config_get_or_set_option`.
fn get_or_set_option(sec: &mut Section, key: &str, value: Value) -> usize {
    if let Some(i) = sec.options.iter().position(|o| o.key == key) {
        return i;
    }
    sec.options.push(Opt { key: key.into(), value });
    sec.options.len() - 1
}

/// `config_add_value` (an `item` becomes a list of two values).
fn add_value(o: &mut Opt, v: String) {
    match &mut o.value {
        Value::Item(old) => o.value = Value::List(vec![std::mem::take(old), v]),
        Value::List(vs) => vs.push(v),
    }
}

fn values(o: &Opt) -> Vec<String> {
    match &o.value {
        Value::Item(v) => vec![v.clone()],
        Value::List(vs) => vs.clone(),
    }
}

const BOOL_VALUES: [[&str; 2]; 4] = [["0", "1"], ["no", "yes"], ["off", "on"], ["false", "true"]];
const NO_YES: usize = 1;
const OFF_ON: usize = 2;

fn item_value<'a>(sec: &'a Section, key: &str) -> Option<&'a str> {
    match &sec.options.iter().find(|o| o.key == key)?.value {
        Value::Item(v) => Some(v),
        Value::List(_) => None,
    }
}

/// `config_get_or_set_bool_option`.
fn get_or_set_bool(sec: &mut Section, key: &str, def: bool, style: usize) -> bool {
    if let Some(v) = item_value(sec, key) {
        for pair in BOOL_VALUES {
            for (j, s) in pair.iter().enumerate() {
                if *s == v {
                    return j == 1;
                }
            }
        }
    }
    set_option(sec, key, Value::Item(BOOL_VALUES[style][def as usize].into()));
    def
}

/// `sscanf(value, "%i%n")` on the whole string.
fn parse_c_int(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let (radix, digits_start) = if b[i..].starts_with(b"0x") || b[i..].starts_with(b"0X") {
        if b.get(i + 2).is_some_and(|c| c.is_ascii_hexdigit()) {
            (16, i + 2)
        } else {
            (8, i + 1) // "0x" without digits: sscanf reads "0" then stops
        }
    } else if b.get(i) == Some(&b'0') {
        (8, i)
    } else {
        (10, i)
    };
    let mut j = digits_start;
    let mut v: i64 = 0;
    while j < b.len() && (b[j] as char).is_digit(radix) {
        v = (v * radix as i64 + (b[j] as char).to_digit(radix).unwrap() as i64).min(1 << 40);
        j += 1;
    }
    if j == digits_start && radix != 8 {
        return None;
    }
    if j != b.len() {
        return None;
    }
    let v = if neg { -v } else { v };
    Some(v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// `config_get_or_set_int_option`.
fn get_or_set_int(sec: &mut Section, key: &str, def: i32) -> i32 {
    if let Some(v) = item_value(sec, key).and_then(parse_c_int) {
        return v;
    }
    set_option(sec, key, Value::Item(def.to_string()));
    def
}

/// `destruct.c:config_int_bounded` (out of bounds: default, file unchanged).
fn int_bounded(sec: &mut Section, key: &str, def: i32, lo: i32, hi: i32, warnings: &mut Vec<String>) -> i32 {
    let v = get_or_set_int(sec, key, def);
    if v < lo || v > hi {
        warnings.push(format!("destruct: '{key}' = {v} is outside [{lo}, {hi}], using default {def}"));
        return def;
    }
    v
}

/// Default keys (`defaultKeyConfig`, SDL_Scancode codes).
pub const DEFAULT_KEYS: [[[u16; 4]; MAX_MOVE]; MAX_PLAYERS] = [
    [
        [6, 0, 0, 0],
        [25, 0, 0, 0],
        [4, 0, 0, 0],
        [29, 0, 0, 0],
        [226, 0, 0, 0],
        [27, 225, 0, 0],
        [224, 0, 0, 0],
        [44, 0, 0, 0],
    ],
    [
        [80, 92, 0, 0],
        [79, 94, 0, 0],
        [82, 96, 0, 0],
        [81, 90, 0, 0],
        [49, 93, 0, 0],
        [73, 40, 98, 88],
        [75, 97, 0, 0],
        [78, 91, 0, 0],
    ],
];

const PLAYER_NAMES: [&str; 2] = ["left", "right"];
const KEY_NAMES: [&str; MAX_MOVE] = ["left", "right", "up", "down", "change", "fire", "previous weapon", "next weapon"];

/// `SDL_GetScancodeName` (SDL 2) for the named codes.
pub const SCANCODE_NAMES: &[(u16, &str)] = &[
    (4, "A"),
    (5, "B"),
    (6, "C"),
    (7, "D"),
    (8, "E"),
    (9, "F"),
    (10, "G"),
    (11, "H"),
    (12, "I"),
    (13, "J"),
    (14, "K"),
    (15, "L"),
    (16, "M"),
    (17, "N"),
    (18, "O"),
    (19, "P"),
    (20, "Q"),
    (21, "R"),
    (22, "S"),
    (23, "T"),
    (24, "U"),
    (25, "V"),
    (26, "W"),
    (27, "X"),
    (28, "Y"),
    (29, "Z"),
    (30, "1"),
    (31, "2"),
    (32, "3"),
    (33, "4"),
    (34, "5"),
    (35, "6"),
    (36, "7"),
    (37, "8"),
    (38, "9"),
    (39, "0"),
    (40, "Return"),
    (41, "Escape"),
    (42, "Backspace"),
    (43, "Tab"),
    (44, "Space"),
    (45, "-"),
    (46, "="),
    (47, "["),
    (48, "]"),
    (49, "\\"),
    (50, "#"),
    (51, ";"),
    (52, "'"),
    (53, "`"),
    (54, ","),
    (55, "."),
    (56, "/"),
    (57, "CapsLock"),
    (58, "F1"),
    (59, "F2"),
    (60, "F3"),
    (61, "F4"),
    (62, "F5"),
    (63, "F6"),
    (64, "F7"),
    (65, "F8"),
    (66, "F9"),
    (67, "F10"),
    (68, "F11"),
    (69, "F12"),
    (70, "PrintScreen"),
    (71, "ScrollLock"),
    (72, "Pause"),
    (73, "Insert"),
    (74, "Home"),
    (75, "PageUp"),
    (76, "Delete"),
    (77, "End"),
    (78, "PageDown"),
    (79, "Right"),
    (80, "Left"),
    (81, "Down"),
    (82, "Up"),
    (83, "Numlock"),
    (84, "Keypad /"),
    (85, "Keypad *"),
    (86, "Keypad -"),
    (87, "Keypad +"),
    (88, "Keypad Enter"),
    (89, "Keypad 1"),
    (90, "Keypad 2"),
    (91, "Keypad 3"),
    (92, "Keypad 4"),
    (93, "Keypad 5"),
    (94, "Keypad 6"),
    (95, "Keypad 7"),
    (96, "Keypad 8"),
    (97, "Keypad 9"),
    (98, "Keypad 0"),
    (99, "Keypad ."),
    (101, "Application"),
    (102, "Power"),
    (103, "Keypad ="),
    (104, "F13"),
    (105, "F14"),
    (106, "F15"),
    (107, "F16"),
    (108, "F17"),
    (109, "F18"),
    (110, "F19"),
    (111, "F20"),
    (112, "F21"),
    (113, "F22"),
    (114, "F23"),
    (115, "F24"),
    (116, "Execute"),
    (117, "Help"),
    (118, "Menu"),
    (119, "Select"),
    (120, "Stop"),
    (121, "Again"),
    (122, "Undo"),
    (123, "Cut"),
    (124, "Copy"),
    (125, "Paste"),
    (126, "Find"),
    (127, "Mute"),
    (128, "VolumeUp"),
    (129, "VolumeDown"),
    (133, "Keypad ,"),
    (134, "Keypad = (AS400)"),
    (153, "AltErase"),
    (154, "SysReq"),
    (155, "Cancel"),
    (156, "Clear"),
    (157, "Prior"),
    (158, "Return"),
    (159, "Separator"),
    (160, "Out"),
    (161, "Oper"),
    (162, "Clear / Again"),
    (163, "CrSel"),
    (164, "ExSel"),
    (176, "Keypad 00"),
    (177, "Keypad 000"),
    (178, "ThousandsSeparator"),
    (179, "DecimalSeparator"),
    (180, "CurrencyUnit"),
    (181, "CurrencySubUnit"),
    (182, "Keypad ("),
    (183, "Keypad )"),
    (184, "Keypad {"),
    (185, "Keypad }"),
    (186, "Keypad Tab"),
    (187, "Keypad Backspace"),
    (188, "Keypad A"),
    (189, "Keypad B"),
    (190, "Keypad C"),
    (191, "Keypad D"),
    (192, "Keypad E"),
    (193, "Keypad F"),
    (194, "Keypad XOR"),
    (195, "Keypad ^"),
    (196, "Keypad %"),
    (197, "Keypad <"),
    (198, "Keypad >"),
    (199, "Keypad &"),
    (200, "Keypad &&"),
    (201, "Keypad |"),
    (202, "Keypad ||"),
    (203, "Keypad :"),
    (204, "Keypad #"),
    (205, "Keypad Space"),
    (206, "Keypad @"),
    (207, "Keypad !"),
    (208, "Keypad MemStore"),
    (209, "Keypad MemRecall"),
    (210, "Keypad MemClear"),
    (211, "Keypad MemAdd"),
    (212, "Keypad MemSubtract"),
    (213, "Keypad MemMultiply"),
    (214, "Keypad MemDivide"),
    (215, "Keypad +/-"),
    (216, "Keypad Clear"),
    (217, "Keypad ClearEntry"),
    (218, "Keypad Binary"),
    (219, "Keypad Octal"),
    (220, "Keypad Decimal"),
    (221, "Keypad Hexadecimal"),
    (224, "Left Ctrl"),
    (225, "Left Shift"),
    (226, "Left Alt"),
    (227, "Left GUI"),
    (228, "Right Ctrl"),
    (229, "Right Shift"),
    (230, "Right Alt"),
    (231, "Right GUI"),
    (257, "ModeSwitch"),
    (258, "AudioNext"),
    (259, "AudioPrev"),
    (260, "AudioStop"),
    (261, "AudioPlay"),
    (262, "AudioMute"),
    (263, "MediaSelect"),
    (264, "WWW"),
    (265, "Mail"),
    (266, "Calculator"),
    (267, "Computer"),
    (268, "AC Search"),
    (269, "AC Home"),
    (270, "AC Back"),
    (271, "AC Forward"),
    (272, "AC Stop"),
    (273, "AC Refresh"),
    (274, "AC Bookmarks"),
    (275, "BrightnessDown"),
    (276, "BrightnessUp"),
    (277, "DisplaySwitch"),
    (278, "KBDIllumToggle"),
    (279, "KBDIllumDown"),
    (280, "KBDIllumUp"),
    (281, "Eject"),
    (282, "Sleep"),
    (283, "App1"),
    (284, "App2"),
    (285, "AudioRewind"),
    (286, "AudioFastForward"),
    (287, "SoftLeft"),
    (288, "SoftRight"),
    (289, "Call"),
    (290, "EndCall"),
];

/// `SDL_GetScancodeFromName`: case-insensitive comparison, first code found.
pub fn scancode_from_name(name: &str) -> u16 {
    SCANCODE_NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(c, _)| *c).unwrap_or(0)
}

/// `SDL_GetScancodeName`.
pub fn scancode_name(code: u16) -> &'static str {
    SCANCODE_NAMES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n).unwrap_or("")
}

/// `destruct.c:load_destruct_config` applied to the content of `destruct.cfg`
/// (empty if it does not exist). Returns the configuration, the file to rewrite and
/// the warnings (out-of-bounds values).
pub fn load(data: &[u8]) -> (DestructConfig, Vec<u8>, Vec<String>) {
    let mut file = ConfigFile::parse(data);
    let mut cfg = DestructConfig::default();
    let mut warnings = Vec::new();

    let s = file.find_or_add_section("destruct", None);
    let sec = &mut file.sections[s];
    cfg.alwaysalias = get_or_set_bool(sec, "antialias craters", false, NO_YES);
    cfg.tracer_laser = get_or_set_bool(sec, "tracer laser", false, OFF_ON);
    cfg.max_shots = int_bounded(sec, "max shots", 40, 1, 1000, &mut warnings) as u32;
    cfg.max_explosions = int_bounded(sec, "max explosions", 40, 1, 1000, &mut warnings) as u32;
    cfg.min_walls = int_bounded(sec, "min walls", 20, 0, 200, &mut warnings) as u32;
    cfg.max_walls = int_bounded(sec, "max walls", 20, 1, 200, &mut warnings) as u32;
    if cfg.min_walls > cfg.max_walls {
        warnings.push(format!(
            "destruct: 'min walls' ({}) > 'max walls' ({}), using defaults 20/20",
            cfg.min_walls, cfg.max_walls
        ));
        cfg.min_walls = 20;
        cfg.max_walls = 20;
    }
    cfg.ai[0] = get_or_set_bool(sec, "left ai", true, NO_YES);
    cfg.jumper_straight[0] = get_or_set_bool(sec, "left jumper fires straight", true, NO_YES);
    cfg.ai[1] = get_or_set_bool(sec, "right ai", false, NO_YES);
    cfg.jumper_straight[1] = get_or_set_bool(sec, "right jumper fires straight", false, NO_YES);

    for (p, player_name) in PLAYER_NAMES.iter().enumerate().take(MAX_PLAYERS) {
        let s = file.find_or_add_section("destruct keyboard", Some(player_name));
        let sec = &mut file.sections[s];
        for (k, key_name) in KEY_NAMES.iter().enumerate().take(MAX_MOVE) {
            let o = get_or_set_option(sec, key_name, Value::List(Vec::new()));
            let mut kept = Vec::new();
            for v in values(&sec.options[o]) {
                let key = scancode_from_name(&v);
                if key != 0 && kept.len() < 4 {
                    cfg.keys[p][k][kept.len()] = key;
                    kept.push(v);
                }
            }
            if !kept.is_empty() {
                for i in kept.len()..4 {
                    cfg.keys[p][k][i] = 0;
                }
                // A single remaining value keeps its form (item or list).
                if !matches!(&sec.options[o].value, Value::Item(_)) || kept.len() != 1 {
                    sec.options[o].value = Value::List(kept);
                }
            } else {
                sec.options[o].value = Value::List(Vec::new());
                for i in 0..4 {
                    if cfg.keys[p][k][i] != 0 {
                        add_value(&mut sec.options[o], scancode_name(cfg.keys[p][k][i]).into());
                    }
                }
            }
        }
    }

    let s = file.find_or_add_section("destruct custom", None);
    let sec = &mut file.sections[s];
    cfg.allow_custom = get_or_set_bool(sec, "enable", false, NO_YES);
    for p in 0..MAX_PLAYERS {
        let key = format!("{} num units", PLAYER_NAMES[p]);
        let def = cfg.custom_units[p][0] as i32;
        cfg.custom_units[p][0] = int_bounded(sec, &key, def, 1, 10, &mut warnings) as u8;
        let key = format!("{} unit", PLAYER_NAMES[p]);
        let o = get_or_set_option(sec, &key, Value::List(Vec::new()));
        let mut kept = Vec::new();
        for v in values(&sec.options[o]) {
            if let Some(u) = UNIT_NAMES.iter().position(|n| *n == v) {
                if 1 + kept.len() < 11 {
                    cfg.custom_units[p][1 + kept.len()] = u as u8;
                    kept.push(v);
                }
            }
        }
        if !kept.is_empty() {
            let single_item = matches!(&sec.options[o].value, Value::Item(_)) && kept.len() == 1;
            if !single_item {
                sec.options[o].value = Value::List(kept.clone());
            }
            for i in kept.len()..10 {
                cfg.custom_units[p][1 + i] = UNIT_TANK as u8;
                add_value(&mut sec.options[o], UNIT_NAMES[UNIT_TANK as usize].into());
            }
        } else {
            sec.options[o].value = Value::List(Vec::new());
            for i in 0..10 {
                add_value(&mut sec.options[o], UNIT_NAMES[cfg.custom_units[p][1 + i] as usize].into());
            }
        }
    }
    let out = file.write();
    (cfg, out, warnings)
}
