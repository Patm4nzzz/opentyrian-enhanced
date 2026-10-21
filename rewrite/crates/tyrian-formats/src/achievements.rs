//! Achievements of the Enhanced version (`src/achievements.[ch]`): table of the 100
//! achievements in the order of the C enumeration (this is the byte order on
//! disk) and "OTYE" block of the GAME section of `opentyrian.dat`
//! (`config.c:JE_saveConfiguration` / `JE_loadConfiguration`).
//!
//! OTYE block (little-endian): u32 magic 0x4F545945, u16 version (1),
//! 7 × u32 statistics (enemies_killed, shots_fired, games_played,
//! total_score, time_played_ms, levels_completed, deaths), then ACH_MAX
//! 0/1 bytes. The rest (v0.9.8 counters, OTY2/OTY3 extensions) is
//! kept as is without being interpreted.

use crate::{FormatError, Result};

/// "OTYE" magic read as a little-endian u32 (`config.c`).
pub const OTYE_MAGIC: u32 = 0x4F54_5945;

/// `achievements.h:ACH_MAX`.
pub const ACH_MAX: usize = 100;

/// (identifier without `ACH_`, display name, description) — `achievements.c:achievements[]`,
/// indexed by the enumeration of `achievements.h`. Generated from the C code.
pub const ACHIEVEMENTS: [(&str, &str, &str); ACH_MAX] = [
    ("ANNIHILATOR", "Annihilator", "Kill 10000 enemies"),
    ("BOSS_SLAYER", "Boss Slayer", "Kill a boss"),
    ("BULLET_HELL", "Bullet Hell", "50+ shots on screen"),
    ("BULLET_STORM", "Bullet Storm", "Fire 100000 shots"),
    ("BULLET_TIME", "Bullet Time", "Use bullet time"),
    ("CENTURION", "Centurion", "Kill 100 enemies"),
    ("CHAIN_REACTION", "Chain Reaction", "Kill 5 enemies in one frame"),
    ("CLOSE_CALL", "Close Call", "Finish level with 1 armor"),
    ("COMBO_KING", "Combo King", "Kill 10 enemies in 2 sec"),
    ("DAREDEVIL", "Daredevil", "Level with 3+ mutators"),
    ("DEDICATED", "Dedicated", "Play for 10 hours total"),
    ("DESTROYER", "Destroyer", "Kill 1000 enemies"),
    ("DIE_HARD", "Die Hard", "Die 500 times total"),
    ("DIED_100", "Endurance", "Die 100 times total"),
    ("DPS_50", "Firepower", "Achieve 50+ DPS"),
    ("DPS_100", "Firepower II", "Achieve 100+ DPS"),
    ("EPISODE_CLEAR", "Episode Clear", "Complete an episode"),
    ("FIRST_BLOOD", "First Blood", "Kill your first enemy"),
    ("GAMES_10", "Games 10", "Start 10 games"),
    ("GAMES_50", "Games 50", "Start 50 games"),
    ("GENOCIDE", "Genocide", "Kill 50000 enemies"),
    ("GLASS_CANNON", "Glass Cannon", "Level with Glass Cannon active"),
    ("GLASS_HOUSE", "Glass House", "Episode 1 with Glass Cannon"),
    ("HARD_MODE", "Hard Mode", "Complete a level on Hard+"),
    ("HIGH_ROLLER", "High Roller", "Score 500k with Risk/Reward"),
    ("IMMORTAL", "Immortal", "20 levels no-death streak"),
    ("IMPOSSIBLE", "Impossible", "Complete a level on Impossible+"),
    ("INSANITY", "Insanity", "Level with ALL mutators active"),
    ("IRONMAN", "Ironman", "10 levels without dying"),
    ("LEVEL_5", "Level 5", "Complete 5 levels"),
    ("LEVEL_20", "Level 20", "Complete 20 levels"),
    ("LEVEL_50", "Level 50", "Complete 50 levels"),
    ("MARATHON", "Marathon", "Play for 1 hour total"),
    ("MAX_POWER", "Max Power", "Have a weapon at max power"),
    ("MILLIONAIRE", "Millionaire", "Accumulate 1000000 score"),
    ("MIRROR_MASTER", "Mirror Master", "Complete a level in mirror mode"),
    ("MIRROR_VETERAN", "Mirror Veteran", "Complete 10 levels in mirror"),
    ("MODIFIER", "Modifier", "Level with any mutator active"),
    ("MUT_SURVIVOR", "Mutant Survivor", "5 levels with One Life active"),
    ("NIGHT_OWL", "Night Owl", "Play 2 hours in one session"),
    ("OBSESSED", "Obsessed", "Play for 50 hours total"),
    ("OVERKILL", "Overkill", "Achieve 500+ DPS"),
    ("PACIFIST", "Pacifist", "Survive 30s without shooting"),
    ("PERSEVERANCE", "Perseverance", "Die 250 times and keep going!"),
    ("PHOENIX", "Phoenix", "Die and still complete the level"),
    ("PHOTOGRAPHER", "Photographer", "Take a screenshot"),
    ("RICH", "Rich", "Accumulate 100000 score"),
    ("SCREENSHOT_PRO", "Screenshot Pro", "Take 25 screenshots"),
    ("SHARPSHOOTER", "Sharpshooter", "50%+ hit ratio in one run"),
    ("SHOPAHOLIC", "Shopaholic", "Spend 50000 credits total"),
    ("SLOW_MOTION_PRO", "Slow Motion Pro", "Use bullet time 50 times"),
    ("SNIPER", "Sniper", "75%+ hit ratio in one run"),
    ("SPEEDSTER", "Speedster", "Level with Fast Enemies"),
    ("SURVIVOR", "Survivor", "Complete a level without dying"),
    ("TRIGGER_HAPPY", "Trigger Happy", "Fire 10000 shots"),
    ("UNTOUCHABLE", "Untouchable", "Complete 5 levels without dying"),
    ("ALL_EPISODES", "All Episodes", "Complete all 4 episodes"),
    ("BOSS_SHREDDER", "Boss Shredder", "Defeat a boss within 10s"),
    ("COMEBACK_KID", "Comeback Kid", "Finish level after <10% HP"),
    ("FILMMAKER", "Filmmaker", "Save 5 replays"),
    ("FULL_HOUSE", "Full House", "Buy 10 items in one shop"),
    ("GLASS_MASTER", "Glass Master", "Level with Glass Cannon + One Life"),
    ("MEGA_RICH", "Mega Rich", "Accumulate 100k credits lifetime"),
    ("MODDED_RUN", "Modded Run", "Complete a level with a mod loaded"),
    ("NIGHT_PLAYER", "Night Player", "Play between midnight and 5 AM"),
    ("PERFECT_SCORE", "Perfect Score", "Complete a level with 100% hit ratio"),
    ("POLYGLOT", "Polyglot", "Play in all 6 languages"),
    ("QUICK_DRAW", "Quick Draw", "Kill enemy within 3s of level start"),
    ("TRUE_PACIFIST", "True Pacifist", "Complete a level without firing"),
    ("ZEN_MASTER", "Zen Master", "Complete a level without killing"),
    ("BOSS_RUSH", "Boss Rush", "Defeat 10 bosses"),
    ("PERFECT_BOSS", "Perfect Boss", "Kill a boss without taking damage"),
    ("OVERCLOCK", "Overclock", "Achieve 1000+ DPS"),
    ("ARSENAL_MASTER", "Arsenal Master", "Use 30 different weapons"),
    ("EXECUTION_CHAIN", "Execution Chain", "Kill 25 enemies in 2 seconds"),
    ("BULLET_BLIZZARD", "Bullet Blizzard", "100+ shots on screen"),
    ("IRON_CURTAIN", "Iron Curtain", "3 levels in a row without damage"),
    ("DODGE_ARTIST", "Dodge Artist", "Survive 2 minutes without a hit"),
    ("FLAWLESS_EPISODE", "Flawless Episode", "Complete an episode without dying"),
    ("ADAMANTIUM", "Adamantium", "50-level no-death streak"),
    ("BULLETPROOF", "Bulletproof", "Finish a level without being hit"),
    ("LAST_STAND", "Last Stand", "Finish a level started at 1 HP"),
    ("BULLET_HELL_BOSS", "Bullet Hell Boss", "Boss with Bullet Hell active"),
    ("ONE_LIFE_EPISODE", "One Life Episode", "Episode with One Life active"),
    ("MIRROR_BOSS", "Mirror Boss", "Boss in mirror mode"),
    ("RISK_TAKER", "Risk Taker", "Cross 4 Risk/Reward tiers in a run"),
    ("CHAOS_THEORY", "Chaos Theory", "All 4 episodes with a mutator"),
    ("MUTATOR_COLLECTOR", "Mutator Collector", "Finish 3 levels with any mutator"),
    ("LEVEL_100", "Level 100", "Complete 100 levels"),
    ("BILLIONAIRE", "Billionaire", "10M lifetime score"),
    ("WEAPON_COLLECTOR", "Weapon Collector", "Buy 30 different weapons"),
    ("SHIP_COLLECTOR", "Ship Collector", "Try 10 different ships"),
    ("SIDEKICK_SWAP", "Sidekick Swap", "Equip 10 different sidekicks"),
    ("PATIENT_INVESTOR", "Patient Investor", "Hold 500000 credits at once"),
    ("COMPLETIONIST", "Completionist", "Unlock 50 achievements"),
    ("ACHIEVEMENT_HUNTER", "Achievement Hunter", "Unlock 75 achievements"),
    ("SPEED_DEMON", "Speed Demon", "Finish a level in under 90s"),
    ("ENDLESS_WARRIOR", "Endless Warrior", "Reach wave 10 in Endless"),
    ("ENDLESS_LEGEND", "Endless Legend", "Reach wave 25 in Endless"),
    ("ARCADE_ENTRY", "Arcade Entry", "Enter the Endless leaderboard"),
];

