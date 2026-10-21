//! Jukebox starfield (`starlib.c`) and the palette that goes with it
//! (`vga_palette.c`, "only used for the jukebox").
//!
//! Same algorithm as the C: stars projected in perspective
//! (`spX / spZ + 160`), trails erased, fifteen spawn patterns
//! (`JE_newStar`) that change by themselves after `changeTime` frames, tuning
//! keys (`+ - 0..9 ! @ # $ [ ] { } ` C S`). The `mt_rand` draw comes
//! from a `MersenneTwister` specific to the screen: the simulation does not depend on it.
//! `Effects` = Wild by default (config.c: `processorType = 4`): the C
//! asks for 2000 stars and bounds them to `starlib_MAX_STARS` = 1000, the number
//! kept here whatever the setting.

use godot::prelude::*;
use tyrian_render::assets::Rgb8Palette;
use tyrian_sim::rng::MersenneTwister;

use crate::vga::TyrianVga;

const MAX_STARS: usize = 1000;
const MAX_TYPES: u32 = 14;
const W: i32 = 320;
const SCREEN: i32 = 320 * 200;

/// `vga_palette.c:vga_palette`.
const VGA_PALETTE: Rgb8Palette = [
    [0, 0, 0],
    [0, 0, 168],
    [0, 168, 0],
    [0, 168, 168],
    [168, 0, 0],
    [168, 0, 168],
    [168, 84, 0],
    [168, 168, 168],
    [84, 84, 84],
    [84, 84, 252],
    [84, 252, 84],
    [84, 252, 252],
    [252, 84, 84],
    [252, 84, 252],
    [252, 252, 84],
    [252, 252, 252],
    [0, 0, 0],
    [20, 20, 20],
    [32, 32, 32],
    [44, 44, 44],
    [56, 56, 56],
    [68, 68, 68],
    [80, 80, 80],
    [96, 96, 96],
    [112, 112, 112],
    [128, 128, 128],
    [144, 144, 144],
    [160, 160, 160],
    [180, 180, 180],
    [200, 200, 200],
    [224, 224, 224],
    [252, 252, 252],
    [0, 0, 252],
    [64, 0, 252],
    [124, 0, 252],
    [188, 0, 252],
    [252, 0, 252],
    [252, 0, 188],
    [252, 0, 124],
    [252, 0, 64],
    [252, 0, 0],
    [252, 64, 0],
    [252, 124, 0],
    [252, 188, 0],
    [252, 252, 0],
    [188, 252, 0],
    [124, 252, 0],
    [64, 252, 0],
    [0, 252, 0],
    [0, 252, 64],
    [0, 252, 124],
    [0, 252, 188],
    [0, 252, 252],
    [0, 188, 252],
    [0, 124, 252],
    [0, 64, 252],
    [124, 124, 252],
    [156, 124, 252],
    [188, 124, 252],
    [220, 124, 252],
    [252, 124, 252],
    [252, 124, 220],
    [252, 124, 188],
    [252, 124, 156],
    [252, 124, 124],
    [252, 156, 124],
    [252, 188, 124],
    [252, 220, 124],
    [252, 252, 124],
    [220, 252, 124],
    [188, 252, 124],
    [156, 252, 124],
    [124, 252, 124],
    [124, 252, 156],
    [124, 252, 188],
    [124, 252, 220],
    [124, 252, 252],
    [124, 220, 252],
    [124, 188, 252],
    [124, 156, 252],
    [180, 180, 252],
    [196, 180, 252],
    [216, 180, 252],
    [232, 180, 252],
    [252, 180, 252],
    [252, 180, 232],
    [252, 180, 216],
    [252, 180, 196],
    [252, 180, 180],
    [252, 196, 180],
    [252, 216, 180],
    [252, 232, 180],
    [252, 252, 180],
    [232, 252, 180],
    [216, 252, 180],
    [196, 252, 180],
    [180, 252, 180],
    [180, 252, 196],
    [180, 252, 216],
    [180, 252, 232],
    [180, 252, 252],
    [180, 232, 252],
    [180, 216, 252],
    [180, 196, 252],
    [0, 0, 112],
    [28, 0, 112],
    [56, 0, 112],
    [84, 0, 112],
    [112, 0, 112],
    [112, 0, 84],
    [112, 0, 56],
    [112, 0, 28],
    [112, 0, 0],
    [112, 28, 0],
    [112, 56, 0],
    [112, 84, 0],
    [112, 112, 0],
    [84, 112, 0],
    [56, 112, 0],
    [28, 112, 0],
    [0, 112, 0],
    [0, 112, 28],
    [0, 112, 56],
    [0, 112, 84],
    [0, 112, 112],
    [0, 84, 112],
    [0, 56, 112],
    [0, 28, 112],
    [56, 56, 112],
    [68, 56, 112],
    [84, 56, 112],
    [96, 56, 112],
    [112, 56, 112],
    [112, 56, 96],
    [112, 56, 84],
    [112, 56, 68],
    [112, 56, 56],
    [112, 68, 56],
    [112, 84, 56],
    [112, 96, 56],
    [112, 112, 56],
    [96, 112, 56],
    [84, 112, 56],
    [68, 112, 56],
    [56, 112, 56],
    [56, 112, 68],
    [56, 112, 84],
    [56, 112, 96],
    [56, 112, 112],
    [56, 96, 112],
    [56, 84, 112],
    [56, 68, 112],
    [80, 80, 112],
    [88, 80, 112],
    [96, 80, 112],
    [104, 80, 112],
    [112, 80, 112],
    [112, 80, 104],
    [112, 80, 96],
    [112, 80, 88],
    [112, 80, 80],
    [112, 88, 80],
    [112, 96, 80],
    [112, 104, 80],
    [112, 112, 80],
    [104, 112, 80],
    [96, 112, 80],
    [88, 112, 80],
    [80, 112, 80],
    [80, 112, 88],
    [80, 112, 96],
    [80, 112, 104],
    [80, 112, 112],
    [80, 104, 112],
    [80, 96, 112],
    [80, 88, 112],
    [0, 0, 64],
    [16, 0, 64],
    [32, 0, 64],
    [48, 0, 64],
    [64, 0, 64],
    [64, 0, 48],
    [64, 0, 32],
    [64, 0, 16],
    [64, 0, 0],
    [64, 16, 0],
    [64, 32, 0],
    [64, 48, 0],
    [64, 64, 0],
    [48, 64, 0],
    [32, 64, 0],
    [16, 64, 0],
    [0, 64, 0],
    [0, 64, 16],
    [0, 64, 32],
    [0, 64, 48],
    [0, 64, 64],
    [0, 48, 64],
    [0, 32, 64],
    [0, 16, 64],
    [32, 32, 64],
    [40, 32, 64],
    [48, 32, 64],
    [56, 32, 64],
    [64, 32, 64],
    [64, 32, 56],
    [64, 32, 48],
    [64, 32, 40],
    [64, 32, 32],
    [64, 40, 32],
    [64, 48, 32],
    [64, 56, 32],
    [64, 64, 32],
    [56, 64, 32],
    [48, 64, 32],
    [40, 64, 32],
    [32, 64, 32],
    [32, 64, 40],
    [32, 64, 48],
    [32, 64, 56],
    [32, 64, 64],
    [32, 56, 64],
    [32, 48, 64],
    [32, 40, 64],
    [44, 44, 64],
    [48, 44, 64],
    [52, 44, 64],
    [60, 44, 64],
    [64, 44, 64],
    [64, 44, 60],
    [64, 44, 52],
    [64, 44, 48],
    [64, 44, 44],
    [64, 48, 44],
    [64, 52, 44],
    [64, 60, 44],
    [64, 64, 44],
    [60, 64, 44],
    [52, 64, 44],
    [48, 64, 44],
    [44, 64, 44],
    [44, 64, 48],
    [44, 64, 52],
    [44, 64, 60],
    [44, 64, 64],
    [44, 60, 64],
    [44, 52, 64],
    [44, 48, 64],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
];

