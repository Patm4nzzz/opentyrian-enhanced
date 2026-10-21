//! Robustness bot for the `tyrian-sim` core.
//!
//! Each game is entirely determined by a `u64` identifier: level,
//! difficulty, equipment, simulation seed, assists and bot inputs all
//! derive from it through SplitMix64 (a generator specific to the bot, independent of those of the
//! game). A failing game can therefore be replayed identically with
//! `tyrian-robustness --only <id>`.
//!
//! Defects detected:
//! - panic during `Sim::new` or `Sim::step` (out-of-bounds access, arithmetic
//!   overflow if the profile enables `overflow-checks`, `unwrap`...);
//! - non-reproducibility: the same game, replayed in another pass, on
//!   another thread and in another order, does not give the same trace
//!   of fingerprints (`frame_record` at each tick, `state_hash` every
//!   [`STATE_HASH_EVERY`] ticks and at the end);
//! - API inconsistencies: variable snapshot size, tick counter that
//!   does not advance by exactly 1 per `step`.
//!
//! The bot never modifies the simulation: it only uses the public API
//! (`Sim::new`, `Sim::step`, `state`, `frame_record`, `state_hash`, `snapshot`).

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

use serde::{Deserialize, Serialize};
use tyrian_formats::hdt::{HdtFile, ItemTables};
use tyrian_formats::lvl::LvlFile;
use tyrian_sim::{LevelSetup, Loadout, PlayerInput, Sim};

/// Frequency of the full fingerprint (`state_hash` serializes ~18 KiB).
pub const STATE_HASH_EVERY: u64 = 32;

/// SplitMix64: the bot's generator (not the game's).
#[derive(Clone)]
pub struct BotRng(u64);

impl BotRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform integer in `lo..=hi` (negligible bias for small ranges).
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u64() % (hi - lo + 1)
    }

    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next_u64() % den < num
    }
}

/// Playable levels, loaded only once (parsing `tyrian.hdt` for each
/// game would cost more than the game itself).
pub struct Catalogue {
    /// (episode, 1-based level number, base setup).
    pub levels: Vec<(u8, usize, LevelSetup)>,
}

impl Catalogue {
    /// Reads `tyrian1.lvl` to `tyrian5.lvl` and the object tables
    /// (`setup.rs:load_level`: `tyrian.hdt` for 1-3, final block for 4-5).
    pub fn load(data_dir: &Path, episodes: &[u8]) -> anyhow::Result<Self> {
        let hdt_items: ItemTables = HdtFile::parse(&std::fs::read(data_dir.join("tyrian.hdt"))?)?.items;
        let mut levels = Vec::new();
        for &ep in episodes {
            let lvl = LvlFile::parse(&std::fs::read(data_dir.join(format!("tyrian{ep}.lvl")))?)?;
            let items = match lvl.item_tables() {
                Some(t) => t?,
                None => hdt_items.clone(),
            };
            for (i, level) in lvl.levels.iter().enumerate() {
                levels.push((
                    ep,
                    i + 1,
                    LevelSetup {
                        episode: ep,
                        section: 0,
                        difficulty: 2,
                        level: level.clone(),
                        items: items.clone(),
                        loadout: default_loadout(),
                        seed: 0,
                        assist: false,
                        initial_difficulty: None,
                        second_player_loadout: None,
                        carry_over: None,
                        modes: Default::default(),
                        endless_armor_mult: None,
                        patch: Default::default(),
                        processor_type: 4,
                    },
                ));
            }
        }
        anyhow::ensure!(!levels.is_empty(), "no level found in {}", data_dir.display());
        Ok(Self { levels })
    }
}

/// `mainint.c:JE_initPlayerData` (equipment of a new game).
pub fn default_loadout() -> Loadout {
    Loadout {
        ship: ShipId(1),
        front_weapon: WeaponId(1),
        front_power: 1,
        rear_weapon: WeaponId(0),
        rear_power: 1,
        shield: ShieldId(4),
        generator: GeneratorId(2),
        left_sidekick: SidekickId(0),
        right_sidekick: SidekickId(0),
        special: SpecialId(0),
        cash: 0,
    }
}

