//! Modern explosions (Setup > Graphics > Explosions = Modern), on top of the
//! Modern explosions of the C, which remain: shockwave starting from the
//! impact point of the fatal shot, sized on the footprint of the destroyed sprites
//! (radius, thickness and duration measured on Raiden II, `ring_plan`), white
//! flash and horizontal streak, debris, secondary explosions.
//!
//! - Small (up to a 2x2 sprite, 24x28): standard explosion and 2 sparks,
//!   no ring or flash.
//! - Medium: white flash, ring, 3 fragments.
//! - Large: white flash and horizontal streak, cascading rings, heavy
//!   fragments some of which smoking, secondary explosions.
//! - Boss: a chain of small explosions over the hull for about one second,
//!   then the final burst: flash, streak crossing the screen, thin rings
//!   extending beyond the screen, golden fragments, a big explosion at the center.
//!
//! Rings are hot: white-yellow at birth, then orange, then a dark red glow
//! that fades, with a halo of 2 to 3 px around the stroke (`ring_color`).
//! The hollow red ring that ends the explosion sprites of the game
//! (`newsh6.shp`, `ring_phase_frame`) is not drawn in Modern style: embers
//! (1 to 2 px, yellow-orange cooling to dark red) leave the explosion at that
//! moment instead. In Original art the embers take the nearest colors of the
//! level palette.
//!
//! Rendering only: separate generators (kills, boss chain, embers: each
//! leaves the draws of the others unchanged), inputs read from the events
//! and the presented world, nothing is written into the simulation. The
//! secondary explosions are Modern explosions of the C
//! (`FxRender::explosion_fx`) requested by the rendering. Nothing is drawn
//! over an enemy shot or over the pixel surrounding it (`reserve`).

use std::f32::consts::PI;
use tyrian_sim::fx::FxEvent;
use tyrian_sim::world::World;

/// x offset of the game in the 320x200 layer (`particles.h:GAME_COORD_X_OFFSET`).
const GX: f32 = -24.0;

/// Point hit in the 12x14 shot sprite drawn at (x + 1, y): middle,
/// toward the front of the shot.
const IMPACT_DX: i32 = 7;
const IMPACT_DY: i32 = 5;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Tier {
    Small,
    Medium,
    Large,
    Boss,
}

/// Size according to the footprint of the destroyed sprites (width x height of
/// all the pieces), in 12x14 cells: up to 4 (a 2x2 sprite,
/// 24x28) = small; up to 16 (48x56, four 2x2 sprites) = medium; beyond
/// = large. The boss comes from EVENT_BOSS_DEFEATED.
pub fn tier_of(w: i32, h: i32) -> Tier {
    let cells = (w.max(12) * h.max(14) + 84) / 168;
    if cells <= 4 {
        Tier::Small
    } else if cells <= 16 {
        Tier::Medium
    } else {
        Tier::Large
    }
}

/// Drawn enemy: position (ex + mapoffset, ey), ground layer, size (1 = 2x2).
#[derive(Clone, Copy, Debug)]
pub struct SeenEnemy {
    pub x: i32,
    pub y: i32,
    pub ground: bool,
    pub size: u8,
}

impl SeenEnemy {
    /// Sprite rectangle (enemy_blits: 2x2 placed at (x - 6, y - 7)).
    fn rect(&self) -> (i32, i32, i32, i32) {
        if self.size == 1 {
            (self.x - 6, self.y - 7, self.x + 18, self.y + 21)
        } else {
            (self.x, self.y, self.x + 12, self.y + 14)
        }
    }

    fn cells(&self) -> u32 {
        if self.size == 1 {
            4
        } else {
            1
        }
    }
}

/// Enemies drawn in the presented frame; slots 25..50 and 75..100 = ground.
pub fn seen_enemies(w: &World) -> Vec<SeenEnemy> {
    w.render
        .drawn_enemies
        .iter()
        .map(|e| SeenEnemy {
            x: e.ex as i32 + e.mapoffset as i32,
            y: e.ey as i32,
            ground: matches!(e.slot, 25..=49 | 75..=99),
            size: e.size,
        })
        .collect()
}

/// A destruction read from the events of a tick.
#[derive(Clone, Debug)]
pub struct Kill {
    /// Impact point of the fatal shot (game coordinates).
    pub impact: (f32, f32),
    /// Direction of the shot (unit; upward with no known shot).
    pub dir: (f32, f32),
    /// Footprint of the destroyed sprites: x0, y0, x1, y1.
    pub bbox: (i32, i32, i32, i32),
    /// Pieces whose sprite is known (0: footprint still empty).
    pub cells: u32,
    pub parts: u32,
    pub value: i32,
    pub tier: Tier,
    pub ground: bool,
}

impl Kill {
    pub fn center(&self) -> (f32, f32) {
        ((self.bbox.0 + self.bbox.2) as f32 / 2.0, (self.bbox.1 + self.bbox.3) as f32 / 2.0)
    }
    pub fn size(&self) -> (i32, i32) {
        (self.bbox.2 - self.bbox.0, self.bbox.3 - self.bbox.1)
    }
}

fn near(a: (i32, i32), b: (i32, i32), d: i32) -> bool {
    (a.0 - b.0).abs() <= d && (a.1 - b.1).abs() <= d
}

/// Drawn enemy closest to `p` within `d` px.
fn nearest(seen: &[SeenEnemy], p: (i32, i32), d: i32) -> Option<SeenEnemy> {
    seen.iter().copied().filter(|e| near((e.x, e.y), p, d)).min_by_key(|e| (e.x - p.0).abs() + (e.y - p.1).abs())
}

fn add_part(k: &mut Kill, r: (i32, i32, i32, i32), cells: u32, ground: bool) {
    if k.cells == 0 {
        k.bbox = r;
    } else {
        k.bbox = (k.bbox.0.min(r.0), k.bbox.1.min(r.1), k.bbox.2.max(r.2), k.bbox.3.max(r.3));
    }
    k.cells += cells;
    k.ground |= ground;
}

/// Piece killed with no explosion placed: its drawn sprite, otherwise a cell at the
/// death point.
fn close_part(kills: &mut [Kill], open: &mut Option<(usize, (i32, i32))>, seen: &[SeenEnemy]) {
    if let Some((i, p)) = open.take() {
        let (r, cells, g) = match nearest(seen, p, 20) {
            Some(e) => (e.rect(), e.cells(), e.ground),
            None => ((p.0, p.1, p.0 + 12, p.1 + 14), 1, false),
        };
        add_part(&mut kills[i], r, cells, g);
    }
}

