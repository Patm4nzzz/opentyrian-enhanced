//! Byte-exact round-trip on all supported files of `data/`,
//! plus decoding checks (level counts, events, HDT…).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use tyrian_formats::*;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn read(name: &str) -> Vec<u8> {
    fs::read(data_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Files present in `data/` (top level) but deliberately outside the
/// Tyrian binary formats, with the reason.
fn known_unsupported(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.ends_with(".json") {
        return Some("Enhanced-specific JSON (not an original format)");
    }
    match n.as_str() {
        "soundfont.sf2" => Some("third-party SoundFont (FluidSynth), not a Tyrian format"),
        "user1.shp" | "user2.shp" => Some("not referenced by the code, format not established (spec § 4.4)"),
        _ => None,
    }
}

#[test]
fn roundtrip_all_data_files() {
    let mut ok = Vec::new();
    let mut failures = Vec::new();
    let mut uncovered = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(data_dir()).expect("data/").flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if !e.file_type().unwrap().is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        match classify(&name) {
            Some(kind) => {
                let bytes = fs::read(e.path()).unwrap();
                match reencode(kind, &bytes) {
                    Ok(out) if out == bytes => ok.push(name),
                    Ok(out) => {
                        let first =
                            out.iter().zip(&bytes).position(|(a, b)| a != b).unwrap_or(out.len().min(bytes.len()));
                        failures.push(format!(
                            "{name}: difference at byte {first} (size {} vs {})",
                            out.len(),
                            bytes.len()
                        ));
                    }
                    Err(err) => failures.push(format!("{name}: {err}")),
                }
            }
            None => uncovered.push(format!("{name} — {}", known_unsupported(&name).unwrap_or("unknown format"))),
        }
    }
    println!("round-trip OK: {} files", ok.len());
    for f in &ok {
        println!("  ok  {f}");
    }
    println!("not covered: {}", uncovered.len());
    for u in &uncovered {
        println!("  --  {u}");
    }
    assert!(failures.is_empty(), "failures: {failures:#?}");
    for u in &uncovered {
        assert!(!u.ends_with("unknown format"), "unclassified file: {u}");
    }
    assert!(ok.len() >= 70, "too few files tested: {}", ok.len());
}

#[test]
fn lvl_level_counts_and_events() {
    let expected = [(1, 18, 37), (2, 12, 25), (3, 12, 25), (4, 20, 41), (5, 8, 17)];
    for (ep, levels, lvl_num) in expected {
        let f = lvl::LvlFile::parse(&read(&format!("tyrian{ep}.lvl"))).unwrap();
        assert_eq!(f.lvl_num as usize, lvl_num, "ep. {ep}");
        assert_eq!(f.levels.len(), levels, "ep. {ep}: lvlNum/2");
        assert_eq!(f.levels.len(), f.lvl_num as usize / 2);
        assert_eq!(f.extra.len(), 1);
        for (i, l) in f.levels.iter().enumerate() {
            let ev = &l.events;
            assert!(ev.tail.is_empty(), "ep. {ep} lvl. {}: tail after events", i + 1);
            assert!(l.map.tail.is_empty());
            assert!(!ev.events.is_empty() && ev.events.len() <= lvl::EVENT_MAXIMUM);
            assert!(ev.enemies.len() <= lvl::LEVEL_ENEMY_MAX);
            assert!(b"WXYZ)wxyz".contains(&ev.header.shape_file), "letter {}", ev.header.shape_file as char);
            assert_eq!((ev.header.map_x, ev.header.map_x2, ev.header.map_x3), (1, 1, 1));
            // Increasing times (the C code walks the list in order).
            assert!(ev.events.windows(2).all(|w| w[0].eventtime <= w[1].eventtime), "ep. {ep} lvl. {}", i + 1);
            // Every cell of the layers references a non-zero mapSh entry ≤ 71.
            for layer in 0..3 {
                for &c in l.map.layer(layer) {
                    assert!(c <= 71);
                    assert!((1..=600).contains(&l.map.map_sh[layer][c as usize]));
                }
            }
        }
        // Last entry: empty (ep. 1–3) or item block (ep. 4–5).
        if ep <= 3 {
            assert!(f.extra[0].is_empty());
            assert!(f.item_tables().is_none());
        } else {
            let items = f.item_tables().unwrap().unwrap();
            assert!(items.tail.is_empty() || items.tail.len() == hdt::EnemyDat::SIZE);
            assert_eq!(items.write(), f.extra[0]);
        }
    }
    let f = lvl::LvlFile::parse(&read("tyrian3.lvl")).unwrap();
    let max = f.levels.iter().map(|l| l.events.events.len()).max().unwrap();
    assert_eq!(max, 2259, "largest maxEvent (spec § 1.2)");
}

