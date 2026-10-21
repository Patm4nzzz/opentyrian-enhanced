//! Hardware palette and fades (`palette.c`).
//!
//! `step_fade_palette` works in integers on 8-bit components with
//! division truncated toward zero (C99): reproduced as is, so the
//! intermediate colors of the fades are identical to the C code.

use crate::assets::Rgb8Palette;

#[derive(Clone)]
pub struct HwPalette {
    /// `palette.c:palette` (displayed colors).
    pub colors: Rgb8Palette,
    diff: Box<[[i32; 3]; 256]>,
}

impl Default for HwPalette {
    fn default() -> Self {
        Self { colors: [[0; 3]; 256], diff: Box::new([[0; 3]; 256]) }
    }
}

impl HwPalette {
    /// `set_palette`.
    pub fn set_palette(&mut self, colors: &Rgb8Palette, first: usize, last: usize) {
        for (dst, src) in self.colors.iter_mut().zip(colors.iter()).take(last.min(255) + 1).skip(first) {
            *dst = *src;
        }
    }

    /// `set_colors`.
    pub fn set_colors(&mut self, color: [u8; 3], first: usize, last: usize) {
        for i in first..=last.min(255) {
            self.colors[i] = color;
        }
    }

    /// `init_step_fade_palette`.
    pub fn init_fade_palette(&mut self, colors: &Rgb8Palette, first: usize, last: usize) {
        let rows = self.diff.iter_mut().zip(colors.iter()).zip(self.colors.iter());
        for ((diff, new), old) in rows.take(last.min(255) + 1).skip(first) {
            for ((d, n), o) in diff.iter_mut().zip(new).zip(old) {
                *d = *n as i32 - *o as i32;
            }
        }
    }

    /// `init_step_fade_solid`.
    pub fn init_fade_solid(&mut self, color: [u8; 3], first: usize, last: usize) {
        let rows = self.diff.iter_mut().zip(self.colors.iter());
        for (diff, old) in rows.take(last.min(255) + 1).skip(first) {
            for ((d, n), o) in diff.iter_mut().zip(&color).zip(old) {
                *d = *n as i32 - *o as i32;
            }
        }
    }

    /// `step_fade_palette`: `steps` = number of steps remaining (>0).
    pub fn step_fade(&mut self, steps: i32, first: usize, last: usize) {
        let steps = steps.max(1);
        for i in first..=last.min(255) {
            for k in 0..3 {
                let delta = self.diff[i][k] / steps;
                self.diff[i][k] -= delta;
                // Uint8 += int: wraps modulo 256 as in C.
                self.colors[i][k] = (self.colors[i][k] as i32 + delta) as u8;
            }
        }
    }

    /// `palette.c:JE_gammaCorrect` is not reproduced: gamma 0 by default.
    pub fn rgb_bytes(&self) -> Vec<u8> {
        self.colors.iter().flat_map(|c| c.iter().copied()).collect()
    }
}