/// Index of an achievement from its C identifier without prefix (`"SURVIVOR"`).
pub fn achievement_index(id: &str) -> Option<usize> {
    ACHIEVEMENTS.iter().position(|a| a.0 == id)
}

/// Statistics and achievements of the OTYE block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AchievementBlock {
    pub version: u16,
    pub stats: [u32; 7],
    pub unlocked: [bool; ACH_MAX],
    /// Bytes that follow the achievements (counters and extensions), copied as is.
    pub trailing: Vec<u8>,
}

impl Default for AchievementBlock {
    fn default() -> Self {
        Self { version: 1, stats: [0; 7], unlocked: [false; ACH_MAX], trailing: Vec::new() }
    }
}

impl AchievementBlock {
    /// Reads a block that starts with the OTYE magic. Like `JE_loadConfiguration`,
    /// a shorter achievement list (old saves with 70 achievements)
    /// leaves the following ones locked.
    pub fn parse(b: &[u8]) -> Result<Self> {
        let bad = |m: &str| FormatError::new(format!("OTYE block: {m}"));
        if b.len() < 6 + 28 {
            return Err(bad("too short"));
        }
        let magic = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if magic != OTYE_MAGIC {
            return Err(bad("missing magic"));
        }
        let version = u16::from_le_bytes([b[4], b[5]]);
        let mut stats = [0u32; 7];
        for (i, s) in stats.iter_mut().enumerate() {
            let o = 6 + i * 4;
            *s = u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        }
        let rest = &b[34..];
        let n = rest.len().min(ACH_MAX);
        let mut unlocked = [false; ACH_MAX];
        for i in 0..n {
            unlocked[i] = rest[i] != 0;
        }
        Ok(Self { version, stats, unlocked, trailing: rest[n..].to_vec() })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(34 + ACH_MAX + self.trailing.len());
        out.extend_from_slice(&OTYE_MAGIC.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        for s in self.stats {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.extend(self.unlocked.iter().map(|&u| u as u8));
        out.extend_from_slice(&self.trailing);
        out
    }

    /// Exact position of the block in the GAME section, as written by
    /// `config.c:JE_saveConfiguration` and read back by `JE_loadConfiguration`:
    /// `saveTemp` (SIZEOF_SAVEGAMETEMP = 2398 + 4 + 100 = 2502 bytes), then the
    /// 10 × 3 T2K scores (s32, u8 length, 29 bytes of name, u8 difficulty =
    /// 35 bytes), then the next 10 × 3 with an extra unknown s32 (39).
    pub const GAME_OFFSET: usize = 2502 + 30 * 35 + 30 * 39;

    /// Read at the exact position (`JE_loadConfiguration`): `None` if the
    /// section is too short or if the magic is missing — the C code then keeps its
    /// default values.
    pub fn at_exact_position(game: &[u8]) -> Option<Self> {
        Self::parse(game.get(Self::GAME_OFFSET..)?).ok()
    }

    /// Looks for the block in a complete GAME section: it follows the encrypted
    /// save and the 20 × 3 T2K high scores, the exact position
    /// depending on the old or new score format; the
    /// first occurrence of the magic after `saveTemp` (2502 bytes) is taken.
    pub fn find_in_game_section(game: &[u8]) -> Option<(usize, Self)> {
        let start = 2502.min(game.len());
        let magic = OTYE_MAGIC.to_le_bytes();
        let pos = game[start..].windows(4).position(|w| w == magic)? + start;
        Self::parse(&game[pos..]).ok().map(|b| (pos, b))
    }
}

/// Lifetime counters that the rewrite owns in the OTYE block: the 7 `u32`
/// of `game_stats` (widened to `u64` by OTY2) and two OTY3 counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LifetimeCounters {
    /// C order: enemies_killed, shots_fired, games_played, total_score,
    /// time_played_ms, levels_completed, deaths.
    pub stats: [u64; 7],
    /// `screenshots_taken` (OTY3, `u32` no. 0).
    pub screenshots_taken: u32,
    /// `bullet_time_uses` (OTY3, extension at the end of the block).
    pub bullet_time_uses: u32,
    /// `replays_saved` (OTY3, `u32` no. 4): manual recording stops (F10), lifetime.
    pub replays_saved: u32,
    /// Collection counters (`stats.c`), persisted as in the C code: v0.9.8
    /// base (after the achievements) then OTY3.
    pub total_bosses_defeated: u32,
    pub mutator_levels_completed: u32,
    pub weapons_used_mask: u64,
    pub ships_tried_mask: u64,
    pub sidekicks_used_mask: u64,
    pub weapons_bought_mask: u64,
    pub weapons_bought_mask_hi: u64,
    pub mirror_levels_completed: u32,
    pub total_credits_spent: u32,
    pub mut_one_life_levels: u32,
    pub zero_damage_level_streak: u32,
    pub deathless_streak: u32,
    pub episodes_completed_mask: u64,
    pub languages_played_mask: u64,
    pub lifetime_credits: u64,
    pub chaos_episodes_mask: u64,
    pub episode_death_count: u32,
    pub episode_start_episode: u32,
}

const MAGIC_OTY2: u32 = 0x3254_594F;
const MAGIC_OTY3: u32 = 0x3354_594F;
/// Bytes of `trailing` before OTY2: 2 × u32 + 4 × u64 (`total_bosses_defeated`…
/// `weapons_bought_mask`, `config.c:JE_saveConfiguration`).
const OTY_BASE: usize = 2 * 4 + 4 * 8;
/// OTY2 payload: 4 × u64.
const OTY2_LEN: usize = 32;
/// Minimal OTY3 payload for `bullet_time_uses`: 7 × u32 + 4 × u64 + u32.
const OTY3_MIN_LEN: usize = 7 * 4 + 4 * 8 + 4;
/// Full OTY3 payload written by the C code: 10 × u32 + 5 × u64.
const OTY3_FULL_LEN: usize = 10 * 4 + 5 * 8;

/// Tagged block (u32 magic, u16 length, payload) at `pos`:
/// (start of payload, length), `None` if absent or truncated.
fn tagged_block(t: &[u8], pos: usize, magic: u32) -> Option<(usize, usize)> {
    let h = t.get(pos..pos + 6)?;
    if u32::from_le_bytes([h[0], h[1], h[2], h[3]]) != magic {
        return None;
    }
    let len = u16::from_le_bytes([h[4], h[5]]) as usize;
    if pos + 6 + len > t.len() {
        return None;
    }
    Some((pos + 6, len))
}

/// Guarantees a `magic` block at `pos` whose payload is at least
/// `min_len` bytes (created with `new_len` zero bytes, otherwise padded with
/// zeros); returns (start of payload, length). The unknown bytes that
/// follow stay intact.
fn ensure_block(t: &mut Vec<u8>, pos: usize, magic: u32, min_len: usize, new_len: usize) -> (usize, usize) {
    match tagged_block(t, pos, magic) {
        Some((start, len)) if len >= min_len => (start, len),
        Some((start, len)) => {
            t.splice(start + len..start + len, std::iter::repeat_n(0u8, min_len - len));
            t[pos + 4..pos + 6].copy_from_slice(&(min_len as u16).to_le_bytes());
            (start, min_len)
        }
        None => {
            let mut blk = magic.to_le_bytes().to_vec();
            blk.extend_from_slice(&(new_len as u16).to_le_bytes());
            blk.extend(std::iter::repeat_n(0u8, new_len));
            t.splice(pos..pos, blk);
            (pos + 6, new_len)
        }
    }
}

fn le32(t: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([t[o], t[o + 1], t[o + 2], t[o + 3]])
}

fn le64(t: &[u8], o: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&t[o..o + 8]);
    u64::from_le_bytes(b)
}

