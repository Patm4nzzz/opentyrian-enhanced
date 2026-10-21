//! Endless mode: deterministic part of `src/endless.c` (reference: `master`,
//! see ADR 0070).
//!
//! Ported here:
//! - the pool of playable sections of an episode (`build_level_pool`);
//! - the rotation in blocks of five levels per episode (`build_rotation`,
//!   `fill_block_queue`, `advance_block`, `pick_next_level`), drawn from the
//!   gameplay generator `rng.c` ([`Xoshiro256`]);
//! - the armor multiplier `1.05f^levels` (`endless_math.c`) and its
//!   application to the armor of a new enemy (`enemy_spawn.c:JE_makeEnemy`);
//! - lives (`endless_try_respawn`), without mutators or anomaly cards
//!   (outside the simulation, divergence no. 6).
//!
//! The leaderboard, resume (ENDR), HUD and screens stay on the Godot side.
//! Parity checked by `tests/endless_golden.rs` against
//! `rewrite/golden/endless/*.egm` (recorded by `c-final:utils/golden/record_endless.sh`).

use crate::fmath::{round_rational, F32};
use crate::rng::Xoshiro256;
use crate::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};
use tyrian_formats::crypt::decrypt;

/// `endless.c:ENDLESS_BLOCK_SIZE`.
pub const BLOCK_SIZE: usize = 5;
/// `episodes.h:EPISODE_MAX`.
pub const EPISODE_MAX: usize = 5;
/// `endless.c:ENDLESS_START_LIVES`.
pub const START_LIVES: i32 = 3;
/// Capacity of the pool in `fill_block_queue` (`int pool[64]`).
const POOL_CAP: usize = 64;

