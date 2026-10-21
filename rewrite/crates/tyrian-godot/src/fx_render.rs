//! Rendering of the Enhanced effects of the C: `particles.c` (particles, fire
//! balls, shockwaves, smoke rings, boss explosion chain),
//! `damagenumbers.c` (damage numbers), flash of `hitstop.c` and
//! rumble requests of `rumble.c`.
//!
//! The C runs these effects on the shared `rng.c` generator; under the
//! golden master they are off (`golden_master.c`: particles_enabled and
//! damage_numbers_enabled set to false), so the rewrite's simulation does not
//! know them. Here they have their own generator (same xoshiro256**
//! algorithm): the player sees the same effects, the simulation stays
//! the golden master one down to the tick (see docs/divergences.md).
//!
//! Input: the events noted by the simulation (`tyrian_sim::fx`), once
//! per tick, then the per-frame update of `JE_starShowVGA`
//! (`particles_update`) and of `tyrian2.c` (`damage_numbers_update_and_draw`).
//! Output: pixels of the RGBA layer (`draw_rgba`, composed by `glow_pixels`
//! like RGBAScreen) and numbers drawn into game_screen (`draw_numbers`).

use crate::modern_explosions::ModernExplosions;
use std::cell::RefCell;
use std::f32::consts::PI;
use tyrian_render::Surface;
use tyrian_sim::fx::FxEvent;
use tyrian_sim::rng::Xoshiro256;
use tyrian_sim::world::World;

thread_local! {
    /// Effects of the current level (only one level plays at a time, like the
    /// globals of particles.c / damagenumbers.c).
    pub(crate) static FX: RefCell<FxRender> = RefCell::new(FxRender::new(1));
}

/// `particles.h:EXPLOSION_STYLE_*`.
pub const STYLE_CLASSIC: u8 = 0;
pub const STYLE_RETRO_PLUS: u8 = 1;
pub const STYLE_MODERN: u8 = 2;

/// Additive glow of video_rgba.c: (image, x, y, radius, color, intensity).
pub type GlowFn = fn(&mut [[u8; 4]], i32, i32, i32, [u8; 3], f32);

/// Player settings read by the rendering (Setup).
#[derive(Clone, Copy)]
pub struct FxOptions {
    /// `particles.c:particles_enabled` (Visual FX preset, true by default).
    pub particles: bool,
    /// `particles.c:explosion_style` (Setup > Graphics > Explosions).
    pub style: u8,
    /// `damagenumbers.c:damage_numbers_enabled` / `colored_damage_numbers`.
    pub damage_numbers: bool,
    pub colored: bool,
    /// `hitstop.c:hitstop_enabled` (the boss flash depends on it).
    pub hitstop: bool,
    /// `rumble.c:rumble_enabled`.
    pub rumble: bool,
    /// `bullet_time.c:bullet_time_active`.
    pub bullet_time: bool,
    /// `screenshake.c:screenshake_enabled` (Visual FX preset).
    pub shake: bool,
    /// `config.c:processorType` (Visual FX preset: 1, 3 or 4).
    pub processor_type: i32,
}

impl Default for FxOptions {
    fn default() -> Self {
        Self {
            particles: false,
            style: STYLE_RETRO_PLUS,
            damage_numbers: false,
            colored: true,
            hitstop: false,
            rumble: false,
            bullet_time: false,
            shake: true,
            processor_type: 4,
        }
    }
}

const MAX_PARTICLES: usize = 1024;
/// Debris of one large explosion (`varz.c:JE_setupExplosionLarge`, the death
/// of a 2x2 enemy piece): hot embers and grey chunks. The C spawns 10 and 4;
/// halved at the maintainer's request (2026-10-20 playtest: too much debris
/// after medium enemies), which also halves their trails, impact dust and
/// landed embers. Bigger enemies and bosses chain several large explosions
/// and keep their own boss burst (`on_boss_defeated`: 15 embers, 6 chunks).
const LARGE_EXPLOSION_EMBERS: i32 = 5;
const LARGE_EXPLOSION_CHUNKS: i32 = 2;
/// `particles.c:CHUNK_GROUND_Y` = PLAY_AREA_HEIGHT - 6.
const CHUNK_GROUND_Y: f32 = 184.0 - 6.0;
const DEFAULT_GRAVITY: f32 = 0.05;
/// `particles.h:GAME_COORD_X_OFFSET`.
const GX: i32 = -24;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Normal,
    Chunk,
    Ember,
    EmberLanded,
}

#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: i32,
    max_life: i32,
    color_start: u32,
    color_end: u32,
    gravity: f32,
    size: u8,
    manual_pos: bool,
    twinkle: bool,
    kind: Kind,
    tumble_phase: u8,
    delay: u8,
}

#[derive(Clone, Copy)]
struct Shockwave {
    cx: i32,
    cy: i32,
    frame: i32,
    max_frames: i32,
    max_radius: i32,
    color: u32,
}

#[derive(Clone, Copy)]
struct Fireball {
    cx: i32,
    cy: i32,
    frame: i32,
    max_radius: i32,
}

#[derive(Clone, Copy)]
struct SmokeRing {
    cx: i32,
    cy: i32,
    radius: i32,
    frame: i32,
    max_frames: i32,
}

#[derive(Clone, Copy)]
struct BossChain {
    cx: i32,
    cy: i32,
    pending: i32,
    cooldown: i32,
}

#[derive(Clone, Copy, Default)]
struct DamageNumber {
    x: i32,
    y: i32,
    x0: i32,
    y0: i32,
    vx: i32,
    age: i32,
    dmg: u32,
    ttl: i32,
    color: u8,
}

/// `particles.c:fireball_phases`.
const FIREBALL_PHASES: [(u32, f32, f32); 15] = [
    (0xFFF8C0, 0.30, 0.85),
    (0xFFF8C0, 0.40, 0.85),
    (0xFFF8C0, 0.50, 0.70),
    (0xFFC830, 0.60, 0.60),
    (0xFFC830, 0.70, 0.55),
    (0xFF6A20, 0.80, 0.50),
    (0xFF6A20, 0.90, 0.45),
    (0xFF6A20, 1.00, 0.40),
    (0xFF6A20, 1.00, 0.35),
    (0xFF6A20, 1.00, 0.30),
    (0x8C1410, 1.00, 0.20),
    (0x8C1410, 0.95, 0.16),
    (0x8C1410, 0.90, 0.12),
    (0x8C1410, 0.85, 0.08),
    (0x8C1410, 0.78, 0.04),
];

const MAX_DAMAGE_NUMBERS: usize = 48;
const DAMAGE_NUMBER_TTL: i32 = 24;