impl AchievementBlock {
    /// Lifetime counters as read back by `config.c:JE_loadConfiguration`:
    /// the 7 `u32`, then OTY2 (u64) replaces them (kills, shots, score, time)
    /// if present with at least 32 bytes, then OTY3 gives
    /// `screenshots_taken` (>= 60 bytes) and `bullet_time_uses` (>= 64).
    pub fn counters(&self) -> LifetimeCounters {
        let mut c = LifetimeCounters { stats: self.stats.map(u64::from), ..Default::default() };
        let t = &self.trailing;
        if t.len() < OTY_BASE {
            return c;
        }
        let mut pos = OTY_BASE;
        c.total_bosses_defeated = le32(t, 0);
        c.mutator_levels_completed = le32(t, 4);
        c.weapons_used_mask = le64(t, 8);
        c.ships_tried_mask = le64(t, 16);
        c.sidekicks_used_mask = le64(t, 24);
        c.weapons_bought_mask = le64(t, 32);
        if let Some((start, len)) = tagged_block(t, pos, MAGIC_OTY2) {
            pos = start + len;
            if len >= OTY2_LEN {
                c.stats[0] = le64(t, start);
                c.stats[1] = le64(t, start + 8);
                c.stats[3] = le64(t, start + 16);
                c.stats[4] = le64(t, start + 24);
            }
        }
        if let Some((start, len)) = tagged_block(t, pos, MAGIC_OTY3) {
            // Same reading as config.c: the core (60 bytes) then
            // extensions added at the end of the block, each optional.
            if len >= OTY3_MIN_LEN - 4 {
                c.screenshots_taken = le32(t, start);
                c.mirror_levels_completed = le32(t, start + 4);
                c.total_credits_spent = le32(t, start + 8);
                c.mut_one_life_levels = le32(t, start + 12);
                c.replays_saved = le32(t, start + 16);
                c.zero_damage_level_streak = le32(t, start + 20);
                c.deathless_streak = le32(t, start + 24);
                c.episodes_completed_mask = le64(t, start + 28);
                c.languages_played_mask = le64(t, start + 36);
                c.lifetime_credits = le64(t, start + 44);
                c.chaos_episodes_mask = le64(t, start + 52);
            }
            if len >= OTY3_MIN_LEN {
                c.bullet_time_uses = le32(t, start + OTY3_MIN_LEN - 4);
            }
            if len >= OTY3_MIN_LEN + 8 {
                c.weapons_bought_mask_hi = le64(t, start + OTY3_MIN_LEN);
            }
            if len >= OTY3_MIN_LEN + 16 {
                c.episode_death_count = le32(t, start + OTY3_MIN_LEN + 8);
                c.episode_start_episode = le32(t, start + OTY3_MIN_LEN + 12);
            }
        }
        c
    }