#[derive(Clone, Copy, Default)]
struct Star {
    sp_x: i16,
    sp_y: i16,
    sp_z: i16,
    last_x: i16,
    last_y: i16,
}

/// Static variables of `starlib.c`.
pub struct Starlib {
    stars: Vec<Star>,
    setup: u8,
    step_counter: u16,
    nsp2: u16,
    nsp_var2_inc: i8,
    nsp: f32,
    nsp_var_inc: f32,
    nsp_var_var_inc: f32,
    change_time: u16,
    do_change: bool,
    speed: i16,
    speed_change: i8,
    p_color: u8,
    mt: MersenneTwister,
}

impl Starlib {
    /// `JE_starlib_init` (first call): starting values, pattern 2,
    /// automatic change active, stars drawn at random.
    pub fn new(seed: u32) -> Self {
        let mut s = Self {
            stars: vec![Star::default(); MAX_STARS],
            setup: 0,
            step_counter: 0,
            nsp2: 0,
            nsp_var2_inc: 0,
            nsp: 0.0,
            nsp_var_inc: 0.0,
            nsp_var_var_inc: 0.0,
            change_time: 0,
            do_change: false,
            speed: 0,
            speed_change: 0,
            p_color: 0,
            mt: MersenneTwister::new(seed),
        };
        s.reset_values();
        s.change_setup(2);
        s.do_change = true;
        for x in 0..MAX_STARS {
            s.stars[x].sp_x = ((s.mt.next_u32() % 64000) as i32 - 32000) as i16;
            s.stars[x].sp_y = ((s.mt.next_u32() % 40000) as i32 - 20000) as i16;
            s.stars[x].sp_z = (x + 1) as i16;
        }
        s
    }