/// Effects state of a level (one instance per TyrianSim).
pub struct FxRender {
    pub options: FxOptions,
    rng: Xoshiro256,
    particles: Vec<Particle>,
    stage_delay: u8,
    shockwaves: Vec<Shockwave>,
    fireballs: Vec<Fireball>,
    smoke_rings: Vec<SmokeRing>,
    boss_chains: Vec<BossChain>,
    last_chain: (u32, i32, i32),
    last_map_x_ofs: i32,
    scroll_delta: i32,
    scroll_primed: bool,
    slowmo_flip: bool,
    /// `video_rgba.c:screen_flash_frames` / `screen_flash_color`.
    flash_frames: i32,
    flash_color: u32,
    /// Value of `screen_flash_frames` when drawing the current frame.
    flash_draw: i32,
    numbers: [DamageNumber; MAX_DAMAGE_NUMBERS],
    next_slot: usize,
    /// Glows placed during the frame (impacts, absorption, thruster):
    /// (x, y, radius, color, intensity), in screen coordinates.
    glows: Vec<(i32, i32, i32, u32, f32)>,
    /// `rumble.c:rumble_trigger` requested since the last read:
    /// (intensity, duration ms).
    pub rumble: Vec<(f32, i32)>,
    /// `hitstop.c:kill_slowmo` (display speed after a boss).
    pub kill_slowmo: f32,
    /// Level palette (768 bytes), for `palette32`.
    palette: Vec<u8>,
    /// Events waiting for the frame where the C draws them.
    pending: Vec<(u64, FxEvent)>,
    /// Particles created since the start of the level (measurement).
    pub spawned: u64,
    /// Random draws made by the effects: those the C would take from the
    /// shared `rng.c` generator (isolation measurement, docs/divergences.md).
    pub rng_draws: u64,
    /// `screenshake.c`: random intensity, directional hit (axis, strength).
    shake_intensity: f32,
    bump: (f32, f32, f32),
    /// Integer offset of the current frame (`screenshake_get_offset`).
    shake_offset: (i32, i32),
    /// Draws of the rendering only (ADR 0003): never the simulation.
    shake_rng: Xoshiro256,
    /// Modern explosions on top of those of the C (`modern_explosions`).
    pub(crate) modern: ModernExplosions,
    /// Layer pixels to leave untouched: enemy shots and their edge
    /// (set by playfield before draw_rgba).
    pub(crate) reserve: Vec<bool>,
    /// The layer is composed over the 8-bit image (Original art): the
    /// embers of the Modern explosions take the colors of the level palette
    /// (set by playfield before draw_rgba).
    pub(crate) palette_art: bool,
}

impl FxRender {
    pub fn new(seed: u64) -> Self {
        Self {
            options: FxOptions::default(),
            rng: Xoshiro256::new(seed ^ 0x5041_5254_4943_4c45),
            particles: Vec::with_capacity(MAX_PARTICLES),
            stage_delay: 0,
            shockwaves: Vec::new(),
            fireballs: Vec::new(),
            smoke_rings: Vec::new(),
            boss_chains: Vec::new(),
            last_chain: (0, 0, 0),
            last_map_x_ofs: 0,
            scroll_delta: 0,
            scroll_primed: false,
            slowmo_flip: false,
            flash_frames: 0,
            flash_color: 0xFFFFFF,
            flash_draw: 0,
            numbers: [DamageNumber::default(); MAX_DAMAGE_NUMBERS],
            next_slot: 0,
            glows: Vec::new(),
            rumble: Vec::new(),
            kill_slowmo: 0.0,
            palette: Vec::new(),
            pending: Vec::new(),
            spawned: 0,
            rng_draws: 0,
            shake_intensity: 0.0,
            bump: (0.0, 0.0, 0.0),
            shake_offset: (0, 0),
            shake_rng: Xoshiro256::new(seed ^ 0x5348_414b_4521_0001),
            modern: ModernExplosions::new(seed),
            reserve: Vec::new(),
            palette_art: false,
        }
    }

    pub fn set_palette(&mut self, pal: &[u8]) {
        if pal.len() >= 768 {
            self.palette = pal[..768].to_vec();
        }
    }

    /// Number of live particles (measurement, tests).
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    pub fn fireball_count(&self) -> usize {
        self.fireballs.len()
    }

    pub fn shockwave_count(&self) -> usize {
        self.shockwaves.len()
    }

    /// Visible damage numbers: (x, y, value, tint).
    pub fn numbers(&self) -> Vec<(i32, i32, u32, u8)> {
        let hue = |n: &DamageNumber| if self.options.colored { n.color } else { 15 };
        self.numbers.iter().filter(|n| n.ttl > 0).map(|n| (n.x, n.y, n.dmg, hue(n))).collect()
    }

    /// Alpha/80 of the current frame's flash (0 = none).
    #[allow(clippy::misnamed_getters)] // drawn value (flash_draw), not the C counter
    pub fn flash_frames(&self) -> i32 {
        self.flash_draw
    }

    /// `palette32[i]` as 0xRRGGBB.
    fn palette32(&self, i: usize) -> u32 {
        if self.palette.len() < 768 {
            return 0x808080;
        }
        let p = &self.palette[i * 3..i * 3 + 3];
        ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32
    }

    fn next_float(&mut self) -> f32 {
        self.rng_draws += 1;
        self.rng.next_float_bits() as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, min: i32, max: i32) -> i32 {
        if min < max {
            self.rng_draws += 1;
        }
        self.rng.range(min, max)
    }

    fn next(&mut self) -> u32 {
        self.rng_draws += 1;
        self.rng.next_u32()
    }

