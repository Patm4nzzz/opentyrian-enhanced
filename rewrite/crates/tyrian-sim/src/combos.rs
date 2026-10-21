//! Combos (`combos.c`): kill chain, tiers and multiplier.
//!
//! The multiplier weighs on money (`collision.c`, "evalue *
//! (combo_enabled ? combo_multiplier : 1)") and on the Endless score
//! (`endless.c:on_enemy_killed`). The C code decays the chain in real time
//! (`combos.c:combo_update(delta)` called by `tyrian2.c:JE_main` with
//! `SDL_GetTicks`): the Sim receives a fixed duration per game loop
//! ([`ComboState::frame_ms`]), like the C golden master (`GOLDEN_COMBO_MS`);
//! the Godot client sets the nominal duration of one loop there.

/// `combos.c:COMBO_WINDOW`: 1.5 s to chain kills.
pub const COMBO_WINDOW_MS: u32 = 1500;

/// `combos.c:combo_tiers` (highest to lowest): (kills,
/// multiplier, translation key of the tier name).
pub const COMBO_TIERS: [(u32, u32, &str); 8] = [
    (100, 10, "combo_impossible"),
    (50, 5, "combo_mythical"),
    (30, 4, "combo_legendary"),
    (25, 4, "combo_godlike"),
    (20, 3, "combo_unstoppable"),
    (15, 3, "combo_frenzy"),
    (10, 2, "combo_rampage"),
    (5, 2, "combo_combo"),
];

/// State of `combos.c` (`combo_count`, `combo_timer`, `combo_enabled`). The
/// multiplier stays in `World::combo_multiplier`, read by the gains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ComboState {
    /// `combo_enabled`: false = combos off (historical golden masters).
    pub enabled: bool,
    pub count: u32,
    pub timer_ms: u32,
    /// Duration of one loop passed to `combo_update` (0 = no decay).
    pub frame_ms: u32,
    /// Index into `COMBO_TIERS` of the last tier reached during this tick
    /// (`EVENT_COMBO_TIER_REACHED`, message "NAME xM"); the client reads it
    /// then resets it to `None`.
    pub tier_reached: Option<usize>,
}

/// Multiplier of the highest tier reached (`combos.c:combo_on_kill`).
pub fn multiplier_for(count: u32) -> u32 {
    COMBO_TIERS.iter().find(|t| count >= t.0).map_or(1, |t| t.1)
}

impl crate::world::World {
    /// `combos.c:combo_on_kill`, listener of `EVENT_ENEMY_KILLED` registered at
    /// startup (`opentyr.c:combos_register_listeners`), hence before the Endless
    /// one: the Endless score sees the already updated multiplier.
    pub fn combo_on_kill(&mut self) {
        if !self.combo.enabled {
            return;
        }
        self.combo.timer_ms = 0;
        self.combo.count += 1;
        self.combo_multiplier = multiplier_for(self.combo.count);
        if let Some(i) = COMBO_TIERS.iter().position(|t| t.0 == self.combo.count) {
            self.combo.tier_reached = Some(i);
        }
    }

    /// `combos.c:combo_update(frame_ms)`; the Combo Lock anomaly
    /// (`g_anomaly_combo_lock`) doubles the window.
    pub fn combo_update(&mut self) {
        let frame_ms = self.combo.frame_ms;
        if frame_ms == 0 || self.combo.count == 0 {
            return;
        }
        self.combo.timer_ms = self.combo.timer_ms.wrapping_add(frame_ms);
        let lock = self.mode.anomaly(crate::modes::ANOM_COMBO_LOCK);
        let window = if lock { COMBO_WINDOW_MS * 2 } else { COMBO_WINDOW_MS };
        if self.combo.timer_ms >= window {
            self.combo.count = 0;
            self.combo_multiplier = 1;
            self.combo.timer_ms = 0;
        }
    }

    /// Multiplier applied to money (`collision.c`: `combo_enabled ?
    /// combo_multiplier: 1`).
    pub fn cash_combo_multiplier(&self) -> u32 {
        if self.combo.enabled {
            self.combo_multiplier
        } else {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_follow_combo_table() {
        // combos.c:combo_on_kill: highest tier reached, 1 below 5.
        let expect = [
            (0, 1),
            (4, 1),
            (5, 2),
            (9, 2),
            (10, 2),
            (15, 3),
            (20, 3),
            (25, 4),
            (30, 4),
            (49, 4),
            (50, 5),
            (99, 5),
            (100, 10),
            (500, 10),
        ];
        for (n, m) in expect {
            assert_eq!(multiplier_for(n), m, "{n} destructions");
        }
    }
}