    /// `mt_rand_1`.
    fn rand_1(&mut self) -> f32 {
        self.mt.next_u32() as f32 / 0xffff_ffffu32 as f32
    }

    /// `JE_resetValues`.
    fn reset_values(&mut self) {
        self.nsp2 = 1;
        self.nsp_var2_inc = 1;
        self.nsp_var_inc = 0.1;
        self.nsp_var_var_inc = 0.0001;
        self.nsp = 0.0;
        self.p_color = 32;
        self.speed = 2;
        self.speed_change = 0;
    }

    /// `JE_changeSetup`: 0 = randomly drawn pattern.
    fn change_setup(&mut self, setup_type: u8) {
        self.step_counter = 0;
        self.change_time = (self.mt.next_u32() % 1000) as u16;
        self.setup = if setup_type > 0 { setup_type } else { (self.mt.next_u32() % (MAX_TYPES + 1)) as u8 };
        if self.setup == 1 {
            self.nsp_var_inc = 0.1;
        }
        if self.nsp_var_inc > 2.2 {
            self.nsp_var_inc = 0.1;
        }
    }

    /// `nsp2 += nspVar2Inc` then bounce at the bounds (patterns 9 to 13).
    fn bounce_nsp2(&mut self) {
        self.nsp2 = self.nsp2.wrapping_add_signed(self.nsp_var2_inc as i16);
        if self.nsp2 == 65535 || self.nsp2 == 0 {
            self.nsp_var2_inc = self.nsp_var2_inc.wrapping_neg();
        }
    }