/// Parameters of a game drawn from its identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameSpec {
    pub id: u64,
    pub episode: u8,
    pub lvl_file: usize,
    pub difficulty: u8,
    pub seed: u64,
    pub assist: bool,
    /// (ship, front, power, rear, power, shield, generator,
    /// left sidekick, right sidekick, special).
    pub loadout: [u8; 10],
}

impl GameSpec {
    /// Equipment is drawn from the tables actually present in
    /// the data (number of `ItemTables` records), power 1..=11
    /// (`shop.c`: 11 power levels). One game in four keeps the game's
    /// starting equipment.
    ///
    /// `wild`: each equipment field is an arbitrary byte (0..=255),
    /// including outside the tables; checks that the core does not panic on a
    /// corrupted save or mod.
    pub fn from_id(id: u64, cat: &Catalogue, wild: bool) -> (Self, BotRng) {
        let mut r = BotRng::new(id ^ 0x5459_5249_414e_2121);
        let (episode, lvl_file, base) = &cat.levels[(r.next_u64() % cat.levels.len() as u64) as usize];
        let it = &base.items;
        let pick = |r: &mut BotRng, n: usize, min: u64| -> u8 { r.range(min, (n.max(1) - 1).min(255) as u64) as u8 };
        let loadout = if wild {
            let mut l = [0u8; 10];
            l.iter_mut().for_each(|v| *v = r.next_u64() as u8);
            l
        } else if r.chance(1, 4) {
            let d = default_loadout();
            [
                d.ship.0,
                d.front_weapon.0,
                d.front_power,
                d.rear_weapon.0,
                d.rear_power,
                d.shield.0,
                d.generator.0,
                d.left_sidekick.0,
                d.right_sidekick.0,
                d.special.0,
            ]
        } else {
            [
                pick(&mut r, it.ships.len(), 1),
                pick(&mut r, it.weapon_ports.len(), 1),
                r.range(1, 11) as u8,
                pick(&mut r, it.weapon_ports.len(), 0),
                r.range(1, 11) as u8,
                pick(&mut r, it.shields.len(), 1),
                pick(&mut r, it.generators.len(), 1),
                pick(&mut r, it.sidekicks.len(), 0),
                pick(&mut r, it.sidekicks.len(), 0),
                pick(&mut r, it.specials.len(), 0),
            ]
        };
        let spec = Self {
            id,
            episode: *episode,
            lvl_file: *lvl_file,
            // config.h: DIFFICULTY_EASY (1) .. DIFFICULTY_10 (10).
            difficulty: r.range(1, 10) as u8,
            seed: r.next_u64(),
            assist: r.chance(1, 2),
            loadout,
        };
        (spec, r)
    }

    pub fn setup(&self, cat: &Catalogue) -> LevelSetup {
        let base = &cat
            .levels
            .iter()
            .find(|(e, l, _)| *e == self.episode && *l == self.lvl_file)
            .expect("level from the catalogue")
            .2;
        let l = self.loadout;
        let mut s = base.clone();
        s.difficulty = self.difficulty;
        s.seed = self.seed;
        s.assist = self.assist;
        s.loadout = Loadout {
            ship: ShipId(l[0]),
            front_weapon: WeaponId(l[1]),
            front_power: l[2],
            rear_weapon: WeaponId(l[3]),
            rear_power: l[4],
            shield: ShieldId(l[5]),
            generator: GeneratorId(l[6]),
            left_sidekick: SidekickId(l[7]),
            right_sidekick: SidekickId(l[8]),
            special: SpecialId(l[9]),
            cash: 0,
        };
        s
    }
}

/// Bot inputs: segments of random duration (1..=40 ticks) during
/// which a mask stays held, like a player who keeps a direction.
pub struct BotInput {
    rng: BotRng,
    mask: u8,
    left: u32,
}

impl BotInput {
    pub fn new(rng: BotRng) -> Self {
        Self { rng, mask: 0, left: 0 }
    }

