//! Byte-exact reading and writing of the original data formats of
//! Tyrian 2000 (see `docs/formats-spec.md`). No dependency on Godot.
//!
//! Each format exposes `parse(&[u8]) -> Result<T>` and `T::write() -> Vec<u8>`
//! with the guarantee `write(parse(b)) == b` on the shipped files.

pub mod achievements;
pub mod campaign_save;
pub mod container;
pub mod crypt;
pub mod cube;
pub mod demo;
pub mod episode;
pub mod hdt;
pub mod io;
pub mod lvl;
pub mod misc;
pub mod music;
pub mod ot_dat;
pub mod palette;
pub mod pic;
pub mod replay;
pub mod save;
pub mod snd;
pub mod sprite;
pub mod tiles;

pub use io::{FormatError, Result};

/// File type recognized from its name (lowercase).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// `tyrianN.lvl` (N = episode).
    Lvl(u8),
    /// `levelsN.dat`.
    EpisodeScript(u8),
    /// `cubetxtN.dat`.
    CubeText(u8),
    /// `tyrian.cdt`.
    Credits,
    /// `tyrian.hdt`.
    Hdt,
    /// `shapesX.dat`.
    TileSet,
    /// `palette.dat`.
    Palette,
    /// `tyrian.pic`.
    Pic,
    /// PCX file (`tshp2.pcx`, `shipedit.pcx`).
    Pcx,
    /// `tyrian.shp`, `tyrianc.shp`.
    MainShapes,
    /// `newsh?.shp` (except `newsh$.shp`).
    Sprite2,
    /// `newsh$.shp`.
    ExtraShips,
    /// `estsc.shp`, `estpa.shp`.
    SpriteTable,
    /// `tyrian.snd`, `voices.snd`, `voicesc.snd`.
    Sound,
    /// `music.mus`.
    Music,
    /// `demo.N`.
    Demo,
    /// `tyrend.anm`.
    Anm,
    /// `exitmsg.bin`.
    TextScreen,
}

/// Classifies a file name; `None` if the format is not supported.
pub fn classify(file_name: &str) -> Option<FileKind> {
    let n = file_name.to_ascii_lowercase();
    let digit = |s: &str| s.parse::<u8>().ok();
    if let Some(rest) = n.strip_prefix("tyrian").and_then(|r| r.strip_suffix(".lvl")) {
        return digit(rest).map(FileKind::Lvl);
    }
    if let Some(rest) = n.strip_prefix("levels").and_then(|r| r.strip_suffix(".dat")) {
        return digit(rest).map(FileKind::EpisodeScript);
    }
    if let Some(rest) = n.strip_prefix("cubetxt").and_then(|r| r.strip_suffix(".dat")) {
        return digit(rest).map(FileKind::CubeText);
    }
    if let Some(rest) = n.strip_prefix("shapes").and_then(|r| r.strip_suffix(".dat")) {
        return (rest.chars().count() == 1).then_some(FileKind::TileSet);
    }
    if let Some(rest) = n.strip_prefix("newsh").and_then(|r| r.strip_suffix(".shp")) {
        return match rest {
            "$" => Some(FileKind::ExtraShips),
            r if r.chars().count() == 1 => Some(FileKind::Sprite2),
            _ => None,
        };
    }
    if let Some(rest) = n.strip_prefix("demo.") {
        return digit(rest).map(|_| FileKind::Demo);
    }
    match n.as_str() {
        "tyrian.cdt" => Some(FileKind::Credits),
        "tyrian.hdt" => Some(FileKind::Hdt),
        "palette.dat" => Some(FileKind::Palette),
        "tyrian.pic" => Some(FileKind::Pic),
        "tshp2.pcx" | "shipedit.pcx" => Some(FileKind::Pcx),
        "tyrian.shp" | "tyrianc.shp" => Some(FileKind::MainShapes),
        "estsc.shp" | "estpa.shp" => Some(FileKind::SpriteTable),
        "tyrian.snd" | "voices.snd" | "voicesc.snd" => Some(FileKind::Sound),
        "music.mus" => Some(FileKind::Music),
        "tyrend.anm" => Some(FileKind::Anm),
        "exitmsg.bin" => Some(FileKind::TextScreen),
        _ => None,
    }
}

/// Parses then rewrites the bytes according to the type: used for round-trip
/// tests (the result must be identical to the input).
pub fn reencode(kind: FileKind, data: &[u8]) -> Result<Vec<u8>> {
    Ok(match kind {
        FileKind::Lvl(_) => lvl::LvlFile::parse(data)?.write(),
        FileKind::EpisodeScript(_) => episode::EpisodeScript::parse(data)?.write(),
        FileKind::CubeText(_) | FileKind::Credits => crypt::EncryptedStringFile::parse(data)?.write(),
        FileKind::Hdt => hdt::HdtFile::parse(data)?.write(),
        FileKind::TileSet => tiles::TileSet::parse(data)?.write(),
        FileKind::Palette => palette::PaletteFile::parse(data)?.write(),
        FileKind::Pic => pic::PicFile::parse(data)?.write(),
        FileKind::Pcx => pic::PcxFile::parse(data)?.write(),
        FileKind::MainShapes => sprite::MainShapes::parse(data)?.write(),
        FileKind::Sprite2 => sprite::Sprite2Sheet::parse(data)?.write(),
        FileKind::ExtraShips => sprite::ExtraShipsFile::parse(data)?.write(),
        FileKind::SpriteTable => sprite::SpriteTable::parse(data)?.write(),
        FileKind::Sound => snd::SoundFile::parse(data)?.write(),
        FileKind::Music => music::MusicFile::parse(data)?.write(),
        FileKind::Demo => demo::Demo::parse(data)?.write(),
        FileKind::Anm => misc::AnmFile::parse(data)?.write(),
        FileKind::TextScreen => misc::TextScreen::parse(data)?.write(),
    })
}
