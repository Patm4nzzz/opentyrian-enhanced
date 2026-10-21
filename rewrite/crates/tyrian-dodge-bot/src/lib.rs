//! Unassisted driving that dodges: enemies, enemy shots and obstacles.
//!
//! The driver only reads the state published by the simulation at each tick
//! ([`SimState`]: ship, enemies, enemy shots), that is, what the game
//! draws on screen, and only returns a mask of normal inputs
//! (directions + fire). It reads neither the internal world, nor the random
//! generators, nor the future: speeds are estimated from one tick to the next,
//! like a player who sees the sprites move. It modifies nothing.
//!
//! Decision at each tick: 162 plans "direction held for `plan_first` (or
//! 3 x `plan_first`) ticks, then another direction up to `horizon`" are simulated with the exact
//! ship movement model (`player_movement.c`: 1 px direct + speed
//! +-4 with friction) against linearly extrapolated threats; the
//! cheapest plan (predicted collision first, then distance from the fire
//! target) gives the direction for the tick.
//!
//! Collision boxes taken from the game rules, which the player learns by
//! playing: enemy hit if |x - ex| < 12 and |y - ey| < 14
//! (`player_collide.c`); enemy shot if |sx - x| < 11..12 and |sy - y| < 10..14
//! (`shot_update.c`, ship zone).
//!
//! "Scenery": in Tyrian, background tiles do not hurt; the obstacles
//! that do hurt (barriers, rocks, asteroids, hulls) are enemies, usually
//! with armor 255 (indestructible), so they are avoided like the others.
//! Many "ground" enemies (turrets, buildings, scenery pieces) do not
//! hurt on contact (`player_collide.c`: `explosiontype & 1`), and nothing
//! on screen tells them apart: like a player, the driver learns it by
//! playing. Each sprite (sheet, frame) touched without any shield or
//! armor loss is marked harmless; touched with a loss and without an enemy shot
//! in range, it is marked dangerous. Only a sprite seen harmless several times
//! and never dangerous stops being avoided. The memory lasts the whole campaign
//! (same [`DodgeBot`] object).

use std::collections::HashMap;

use tyrian_sim::{EnemyView, PlayerInput, SimState};

/// Ship bounds (`player_movement.c`).
const X_MIN: i32 = 40;
const X_MAX: i32 = 256;
const Y_MIN: i32 = 10;
const Y_MAX: i32 = 160;

/// `tyrian.shp` sheets of objects (10) and coins / gems (11)
/// (`EnemyView::sheet`): bonuses to pick up, not threats.
const SHEET_OBJECTS: u8 = 10;
const SHEET_COINS: u8 = 11;

/// Candidate directions (dx, dy).
const DIRS: [(i32, i32); 9] = [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// Driver tuning (default values: those of the measurements).
#[derive(Debug, Clone, Copy)]
pub struct Tuning {
    /// Preferred flight line (ship y) when nothing threatens.
    pub cruise_y: i32,
    /// Margin added to the collision boxes (px), plus the uncertainty.
    pub margin: i32,
    /// Margin growth per extrapolation tick (in 1/8 px).
    pub margin_growth_8th: i32,
    /// Pick up bonuses (sheets 10 and 11) when the way is clear.
    pub collect: bool,
    /// Weight of the clutter around the end position of the plan.
    pub crowd: i64,
    /// Also extrapolate the acceleration of enemies (otherwise: speed only).
    pub accel: bool,
    /// Weight of near misses (less than 10 px of clearance).
    pub close: i64,
    /// Ticks simulated ahead of the ship.
    pub horizon: i32,
    /// Duration of the first direction of a plan.
    pub plan_first: i32,
    /// Weight of the horizontal offset from the fire target (per px).
    pub aim: i64,
    /// Weight of the proximity to an edge (per px under 24 px, sides and top;
    /// the bottom counts half, the screen only closes there through the sides).
    pub edge: i64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            cruise_y: 130,
            margin: 3,
            margin_growth_8th: 4,
            collect: true,
            crowd: 4,
            accel: false,
            close: 6,
            horizon: 18,
            plan_first: 4,
            aim: 8,
            edge: 12,
        }
    }
}

