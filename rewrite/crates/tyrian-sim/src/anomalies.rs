//! Anomalies (`anomalies.c`) and mutators (`mutators.c`) in Endless: card
//! deck, draw of three cards weighted by rank between two waves, and
//! final score multiplier (`endless.c:endless_record_score`,
//! `endless_on_level_end`: `anomaly_deck_score_multiplier × mutators_score_multiplier`,
//! applied by `endless_math.c:endless_apply_score_mult`).
//!
//! The in-game effects of the cards go through `ModeState::anomalies`
//! (`ANOM_*`, `anomalies.c:apply_effect`), already ported; this module only holds
//! the deck and the C `float` arithmetic, bit-exact (`f32ops`).

use crate::f32ops::F32;

/// `anomalies.h:ANOMALY_DECK_MAX`.
pub const DECK_MAX: usize = 6;
/// `anomalies.h:ANOMALY_PICK_SIZE`.
pub const PICK_SIZE: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    Bronze,
    Silver,
    Gold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Curse,
    Affliction,
    Twist,
    Faustian,
    Synergy,
}

/// `anomalies.h:ANOM_TAG_*`.
pub const TAG_BULLET: u32 = 1 << 0;
pub const TAG_FRAGILE: u32 = 1 << 1;
pub const TAG_AGGRO: u32 = 1 << 2;
pub const TAG_FIRERATE: u32 = 1 << 3;
pub const TAG_SCORE_AMP: u32 = 1 << 4;

/// Card (`anomalies.h:Anomaly`). `effect` = `ModeSetup::anomalies` bit
/// (0 = synergy with no in-game effect).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    pub id: &'static str,
    pub rank: Rank,
    pub category: Category,
    /// Bits IEEE 754 de `score_mult` (`float`).
    pub score_mult: u32,
    pub tags: u32,
    pub exclusion_tags: u32,
    pub effect: u16,
}

use Category::*;
use Rank::*;

/// `anomalies.c:fallback_registry`, identical to the C version's `data/anomalies.json` (eleven
/// cards, same order; the JSON is re-read at startup by
/// `anomalies_json.c`, the deck indices refer to this order).
pub const REGISTRY: [Card; 11] = [
    Card {
        id: "glass_hull",
        rank: Gold,
        category: Affliction,
        score_mult: 0x4040_0000,
        tags: TAG_FRAGILE,
        exclusion_tags: 0,
        effect: 0x001,
    },
    Card {
        id: "bullet_storm",
        rank: Silver,
        category: Curse,
        score_mult: 0x3fe6_6666,
        tags: TAG_BULLET | TAG_AGGRO,
        exclusion_tags: 0,
        effect: 0x002,
    },
    Card {
        id: "twin_cannons",
        rank: Silver,
        category: Faustian,
        score_mult: 0x3fb3_3333,
        tags: TAG_FIRERATE,
        exclusion_tags: 0,
        effect: 0x004,
    },
    Card {
        id: "hardened",
        rank: Bronze,
        category: Curse,
        score_mult: 0x3fa6_6666,
        tags: TAG_AGGRO,
        exclusion_tags: 0,
        effect: 0x008,
    },
    Card {
        id: "slow_hands",
        rank: Bronze,
        category: Affliction,
        score_mult: 0x3f99_999a,
        tags: TAG_FIRERATE,
        exclusion_tags: TAG_FIRERATE,
        effect: 0x010,
    },
    Card {
        id: "eyes_closed",
        rank: Gold,
        category: Affliction,
        score_mult: 0x4020_0000,
        tags: 0,
        exclusion_tags: 0,
        effect: 0x020,
    },
    Card {
        id: "combo_lock",
        rank: Bronze,
        category: Twist,
        score_mult: 0x3fa6_6666,
        tags: 0,
        exclusion_tags: 0,
        effect: 0x040,
    },
    Card {
        id: "frost_hands",
        rank: Bronze,
        category: Affliction,
        score_mult: 0x3f99_999a,
        tags: 0,
        exclusion_tags: 0,
        effect: 0x080,
    },
    Card {
        id: "heavy_iron",
        rank: Silver,
        category: Curse,
        score_mult: 0x3fd9_999a,
        tags: TAG_AGGRO,
        exclusion_tags: TAG_AGGRO,
        effect: 0x100,
    },
    Card {
        id: "echo_chamber",
        rank: Silver,
        category: Synergy,
        score_mult: 0x3f80_0000,
        tags: TAG_SCORE_AMP,
        exclusion_tags: TAG_SCORE_AMP,
        effect: 0,
    },
    Card {
        id: "curse_doubler",
        rank: Gold,
        category: Synergy,
        score_mult: 0x3f80_0000,
        tags: TAG_SCORE_AMP,
        exclusion_tags: TAG_SCORE_AMP,
        effect: 0,
    },
];

/// Active deck (`anomalies.h:AnomalyDeck`). The C `cooldowns` are
/// never set to a non-zero value (only decremented in
/// `anomaly_pick_three`): not ported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deck {
    pub active: Vec<u8>,
    pub rng_state: u32,
}