    #[allow(clippy::should_implement_trait)] // infinite stream with no end and no Option
    pub fn next(&mut self) -> PlayerInput {
        if self.left == 0 {
            let r = &mut self.rng;
            let mut m = (r.next_u64() & 0x0f) as u8; // directions (up/down/left/right)
            if r.chance(3, 4) {
                m |= PlayerInput::FIRE;
            }
            if r.chance(1, 50) {
                m |= PlayerInput::CHANGE_FIRE;
            }
            if r.chance(1, 10) {
                m |= PlayerInput::LEFT_SIDEKICK;
            }
            if r.chance(1, 10) {
                m |= PlayerInput::RIGHT_SIDEKICK;
            }
            self.mask = m;
            self.left = r.range(1, 40) as u32;
        }
        self.left -= 1;
        PlayerInput(self.mask)
    }
}

/// Result of a game.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameOutcome {
    pub spec: GameSpec,
    pub ticks: u64,
    pub level_ended: bool,
    /// Fingerprint of the whole trace (FNV-1a over frame_record + state_hash).
    pub trace_hash: u64,
    pub final_state_hash: u64,
    /// Panic message and the tick where it occurred.
    pub panic: Option<(u64, String)>,
    /// API inconsistency (snapshot size, tick counter).
    pub invariant: Option<(u64, String)>,
}

fn fnv(h: u64, v: u64) -> u64 {
    let mut h = h;
    for b in v.to_le_bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

thread_local! {
    /// Location (`file:line`) of the last panic of the current thread.
    static PANIC_LOCATION: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Replaces the default display of panics (thousands of lines otherwise)
/// with the recording of their location, used in the report.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        PANIC_LOCATION.with(|p| *p.borrow_mut() = loc);
    }));
}

fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    let msg = if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic without a message".to_string()
    };
    let loc = PANIC_LOCATION.with(|p| std::mem::take(&mut *p.borrow_mut()));
    if loc.is_empty() {
        msg
    } else {
        format!("{loc}: {msg}")
    }
}

/// Plays a game until the end of the level or `max_ticks`.
pub fn play(id: u64, cat: &Catalogue, max_ticks: u64, wild: bool) -> GameOutcome {
    let (spec, rng) = GameSpec::from_id(id, cat, wild);
    let setup = spec.setup(cat);
    let mut out = GameOutcome {
        spec,
        ticks: 0,
        level_ended: false,
        trace_hash: 0xcbf2_9ce4_8422_2325,
        final_state_hash: 0,
        panic: None,
        invariant: None,
    };
    let mut sim = match catch_unwind(AssertUnwindSafe(|| Sim::new(setup))) {
        Ok(s) => s,
        Err(e) => {
            out.panic = Some((0, format!("Sim::new: {}", panic_message(e.as_ref()))));
            return out;
        }
    };
    let snap_len = sim.snapshot().len();
    let mut input = BotInput::new(rng);
    let mut h = out.trace_hash;
    let mut tick = 0u64;
    let res = catch_unwind(AssertUnwindSafe(|| {
        while tick < max_ticks && !sim.state().level_ended {
            let before = sim.state().tick;
            // Player 2 also receives inputs: the solo game must ignore them.
            let p1 = input.next();
            let p2 = PlayerInput((p1.0.rotate_left(3)) ^ 0x10);
            sim.step([p1, p2]);
            tick += 1;
            if sim.state().tick != before + 1 {
                return Some((tick, format!("tick counter {} -> {}", before, sim.state().tick)));
            }
            let f = sim.frame_record();
            for v in [
                f.h_player,
                f.h_scroll,
                f.h_enemies,
                f.h_player_shots,
                f.h_enemy_shots,
                f.h_mt,
                f.h_rng,
                f.cur_loc as u64,
            ] {
                h = fnv(h, v);
            }
            if tick.is_multiple_of(STATE_HASH_EVERY) {
                let snap = sim.snapshot();
                if snap.len() != snap_len {
                    return Some((tick, format!("snapshot size {} -> {}", snap_len, snap.len())));
                }
                h = fnv(h, sim.state_hash());
            }
        }
        None
    }));
    out.ticks = tick;
    match res {
        Ok(inv) => {
            out.invariant = inv;
            out.level_ended = sim.state().level_ended;
            out.final_state_hash = sim.state_hash();
            out.trace_hash = fnv(h, out.final_state_hash);
        }
        Err(e) => out.panic = Some((tick, panic_message(e.as_ref()))),
    }
    out
}
