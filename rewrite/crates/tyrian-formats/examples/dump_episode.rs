//! Prints the decrypted script of a `levelsN.dat` (diagnostic tool).
fn main() {
    let path = std::env::args().nth(1).expect("path to levelsN.dat");
    let data = std::fs::read(&path).expect("read");
    let f = tyrian_formats::crypt::EncryptedStringFile::parse(&data).expect("format");
    for (i, l) in f.lines().iter().enumerate() {
        println!("{i:4} {l}");
    }
}
