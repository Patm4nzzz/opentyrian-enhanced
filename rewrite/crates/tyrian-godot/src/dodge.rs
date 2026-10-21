//! Unassisted driving for the campaign bot (`tools/campaign_bot.gd
//! --campaign-bot-drive=dodge`): relays [`tyrian_dodge_bot::DodgeBot`], which
//! reads the published simulation state (`Sim::state`) and returns a mask
//! of normal inputs. Read-only: nothing is written to the simulation,
//! the mask goes through the player actions as with a keyboard. Unused in
//! game.

use godot::prelude::*;
use tyrian_dodge_bot::{DodgeBot, Tuning};

use crate::TyrianSim;

#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct TyrianDodgeBot {
    /// A single object for the whole campaign: what the driver learns on
    /// contact (harmless sprites) is used in the following levels.
    bot: DodgeBot,
}

#[godot_api]
impl IRefCounted for TyrianDodgeBot {
    fn init(_base: Base<RefCounted>) -> Self {
        Self { bot: DodgeBot::new(Tuning::default()) }
    }
}

#[godot_api]
impl TyrianDodgeBot {
    /// Input mask (bits of `PlayerInput`, those of `Game.MASK_*`) of
    /// player 1 for the next tick of `sim`; 0 if no level is loaded.
    #[func]
    fn next_mask(&mut self, sim: Gd<TyrianSim>) -> i32 {
        let s = sim.bind();
        match s.sim.as_ref() {
            Some(sim) => self.bot.next(sim.state()).0 as i32,
            None => 0,
        }
    }

    /// Tuning for attempt `attempt` (0 = first) of the level that is
    /// starting (`Tuning::attempt`); the contact memory is kept.
    #[func]
    fn set_attempt(&mut self, attempt: i32) {
        self.bot.tuning = Tuning::attempt(attempt.max(0) as u32);
    }

    /// Sprites known to be harmless on contact (taken from the log).
    #[func]
    fn harmless_known(&self) -> i32 {
        self.bot.harmless_known() as i32
    }
}