/// Destructions of a tick, in the order of the simulation events:
/// EVENT_SHOT_HIT (impact point), EVENT_ENEMY_KILLED (one per piece) followed
/// by the explosion placed on that piece (`collision.c`), EVENT_BOSS_DEFEATED.
/// Pieces killed by the same shot form a single destruction.
/// `seen`: enemies drawn in the current frame then in the previous one.
pub fn read_kills(events: &[FxEvent], seen: &[SeenEnemy]) -> Vec<Kill> {
    let mut kills: Vec<Kill> = Vec::new();
    // Grouping key: position of the shot.
    let mut keys: Vec<(i32, i32)> = Vec::new();
    let mut last_hit: Option<(i32, i32)> = None;
    // Killed piece waiting for its explosion: (destruction, death position).
    let mut open: Option<(usize, (i32, i32))> = None;
    let mut boss: Option<(i32, i32)> = None;
    for ev in events {
        match *ev {
            FxEvent::ShotHit { x, y, .. } => {
                close_part(&mut kills, &mut open, seen);
                last_hit = Some((x, y));
            }
            FxEvent::EnemyKilled { x, y, value, shot_dx, shot_dy, .. } => {
                close_part(&mut kills, &mut open, seen);
                // Death chain: position of the shot; damage chain: position
                // of the piece, the shot is the last EVENT_SHOT_HIT.
                let key = match last_hit {
                    Some(h) if near(h, (x, y), 96) => h,
                    _ => (x, y),
                };
                let i = match keys.iter().position(|&k| near(k, key, 2)) {
                    Some(i) => i,
                    None => {
                        let len = ((shot_dx * shot_dx + shot_dy * shot_dy) as f32).sqrt();
                        let dir = if len > 0.5 { (shot_dx as f32 / len, shot_dy as f32 / len) } else { (0.0, -1.0) };
                        let impact = ((key.0 + IMPACT_DX) as f32, (key.1 + IMPACT_DY) as f32);
                        keys.push(key);
                        kills.push(Kill {
                            impact,
                            dir,
                            bbox: (key.0, key.1, key.0, key.1),
                            cells: 0,
                            parts: 0,
                            value,
                            tier: Tier::Small,
                            ground: false,
                        });
                        kills.len() - 1
                    }
                };
                kills[i].parts += 1;
                kills[i].value = kills[i].value.max(value);
                open = Some((i, (x, y)));
            }
            FxEvent::Explosion { x, y, large, ground } => {
                if let Some((i, _)) = open.take() {
                    // Sprite of the enemy at this spot, otherwise the size of the explosion.
                    let (r, cells, g) = match nearest(seen, (x, y), 8) {
                        Some(e) => (e.rect(), e.cells(), e.ground),
                        None if large => ((x - 6, y - 7, x + 18, y + 21), 4, ground),
                        None => ((x, y, x + 12, y + 14), 1, ground),
                    };
                    add_part(&mut kills[i], r, cells, g || ground);
                }
            }
            FxEvent::BossDefeated { x, y, .. } => boss = Some((x, y)),
            _ => {}
        }
    }
    close_part(&mut kills, &mut open, seen);
    for k in kills.iter_mut() {
        let (w, h) = k.size();
        k.tier = tier_of(w, h);
        // At most two 2x2 sprites (or isolated pieces): on screen it is a small
        // enemy, even if the footprint (two sprites side by side) exceeds 24x28.
        if k.tier == Tier::Medium && k.cells <= 8 {
            k.tier = Tier::Small;
        }
    }
    if let Some(b) = boss {
        match keys.iter().position(|&k| near(k, b, 2)) {
            Some(i) => kills[i].tier = Tier::Boss,
            None => kills.push(Kill {
                impact: ((b.0 + IMPACT_DX) as f32, (b.1 + IMPACT_DY) as f32),
                dir: (0.0, -1.0),
                bbox: (b.0 - 18, b.1 - 21, b.0 + 30, b.1 + 35),
                cells: 16,
                parts: 0,
                value: 0,
                tier: Tier::Boss,
                ground: false,
            }),
        }
    }
    kills
}
// --- Drawn elements ----------------------------------------------------------

const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const YELLOW: [u8; 3] = [0xFF, 0xF4, 0xA8];
const METAL: [u8; 3] = [0x9C, 0x98, 0x90];
const GOLD: [u8; 3] = [0xD8, 0xB0, 0x60];
const SMOKE: [u8; 3] = [0x6C, 0x68, 0x64];
const SPARK_HOT: [u8; 3] = [0xFF, 0xE8, 0x90];
const SPARK_COLD: [u8; 3] = [0xFF, 0x70, 0x20];

/// Color ramp of the hot rings and embers, from hot (1) to cold (0):
/// white-yellow, yellow, orange, red-orange, dark red.
const HEAT_RAMP: [[u8; 3]; 5] =
    [[0x82, 0x1E, 0x0C], [0xD2, 0x4B, 0x16], [0xFF, 0x91, 0x2C], [0xFF, 0xD2, 0x64], [0xFF, 0xF8, 0xD2]];

/// Color of temperature `t` (0 cold .. 1 hot) on `HEAT_RAMP`.
pub fn heat_color(t: f32) -> [u8; 3] {
    let x = t.clamp(0.0, 1.0) * (HEAT_RAMP.len() - 1) as f32;
    let i = (x.floor() as usize).min(HEAT_RAMP.len() - 2);
    mix(HEAT_RAMP[i], HEAT_RAMP[i + 1], x - i as f32)
}

/// Final radius of a medium or large ring: this ratio x the longest side of
/// the footprint. Raiden II measured: halo 80 to 90 px wide for a 30 px
/// target, i.e. a radius of 42 to 45 px of video. Two readings give
/// bounds: in absolute terms, x 264 / 480 = 0.55 (Tyrian's field is 264 px, the
/// game occupies the 480 px of the video), i.e. 23 to 25 px; in proportion to
/// the footprint (radius = 1.4 x the side), 67 px on a medium of 48 px. Since the
/// Tyrian sprites are larger on screen than those of Raiden II, the
/// pure proportion would be too wide: 0.7 x the side (34 px on 48 px) falls
/// between the two.
const RING_PER_SIDE: f32 = 0.7;

/// Medium ring: half of the longest side of the footprint (feedback from the maintainer
/// on P3-final: the medium rings of 34 to 39 px were too large).
const MEDIUM_RING_PER_SIDE: f32 = 0.5;

/// A ring: delay, expansion frames (fast, slowing at the end), life frames
/// (opacity fades out until then), start and end radii,
/// thickness at birth (it thins to 1 px during the expansion), temperature
/// at birth (`heat_color`, it cools down to 0 over the life), halo width
/// around the stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingSpec {
    pub delay: i32,
    pub grow: i32,
    pub frames: i32,
    pub r0: f32,
    pub r1: f32,
    pub thick: f32,
    pub heat: f32,
    pub halo: f32,
    pub alpha: f32,
}

#[derive(Clone, Copy)]
struct Ring {
    x: f32,
    y: f32,
    age: i32,
    spec: RingSpec,
    ground: bool,
}

/// Rings of a destruction according to its size and footprint (width,
/// height): none for a small enemy.
pub fn ring_plan(tier: Tier, w: i32, h: i32) -> Vec<RingSpec> {
    let side = w.max(h) as f32;
    let spec = |delay, grow, frames, r1: f32, thick: f32, heat, halo, alpha| RingSpec {
        delay,
        grow,
        frames,
        r0: (r1 * 0.15).max(2.0),
        r1,
        thick,
        heat,
        halo,
        alpha,
    };
    match tier {
        Tier::Small => Vec::new(),
        Tier::Medium => {
            // Medium: half of the longest side (48x28 -> 24 px, 48x56 -> 28 px).
            let r = (MEDIUM_RING_PER_SIDE * side).clamp(20.0, 28.0);
            vec![spec(1, 4, 9, r, 3.0, 1.0, 2.0, 0.9)]
        }
        Tier::Large => {
            let r = (RING_PER_SIDE * side).clamp(40.0, 110.0);
            vec![
                spec(1, 5, 12, r, (r / 10.0).round().clamp(4.0, 7.0), 1.0, 3.0, 0.95),
                spec(4, 5, 12, r * 0.7, 2.0, 0.85, 2.0, 0.6),
                spec(8, 6, 14, (r * 1.3).min(130.0), 2.0, 0.75, 3.0, 0.45),
            ]
        }
        // Boss: thin rings extending beyond the screen (264 px wide).
        Tier::Boss => vec![
            spec(1, 7, 14, 230.0, 2.0, 1.0, 3.0, 1.0),
            spec(5, 6, 12, 150.0, 2.0, 0.9, 3.0, 0.7),
            spec(9, 5, 12, 95.0, 2.0, 0.8, 2.0, 0.6),
        ],
    }
}

/// White flash of 2 frames at the impact point: radius and length of the
/// horizontal streak (0 = no streak). Medium, large and boss only; streak on
/// the large and the boss. Raiden II: disc ~70 video px on a 30 px tank,
/// i.e. 38 Tyrian px; the radius follows the footprint, capped so as not to
/// dazzle.
pub fn flash_plan(tier: Tier, w: i32, h: i32) -> Option<(f32, f32)> {
    let side = w.max(h) as f32;
    match tier {
        Tier::Small => None,
        Tier::Medium => Some(((0.17 * side).clamp(7.0, 9.0), 0.0)),
        Tier::Large => Some(((0.25 * side).clamp(12.0, 20.0), (0.6 * side).clamp(40.0, 90.0))),
        Tier::Boss => Some((36.0, 280.0)),
    }
}

/// White flash at the impact point, with horizontal streak (`flare`, the
/// Raiden II signature) on the large and the boss.
#[derive(Clone, Copy)]
struct Flash {
    x: f32,
    y: f32,
    age: i32,
    r: f32,
    flare: f32,
    ground: bool,
}

const FLASH_FRAMES: i32 = 2;

