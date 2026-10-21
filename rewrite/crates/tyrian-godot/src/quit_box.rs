//! Confirmation box placed on a screen whose palette is not the game's
//! (title: palette 8, red) with its true colors.
//!
//! `JE_quitRequestMsg` draws the box with indices of the game
//! palette (sprite 35, text of banks 0, 12 and 15). On the title, these indices
//! are displayed in the colors of the title palette: red blob and dark
//! title on dark. The pixels of the box receive here indices that
//! the screen does not use elsewhere, and the hardware palette takes the
//! colors of the game palette there. The box stays in the 8-bit surface: it
//! follows fades, captures and filters like the rest of the screen.

use godot::prelude::*;
use tyrian_render::assets::Rgb8Palette;
use tyrian_render::Surface;

use crate::vga::TyrianVga;

/// Indices 240 to 255 stay out of reach: the title does not fade them
/// with the rest of the image (`fade_palette(10, 0, 255 - 16)`).
const LAST_FREE_SLOT: usize = 239;

/// Screen and palette before the box was placed.
pub struct BoxBackup {
    surface: Surface,
    colors: Rgb8Palette,
}

fn game_palette(bytes: &PackedByteArray) -> Option<Rgb8Palette> {
    let b = bytes.as_slice();
    if b.len() < 768 {
        return None;
    }
    let mut p = [[0u8; 3]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        *c = [b[i * 3], b[i * 3 + 1], b[i * 3 + 2]];
    }
    Some(p)
}

/// Screen index of each index of the box; `None` if the palette has no
/// free index left.
fn slots_for(needed: &[bool; 256], shown: &Rgb8Palette, game: &Rgb8Palette, used: &[bool; 256]) -> Option<[u8; 256]> {
    let mut slot = [0u8; 256];
    let mut reserved = *used;
    for i in (0..256).filter(|&i| needed[i] && shown[i] == game[i]) {
        slot[i] = i as u8;
        reserved[i] = true;
    }
    for i in (0..256).filter(|&i| needed[i] && shown[i] != game[i]) {
        let free = |s: usize| !reserved[s];
        let s = if i <= LAST_FREE_SLOT && free(i) { i } else { (1..=LAST_FREE_SLOT).rev().find(|&s| free(s))? };
        reserved[s] = true;
        slot[i] = s as u8;
    }
    Some(slot)
}

#[godot_api(secondary)]
impl TyrianVga {
    /// Places on surface `surf` the box drawn with the indices of the
    /// game palette (`game_palette`, 768 bytes) on two blank surfaces,
    /// cleared to 0 (`on_zero`) and to 1 (`on_one`): equal pixels are those
    /// of the box. True if the box is placed; false if the displayed palette
    /// already shows its colors (shop, game: the caller then draws it
    /// directly) or has no free index. Can be called again on every frame:
    /// the screen from before the box is restored before each placement.
    #[func]
    fn place_true_colour_box(
        &mut self,
        surf: i32,
        on_zero: PackedByteArray,
        on_one: PackedByteArray,
        game_palette_bytes: PackedByteArray,
    ) -> bool {
        let si = surf.clamp(0, 2) as usize;
        let (a, b) = (on_zero.as_slice(), on_one.as_slice());
        let Some(game) = game_palette(&game_palette_bytes) else { return false };
        if a.len() != b.len() || a.len() != self.surfaces[si].pixels.len() {
            return false;
        }
        let opaque: Vec<bool> = a.iter().zip(b).map(|(x, y)| x == y).collect();
        let mut needed = [false; 256];
        for (&v, _) in a.iter().zip(&opaque).filter(|(_, &o)| o) {
            needed[v as usize] = true;
        }
        if self.box_backup.is_none() {
            let shown = &self.palette.colors;
            if (0..256).all(|i| !needed[i] || shown[i] == game[i]) {
                return false;
            }
            self.box_backup = Some(BoxBackup { surface: self.surfaces[si].clone(), colors: self.palette.colors });
        }
        let backup = self.box_backup.as_ref().expect("backup set above");
        let mut used = [false; 256];
        for (&p, _) in backup.surface.pixels.iter().zip(&opaque).filter(|(_, &o)| !o) {
            used[p as usize] = true;
        }
        let Some(slot) = slots_for(&needed, &backup.colors, &game, &used) else { return false };
        self.surfaces[si] = backup.surface.clone();
        self.palette.colors = backup.colors;
        for i in (0..256).filter(|&i| needed[i] && backup.colors[i] != game[i]) {
            self.palette.colors[slot[i] as usize] = game[i];
        }
        let sf = &mut self.surfaces[si];
        for (j, _) in opaque.iter().enumerate().filter(|(_, &o)| o) {
            sf.pixels[j] = slot[a[j] as usize];
            sf.forget(j);
        }
        true
    }

    /// Closes the box placed by `place_true_colour_box`: `keep` false returns
    /// the screen and palette from before ("No"); true keeps both ("Yes":
    /// the fade to black that follows takes the box away with the screen).
    #[func]
    fn close_true_colour_box(&mut self, surf: i32, keep: bool) {
        if let Some(b) = self.box_backup.take().filter(|_| !keep) {
            self.surfaces[surf.clamp(0, 2) as usize] = b.surface;
            self.palette.colors = b.colors;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(f: impl Fn(usize) -> [u8; 3]) -> Rgb8Palette {
        let mut p = [[0u8; 3]; 256];
        for (i, c) in p.iter_mut().enumerate() {
            *c = f(i);
        }
        p
    }

    #[test]
    fn already_shown_colours_keep_their_index() {
        let game = palette(|i| [i as u8, 1, 2]);
        let mut needed = [false; 256];
        needed[5] = true;
        let slot = slots_for(&needed, &game, &game, &[false; 256]).unwrap();
        assert_eq!(slot[5], 5);
    }

    #[test]
    fn colours_not_shown_take_a_free_index_never_one_in_use() {
        let game = palette(|i| [i as u8, 1, 2]);
        let shown = palette(|i| [i as u8, 9, 9]);
        let (mut needed, mut used) = ([false; 256], [false; 256]);
        needed[5] = true;
        needed[6] = true;
        used[5] = true;
        let slot = slots_for(&needed, &shown, &game, &used).unwrap();
        assert_eq!(slot[6], 6);
        assert_ne!(slot[5], 5);
        assert!(!used[slot[5] as usize]);
        assert!(slot[5] as usize <= LAST_FREE_SLOT);
    }

    #[test]
    fn no_free_index_gives_up() {
        let game = palette(|i| [i as u8, 1, 2]);
        let shown = palette(|i| [i as u8, 9, 9]);
        let mut needed = [false; 256];
        needed[5] = true;
        assert!(slots_for(&needed, &shown, &game, &[true; 256]).is_none());
    }
}
