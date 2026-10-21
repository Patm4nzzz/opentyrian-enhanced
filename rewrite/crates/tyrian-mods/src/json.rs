//! Bounded JSON5 reader for mod files (untrusted data).
//!
//! Same dialect as the C version (`json5.c:cJSON_Parse5`): strict JSON plus
//! `//` and `/* */` comments and trailing commas. Like cJSON:
//! - key order is kept and a duplicate key keeps the FIRST
//!   value (`cJSON_GetObjectItemCaseSensitive` returns the first);
//! - numbers are `f64` (`strtod`), converted to integer by
//!   [`Value::as_c_int`] like `cJSON.valueint`.
//!
//! Bounds (ADR 0080): text size, depth, string length and
//! total node count are capped; no input can cause a
//! panic or unbounded recursion.

use std::fmt;

/// Maximum size of a mod JSON file (1 MiB, the largest shipped mod is ~60 KiB).
pub const MAX_TEXT_BYTES: usize = 1 << 20;
pub const MAX_DEPTH: usize = 48;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_NODES: usize = 200_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.column, self.message)
    }
}

impl Value {
    /// `cJSON_GetObjectItemCaseSensitive`: first equal key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(kv) => Some(kv),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// `cJSON.valueint`: saturation to `i32`, otherwise truncation toward zero.
    pub fn as_c_int(&self) -> Option<i32> {
        let n = self.as_f64()?;
        Some(c_int_of(n))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}

/// cJSON `double` → `int` conversion (`cJSON_SetNumberHelper`).
pub fn c_int_of(n: f64) -> i32 {
    if n.is_nan() {
        0
    } else if n >= i32::MAX as f64 {
        i32::MAX
    } else if n <= i32::MIN as f64 {
        i32::MIN
    } else {
        n as i32
    }
}

pub fn parse(text: &str) -> Result<Value, ParseError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(ParseError {
            line: 1,
            column: 1,
            message: format!("file too large ({} bytes, maximum {})", text.len(), MAX_TEXT_BYTES),
        });
    }
    let mut p = Parser { s: text.as_bytes(), i: 0, nodes: 0 };
    // UTF-8 BOM tolerated.
    if p.s.starts_with(&[0xEF, 0xBB, 0xBF]) {
        p.i = 3;
    }
    p.skip_ws()?;
    let v = p.value(0)?;
    p.skip_ws()?;
    if p.i != p.s.len() {
        return Err(p.err("content after the end of the document"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> ParseError {
        let upto = &self.s[..self.i.min(self.s.len())];
        let line = upto.iter().filter(|&&c| c == b'\n').count() + 1;
        let column = upto.iter().rev().take_while(|&&c| c != b'\n').count() + 1;
        ParseError { line, column, message: msg.to_string() }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.i += 1,
                Some(b'/') if self.s.get(self.i + 1) == Some(&b'/') => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.i += 1;
                    }
                }
                Some(b'/') if self.s.get(self.i + 1) == Some(&b'*') => {
                    self.i += 2;
                    loop {
                        match self.peek() {
                            None => return Err(self.err("unterminated /* comment")),
                            Some(b'*') if self.s.get(self.i + 1) == Some(&b'/') => {
                                self.i += 2;
                                break;
                            }
                            _ => self.i += 1,
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(self.err("too many elements"));
        }
        match self.peek() {
            None => Err(self.err("unexpected end of file")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("value expected")),
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, ParseError> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("value expected"))
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let digits = |p: &mut Self| {
            let s = p.i;
            while matches!(p.peek(), Some(b'0'..=b'9')) {
                p.i += 1;
            }
            p.i - s
        };
        if digits(self) == 0 {
            return Err(self.err("invalid number"));
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if digits(self) == 0 {
                return Err(self.err("invalid number"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if digits(self) == 0 {
                return Err(self.err("invalid number"));
            }
        }
        if self.i - start > 64 {
            return Err(self.err("number too long"));
        }
        let lexeme = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("invalid number"))?;
        lexeme.parse::<f64>().map(Value::Number).map_err(|_| self.err("invalid number"))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.i += 1; // opening quote
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated string"));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let cp = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&cp) {
                                if self.s[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err(self.err("invalid surrogate pair"));
                                    }
                                    char::from_u32(0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00))
                                } else {
                                    None
                                }
                            } else {
                                char::from_u32(cp)
                            };
                            let ch = ch.ok_or_else(|| self.err("invalid \\u character"))?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                0..=0x1f => return Err(self.err("control character in a string")),
                _ => out.push(c),
            }
            if out.len() > MAX_STRING_BYTES {
                return Err(self.err("string too long"));
            }
        }
        String::from_utf8(out).map_err(|_| self.err("non-UTF-8 string"))
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let h = self.s.get(self.i..self.i + 4).ok_or_else(|| self.err("truncated \\u escape"))?;
        let t = std::str::from_utf8(h).map_err(|_| self.err("invalid \\u escape"))?;
        let v = u32::from_str_radix(t, 16).map_err(|_| self.err("invalid \\u escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.i += 1;
        let mut items = Vec::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b']') {
                self.i += 1;
                return Ok(Value::Array(items));
            }
            items.push(self.value(depth + 1)?);
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("',' or ']' expected")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.i += 1;
        let mut kv = Vec::new();
        loop {
            self.skip_ws()?;
            match self.peek() {
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(kv));
                }
                Some(b'"') => {}
                _ => return Err(self.err("quoted key expected")),
            }
            let k = self.string()?;
            self.skip_ws()?;
            if self.peek() != Some(b':') {
                return Err(self.err("':' expected"));
            }
            self.i += 1;
            self.skip_ws()?;
            let v = self.value(depth + 1)?;
            kv.push((k, v));
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(kv));
                }
                _ => return Err(self.err("',' or '}' expected")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json5_comments_and_trailing_commas() {
        let v = parse("// head\n{ \"a\": [1, 2,], /* block */ \"b\": \"x//y\", }").unwrap();
        assert_eq!(v.get("a").unwrap().as_array().unwrap().len(), 2);
        assert_eq!(v.get("b").unwrap().as_str(), Some("x//y"));
    }

    #[test]
    fn first_duplicate_key_wins() {
        let v = parse("{\"k\": 1, \"k\": 2}").unwrap();
        assert_eq!(v.get("k").unwrap().as_c_int(), Some(1));
    }

    #[test]
    fn c_int_conversion() {
        assert_eq!(c_int_of(3.9), 3);
        assert_eq!(c_int_of(-3.9), -3);
        assert_eq!(c_int_of(1e20), i32::MAX);
        assert_eq!(c_int_of(-1e20), i32::MIN);
    }

    #[test]
    fn hostile_inputs_are_errors_not_panics() {
        let deep = "[".repeat(10_000);
        assert!(parse(&deep).is_err());
        assert!(parse("{\"a\": \"\\ud800\"}").is_err());
        assert!(parse("{\"a\": 1e999999}").is_ok()); // inf: strtod gives HUGE_VAL
        assert!(parse("/* never closed").is_err());
        assert!(parse("\"\u{1}\"").is_err());
        assert!(parse(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
        for cut in 0..40 {
            let s = "{\"a\": [1, {\"b\": \"c\\u00e9\"}], \"d\": -1.5e3}";
            let _ = parse(&s[..cut.min(s.len())]);
        }
    }
}
