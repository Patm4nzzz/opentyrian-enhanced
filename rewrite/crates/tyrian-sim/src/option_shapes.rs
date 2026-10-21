//! Sprite sizes of the `OPTION_SHAPES` table (table 5 of `tyrian.shp`).
//!
//! `shots.c:player_shot_move_and_draw` derives the collision radius of "special"
//! shots (`shotGr + shotAni > 60000`) from it: `sprite(OPTION_SHAPES, n)->width / 2`
//! and `->height / 2`. The simulation does not load a sprite sheet: the
//! sizes are extracted once from the original file (test
//! `matches_tyrian_shp` below). Beyond the 47 loaded entries, the C array
//! `sprite_table[5].sprite[]` is zero (static): size (0, 0).

/// (width, height) of sprites 0 to 46 of `OPTION_SHAPES`.
pub const OPTION_SHAPE_SIZES: [(u16, u16); 47] = [
    (20, 10),
    (18, 12),
    (28, 12),
    (27, 13),
    (27, 11),
    (23, 12),
    (28, 12),
    (27, 11),
    (27, 12),
    (28, 12),
    (28, 12),
    (28, 13),
    (153, 185),
    (12, 20),
    (13, 20),
    (12, 20),
    (13, 20),
    (114, 25),
    (10, 8),
    (10, 8),
    (169, 186),
    (55, 54),
    (62, 24),
    (65, 28),
    (62, 23),
    (22, 24),
    (99, 23),
    (93, 106),
    (191, 201),
    (5, 5),
    (5, 5),
    (85, 112),
    (93, 110),
    (80, 79),
    (15, 15),
    (189, 101),
    (228, 9),
    (25, 13),
    (28, 12),
    (32, 59),
    (14, 30),
    (32, 38),
    (50, 43),
    (47, 9),
    (84, 109),
    (95, 107),
    (20, 27),
];

/// Half-size (radius) of sprite `index` of `OPTION_SHAPES`, (0, 0) outside the table.
pub fn option_shape_radius(index: usize) -> (u16, u16) {
    OPTION_SHAPE_SIZES.get(index).map(|&(w, h)| (w / 2, h / 2)).unwrap_or((0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_tyrian_shp() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/tyrian.shp");
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("tyrian.shp missing: test skipped");
            return;
        };
        let shp = tyrian_formats::sprite::MainShapes::parse(&bytes).unwrap();
        let got: Vec<(u16, u16)> = shp.simple[5]
            .entries
            .iter()
            .map(|e| e.sprite.as_ref().map(|s| (s.width, s.height)).unwrap_or((0, 0)))
            .collect();
        assert_eq!(got, OPTION_SHAPE_SIZES.to_vec());
    }
}
