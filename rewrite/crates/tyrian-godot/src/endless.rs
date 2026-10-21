//! Endless run (`tyrian_sim::endless::EndlessRun`) exposed to Godot.
//!
//! `RefCounted` object held by the `Game` autoload: it survives the scene change
//! between two waves, whereas each level creates its own `TyrianSim`
//! node. The chosen level is loaded by `TyrianSim.load_level` after
//! `TyrianSim.set_endless_armor_mult(armor_mult_bits())`.

use std::path::Path;

use godot::prelude::*;
use tyrian_sim::anomalies::Deck;
use tyrian_sim::endless::{f32_bits, start_loadout, EndlessRun, START_DIFFICULTY};

#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct TyrianEndless {
    run: Option<EndlessRun>,
    /// Last level drawn: episode, section, level of `tyrianN.lvl`.
    current: [i32; 3],
    level_name: GString,
    /// `endless.c:s_anomaly_deck`.
    deck: Deck,
    /// `endless.c:s_last_two_player`.
    two_player: bool,
    /// Lives of player 2 (`s_lives_p[1]`).
    lives_p2: i32,
}

#[godot_api]
impl TyrianEndless {
    /// `endless_start(two_player)`. `difficulty` <= 0: Hard by default.
    /// `deck_seed`: seed of the anomaly deck (`anomaly_deck_init`, the C
    /// takes `SDL_GetTicks()`).
    #[func]
    fn start_run(&mut self, data_dir: GString, seed: i64, difficulty: i32, two_player: bool, deck_seed: i64) {
        self.start(data_dir, seed, difficulty);
        self.two_player = two_player;
        self.lives_p2 = if two_player { tyrian_sim::endless::START_LIVES } else { 0 };
        self.deck = Deck::new(deck_seed as u32);
    }

    #[func]
    fn two_player(&self) -> bool {
        self.two_player
    }

    #[func]
    fn lives_p2(&self) -> i32 {
        self.lives_p2
    }

    /// End of wave with two players: lives of both players read back from the Sim.
    #[func]
    fn end_wave_2p(&mut self, lives0: i32, lives1: i32, wave_score: i64) {
        self.end_wave(lives0, wave_score);
        self.lives_p2 = lives1;
    }

    /// `endless.c:lives_exhausted`: with two players, the game continues as long as one
    /// player has a life.
    #[func]
    fn run_over(&self) -> bool {
        self.lives_exhausted() && !(self.two_player && self.lives_p2 > 0)
    }

    /// Equipment of both players (`endless_start(true)`), 2 x 11 values.
    #[func]
    fn start_loadouts_2p() -> PackedInt32Array {
        let mut v = Vec::new();
        for l in tyrian_sim::endless::start_loadouts_two_player() {
            v.extend(loadout_values(&l));
        }
        PackedInt32Array::from(v)
    }

    /// `anomaly_pick_three(deck, wave)`: indices of the offered cards (0 to 3).
    #[func]
    fn pick_three(&mut self, wave: i32) -> PackedInt32Array {
        if self.deck.active.len() >= tyrian_sim::anomalies::DECK_MAX {
            return PackedInt32Array::new();
        }
        PackedInt32Array::from(
            self.deck
                .pick_three(wave.clamp(0, u16::MAX as i32) as u16)
                .into_iter()
                .map(|i| i as i32)
                .collect::<Vec<_>>(),
        )
    }

    /// `anomaly_deck_add` + `anomaly_apply` + `glass_hull_clamp_lives`.
    #[func]
    fn add_card(&mut self, card: i32) -> bool {
        if card < 0 || !self.deck.add(card as usize) {
            return false;
        }
        if self.deck.effects() & tyrian_sim::modes::ANOM_GLASS_HULL != 0 {
            if let Some(r) = self.run.as_mut() {
                r.lives = r.lives.min(1);
            }
            self.lives_p2 = self.lives_p2.min(1);
        }
        true
    }

    /// Cards of the deck (registry indices).
    #[func]
    fn deck_cards(&self) -> PackedInt32Array {
        PackedInt32Array::from(self.deck.active.iter().map(|&c| c as i32).collect::<Vec<_>>())
    }

    /// Identifier (`anomalies.json` "id") and rank (0 bronze, 1 silver, 2 gold).
    #[func]
    fn card_id(card: i32) -> GString {
        tyrian_sim::anomalies::REGISTRY.get(card.max(0) as usize).map_or(GString::new(), |c| GString::from(c.id))
    }

    #[func]
    fn card_rank(card: i32) -> i32 {
        tyrian_sim::anomalies::REGISTRY.get(card.max(0) as usize).map_or(0, |c| c.rank as i32)
    }

    /// Mask of the deck effects for `TyrianSim.set_endless_options`.
    #[func]
    fn anomaly_effects(&self) -> i32 {
        self.deck.effects() as i32
    }

    /// `anomaly_deck_score_multiplier × mutators_score_multiplier`, bits IEEE.
    #[func]
    fn score_mult_bits(&self, mutators: i32) -> i64 {
        let m = self
            .deck
            .score_multiplier()
            .mul(tyrian_sim::anomalies::mutators_score_multiplier(mutators.clamp(0, 255) as u8));
        m.0 as i64
    }

    /// Final score (`endless_apply_score_mult(s_score, deck x mutators)`).
    #[func]
    fn final_score(&self, mutators: i32) -> i64 {
        let base = self.run.as_ref().map_or(0, |r| r.score);
        tyrian_sim::anomalies::final_score(base, &self.deck, mutators.clamp(0, 255) as u8).min(i64::MAX as u64) as i64
    }