impl Deck {
    /// `anomaly_deck_init`: seed 0 → 0xDEADBEEF.
    pub fn new(seed: u32) -> Deck {
        Deck { active: Vec::new(), rng_state: if seed != 0 { seed } else { 0xDEAD_BEEF } }
    }

    /// `anomaly_deck_add`: refused at the cap or outside the registry.
    pub fn add(&mut self, card: usize) -> bool {
        if self.active.len() >= DECK_MAX || card >= REGISTRY.len() {
            return false;
        }
        self.active.push(card as u8);
        true
    }

    /// `ModeSetup::anomalies` mask of the deck cards
    /// (`anomaly_deck_reapply` → `anomalies.c:apply_effect`).
    pub fn effects(&self) -> u16 {
        self.active.iter().fold(0, |m, &c| m | REGISTRY[c as usize].effect)
    }

    fn acceptable(&self, card: usize) -> bool {
        // anomalies.c:card_acceptable
        if self.active.iter().any(|&c| c as usize == card) {
            return false;
        }
        let (tags, excl) = self
            .active
            .iter()
            .fold((0, 0), |(t, e), &c| (t | REGISTRY[c as usize].tags, e | REGISTRY[c as usize].exclusion_tags));
        REGISTRY[card].exclusion_tags & tags == 0 && excl & REGISTRY[card].tags == 0
    }

    /// `anomalies.c:anomaly_pick_three` (xorshift32, weights from
    /// `rank_weight(rank, wave)`). Returns the drawn indices (0 to 3).
    pub fn pick_three(&mut self, wave: u16) -> Vec<usize> {
        let mut cand: Vec<(usize, u32)> = (0..REGISTRY.len())
            .filter(|&i| self.acceptable(i))
            .map(|i| (i, rank_weight(REGISTRY[i].rank, wave)))
            .collect();
        let mut total: u32 = cand.iter().map(|c| c.1).sum();
        let mut out = Vec::new();
        while out.len() < PICK_SIZE && !cand.is_empty() && total > 0 {
            let mut roll = xs32_next(&mut self.rng_state) % total;
            let mut pick = 0;
            for (j, c) in cand.iter().enumerate() {
                if roll < c.1 {
                    pick = j;
                    break;
                }
                roll -= c.1;
            }
            out.push(cand[pick].0);
            total -= cand[pick].1;
            cand.swap_remove(pick);
        }
        out
    }

    /// `anomalies.c:anomaly_deck_score_multiplier` (in `float`), with
    /// `anomalies_enabled` true.
    pub fn score_multiplier(&self) -> F32 {
        let one = F32(0x3f80_0000);
        let echo = self.active.iter().any(|&c| REGISTRY[c as usize].id == "echo_chamber");
        let doubler = self.active.iter().any(|&c| REGISTRY[c as usize].id == "curse_doubler");
        let mut total = one;
        for &c in &self.active {
            let card = REGISTRY[c as usize];
            if card.category == Synergy {
                continue;
            }
            let mut bonus = F32(card.score_mult).sub(one);
            if doubler && card.category == Curse {
                bonus = bonus.mul(F32(0x4000_0000));
            }
            if echo {
                bonus = bonus.mul(F32(0x3fa0_0000));
            }
            total = total.add(bonus);
        }
        total
    }
}

fn xs32_next(s: &mut u32) -> u32 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = if x != 0 { x } else { 1 };
    *s
}

/// `anomalies.c:rank_weight`.
pub fn rank_weight(rank: Rank, wave: u16) -> u32 {
    let w = match wave {
        0..=3 => [75, 22, 3],
        4..=9 => [60, 30, 10],
        10..=19 => [50, 35, 15],
        _ => [30, 40, 30],
    };
    w[rank as usize]
}

/// `mutators.c:mutators_challenge_count`: difficulty mutators (all
/// except Risk/Reward).
pub fn mutators_challenge_count(mutators: u8) -> u32 {
    (mutators & 0x7f).count_ones()
}

/// `mutators.c:mutators_score_multiplier`: 1 without Risk/Reward, otherwise
/// `1.0f + 0.25f * difficulty mutators`.
pub fn mutators_score_multiplier(mutators: u8) -> F32 {
    if mutators & crate::modes::MUT_RISK_REWARD == 0 {
        return F32(0x3f80_0000);
    }
    F32(0x3e80_0000).mul(F32::from_int(mutators_challenge_count(mutators) as i64)).add(F32(0x3f80_0000))
}

/// Final score of an Endless run (`endless.c:endless_record_score`):
/// `endless_apply_score_mult(s_score, deck × mutators)`.
pub fn final_score(base: u64, deck: &Deck, mutators: u8) -> u64 {
    let mult = deck.score_multiplier().mul(mutators_score_multiplier(mutators));
    mult.mul_u64_trunc_via_double(base)
}