#[derive(Clone, Copy)]
struct Debris {
    x: f32,
    y: f32,
    /// Height above the ground of the enemy (drawn toward the top of the screen).
    h: f32,
    vx: f32,
    vy: f32,
    vh: f32,
    age: i32,
    life: i32,
    size: u8,
    spin: f32,
    spin_speed: f32,
    colors: [[u8; 3]; 3],
    smoking: bool,
    ground: bool,
    bounces: u8,
    resting: bool,
    /// Spark: hot point with a trail, not a metal fragment.
    spark: bool,
    px: f32,
    py: f32,
}

#[derive(Clone, Copy)]
struct Puff {
    x: f32,
    y: f32,
    age: i32,
    life: i32,
    r: f32,
    ground: bool,
}

/// Secondary explosion pending; `ring` > 0: hot ring of this radius;
/// `flash` > 0: white flash of this radius; `boss`: link of the boss chain
/// (its draws come from the boss generator).
#[derive(Clone, Copy)]
struct Pop {
    delay: i32,
    x: f32,
    y: f32,
    large: bool,
    ground: bool,
    ring: f32,
    flash: f32,
    boss: bool,
}

/// Fragments: direction, spread, counts, sizes, speeds, vertical momentum.
#[derive(Clone, Copy)]
struct Burst {
    aim: (f32, f32),
    spread: f32,
    pieces: usize,
    sparks: usize,
    sizes: (u8, u8),
    speed: (f32, f32),
    lift: (f32, f32),
    smoking: usize,
}

/// Ember left by an explosion sprite at its ring phase: hot point of 1 or 2 px
/// that cools from yellow-orange to dark red, light gravity, 1 px trail.
#[derive(Clone, Copy)]
struct Ember {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    px: f32,
    py: f32,
    age: i32,
    life: i32,
    size: u8,
    ground: bool,
}

/// Final burst of a boss waiting for the end of its chain.
#[derive(Clone)]
struct BossFinal {
    delay: i32,
    kill: Kill,
}

const MAX_RINGS: usize = 64;
const MAX_DEBRIS: usize = 500;
const MAX_PUFFS: usize = 400;
const MAX_POPS: usize = 64;
const MAX_EMBERS: usize = 400;
const GRAVITY: f32 = 0.16;
const EMBER_GRAVITY: f32 = 0.05;
const EMBERS_PER_SPRITE: usize = 5;

/// Boss chain: small explosions over the hull during `BOSS_CHAIN_FRAMES`
/// (35 frames = 1 s at 35 frames/s), then the final burst.
pub const BOSS_CHAIN_FRAMES: i32 = 35;
pub const BOSS_CHAIN_LINKS: (usize, usize) = (5, 8);

/// splitmix64: draws for rendering only.
#[derive(Clone, Copy)]
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

/// Explosion sprite frames of `newsh6.shp` (index drawn, 1-based) that show
/// the hollow red ring ending a fire explosion: the last frames of the
/// sequences of `varz.c:explosion_data` types 1 to 5 and 9 (center of the
/// 12x14 cell transparent on at least half of its middle third). First frame
/// of each ring phase, then last frame.
pub const RING_PHASES: [(u16, u16); 6] = [(129, 133), (162, 165), (183, 184), (203, 203), (221, 222), (51, 52)];

/// Frame `sprite` of `newsh6.shp` is a hollow red ring (replaced by embers
/// in Modern style).
pub fn ring_phase_frame(sprite: u16) -> bool {
    RING_PHASES.iter().any(|&(a, b)| (a..=b).contains(&sprite))
}

/// First frame of a ring phase: the embers leave the explosion there.
fn ring_phase_start(sprite: u16) -> bool {
    RING_PHASES.iter().any(|&(a, _)| a == sprite)
}

/// Ongoing effects of the Modern explosions, for one level.
pub struct ModernExplosions {
    rng: SplitMix,
    boss_rng: SplitMix,
    ember_rng: SplitMix,
    rings: Vec<Ring>,
    flashes: Vec<Flash>,
    debris: Vec<Debris>,
    puffs: Vec<Puff>,
    pops: Vec<Pop>,
    embers: Vec<Ember>,
    boss_finals: Vec<BossFinal>,
    /// Secondary explosions to place this frame: (x, y, large, ground), in
    /// C explosion coordinates (sprite corner).
    pub ready_pops: Vec<(i32, i32, bool, bool)>,
    last_seen: Vec<SeenEnemy>,
    /// Destructions seen by size (small, medium, large, boss).
    pub tier_counts: [u64; 4],
    /// Explosion sprites turned into embers since the start of the level.
    pub ember_sprites: u64,
}

