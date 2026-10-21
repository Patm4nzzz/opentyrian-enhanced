//! Enhanced effects exposed to Godot (Setup > Gameplay and Graphics): see
//! `fx_render` (rendering) and `tyrian_sim::fx` (what acts on the simulation).
//!
//! `set_effects` is only called by the player's game (level.gd); the tools
//! (golden masters, captures, benches, replays) do not call it and keep
//! the settings forced by `golden_master.c:golden_master_level_start`.

use godot::prelude::*;

use crate::fx_render::{FxOptions, FxRender, FX};
use crate::TyrianSim;

impl TyrianSim {
    /// After each player tick: events and per-frame update.
    pub(crate) fn fx_after_step(&mut self) {
        if !self.fx_on {
            return;
        }
        let Some(sim) = self.sim.as_mut() else { return };
        let events = sim.take_fx_events();
        let w = sim.presented_world();
        let now = tyrian_sim::achievements::clock_ms(w.iteration);
        FX.with(|f| f.borrow_mut().frame(events, w, w.iteration, now));
    }

    /// New level: settings set in the Sim, effects reset to zero
    /// (tyrian2.c: hitstop_reset, particles_clear, damage_numbers_reset).
    /// `.rep` replay: effects rendering only, the simulation keeps the
    /// golden master settings (hitstop and Modern explosions off); the same
    /// goes while recording a `.rep`, so that the game played
    /// is the one the playback will redo.
    pub(crate) fn fx_level_start(&mut self, seed: u64, replay: bool) {
        let opts = if self.fx_on { self.fx_opts } else { FxOptions::default() };
        if self.fx_on {
            // Recording a .rep: same simulation as at playback (golden master settings).
            let cfg = if replay || self.recorder.is_some() {
                tyrian_sim::fx::FxConfig { events: true, ..Default::default() }
            } else {
                self.fx_cfg
            };
            if let Some(sim) = self.sim.as_mut() {
                sim.set_fx_config(cfg);
            }
        }
        FX.with(|f| {
            let mut r = FxRender::new(seed);
            r.options = opts;
            *f.borrow_mut() = r;
        });
    }
}

#[godot_api(secondary)]
impl TyrianSim {
    /// Setup effects: Freeze Frames (`hitstop_enabled`), explosion
    /// style (0 Classic, 1 Retro+, 2 Modern), damage numbers
    /// (0 off, 1 on, 2 colored), rumble, particles (`particles_enabled`).
    /// Applied immediately to the current level (C globals) and to the
    /// following ones. No effect on a replay (golden master settings).
    #[func]
    fn set_effects(&mut self, hitstop: bool, explosion_style: i32, damage_numbers: i32, rumble: bool, particles: bool) {
        let style = explosion_style.clamp(0, 2) as u8;
        self.fx_on = true;
        self.fx_cfg = tyrian_sim::fx::FxConfig {
            hitstop,
            modern_explosions: particles
                && style == crate::fx_render::STYLE_MODERN
                && self.fx_opts.processor_type >= 2
                && self.fx_opts.shake,
            events: true,
        };
        self.fx_opts = FxOptions {
            particles,
            style,
            damage_numbers: damage_numbers > 0,
            colored: damage_numbers == 2,
            hitstop,
            rumble,
            bullet_time: self.fx_opts.bullet_time,
            shake: self.fx_opts.shake,
            processor_type: self.fx_opts.processor_type,
        };
        if self.replay.is_none() && self.recorder.is_none() {
            let cfg = self.fx_cfg;
            if let Some(sim) = self.sim.as_mut() {
                sim.set_fx_config(cfg);
            }
        }
        let opts = self.fx_opts;
        FX.with(|f| f.borrow_mut().options = opts);
    }