    /// `JE_newStar`: spawn position according to the current pattern; the
    /// patterns that place nothing keep the (x, y) received, like the C.
    fn new_star(&mut self, x: i32, y: i32) -> (i32, i32) {
        let (mut x, mut y) = (x, y);
        if self.setup == 0 {
            x = (self.mt.next_u32() % 64000) as i32 - 32000;
            y = (self.mt.next_u32() % 40000) as i32 - 20000;
            return (x, y);
        }
        self.nsp += self.nsp_var_inc;
        let nsp = self.nsp;
        match self.setup {
            1 => {
                x = ((nsp / 30.0).sin() * 20000.0) as i32;
                y = (self.mt.next_u32() % 40000) as i32 - 20000;
            }
            2 => {
                x = (nsp.cos() * 20000.0) as i32;
                y = (nsp.sin() * 20000.0) as i32;
            }
            3 => {
                let r = (nsp / 6.0) as i32 % 200;
                x = (((nsp * 15.0).cos() * 100.0) as i32).wrapping_mul(r);
                y = (((nsp * 15.0).sin() * 100.0) as i32).wrapping_mul(r);
            }
            4 | 5 => {
                x = (if self.setup == 4 { (nsp / 60.0).sin() } else { (nsp / 2.0).sin() } * 20000.0) as i32;
                y = (nsp.cos() * ((nsp / 200.0).sin() * 300.0) as i32 as f32 * 100.0) as i32;
            }
            6 => {
                x = (nsp.sin() * 40000.0) as i32;
                y = (nsp.cos() * 20000.0) as i32;
            }
            8 => {
                x = ((nsp / 2.0).sin() * 40000.0) as i32;
                y = (nsp.cos() * 20000.0) as i32;
            }
            7 => {
                x = (self.mt.next_u32() % 65535) as i32;
                y = if self.mt.next_u32().is_multiple_of(2) {
                    ((nsp / 80.0).cos() * 10000.0) as i32 + 15000
                } else {
                    50000 - ((nsp / 80.0).cos() * 13000.0) as i32
                };
            }
            9..=11 => {
                self.bounce_nsp2();
                let (div, k) = match self.setup {
                    9 => (10.0, nsp / 500.0),
                    10 => (5.0, nsp / 100.0),
                    _ => (1000.0, nsp / 2.0),
                };
                let n2 = self.nsp2 as f32 / div;
                x = ((n2.sin() + k).cos() * 32000.0) as i32;
                y = ((n2.cos() + k).sin() * 30000.0) as i32;
            }
            12 => {
                if nsp != 0.0 {
                    self.bounce_nsp2();
                    let n2 = self.nsp2 as f32;
                    let d = nsp.abs().sqrt() / 10.0 + 1.0;
                    x = (((n2 / 2.0).sin() / d + n2 / 100.0).cos() * 32000.0) as i32;
                    y = (((n2 / 2.0).cos() / d + n2 / 100.0).sin() * 30000.0) as i32;
                }
            }
            13 => {
                if nsp != 0.0 {
                    self.bounce_nsp2();
                    let n2 = self.nsp2 as f32;
                    x = (((n2 / 10.0).sin() / 2.0 + nsp / 20.0).cos() * 32000.0) as i32;
                    y = (((n2 / 11.0).sin() / 2.0 + nsp / 20.0).sin() * 30000.0) as i32;
                }
            }
            14 => {
                self.nsp2 = self.nsp2.wrapping_add_signed(self.nsp_var2_inc as i16);
                x = ((nsp.sin() + (self.nsp2 as f32 / 1000.0).cos() * 3.0) * 12000.0) as i32;
                y = (nsp.cos() * 10000.0) as i32 + self.nsp2 as i32;
            }
            _ => {}
        }
        (x, y)
    }

    /// Tuning key (`JE_starlib_main`, `switch (toupper(key))`).
    /// "P" (pause) and "X" / Esc (`run = false`, read by nobody) are
    /// handled by the screen.
    pub fn key(&mut self, key: char) {
        match key.to_ascii_uppercase() {
            '+' => {
                self.speed = self.speed.wrapping_add(1);
                self.speed_change = 0;
            }
            '-' => {
                self.speed = self.speed.wrapping_sub(1);
                self.speed_change = 0;
            }
            c @ '1'..='9' => self.change_setup(c as u8 - b'0'),
            '0' => self.change_setup(10),
            '!' => self.change_setup(11),
            '@' => self.change_setup(12),
            '#' => self.change_setup(13),
            '$' => self.change_setup(14),
            'C' => self.reset_values(),
            'S' => self.nsp_var_var_inc = self.rand_1() * 0.01 - 0.005,
            '[' => self.p_color = self.p_color.wrapping_sub(1),
            ']' => self.p_color = self.p_color.wrapping_add(1),
            '{' => self.p_color = self.p_color.wrapping_sub(72),
            '}' => self.p_color = self.p_color.wrapping_add(72),
            '`' => self.do_change = !self.do_change,
            _ => {}
        }
    }