    /// `endless_start` (one player). `difficulty` <= 0: Hard by default.
    #[func]
    fn start(&mut self, data_dir: GString, seed: i64, difficulty: i32) {
        let d = if difficulty <= 0 { START_DIFFICULTY } else { difficulty.clamp(1, 9) as u8 };
        self.run = Some(EndlessRun::start(Path::new(&data_dir.to_string()), seed as u64, d));
        self.current = [0; 3];
    }

    /// Draws the next level; false if no playable level (end of game).
    #[func]
    fn next_level(&mut self) -> bool {
        match self.run.as_mut().and_then(|r| r.next_level()) {
            Some(l) => {
                self.current = [l.episode as i32, l.section as i32, l.lvl_file_num as i32];
                self.level_name = GString::from(l.level_name.as_str());
                true
            }
            None => false,
        }
    }

    #[func]
    fn level_name(&self) -> GString {
        self.level_name.clone()
    }

    #[func]
    fn episode(&self) -> i32 {
        self.current[0]
    }

    #[func]
    fn section(&self) -> i32 {
        self.current[1]
    }

    #[func]
    fn lvl_file_num(&self) -> i32 {
        self.current[2]
    }

    #[func]
    fn difficulty(&self) -> i32 {
        self.run.as_ref().map_or(START_DIFFICULTY as i32, |r| r.difficulty as i32)
    }

    #[func]
    fn wave(&self) -> i32 {
        self.run.as_ref().map_or(0, |r| r.wave())
    }

    #[func]
    fn lives(&self) -> i32 {
        self.run.as_ref().map_or(0, |r| r.lives)
    }

    /// IEEE 754 bits of `endless_armor_multiplier()` for the current wave.
    #[func]
    fn armor_mult_bits(&self) -> i64 {
        self.run.as_ref().map_or(0x3f80_0000, |r| f32_bits(r.armor_mult()) as i64)
    }

    /// Cumulative score of the completed waves (`endless.c:endless_score`).
    #[func]
    fn score(&self) -> i64 {
        self.run.as_ref().map_or(0, |r| r.score.min(i64::MAX as u64) as i64)
    }

    /// End of wave: remaining lives and wave score read back from the Sim.
    #[func]
    fn end_wave(&mut self, lives: i32, wave_score: i64) {
        if let Some(r) = self.run.as_mut() {
            r.lives = lives;
            r.score = r.score.saturating_add(wave_score.max(0) as u64);
        }
    }

    /// `mainint.c:adjust_difficulty` (ENH-02): called once per completed wave
    /// (`JE_endLevelAni` in Endless) with the total score (`JE_totalScore` of player 1,
    /// or the sum of the two banks with two players).
    #[func]
    fn adjust_difficulty(&mut self, total_score: i64) {
        let two = self.two_player;
        if let Some(r) = self.run.as_mut() {
            r.adjust_difficulty(total_score.max(0) as u64, two);
        }
    }

    /// `initialDifficulty` of the game.
    #[func]
    fn initial_difficulty(&self) -> i32 {
        self.run.as_ref().map_or(START_DIFFICULTY as i32, |r| r.initial_difficulty as i32)
    }

    /// Resume (ENDR): current and initial difficulty.
    #[func]
    fn restore_difficulty(&mut self, diff: i32, initial: i32) {
        if let Some(r) = self.run.as_mut() {
            r.difficulty = diff.clamp(1, 9) as u8;
            r.initial_difficulty = initial.clamp(1, 9) as u8;
        }
    }

    /// `deck.rng_state` (ENDR): state of the card draw.
    #[func]
    fn deck_rng(&self) -> i64 {
        self.deck.rng_state as i64
    }

    #[func]
    fn set_deck_rng(&mut self, state: i64) {
        if state as u32 != 0 {
            self.deck.rng_state = state as u32;
        }
    }

    /// `endless_math.c:endless_summary_bonus_pct` of the deck multiplier.
    #[func]
    fn deck_bonus_pct(&self) -> i32 {
        let mult = f32::from_bits(self.deck.score_multiplier().0);
        let pct = (mult - 1.0f32) * 100.0f32;
        (pct + if pct >= 0.0 { 0.5f32 } else { -0.5f32 }) as i32
    }

    /// `endless.c:lives_exhausted`.
    #[func]
    fn lives_exhausted(&self) -> bool {
        self.run.as_ref().is_none_or(|r| r.lives_exhausted())
    }

    #[func]
    fn level_completed(&mut self) {
        if let Some(r) = self.run.as_mut() {
            r.level_completed();
        }
    }

    /// Starting equipment, same order as `TyrianSim.set_loadout`.
    #[func]
    fn start_loadout() -> PackedInt32Array {
        PackedInt32Array::from(loadout_values(&start_loadout()))
    }
}

/// Same order as `TyrianSim.set_loadout`.
fn loadout_values(l: &tyrian_sim::Loadout) -> Vec<i32> {
    vec![
        l.ship.0 as i32,
        l.front_weapon.0 as i32,
        l.front_power as i32,
        l.rear_weapon.0 as i32,
        l.rear_power as i32,
        l.shield.0 as i32,
        l.generator.0 as i32,
        l.left_sidekick.0 as i32,
        l.right_sidekick.0 as i32,
        l.special.0 as i32,
        l.cash as i32,
    ]
}