    /// Visual FX preset (`visual_fx.c`): `screenshake_enabled` and
    /// `processorType`. Controls shakes, boss / secret flashes,
    /// the Modern glow stack (`particles.c:glow_stack_active`, `varz.c:modern_fx`).
    #[func]
    fn set_screen_fx(&mut self, shake: bool, processor_type: i32) {
        self.fx_opts.shake = shake;
        self.fx_opts.processor_type = processor_type;
        self.fx_cfg.modern_explosions = self.fx_opts.particles
            && self.fx_opts.style == crate::fx_render::STYLE_MODERN
            && processor_type >= 2
            && shake;
        if self.fx_on && self.replay.is_none() && self.recorder.is_none() {
            let cfg = self.fx_cfg;
            if let Some(sim) = self.sim.as_mut() {
                sim.set_fx_config(cfg);
            }
        }
        let opts = self.fx_opts;
        FX.with(|f| f.borrow_mut().options = opts);
    }

    /// `screenshake_get_offset` of the current frame: integer (dx, dy) to
    /// add to the destination rectangle (video.c:449-455).
    #[func]
    fn shake_offset(&self) -> Vector2i {
        FX.with(|f| {
            let (x, y) = f.borrow().shake_offset();
            Vector2i::new(x, y)
        })
    }

    /// Frame presented outside the level ticks (pause menu, Quit box,
    /// help: JE_inGameSetup calls JE_showVGA on every turn): video.c:449
    /// decays the shake at each presentation. Returns the offset.
    #[func]
    fn shake_present_frame(&mut self) -> Vector2i {
        FX.with(|f| {
            let mut f = f.borrow_mut();
            f.shake_frame();
            let (x, y) = f.shake_offset();
            Vector2i::new(x, y)
        })
    }

    /// `bullet_time.c:bullet_time_active` (merging of numbers, fireballs).
    #[func]
    fn set_fx_bullet_time(&mut self, on: bool) {
        self.fx_opts.bullet_time = on;
        FX.with(|f| f.borrow_mut().options.bullet_time = on);
    }

    /// Rumbles requested since the last call (`rumble.c:rumble_trigger`):
    /// pairs (intensity 0..1, duration in ms).
    #[func]
    fn take_rumble(&mut self) -> PackedFloat32Array {
        FX.with(|f| {
            let mut f = f.borrow_mut();
            let mut out = PackedFloat32Array::new();
            for (i, ms) in f.rumble.drain(..) {
                out.push(i);
                out.push(ms as f32);
            }
            out
        })
    }

    /// `hitstop.c:kill_slowmo_update`: display speed (0.3 -> 1) after
    /// a defeated boss, or -1 outside slow motion.
    #[func]
    fn kill_slowmo_speed(&self) -> f32 {
        FX.with(|f| {
            let k = f.borrow().kill_slowmo;
            if k > 0.0 {
                0.3 + (1.0 - k) * 0.7
            } else {
                -1.0
            }
        })
    }

    /// Measurements: [live particles, fireballs, shockwaves,
    /// particles created since the start of the level, flash of the frame,
    /// remaining freeze frames, frozen enemy updates, random draws
    /// of the effects].
    #[func]
    fn fx_stats(&self) -> PackedInt64Array {
        let (freeze, frozen) = self.sim.as_ref().map_or((0, 0), |s| s.hitstop_state());
        FX.with(|f| {
            let f = f.borrow();
            PackedInt64Array::from(vec![
                f.particle_count() as i64,
                f.fireball_count() as i64,
                f.shockwave_count() as i64,
                f.spawned as i64,
                f.flash_frames() as i64,
                freeze as i64,
                frozen as i64,
                f.rng_draws as i64,
            ])
        })
    }

    /// Visible damage numbers, four at a time: x, y (game_screen), value, tint.
    #[func]
    fn damage_numbers(&self) -> PackedInt32Array {
        FX.with(|f| {
            let mut out = PackedInt32Array::new();
            for (x, y, v, h) in f.borrow().numbers() {
                out.extend([x, y, v as i32, h as i32]);
            }
            out
        })
    }
}
