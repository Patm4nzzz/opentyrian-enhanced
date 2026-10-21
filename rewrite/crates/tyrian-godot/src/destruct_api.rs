//! `TyrianDestruct`: Destruct mini-game (`tyrian_sim::destruct`) exposed to
//! GDScript. The `destruct.gd` scene reproduces the menus and the loop of
//! `destruct.c:JE_destructMain`; each tick, it calls `tick_draw`
//! (drawing into the `VGAScreen` of the shared `TyrianVga`, like the C), presents
//! the image, then `tick_input` with the held keys.

use godot::prelude::*;
use tyrian_render::font;
use tyrian_render::Surface;
use tyrian_sim::destruct::{Canvas, Destruct, TickEnd, MAX_MOVE, MAX_PLAYERS};
use tyrian_sim::destruct_config;

use crate::vga::{with_assets, TyrianVga};

/// `destructSpriteSheet` : `JE_loadCompShapes(&destructSpriteSheet, '~')`.
const SHEET: &str = "newsh~.shp";

/// `VGAScreen` of a `TyrianVga` and the C blitters.
struct VgaCanvas<'a> {
    sf: &'a mut Surface,
}

impl Canvas for VgaCanvas<'_> {
    fn pixels(&mut self) -> &mut [u8] {
        &mut self.sf.pixels
    }

    fn blit_sprite2(&mut self, x: i32, y: i32, index: u32) {
        let sf = &mut *self.sf;
        with_assets(|a| {
            if let Some(data) = a.sheet(SHEET) {
                sf.blit_sprite2(x, y, data, index);
            }
        });
    }

    fn out_text(&mut self, x: i32, y: i32, text: &[u8], colorbank: u8, brightness: i32) {
        let sf = &mut *self.sf;
        with_assets(|a| font::out_text(sf, a, x, y, text, colorbank, brightness));
    }
}

#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct TyrianDestruct {
    game: Option<Destruct>,
}

impl TyrianDestruct {
    fn with_canvas<R>(&mut self, vga: &Gd<TyrianVga>, f: impl FnOnce(&mut Destruct, &mut VgaCanvas) -> R) -> Option<R> {
        let game = self.game.as_mut()?;
        let mut v = vga.clone();
        let mut b = v.bind_mut();
        let sf = &mut b.surfaces[0];
        sf.tag = 0;
        let mut c = VgaCanvas { sf };
        Some(f(game, &mut c))
    }
}

#[godot_api]
impl TyrianDestruct {
    /// `destruct.c:load_destruct_config` on the contents of `destruct.cfg`
    /// (empty if it does not exist): {"file": contents to rewrite,
    /// "warnings": PackedStringArray}. New session: `rng.c` generator
    /// seeded by `seed`, then `skip` draws (those made by the C before
    /// entering Destruct).
    #[func]
    fn load_config(&mut self, data: PackedByteArray, seed: i64, skip: i32) -> VarDictionary {
        let (cfg, file, warnings) = destruct_config::load(data.as_slice());
        // rng_seed(0) replaces 0 with the time: a zero seed does not exist.
        let mut d = Destruct::new(cfg, if seed == 0 { 1 } else { seed as u64 });
        for _ in 0..skip.max(0) {
            d.rng.next_u32();
        }
        d.reset_players();
        self.game = Some(d);
        let mut out = VarDictionary::new();
        out.set("file", &PackedByteArray::from(file.as_slice()));
        out.set("warnings", &warnings.iter().map(GString::from).collect::<PackedStringArray>());
        out
    }

    /// `weaponNames[]` and the two HUD words (already translated).
    #[func]
    fn set_texts(&mut self, weapon_names: PackedStringArray, dmg: GString, pts: GString) {
        if let Some(g) = self.game.as_mut() {
            let names = weapon_names.as_slice().iter().map(|s| font::to_cp437(&s.to_string())).collect();
            g.set_texts(names, font::to_cp437(&dmg.to_string()), font::to_cp437(&pts.to_string()));
        }
    }

    /// `config.allow_custom` (sixth "Custom" mode).
    #[func]
    fn allow_custom(&self) -> bool {
        self.game.as_ref().is_some_and(|g| g.config.allow_custom)
    }

