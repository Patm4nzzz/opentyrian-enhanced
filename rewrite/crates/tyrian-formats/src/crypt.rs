//! Encrypted Pascal strings (`helptext.c:read_encrypted_pascal_string`,
//! `decrypt_string`) and files made only of such strings
//! (`levelsN.dat`, `cubetxtN.dat`, `tyrian.cdt`, help area of `tyrian.hdt`).

use crate::io::{FormatError, Reader, Result, Writer};
use serde::Serialize;

/// Key of `helptext.c:decrypt_string`.
pub const CRYPT_KEY: [u8; 10] = [204, 129, 63, 255, 71, 19, 25, 62, 1, 99];

/// Decrypts encrypted bytes (from the end to the start, like the C code).
pub fn decrypt(cipher: &[u8]) -> Vec<u8> {
    let mut out = cipher.to_vec();
    for i in (0..out.len()).rev() {
        out[i] ^= CRYPT_KEY[i % CRYPT_KEY.len()];
        if i > 0 {
            out[i] ^= cipher[i - 1];
        }
    }
    out
}

/// Encrypts a plaintext (exact inverse of [`decrypt`]).
pub fn encrypt(plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(plain.len());
    for (i, &p) in plain.iter().enumerate() {
        let mut c = p ^ CRYPT_KEY[i % CRYPT_KEY.len()];
        if i > 0 {
            c ^= out[i - 1];
        }
        out.push(c);
    }
    out
}

/// Converts game bytes to a `String` (1 byte = 1 Latin-1 character;
/// reversible, the game fonts index the byte directly).
pub fn bytes_to_text(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// An encrypted Pascal string. The original encrypted bytes are kept (the
/// length is implicit: `cipher.len()`, always ≤ 255).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncryptedString {
    pub cipher: Vec<u8>,
}

impl EncryptedString {
    pub fn read(r: &mut Reader) -> Result<Self> {
        let len = r.u8()? as usize;
        Ok(Self { cipher: r.bytes(len)?.to_vec() })
    }

    pub fn write(&self, w: &mut Writer) {
        w.u8(self.cipher.len() as u8);
        w.bytes(&self.cipher);
    }

    /// Builds from a plaintext (≤ 255 bytes).
    pub fn from_plain(plain: &[u8]) -> Result<Self> {
        if plain.len() > 255 {
            return Err(FormatError::new("Pascal string > 255 bytes"));
        }
        Ok(Self { cipher: encrypt(plain) })
    }

    /// Plaintext (raw bytes).
    pub fn plain(&self) -> Vec<u8> {
        decrypt(&self.cipher)
    }

    /// Plaintext as a `String` (Latin-1).
    pub fn text(&self) -> String {
        bytes_to_text(&self.plain())
    }
}

/// File = contiguous sequence of encrypted Pascal strings, without a header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncryptedStringFile {
    pub strings: Vec<EncryptedString>,
}

impl EncryptedStringFile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut strings = Vec::new();
        while r.remaining() > 0 {
            strings.push(EncryptedString::read(&mut r)?);
        }
        Ok(Self { strings })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::new();
        for s in &self.strings {
            s.write(&mut w);
        }
        w.into_inner()
    }

    /// All decrypted lines.
    pub fn lines(&self) -> Vec<String> {
        self.strings.iter().map(|s| s.text()).collect()
    }
}

/// Equivalent of `atoi(s + offset)` on a C string: leading spaces,
/// optional sign, digits. Beyond the end of the string → 0.
pub fn atoi_at(s: &[u8], offset: usize) -> i32 {
    if offset >= s.len() {
        return 0;
    }
    let mut i = offset;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = (v * 10 + (s[i] - b'0') as i64).min(i32::MAX as i64 + 1);
        i += 1;
    }
    let v = if neg { -v } else { v };
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// Equivalent of the successive calls of `mainint.c:str_pop_int` (strtol in
/// a loop): extracts the integers as long as strtol consumes something.
pub fn pop_ints(s: &[u8]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let start = i;
        let mut j = i;
        while j < s.len() && matches!(s[j], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
            j += 1;
        }
        let mut k = j;
        if k < s.len() && (s[k] == b'+' || s[k] == b'-') {
            k += 1;
        }
        let digits_start = k;
        while k < s.len() && s[k].is_ascii_digit() {
            k += 1;
        }
        if k == digits_start {
            // strtol consumed nothing: stop (end == str).
            let _ = start;
            break;
        }
        out.push(atoi_at(s, j));
        i = k;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypt_roundtrip() {
        let p = b"]L[ 9999 048 SAVARA   21 20".to_vec();
        assert_eq!(decrypt(&encrypt(&p)), p);
    }

    #[test]
    fn atoi_like_c() {
        assert_eq!(atoi_at(b"]L[ 9999 048", 4), 9999);
        assert_eq!(atoi_at(b"  -12x", 0), -12);
        assert_eq!(atoi_at(b"abc", 0), 0);
        assert_eq!(atoi_at(b"abc", 10), 0);
        assert_eq!(pop_ints(b" 1 2  33 x 4"), vec![1, 2, 33]);
    }
}