    /// Writes the counters like `config.c:JE_saveConfiguration`: `u32`
    /// truncated in `stats`, u64 in OTY2, all OTY3 counters. Absent
    /// blocks are created (OTY3 at its full size); the unknown bytes
    /// of `trailing` are kept.
    pub fn set_counters(&mut self, c: &LifetimeCounters) {
        for (dst, v) in self.stats.iter_mut().zip(c.stats) {
            *dst = v as u32;
        }
        let t = &mut self.trailing;
        if t.len() < OTY_BASE {
            t.resize(OTY_BASE, 0);
        }
        t[0..4].copy_from_slice(&c.total_bosses_defeated.to_le_bytes());
        t[4..8].copy_from_slice(&c.mutator_levels_completed.to_le_bytes());
        for (i, v) in [c.weapons_used_mask, c.ships_tried_mask, c.sidekicks_used_mask, c.weapons_bought_mask]
            .into_iter()
            .enumerate()
        {
            t[8 + i * 8..16 + i * 8].copy_from_slice(&v.to_le_bytes());
        }
        let (s2, l2) = ensure_block(t, OTY_BASE, MAGIC_OTY2, OTY2_LEN, OTY2_LEN);
        for (i, v) in [c.stats[0], c.stats[1], c.stats[3], c.stats[4]].into_iter().enumerate() {
            t[s2 + i * 8..s2 + i * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        let (s3, _) = ensure_block(t, s2 + l2, MAGIC_OTY3, OTY3_FULL_LEN, OTY3_FULL_LEN);
        for (o, v) in [
            (0, c.screenshots_taken),
            (4, c.mirror_levels_completed),
            (8, c.total_credits_spent),
            (12, c.mut_one_life_levels),
            (16, c.replays_saved),
            (20, c.zero_damage_level_streak),
            (24, c.deathless_streak),
            (60, c.bullet_time_uses),
            (72, c.episode_death_count),
            (76, c.episode_start_episode),
        ] {
            t[s3 + o..s3 + o + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (o, v) in [
            (28, c.episodes_completed_mask),
            (36, c.languages_played_mask),
            (44, c.lifetime_credits),
            (52, c.chaos_episodes_mask),
            (64, c.weapons_bought_mask_hi),
        ] {
            t[s3 + o..s3 + o + 8].copy_from_slice(&v.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_c_enum_order() {
        assert_eq!(ACHIEVEMENTS[0].0, "ANNIHILATOR");
        assert_eq!(achievement_index("SURVIVOR"), Some(53));
        assert_eq!(ACHIEVEMENTS[ACH_MAX - 1].0, "ARCADE_ENTRY");
    }

    #[test]
    fn roundtrip_and_short_legacy_block() {
        let mut b = AchievementBlock { stats: [1, 2, 3, 4, 5, 6, 7], ..Default::default() };
        b.unlocked[achievement_index("SURVIVOR").unwrap()] = true;
        b.unlocked[achievement_index("SPEED_DEMON").unwrap()] = true; // index 96
        b.trailing = vec![9, 9];
        let bytes = b.write();
        assert_eq!(AchievementBlock::parse(&bytes).unwrap(), b);
        // Save with 70 achievements: the following 30 stay locked.
        let mut legacy = bytes[..34 + 70].to_vec();
        legacy[34] = 1;
        let l = AchievementBlock::parse(&legacy).unwrap();
        assert!(l.unlocked[0] && l.unlocked[53] && !l.unlocked[96] && l.trailing.is_empty());
    }

    /// Block as written by the C code (`JE_saveConfiguration`): 40 bytes of
    /// v0.9.8 counters, OTY2 (32), OTY3 (80, including bullet_time_uses = 77).
    fn c_layout_block() -> Vec<u8> {
        let mut b = AchievementBlock {
            stats: [10, 20, 3, 4000, 5000, 6, 7],
            trailing: vec![0xAA; OTY_BASE],
            ..Default::default()
        };
        b.trailing.extend_from_slice(&MAGIC_OTY2.to_le_bytes());
        b.trailing.extend_from_slice(&32u16.to_le_bytes());
        for v in [(1u64 << 33) + 1, (1 << 34) + 2, (1 << 35) + 3, (1 << 36) + 4] {
            b.trailing.extend_from_slice(&v.to_le_bytes());
        }
        b.trailing.extend_from_slice(&MAGIC_OTY3.to_le_bytes());
        b.trailing.extend_from_slice(&80u16.to_le_bytes());
        let mut payload = vec![0x55u8; 80];
        payload[0..4].copy_from_slice(&24u32.to_le_bytes()); // screenshots_taken
        payload[60..64].copy_from_slice(&77u32.to_le_bytes()); // bullet_time_uses
        b.trailing.extend_from_slice(&payload);
        b.write()
    }

    #[test]
    fn c_layout_counters_parse() {
        let b = AchievementBlock::parse(&c_layout_block()).unwrap();
        let c = b.counters();
        // OTY2 replaces kills, shots, score, time; the rest comes from the u32.
        assert_eq!(c.stats, [(1 << 33) + 1, (1 << 34) + 2, 3, (1 << 35) + 3, (1 << 36) + 4, 6, 7]);
        assert_eq!((c.screenshots_taken, c.bullet_time_uses), (24, 77));
    }

    #[test]
    fn counters_roundtrip_preserves_other_bytes() {
        let bytes = c_layout_block();
        let mut b = AchievementBlock::parse(&bytes).unwrap();
        let mut c = b.counters();
        c.stats[0] += 5;
        c.stats[2] = 9;
        c.stats[3] = 0x1_0000_0005;
        c.screenshots_taken = 25;
        c.bullet_time_uses = 50;
        b.set_counters(&c);
        let out = b.write();
        assert_eq!(out.len(), bytes.len());
        let b2 = AchievementBlock::parse(&out).unwrap();
        assert_eq!(b2.counters(), c);
        // u32 truncated like the C code.
        assert_eq!(b2.stats[3], 5);
        assert_eq!(b2.stats[2], 9);
        // Unowned bytes (v0.9.8 base, rest of OTY3) unchanged.
        assert_eq!(&b2.trailing[..OTY_BASE], &b.trailing[..OTY_BASE]);
        let o3 = OTY_BASE + 6 + 32 + 6;
        assert!(b2.trailing[o3 + 4..o3 + 16].iter().all(|&x| x == 0x55));
        assert!(b2.trailing[o3 + 20..o3 + 60].iter().all(|&x| x == 0x55));
        assert!(b2.trailing[o3 + 64..].iter().all(|&x| x == 0x55));
    }

    #[test]
    fn counters_created_when_blocks_absent() {
        let mut b = AchievementBlock::default();
        b.set_counters(&LifetimeCounters {
            stats: [1, 2, 3, 4, 5, 6, 7],
            screenshots_taken: 8,
            bullet_time_uses: 9,
            replays_saved: 3,
            ..Default::default()
        });
        let b2 = AchievementBlock::parse(&b.write()).unwrap();
        assert_eq!(
            b2.counters(),
            LifetimeCounters {
                stats: [1, 2, 3, 4, 5, 6, 7],
                screenshots_taken: 8,
                bullet_time_uses: 9,
                replays_saved: 3,
                ..Default::default()
            }
        );
        // OTY3 written at its full size (10 u32 + 5 u64 = 80 bytes).
        assert_eq!(b2.trailing.len(), OTY_BASE + 6 + 32 + 6 + 80);
    }
}