    // --- particles.c: creation ---------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn spawn_full(
        &mut self,
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        life: i32,
        color_start: u32,
        color_end: u32,
        gravity: f32,
        size: u8,
        manual_pos: bool,
        twinkle: bool,
    ) {
        if self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let tumble_phase = (self.next() & 3) as u8;
        self.particles.push(Particle {
            x,
            y,
            vx,
            vy,
            life,
            max_life: life,
            color_start,
            color_end,
            gravity,
            size,
            manual_pos,
            twinkle,
            kind: Kind::Normal,
            tumble_phase,
            delay: self.stage_delay,
        });
        self.spawned += 1;
    }

    /// Modern style with particles: the Modern explosions are drawn and the
    /// hollow ring ending the explosion sprites gives way to their embers
    /// (`modern_explosions::ring_phase_frame`).
    pub(crate) fn modern_explosions_on(&self) -> bool {
        self.options.particles && self.options.style == STYLE_MODERN
    }

    /// `particles.c:explosion_scaled`.
    fn explosion_scaled(&self, mut count: i32) -> i32 {
        let n = self.particles.len();
        if n > 700 {
            count /= 3;
        } else if n > 450 {
            count /= 2;
        }
        count.max(1)
    }

    /// `particles.c:glow_stack_active`: Modern style, processorType >= 2 and
    /// screenshake active (the RGBA layer always exists on the Godot side).
    fn glow_stack_active(&self) -> bool {
        self.options.style == STYLE_MODERN && self.modern_stack()
    }

    /// `processorType >= 2 && screenshake_enabled`: gate of the whole stack
    /// of modern effects of the C (pickup flash, shield absorption...).
    fn modern_stack(&self) -> bool {
        self.options.processor_type >= 2 && self.options.shake
    }

    // --- screenshake.c --------------------------------------------------

    /// `screenshake_trigger`: keeps the strongest.
    fn shake_trigger(&mut self, intensity: f32) {
        if intensity > self.shake_intensity {
            self.shake_intensity = intensity;
        }
    }

    /// `screenshake_bump(dir, magnitude)`: keeps the strongest hit.
    fn shake_bump(&mut self, dx: f32, dy: f32, magnitude: f32) {
        let mag = (dx * dx + dy * dy).sqrt();
        if mag < 0.01 || magnitude <= 0.0 || magnitude <= self.bump.2 {
            return;
        }
        self.bump = (dx / mag, dy / mag, magnitude);
    }

    /// `screenshake_update` then `screenshake_get_offset`, once per frame
    /// (video.c:scale_and_flip).
    pub(crate) fn shake_frame(&mut self) {
        self.shake_intensity = if self.shake_intensity < 0.5 { 0.0 } else { self.shake_intensity * 0.90 };
        self.bump.2 = if self.bump.2 > 0.5 { self.bump.2 - 0.5 } else { 0.0 };
        if !self.options.shake {
            self.shake_offset = (0, 0);
            return;
        }
        let r = self.shake_intensity as i32;
        let on = self.shake_intensity >= 0.5 && r >= 1;
        let rx = if on { self.shake_rng.range(-r, r) } else { 0 };
        let ry = if on { self.shake_rng.range(-r, r) } else { 0 };
        let (bx, by) = if self.bump.2 >= 0.5 {
            ((self.bump.0 * self.bump.2) as i32, (self.bump.1 * self.bump.2) as i32)
        } else {
            (0, 0)
        };
        self.shake_offset = (rx + bx, ry + by);
    }

    /// `screenshake.c`: event listeners (kill, player hit, death,
    /// boss defeated).
    pub(crate) fn shake_on_event(&mut self, ev: &FxEvent) {
        match *ev {
            FxEvent::EnemyKilled { value, .. } => {
                if value >= 1000 {
                    self.shake_trigger(12.0);
                    self.shake_bump(0.0, -1.0, 4.0);
                } else if value >= 300 {
                    self.shake_trigger(6.0);
                    self.shake_bump(0.0, -1.0, 2.5);
                } else if value >= 100 {
                    self.shake_bump(0.0, -1.0, 1.5);
                }
            }
            FxEvent::PlayerHit { .. } => self.shake_trigger(10.0),
            FxEvent::PlayerDied { .. } => self.shake_trigger(20.0),
            FxEvent::BossDefeated { .. } => self.shake_trigger(16.0),
            _ => {}
        }
    }

    /// Frame offset to add to the destination rectangle.
    pub fn shake_offset(&self) -> (i32, i32) {
        self.shake_offset
    }

    fn tag_last(&mut self, kind: Kind) {
        if let Some(p) = self.particles.last_mut() {
            p.kind = kind;
        }
    }

    /// `particles.c:spawn_impact_dust`.
    fn spawn_impact_dust(&mut self, x: f32, y: f32) {
        for _ in 0..3 {
            let vx = (self.next_float() - 0.5) * 2.4;
            let vy = -0.3 - self.next_float() * 0.5;
            let life = 8 + self.range(-2, 3);
            let px = x + (self.next_float() - 0.5) * 2.0;
            self.spawn_full(px, y, vx, vy, life, 0xFFB0B0B4, 0x00303034, 0.06, 1, false, false);
        }
    }

    /// `particles.c:spawn_chunk_trail`.
    fn spawn_chunk_trail(&mut self, c: Particle) {
        let s = c.color_start;
        let dim = 0xFF00_0000 | ((((s >> 16) & 0xFF) / 2) << 16) | ((((s >> 8) & 0xFF) / 2) << 8) | ((s & 0xFF) / 2);
        self.spawn_full(c.x, c.y, 0.0, 0.0, 5, dim, 0x00101014, 0.0, 1, false, false);
    }

    /// `particles.c:particles_spawn_burst_directed`.
    #[allow(clippy::too_many_arguments)]
    fn spawn_burst_directed(
        &mut self,
        x: i32,
        y: i32,
        count: i32,
        color: u32,
        speed: f32,
        lifetime: i32,
        shot_dx: i32,
        shot_dy: i32,
    ) {
        let base = if shot_dx == 0 && shot_dy == 0 { -PI / 2.0 } else { (-shot_dy as f32).atan2(-shot_dx as f32) };
        let color_end = 0x0040_0000 | (color & 0xFF00_0000);
        let directed = count * 7 / 10;
        let omni = count - directed;
        let size = if count >= 10 { 2 } else { 1 };
        for _ in 0..directed {
            let angle = base + (self.next_float() - 0.5) * (PI / 2.5);
            let spd = speed * (0.7 + self.next_float() * 0.6);
            let life = lifetime + self.range(-lifetime / 4, lifetime / 4);
            self.spawn_full(
                x as f32,
                y as f32,
                angle.cos() * spd,
                angle.sin() * spd,
                life,
                color,
                color_end,
                DEFAULT_GRAVITY,
                size,
                false,
                false,
            );
        }
        for _ in 0..omni {
            let angle = self.next_float() * 2.0 * PI;
            let spd = speed * (0.3 + self.next_float() * 0.4);
            let life = lifetime * 2 / 3 + self.range(-lifetime / 6, lifetime / 6);
            self.spawn_full(
                x as f32,
                y as f32,
                angle.cos() * spd,
                angle.sin() * spd,
                life,
                color,
                color_end,
                DEFAULT_GRAVITY,
                size,
                false,
                false,
            );
        }
    }

    /// `particles.c:particles_spawn_smoke`.
    fn spawn_smoke(&mut self, x: i32, y: i32, color: u32) {
        let vx = (self.next_float() - 0.5) * 0.3;
        let vy = -0.4 - self.next_float() * 0.3;
        let life = 18 + self.range(-2, 4);
        let color_end = ((color & 0x00FF_FFFF) | 0x0020_2020) & 0x3F80_8080;
        let px = x as f32 + (self.next_float() - 0.5) * 3.0;
        self.spawn_full(px, y as f32, vx, vy, life, color, color_end, -0.01, 2, false, false);
    }

    /// `particles.c:particles_spawn_enemy_trail`.
    fn spawn_enemy_trail(&mut self, x: i32, y: i32) {
        let jx = (self.next_float() - 0.5) * 2.0;
        let jy = (self.next_float() - 0.5) * 2.0;
        self.spawn_full(x as f32 + jx, y as f32 + jy, 0.0, 0.0, 10, 0xFFD0E0FF, 0x00203050, 0.0, 1, false, false);
    }

    /// `particles.c:particles_spawn_exhaust` (particles_exhaust true), with
    /// its glow (processorType >= 2). `now_ms`: SDL_GetTicks.
    fn spawn_exhaust(&mut self, w: &World, now_ms: u32) {
        let p = &w.players[0];
        let (x, y) = (p.x - 18, p.y + 21);
        let gen = (p.items.generator.0 as i32).clamp(1, 5);
        const START: [u32; 5] = [0xFFE8E8F0, 0xFFD8E8F8, 0xFFB8E0FF, 0xFFA0D8FF, 0xFF90D0FF];
        const END: [u32; 5] = [0x00202030, 0x00202038, 0x00102030, 0x00102040, 0x00102048];
        let (start, end) = (START[gen as usize - 1], END[gen as usize - 1]);
        let dy = p.y - p.last_y_explosion_follow;
        let thrust = if dy < 0 { ((-dy) as f32).min(3.0) } else { 0.0 };
        let thrust_mult = 1.0 + thrust * 0.3;
        let base = 2 + (gen - 1) / 2;
        let count = (((base + (self.next() % 2) as i32) as f32 * thrust_mult) as i32).max(1);
        for _ in 0..count {
            let vx = (self.next_float() - 0.5) * 0.3;
            let vy = (0.5 + self.next_float() * 0.6) * thrust_mult;
            let dim = self.next() % 16;
            let mut js = start;
            if dim != 0 {
                let c = |v: u32| if v > dim { v - dim } else { v };
                js = (start & 0xFF00_0000)
                    | (c((start >> 16) & 0xFF) << 16)
                    | (c((start >> 8) & 0xFF) << 8)
                    | c(start & 0xFF);
            }
            let px = x as f32 + (self.next_float() - 0.5) * 2.0;
            self.spawn_full(px, y as f32, vx, vy, 12, js, end, DEFAULT_GRAVITY, 2, true, false);
        }
        let pulse = 0.85 + 0.15 * (now_ms as f32 * 0.006).sin();
        let radius = 3 + (gen - 1) / 2 + if thrust > 0.0 { 1 } else { 0 };
        let intensity = (0.12 + 0.03 * gen as f32) * pulse * thrust_mult;
        self.glows.push((x, y + 2, radius, start & 0x00FF_FFFF, intensity));
    }

    /// `particles.c:shockwave_trigger_sized`.
    fn shockwave(&mut self, x: i32, y: i32, color: u32, max_radius: i32, max_frames: i32) {
        if self.shockwaves.len() < 8 {
            self.shockwaves.push(Shockwave { cx: x, cy: y, frame: 0, max_frames, max_radius, color });
        }
    }

    /// `particles.c:fireball_spawn`.
    fn fireball_spawn(&mut self, cx: i32, cy: i32, large: bool, combo_multiplier: u32) {
        if self.fireballs.len() >= 12 {
            return;
        }
        let base = if large { 22 } else { 11 };
        let mut r = base + (self.next() % 5) as i32 - 2;
        if combo_multiplier >= 4 {
            let bonus = if combo_multiplier >= 8 { 30 } else { 20 };
            r += r * bonus / 100;
        }
        self.fireballs.push(Fireball { cx, cy, frame: 0, max_radius: r });
    }

    /// `particles.c:smoke_ring_spawn`.
    fn smoke_ring_spawn(&mut self, cx: i32, cy: i32, base_radius: i32) {
        if self.smoke_rings.len() >= 8 {
            return;
        }
        if self
            .smoke_rings
            .iter()
            .any(|s| (s.cx - cx).abs() < 40 && (s.cy - cy).abs() < 40 && s.frame < s.max_frames / 2)
        {
            return;
        }
        self.smoke_rings.push(SmokeRing {
            cx,
            cy,
            radius: (base_radius as f32 * 1.5) as i32,
            frame: 0,
            max_frames: 45,
        });
    }

    /// `particles.c:spawn_raiden_thermal`.
    fn spawn_thermal(&mut self, cx: i32, cy: i32, count: i32) {
        const START: [u32; 3] = [0xFFFFFFFF, 0xFFFFE050, 0xFFFF8000];
        const END: [u32; 3] = [0xFFFFE050, 0xFFFF4010, 0x00200000];
        let count = self.explosion_scaled(count);
        for _ in 0..count {
            let stage = (self.next() % 3) as usize;
            let angle = self.next_float() * 2.0 * PI;
            let spd = 1.1 + self.next_float() * 1.6;
            let life = 20 + self.range(-4, 8);
            let px = cx as f32 + (self.next_float() - 0.5) * 3.0;
            let py = cy as f32 + (self.next_float() - 0.5) * 3.0;
            self.spawn_full(
                px,
                py,
                angle.cos() * spd,
                angle.sin() * spd,
                life,
                START[stage],
                END[stage],
                0.04,
                1,
                false,
                false,
            );
            self.tag_last(Kind::Ember);
        }
    }

    /// `particles.c:spawn_raiden_chunks`.
    fn spawn_chunks(&mut self, cx: i32, cy: i32, count: i32, ground: bool) {
        let light = if ground { 0xFFA8A29E } else { 0xFFBCBCC8 };
        let dark = if ground { 0xFF6E6660 } else { 0xFF8C8C98 };
        let end = if ground { 0xFF1E1C1A } else { 0xFF202028 };
        let count = self.explosion_scaled(count);
        for _ in 0..count {
            let angle = self.next_float() * PI;
            let spd = 1.8 + self.next_float() * 1.4;
            let vx = angle.cos() * spd;
            let vy = -angle.sin() * spd * 0.9;
            let life = 28 + self.range(-4, 10);
            let start = if self.next() & 1 != 0 { light } else { dark };
            self.spawn_full(cx as f32, cy as f32, vx, vy, life, start, end, 0.16, 2, false, false);
            self.tag_last(Kind::Chunk);
        }
    }

    /// `particles.c:particles_spawn_explosion_fx`.
    fn explosion_fx(&mut self, x: i32, y: i32, large: bool, ground: bool, combo_multiplier: u32) {
        if self.options.style == STYLE_CLASSIC {
            return;
        }
        let cx = x + 6;
        let cy = if large { y } else { y + 7 };
        if self.glow_stack_active() {
            self.fireball_spawn(cx, cy, large, combo_multiplier);
        }
        let count = self.explosion_scaled(if large { 3 } else { 2 });
        self.stage_delay = 2;
        for _ in 0..count {
            let angle = self.next_float() * 2.0 * PI;
            let spd = (if large { 1.4 } else { 1.0 }) * (0.5 + self.next_float() * 0.7);
            let life = (if large { 18 } else { 13 }) + self.range(-3, 3);
            self.spawn_full(
                cx as f32,
                cy as f32,
                angle.cos() * spd,
                angle.sin() * spd,
                life,
                0xFFFF7020,
                0x00301000,
                0.03,
                2,
                false,
                false,
            );
        }
        self.stage_delay = 0;
        if large {
            self.stage_delay = 10;
            let vx = (self.next_float() - 0.5) * 0.4;
            let vy = -0.3 - self.next_float() * 0.4;
            let life = 28 + self.range(-4, 6);
            let px = cx as f32 + (self.next_float() - 0.5) * 6.0;
            self.spawn_full(px, cy as f32, vx, vy, life, 0xFFA0A0A0, 0x00101010, -0.015, 2, false, false);
            self.stage_delay = 4;
            self.spawn_thermal(cx, cy, LARGE_EXPLOSION_EMBERS);
            self.stage_delay = 5;
            self.spawn_chunks(cx, cy, LARGE_EXPLOSION_CHUNKS, ground);
            self.stage_delay = 0;
        }
    }

    // --- Listeners (event_bus.c), in the registration order of opentyr.c --

    fn on_event(&mut self, ev: FxEvent, w: &World, now_ms: u32) {
        self.shake_on_event(&ev);
        let o = self.options;
        let hue_of = |frame: u16| -> i32 {
            crate::vga::with_assets(|a| {
                a.sheet("main:7").map(|s| crate::playfield::sprite2_dominant_hue(s, frame as u32, 15) as i32)
            })
            .flatten()
            .unwrap_or(15)
        };
        match ev {
            FxEvent::ShotHit { x, y, damage, frame } => {
                let hue = hue_of(frame);
                // damagenumbers.c:damagenumbers_on_shot_hit.
                self.damage_number_spawn(x, y, damage, if o.colored { hue as u8 } else { 15 });
                // particles.c:on_shot_hit.
                if o.particles && !(damage < 3 && !self.next().is_multiple_of(3)) {
                    if damage >= 3 && self.glow_stack_active() {
                        self.glows.push((x + GX, y, 3, 0xFFF8C0, 0.30));
                    }
                    let hue = if hue > 0 { hue } else { 15 } as usize;
                    let color = 0xFF00_0000 | self.palette32((hue << 4) | 0x0C);
                    let color_end = 0xFF00_0000 | self.palette32((hue << 4) | 0x04);
                    let count = if damage >= 10 { 4 } else { 2 };
                    for _ in 0..count {
                        let angle = self.next_float() * 2.0 * PI;
                        let spd = 0.5 + self.next_float() * 0.8;
                        let life = 8 + self.range(-2, 4);
                        self.spawn_full(
                            x as f32,
                            y as f32,
                            angle.cos() * spd,
                            angle.sin() * spd,
                            life,
                            color,
                            color_end,
                            0.02,
                            1,
                            false,
                            true,
                        );
                    }
                }
            }
            FxEvent::HitRipple { x, y } | FxEvent::PlayerRipple { x, y } => {
                if o.particles {
                    self.shockwave(x, y, 0xFFF4E0B0, 8, 5);
                }
            }
            FxEvent::EnemyKilled { x, y, damage, value, frame, shot_dx, shot_dy } => {
                // rumble.c:on_enemy_killed.
                if o.rumble {
                    self.rumble.push(if value >= 500 { (0.6, 200) } else { (0.2, 100) });
                }
                // damagenumbers.c:damagenumbers_on_enemy_killed.
                let hue = hue_of(frame);
                self.damage_number_spawn(x, y, damage, if o.colored { hue as u8 } else { 15 });
                // particles.c:on_enemy_killed.
                if o.particles && o.style != STYLE_CLASSIC {
                    let count = if value >= 2000 {
                        30
                    } else if value >= 500 {
                        14
                    } else if value >= 100 {
                        8
                    } else {
                        4
                    };
                    let count = self.explosion_scaled(count);
                    self.stage_delay = 3;
                    self.spawn_burst_directed(x, y, count, 0xFFFFB030, 2.0, 25, shot_dx, shot_dy);
                    self.stage_delay = 0;
                    if value >= 500 && self.glow_stack_active() {
                        const PRESETS: [(f32, f32); 5] = [(0.75, 1.0), (1.0, 1.0), (1.3, 1.2), (1.0, 1.5), (1.1, 0.8)];
                        let (rm, fm) = PRESETS[(self.next() % 5) as usize];
                        let r = ((30.0 * rm) as i32).max(4);
                        let f = ((11.0 * fm) as i32).max(4);
                        let c = if self.next().is_multiple_of(5) { 0xFFF4E0B0 } else { 0xFFE8E8F0 };
                        self.shockwave(x + 6, y, c, r, f);
                        if value >= 2000 {
                            self.smoke_ring_spawn(x + 6, y, 60);
                        }
                    }
                }
            }
            FxEvent::BossDefeated { x, y, frame, .. } => {
                // hitstop.c:on_boss_defeated (rendering part): shockwave,
                // slow motion and flash (processorType >= 2).
                let hue = hue_of(frame);
                let hue = if hue > 0 { hue } else { 15 } as usize;
                let kill_color = self.palette32((hue << 4) | 0x0F) | 0x808080;
                self.shockwave(x, y, kill_color, 120, 20);
                // hitstop.c:on_boss_defeated: processorType < 2 exits here,
                // with no slow motion or flash (nor cascade, on the simulation side).
                if self.options.processor_type >= 2 {
                    self.kill_slowmo = 1.0;
                    self.flash_frames = 3;
                    self.flash_color = kill_color;
                }
                // particles.c:on_boss_defeated_particles.
                if o.particles && o.style != STYLE_CLASSIC {
                    let (cx, cy) = (x + 6, y);
                    self.stage_delay = 4;
                    self.spawn_thermal(cx, cy, 15);
                    self.stage_delay = 5;
                    self.spawn_chunks(cx, cy, 6, false);
                    self.stage_delay = 0;
                    if self.glow_stack_active() {
                        let (t, lx, ly) = self.last_chain;
                        if !(t != 0 && now_ms.wrapping_sub(t) < 1500 && (cx - lx).abs() < 32 && (cy - ly).abs() < 32) {
                            self.last_chain = (now_ms, cx, cy);
                            let n = 6 + (self.next() % 5) as i32;
                            if self.boss_chains.len() >= 3 {
                                self.boss_chains.remove(0);
                            }
                            self.boss_chains.push(BossChain { cx, cy, pending: n, cooldown: 1 });
                        }
                    }
                }
            }
            FxEvent::ShieldAbsorb { x, y } => {
                // varz.c:JE_playerDamage : processorType >= 2 && screenshake && particles.
                if o.particles && self.modern_stack() {
                    self.glows.push((x + 1 + GX, y + 2, 18, 0x40D0FF, 0.55));
                }
            }
            FxEvent::PlayerHit { .. } => {
                if o.rumble {
                    self.rumble.push((0.8, 300));
                }
            }
            FxEvent::PlayerDied { .. } => {
                if o.rumble {
                    self.rumble.push((1.0, 500));
                }
            }
            FxEvent::Explosion { x, y, large, ground } => {
                if o.particles {
                    self.explosion_fx(x, y, large, ground, w.combo_multiplier);
                }
            }
            FxEvent::EnemyTrail { x, y } => {
                if o.particles {
                    self.spawn_enemy_trail(x, y);
                }
            }
            FxEvent::EnemySmoke { x, y } => {
                if o.particles {
                    self.spawn_smoke(x, y, 0xFFE0E0E0);
                }
            }
            FxEvent::SecretWarp => {
                // hitstop.c:on_powerup_collected : processorType >= 2 && screenshake.
                if self.modern_stack() {
                    self.flash_frames = 3;
                    self.flash_color = 0xFFFFFF;
                }
            }
        }
    }

    // --- damagenumbers.c ----------------------------------------------------

    /// `nortsong.c:game_speed`: 1 at rest, `BULLET_TIME_SPEED` (0.25) in slow motion,
    /// ramp 0.3 -> 1 after a boss (`hitstop.c:kill_slowmo_update`, which takes precedence).
    fn game_speed(&self) -> f32 {
        if self.kill_slowmo > 0.0 {
            0.3 + (1.0 - self.kill_slowmo) * 0.7
        } else if self.options.bullet_time {
            0.25
        } else {
            1.0
        }
    }

    /// `damagenumbers.c:damage_number_spawn` (merging depending on `game_speed`).
    fn damage_number_spawn(&mut self, x: i32, y: i32, dmg: i32, color: u8) {
        if !self.options.damage_numbers || dmg <= 0 {
            return;
        }
        let spawn_y = y - 6;
        let (merge_min_ttl, merge_dist) = damage_merge_params(self.game_speed(), self.options.bullet_time);
        for n in self.numbers.iter_mut() {
            if n.ttl < merge_min_ttl || n.color != color {
                continue;
            }
            let (dx, dy) = (n.x - x, n.y - spawn_y);
            if dx * dx + dy * dy <= merge_dist * merge_dist {
                n.dmg += dmg as u32;
                n.ttl = DAMAGE_NUMBER_TTL;
                return;
            }
        }
        // damage_numbers_math.c:damage_number_pick_slot.
        let slot = (0..MAX_DAMAGE_NUMBERS)
            .map(|i| (self.next_slot + i) % MAX_DAMAGE_NUMBERS)
            .find(|&i| self.numbers[i].ttl <= 0)
            .unwrap_or_else(|| (0..MAX_DAMAGE_NUMBERS).min_by_key(|&i| self.numbers[i].ttl).unwrap_or(0));
        self.next_slot = (slot + 1) % MAX_DAMAGE_NUMBERS;
        let vx = self.range(-16, 16);
        self.numbers[slot] = DamageNumber {
            x,
            y: spawn_y,
            x0: x,
            y0: spawn_y,
            vx,
            age: 0,
            dmg: dmg as u32,
            ttl: DAMAGE_NUMBER_TTL,
            color,
        };
    }

    /// `damagenumbers.c:damage_numbers_update_and_draw`, update
    /// (processorType >= 2: parabolic arc, `damage_numbers_math.c`).
    fn damage_numbers_update(&mut self) {
        let classic = self.options.processor_type < 2;
        for n in self.numbers.iter_mut() {
            if n.ttl <= 0 {
                continue;
            }
            n.ttl -= 1;
            n.age += 1;
            if classic {
                // Classic (processorType < 2): 1 px every DAMAGE_NUMBER_RISE_RATE (2) frames.
                if n.ttl % 2 == 0 {
                    n.y -= 1;
                }
                continue;
            }
            let a = n.age.clamp(0, DAMAGE_NUMBER_TTL);
            n.y = n.y0 + -(a * (DAMAGE_NUMBER_TTL - a)) / 10;
            n.x = n.x0 + (n.vx * a) / 8;
        }
    }

    /// Drawing of the numbers into game_screen (after the level texts,
    /// before JE_drawSP), fonts and tiers of `damagenumbers.c`.
    pub fn draw_numbers(&self, surf: &mut Surface, a: &tyrian_render::Assets) {
        use tyrian_render::assets::{SMALL_FONT_SHAPES, TINY_FONT};
        use tyrian_render::font::{self, Align};
        if !self.options.damage_numbers {
            return;
        }
        for n in self.numbers.iter() {
            if n.ttl <= 0 || n.x < 0 || n.x > 260 || n.y < 0 || n.y > 183 {
                continue;
            }
            let brightness = (n.ttl * 4 / DAMAGE_NUMBER_TTL).min(4);
            let buf = n.dmg.to_string();
            let s = buf.as_bytes();
            let hue = if self.options.colored { n.color } else { 15 };
            if n.dmg < 8 {
                font::text_shade(surf, a, n.x, n.y, s, hue, brightness, font::FULL_SHADE);
            } else if n.dmg < 25 {
                font::draw_font_hv_shadow(
                    surf,
                    a,
                    n.x,
                    n.y,
                    s,
                    TINY_FONT,
                    Align::Left,
                    hue,
                    (-3 + brightness) as i8,
                    true,
                    1,
                );
            } else if n.dmg < 60 {
                font::draw_font_hv_shadow(
                    surf,
                    a,
                    n.x,
                    n.y,
                    s,
                    SMALL_FONT_SHAPES,
                    Align::Left,
                    hue,
                    (-3 + brightness) as i8,
                    true,
                    1,
                );
            } else {
                font::draw_font_hv_shadow(
                    surf,
                    a,
                    n.x + 1,
                    n.y,
                    s,
                    SMALL_FONT_SHAPES,
                    Align::Left,
                    hue,
                    (-3 + brightness) as i8,
                    true,
                    1,
                );
                font::draw_font_hv_shadow(
                    surf,
                    a,
                    n.x,
                    n.y,
                    s,
                    SMALL_FONT_SHAPES,
                    Align::Left,
                    hue,
                    (-1 + brightness) as i8,
                    true,
                    1,
                );
            }
        }
    }

    // --- One frame -----------------------------------------------------------

    /// After a tick: events of the presented frame (iteration
    /// `presented`), then the C per-frame updates in the order of
    /// tyrian2.c (damage numbers, slow motion) and of JE_starShowVGA
    /// (thruster, scrolling, particles_update, shockwave_update).
    pub fn frame(&mut self, events: Vec<(u64, FxEvent)>, w: &World, presented: u64, now_ms: u32) {
        self.pending.extend(events);
        self.glows.clear();
        let ready: Vec<FxEvent> = {
            let (now, later): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|(it, _)| *it <= presented);
            self.pending = later;
            now.into_iter().map(|(_, e)| e).collect()
        };
        // Modern explosions: Modern style and particles active, in addition to the
        // Modern explosions of the C.
        let spawn = self.modern_explosions_on();
        if spawn || self.modern.live_count() > 0 {
            self.modern.frame(&ready, w, spawn);
            // Secondary explosions: Modern explosions of the C placed by the rendering.
            for (x, y, large, ground) in std::mem::take(&mut self.modern.ready_pops) {
                self.explosion_fx(x, y, large, ground, w.combo_multiplier);
            }
        }
        for ev in ready {
            self.on_event(ev, w, now_ms);
        }
        // video.c:scale_and_flip: decay then frame offset.
        self.shake_frame();
        // particles.c:boss_chain_update: without screenshake, the chains stop.
        if !self.options.shake {
            self.boss_chains.clear();
        }
        // enemy_render.c:JE_starShowVGA: flash drawn then counted down.
        self.flash_draw = if self.options.hitstop { self.flash_frames } else { 0 };
        if self.flash_frames > 0 && self.options.hitstop {
            self.flash_frames -= 1;
        }
        self.damage_numbers_update();
        // hitstop.c:kill_slowmo_update.
        if self.kill_slowmo > 0.0 {
            self.kill_slowmo -= 0.006;
            if self.kill_slowmo <= 0.0 {
                self.kill_slowmo = 0.0;
            }
        }
        if self.options.particles {
            self.spawn_exhaust(w, now_ms);
        }
        // particles_set_scroll(tempMapXOfs).
        let ofs = w.background.temp_map_x_ofs as i32;
        if !self.scroll_primed {
            self.last_map_x_ofs = ofs;
            self.scroll_delta = 0;
            self.scroll_primed = true;
        } else {
            self.scroll_delta = ofs - self.last_map_x_ofs;
            self.last_map_x_ofs = ofs;
        }
        if self.options.particles {
            self.particles_update();
        }
        // shockwave_update.
        self.shockwaves.iter_mut().for_each(|s| s.frame += 1);
        let mut i = 0;
        while i < self.shockwaves.len() {
            if self.shockwaves[i].frame >= self.shockwaves[i].max_frames {
                self.shockwaves.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// `particles.c:particles_update` (+ fireball, rings, chains).
    fn particles_update(&mut self) {
        let mut i = 0;
        while i < self.particles.len() {
            let p = &mut self.particles[i];
            if p.delay > 0 {
                p.delay -= 1;
                i += 1;
                continue;
            }
            p.x += p.vx;
            p.y += p.vy;
            p.vy += p.gravity;
            p.vx *= 0.98;
            p.vy *= 0.98;
            p.life -= 1;
            if p.kind == Kind::Ember && p.y >= CHUNK_GROUND_Y && p.vy > 0.0 {
                p.y = CHUNK_GROUND_Y;
                p.vx = 0.0;
                p.vy = 0.0;
                p.gravity = 0.0;
                let life = 15 + self.range(0, 12);
                let p = &mut self.particles[i];
                p.life = life;
                p.max_life = life;
                p.twinkle = true;
                p.size = 1;
                p.kind = Kind::EmberLanded;
                i += 1;
                continue;
            }
            if p.kind == Kind::Chunk {
                if p.life & 2 == 0 && (p.vx.abs() > 0.3 || p.vy.abs() > 0.3) {
                    p.tumble_phase = (p.tumble_phase + 1) & 3;
                }
                let c = *p;
                if c.life < c.max_life && c.life & 1 == 0 {
                    self.spawn_chunk_trail(c);
                }
                if c.y >= CHUNK_GROUND_Y && c.vy > 0.0 {
                    self.spawn_impact_dust(c.x, CHUNK_GROUND_Y);
                    self.particles.swap_remove(i);
                    continue;
                }
            }
            let c = self.particles[i];
            if c.life <= 0 {
                if c.kind == Kind::Chunk {
                    self.spawn_impact_dust(c.x, c.y);
                }
                self.particles.swap_remove(i);
                continue;
            }
            i += 1;
        }
        // fireball_update (half speed in bullet time).
        let skip = if self.options.bullet_time {
            self.slowmo_flip = !self.slowmo_flip;
            !self.slowmo_flip
        } else {
            false
        };
        if !skip {
            self.fireballs.iter_mut().for_each(|f| f.frame += 1);
            self.fireballs.retain(|f| f.frame < FIREBALL_PHASES.len() as i32);
        }
        // smoke_ring_update.
        let d = self.scroll_delta;
        for s in self.smoke_rings.iter_mut() {
            s.cx += d;
            s.frame += 1;
        }
        self.smoke_rings.retain(|s| s.frame < s.max_frames && s.cx >= -80 && s.cx <= 400);
        // boss_chain_update.
        let mut k = 0;
        while k < self.boss_chains.len() {
            self.boss_chains[k].cx += d;
            self.boss_chains[k].cooldown -= 1;
            let bc = self.boss_chains[k];
            if bc.cooldown <= 0 && bc.pending > 0 {
                let ox = (self.next() % 41) as i32 - 20;
                let oy = (self.next() % 21) as i32 - 10;
                let fin = bc.pending == 1;
                let (fx, fy) = if fin { (bc.cx, bc.cy) } else { (bc.cx + ox, bc.cy + oy) };
                self.fireball_spawn(fx, fy, fin, 1);
                self.spawn_thermal(fx, fy, if fin { 12 } else { 5 });
                let cd = 2 + (self.next() % 3) as i32;
                let bc = &mut self.boss_chains[k];
                bc.cooldown = cd;
                bc.pending -= 1;
            }
            if self.boss_chains[k].pending <= 0 {
                self.boss_chains.swap_remove(k);
                continue;
            }
            k += 1;
        }
    }

    /// RGBA layer (r, g, b, a not premultiplied, like the writes of
    /// particles.c into RGBAScreen) 320x200: frame glows, fire
    /// balls, particles, smoke rings, shockwaves, graduated waves of the
    /// Modern style, then boss flash (enemy_render.c:JE_starShowVGA,
    /// `screen_flash_frames && hitstop_enabled`). `glow`: additive glow
    /// function of video_rgba.c.
    pub fn draw_rgba(&self, px: &mut [[u8; 4]], glow: GlowFn) {
        let rgb = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8];
        for &(x, y, r, c, i) in &self.glows {
            glow(px, x, y, r, rgb(c), i);
        }
        if self.options.particles {
            for f in &self.fireballs {
                let (c, rm, inten) = FIREBALL_PHASES[f.frame as usize];
                let r = (f.max_radius as f32 * rm) as i32;
                if r >= 1 {
                    glow(px, f.cx + GX, f.cy, r, rgb(c), inten);
                }
            }
            let mut plot = |x: i32, y: i32, pixel: [u8; 4]| {
                if (0..320).contains(&x) && (0..200).contains(&y) {
                    px[(y * 320 + x) as usize] = pixel;
                }
            };
            for p in &self.particles {
                if p.delay > 0 {
                    continue;
                }
                let mut x = p.x as i32;
                let y = p.y as i32;
                if !p.manual_pos {
                    x += GX;
                }
                if !(-1..321).contains(&x) || !(-1..201).contains(&y) {
                    continue;
                }
                let t = 1.0 - p.life as f32 / p.max_life as f32;
                let color = lerp_argb(p.color_start, p.color_end, t);
                let mut alpha = p.life as f32 / p.max_life as f32;
                if p.twinkle {
                    let phase = (p.max_life - p.life) as f32 * 0.6;
                    alpha *= 0.5 + 0.5 * phase.sin().abs();
                }
                let a = (alpha * 255.0) as u8;
                let pixel = [(color >> 16) as u8, (color >> 8) as u8, color as u8, a];
                if p.kind == Kind::Chunk && p.size >= 2 {
                    const TUMBLE: [[(i32, i32); 3]; 4] = [
                        [(0, 0), (1, 0), (0, 1)],
                        [(0, 0), (1, 0), (1, 1)],
                        [(1, 0), (0, 1), (1, 1)],
                        [(0, 0), (0, 1), (1, 1)],
                    ];
                    for (dx, dy) in TUMBLE[(p.tumble_phase & 3) as usize] {
                        plot(x + dx, y + dy, pixel);
                    }
                } else {
                    plot(x, y, pixel);
                    if p.size >= 2 {
                        plot(x + 1, y, pixel);
                        plot(x, y + 1, pixel);
                        plot(x + 1, y + 1, pixel);
                    }
                }
            }
            for s in &self.smoke_rings {
                let t = s.frame as f32 / s.max_frames as f32;
                let alpha = ((1.0 - t) * 60.0) as u8;
                if alpha >= 4 {
                    ring(px, s.cx + GX, s.cy, (s.radius - 4).max(0), s.radius, [0x40, 0x40, 0x40], alpha);
                }
            }
        }
        for s in &self.shockwaves {
            let t = s.frame as f32 / s.max_frames as f32;
            let radius = (t * s.max_radius as f32) as i32;
            let thickness = (2.0 + (1.0 - t) * 3.0) as i32;
            let alpha = ((1.0 - t) * 200.0) as u8;
            if alpha < 5 || radius < 1 {
                continue;
            }
            ring(px, s.cx + GX, s.cy, (radius - thickness / 2).max(0), radius + thickness / 2, rgb(s.color), alpha);
        }
        if self.modern.live_count() > 0 {
            let palette = (self.palette_art && self.palette.len() >= 768).then_some(self.palette.as_slice());
            self.modern.draw(px, &self.reserve, palette);
        }
        if self.flash_draw > 0 {
            let a = (self.flash_draw * 80) as u8;
            let c = rgb(self.flash_color);
            for e in px.iter_mut() {
                *e = [c[0], c[1], c[2], a];
            }
        }
    }
}