/// `endless.c:build_level_pool`: sections of `levels<ep>.dat` that carry a
/// "]L" without a bonus/secret marker ("$" or "*" in columns 27-28), excluding
/// mini-games ("]e", "]g") and "Timed Battle" arenas / "special sh[ip]"
/// switches. `data` is the raw file (encrypted Pascal strings).
pub fn build_level_pool(data: &[u8]) -> Vec<i32> {
    let mut pool = Vec::new();
    let (mut section, mut pos) = (0i32, 0usize);
    let mut awaiting_l = false;
    let mut mini_game = false;
    let mut header: Vec<u8> = Vec::new();
    while pool.len() < POOL_CAP {
        let Some(&len) = data.get(pos) else { break };
        let len = len as usize;
        pos += 1;
        if pos + len > data.len() {
            break;
        }
        let s = decrypt(&data[pos..pos + len]);
        pos += len;
        if len > 0 && s[0] == b'*' {
            section += 1;
            awaiting_l = true;
            mini_game = false;
            header = s;
        } else if awaiting_l && len > 1 && s[0] == b']' && (s[1] == b'e' || s[1] == b'g') {
            mini_game = true;
        } else if awaiting_l && len > 1 && s[0] == b']' && s[1] == b'L' {
            let c27 = s.get(27).copied().unwrap_or(b' ');
            let c28 = s.get(28).copied().unwrap_or(b' ');
            let bonus = [c27, c28].iter().any(|&c| c == b'$' || c == b'*');
            // strstr on a C string: stops at the first NUL.
            let h = header.split(|&b| b == 0).next().unwrap_or(&[]);
            let not_a_level = contains(h, b"Timed Battle") || contains(h, b"special sh");
            if !bonus && !not_a_level && !mini_game {
                pool.push(section);
            }
            awaiting_l = false;
        }
    }
    pool
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Rotation state of an Endless run (`endless.c`, `s_rot_*` variables,
/// `s_block_queue*`).
#[derive(Clone, Debug)]
pub struct Rotation {
    /// Pool per episode (index 0 = episode 1); empty = episode absent.
    pools: [Vec<i32>; EPISODE_MAX],
    episodes: Vec<usize>,
    idx: usize,
    queue: Vec<i32>,
    queue_pos: usize,
}

impl Rotation {
    /// `build_rotation`: available episodes in order 1..N
    /// (`episodeAvail`, i.e. `tyrian<N>.lvl` present). `pools[ep-1]` is
    /// `None` if the episode is absent.
    pub fn new(pools: [Option<Vec<i32>>; EPISODE_MAX]) -> Self {
        let episodes = (0..EPISODE_MAX).filter(|&i| pools[i].is_some()).map(|i| i + 1).collect();
        let pools = pools.map(|p| p.unwrap_or_default());
        Self { pools, episodes, idx: 0, queue: Vec::new(), queue_pos: 0 }
    }

    /// `fill_block_queue`: partial Fisher-Yates, up to five sections.
    ///
    /// The C code goes through `JE_initEpisode` and exits if `lvlNum < 1`; with the
    /// original data `lvlNum` is always at least 1, so the condition
    /// is not reproduced.
    pub fn fill_block_queue(&mut self, rng: &mut Xoshiro256) {
        self.queue.clear();
        self.queue_pos = 0;
        if self.episodes.is_empty() {
            return;
        }
        let mut pool = self.pools[self.episodes[self.idx] - 1].clone();
        let n = pool.len();
        if n == 0 {
            return;
        }
        let take = n.min(BLOCK_SIZE);
        for i in 0..take {
            let j = i + (rng.next_u32() % (n - i) as u32) as usize;
            pool.swap(i, j);
            self.queue.push(pool[i]);
        }
    }

    /// `advance_block`.
    fn advance_block(&mut self, rng: &mut Xoshiro256) {
        self.idx += 1;
        if self.idx >= self.episodes.len() {
            self.idx = 0;
        }
        self.fill_block_queue(rng);
    }

    /// `pick_next_level`: next (episode, section), `None` if no
    /// episode has a playable level (the C code then abandons the run).
    /// Like the C code, assumes `fill_block_queue` was called at startup
    /// (`endless_start`).
    pub fn pick_next_level(&mut self, rng: &mut Xoshiro256) -> Option<(usize, i32)> {
        if self.episodes.is_empty() {
            return None;
        }
        let mut guard = self.episodes.len() as i32 + 1;
        while self.queue_pos >= self.queue.len() {
            guard -= 1;
            if guard <= 0 {
                return None;
            }
            self.advance_block(rng);
        }
        let lvl = self.queue[self.queue_pos];
        self.queue_pos += 1;
        Some((self.episodes[self.idx], lvl))
    }
}

/// `1.05f` (0x3F866666).
const MULT_STEP: F32 = F32 { neg: false, mant: 0x866666, exp: -23 };

/// `endless_math.c:endless_armor_mult_for_levels`: `powf(1.05f, n)`, 1 if
/// n ≤ 0. Computed as the correct rounding of the exact power (value
/// tracked on 64 mantissa bits, the truncation error staying well
/// below half a float ULP); equality with glibc's `powf` checked bit by
/// bit for n = 0..130 by the golden master.
pub fn armor_mult_for_levels(levels_completed: i32) -> F32 {
    if levels_completed <= 0 {
        return F32 { neg: false, mant: 1 << 23, exp: -23 };
    }
    // v = mant * 2^exp, mant normalized to 64 bits.
    let (mut mant, mut exp) = (MULT_STEP.mant << 40, MULT_STEP.exp - 40);
    for _ in 1..levels_completed {
        let p = mant as u128 * MULT_STEP.mant as u128;
        let shift = 128 - p.leading_zeros() as i32 - 64;
        mant = (p >> shift) as u64;
        exp += MULT_STEP.exp + shift;
    }
    // Rounding to 24 bits (to nearest; an exact tie is impossible
    // here because the exact value has far more than 64 significant bits).
    let r = (mant >> 40) + ((mant >> 39) & 1);
    let (num, den) = if exp + 40 >= 0 { ((r as i128) << (exp + 40), 1) } else { (r as i128, 1i128 << -(exp + 40)) };
    round_rational(num, den)
}

/// IEEE 754 representation of a positive normal float (golden check).
pub fn f32_bits(v: F32) -> u32 {
    (((v.exp + 23 + 127) as u32) << 23) | (v.mant as u32 & 0x7f_ffff)
}

/// `enemy_spawn.c:JE_makeEnemy`, Endless block: `(int)((float)armor * mult)`
/// clamped to 1..254, applied only if `armor < 255` (the C code tests
/// `tempArmor < 255`, the armor read before difficulty scaling).
pub fn scale_armor(armor: u8, mult: F32) -> u8 {
    mult.mul_int(armor as i64).floorf().clamp(1, 254) as u8
}

/// `endless.c:endless_try_respawn` (without the "one life" mutator or the
/// Glass Hull card): consumes a life and returns true, or sets the counter to 0.
pub fn try_respawn(lives: &mut i32) -> bool {
    if *lives > 1 {
        *lives -= 1;
        true
    } else {
        *lives = 0;
        false
    }
}

/// `endless.c:on_enemy_killed`: `value × max(combo_multiplier, 1)`, a
/// value ≤ 0 counts as 50; saturating addition ("Bug #10", `ULONG_MAX`).
pub fn score_delta(value: i32, combo_multiplier: u32) -> u64 {
    let mult = combo_multiplier.max(1) as u64;
    let value = if value <= 0 { 50 } else { value as u64 };
    value * mult
}

impl crate::world::World {
    /// Endless listener of `EVENT_ENEMY_KILLED` (`endless.c:on_enemy_killed`);
    /// no effect outside Endless (`if (!endlessMode) return`).
    pub fn endless_on_enemy_killed(&mut self, value: i32) {
        // Order of the EVENT_ENEMY_KILLED listeners: combos.c (opentyr.c,
        // registered at startup) before endless.c (endless_start).
        self.combo_on_kill();
        if self.endless_lives.is_some() {
            self.endless_score = self.endless_score.saturating_add(score_delta(value, self.combo_multiplier));
        }
    }
}

/// Equipment of a one-player Endless run (`endless.c:endless_start`):
/// Stalker (8), Pulse-Cannon (1) power 5, no rear weapon (power
/// 1), shield 5, generator 4, special 1, no money.
pub fn start_loadout() -> crate::Loadout {
    crate::Loadout {
        ship: ShipId(8),
        front_weapon: WeaponId(1),
        front_power: 5,
        rear_weapon: WeaponId(0),
        rear_power: 1,
        shield: ShieldId(5),
        generator: GeneratorId(4),
        left_sidekick: SidekickId(0),
        right_sidekick: SidekickId(0),
        special: SpecialId(1),
        cash: 0,
    }
}

/// Equipment of a two-player Endless run (`endless.c:endless_start(true)`):
/// player 1 on ship 11 (`ship_for[0] = two_player ? 11 : 8`), player 2
/// (Dragonwing) on ship 8 with the Vulcan Cannon (15) power 5 at the
/// rear; the rest as [`start_loadout`].
pub fn start_loadouts_two_player() -> [crate::Loadout; 2] {
    let p1 = crate::Loadout { ship: ShipId(11), ..start_loadout() };
    let p2 = crate::Loadout { ship: ShipId(8), rear_weapon: WeaponId(15), rear_power: 5, ..start_loadout() };
    [p1, p2]
}

/// Mode of an Endless run (`endless.c:endless_start`): `twoPlayerMode =
/// two_player`, `onePlayerAction = !two_player`, `ENDLESS_START_LIVES` lives
/// for both players; with two, `items2` = player 2 equipment.
pub fn mode_setup(two_player: bool, lives: [i32; 2]) -> crate::modes::ModeSetup {
    crate::modes::ModeSetup {
        two_player,
        one_player_action: !two_player,
        // One player: "all devices" (0), for lack of a setting in the interface;
        // two players: keyboard and mouse (config.c:inputDevice).
        devices: if two_player { (1, 2) } else { (0, 0) },
        initial_difficulty: START_DIFFICULTY,
        loadout2: if two_player { Some(start_loadouts_two_player()[1]) } else { None },
        endless_lives: Some(lives),
        ..Default::default()
    }
}

/// `endless.c:s_start_difficulty`: Hard (3) by default.
pub const START_DIFFICULTY: u8 = 3;

/// Level chosen by the rotation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndlessLevel {
    pub episode: u8,
    /// Section de `levels<ep>.dat` (`mainLevel`).
    pub section: u8,
    /// Level of `tyrian<ep>.lvl` designated by the first "]L" of the section
    /// (`level_load.c:JE_loadMap`, `case 'L'`, `atoi(s + 25)`).
    pub lvl_file_num: usize,
    /// `levelName` (`s[13..22]` of the "]L"): "ENTERING %s" banner.
    pub level_name: String,
}

/// One-player Endless run: rotation, completed waves, lives.
/// The screens (name, summary, leaderboard) stay on the Godot side.
#[derive(Clone)]
pub struct EndlessRun {
    rng: Xoshiro256,
    rotation: Rotation,
    /// `lvl_file_num` per (episode, section).
    lvl_files: [Vec<(i32, usize, String)>; EPISODE_MAX],
    /// `s_levels_completed`.
    pub levels_completed: i32,
    /// `s_lives_p[0]`.
    pub lives: i32,
    pub difficulty: u8,
    /// `initialDifficulty` (starting choice, fixed: score multiplier of the ramp).
    pub initial_difficulty: u8,
    /// `s_score` of the completed waves (the Sim of the current wave holds the rest).
    pub score: u64,
}

impl EndlessRun {
    /// `endless_start` (one-player run): `JE_scanForEpisodes`,
    /// `rng_seed(seed)` (the C code takes `time(NULL)`), `build_rotation`,
    /// `fill_block_queue`.
    pub fn start(data_dir: &std::path::Path, seed: u64, difficulty: u8) -> Self {
        use tyrian_formats::episode::EpisodeScript;
        let mut lvl_files: [Vec<(i32, usize, String)>; EPISODE_MAX] = Default::default();
        let pools: [Option<Vec<i32>>; EPISODE_MAX] = std::array::from_fn(|i| {
            let ep = i + 1;
            if !data_dir.join(format!("tyrian{ep}.lvl")).exists() {
                return None;
            }
            let bytes = std::fs::read(data_dir.join(format!("levels{ep}.dat"))).unwrap_or_default();
            if let Ok(script) = EpisodeScript::parse(&bytes) {
                for sec in script.sections() {
                    if let Some(l) = sec.levels.first() {
                        // Name as levels*.dat gives it (trailing spaces included):
                        // the C code displays it that way (levelName, wave entry banner).
                        lvl_files[i].push((sec.number as i32, l.lvl_file_num.max(1) as usize, l.level_name.clone()));
                    }
                }
            }
            Some(build_level_pool(&bytes))
        });
        let mut rng = Xoshiro256::new(seed);
        let mut rotation = Rotation::new(pools);
        rotation.fill_block_queue(&mut rng);
        Self {
            rng,
            rotation,
            lvl_files,
            levels_completed: 0,
            lives: START_LIVES,
            difficulty: difficulty.clamp(1, 9),
            initial_difficulty: difficulty.clamp(1, 9),
            score: 0,
        }
    }

    /// `pick_next_level` + section → file level mapping.
    pub fn next_level(&mut self) -> Option<EndlessLevel> {
        let (ep, section) = self.rotation.pick_next_level(&mut self.rng)?;
        let (_, lvl_file_num, level_name) = self.lvl_files[ep - 1].iter().find(|(s, _, _)| *s == section).cloned()?;
        Some(EndlessLevel { episode: ep as u8, section: section as u8, lvl_file_num, level_name })
    }

    /// `endless_wave`: current wave (1-based).
    pub fn wave(&self) -> i32 {
        self.levels_completed + 1
    }

    /// Armor multiplier of the enemies of the current wave.
    pub fn armor_mult(&self) -> F32 {
        armor_mult_for_levels(self.levels_completed)
    }

    /// Successful level end (`endless_on_level_end`, "next
    /// wave" branch): `s_levels_completed++`.
    pub fn level_completed(&mut self) {
        self.levels_completed += 1;
    }

    /// `mainint.c:adjust_difficulty`, called by `JE_endLevelAni` at each completed wave in
    /// Endless: difficulty rises with the total score (money + equipment value; with
    /// two players, sum of the two purses), never goes down.
    pub fn adjust_difficulty(&mut self, total_score: u64, two_player: bool) {
        self.difficulty =
            crate::campaign::adjust_difficulty_mode(self.difficulty, self.initial_difficulty, total_score, two_player);
    }

    /// `endless.c:lives_exhausted` (one player): no lives left.
    pub fn lives_exhausted(&self) -> bool {
        self.lives <= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mult_zero_is_one() {
        let m = armor_mult_for_levels(0);
        assert_eq!(scale_armor(100, m), 100);
    }

    /// ENH-02: difficulty rises with the score, never going down.
    #[test]
    fn endless_difficulty_ramps_with_score() {
        let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        let mut run = EndlessRun::start(&data, 1, START_DIFFICULTY);
        assert_eq!(run.difficulty, 3);
        run.adjust_difficulty(1000, false);
        assert_eq!(run.difficulty, 3, "low score: no change");
        run.adjust_difficulty(200_000, false);
        assert!(run.difficulty > 3, "200,000 points ×1.3: Impossible or higher, got {}", run.difficulty);
        let d = run.difficulty;
        run.adjust_difficulty(0, false);
        assert_eq!(run.difficulty, d, "never decreases");
    }

    #[test]
    fn respawn_counts_down() {
        let mut l = START_LIVES;
        for _ in 0..2 {
            assert!(try_respawn(&mut l));
        }
        assert!(!try_respawn(&mut l));
        assert_eq!(l, 0);
    }

    #[test]
    fn run_picks_episode1_first() {
        let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
        let mut run = EndlessRun::start(&data, 1, START_DIFFICULTY);
        // Golden master seed 1: "pick 0 1 28".
        let l = run.next_level().unwrap();
        assert_eq!((l.episode, l.section), (1, 28));
        assert!(l.lvl_file_num >= 1);
    }
}