#[test]
fn hdt_tables() {
    let f = hdt::HdtFile::parse(&read("tyrian.hdt")).unwrap();
    assert_eq!(f.episode1_data_loc, 21174);
    match &f.help {
        hdt::HelpArea::Strings(s) => assert_eq!(s.strings.len(), 677),
        hdt::HelpArea::Raw(_) => panic!("help area not parsed into strings"),
    }
    let it = &f.items;
    assert_eq!(it.item_num, [818, 60, 6, 18, 37, 11, 850]);
    assert_eq!(it.weapons.len(), 1638);
    assert_eq!(it.enemies.len(), 1701);
    assert_eq!(it.tail.len(), 77);
    assert_eq!(hdt::ITEM_BLOCK_SIZE + 77, 273_895);
    assert_eq!(it.weapon_ports[0].name.text(), "None");
    assert_eq!(it.weapon_ports[1].name.text(), "Pulse-Cannon");
    assert_eq!(it.ships[1].name.text(), "USP Talon   Light Fighter");
    assert_eq!(it.shields[11].name.text(), "MicroCorp HXS Class C");
    assert_eq!(it.generators[6].name.text(), "Gravitron Pulse-Wave");
    assert!(it.enemy(1001).is_some() && it.enemy(1000).is_none() && it.enemy(1851).is_none());
    assert!(it.weapon(1818).is_some() && it.weapon(819).is_none());
}

#[test]
fn episode_scripts() {
    let counts = [(1, 481, 48), (2, 277, 24), (3, 311, 25), (4, 683, 51), (5, 184, 20)];
    let mut total_l = 0;
    let mut total_i = 0;
    for (ep, strings, sections) in counts {
        let s = episode::EpisodeScript::parse(&read(&format!("levels{ep}.dat"))).unwrap();
        assert_eq!(s.strings.strings.len(), strings);
        let secs = s.sections();
        assert_eq!(secs.len(), sections, "ep. {ep}");
        for sec in &secs {
            total_l += sec.levels.len();
            for shop in &sec.shops {
                total_i += 1;
                assert_eq!(shop.categories.len(), 9);
            }
            for l in &sec.levels {
                assert!(l.lvl_file_num >= 1 && l.level_song >= 1, "{}", l.raw);
            }
        }
    }
    // Usage count cited in level_load.c: L=76, I=66.
    assert_eq!(total_l, 76);
    assert_eq!(total_i, 66);
    let s = episode::EpisodeScript::parse(&read("levels1.dat")).unwrap();
    let secs = s.sections();
    let l = &secs[2].levels[0];
    assert_eq!((l.next_level, l.level_name.as_str(), l.level_song, l.lvl_file_num), (4, "TYRIAN   ", 18, 9));
}

#[test]
fn tiles_and_sprites() {
    let blanks =
        [("shapesw.dat", 345), ("shapesx.dat", 278), ("shapesy.dat", 294), ("shapesz.dat", 127), ("shapes).dat", 425)];
    for (name, b) in blanks {
        let t = tiles::TileSet::parse(&read(name)).unwrap();
        assert_eq!(t.tiles.iter().filter(|t| t.is_blank()).count(), b, "{name}");
        assert_eq!(t.tail.len(), 520);
    }
    let m = sprite::MainShapes::parse(&read("tyrian.shp")).unwrap();
    let counts: Vec<usize> = m.simple.iter().map(|t| t.entries.len()).collect();
    assert_eq!(counts, [60, 85, 127, 152, 18, 47, 22]);
    assert!(m.simple.iter().all(|t| t.tail.is_empty()));
    assert!(m.sheets.iter().all(|s| s.count() == 304));
    let est = sprite::SpriteTable::parse(&read("estsc.shp")).unwrap();
    assert_eq!(est.entries.len(), 9);
    // All Sprite2 streams of newsh?.shp end with 0x0F.
    for e in fs::read_dir(data_dir()).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if classify(&n) == Some(FileKind::Sprite2) {
            let s = sprite::Sprite2Sheet::parse(&fs::read(e.path()).unwrap()).unwrap();
            assert_eq!(s.count(), 304, "{n}");
            for i in 1..=304 {
                assert!(s.sprite_pixels(i).unwrap().1, "{n} sprite {i} not terminated");
            }
        }
    }
    let x = sprite::ExtraShipsFile::parse(&read("newsh$.shp")).unwrap();
    assert!(sprite::decrypt_extra_ships(&x.extra_ships).1, "extraShips checks");
    // A decoded simple sprite does have its dimensions.
    let s = m.simple[0].entries.iter().find_map(|e| e.sprite.as_ref()).unwrap();
    let img = s.decode();
    assert_eq!((img.width, img.height), (s.width as usize, s.height as usize));
}

