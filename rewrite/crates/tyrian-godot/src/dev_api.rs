//! Development keys (`dev_console.c`, `--devmode`): effects on the
//! simulation of the current level. Without a call, nothing changes (parity).

use godot::prelude::*;

use crate::TyrianSim;

#[godot_api(secondary)]
impl TyrianSim {
    /// Ctrl+Shift+G: toggles `constantPlay`; returns the new value.
    #[func]
    fn dev_toggle_god_mode(&mut self) -> bool {
        self.sim.as_mut().is_some_and(|s| s.dev_toggle_constant_play())
    }

    /// Sets `constantPlay` (carried over from one level to the next by the client).
    #[func]
    fn set_dev_god_mode(&mut self, on: bool) {
        if let Some(s) = self.sim.as_mut() {
            s.set_dev_constant_play(on);
        }
    }

    #[func]
    fn dev_god_mode(&self) -> bool {
        self.sim.as_ref().is_some_and(|s| s.dev_constant_play())
    }

    /// Ctrl+Shift+H: player 1 armor to 28.
    #[func]
    fn dev_restore_armor(&mut self) {
        if let Some(s) = self.sim.as_mut() {
            s.dev_restore_armor();
        }
    }

    /// Ctrl+Shift+C: +`amount` credits for player 1.
    #[func]
    fn dev_add_cash(&mut self, amount: i64) {
        if let Some(s) = self.sim.as_mut() {
            s.dev_add_cash(amount.max(0) as u64);
        }
    }

    /// Ctrl+Shift+N: level completed successfully.
    #[func]
    fn dev_complete_level(&mut self) {
        if let Some(s) = self.sim.as_mut() {
            s.dev_complete_level();
        }
    }

    /// Ctrl+Shift+U: all achievements unlocked (session tracking), then
    /// saved to `dat_path` if non-empty (`JE_saveConfiguration`).
    #[func]
    fn dev_unlock_all_achievements(&self, dat_path: GString) -> bool {
        crate::ach_api::unlock_all();
        dat_path.is_empty() || crate::ach_api::save_tracker(&dat_path.to_string())
    }
}