impl Tuning {
    /// Tuning for attempt `n` (0 = first) of the same level: a
    /// player who restarts after a game over does not replay identically.
    /// The level, though, replays identically (same seed, same equipment:
    /// "LAST LEVEL"), hence variants of flight line and caution.
    pub fn attempt(n: u32) -> Self {
        let base = Self::default();
        match n % 4 {
            0 => base,
            1 => Self { cruise_y: 112, ..base },
            2 => Self { cruise_y: 148, crowd: 6, ..base },
            _ => Self { cruise_y: 120, margin: 4, horizon: 24, ..base },
        }
    }
}

/// Extrapolated threat: reference point, speed, half-box.
#[derive(Debug, Clone, Copy)]
struct Threat {
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
    /// Estimated acceleration (px/tick^2), bounded.
    ax: i32,
    ay: i32,
    half_w: i32,
    half_h: i32,
    /// Unknown speed (appeared this tick): widened margin.
    unsure: bool,
    /// Lead of the published position over the collision position, in ticks:
    /// an enemy is published where it was drawn, before its movement of the
    /// tick (enemy_render.c); a shot, at its current position.
    lead: i32,
}

impl Threat {
    /// Position extrapolated in `k` ticks; acceleration only counts over
    /// 8 ticks (beyond that, the estimate is worthless).
    fn at(&self, k: i32) -> (i32, i32) {
        let ka = k.min(8);
        (self.x + self.vx * k + self.ax * ka * ka / 2, self.y + self.vy * k + self.ay * ka * ka / 2)
    }
}

/// Driver: keeps from one tick to the next what a player remembers (where
/// the sprites were, which direction was held).
#[derive(Debug, Default)]
pub struct DodgeBot {
    pub tuning: Tuning,
    prev_tick: u64,
    prev_player: Option<(i32, i32)>,
    prev_dir: (i32, i32),
    /// Estimated ship speed (inertia, `x_velocity`).
    vel: (i32, i32),
    /// Slot -> (x, y, vx, vy) at the previous tick.
    prev_enemies: HashMap<u16, (i32, i32, i32, i32)>,
    prev_shots: Vec<(i32, i32, i32, i32)>,
    // Frozen target (guardian, level lock): sweep like autopilot.c.
    last_cur_loc: u32,
    stalled: u32,
    sweep_dir: i32,
    sweep_left: i32,
    armor_sum: i64,
    no_damage: u32,
    prev_hp: i32,
    /// Sprites (sheet, frame) touched: (contacts without damage, with damage).
    contact: HashMap<(u8, u16), (u32, u32)>,
}

impl DodgeBot {
    pub fn new(tuning: Tuning) -> Self {
        Self { tuning, sweep_dir: 1, ..Default::default() }
    }

    /// Input mask of player 1 for the next `Sim::step`.
    pub fn next(&mut self, st: &SimState) -> PlayerInput {
        let p = st.players[0];
        let mut mask = PlayerInput::FIRE;
        if !p.alive {
            self.reset_tracking();
            return PlayerInput(mask);
        }
        // New game, or non-consecutive state: forget the history.
        if st.tick != self.prev_tick + 1 {
            self.reset_tracking();
        }
        self.prev_tick = st.tick;
        // Ship speed: observed displacement minus the direct pixel of
        // the held direction (player_movement.c).
        if let Some((ox, oy)) = self.prev_player {
            let (dx, dy) = (p.x - ox, p.y - oy);
            if dx.abs() <= 6 && dy.abs() <= 6 {
                self.vel = ((dx - self.prev_dir.0).clamp(-4, 4), (dy - self.prev_dir.1).clamp(-4, 4));
            }
        }
        self.learn_contacts(st);
        let threats = self.observe(st);
        let (tx, ty) = self.target(st, p.x, p.y);

        let mut best = (i64::MAX, (0, 0));
        for first in [self.tuning.plan_first, self.tuning.plan_first * 3] {
            for &a in &DIRS {
                for &b in &DIRS {
                    let c = self.plan_cost(&threats, (p.x, p.y), first, a, b, (tx, ty));
                    if c < best.0 {
                        best = (c, a);
                    }
                }
            }
        }
        let (dx, dy) = best.1;
        if dx < 0 {
            mask |= PlayerInput::LEFT;
        } else if dx > 0 {
            mask |= PlayerInput::RIGHT;
        }
        if dy < 0 {
            mask |= PlayerInput::UP;
        } else if dy > 0 {
            mask |= PlayerInput::DOWN;
        }
        self.prev_dir = (dx, dy);
        self.prev_player = Some((p.x, p.y));
        PlayerInput(mask)
    }