/// `particles.c:lerp_argb`.
fn lerp_argb(a: u32, b: u32, t: f32) -> u32 {
    let ch = |v: u32, s: u32| ((v >> s) & 0xFF) as i32;
    let mut out = 0u32;
    for s in [24, 16, 8, 0] {
        let (x, y) = (ch(a, s), ch(b, s));
        out |= ((x + ((y - x) as f32 * t) as i32) as u32 & 0xFF) << s;
    }
    out
}

/// Ring of `shockwave_draw` / `smoke_ring_draw`: accumulated alpha, color placed.
fn ring(px: &mut [[u8; 4]], cx: i32, cy: i32, r_inner: i32, r_outer: i32, c: [u8; 3], alpha: u8) {
    let (ri2, ro2) = (r_inner * r_inner, r_outer * r_outer);
    let (x0, x1) = ((cx - r_outer).max(0), (cx + r_outer).min(319));
    let (y0, y1) = ((cy - r_outer).max(0), (cy + r_outer).min(199));
    for y in y0..=y1 {
        let dy2 = (y - cy) * (y - cy);
        for x in x0..=x1 {
            let d2 = (x - cx) * (x - cx) + dy2;
            if d2 >= ri2 && d2 <= ro2 {
                let e = &mut px[(y * 320 + x) as usize];
                let fa = (e[3] as u32 + alpha as u32).min(255) as u8;
                *e = [c[0], c[1], c[2], fa];
            }
        }
    }
}

