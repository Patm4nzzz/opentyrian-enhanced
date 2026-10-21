//! Side modes (block 5): starting parameters and mode-specific state,
//! hashed like `golden_master.c:hash_mode` (golden master format v2).
//!
//! The corresponding C variables are globals (`varz.c`, `tyrian2.c`,
//! `player_movement.c`); here they live in [`ModeState`], carried by `World`.

use crate::Loadout;

/// "mode" + "items2" line of a v2 golden master (`golden_master.c:golden_master_level_start`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModeSetup {
    pub two_player: bool,
    pub galaga: bool,
    pub one_player_action: bool,
    /// `superArcadeMode` (0 = disabled, otherwise Super Arcade ship index).
    pub super_arcade: u8,
    pub super_tyrian: bool,
    /// `timeBattleSelection` if `timedBattleMode`, otherwise 0.
    pub timed: u8,
    pub mutators: u8,
    pub anomalies: u16,
    /// `inputDevice[0..2]` (1 keyboard, 2 mouse, 3+ gamepads).
    pub devices: (u8, u8),
    pub cash: (u64, u64),
    pub initial_difficulty: u8,
    /// Player 2 equipment (`items2`).
    pub loadout2: Option<Loadout>,
    /// Endless mode: starting lives per player (`endless.c:s_lives_p`),
    /// consumed by `endless.c:endless_try_respawn` instead of
    /// `*player[p].lives` (alias of the weapon power). `None` outside Endless.
    pub endless_lives: Option<[i32; 2]>,
}

/// State of the side modes during the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeState {
    pub galaga_mode: bool,
    pub super_arcade_mode: u8,
    /// `superArcadePowerUp` (tyrian2.c:JE_main resets it to 1 at start).
    pub super_arcade_power_up: u8,
    pub two_player_linked: bool,
    /// IEEE-754 bits of the `float linkGunDirec` (tyrian2.c:JE_main: `M_PI`).
    pub link_gun_direc: u32,
    pub timed: u8,
    /// `galagaLife` (tyrian2.c:JE_main: 10000).
    pub galaga_life: u32,
    pub super_tyrian: bool,
    /// Mask of active mutators (`MUT_*`).
    pub mutators: u8,
    /// Mask of active anomalies (`ANOM_*`).
    pub anomalies: u16,
    /// Charged cannon of player 2 (tyrian2.c:JE_main: chargeWait 5, chargeLevel 0, chargeMax 5).
    pub charge_wait: u8,
    pub charge_level: u8,
    pub charge_max: u8,
    /// `inputDevice[0..2]` (0 all, 1 keyboard, 2 mouse, 3+ gamepad).
    pub devices: [u8; 2],
    /// Mouse of the tick: `mouse_pressed[]` (keyboard mask bits: 0x10 fire, 0x20
    /// fire mode change, 0x40/0x80 sidekicks) and relative movement.
    pub mouse: MouseInput,
    /// `golden_master.c:golden_master_mouse` / `keyboard.c:mouseGetRelativePosition`:
    /// the movement is read only once per tick (accumulator reset to zero).
    pub mouse_pending: bool,
    /// Analog stick of each player for the tick, already reduced like
    /// `joystick.c:joystick_axis_reduce` (threshold then division by the
    /// sensitivity): added to `mouseXC`/`mouseYC` as in
    /// `player_movement.c` when `joystick[j].analog` is true. (0, 0) without a
    /// gamepad, hence no effect on the golden masters.
    pub analog: [(i32, i32); 2],
    /// `newkey` (keyboard.c): a key has been pressed since the previous
    /// tick. Read only by the "Game Over" screen (tyrian2.c);
    /// reset to false after each tick.
    pub new_key: bool,
}

/// Mouse input of a tick (`keyboard.c`: `mouse_pressed[]`, `mouseWindow*Relative`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MouseInput {
    pub mask: u8,
    pub dx: i32,
    pub dy: i32,
}

/// `SA` (varz.h): number of Super Arcade ships.
pub const SA: u8 = 9;

/// `varz.c:SAWeapon[SA][5]` (red, blue, black, green, purple).
pub const SA_WEAPON: [[u8; 5]; SA as usize] = [
    [9, 31, 32, 33, 34],
    [19, 8, 22, 41, 34],
    [27, 5, 20, 42, 31],
    [15, 3, 28, 22, 12],
    [23, 35, 25, 14, 6],
    [2, 5, 21, 4, 7],
    [40, 38, 37, 41, 36],
    [47, 45, 19, 33, 19],
    [44, 26, 46, 26, 1],
];