    /// Sprites known to be harmless on contact (for the measurements).
    pub fn harmless_known(&self) -> usize {
        self.contact.values().filter(|c| is_harmless(**c)).count()
    }

    /// Learning on contact: sprites that overlap the ship's collision
    /// box (player_collide.c) and the change in shield +
    /// armor since the previous tick.
    fn learn_contacts(&mut self, st: &SimState) {
        let p = st.players[0];
        let hp = p.armor + p.shield;
        let lost = self.prev_player.is_some() && hp < self.prev_hp;
        self.prev_hp = hp;
        if p.invulnerable_ticks > 0 {
            return;
        }
        let shot_near = st.enemy_shots.iter().any(|s| (s.x - p.x).abs() < 16 && (s.y - p.y).abs() < 18);
        for e in &st.enemies {
            if is_pickup(e) {
                continue;
            }
            let (x, y) = enemy_ref(e);
            // Clear overlap only: the published position is one tick
            // behind, a near miss is not a sure contact.
            if (x - p.x).abs() < 6 && (y - p.y).abs() < 7 {
                let c = self.contact.entry((e.sheet, e.frame)).or_default();
                if !lost {
                    c.0 += 1;
                } else if !shot_near {
                    c.1 += 1;
                }
            }
        }
    }

    fn reset_tracking(&mut self) {
        self.prev_player = None;
        self.prev_dir = (0, 0);
        self.vel = (0, 0);
        self.prev_enemies.clear();
        self.prev_shots.clear();
    }