/// `damagenumbers.c:damage_number_spawn`: (minimum duration, distance) of merging according to
/// `game_speed`: `(int)(14 / spd)` (at least 4) and `(int)(20 / spd)` (at least 8), x 3 in
/// slow motion. Beyond `DAMAGE_NUMBER_TTL` (24) no merging is possible.
pub fn damage_merge_params(game_speed: f32, bullet_time: bool) -> (i32, i32) {
    let spd = if game_speed > 0.1 { game_speed } else { 1.0 };
    let min_ttl = ((14.0f32 / spd) as i32).max(4);
    let mut dist = ((20.0f32 / spd) as i32).max(8);
    if bullet_time {
        dist *= 3;
    }
    (min_ttl, dist)
}

#[cfg(test)]
mod damage_merge_tests {
    use super::*;

    /// The C merges nothing in slow motion (min_ttl 56 > TTL 24).
    #[test]
    fn merge_follows_game_speed() {
        assert_eq!(damage_merge_params(1.0, false), (14, 20));
        assert_eq!(damage_merge_params(0.25, true), (56, 240));
        assert!(damage_merge_params(0.25, true).0 > DAMAGE_NUMBER_TTL);
        // Turbo 1.5x: window and radius reduced, with the floors 4 and 8.
        assert_eq!(damage_merge_params(1.5, false), (9, 13));
        assert_eq!(damage_merge_params(10.0, false), (4, 8));
    }