impl ModernExplosions {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix(seed ^ 0x4558_504c_4f53_4932),
            boss_rng: SplitMix(seed ^ 0x424f_5353_4348_4149),
            ember_rng: SplitMix(seed ^ 0x454d_4245_5253_2121),
            rings: Vec::new(),
            flashes: Vec::new(),
            debris: Vec::new(),
            puffs: Vec::new(),
            pops: Vec::new(),
            embers: Vec::new(),
            boss_finals: Vec::new(),
            ready_pops: Vec::new(),
            last_seen: Vec::new(),
            tier_counts: [0; 4],
            ember_sprites: 0,
        }
    }

    pub fn live_count(&self) -> usize {
        self.rings.len()
            + self.flashes.len()
            + self.debris.len()
            + self.puffs.len()
            + self.pops.len()
            + self.embers.len()
            + self.boss_finals.len()
    }

    fn next(&mut self) -> u64 {
        self.rng.next()
    }

    fn between(&mut self, a: f32, b: f32) -> f32 {
        self.rng.between(a, b)
    }

    /// Events of the presented frame (`spawn` false: nothing new, the
    /// ongoing effects finish), then update.
    pub fn frame(&mut self, events: &[FxEvent], w: &World, spawn: bool) {
        let now = seen_enemies(w);
        self.ready_pops.clear();
        if spawn {
            let mut seen = now.clone();
            seen.extend_from_slice(&self.last_seen);
            for k in read_kills(events, &seen) {
                self.tier_counts[k.tier as usize] += 1;
                self.spawn(&k);
            }
            for &(x, y, sprite) in &w.render.drawn_explosions {
                if ring_phase_start(sprite) {
                    // Center of the 12x14 cell of the sprite.
                    self.spawn_embers(x as f32 + 6.0, y as f32 + 7.0);
                }
            }
        }
        self.last_seen = now;
        self.update(w);
    }

    fn ring(&mut self, x: f32, y: f32, spec: RingSpec, ground: bool) {
        if self.rings.len() < MAX_RINGS {
            self.rings.push(Ring { x, y, age: 0, spec, ground });
        }
    }

    /// Embers of an explosion sprite reaching its ring phase.
    fn spawn_embers(&mut self, x: f32, y: f32) {
        self.ember_sprites += 1;
        // The explosion sprites follow the ground scrolling (explodeMove).
        let ground = true;
        for _ in 0..EMBERS_PER_SPRITE {
            if self.embers.len() >= MAX_EMBERS {
                return;
            }
            let r = &mut self.ember_rng;
            let a = r.between(0.0, 2.0 * PI);
            let s = r.between(0.5, 1.4);
            let (ox, oy) = (r.between(-3.0, 3.0), r.between(-3.0, 3.0));
            let life = 10 + (r.next() % 9) as i32;
            let size = if r.next() % 10 < 3 { 2 } else { 1 };
            self.embers.push(Ember {
                x: x + ox,
                y: y + oy,
                // Slightly upward: the embers rise before falling.
                vx: a.cos() * s,
                vy: a.sin() * s - 0.3,
                px: x + ox,
                py: y + oy,
                age: 0,
                life,
                size,
                ground,
            });
        }
    }

    fn burst(&mut self, x: f32, y: f32, b: Burst, colors: [[u8; 3]; 3], ground: bool, boss: bool) {
        let mut rng = if boss { self.boss_rng } else { self.rng };
        let base = b.aim.1.atan2(b.aim.0);
        for i in 0..b.pieces + b.sparks {
            if self.debris.len() >= MAX_DEBRIS {
                break;
            }
            let spark = i >= b.pieces;
            let a = base + rng.between(-b.spread, b.spread);
            let s = rng.between(b.speed.0, b.speed.1) * if spark { 1.4 } else { 1.0 };
            let size = if spark { 1 } else { b.sizes.0 + (rng.next() % (b.sizes.1 - b.sizes.0 + 1) as u64) as u8 };
            let vh = rng.between(b.lift.0, b.lift.1) * if spark { 0.5 } else { 1.0 };
            let life = if spark { 8 + (rng.next() % 8) as i32 } else { 50 + (rng.next() % 30) as i32 };
            let jx = rng.between(-2.0, 2.0);
            let jy = rng.between(-2.0, 2.0);
            let spin = rng.between(0.0, 4.0);
            let spin_speed = rng.between(0.15, 0.45) * if rng.next().is_multiple_of(2) { 1.0 } else { -1.0 };
            self.debris.push(Debris {
                x: x + jx,
                y: y + jy,
                h: 0.0,
                vx: a.cos() * s,
                vy: a.sin() * s,
                vh,
                age: 0,
                life,
                size,
                spin,
                spin_speed,
                colors,
                smoking: !spark && i < b.smoking,
                ground,
                bounces: if ground { 1 } else { 0 },
                resting: false,
                spark,
                px: x + jx,
                py: y + jy,
            });
        }
        if boss {
            self.boss_rng = rng;
        } else {
            self.rng = rng;
        }
    }

    /// `n` secondary explosions over the footprint, between frames `from` and `to`.
    fn pops_over(&mut self, k: &Kill, n: usize, from: i32, to: i32) {
        let (w, h) = k.size();
        for i in 0..n {
            if self.pops.len() >= MAX_POPS {
                return;
            }
            let x = k.bbox.0 as f32 + self.between(0.15, 0.85) * w.max(12) as f32;
            let y = k.bbox.1 as f32 + self.between(0.15, 0.85) * h.max(14) as f32;
            let t = from + ((to - from) * i as i32) / n.max(1) as i32 + (self.next() % 4) as i32;
            self.pops.push(Pop { delay: t, x, y, large: false, ground: k.ground, ring: 0.0, flash: 0.0, boss: false });
        }
    }

    /// Boss chain: 5 to 8 small explosions spread over the hull during the
    /// first second (one per eighth of the footprint, in a random order),
    /// each with a small flash and a small hot ring; then the final burst.
    fn boss_chain(&mut self, k: &Kill) {
        let (w, h) = k.size();
        let (w, h) = (w.max(48) as f32, h.max(56) as f32);
        let r = &mut self.boss_rng;
        let n = BOSS_CHAIN_LINKS.0 + (r.next() % (BOSS_CHAIN_LINKS.1 - BOSS_CHAIN_LINKS.0 + 1) as u64) as usize;
        // Columns visited in a shuffled order so that the chain walks the hull.
        let mut cols: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (r.next() % (i as u64 + 1)) as usize;
            cols.swap(i, j);
        }
        let mut links = Vec::with_capacity(n);
        for (i, &c) in cols.iter().enumerate() {
            let fx = (c as f32 + r.between(0.2, 0.8)) / n as f32;
            let x = k.bbox.0 as f32 + w * (0.1 + 0.8 * fx);
            let y = k.bbox.1 as f32 + h * r.between(0.2, 0.8);
            // Links spread over the second, the last one slightly before the burst.
            let delay = 1 + (i as i32 * (BOSS_CHAIN_FRAMES - 6)) / (n as i32 - 1).max(1) + (r.next() % 3) as i32;
            let ring = r.between(14.0, 20.0);
            let flash = r.between(5.0, 7.0);
            links.push(Pop { delay, x, y, large: false, ground: false, ring, flash, boss: true });
        }
        for p in links {
            if self.pops.len() < MAX_POPS {
                self.pops.push(p);
            }
        }
        // Fires BOSS_CHAIN_FRAMES frames after the death (counted down from
        // the update of the death frame).
        self.boss_finals.push(BossFinal { delay: BOSS_CHAIN_FRAMES + 1, kill: k.clone() });
    }

    fn spawn(&mut self, k: &Kill) {
        if k.tier == Tier::Boss {
            self.boss_chain(k);
            return;
        }
        let (ix, iy) = k.impact;
        let (cx, cy) = k.center();
        let (fw, fh) = k.size();
        let g = k.ground;
        // Debris: from the impact, toward the center of the enemy then beyond.
        let away = {
            let (dx, dy) = (cx - ix, cy - iy);
            let l = (dx * dx + dy * dy).sqrt();
            if l > 3.0 {
                (dx / l, dy / l)
            } else {
                k.dir
            }
        };
        let colors = [lighten(METAL, 0.45), METAL, darken(METAL, 0.5)];
        let burst = |spread, pieces, sparks, sizes, speed, lift, smoking| Burst {
            aim: away,
            spread,
            pieces,
            sparks,
            sizes,
            speed,
            lift,
            smoking,
        };
        if let Some((r, flare)) = flash_plan(k.tier, fw, fh) {
            self.flashes.push(Flash { x: ix, y: iy, age: -1, r, flare, ground: g });
        }
        for spec in ring_plan(k.tier, fw, fh) {
            self.ring(ix, iy, spec, g);
        }
        match k.tier {
            Tier::Small => self.burst(ix, iy, burst(3.2, 0, 2, (1, 1), (0.8, 1.6), (0.6, 1.2), 0), colors, g, false),
            Tier::Medium => self.burst(ix, iy, burst(2.6, 3, 0, (2, 2), (0.8, 1.6), (0.8, 1.6), 0), colors, g, false),
            Tier::Large => {
                self.burst(ix, iy, burst(2.4, 18, 4, (3, 5), (0.8, 2.2), (1.4, 2.8), 9), colors, g, false);
                self.pops_over(k, 4, 6, 30);
            }
            Tier::Boss => {}
        }
    }

    /// Final burst of a boss, at the end of its chain: flash and streak,
    /// rings, golden fragments, a big explosion at the center.
    fn boss_final(&mut self, k: &Kill) {
        let (ix, iy) = k.impact;
        let (cx, cy) = k.center();
        let (fw, fh) = k.size();
        let gold = mix(GOLD, METAL, 0.3);
        let gold_colors = [lighten(gold, 0.5), gold, darken(gold, 0.5)];
        if let Some((r, flare)) = flash_plan(Tier::Boss, fw, fh) {
            self.flashes.push(Flash { x: ix, y: iy, age: -1, r, flare, ground: false });
        }
        for spec in ring_plan(Tier::Boss, fw, fh) {
            self.ring(ix, iy, spec, false);
        }
        let b = Burst {
            aim: k.dir,
            spread: PI,
            pieces: 36,
            sparks: 10,
            sizes: (4, 5),
            speed: (1.0, 2.8),
            lift: (1.6, 3.2),
            smoking: 16,
        };
        self.burst(ix, iy, b, gold_colors, false, true);
        // Big explosion of the C at the center (fire ball and embers).
        self.ready_pops.push((cx as i32 - 6, cy as i32, true, false));
    }

    fn update(&mut self, w: &World) {
        // Scrolling of ground explosions (explosion_update.c: y += explodeMove).
        let fall = w.background.explode_move as f32;
        for r in self.rings.iter_mut() {
            if r.spec.delay > 0 {
                r.spec.delay -= 1;
            } else {
                r.age += 1;
            }
            if r.ground {
                r.y += fall;
            }
        }
        self.rings.retain(|r| r.age < r.spec.frames);
        for f in self.flashes.iter_mut() {
            f.age += 1;
            if f.ground {
                f.y += fall;
            }
        }
        self.flashes.retain(|f| f.age < FLASH_FRAMES);
        let mut new_puffs = Vec::new();
        for d in self.debris.iter_mut() {
            d.age += 1;
            d.px = d.x;
            d.py = d.y - d.h;
            if d.ground {
                d.y += fall;
                d.py += fall;
            }
            if d.resting {
                continue;
            }
            d.x += d.vx;
            d.y += d.vy;
            d.h += d.vh;
            d.vh -= GRAVITY;
            let drag = if d.spark { 0.86 } else { 0.95 };
            d.vx *= drag;
            d.vy *= drag;
            d.spin += d.spin_speed;
            if d.ground && d.h <= 0.0 {
                d.h = 0.0;
                if d.bounces > 0 && d.vh < -0.7 {
                    d.bounces -= 1;
                    d.vh = -d.vh * 0.35;
                    d.vx *= 0.5;
                    d.vy *= 0.5;
                } else {
                    // Landed on the ground: stays for a while (Raiden II: the large
                    // debris stay on the ground), then fades out.
                    d.resting = true;
                    d.vx = 0.0;
                    d.vy = 0.0;
                    d.life = d.life.max(d.age + 30);
                }
            }
            if d.smoking && d.age % 3 == 0 && d.age < d.life - 10 {
                new_puffs.push(Puff { x: d.x, y: d.y - d.h, age: 0, life: 16, r: 1.5, ground: d.ground });
            }
        }
        // Fragments of an enemy in flight: they fall toward the ground, far below it.
        self.debris.retain(|d| d.age < d.life && d.h > -40.0);
        for e in self.embers.iter_mut() {
            e.age += 1;
            e.px = e.x;
            e.py = e.y;
            e.x += e.vx;
            e.y += e.vy;
            e.vy += EMBER_GRAVITY;
            e.vx *= 0.92;
            e.vy *= 0.92;
            if e.ground {
                e.y += fall;
                e.py += fall;
            }
        }
        self.embers.retain(|e| e.age < e.life);
        for p in self.puffs.iter_mut() {
            p.age += 1;
            p.r += 0.12;
            if p.ground {
                p.y += fall;
            } else {
                p.y -= 0.1;
            }
        }
        self.puffs.retain(|p| p.age < p.life);
        for p in new_puffs {
            if self.puffs.len() < MAX_PUFFS {
                self.puffs.push(p);
            }
        }
        let mut fired = Vec::new();
        for p in self.pops.iter_mut() {
            p.delay -= 1;
            if p.ground {
                p.y += fall;
            }
            if p.delay <= 0 {
                fired.push(*p);
            }
        }
        self.pops.retain(|p| p.delay > 0);
        let colors = [lighten(METAL, 0.45), METAL, darken(METAL, 0.5)];
        for p in fired {
            // Corner of the C explosion sprite: center at (x + 6, y + 7), or (x + 6, y) when large.
            let (ex, ey) = (p.x as i32 - 6, if p.large { p.y as i32 } else { p.y as i32 - 7 });
            self.ready_pops.push((ex, ey, p.large, p.ground));
            if p.ring > 0.0 {
                let spec = RingSpec {
                    delay: 0,
                    grow: 4,
                    frames: 9,
                    r0: 2.0,
                    r1: p.ring,
                    thick: 2.0,
                    heat: 0.95,
                    halo: 2.0,
                    alpha: 0.75,
                };
                self.ring(p.x, p.y, spec, p.ground);
            }
            if p.flash > 0.0 {
                self.flashes.push(Flash { x: p.x, y: p.y, age: -1, r: p.flash, flare: 0.0, ground: p.ground });
            }
            let b = Burst {
                aim: (0.0, -1.0),
                spread: PI,
                pieces: 0,
                sparks: 3,
                sizes: (1, 1),
                speed: (0.8, 1.6),
                lift: (0.4, 1.0),
                smoking: 0,
            };
            self.burst(p.x, p.y, b, colors, p.ground, p.boss);
        }
        for f in self.boss_finals.iter_mut() {
            f.delay -= 1;
        }
        let due: Vec<Kill> = self.boss_finals.iter().filter(|f| f.delay <= 0).map(|f| f.kill.clone()).collect();
        self.boss_finals.retain(|f| f.delay > 0);
        for k in due {
            self.boss_final(&k);
        }
    }

    /// Draws over `px` (320x200 RGBA layer). `reserve`: layer pixels to
    /// leave untouched (enemy shots and their edge), or empty. `palette`:
    /// level palette (768 bytes) in Original art, the embers take its
    /// nearest colors; None in HD art.
    pub fn draw(&self, px: &mut [[u8; 4]], reserve: &[bool], palette: Option<&[u8]>) {
        let mut put = |x: i32, y: i32, c: [u8; 3], a: f32| {
            if !(0..320).contains(&x) || !(0..200).contains(&y) || a < 0.02 {
                return;
            }
            let i = (y * 320 + x) as usize;
            if reserve.get(i).copied().unwrap_or(false) {
                return;
            }
            over(&mut px[i], c, (a.min(1.0) * 255.0) as u32);
        };
        for p in &self.puffs {
            let t = p.age as f32 / p.life as f32;
            let a = 0.45 * (1.0 - t);
            let (cx, cy) = ((p.x + GX) as i32, p.y as i32);
            let r = p.r;
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d2 = (dx * dx + dy * dy) as f32;
                    // Edge dithered in a checkerboard: pixel-art smoke.
                    if d2 <= r * r && (d2 < (r - 1.0).max(0.0).powi(2) || (dx + dy) & 1 == 0) {
                        put(cx + dx, cy + dy, SMOKE, a);
                    }
                }
            }
        }
        for d in &self.debris {
            let (x, y) = (d.x + GX, d.y - d.h);
            let mut fade = ((d.life - d.age) as f32 / 12.0).min(1.0);
            if !d.ground && d.h < 0.0 {
                fade *= (1.0 + d.h / 40.0).max(0.0);
            }
            if d.spark {
                let c = mix(SPARK_HOT, SPARK_COLD, d.age as f32 / d.life as f32);
                put(x as i32, y as i32, c, fade);
                put((d.px + GX) as i32, d.py as i32, c, fade * 0.5);
                continue;
            }
            // Fragment falling far below the enemy: it shrinks.
            let mut size = d.size;
            if !d.ground && d.h < -12.0 {
                size = size.saturating_sub(((-d.h - 12.0) / 10.0) as u8).max(1);
            }
            let shape = SHAPES[(size - 1) as usize];
            let n = shape.len() as i32;
            let rot = d.spin.rem_euclid(4.0) as u8;
            for (sy, row) in shape.iter().enumerate() {
                for (sx, &v) in row.iter().enumerate() {
                    if v == 0 {
                        continue;
                    }
                    let (rx, ry) = rotate(sx as i32, sy as i32, n, rot);
                    let c = d.colors[(v - 1) as usize];
                    let c = if d.resting { darken(c, 0.25) } else { c };
                    put(x as i32 - n / 2 + rx, y as i32 - n / 2 + ry, c, fade);
                }
            }
        }
        for r in self.rings.iter().filter(|r| r.spec.delay == 0) {
            draw_ring(&mut put, r);
        }
        for e in &self.embers {
            let t = e.age as f32 / e.life as f32;
            // Hot (yellow-orange) at birth, dark red at the end of its life.
            let c = heat_color(0.8 * (1.0 - t));
            let c = palette.map_or(c, |p| nearest_palette_color(p, c));
            let a = (1.0 - t * t).max(0.0);
            let (x, y) = ((e.x + GX) as i32, e.y as i32);
            put((e.px + GX) as i32, e.py as i32, c, 0.45 * a);
            put(x, y, c, a);
            if e.size >= 2 {
                put(x + 1, y, c, a);
                put(x, y + 1, c, a);
                put(x + 1, y + 1, c, a);
            }
        }
        for f in &self.flashes {
            // Short and not dazzling: 65% then 30% opacity.
            let a = [0.65, 0.3][f.age.clamp(0, 1) as usize];
            let (cx, cy) = (f.x + GX, f.y);
            let r = f.r * (1.0 + 0.25 * f.age as f32);
            let ri = r.ceil() as i32 + 1;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    let edge = (r - d + 0.5).clamp(0.0, 1.0);
                    if edge > 0.0 {
                        let c = if d > r - 1.5 { YELLOW } else { WHITE };
                        put(cx as i32 + dx, cy as i32 + dy, c, a * edge);
                    }
                }
            }
            if f.flare > 0.0 {
                // Raiden II horizontal streak, shorter at each frame.
                let half = f.flare / 2.0 * (1.0 - 0.3 * f.age.max(0) as f32);
                // The streak keeps its intensity on the first frame, then drops with the flash.
                let k = a / 0.65;
                for dx in -(half as i32)..=(half as i32) {
                    let fall_off = 1.0 - (dx.abs() as f32 / half.max(1.0));
                    // 3 px streak at the center, tapering at the ends.
                    put(cx as i32 + dx, cy as i32, WHITE, ((0.5 + 0.5 * fall_off) * k).min(1.0));
                    if dx.abs() < (half * 0.7) as i32 {
                        put(cx as i32 + dx, cy as i32 - 1, WHITE, 0.6 * fall_off * k);
                        put(cx as i32 + dx, cy as i32 + 1, WHITE, 0.6 * fall_off * k);
                    }
                }
            }
        }
    }
}