    /// Visible threats and their estimated speeds.
    fn observe(&mut self, st: &SimState) -> Vec<Threat> {
        let mut out = Vec::with_capacity(st.enemies.len() + st.enemy_shots.len());
        let mut seen = HashMap::with_capacity(st.enemies.len());
        for e in &st.enemies {
            let (x, y) = enemy_ref(e);
            if is_pickup(e) || e.armor <= 0 || self.contact.get(&(e.sheet, e.frame)).is_some_and(|c| is_harmless(*c)) {
                continue;
            }
            let (vx, vy, ax, ay, unsure) = match self.prev_enemies.get(&e.slot) {
                Some(&(ox, oy, ovx, ovy)) if (x - ox).abs() <= 32 && (y - oy).abs() <= 32 => {
                    let (vx, vy) = (x - ox, y - oy);
                    let a = self.tuning.accel as i32;
                    (vx, vy, a * (vx - ovx).clamp(-2, 2), a * (vy - ovy).clamp(-2, 2), false)
                }
                _ => (0, 0, 0, 0, true),
            };
            seen.insert(e.slot, (x, y, vx, vy));
            out.push(Threat { x, y, vx, vy, ax, ay, half_w: 12, half_h: 14, unsure, lead: 1 });
        }
        self.prev_enemies = seen;
        // Shots: matched to the previous tick's shot closest to its
        // predicted position (shots have no identifier on screen).
        let mut shots = Vec::with_capacity(st.enemy_shots.len());
        for s in &st.enemy_shots {
            let mut best: Option<(i32, i32, i32)> = None;
            for &(ox, oy, ovx, ovy) in &self.prev_shots {
                let d = (ox + ovx - s.x).abs() + (oy + ovy - s.y).abs();
                if d <= 10 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, s.x - ox, s.y - oy));
                }
            }
            let (vx, vy, unsure) = match best {
                Some((_, vx, vy)) => (vx, vy, false),
                None => (0, 2, true),
            };
            shots.push((s.x, s.y, vx, vy));
            out.push(Threat { x: s.x, y: s.y, vx, vy, ax: 0, ay: 0, half_w: 12, half_h: 14, unsure, lead: 0 });
        }
        self.prev_shots = shots;
        out
    }

    /// Target point: under the closest destructible enemy above, or
    /// a nearby bonus; sweep if the map is frozen with no damage dealt.
    fn target(&mut self, st: &SimState, px: i32, py: i32) -> (i32, i32) {
        let cruise = self.tuning.cruise_y;
        let mut best: Option<(i32, i32)> = None;
        let mut sum = 0i64;
        let mut strongest: Option<(i32, i32)> = None;
        for e in &st.enemies {
            if is_pickup(e) || e.armor <= 0 || e.armor >= 255 {
                continue;
            }
            sum += e.armor as i64;
            let (x, y) = enemy_ref(e);
            if y > py || y < -20 {
                continue;
            }
            let d = (py - y) + 2 * (px - x).abs();
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, x));
            }
            if strongest.is_none_or(|s| e.armor > s.0) {
                strongest = Some((e.armor, x));
            }
        }
        // Frozen map (cur_loc no longer moves: guardian, level lock).
        if st.cur_loc == self.last_cur_loc {
            self.stalled += 1;
        } else {
            self.last_cur_loc = st.cur_loc;
            self.stalled = 0;
            self.sweep_left = 0;
        }
        if sum != self.armor_sum {
            self.armor_sum = sum;
            self.no_damage = 0;
        } else {
            self.no_damage += 1;
        }
        if self.stalled > 200 && self.no_damage > 400 && self.sweep_left <= 0 {
            self.sweep_left = 300;
            self.no_damage = 0;
        }
        if self.sweep_left > 0 {
            self.sweep_left -= 1;
            if px <= X_MIN + 8 {
                self.sweep_dir = 1;
            } else if px >= X_MAX - 8 {
                self.sweep_dir = -1;
            }
            return (px + self.sweep_dir * 64, cruise);
        }
        if self.tuning.collect {
            let mut pick: Option<(i32, i32, i32)> = None;
            for e in &st.enemies {
                if !is_pickup(e) {
                    continue;
                }
                let (x, y) = enemy_ref(e);
                let d = (px - x).abs() + (py - y).abs();
                if d < 90 && y > 20 && pick.is_none_or(|b| d < b.0) {
                    pick = Some((d, x, y));
                }
            }
            if let Some((_, x, y)) = pick {
                return (x, y.max(cruise - 60));
            }
        }
        let x = if self.stalled > 200 { strongest.map(|s| s.1) } else { None }.or(best.map(|b| b.1)).unwrap_or(px);
        (x, cruise)
    }

    /// Base margin: reduced when the map has been frozen with no damage dealt
    /// for a long time (protected guardian, you must get close to reach it).
    fn margin(&self) -> i32 {
        if self.stalled > 600 && self.no_damage > 300 {
            0
        } else {
            self.tuning.margin
        }
    }

    /// Cost of a plan: predicted collisions (weighted by their proximity in
    /// time), then distance to the target at the end of the plan.
    fn plan_cost(
        &self,
        threats: &[Threat],
        start: (i32, i32),
        first: i32,
        a: (i32, i32),
        b: (i32, i32),
        target: (i32, i32),
    ) -> i64 {
        let (mut x, mut y) = start;
        let (mut vx, mut vy) = self.vel;
        let mut cost: i64 = 0;
        let mut close: i64 = 0;
        let horizon = self.tuning.horizon;
        for k in 1..=horizon {
            let (dx, dy) = if k <= first { a } else { b };
            // player_movement.c: direct pixel, acceleration, friction
            // (one tick in two in x, one in three in y; approximate phase).
            x += dx;
            y += dy;
            if k % 2 == 0 {
                vx -= vx.signum();
            }
            if k % 3 == 0 {
                vy -= vy.signum();
            }
            vx = (vx + dx).clamp(-4, 4);
            vy = (vy + dy).clamp(-4, 4);
            x += vx;
            y += vy;
            if x > X_MAX {
                x = X_MAX;
                vx = 0;
            }
            if x < X_MIN {
                x = X_MIN;
                vx = 0;
            }
            if y > Y_MAX {
                y = Y_MAX;
                vy = 0;
            }
            if y < Y_MIN {
                y = Y_MIN;
                vy = 0;
            }
            let weight = (horizon + 1 - k) as i64;
            for t in threats {
                let m = self.margin() + (k * self.tuning.margin_growth_8th) / 8 + if t.unsure { 2 + k / 2 } else { 0 };
                let (tx, ty) = t.at(k + t.lead);
                let ddx = (x - tx).abs() - t.half_w;
                let ddy = (y - ty).abs() - t.half_h;
                if ddx < m && ddy < m {
                    cost += 10_000 * weight;
                } else {
                    // Near miss: we prefer to keep some air.
                    let gap = ddx.max(ddy) - m;
                    if gap < 10 {
                        close += (10 - gap as i64) * weight;
                    }
                }
            }
        }
        let mut crowd: i64 = 0;
        for t in threats {
            let (cx, cy) = t.at(horizon + 8 + t.lead);
            let d = (x - cx).abs().max((y - cy).abs());
            if d < 48 {
                crowd += (48 - d) as i64;
            }
        }
        let aim = (x - target.0).abs() as i64 * self.tuning.aim + (y - target.1).abs() as i64 * 3;
        // Stuck to an edge: fewer ways out.
        let side = (x - X_MIN).min(X_MAX - x);
        let e = self.tuning.edge;
        let edge = if side < 24 { (24 - side) as i64 * e } else { 0 }
            + if y < Y_MIN + 40 { (Y_MIN + 40 - y) as i64 * e * 2 / 3 } else { 0 }
            + if y > Y_MAX - 12 { (y - (Y_MAX - 12)) as i64 * e / 2 } else { 0 };
        cost + close * self.tuning.close + crowd * self.tuning.crowd + aim + edge
    }
}