    #[test]
    fn no_merge_during_bullet_time() {
        let mut r = FxRender::new(1);
        r.options.damage_numbers = true;
        r.options.bullet_time = true;
        r.damage_number_spawn(100, 100, 5, 7);
        r.damage_number_spawn(100, 100, 5, 7);
        assert_eq!(r.numbers.iter().filter(|n| n.ttl > 0).count(), 2, "two numbers, no merging");
        let mut r = FxRender::new(1);
        r.options.damage_numbers = true;
        r.damage_number_spawn(100, 100, 5, 7);
        r.damage_number_spawn(100, 100, 5, 7);
        assert_eq!(r.numbers.iter().filter(|n| n.ttl > 0).count(), 1, "speed 1: merged");
    }

    #[test]
    fn classic_rises_linearly() {
        let mut r = FxRender::new(1);
        r.options.damage_numbers = true;
        r.options.processor_type = 1;
        r.damage_number_spawn(100, 100, 5, 7);
        let y0 = r.numbers.iter().find(|n| n.ttl > 0).unwrap().y;
        for _ in 0..10 {
            r.damage_numbers_update();
        }
        let n = r.numbers.iter().find(|n| n.ttl > 0).unwrap();
        assert_eq!(n.y, y0 - 5, "1 px every 2 frames");
        assert_eq!(n.x, 100, "no horizontal drift");
    }
}

