//! Regressions of the dimension limits before decoding.
use tyrian_formats::sprite::SpriteTable;

#[test]
fn security_nine_byte_shp_rejected_before_decode() {
    assert!(SpriteTable::parse(&[1, 0, 1, 255, 255, 255, 255, 0, 0]).is_err());
}

#[test]
fn security_direct_invalid_sprite_does_not_allocate() {
    let s = tyrian_formats::sprite::SimpleSprite { width: u16::MAX, height: u16::MAX, data: vec![] };
    assert!(s.validate_dimensions().is_err());
    assert!(s.decode().pixels.is_empty());
    for (width, height) in [(321u16, 1u16), (1, 257)] {
        let mut bytes = vec![1, 0, 1];
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        assert!(SpriteTable::parse(&bytes).is_err());
    }
}

#[test]
fn security_original_tables_fit_limits_including_201_pixel_sprite() {
    use tyrian_formats::sprite::MainShapes;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    for name in ["tyrian.shp", "tyrianc.shp"] {
        let main = MainShapes::parse(&std::fs::read(data.join(name)).unwrap()).unwrap();
        assert!(main.simple.iter().flat_map(|t| &t.entries).filter_map(|e| e.sprite.as_ref()).any(|s| s.height == 201));
    }
    for name in ["estsc.shp", "estpa.shp"] {
        SpriteTable::parse(&std::fs::read(data.join(name)).unwrap()).unwrap();
    }
}

#[test]
fn security_screen_dimensions_allowed() {
    let table = SpriteTable::parse(&[1, 0, 1, 64, 1, 200, 0, 0, 0]).unwrap();
    let img = table.entries[0].sprite.as_ref().unwrap().decode();
    assert_eq!(img.pixels.len(), 320 * 200);
}
