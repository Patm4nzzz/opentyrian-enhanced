//! Palettized 8-bit rendering, identical to the framebuffer of the C version.
//!
//! The C version draws everything into 8-bit SDL surfaces (`VGAScreen`,
//! `VGAScreen2`, `game_screen`) and then converts through the hardware palette at
//! the time of `JE_showVGA` (ADR 0020). This crate reproduces those surfaces and
//! the blitters of `sprite.c`, the fonts of `font.c`/`fonthand.c`, the
//! fades of `palette.c`: the Godot rewrite only displays the result
//! (indexed image + palette), it does not reinterpret any drawing.

pub mod assets;
pub mod font;
pub mod palette;
pub mod surface;

pub use assets::Assets;
pub use palette::HwPalette;
pub use surface::Surface;

/// 320x200 RGB image: indices passed through the hardware palette.
pub fn to_rgb(surface: &Surface, pal: &HwPalette) -> Vec<u8> {
    let mut out = Vec::with_capacity(surface.pixels.len() * 3);
    for &i in &surface.pixels {
        out.extend_from_slice(&pal.colors[i as usize]);
    }
    out
}