/// Pixel-art metal fragments, 1 to 5 px: 1 light, 2 medium, 3 dark.
const SHAPES: [&[&[u8]]; 5] = [
    &[&[2]],
    &[&[1, 2], &[0, 3]],
    &[&[1, 2, 0], &[2, 2, 3], &[0, 3, 0]],
    &[&[0, 1, 2, 0], &[1, 2, 2, 3], &[2, 2, 3, 0], &[0, 3, 0, 0]],
    &[&[0, 1, 1, 0, 0], &[1, 2, 2, 2, 0], &[0, 2, 2, 3, 3], &[0, 2, 3, 3, 0], &[0, 0, 3, 0, 0]],
];

/// Quarter turn `rot` of a pixel (x, y) of a square of side `n`.
fn rotate(x: i32, y: i32, n: i32, rot: u8) -> (i32, i32) {
    match rot & 3 {
        0 => (x, y),
        1 => (n - 1 - y, x),
        2 => (n - 1 - x, n - 1 - y),
        _ => (y, n - 1 - x),
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let f = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round().clamp(0.0, 255.0) as u8;
    [f(0), f(1), f(2)]
}

fn lighten(c: [u8; 3], t: f32) -> [u8; 3] {
    mix(c, WHITE, t)
}

fn darken(c: [u8; 3], t: f32) -> [u8; 3] {
    mix(c, [0, 0, 0], t)
}

/// Color `c` of opacity `a8` (0..255) laid over `e` (accumulated alpha).
fn over(e: &mut [u8; 4], c: [u8; 3], a8: u32) {
    let ea = e[3] as u32;
    let out_a = a8 + ea * (255 - a8) / 255;
    if out_a == 0 {
        return;
    }
    for k in 0..3 {
        e[k] = ((c[k] as u32 * a8 + e[k] as u32 * ea * (255 - a8) / 255) / out_a).min(255) as u8;
    }
    e[3] = out_a.min(255) as u8;
}
/// Radius, thickness and opacity of a ring at frame `age`: fast expansion
/// (cubic ease-out) over `grow` frames, thickness going from `thick` to
/// 1 px during this expansion, opacity fading over the whole duration.
pub fn ring_state(spec: &RingSpec, age: i32) -> (f32, f32, f32) {
    let grow = spec.grow.max(1) as f32;
    let reach = ((age + 1) as f32 / grow).min(1.0);
    let ease = 1.0 - (1.0 - reach).powi(3);
    let radius = spec.r0 + (spec.r1 - spec.r0) * ease;
    let thick = spec.thick + (1.0 - spec.thick) * (age as f32 / grow).min(1.0);
    let alpha = spec.alpha * (1.0 - age as f32 / spec.frames as f32);
    (radius, thick.max(1.0), alpha)
}

/// Stroke color of a ring at frame `age`: its birth temperature cooling
/// down linearly to 0 over its life (white-yellow, orange, dark red).
pub fn ring_color(spec: &RingSpec, age: i32) -> [u8; 3] {
    heat_color(spec.heat * (1.0 - age as f32 / spec.frames as f32))
}

/// Nearest color of `palette` (768 bytes RGB) to `c`.
pub fn nearest_palette_color(palette: &[u8], c: [u8; 3]) -> [u8; 3] {
    let d = |p: &[u8; 3]| (0..3).map(|k| (p[k] as i32 - c[k] as i32).pow(2)).sum::<i32>();
    palette.as_chunks::<3>().0.iter().take(256).min_by_key(|p| d(p)).copied().unwrap_or(c)
}

/// Hot ring: sharp stroke (thickness from `ring_state`, color from
/// `ring_color`) inside a halo of `spec.halo` px on each side, cooler and
/// fading toward its edge.
fn draw_ring(put: &mut impl FnMut(i32, i32, [u8; 3], f32), r: &Ring) {
    let (radius, thick, alpha) = ring_state(&r.spec, r.age);
    let half = thick * 0.5;
    if alpha < 0.02 {
        return;
    }
    let core = ring_color(&r.spec, r.age);
    let glow = heat_color(r.spec.heat * (1.0 - r.age as f32 / r.spec.frames as f32) * 0.75);
    let halo = r.spec.halo;
    let (cx, cy) = (r.x + GX, r.y);
    let ro = radius + half + halo + 1.0;
    let inner = (radius - half - halo - 1.0).max(0.0);
    let (y0, y1) = ((cy - ro).floor().max(0.0) as i32, (cy + ro).ceil().min(199.0) as i32);
    for y in y0..=y1 {
        let dy = y as f32 - cy;
        let dy2 = dy * dy;
        if dy2 > ro * ro {
            continue;
        }
        let span_o = (ro * ro - dy2).sqrt();
        let span_i = if dy2 < inner * inner { (inner * inner - dy2).sqrt() } else { 0.0 };
        let spans = if span_i > 0.0 {
            [(cx - span_o, cx - span_i), (cx + span_i, cx + span_o)]
        } else {
            [(cx - span_o, cx + span_o), (0.0, -1.0)]
        };
        for (a, b) in spans {
            if b < a {
                continue;
            }
            for x in a.floor().max(0.0) as i32..=b.ceil().min(319.0) as i32 {
                let dx = x as f32 - cx;
                let d = (dx * dx + dy2).sqrt();
                let off = (d - radius).abs();
                if off <= half + 0.25 {
                    put(x, y, core, alpha);
                } else if off <= half + halo + 0.25 {
                    let fall_off = 1.0 - (off - half - 0.25) / (halo + 0.5);
                    put(x, y, glow, alpha * 0.4 * fall_off);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_follow_sprite_footprint() {
        assert_eq!(tier_of(12, 14), Tier::Small);
        assert_eq!(tier_of(24, 28), Tier::Small);
        assert_eq!(tier_of(48, 28), Tier::Medium);
        assert_eq!(tier_of(48, 56), Tier::Medium);
        assert_eq!(tier_of(72, 56), Tier::Large);
        assert_eq!(tier_of(223, 34), Tier::Large);
    }

    #[test]
    fn small_enemies_get_no_ring_and_no_flash() {
        assert!(ring_plan(Tier::Small, 24, 28).is_empty());
        assert!(flash_plan(Tier::Small, 24, 28).is_none());
    }

    #[test]
    fn flash_on_medium_and_up_streak_on_large_and_boss_only() {
        assert_eq!(flash_plan(Tier::Medium, 48, 28).unwrap().1, 0.0);
        assert!(flash_plan(Tier::Large, 96, 84).unwrap().1 > 0.0);
        let boss = flash_plan(Tier::Boss, 332, 84).unwrap();
        assert!(boss.1 >= 264.0, "the boss streak crosses the screen");
    }

    /// Radius proportional to the footprint and bounded; thickness at birth
    /// then 1 px; maximum size reached in `grow` frames.
    #[test]
    fn ring_radius_follows_footprint_and_thins_to_one_pixel() {
        let radius = |t, w, h| ring_plan(t, w, h)[0].r1;
        assert!(radius(Tier::Medium, 36, 28) < radius(Tier::Medium, 48, 56));
        assert!(radius(Tier::Large, 72, 56) < radius(Tier::Large, 96, 140));
        assert_eq!(radius(Tier::Large, 400, 50), 110.0);
        // Medium: half of the longest side, 24 px on 48x28 and 28 px on 48x56.
        assert_eq!(radius(Tier::Medium, 48, 28), 24.0);
        assert_eq!(radius(Tier::Medium, 48, 56), 28.0);
        assert_eq!(ring_plan(Tier::Medium, 48, 28)[0].thick, 3.0);
        for t in [Tier::Medium, Tier::Large, Tier::Boss] {
            for spec in ring_plan(t, 96, 84) {
                let (r_birth, th_birth, a_birth) = ring_state(&spec, 0);
                assert_eq!(th_birth, spec.thick);
                assert!(r_birth > spec.r0 && a_birth == spec.alpha);
                let (r_end, th_end, _) = ring_state(&spec, spec.grow);
                assert_eq!((r_end, th_end), (spec.r1, 1.0));
                assert!(ring_state(&spec, spec.frames - 1).2 < spec.alpha * 0.2);
            }
        }
        // Boss: thin (1 to 2 px) and beyond the 264 px screen.
        for spec in ring_plan(Tier::Boss, 332, 84) {
            assert!(spec.thick <= 2.0);
        }
        assert!(ring_plan(Tier::Boss, 332, 84)[0].r1 >= 220.0);
    }

    /// Hot rings: white-yellow at birth, orange halfway, dark red at the end.
    #[test]
    fn rings_cool_from_white_yellow_to_dark_red() {
        for t in [Tier::Medium, Tier::Large, Tier::Boss] {
            let spec = ring_plan(t, 96, 84)[0];
            let birth = ring_color(&spec, 0);
            assert!(birth[0] == 255 && birth[1] >= 240 && birth[2] >= 180, "{t:?} birth {birth:?}");
            let mid = ring_color(&spec, spec.frames / 2);
            assert!(mid[0] == 255 && (100..=200).contains(&mid[1]) && mid[2] < 100, "{t:?} middle {mid:?}");
            let end = ring_color(&spec, spec.frames - 1);
            assert!(end[0] < 180 && end[1] < 60 && end[2] < 40, "{t:?} end {end:?}");
            assert!((2.0..=3.0).contains(&spec.halo));
        }
    }

    /// The hollow red ring of the explosion sprites (type 1: frames 122 to 133)
    /// is replaced from frame 129, the fire ball before it stays.
    #[test]
    fn only_the_ring_frames_of_the_explosion_sprites_are_replaced() {
        assert!((122..=128).all(|f| !ring_phase_frame(f)));
        assert!((129..=133).all(ring_phase_frame));
        assert!(!ring_phase_frame(134));
        assert!(ring_phase_start(129) && !ring_phase_start(130));
    }

    /// Embers: 1 or 2 px, yellow-orange at birth, dark red at the end; in
    /// Original art they take the nearest color of the palette.
    #[test]
    fn embers_cool_and_follow_the_palette_in_original_art() {
        let mut m = ModernExplosions::new(5);
        m.spawn_embers(100.0, 80.0);
        assert_eq!(m.embers.len(), EMBERS_PER_SPRITE);
        assert!(m.embers.iter().all(|e| (1..=2).contains(&e.size) && e.life <= 18));
        let hot = heat_color(0.8);
        assert!(hot[0] == 255 && hot[1] > 150 && hot[2] < 140, "{hot:?}");
        let cold = heat_color(0.05);
        assert!(cold[0] < 160 && cold[1] < 50, "{cold:?}");
        let mut pal = vec![0u8; 768];
        pal[3..6].copy_from_slice(&[250, 200, 90]);
        pal[6..9].copy_from_slice(&[120, 30, 10]);
        assert_eq!(nearest_palette_color(&pal, hot), [250, 200, 90]);
        assert_eq!(nearest_palette_color(&pal, cold), [120, 30, 10]);
    }

    /// The boss draws from its own generator: the debris of another enemy
    /// destroyed after it are the same as without the boss.
    #[test]
    fn boss_chain_leaves_the_debris_of_other_enemies_unchanged() {
        let kill = |tier, bbox| Kill {
            impact: (107.0, 85.0),
            dir: (0.0, -1.0),
            bbox,
            cells: 16,
            parts: 4,
            value: 100,
            tier,
            ground: false,
        };
        let medium = kill(Tier::Medium, (80, 70, 128, 126));
        let boss = kill(Tier::Boss, (0, 20, 200, 104));
        let mut a = ModernExplosions::new(9);
        a.spawn(&medium);
        let mut b = ModernExplosions::new(9);
        b.spawn(&boss);
        b.boss_final(&boss);
        let before = b.debris.len();
        b.spawn(&medium);
        let pos = |d: &[Debris]| d.iter().map(|d| (d.x, d.y, d.vx, d.vy, d.size)).collect::<Vec<_>>();
        assert!(before > 0, "the final burst throws fragments");
        assert_eq!(pos(&b.debris[before..]), pos(&a.debris));
    }

    /// Boss: 5 to 8 small explosions over the hull during the first second,
    /// no big flash before; then the final burst (flash, streak and rings).
    #[test]
    fn boss_dies_in_a_chain_then_bursts() {
        use tyrian_sim::PlayerInput;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let data = root.join("data");
        let f = root.join("rewrite/golden/e1-tyrian-autopilot.gm");
        if !data.join("tyrian.hdt").exists() || !f.exists() {
            eprintln!("data or golden master missing: test skipped");
            return;
        }
        let gm = tyrian_golden::GoldenMaster::load(&f).unwrap();
        let mut sim = gm.make_sim(&data).unwrap();
        sim.set_fx_config(tyrian_sim::fx::FxConfig { events: true, ..Default::default() });
        sim.set_keep_presented(true);
        let mut fx = crate::fx_render::FxRender::new(7);
        fx.options.particles = true;
        fx.options.style = crate::fx_render::STYLE_MODERN;
        let (mut killed_at, mut links, mut final_at, mut big_before) = (None, Vec::new(), None, false);
        for (i, t) in gm.ticks.iter().enumerate() {
            sim.step([PlayerInput(t.mask), PlayerInput(0)]);
            let ev = sim.take_fx_events();
            let w = sim.presented_world();
            fx.frame(ev, w, w.iteration, 0);
            let m = &fx.modern;
            if killed_at.is_none() && m.tier_counts[3] > 0 {
                killed_at = Some(i);
                big_before = m.flashes.iter().any(|f| f.flare > 0.0 && f.r > 30.0);
            }
            let Some(k) = killed_at else { continue };
            // A flash placed during the update is drawn at age -1 in the frame it fires.
            for fl in m.flashes.iter().filter(|f| f.age == -1) {
                if fl.flare >= 264.0 {
                    final_at.get_or_insert(i - k);
                } else if fl.flare == 0.0 && fl.r <= 7.0 && final_at.is_none() {
                    links.push(i - k);
                }
            }
            if i > k + 60 {
                break;
            }
        }
        assert!(killed_at.is_some(), "no boss death");
        assert!(!big_before, "big flash at the moment of death");
        assert!((5..=8).contains(&links.len()), "links {links:?}");
        assert!(links.iter().all(|&l| l < BOSS_CHAIN_FRAMES as usize), "links {links:?}");
        assert_eq!(final_at, Some(BOSS_CHAIN_FRAMES as usize), "final burst");
    }

    /// Death by the death chain: the impact is the position of the shot, not the
    /// center of the enemy; one 2x2 sprite is small, four 2x2 in a row
    /// medium, nine 2x2 in a square large.
    #[test]
    fn impact_is_the_killing_shot_and_size_the_sprites() {
        let hit = FxEvent::ShotHit { x: 100, y: 80, damage: 5, frame: 1 };
        let kill = FxEvent::EnemyKilled { x: 100, y: 80, damage: 5, value: 30, frame: 1, shot_dx: 0, shot_dy: -8 };
        let one = read_kills(&[hit, kill, FxEvent::Explosion { x: 90, y: 70, large: true, ground: false }], &[]);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].impact, ((100 + IMPACT_DX) as f32, (80 + IMPACT_DY) as f32));
        assert_eq!(one[0].tier, Tier::Small);
        assert_eq!(one[0].dir, (0.0, -1.0));
        let mut ev = vec![hit];
        for k in 0..4 {
            ev.push(kill);
            ev.push(FxEvent::Explosion { x: 70 + 24 * k, y: 70, large: true, ground: false });
        }
        let four = read_kills(&ev, &[]);
        assert_eq!(four.len(), 1);
        assert_eq!(four[0].tier, Tier::Medium);
        assert_eq!(four[0].parts, 4);
        assert_eq!(four[0].size(), (96, 28));
        let mut nine = vec![hit];
        for k in 0..9 {
            nine.push(kill);
            nine.push(FxEvent::Explosion { x: 70 + 24 * (k % 3), y: 70 + 28 * (k / 3), large: true, ground: false });
        }
        assert_eq!(read_kills(&nine, &[])[0].tier, Tier::Large);
        let small = read_kills(&[hit, kill, FxEvent::Explosion { x: 98, y: 78, large: false, ground: false }], &[]);
        assert_eq!(small[0].tier, Tier::Small);
        assert_eq!(small[0].size(), (12, 14));
        ev.push(FxEvent::BossDefeated { x: 100, y: 80, value: 500, frame: 1 });
        assert_eq!(read_kills(&ev, &[])[0].tier, Tier::Boss);
    }

    /// The Modern explosions only read the presented world: a game
    /// played with and without them keeps the same state fingerprint, tick by
    /// tick, and they saw small and large destructions and the boss.
    #[test]
    fn modern_explosions_leave_the_simulation_untouched() {
        use tyrian_sim::PlayerInput;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let data = root.join("data");
        let f = root.join("rewrite/golden/e1-tyrian-autopilot.gm");
        if !data.join("tyrian.hdt").exists() || !f.exists() {
            eprintln!("data or golden master missing: test skipped");
            return;
        }
        let gm = tyrian_golden::GoldenMaster::load(&f).unwrap();
        let run = |modern: bool| -> (Vec<u64>, [u64; 4]) {
            let mut sim = gm.make_sim(&data).unwrap();
            sim.set_fx_config(tyrian_sim::fx::FxConfig { events: true, ..Default::default() });
            sim.set_keep_presented(true);
            let mut fx = crate::fx_render::FxRender::new(7);
            fx.options.particles = modern;
            fx.options.style = crate::fx_render::STYLE_MODERN;
            let mut px = vec![[0u8; 4]; 320 * 200];
            let mut hashes = Vec::new();
            for t in &gm.ticks {
                sim.step([PlayerInput(t.mask), PlayerInput(0)]);
                let ev = sim.take_fx_events();
                let w = sim.presented_world();
                fx.frame(ev, w, w.iteration, 0);
                px.iter_mut().for_each(|p| *p = [0; 4]);
                fx.draw_rgba(&mut px, |_, _, _, _, _, _| {});
                hashes.push(sim.state_hash());
            }
            (hashes, fx.modern.tier_counts)
        };
        let (base, _) = run(false);
        let (h, tiers) = run(true);
        assert_eq!(h, base, "the simulation changed");
        assert!(tiers[0] > 0 && tiers[2] > 0 && tiers[3] >= 1, "sizes seen {tiers:?}");
    }

    /// Table of rings and flashes (cargo test --release -- --ignored
    /// --nocapture ring_table): per class and reference footprint, radius
    /// at peak, thickness frame by frame, duration.
    #[test]
    #[ignore]
    fn ring_table() {
        let classes = [
            ("small", Tier::Small, 24, 28),
            ("medium", Tier::Medium, 48, 28),
            ("medium wide", Tier::Medium, 48, 56),
            ("large", Tier::Large, 96, 84),
            ("large long", Tier::Large, 223, 34),
            ("boss", Tier::Boss, 332, 84),
        ];
        for (name, t, w, h) in classes {
            println!("{name} {w}x{h}: flash {:?}", flash_plan(t, w, h));
            for s in ring_plan(t, w, h) {
                let rows: Vec<String> = (0..s.frames)
                    .map(|a| {
                        let (r, th, al) = ring_state(&s, a);
                        format!("r{r:.0}/e{th:.1}/a{al:.2}")
                    })
                    .collect();
                println!(
                    "  ring delay {} growth {} life {}: radius {:.0}->{:.0}, thickness {:.0}->1: {}",
                    s.delay,
                    s.grow,
                    s.frames,
                    s.r0,
                    s.r1,
                    s.thick,
                    rows.join(" ")
                );
            }
        }
    }

    /// Worst case measured (cargo test --release -- --ignored --nocapture): one
    /// boss death and 24 destructions (8 of each size) in the same
    /// frame, then 90 frames; update and draw duration.
    #[test]
    #[ignore]
    fn worst_case_timing() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let data = root.join("data");
        let gm = tyrian_golden::GoldenMaster::load(&root.join("rewrite/golden/e1-tyrian-autopilot.gm")).unwrap();
        let sim = gm.make_sim(&data).unwrap();
        let w = sim.world();
        {
            let mut fx = crate::fx_render::FxRender::new(3);
            fx.options.particles = true;
            fx.options.style = crate::fx_render::STYLE_MODERN;
            let mut ev = vec![(1, FxEvent::ShotHit { x: 130, y: 90, damage: 10, frame: 1 })];
            ev.push((
                1,
                FxEvent::EnemyKilled { x: 130, y: 90, damage: 10, value: 750, frame: 1, shot_dx: 0, shot_dy: -8 },
            ));
            ev.push((1, FxEvent::Explosion { x: 124, y: 80, large: true, ground: false }));
            ev.push((1, FxEvent::BossDefeated { x: 130, y: 90, value: 750, frame: 1 }));
            for k in 0..24 {
                let (x, y) = (20 + (k % 8) * 30, 20 + (k / 8) * 50);
                ev.push((1, FxEvent::ShotHit { x, y, damage: 10, frame: 1 }));
                // Small: one 2x2 sprite; medium: four 2x2 in a row; large: nine 2x2 in a square.
                let parts = [1, 4, 9][(k / 8) as usize];
                let cols = [1, 4, 3][(k / 8) as usize];
                for p in 0..parts {
                    ev.push((
                        1,
                        FxEvent::EnemyKilled { x, y, damage: 10, value: 50, frame: 1, shot_dx: 0, shot_dy: -8 },
                    ));
                    let (ex, ey) = (x + 24 * (p % cols), y + 28 * (p / cols));
                    ev.push((1, FxEvent::Explosion { x: ex, y: ey, large: true, ground: k % 2 == 0 }));
                }
            }

            let mut px = vec![[0u8; 4]; 320 * 200];
            let (mut worst_u, mut worst_d, mut total, mut peak) = (0u128, 0u128, 0u128, 0usize);
            for i in 0..90u64 {
                let t0 = std::time::Instant::now();
                fx.frame(if i == 0 { ev.clone() } else { Vec::new() }, w, 1 + i, 0);
                let t1 = std::time::Instant::now();
                px.iter_mut().for_each(|p| *p = [0; 4]);
                fx.draw_rgba(&mut px, |_, _, _, _, _, _| {});
                let t2 = std::time::Instant::now();
                worst_u = worst_u.max((t1 - t0).as_micros());
                worst_d = worst_d.max((t2 - t1).as_micros());
                total += (t2 - t0).as_micros();
                peak = peak.max(fx.modern.live_count() + fx.particle_count());
            }
            println!(
                "sizes {:?}, worst update {worst_u} µs, worst draw {worst_d} µs, average {} µs, elements at peak {peak}",
                fx.modern.tier_counts,
                total / 90
            );
        }
    }

    /// Classification table of episode 1 (cargo test --release -- --ignored
    /// --nocapture): for each e1-* golden master, each distinct destruction
    /// (footprint, pieces, value) and its size.
    #[test]
    #[ignore]
    fn episode_1_classification_table() {
        use std::collections::BTreeMap;
        use tyrian_sim::PlayerInput;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let data = root.join("data");
        let dir = root.join("rewrite/golden");
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
            .filter(|n| n.starts_with("e1-") && n.ends_with("-autopilot.gm"))
            .collect();
        names.sort();
        // (level, size, width, height) -> (destructions, pieces min/max, value min/max).
        type Row = (u32, u32, u32, i32, i32);
        let mut table: BTreeMap<(String, Tier, i32, i32), Row> = BTreeMap::new();
        for n in &names {
            let gm = tyrian_golden::GoldenMaster::load(&dir.join(n)).unwrap();
            let level = gm.header.level_name.trim().to_string();
            let mut sim = gm.make_sim(&data).unwrap();
            sim.set_fx_config(tyrian_sim::fx::FxConfig { events: true, ..Default::default() });
            sim.set_keep_presented(true);
            let mut last: Vec<SeenEnemy> = Vec::new();
            for t in &gm.ticks {
                sim.step([PlayerInput(t.mask), PlayerInput(0)]);
                let ev: Vec<FxEvent> = sim.take_fx_events().into_iter().map(|(_, e)| e).collect();
                let now = seen_enemies(sim.presented_world());
                let mut seen = now.clone();
                seen.extend_from_slice(&last);
                for k in read_kills(&ev, &seen) {
                    let (w, h) = k.size();
                    if n == "e1-tyrian-autopilot.gm" {
                        println!("tick {}: {:?} {w}x{h}", t.tick, k.tier);
                    }
                    let e = table.entry((level.clone(), k.tier, w, h)).or_insert((0, u32::MAX, 0, i32::MAX, i32::MIN));
                    *e = (e.0 + 1, e.1.min(k.parts), e.2.max(k.parts), e.3.min(k.value), e.4.max(k.value));
                }
                last = now;
            }
        }
        println!("level | size | footprint (px) | destructions | pieces | value");
        for ((level, tier, w, h), (count, p0, p1, v0, v1)) in &table {
            println!("{level} | {tier:?} | {w}x{h} | {count} | {p0}-{p1} | {v0}-{v1}");
        }
    }
}