/// `varz.c:SASpecialWeapon[SA]` and `SASpecialWeaponB[SA]`.
pub const SA_SPECIAL_WEAPON: [u8; SA as usize] = [7, 8, 9, 10, 11, 12, 13, 48, 47];
pub const SA_SPECIAL_WEAPON_B: [u8; SA as usize] = [37, 6, 15, 40, 16, 14, 41, 48, 47];

/// `varz.c:chargeGunWeapons[PORT_NUM]` (indexed by rear weapon - 1; missing entries = 0).
pub const CHARGE_GUN_WEAPONS: [u16; 38] = [
    0, 0, 0, 0, 0, 0, 0, 0, 476, 458, 464, 482, 0, 488, 470, 0, 0, 0, 0, 0, 494, 500, 0, 528, 0, 558, 458, 458, 458,
    458, 458, 458, 458, 458, 458, 458, 458, 458,
];

/// Bits of `ModeSetup::mutators` (order of replay.c, `golden_master.c:apply_mode`).
pub const MUT_FAST_ENEMIES: u8 = 0x01;
pub const MUT_ARMORED: u8 = 0x02;
pub const MUT_BULLET_HELL: u8 = 0x04;
pub const MUT_GLASS_CANNON: u8 = 0x08;
pub const MUT_NO_POWERUPS: u8 = 0x10;
pub const MUT_ONE_LIFE: u8 = 0x20;
pub const MUT_MIRROR: u8 = 0x40;
pub const MUT_RISK_REWARD: u8 = 0x80;

/// `mutators.c:mutators_scale_speed`: x1.5 rounded to nearest (halves
/// away from zero), clamped to ±127.
pub fn scale_speed(v: i32) -> i32 {
    let r = if v >= 0 { (v * 3 + 1) / 2 } else { -(((-v) * 3 + 1) / 2) };
    r.clamp(-127, 127)
}

/// Bits of `ModeSetup::anomalies` (order of AnomalyEffect starting from GLASS_HULL,
/// `golden_master.c:apply_mode`). eyes_closed and combo_lock do not touch the simulation
/// (HUD, combos disabled by the golden master).
pub const ANOM_GLASS_HULL: u16 = 0x001;
pub const ANOM_BULLET_STORM: u16 = 0x002;
pub const ANOM_TWIN_CANNONS: u16 = 0x004;
pub const ANOM_HARDENED: u16 = 0x008;
pub const ANOM_SLOW_HANDS: u16 = 0x010;
/// `g_anomaly_combo_lock`: doubled combo window (`combos.c:combo_update`).
pub const ANOM_COMBO_LOCK: u16 = 0x040;
pub const ANOM_FROST_HANDS: u16 = 0x080;
pub const ANOM_HEAVY_IRON: u16 = 0x100;

/// `(float)M_PI`.
pub const PI_F32_BITS: u32 = 0x4049_0fdb;

impl ModeState {
    pub fn mutator(&self, bit: u8) -> bool {
        self.mutators & bit != 0
    }

    pub fn anomaly(&self, bit: u16) -> bool {
        self.anomalies & bit != 0
    }
}

impl Default for ModeState {
    fn default() -> Self {
        Self {
            galaga_mode: false,
            super_arcade_mode: 0,
            super_arcade_power_up: 1,
            two_player_linked: false,
            link_gun_direc: PI_F32_BITS,
            timed: 0,
            galaga_life: 10000,
            super_tyrian: false,
            mutators: 0,
            anomalies: 0,
            charge_wait: 5,
            charge_level: 0,
            charge_max: 5,
            devices: [1, 2],
            mouse: MouseInput::default(),
            mouse_pending: false,
            analog: [(0, 0); 2],
            new_key: false,
        }
    }
}

/// Extended fields of a v2 tick line (`golden_master.c:write_tick`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModeRecord {
    pub mouse_mask: u8,
    pub mouse_dx: i32,
    pub mouse_dy: i32,
    pub p2x: i32,
    pub p2y: i32,
    pub h_mode: u64,
}