#[test]
fn pic_palette_sound_music_demo() {
    let pal = palette::PaletteFile::parse(&read("palette.dat")).unwrap();
    assert_eq!(pal.palettes.len(), 24);
    assert!(pal.palettes.iter().all(|p| p.raw.iter().flatten().all(|&c| c <= 63)));
    assert_eq!(palette::vga6_to_8(63), 255);

    let p = pic::PicFile::parse(&read("tyrian.pic")).unwrap();
    assert_eq!(p.entries.len(), 14);
    for i in 0..14 {
        let (px, left) = p.decode(i).unwrap();
        assert_eq!(px.len(), 64000);
        assert_eq!(left, 1, "image {i}");
    }
    let pcx = pic::PcxFile::parse(&read("tshp2.pcx")).unwrap();
    assert!(pcx.palette_rgb8().is_some());
    assert_eq!(pcx.decode().unwrap().len(), 64000);

    let sfx = snd::SoundFile::parse(&read("tyrian.snd")).unwrap();
    assert_eq!(sfx.samples.len(), snd::SFX_COUNT);
    for v in ["voices.snd", "voicesc.snd"] {
        assert_eq!(snd::SoundFile::parse(&read(v)).unwrap().samples.len(), snd::VOICE_COUNT);
    }
    let wav = snd::pcm_s8_to_wav(&sfx.samples[0], snd::SAMPLE_RATE);
    assert_eq!(&wav[..4], b"RIFF");

    let mus = music::MusicFile::parse(&read("music.mus")).unwrap();
    assert_eq!(mus.songs.len(), 41);
    assert_eq!((mus.songs[0].mode, mus.songs[0].patches.len(), mus.songs[0].positions.len()), (2, 13, 14));

    let mut sizes = BTreeMap::new();
    for n in 1..=5 {
        let d = demo::Demo::parse(&read(&format!("demo.{n}"))).unwrap();
        assert!((1..=5).contains(&d.header.episode));
        sizes.insert(n, d.tail.len());
    }
    // size - 30 ≡ 0 mod 3 → 1 trailing byte after the last complete triplet.
    assert!(sizes.values().all(|&t| t == 1), "{sizes:?}");

    let anm = misc::AnmFile::parse(&read("tyrend.anm")).unwrap();
    assert_eq!((anm.nlps, anm.n_records), (51, 112));
}

/// The x of the tiny font reads as an x (not "%"): solid,
/// symmetric left-right and top-bottom, at the height of a, e, o; the small
/// font is unchanged.
#[test]
fn tiny_font_x_is_redrawn() {
    let original = sprite::MainShapes::parse(&read("tyrian.shp")).unwrap();
    let m = original.clone().with_redrawn_x();
    let glyph = m.simple[2].entries[57].sprite.as_ref().unwrap().decode();
    assert_eq!((glyph.width, glyph.height), (4, 6));
    let rows: Vec<String> =
        (0..6).map(|y| (0..4).map(|x| if glyph.opaque[y * 4 + x] { 'x' } else { '.' }).collect()).collect();
    assert_eq!(rows, ["....", "....", "x..x", ".xx.", ".xx.", "x..x"]);
    assert!(glyph.pixels.iter().zip(&glyph.opaque).all(|(&v, &o)| !o || v == 7));
    assert_eq!(m.simple[1], original.simple[1]);
}