/// Harmless: touched without damage at least 3 times, never with damage.
fn is_harmless((safe, hurt): (u32, u32)) -> bool {
    safe >= 3 && hurt == 0
}

/// Collision reference point (ex + mapoffset, ey) of a published enemy:
/// `SimState` publishes the sprite corner, shifted by (-6, -7) for a large one.
pub fn enemy_ref(e: &EnemyView) -> (i32, i32) {
    if e.size >= 2 {
        (e.x + 6, e.y + 7)
    } else {
        (e.x, e.y)
    }
}

/// Bonus to pick up (coins, gems, weapons, shield...): object sheets.
pub fn is_pickup(e: &EnemyView) -> bool {
    e.sheet == SHEET_OBJECTS || e.sheet == SHEET_COINS
}

#[cfg(test)]
mod tests {
    use super::*;
    use tyrian_sim::{PlayerView, ShotView};

    fn state(tick: u64, px: i32, py: i32) -> SimState {
        let mut st = SimState { tick, ..Default::default() };
        st.players[0] = PlayerView { x: px, y: py, armor: 10, shield: 5, alive: true, ..Default::default() };
        st
    }

    fn enemy(slot: u16, x: i32, y: i32) -> EnemyView {
        EnemyView { slot, x, y, sheet: 1, frame: 1, size: 1, layer: 0, armor: 10 }
    }

    /// A shot coming straight down at the ship: the bot steps out of
    /// the column instead of staying under it.
    #[test]
    fn dodges_a_shot_falling_on_the_ship() {
        let mut bot = DodgeBot::new(Tuning::default());
        let py = 130;
        let mut x = 150;
        for t in 1..=12u64 {
            let mut st = state(t, x, py);
            st.enemy_shots.push(ShotView { x: 150, y: 40 + 6 * t as i32, sprite: 0 });
            let m = bot.next(&st);
            assert!(m.has(PlayerInput::FIRE), "constant fire");
            if m.has(PlayerInput::LEFT) {
                x -= 2;
            } else if m.has(PlayerInput::RIGHT) {
                x += 2;
            }
        }
        assert!((x - 150).abs() >= 8, "ship stayed under the shot: x = {x}");
    }

    /// With no threat, the bot positions itself under the destructible enemy above
    /// it to hit it.
    #[test]
    fn moves_under_the_nearest_enemy_above() {
        let mut bot = DodgeBot::new(Tuning::default());
        let mut st = state(1, 100, 130);
        st.enemies.push(enemy(0, 180, 20));
        let m = bot.next(&st);
        assert!(m.has(PlayerInput::RIGHT) && !m.has(PlayerInput::LEFT));
    }

    /// An enemy charging at the ship from above: collision avoided
    /// (the bot does not stay in its path).
    #[test]
    fn leaves_the_path_of_a_diving_enemy() {
        let mut bot = DodgeBot::new(Tuning::default());
        let (mut x, py) = (150, 130);
        let mut hit = false;
        for t in 1..=20u64 {
            let ey = 20 + 6 * t as i32;
            let mut st = state(t, x, py);
            st.enemies.push(enemy(3, 150, ey));
            let m = bot.next(&st);
            if m.has(PlayerInput::LEFT) {
                x -= 2;
            } else if m.has(PlayerInput::RIGHT) {
                x += 2;
            }
            hit |= (x - 150).abs() < 12 && (py - (ey + 6)).abs() < 14;
        }
        assert!(!hit, "collision with the diving enemy");
    }

    /// Bonuses (object sheets) are not threats.
    #[test]
    fn pickups_are_not_threats() {
        let e = EnemyView { sheet: SHEET_COINS, ..enemy(0, 10, 10) };
        assert!(is_pickup(&e));
        assert!(!is_pickup(&enemy(0, 10, 10)));
    }
}