#[cfg(test)]
mod shake_tests {
    use super::*;

    fn kill(value: i32) -> FxEvent {
        FxEvent::EnemyKilled { x: 100, y: 100, damage: 1, value, frame: 1, shot_dx: 0, shot_dy: 0 }
    }

    /// Successive offsets of a shake left to itself.
    fn run(r: &mut FxRender, frames: usize) -> Vec<(i32, i32)> {
        (0..frames)
            .map(|_| {
                r.shake_frame();
                r.shake_offset()
            })
            .collect()
    }

    /// screenshake.c: kill >= 1000 -> intensity 12 then x0.90 per frame,
    /// cut off below 0.5; hit (0,-1) of 4 that loses 0.5 per frame.
    #[test]
    fn big_kill_shakes_then_decays_to_zero() {
        let mut r = FxRender::new(7);
        r.shake_on_event(&kill(1500));
        let offs = run(&mut r, 60);
        // 1st frame: 12 x 0.9 = 10.8 -> integer amplitude 10, + hit (0, -3).
        assert!(offs[0].0.abs() <= 10 && offs[0].1 >= -13 && offs[0].1 <= 7, "{:?}", offs[0]);
        assert!(offs.iter().take(10).any(|o| o.0 != 0 || o.1 != 0));
        // 12 x 0.9^n < 0.5 around n = 31: everything is back to zero before 60 frames.
        assert_eq!(offs[59], (0, 0));
        assert_eq!(r.shake_intensity, 0.0);
    }