    /// `JE_starlib_main` on `surf` (320x200 indices), including the key of the
    /// previous frame (`newkey`).
    pub fn step(&mut self, surf: &mut [u8], key: Option<char>) {
        self.speed = self.speed.wrapping_add(self.speed_change as i16);
        for i in 0..MAX_STARS {
            let s = self.stars[i];
            let off = s.last_x as i32 + s.last_y as i32 * W;
            if (2 * W..SCREEN - 2 * W).contains(&off) {
                for d in [0, -1, 1, -2, 2, -W, W, -2 * W, 2 * W] {
                    surf[(off + d) as usize] = 0;
                }
            }
            let z = s.sp_z;
            let x = s.sp_x as i32 / z as i32 + 160;
            let y = s.sp_y as i32 / z as i32 + 100;
            let z = z.wrapping_sub(self.speed);
            if z <= 0 || y == 0 || y > 198 || !(1..=318).contains(&x) {
                let (nx, ny) = self.new_star(x, y);
                let st = &mut self.stars[i];
                st.sp_z = 500;
                st.sp_x = nx as i16;
                st.sp_y = ny as i16;
            } else {
                let st = &mut self.stars[i];
                st.last_x = x as i16;
                st.last_y = y as i16;
                st.sp_z = z;
                let off = x + y * W;
                let col = self.p_color.wrapping_add(((z >> 4) & 31) as u8);
                if (2 * W..SCREEN - 2 * W).contains(&off) {
                    surf[off as usize] = col;
                    for d in [-1, 1, -W, W] {
                        surf[(off + d) as usize] = col.wrapping_add(72);
                    }
                    for d in [-2, 2, -2 * W, 2 * W] {
                        surf[(off + d) as usize] = col.wrapping_add(144);
                    }
                }
            }
        }
        if let Some(k) = key {
            self.key(k);
        }
        if self.do_change {
            self.step_counter = self.step_counter.wrapping_add(1);
            if self.step_counter > self.change_time {
                self.change_setup(0);
            }
        }
        if self.mt.next_u32() % 1000 == 1 {
            self.nsp_var_var_inc = self.rand_1() * 0.01 - 0.005;
        }
        self.nsp_var_inc += self.nsp_var_var_inc;
    }
}

/// Starfield as seen from GDScript (Jukebox screen). One instance kept by
/// the screen between two visits, like the statics of `starlib.c`
/// (`JE_starlib_init` only initializes on the first call).
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct TyrianStarlib {
    lib: Starlib,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for TyrianStarlib {
    fn init(base: Base<RefCounted>) -> Self {
        Self { lib: Starlib::new(0), base }
    }
}

#[godot_api]
impl TyrianStarlib {
    /// Restarts from `JE_starlib_init` with the seed `mt_srand(seed)`.
    #[func]
    fn reseed(&mut self, seed: i64) {
        self.lib = Starlib::new(seed as u32);
    }

    /// One frame of `JE_starlib_main` in surface `surf` of `vga`;
    /// `key`: character of the received key (empty = none).
    #[func]
    fn step(&mut self, mut vga: Gd<TyrianVga>, surf: i32, key: GString) {
        let k = key.to_string().chars().next();
        let mut v = vga.bind_mut();
        let i = surf.clamp(0, 2) as usize;
        self.lib.step(&mut v.surfaces[i].pixels, k);
    }
}

#[godot_api(secondary)]
impl TyrianVga {
    /// `colors = vga_palette` (jukebox.c: target of the fade-in).
    #[func]
    fn load_vga_palette(&mut self) {
        self.colors = VGA_PALETTE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_stay_inside_and_are_deterministic() {
        let mut a = Starlib::new(7);
        let mut b = Starlib::new(7);
        let (mut sa, mut sb) = (vec![0u8; SCREEN as usize], vec![0u8; SCREEN as usize]);
        for n in 0..3000 {
            // All the patterns pass (keys 1..9, 0, ! @ # $).
            let k = "1234567890!@#$".chars().nth((n / 200) % 14);
            let key = if n % 200 == 0 { k } else { None };
            a.step(&mut sa, key);
            b.step(&mut sb, key);
        }
        assert_eq!(sa, sb);
        assert!(sa.iter().any(|&p| p != 0), "no star drawn");
        // Different seed: another sky (the draw is not frozen).
        let mut c = Starlib::new(8);
        let mut sc = vec![0u8; SCREEN as usize];
        for _ in 0..50 {
            c.step(&mut sc, None);
        }
        assert_ne!(sc, sa);
    }

    #[test]
    fn vga_palette_matches_c_corners() {
        assert_eq!(VGA_PALETTE[1], [0, 0, 168]);
        assert_eq!(VGA_PALETTE[15], [252, 252, 252]);
        assert_eq!(VGA_PALETTE[255], [0, 0, 0]);
    }
}
