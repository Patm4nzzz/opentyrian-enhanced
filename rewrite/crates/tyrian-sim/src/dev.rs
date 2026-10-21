//! Development keys of the C version (`dev_console.c`, active
//! only with `--devmode`): effects applied to the simulation.
//!
//! Nothing here is read until one of these functions is called:
//! `DevState::default()` leaves the simulation identical to the C to the tick
//! (golden masters unchanged). `constantPlay` does not enter the state
//! hash (`Sim::snapshot`): it is a session setting, not a level
//! state, like the C global that survives levels (the client
//! sets it again on each load).
//!
//! When applied: `dev_console_poll` is called at the end of an iteration
//! of `tyrian2.c:JE_main`, after `JE_inGameDisplays` and before
//! `JE_starShowVGA`, i.e. between the bottom half of one iteration and
//! the top half of the next. Requests are therefore queued
//! and played by `Sim::step` between `loop_bottom` and `loop_top`.

use crate::world::World;
use crate::Sim;

/// State of the development cheats (`params.c:constantPlay`,
/// `mainint.c:constantLastX`) and pending requests.
#[derive(Debug, Clone, Copy)]
pub struct DevState {
    /// `params.c:constantPlay` ("god mode" of `dev_console.c`, Ctrl+Shift+G).
    pub constant_play: bool,
    /// `mainint.c:constantLastX`: horizontal drift of the ship when
    /// `constantPlay` is true, reversed at the edges (player_movement.c).
    pub constant_last_x: i32,
    /// Ctrl+Shift+H pending.
    pub restore_armor: bool,
    /// Ctrl+Shift+C pending: credits to add.
    pub add_cash: u64,
    /// Ctrl+Shift+N pending.
    pub complete_level: bool,
}

impl Default for DevState {
    /// `tyrian2.c:JE_main`: "constantLastX = -1" at each level start.
    fn default() -> Self {
        Self { constant_play: false, constant_last_x: -1, restore_armor: false, add_cash: 0, complete_level: false }
    }
}

impl World {
    /// `dev_console.c:dev_console_poll`, pending effects (none: nothing changes).
    pub(crate) fn apply_dev_pending(&mut self) {
        let d = &mut self.dev;
        if d.restore_armor {
            // Ctrl+Shift+H: "player[0].armor = 28".
            d.restore_armor = false;
            self.players[0].armor = 28;
        }
        if d.add_cash > 0 {
            // Ctrl+Shift+C: "player[0].cash += 10000".
            self.players[0].cash += d.add_cash;
            d.add_cash = 0;
        }
        if d.complete_level {
            // Ctrl+Shift+N: successful level end (level-skip idiom of
            // mainint.c): "levelTimer = true; levelTimerCountdown = 0;
            // endLevel = true; levelEnd = 40;".
            d.complete_level = false;
            self.script.level_timer = true;
            self.script.level_timer_countdown = 0;
            self.ending.end_level = true;
            self.ending.level_end = 40;
        }
    }
}

impl Sim {
    /// `dev_console.c` Ctrl+Shift+G: "constantPlay = !constantPlay".
    /// Returns the new value.
    pub fn dev_toggle_constant_play(&mut self) -> bool {
        self.world.dev.constant_play = !self.world.dev.constant_play;
        self.world.dev.constant_play
    }

    /// Sets `constantPlay` (value kept by the client from one level to the next,
    /// like the C global).
    pub fn set_dev_constant_play(&mut self, on: bool) {
        self.world.dev.constant_play = on;
    }

    pub fn dev_constant_play(&self) -> bool {
        self.world.dev.constant_play
    }

    /// Ctrl+Shift+H, played at the next `step`.
    pub fn dev_restore_armor(&mut self) {
        self.world.dev.restore_armor = true;
    }

    /// Ctrl+Shift+C (`amount` = 10000), played at the next `step`.
    pub fn dev_add_cash(&mut self, amount: u64) {
        self.world.dev.add_cash += amount;
    }

    /// Ctrl+Shift+N, played at the next `step`.
    pub fn dev_complete_level(&mut self) {
        self.world.dev.complete_level = true;
    }
}