    /// Keys of `destruct.cfg`: 2 players x 8 actions x 4 slots,
    /// SDL_Scancode codes (0 = empty), order of `defaultKeyConfig`.
    #[func]
    fn key_config(&self) -> PackedInt32Array {
        let Some(g) = self.game.as_ref() else {
            return PackedInt32Array::new();
        };
        g.config.keys.iter().flatten().flatten().map(|&k| k as i32).collect()
    }

    /// `DE_ResetPlayers` + `is_cpu = config.ai`.
    #[func]
    fn reset_players(&mut self) {
        if let Some(g) = self.game.as_mut() {
            g.reset_players();
        }
    }

    /// Start of a round (`destructFirstTime`, `DE_ResetUnits`,
    /// `DE_ResetLevel`): terrain drawn into VGAScreen. Returns the track to
    /// play (`play_song`).
    #[func]
    fn start_round(&mut self, vga: Gd<TyrianVga>, mode: i32) -> i32 {
        self.with_canvas(&vga, |g, c| {
            g.start_round(c, mode.max(0) as usize);
            g.song
        })
        .unwrap_or(0)
    }

    /// `DE_RunTick` up to the HUD (before `JE_showVGA`).
    #[func]
    fn tick_draw(&mut self, vga: Gd<TyrianVga>) {
        self.with_canvas(&vga, |g, c| g.tick_draw(c));
    }

    /// True once, after the first frame of a round (fade-in).
    #[func]
    fn take_first_time(&mut self) -> bool {
        self.game.as_mut().is_some_and(|g| g.take_first_time())
    }

    /// `DE_RunTickGetInput` (already resolved: `moves[p * 8 + k]`), `DE_ProcessInput`
    /// and end of round. Returns 1 for `STATE_RELOAD`, 0 otherwise.
    #[func]
    fn tick_input(&mut self, moves: PackedInt32Array) -> i32 {
        let Some(g) = self.game.as_mut() else { return 0 };
        let mut keys = [[false; MAX_MOVE]; MAX_PLAYERS];
        for (i, &m) in moves.as_slice().iter().enumerate().take(MAX_MOVE * MAX_PLAYERS) {
            keys[i / MAX_MOVE][i % MAX_MOVE] = m != 0;
        }
        (g.tick_input(keys) == TickEnd::Reload) as i32
    }

    /// Sounds of the queue (`DE_RunTickPlaySounds`): triplets channel, sample
    /// (1-based), x position.
    #[func]
    fn take_sounds(&mut self) -> PackedInt32Array {
        let Some(g) = self.game.as_mut() else {
            return PackedInt32Array::new();
        };
        g.take_sounds().iter().flat_map(|s| [s.channel as i32, s.sample as i32, s.x as i32]).collect()
    }

    #[func]
    fn is_cpu(&self, player: i32) -> bool {
        self.game.as_ref().is_some_and(|g| g.players[player.clamp(0, 1) as usize].is_cpu)
    }

    /// F10 / F11 : bascule humain / ordinateur.
    #[func]
    fn toggle_cpu(&mut self, player: i32) {
        if let Some(g) = self.game.as_mut() {
            let p = &mut g.players[player.clamp(0, 1) as usize];
            p.is_cpu = !p.is_cpu;
        }
    }

    /// State read by the tests: [left score, right score, remaining units
    /// left, right, shots in flight, explosions, end-of-round delay,
    /// map flags].
    #[func]
    fn state(&self) -> PackedInt32Array {
        let Some(g) = self.game.as_ref() else {
            return PackedInt32Array::new();
        };
        PackedInt32Array::from(vec![
            g.players[0].score as i32,
            g.players[1].score as i32,
            g.players[0].units_remaining as i32,
            g.players[1].units_remaining as i32,
            g.active_shots() as i32,
            g.active_explosions() as i32,
            g.end_delay() as i32,
            g.map_flags as i32,
        ])
    }

    /// Chosen unit of a player: [type, weapon, angle (f32 bits), power
    /// (f32 bits), x, rounded y, health].
    #[func]
    fn selected_unit(&self, player: i32) -> PackedInt32Array {
        let Some(g) = self.game.as_ref() else {
            return PackedInt32Array::new();
        };
        let p = &g.players[player.clamp(0, 1) as usize];
        let u = &p.units[p.unit_selected as usize];
        PackedInt32Array::from(vec![
            u.unit_type,
            u.shot_type,
            u.angle.0 as i32,
            u.power.0 as i32,
            u.unit_x as i32,
            u.unit_y.roundf() as i32,
            u.health,
        ])
    }
}