    #[test]
    fn thresholds_and_events_match_the_c() {
        let mut r = FxRender::new(1);
        r.shake_on_event(&kill(99));
        assert_eq!((r.shake_intensity, r.bump.2), (0.0, 0.0));
        r.shake_on_event(&kill(100));
        assert_eq!((r.shake_intensity, r.bump.2), (0.0, 1.5));
        r.shake_on_event(&kill(300));
        assert_eq!((r.shake_intensity, r.bump.2), (6.0, 2.5));
        r.shake_on_event(&kill(1000));
        assert_eq!((r.shake_intensity, r.bump.2), (12.0, 4.0));
        // A weaker hit does not replace the stronger one.
        r.shake_on_event(&kill(100));
        assert_eq!(r.bump.2, 4.0);
        let mut r = FxRender::new(1);
        r.shake_on_event(&FxEvent::PlayerHit { player: 0, damage: 5 });
        assert_eq!(r.shake_intensity, 10.0);
        r.shake_on_event(&FxEvent::PlayerDied { player: 0 });
        assert_eq!(r.shake_intensity, 20.0);
        let mut r = FxRender::new(1);
        r.shake_on_event(&FxEvent::BossDefeated { x: 0, y: 0, value: 1, frame: 1 });
        assert_eq!(r.shake_intensity, 16.0);
    }

    #[test]
    fn disabled_shake_gives_no_offset() {
        let mut r = FxRender::new(3);
        r.options.shake = false;
        r.shake_on_event(&kill(5000));
        assert!(run(&mut r, 20).iter().all(|&o| o == (0, 0)));
    }

    /// The draw comes from the rendering: it does not touch the generator of the effects
    /// (ADR 0003: simulation parity).
    #[test]
    fn shake_draws_do_not_use_effect_rng() {
        let mut r = FxRender::new(3);
        r.shake_on_event(&kill(5000));
        run(&mut r, 20);
        assert_eq!(r.rng_draws, 0);
    }

    /// processorType < 2 or screenshake off: no secret flash, no glows.
    #[test]
    fn modern_stack_follows_processor_type_and_shake() {
        let mut r = FxRender::new(1);
        r.options.style = STYLE_MODERN;
        assert!(r.glow_stack_active());
        r.options.processor_type = 1;
        assert!(!r.glow_stack_active());
        r.options.processor_type = 3;
        assert!(r.glow_stack_active());
        r.options.shake = false;
        assert!(!r.glow_stack_active());
    }
}

#[cfg(test)]
mod debris_tests {
    use super::*;

    fn count(r: &FxRender, kind: Kind) -> usize {
        r.particles.iter().filter(|p| p.kind == kind).count()
    }

    /// Large explosion (death of a 2x2 enemy piece), Retro+: 3 sparks, 1 smoke
    /// puff, then the halved debris (5 embers, 2 chunks instead of 10 and 4).
    #[test]
    fn large_explosion_debris_is_toned_down() {
        let mut r = FxRender::new(5);
        r.options.particles = true;
        r.explosion_fx(100, 80, true, false, 1);
        assert_eq!(count(&r, Kind::Ember), LARGE_EXPLOSION_EMBERS as usize);
        assert_eq!(count(&r, Kind::Chunk), LARGE_EXPLOSION_CHUNKS as usize);
        assert_eq!(r.particles.len(), 3 + 1 + 5 + 2);
        // Small explosion and Classic style unchanged: 2 sparks, nothing.
        let mut r = FxRender::new(5);
        r.options.particles = true;
        r.explosion_fx(100, 80, false, false, 1);
        assert_eq!(r.particles.len(), 2);
        r.options.style = STYLE_CLASSIC;
        r.explosion_fx(100, 80, true, false, 1);
        assert_eq!(r.particles.len(), 2);
    }
}
