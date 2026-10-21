//! Shop pricing rules, as pure functions (no global state):
//! `mainint.c:weapon_upgrade_cost`, `mainint.c:JE_getCost`,
//! `mainint.c:JE_getValue`, `game_menu.c:JE_cashLeft`, and the
//! enter / exit accounting of the purchase submenu (`game_menu.c:JE_menuFunction`,
//! `shop_commit_done`).
//!
//! C model: when entering a category, the owned item is
//! "sold back" (`cash = cash * 2 - JE_cashLeft()`, i.e. `cash + price of
//! the owned item and of its power levels`); on confirmation,
//! the new item is paid for (`cash = JE_cashLeft()`). Selling back then buying again
//! the same item therefore costs nothing.

use crate::tables::Tables;
use crate::world::{Items, WeaponId, WeaponSlot};

/// Categories of `curSel[MENU_UPGRADES]` (game_menu.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopCategory {
    Ship = 2,
    FrontWeapon = 3,
    RearWeapon = 4,
    Shield = 5,
    Generator = 6,
    LeftSidekick = 7,
    RightSidekick = 8,
}

/// `mainint.c:weapon_upgrade_cost`: cumulative cost to go from power
/// 1 to `power + 1` (0 1 3 6 10 15...) × base price.
pub fn weapon_upgrade_cost(base_cost: i64, power: u32) -> i64 {
    let mut cost_sum: i64 = 0;
    let mut p = power;
    while p > 0 {
        cost_sum += p as i64;
        p -= 1;
    }
    base_cost * cost_sum
}

/// `mainint.c:JE_getCost`: base purchase price of an item (without the
/// power levels). Ships > 90 (ship editor) cost 100.
pub fn item_cost(t: &Tables, cat: ShopCategory, item: u16) -> i64 {
    let i = item as usize;
    match cat {
        ShopCategory::Ship => {
            if item > 90 {
                100
            } else {
                t.ships.get(i).map_or(0, |s| s.cost as i64)
            }
        }
        ShopCategory::FrontWeapon | ShopCategory::RearWeapon => t.ports.get(i).map_or(0, |p| p.cost as i64),
        ShopCategory::Shield => t.shields.get(i).map_or(0, |s| s.cost as i64),
        ShopCategory::Generator => t.generators.get(i).map_or(0, |g| g.cost as i64),
        ShopCategory::LeftSidekick | ShopCategory::RightSidekick => t.options.get(i).map_or(0, |o| o.cost as i64),
    }
}

/// `mainint.c:JE_getCost`, side effects `downgradeCost` / `upgradeCost`
/// for a weapon of power `power` (1..11): (downgrade cost,
/// cost of the next upgrade).
pub fn weapon_power_costs(t: &Tables, item: u16, power: u8) -> (i64, i64) {
    let cost = t.ports.get(item as usize).map_or(0, |p| p.cost as i64);
    let item_power = power.saturating_sub(1) as u32;
    (weapon_upgrade_cost(cost, item_power), weapon_upgrade_cost(cost, item_power + 1))
}

/// `mainint.c:JE_getValue`: value of the owned item (base price plus
/// all its power levels for a weapon).
pub fn item_value(t: &Tables, cat: ShopCategory, item: u16, power: u8) -> i64 {
    match cat {
        ShopCategory::Ship => t.ships.get(item as usize).map_or(0, |s| s.cost as i64),
        ShopCategory::FrontWeapon | ShopCategory::RearWeapon => {
            let base = t.ports.get(item as usize).map_or(0, |p| p.cost as i64);
            let mut v = base;
            for i in 1..power.saturating_sub(1) as u32 + 1 {
                v += weapon_upgrade_cost(base, i);
            }
            v
        }
        _ => item_cost(t, cat, item),
    }
}

/// `game_menu.c:JE_cashLeft`: money left if item `item` (at power
/// `power` for a weapon) replaces the current item. The result
/// is an `int32_t` in the C code.
///
/// The cumulative cost of the power levels is computed without overflow:
/// the C code accumulates it in a `uint16_t` (`tempW`) that wraps around as soon as
/// price × (1 + … + 10) exceeds 65,535, whereas the sell-back (`JE_getValue`)
/// counts in `long`. A mod weapon costing more than 1,191 credits was thus bought
/// for less than it sold back for (infinite money by buying / selling).
/// The original weapons cost at most 1,101: nothing changes for them
/// (docs/divergences.md).
pub fn cash_left(t: &Tables, cash: u64, cat: ShopCategory, item: u16, power: u8) -> i32 {
    let mut temp_l = cash as i32;
    temp_l = temp_l.wrapping_sub(item_cost(t, cat, item) as i32);
    if matches!(cat, ShopCategory::FrontWeapon | ShopCategory::RearWeapon) {
        let port_cost = t.ports.get(item as usize).map_or(0, |p| p.cost as i64);
        let mut temp_w: i64 = 0;
        for i in 1..power as i64 {
            temp_w += port_cost * i;
            temp_l = temp_l.wrapping_sub(temp_w as i32);
        }
    }
    temp_l
}

/// Entering the submenu of a category (`game_menu.c:JE_menuFunction`,
/// `player[0].cash = player[0].cash * 2 - JE_cashLeft()`): credit for
/// selling back the owned item.
pub fn enter_category(t: &Tables, cash: u64, cat: ShopCategory, owned: u16, owned_power: u8) -> u64 {
    let left = cash_left(t, cash, cat, owned, owned_power) as i64;
    (cash as i64 * 2 - left) as u64
}

/// Confirming a purchase (`game_menu.c:shop_commit_done`, `cash = JE_cashLeft()`).
/// Returns `None` if there is not enough money (the C code refuses the purchase: button
/// greyed out when `JE_getCost(...) > player[0].cash`).
pub fn buy(t: &Tables, cash_in_menu: u64, cat: ShopCategory, item: u16, power: u8) -> Option<u64> {
    let left = cash_left(t, cash_in_menu, cat, item, power);
    if left < 0 {
        None
    } else {
        Some(left as u64)
    }
}

/// Cap on the purchasable power (`game_menu.c`, computation of `rightPower`):
/// levels actually defined for the port; in a two-mode rear bay, the
/// minimum of the two modes; "None" in front can still be upgraded up to 11.
pub fn max_power_level(t: &Tables, cat: ShopCategory, item: u16) -> u8 {
    if item == 0 {
        return 11;
    }
    let mut max = t.port_level_count(item as usize, 0) as u8;
    if cat == ShopCategory::RearWeapon && t.ports.get(item as usize).map_or(0, |p| p.opnum) >= 2 {
        let alt = t.port_level_count(item as usize, 1) as u8;
        max = max.min(alt);
    }
    max.max(1)
}

/// Shop menus played by [`ShopSession`] (`shop.h`, `MENU_*` indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopMenu {
    /// `MENU_FULL_GAME`: Data Cubes, Ship Specs, Upgrade Ship, Options, Play Next Level, Quit.
    FullGame = 0,
    /// `MENU_UPGRADES`: the 7 categories then "Done".
    Upgrades = 1,
    /// `MENU_PLAY_NEXT_LEVEL`: destinations (`mapSection`).
    PlayNextLevel = 3,
    /// `MENU_UPGRADE_SUB`: items of a category then "Done".
    UpgradeSub = 4,
    /// `MENU_DATA_CUBES`: collected data cubes (`cubeMax`) then back.
    DataCubes = 7,
    /// `MENU_DATA_CUBE_SUB`: reading a cube (scrolling held by the screen).
    DataCubeSub = 8,
}

/// Keys handled by `game_menu.c:JE_itemScreen` (`newkey` branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopKey {
    Up,
    Down,
    Left,
    Right,
    /// Enter or Space.
    Select,
    Escape,
    /// `/`: rear weapon mode, within its category (`SDL_SCANCODE_SLASH`).
    Slash,
}

impl ShopKey {
    /// Letter of the `GOLDEN_SHOP` script (`golden_master.c:golden_master_shop_key`).
    pub fn from_script(c: char) -> Option<Self> {
        Some(match c {
            'U' => Self::Up,
            'D' => Self::Down,
            'L' => Self::Left,
            'R' => Self::Right,
            'E' => Self::Select,
            'X' => Self::Escape,
            _ => return None,
        })
    }
}

/// What the screen must do after a key (what `JE_itemScreen` does not
/// simulate itself: sub-screens, exit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopAction {
    None,
    /// `MENU_FULL_GAME` entry 3 (Ship Specs, `JE_doShipSpecs`) or 5
    /// (Options: save screen of the rewrite, `save_menu.gd`):
    /// side screen opened by the caller.
    FullGameEntry(u8),
    /// Entry on a cube of `MENU_DATA_CUBES`: reading cube `n`
    /// (0-based, `currentCube`). The screen resets its scrolling to zero.
    ReadCube(u8),
    /// `MENU_PLAY_NEXT_LEVEL`: chosen destination (index into `mapSection`).
    /// The C code then leaves the shop (`mainLevel = mapSection[i]`).
    Leave(usize),
    /// Escape on `MENU_FULL_GAME` (`menuEsc` = 0) or "Quit Game" entry:
    /// `JE_quitRequest` box, to be displayed by the caller.
    QuitRequest,
}

/// `game_menu.c:menuChoicesDefault` for `MENU_FULL_GAME` and `MENU_UPGRADES`.
const FULL_GAME_CHOICES: u8 = 7;
const UPGRADES_CHOICES: u8 = 9;
/// Number of `shop.h` menus covered (`MENU_DATA_CUBE_SUB` + 1).
const MENUS: usize = 9;
/// `game_menu.c:itemAvailMap`: `]I[` line (1-based) of each category
/// (ship, front, rear, shield, generator, left/right sidekick).
const ITEM_AVAIL_MAP: [usize; 7] = [1, 2, 3, 9, 4, 6, 7];

/// "Always available" items of the mods (`always_available`), per
/// category: `hdt_json.c:always_avail_*`. Injected when the shop opens
/// (`game_menu.c:JE_itemScreen`, `INJECT_AVAIL` macro).
#[derive(Debug, Clone, Default)]
pub struct AlwaysAvailable {
    pub ships: Vec<u8>,
    pub front_weapons: Vec<u8>,
    pub rear_weapons: Vec<u8>,
    pub sidekicks: Vec<u8>,
    pub shields: Vec<u8>,
}

/// Capacity of a shop list once the mods' items are injected (ADR 0134): the
/// level's own list (`itemAvail[9][10]`, at most 10) plus the mods'
/// `always_available` items (at most 16 per category, `tyrian-mods`
/// `MAX_ALWAYS_AVAIL`). The C version stopped at 10, which hid most of the
/// items of a second ship pack; the list scrolls (`UPGRADE_VISIBLE` rows).
const SHOP_LIST_CAPACITY: usize = 10 + 16;

/// `game_menu.c:playeritem_map`: item of category `cat` (0..7), by
/// its number in the category table (the shop's `]I[` lists
/// are numbers, shared by the seven categories).
pub fn item_slot(items: &mut Items, cat: usize) -> &mut u8 {
    match cat {
        0 => &mut items.ship.0,
        1 => &mut items.weapon[0].id.0,
        2 => &mut items.weapon[1].id.0,
        3 => &mut items.shield.0,
        4 => &mut items.generator.0,
        5 => &mut items.sidekick[0].0,
        _ => &mut items.sidekick[1].0,
    }
}

fn category(sel: u8) -> ShopCategory {
    match sel {
        2 => ShopCategory::Ship,
        3 => ShopCategory::FrontWeapon,
        4 => ShopCategory::RearWeapon,
        5 => ShopCategory::Shield,
        6 => ShopCategory::Generator,
        7 => ShopCategory::LeftSidekick,
        _ => ShopCategory::RightSidekick,
    }
}

/// Interactive shop: port of the keyboard state machine of
/// `game_menu.c:JE_itemScreen` / `JE_menuFunction` for the "Full
/// Game", "Upgrade Ship", categories and "Play Next Level" menus. Player 1
/// only (the C code only opens this shop in a one-player game).
///
/// C accounting reproduced as is: entering a category =
/// sell-back of the owned item (`cash * 2 - JE_cashLeft()`), the cursor
/// equips the hovered item live, Enter keeps it (`shop_choose_row`),
/// "Done" pays for it (`shop_commit_done`, `cash = JE_cashLeft()`), Escape
/// cancels (`old_items`, `cash = JE_cashLeft()`).
#[derive(Debug, Clone)]
pub struct ShopSession {
    pub menu: ShopMenu,
    /// `curSel[]`, indexed by `ShopMenu as usize` (2 = first row).
    pub cur_sel: [u8; MENUS],
    /// `menuChoices[]` (last selectable row).
    pub menu_choices: [u8; MENUS],
    /// `cubeMax`: data cubes readable in `MENU_DATA_CUBES` (0..4),
    /// set by the caller (level end + `]?` `]!` `]+` directives).
    pub cube_max: u8,
    /// `currentCube`: cube open in `MENU_DATA_CUBE_SUB` (0-based).
    pub current_cube: u8,
    pub items: Items,
    pub cash: u64,
    /// `itemAvail` / `itemAvailMax` after injection and sorting (`JE_itemScreen`).
    pub item_avail: [Vec<u8>; 9],
    /// Number of destinations of the "Play Next Level" menu (`mapPNum`).
    pub destinations: usize,
    chosen_items: Items,
    old_items: Items,
    /// Provisional power of each row of the open weapon list, then of Done.
    temp_weapon_power: [u8; SHOP_LIST_CAPACITY + 1],
    last_cur_sel: u8,
    last_direction: i8,
    /// `leftPower`, `rightPower`, `rightPowerAfford` (power arrows).
    pub left_power: bool,
    pub right_power: bool,
    pub right_power_afford: bool,
    /// `player[0].weapon_mode` (1 or 2): rear weapon mode shown by
    /// the fire demonstration. The C code keeps it for the next level; here the
    /// level restarts from mode 1 (`World::start_level`), as after any
    /// arrow in the rear weapon list.
    pub weapon_mode: u8,
    /// Confirmed purchases (`shop_commit_done`) not yet read by the caller:
    /// (credits spent, weapon port bought or 0). Feeds the Shopaholic,
    /// Weapon Collector and Full House achievements.
    pub purchases: Vec<(u64, u8)>,
    /// True if the last key or click paid for a purchase
    /// (`shop_commit_done` plays S_ITEM).
    pub committed: bool,
}

impl ShopSession {
    /// Shop opening (`JE_itemScreen`, head): `]I[` lists of the
    /// section, owned item added if missing (`player[0].last_items`,
    /// equal to the equipment at level end, `mainint.c:JE_endLevelAni`),
    /// then sort "JE: Sort items in merchant inventory".
    pub fn new(t: &Tables, items: Items, cash: u64, items_available: &[Vec<i32>], destinations: usize) -> Self {
        Self::with_always_available(t, items, cash, items_available, destinations, &AlwaysAvailable::default())
    }

    /// Like `new`, with the injection of the mods' `always_available` items
    /// after the owned item and before the sort (`INJECT_AVAIL`): an item
    /// already present is not doubled, a full list (10) ignores the surplus.
    pub fn with_always_available(
        t: &Tables,
        items: Items,
        cash: u64,
        items_available: &[Vec<i32>],
        destinations: usize,
        always: &AlwaysAvailable,
    ) -> Self {
        let mut avail: [Vec<u8>; 9] = Default::default();
        for (row, list) in items_available.iter().take(9).enumerate() {
            avail[row] = list.iter().take(10).map(|&v| v as u8).collect();
        }
        let mut last = items;
        for i in 0..7 {
            let item = *item_slot(&mut last, i);
            let named = |n: Option<String>| n.is_some_and(|s| !s.is_empty());
            let valid = match i {
                0 => item != 0 && named(t.ships.get(item as usize).map(|s| s.name.text())),
                1 | 2 => item != 0 && named(t.ports.get(item as usize).map(|s| s.name.text())),
                3 => item != 0 && named(t.shields.get(item as usize).map(|s| s.name.text())),
                4 => item != 0,
                _ => item == 0 || named(t.options.get(item as usize).map(|s| s.name.text())),
            };
            let row = &mut avail[ITEM_AVAIL_MAP[i] - 1];
            if valid && !row.contains(&item) && row.len() < 10 {
                row.push(item);
            }
        }
        // `INJECT_AVAIL`: row 0 ships, 1 front, 2 rear, 5 and 6
        // left and right sidekicks, 8 shields (the generator gets none).
        for (row, list) in [
            (0, &always.ships),
            (1, &always.front_weapons),
            (2, &always.rear_weapons),
            (5, &always.sidekicks),
            (6, &always.sidekicks),
            (8, &always.shields),
        ] {
            for &id in list {
                if !avail[row].contains(&id) && avail[row].len() < SHOP_LIST_CAPACITY {
                    avail[row].push(id);
                }
            }
        }
        for row in avail.iter_mut() {
            let n = row.len();
            if n > 1 {
                for a in 0..n - 1 {
                    for b in a..n {
                        if row[a] == 0 || (row[a] > row[b] && row[b] != 0) {
                            row.swap(a, b);
                        }
                    }
                }
            }
        }
        let mut s = Self {
            menu: ShopMenu::FullGame,
            cur_sel: [2; MENUS],
            menu_choices: [FULL_GAME_CHOICES, UPGRADES_CHOICES, 0, 0, 0, 0, 0, 2, 0],
            cube_max: 0,
            current_cube: 0,
            items,
            cash,
            item_avail: avail,
            destinations,
            chosen_items: items,
            old_items: items,
            temp_weapon_power: [1; SHOP_LIST_CAPACITY + 1],
            last_cur_sel: 2,
            last_direction: 1,
            left_power: false,
            right_power: false,
            right_power_afford: false,
            weapon_mode: 1,
            purchases: Vec::new(),
            committed: false,
        };
        s.prepare(t);
        s
    }

    fn sel(&self, m: ShopMenu) -> u8 {
        self.cur_sel[m as usize]
    }

    /// Open category (`curSel[MENU_UPGRADES]`, 2..8).
    pub fn upgrade_sel(&self) -> u8 {
        self.sel(ShopMenu::Upgrades)
    }

    /// Items of the open category (`itemAvail[itemAvailMap[cat]-1]`).
    pub fn category_items(&self) -> &[u8] {
        let cat = (self.upgrade_sel().clamp(2, 8) - 2) as usize;
        &self.item_avail[ITEM_AVAIL_MAP[cat] - 1]
    }

    fn done_slot(&self) -> usize {
        self.category_items().len()
    }

    fn is_weapon_cat(&self) -> bool {
        matches!(self.upgrade_sel(), 3 | 4)
    }

    /// Item of the open category in `old_items` ("owned" mark
    /// of `JE_itemScreen`) and in `chosen_items` (row highlighted on Done).
    /// Read-only, for display.
    pub fn owned_and_chosen(&self) -> (u8, u8) {
        let cat = (self.upgrade_sel().clamp(2, 8) - 2) as usize;
        let (mut o, mut c) = (self.old_items, self.chosen_items);
        (*item_slot(&mut o, cat), *item_slot(&mut c, cat))
    }

    /// Power of the weapon owned when entering the category
    /// (`old_items[0].weapon[port].power`), base of the displayed sell-back.
    pub fn old_power(&self, port: usize) -> u8 {
        self.old_items.weapon[port.min(1)].power
    }

    /// `JE_cashLeft()` with the current equipment of the open category.
    pub fn cash_left(&self, t: &Tables) -> i32 {
        let sel = self.upgrade_sel();
        let mut it = self.items;
        let item = *item_slot(&mut it, (sel - 2) as usize);
        let power = if self.is_weapon_cat() { self.items.weapon[(sel - 3) as usize].power } else { 1 };
        cash_left(t, self.cash, category(sel), item as u16, power)
    }

    /// Top of the `JE_itemScreen` loop then display block of
    /// `MENU_UPGRADE_SUB`: nothing is drawn here, but these lines
    /// modify the state (provisional powers, skipping of items that are too
    /// expensive, hovered item equipped live, power arrows).
    fn prepare(&mut self, t: &Tables) {
        self.left_power = false;
        self.right_power = false;
        if self.menu == ShopMenu::Upgrades && self.is_weapon_cat() {
            // "reset temp_weapon_power[] every time we select upgrading front or back"
            let port = (self.upgrade_sel() - 3) as usize;
            let WeaponSlot { id: WeaponId(item), power } = self.items.weapon[port];
            let list = self.category_items().to_vec();
            for (slot, &v) in list.iter().enumerate() {
                self.temp_weapon_power[slot] = if v == item { power } else { 1 };
            }
            self.temp_weapon_power[list.len()] = power;
        }
        if self.menu != ShopMenu::UpgradeSub {
            return;
        }
        let sub = ShopMenu::UpgradeSub as usize;
        let sel_up = self.upgrade_sel();
        let cat = category(sel_up);
        let list = self.category_items().to_vec();
        // "Move cursor until we hit either "Done" or a weapon the player can afford"
        while self.cur_sel[sub] < self.menu_choices[sub]
            && item_cost(t, cat, list[(self.cur_sel[sub] - 2) as usize] as u16) > self.cash as i64
        {
            self.cur_sel[sub] = (self.cur_sel[sub] as i16 + self.last_direction as i16) as u8;
            if self.cur_sel[sub] < 2 {
                self.cur_sel[sub] = self.menu_choices[sub];
            } else if self.cur_sel[sub] > self.menu_choices[sub] {
                self.cur_sel[sub] = 2;
            }
        }
        let sel = self.cur_sel[sub];
        let ci = (sel_up - 2) as usize;
        *item_slot(&mut self.items, ci) = if sel < self.menu_choices[sub] {
            list[(sel - 2) as usize]
        } else {
            *item_slot(&mut self.chosen_items.clone(), ci)
        };
        if self.is_weapon_cat() && sel < self.menu_choices[sub] {
            let hovered = list[(sel - 2) as usize];
            if sel_up == 4 && hovered == 0 {
                // ""None" on rear weapon menu cannot be upgraded"
                return;
            }
            let level = self.items.weapon[(sel_up - 3) as usize].power;
            let (_, upgrade_cost) = weapon_power_costs(t, hovered as u16, level);
            let max_level = max_power_level(t, cat, hovered as u16);
            self.left_power = level > 1;
            self.right_power = level < max_level;
            if self.right_power {
                self.right_power_afford = self.cash_left(t) as i64 >= upgrade_cost;
            }
        }
    }

    /// `weaponPort[player[0].items.weapon[REAR_WEAPON].id].opnum`: number of
    /// modes of the rear weapon (`shop_preview.c:JE_weaponSimUpdate` only shows
    /// "[/] Rear Weapon Mode" if there are two).
    pub fn rear_mode_count(&self, t: &Tables) -> u8 {
        t.ports[self.items.weapon[1].id.0.min(crate::tables::PORT_NUM as u8) as usize].opnum
    }

    /// `shop_choose_row`: keeps row `row` as the purchase.
    fn choose_row(&mut self, row: u8) {
        let sub = ShopMenu::UpgradeSub as usize;
        if row < 2 || row >= self.menu_choices[sub] {
            return;
        }
        let ci = (self.upgrade_sel() - 2) as usize;
        *item_slot(&mut self.chosen_items, ci) = self.category_items()[(row - 2) as usize];
        if self.is_weapon_cat() {
            let done = self.done_slot();
            self.temp_weapon_power[done] = self.temp_weapon_power[(row - 2) as usize];
        }
    }

    /// `shop_commit_done`: pays for the kept item.
    fn commit_done(&mut self, t: &Tables) {
        let sub = ShopMenu::UpgradeSub as usize;
        self.cur_sel[sub] = self.menu_choices[sub];
        let ci = (self.upgrade_sel() - 2) as usize;
        *item_slot(&mut self.items, ci) = *item_slot(&mut self.chosen_items.clone(), ci);
        if self.is_weapon_cat() {
            let done = self.done_slot();
            self.items.weapon[(self.upgrade_sel() - 3) as usize].power = self.temp_weapon_power[done];
        }
        // `player[0].cash = new_cash`: int32_t to unsigned long.
        let new_cash = self.cash_left(t);
        // game_menu.c: a purchase = the purse went down; the weapon port
        // (categories 3 and 4) is marked as bought. Rewrite: only if
        // the equipment changed (Done without change, or after an item
        // already bought by buy_row, does not count as a second purchase).
        let changed = self.items != self.old_items;
        if changed && new_cash >= 0 && (new_cash as u64) < self.cash {
            let weapon =
                if self.is_weapon_cat() { self.items.weapon[(self.upgrade_sel() - 3) as usize].id.0 } else { 0 };
            self.purchases.push((self.cash - new_cash as u64, weapon));
        }
        self.cash = new_cash as i64 as u64;
        self.menu = ShopMenu::Upgrades;
        self.committed = true;
    }

    /// Choosing an item (Enter or click on its row): it is bought right
    /// away, like Done, then the list stays open on the same row with
    /// that item as the new equipment ("owned" mark, sell-back and
    /// Escape start again from it). Intentional divergence: in the C code (game_menu.c:2924
    /// shop_choose_row) the choice was only paid for on confirming Done, the mark
    /// did not move and Escape cancelled the purchase.
    fn buy_row(&mut self, t: &Tables, row: u8) {
        let sub = ShopMenu::UpgradeSub as usize;
        self.choose_row(row);
        self.commit_done(t);
        self.menu = ShopMenu::UpgradeSub;
        self.cur_sel[sub] = row;
        self.last_cur_sel = row;
        self.old_items = self.items;
        self.chosen_items = self.items;
        // Same opening as JE_menuFunction(MENU_UPGRADES): the purse
        // includes the sell-back of the item now owned.
        let left = self.cash_left(t) as i64;
        self.cash = (self.cash as i64).wrapping_mul(2).wrapping_sub(left) as u64;
    }

    /// `JE_menuFunction(select)`.
    fn menu_function(&mut self, t: &Tables, select: u8) -> ShopAction {
        match self.menu {
            ShopMenu::FullGame => match select {
                2 => {
                    // "cubes": the list always starts again from the first cube.
                    self.menu = ShopMenu::DataCubes;
                    self.cur_sel[ShopMenu::DataCubes as usize] = 2;
                    self.menu_choices[ShopMenu::DataCubes as usize] = self.cube_max + 2;
                }
                4 => self.menu = ShopMenu::Upgrades,
                6 => {
                    self.menu = ShopMenu::PlayNextLevel;
                    self.menu_choices[ShopMenu::PlayNextLevel as usize] = self.destinations as u8 + 2;
                    self.cur_sel[ShopMenu::PlayNextLevel as usize] = 2;
                }
                7 => return ShopAction::QuitRequest,
                other => return ShopAction::FullGameEntry(other),
            },
            ShopMenu::DataCubes => {
                let m = ShopMenu::DataCubes as usize;
                if select == self.menu_choices[m] || self.cube_max == 0 {
                    self.menu = ShopMenu::FullGame;
                } else {
                    self.menu = ShopMenu::DataCubeSub;
                    self.current_cube = select - 2;
                    return ShopAction::ReadCube(self.current_cube);
                }
            }
            ShopMenu::DataCubeSub => self.menu = ShopMenu::DataCubes,
            ShopMenu::Upgrades => {
                if select == 9 {
                    self.menu = ShopMenu::FullGame;
                } else {
                    self.old_items = self.items;
                    self.chosen_items = self.items;
                    self.last_direction = 1;
                    // game_menu_nav.c:JE_genItemMenu
                    let list = self.category_items().to_vec();
                    let sub = ShopMenu::UpgradeSub as usize;
                    self.menu_choices[sub] = list.len() as u8 + 2;
                    let owned = *item_slot(&mut self.items.clone(), (select - 2) as usize);
                    self.cur_sel[sub] = list.iter().rposition(|&v| v == owned).map_or(2, |i| i as u8 + 2);
                    self.menu = ShopMenu::UpgradeSub;
                    self.last_cur_sel = self.cur_sel[sub];
                    let left = self.cash_left(t) as i64;
                    self.cash = (self.cash as i64).wrapping_mul(2).wrapping_sub(left) as u64;
                }
            }
            ShopMenu::UpgradeSub => {
                if self.sel(ShopMenu::UpgradeSub) < self.menu_choices[ShopMenu::UpgradeSub as usize] {
                    self.buy_row(t, select);
                } else {
                    self.commit_done(t);
                }
            }
            ShopMenu::PlayNextLevel => {
                // Last row: back (miscText[5]); otherwise destination.
                if (select as usize) < self.destinations + 2 {
                    return ShopAction::Leave(select as usize - 2);
                }
                self.menu = ShopMenu::FullGame;
            }
        }
        ShopAction::None
    }

    /// One key, like the `else if (newkey)` branch of `JE_itemScreen`,
    /// followed by the next loop top.
    pub fn key(&mut self, t: &Tables, key: ShopKey) -> ShopAction {
        let m = self.menu as usize;
        let sub_weapon = self.menu == ShopMenu::UpgradeSub && self.is_weapon_cat();
        let mut action = ShopAction::None;
        self.committed = false;
        match key {
            // Reading a cube: Up / Down scroll the text (held by
            // the screen); the `curSel[MENU_DATA_CUBE_SUB]` that they modify
            // in the C code is read by nobody.
            ShopKey::Up | ShopKey::Down if self.menu == ShopMenu::DataCubeSub => {}
            ShopKey::Up | ShopKey::Down => {
                self.last_direction = if key == ShopKey::Up { -1 } else { 1 };
                if key == ShopKey::Up {
                    self.cur_sel[m] = self.cur_sel[m].wrapping_sub(1);
                    if self.cur_sel[m] < 2 {
                        self.cur_sel[m] = self.menu_choices[m];
                    }
                } else {
                    self.cur_sel[m] += 1;
                    if self.cur_sel[m] > self.menu_choices[m] {
                        self.cur_sel[m] = 2;
                    }
                }
                if sub_weapon {
                    let port = (self.upgrade_sel() - 3) as usize;
                    self.items.weapon[port].power = self.temp_weapon_power[(self.cur_sel[m] - 2) as usize];
                    if port == 1 {
                        self.weapon_mode = 1;
                    }
                }
            }
            ShopKey::Slash => {
                // "cycle weapon modes" (rear weapon category only).
                if self.menu == ShopMenu::UpgradeSub && self.upgrade_sel() == 4 {
                    self.weapon_mode += 1;
                    let rear = self.items.weapon[1].id.0 as usize;
                    if self.weapon_mode > t.ports[rear.min(crate::tables::PORT_NUM)].opnum {
                        self.weapon_mode = 1;
                    }
                }
            }
            ShopKey::Select => {
                if sub_weapon {
                    let done = self.done_slot();
                    self.temp_weapon_power[done] = self.items.weapon[(self.upgrade_sel() - 3) as usize].power;
                    if self.upgrade_sel() == 4 {
                        self.weapon_mode = 1;
                    }
                }
                action = self.menu_function(t, self.cur_sel[m]);
            }
            ShopKey::Escape => match self.menu {
                ShopMenu::FullGame => action = ShopAction::QuitRequest,
                ShopMenu::UpgradeSub => {
                    // "leaving upgrade menu without buying"
                    self.items = self.old_items;
                    self.cur_sel[m] = self.last_cur_sel;
                    self.cash = self.cash_left(t) as i64 as u64;
                    self.menu = ShopMenu::Upgrades;
                }
                // menuEsc: MENU_DATA_CUBE_SUB -> MENU_DATA_CUBES.
                ShopMenu::DataCubeSub => self.menu = ShopMenu::DataCubes,
                // menuEsc: MENU_UPGRADES, MENU_PLAY_NEXT_LEVEL,
                // MENU_DATA_CUBES -> MENU_FULL_GAME.
                _ => self.menu = ShopMenu::FullGame,
            },
            ShopKey::Left | ShopKey::Right => {
                if sub_weapon {
                    let row = (self.cur_sel[m] - 2) as usize;
                    let port = (self.upgrade_sel() - 3) as usize;
                    if key == ShopKey::Left && self.left_power {
                        self.temp_weapon_power[row] -= 1;
                        self.items.weapon[port].power = self.temp_weapon_power[row];
                        self.choose_row(self.cur_sel[m]);
                    } else if key == ShopKey::Right && self.right_power && self.right_power_afford {
                        self.temp_weapon_power[row] += 1;
                        self.items.weapon[port].power = self.temp_weapon_power[row];
                        self.choose_row(self.cur_sel[m]);
                    }
                }
            }
        }
        self.prepare(t);
        action
    }

    /// Power arrows clicked (`game_menu.c:JE_itemScreen`, block
    /// "curMenu == MENU_UPGRADE_SUB && curSel[MENU_UPGRADES] == 3 || 4"):
    /// `raise` false = left arrow (`leftPower`), true = right arrow
    /// (`rightPower && rightPowerAfford`). True if the power changed;
    /// false otherwise (the C code then plays S_CLINK). Nothing outside the weapon categories.
    pub fn click_power_arrow(&mut self, t: &Tables, raise: bool) -> Option<bool> {
        if !(self.menu == ShopMenu::UpgradeSub && self.is_weapon_cat()) {
            return None;
        }
        let m = self.menu as usize;
        let row = (self.cur_sel[m] - 2) as usize;
        let port = (self.upgrade_sel() - 3) as usize;
        let applied = if !raise && self.left_power {
            self.temp_weapon_power[row] -= 1;
            true
        } else if raise && self.right_power && self.right_power_afford {
            self.temp_weapon_power[row] += 1;
            true
        } else {
            false
        };
        if applied {
            self.items.weapon[port].power = self.temp_weapon_power[row];
            self.choose_row(self.cur_sel[m]);
        }
        self.last_direction = 1;
        self.prepare(t);
        Some(applied)
    }

    /// `game_menu.c:menu_row_under_mouse`: row (`curSel`, 2 = first)
    /// under point (x, y) of the current menu, `None` outside the column of
    /// choices (`mouse_in_menu_column`). `scroll`: `shop_scroll`, first
    /// visible row of the item list.
    pub fn row_under_mouse(&self, x: i32, y: i32, scroll: i32) -> Option<u8> {
        // mouse_in_menu_column: curMenu != MENU_DATA_CUBE_SUB && mouseY > 20 && 170 < mouseX < 308.
        if self.menu == ShopMenu::DataCubeSub || y <= 20 || x <= 170 || x >= 308 {
            return None;
        }
        // mouseSelectionY[curMenu] = { 16, 16, 16, 16, 26, 12, 11, 28, 0, … }.
        let step = match self.menu {
            ShopMenu::UpgradeSub => 26,
            ShopMenu::DataCubes => 28,
            ShopMenu::DataCubeSub => return None,
            _ => 16,
        };
        // C integer division (truncated towards zero).
        let mut sel = (y - 38) / step + 2;
        if self.menu == ShopMenu::UpgradeSub {
            sel += scroll;
        }
        let choices = self.menu_choices[self.menu as usize] as i32;
        if self.menu == ShopMenu::FullGame && sel > 7 {
            sel = 7;
        }
        if self.menu == ShopMenu::PlayNextLevel && sel == choices + 1 {
            sel = choices;
        }
        if sel < 2 || sel > choices {
            return None;
        }
        Some(sel as u8)
    }

    /// Hover (inner loop of `JE_itemScreen`, mouse with no button that
    /// moved): row `row` becomes the selection, with
    /// `menu_apply_selection_side_effects` (power of the hovered weapon,
    /// rear mode reset to 1) and `lastDirection = 1`. True if the selection
    /// changed (the C code then plays S_CURSOR).
    pub fn hover_row(&mut self, t: &Tables, row: u8) -> bool {
        let m = self.menu as usize;
        if row < 2 || row == self.cur_sel[m] || row > self.menu_choices[m] {
            return false;
        }
        self.cur_sel[m] = row;
        if self.menu == ShopMenu::UpgradeSub && self.is_weapon_cat() {
            let port = (self.upgrade_sel() - 3) as usize;
            self.items.weapon[port].power = self.temp_weapon_power[(row - 2) as usize];
            if port == 1 {
                self.weapon_mode = 1;
            }
        }
        self.last_direction = 1;
        self.prepare(t);
        true
    }

    /// Click on row `row` (`JE_itemScreen`, `mouseButton > 0` branch,
    /// `mouse_in_menu_column`): "Done" of a category pays
    /// (`shop_commit_done`, S_ITEM); a click on the chosen row activates it
    /// (`JE_menuFunction`); on another row, it chooses it (S_CLICK),
    /// except an item that is too expensive (S_CLINK), and in a category keeps it
    /// as the purchase (`shop_choose_row`).
    pub fn click_row(&mut self, t: &Tables, row: u8) -> (ShopAction, ShopSound) {
        let m = self.menu as usize;
        self.last_direction = 1;
        self.committed = false;
        if row < 2 || row > self.menu_choices[m] {
            return (ShopAction::None, ShopSound::None);
        }
        let mut sound = ShopSound::Click;
        let mut action = ShopAction::None;
        if self.menu == ShopMenu::UpgradeSub && row == self.menu_choices[m] {
            // "Same path as the keyboard confirmation."
            self.commit_done(t);
            sound = ShopSound::Item;
        } else if self.cur_sel[m] == row {
            action = self.menu_function(t, row);
        } else {
            let sub = self.menu == ShopMenu::UpgradeSub;
            let too_expensive = sub && {
                let list = self.category_items();
                item_cost(t, category(self.upgrade_sel()), list[(row - 2) as usize] as u16) > self.cash as i64
            };
            if too_expensive {
                sound = ShopSound::Clink;
            } else {
                if self.upgrade_sel() == 4 {
                    self.weapon_mode = 1;
                }
                self.cur_sel[m] = row;
            }
            if sub && self.is_weapon_cat() {
                let port = (self.upgrade_sel() - 3) as usize;
                self.items.weapon[port].power = self.temp_weapon_power[(self.cur_sel[m] - 2) as usize];
            }
            // A click on an item buys it, like Enter (see buy_row).
            if sub && !too_expensive {
                self.buy_row(t, row);
            }
        }
        if self.committed {
            sound = ShopSound::Item;
        }
        self.prepare(t);
        (action, sound)
    }
}

/// Sound played by a mouse click in the shop (`JE_playSampleNum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopSound {
    None,
    /// `S_CLICK`: row chosen or activated.
    Click,
    /// `S_CLINK`: item too expensive.
    Clink,
    /// `S_ITEM`: purchase paid ("Done").
    Item,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_cost_is_triangular() {
        // "0 1 3 6 10 15 21 29" (mainint.c, comment of weapon_upgrade_cost).
        let v: Vec<i64> = (0..6).map(|p| weapon_upgrade_cost(1, p)).collect();
        assert_eq!(v, vec![0, 1, 3, 6, 10, 15]);
        assert_eq!(weapon_upgrade_cost(500, 3), 3000);
    }

    /// `tyrian.hdt` tables (episodes 1-3); `None` without the game data.
    fn tables() -> Option<Tables> {
        let hdt = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/tyrian.hdt");
        let bytes = std::fs::read(hdt).ok()?;
        Some(Tables::new(&tyrian_formats::hdt::HdtFile::parse(&bytes).ok()?.items))
    }

    /// Test shop: `]I[` lists (rows 1 ships, 2 front, 3
    /// rear, 4 generators, 6 and 7 sidekicks, 9 shields) and the equipment
    /// of a new game (mainint.c:JE_initPlayerData).
    fn session(t: &Tables, cash: u64, cube_max: u8) -> ShopSession {
        let avail: Vec<Vec<i32>> = vec![
            vec![1, 2],
            vec![1, 2, 3],
            vec![0, 9, 10],
            vec![2, 3],
            vec![],
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![],
            vec![4, 5],
        ];
        let items = Items {
            ship: crate::ShipId(1),
            weapon: [WeaponSlot { id: WeaponId(1), power: 1 }, WeaponSlot { id: WeaponId(0), power: 1 }],
            shield: crate::ShieldId(4),
            generator: crate::GeneratorId(2),
            ..Default::default()
        };
        let mut s = ShopSession::new(t, items, cash, &avail, 2);
        s.cube_max = cube_max;
        s
    }

    /// `INJECT_AVAIL`: mod items join the right list, without
    /// duplicates, in sorted order; the generator receives none; past the
    /// level's 10 items the list still takes the mods' items, up to
    /// `SHOP_LIST_CAPACITY`, then ignores the surplus.
    #[test]
    fn always_available_items_are_injected_like_the_c() {
        let Some(t) = tables() else { return };
        let always = AlwaysAvailable {
            ships: vec![200, 201],
            front_weapons: vec![250, 1],
            rear_weapons: vec![251],
            sidekicks: vec![40],
            shields: vec![30],
        };
        let items = Items { ship: crate::ShipId(1), ..Default::default() };
        let avail: Vec<Vec<i32>> =
            vec![vec![1, 2], vec![1, 2, 3], vec![9, 10], vec![2, 3], vec![], vec![], vec![], vec![], vec![4]];
        let s = ShopSession::with_always_available(&t, items, 1000, &avail, 2, &always);
        assert_eq!(s.item_avail[0], vec![1, 2, 200, 201]);
        assert_eq!(s.item_avail[1], vec![1, 2, 3, 250], "1 already present: no duplicate");
        assert_eq!(s.item_avail[2], vec![9, 10, 251]);
        assert_eq!(s.item_avail[3], vec![2, 3], "generator: no injection");
        assert_eq!(s.item_avail[5], vec![40, 0]);
        assert_eq!(s.item_avail[6], vec![40, 0]);
        assert_eq!(s.item_avail[8], vec![4, 30]);

        let full: Vec<Vec<i32>> = vec![vec![], (1..=10).collect()];
        let s = ShopSession::with_always_available(&t, items, 1000, &full, 2, &always);
        assert_eq!(s.item_avail[1].len(), 11, "level list of 10: the mod item 250 still joins");
        assert!(s.item_avail[1].contains(&250));

        // 14 mod ships (every mod slot) after 4 level ships: all offered.
        let ships = AlwaysAvailable { ships: (19..=32).collect(), ..Default::default() };
        let level: Vec<Vec<i32>> = vec![vec![1, 2, 3, 4]];
        let s = ShopSession::with_always_available(&t, items, 1000, &level, 2, &ships);
        assert_eq!(s.item_avail[0], [1, 2, 3, 4].into_iter().chain(19..=32).collect::<Vec<u8>>());

        // Beyond the capacity, the surplus is ignored.
        let many = AlwaysAvailable { front_weapons: (100..=130).collect(), ..Default::default() };
        let s = ShopSession::with_always_available(&t, items, 1000, &full, 2, &many);
        assert_eq!(s.item_avail[1].len(), SHOP_LIST_CAPACITY);
        assert!(!s.item_avail[1].contains(&130));
    }

    /// A weapon list filled to `SHOP_LIST_CAPACITY` (the level's items and the
    /// mods' ones, ADR 0134): every row can be hovered, its power raised and
    /// lowered, and Done reached, front and rear, without leaving the arrays.
    #[test]
    fn full_weapon_lists_are_browsable() {
        let Some(t) = tables() else { return };
        let always = AlwaysAvailable {
            front_weapons: (4..=40).collect(),
            rear_weapons: (4..=40).collect(),
            ..Default::default()
        };
        let avail: Vec<Vec<i32>> = vec![vec![1], vec![1, 2, 3], vec![0, 9, 10]];
        let items = Items {
            ship: crate::ShipId(1),
            weapon: [WeaponSlot { id: WeaponId(1), power: 1 }, WeaponSlot { id: WeaponId(0), power: 1 }],
            ..Default::default()
        };
        for cat in [3, 4] {
            let mut s = ShopSession::with_always_available(&t, items, 1_000_000, &avail, 2, &always);
            open_category(&mut s, &t, cat);
            assert_eq!(s.category_items().len(), SHOP_LIST_CAPACITY);
            let sub = ShopMenu::UpgradeSub as usize;
            for _ in 0..2 * SHOP_LIST_CAPACITY {
                keys(&mut s, &t, "DRRL");
            }
            for _ in 0..SHOP_LIST_CAPACITY + 1 {
                s.key(&t, ShopKey::Up);
            }
            assert!((2..=s.menu_choices[sub]).contains(&s.cur_sel[sub]));
        }
    }

    fn keys(s: &mut ShopSession, t: &Tables, ks: &str) -> Vec<ShopAction> {
        ks.chars().filter(|c| !c.is_whitespace()).map(|c| s.key(t, ShopKey::from_script(c).unwrap())).collect()
    }

    /// Opens category `cat` (2..8) from the root.
    fn open_category(s: &mut ShopSession, t: &Tables, cat: u8) {
        keys(s, t, "DDE");
        for _ in 2..cat {
            s.key(t, ShopKey::Down);
        }
        s.key(t, ShopKey::Select);
        assert_eq!(s.menu, ShopMenu::UpgradeSub);
        assert_eq!(s.upgrade_sel(), cat);
    }

    /// Brings the cursor to item `id` (Down, at most one pass over the list).
    fn hover(s: &mut ShopSession, t: &Tables, id: u8) {
        let cat = (s.upgrade_sel() - 2) as usize;
        for _ in 0..12 {
            let mut it = s.items;
            if *item_slot(&mut it, cat) == id
                && s.cur_sel[ShopMenu::UpgradeSub as usize] < s.menu_choices[ShopMenu::UpgradeSub as usize]
            {
                return;
            }
            s.key(t, ShopKey::Down);
        }
        panic!("item {id} not found");
    }

    /// Confirms the current row (Enter) then "Done" (Up wraps to the
    /// last row, Enter pays).
    fn choose_and_commit(s: &mut ShopSession, t: &Tables) {
        s.key(t, ShopKey::Select);
        while s.cur_sel[ShopMenu::UpgradeSub as usize] != s.menu_choices[ShopMenu::UpgradeSub as usize] {
            s.key(t, ShopKey::Down);
        }
        s.key(t, ShopKey::Select);
        assert_eq!(s.menu, ShopMenu::Upgrades);
    }

    #[test]
    fn buying_resells_the_owned_item_at_its_value() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 50_000, 0);
        let cash = s.cash;
        // Generator: row 2 of the ]I[ (3) replaces the owned 2.
        open_category(&mut s, &t, 6);
        assert_eq!(
            s.cash,
            cash + item_value(&t, ShopCategory::Generator, 2, 1) as u64,
            "entry = trade-in of the owned generator"
        );
        hover(&mut s, &t, 3);
        assert_eq!(s.items.generator.0, 3, "hovered item fitted live");
        choose_and_commit(&mut s, &t);
        let price = item_cost(&t, ShopCategory::Generator, 3) - item_cost(&t, ShopCategory::Generator, 2);
        assert_eq!(s.items.generator.0, 3);
        assert_eq!(s.cash as i64, cash as i64 - price, "purchase = new item price minus trade-in of the old one");
    }

    /// Player bug of 2026-10-10: Enter on another weapon did not equip it
    /// (the "owned" mark stayed on the old one, nothing was paid before
    /// Done). Choosing an item now buys it right away, the cursor stays
    /// on the row, and Escape no longer takes it back.
    #[test]
    fn enter_on_an_item_buys_it_and_moves_the_owned_marker() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 50_000, 0);
        let cash = s.cash;
        open_category(&mut s, &t, 6);
        hover(&mut s, &t, 3);
        let row = s.cur_sel[ShopMenu::UpgradeSub as usize];
        s.key(&t, ShopKey::Select);
        let price = item_cost(&t, ShopCategory::Generator, 3) - item_cost(&t, ShopCategory::Generator, 2);
        assert_eq!(s.menu, ShopMenu::UpgradeSub, "stays in the list");
        assert_eq!(s.cur_sel[ShopMenu::UpgradeSub as usize], row, "cursor on the purchased item");
        assert_eq!(s.owned_and_chosen().0, 3, "'owned' mark on the new item");
        assert_eq!(s.cash_left(&t) as i64, cash as i64 - price, "displayed cash = purchase paid");
        assert_eq!(s.purchases.len(), 1, "one purchase counted");
        s.key(&t, ShopKey::Escape);
        assert_eq!((s.menu, s.items.generator.0), (ShopMenu::Upgrades, 3), "Esc keeps the purchase");
        assert_eq!(s.cash as i64, cash as i64 - price);
    }

    #[test]
    fn click_on_another_weapon_buys_it() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 50_000, 0);
        let cash = s.cash;
        open_category(&mut s, &t, 3);
        let owned = s.owned_and_chosen().0;
        let list = s.category_items().to_vec();
        let row = (list.iter().position(|&v| v != owned && v != 0).unwrap() + 2) as u8;
        let id = list[(row - 2) as usize];
        let (_, sound) = s.click_row(&t, row);
        assert_eq!(sound, ShopSound::Item, "purchase paid: S_ITEM");
        assert_eq!(s.owned_and_chosen().0, id, "'owned' mark moved on click");
        assert_eq!(s.items.weapon[0].id.0, id);
        // Done does not pay again: no second purchase, purse unchanged.
        let after = s.cash_left(&t);
        let done = s.menu_choices[ShopMenu::UpgradeSub as usize];
        s.click_row(&t, done);
        assert_eq!((s.menu, s.cash as i32, s.purchases.len()), (ShopMenu::Upgrades, after, 1));
        assert!((s.cash as i64) < cash as i64);
    }

    #[test]
    fn escape_cancels_and_refunds() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 50_000, 0);
        let (cash, items) = (s.cash, s.items);
        open_category(&mut s, &t, 5);
        // Item hovered without Enter: Escape takes it back and refunds.
        hover(&mut s, &t, 5);
        s.key(&t, ShopKey::Escape);
        assert_eq!((s.menu, s.cash, s.items.shield), (ShopMenu::Upgrades, cash, items.shield));
    }

    #[test]
    fn weapon_power_up_and_down_cost_and_refund() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 100_000, 0);
        let cash = s.cash as i64;
        open_category(&mut s, &t, 3);
        let base = t.ports[1].cost as i64;
        assert!(s.right_power && !s.left_power, "power 1: right arrow only");
        let left0 = s.cash_left(&t) as i64;
        s.key(&t, ShopKey::Right);
        assert_eq!(s.items.weapon[0].power, 2);
        assert_eq!(left0 - s.cash_left(&t) as i64, base, "POWER 2 costs the base price once");
        s.key(&t, ShopKey::Right);
        // game_menu.c:JE_cashLeft: tempW accumulates base·i and each pass subtracts
        // the cumulative amount, i.e. base·(1) + base·(1 + 2) for POWER 3.
        assert_eq!(left0 - s.cash_left(&t) as i64, base + 3 * base, "POWER 3: cumulative cost 1 + 3");
        s.key(&t, ShopKey::Left);
        assert_eq!(s.items.weapon[0].power, 2);
        assert_eq!(left0 - s.cash_left(&t) as i64, base, "downgrade refunded");
        choose_and_commit(&mut s, &t);
        assert_eq!(s.items.weapon[0], WeaponSlot { id: WeaponId(1), power: 2 });
        assert_eq!(s.cash as i64, cash - base);
        // Selling back this POWER 2 cannon yields its full value (JE_getValue).
        s.key(&t, ShopKey::Select);
        assert_eq!(s.upgrade_sel(), 3);
        assert_eq!(s.cash as i64, cash - base + item_value(&t, ShopCategory::FrontWeapon, 1, 2));
        assert_eq!(item_value(&t, ShopCategory::FrontWeapon, 1, 2), 2 * base);
    }

    #[test]
    fn rear_none_has_no_power_arrows() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 100_000, 0);
        open_category(&mut s, &t, 4);
        assert_eq!(s.items.weapon[1].id.0, 0);
        assert!(!s.left_power && !s.right_power, "'None' in the rear bay cannot be upgraded");
        hover(&mut s, &t, 9);
        assert!(s.right_power, "a real rear gun can be upgraded");
    }

    #[test]
    fn sidekicks_left_and_right_are_separate_lists() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 100_000, 0);
        // Left: ]I[ row 6 (0, 1, 2); right: row 7 (0, 1, 3).
        open_category(&mut s, &t, 7);
        assert_eq!(s.category_items(), &[1, 2, 0][..], "C sort order: None (0) last");
        hover(&mut s, &t, 2);
        choose_and_commit(&mut s, &t);
        assert_eq!(s.items.sidekick, [crate::SidekickId(2), crate::SidekickId(0)]);
        s.key(&t, ShopKey::Down);
        s.key(&t, ShopKey::Select);
        assert_eq!(s.category_items(), &[1, 3, 0][..]);
        hover(&mut s, &t, 3);
        choose_and_commit(&mut s, &t);
        assert_eq!(s.items.sidekick, [crate::SidekickId(2), crate::SidekickId(3)]);
    }

    #[test]
    fn ship_and_shield_categories_buy() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 1_000_000, 0);
        open_category(&mut s, &t, 2);
        hover(&mut s, &t, 2);
        choose_and_commit(&mut s, &t);
        assert_eq!(s.items.ship.0, 2);
        s.key(&t, ShopKey::Down);
        s.key(&t, ShopKey::Down);
        s.key(&t, ShopKey::Down);
        s.key(&t, ShopKey::Select);
        assert_eq!(s.upgrade_sel(), 5);
        hover(&mut s, &t, 5);
        choose_and_commit(&mut s, &t);
        assert_eq!(s.items.shield.0, 5);
    }

    #[test]
    fn unaffordable_rows_are_skipped() {
        let Some(t) = tables() else { return };
        // Just the sell-back of the owned generator: the 3 is too expensive.
        let mut s = session(&t, 0, 0);
        open_category(&mut s, &t, 6);
        s.key(&t, ShopKey::Down);
        assert_eq!(
            s.cur_sel[ShopMenu::UpgradeSub as usize],
            s.menu_choices[ShopMenu::UpgradeSub as usize],
            "cursor jumped to Done"
        );
    }

    /// The power cost does not wrap around at 16 bits: buying then
    /// selling back an expensive weapon returns exactly what it cost.
    #[test]
    fn expensive_weapon_power_does_not_wrap() {
        let Some(mut t) = tables() else { return };
        t.ports[1].cost = 2000;
        let paid = 1_000_000 - cash_left(&t, 1_000_000, ShopCategory::FrontWeapon, 1, 11) as i64;
        assert_eq!(paid, item_value(&t, ShopCategory::FrontWeapon, 1, 11));
        assert_eq!(paid, 2000 * (1 + 1 + 3 + 6 + 10 + 15 + 21 + 28 + 36 + 45 + 55));
    }

    #[test]
    fn data_cubes_menu() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 0, 2);
        assert_eq!(s.key(&t, ShopKey::Select), ShopAction::None);
        assert_eq!(s.menu, ShopMenu::DataCubes);
        assert_eq!(s.menu_choices[ShopMenu::DataCubes as usize], 4, "two cubes + Exit");
        s.key(&t, ShopKey::Down);
        assert_eq!(s.key(&t, ShopKey::Select), ShopAction::ReadCube(1));
        assert_eq!(s.menu, ShopMenu::DataCubeSub);
        s.key(&t, ShopKey::Down);
        assert_eq!(s.cur_sel[ShopMenu::DataCubes as usize], 3, "scrolling the text does not change the cube");
        s.key(&t, ShopKey::Escape);
        assert_eq!(s.menu, ShopMenu::DataCubes);
        keys(&mut s, &t, "DE");
        assert_eq!(s.menu, ShopMenu::FullGame, "Exit to Game Menu");
        // Without a cube: Enter returns to the menu.
        let mut s = session(&t, 0, 0);
        keys(&mut s, &t, "EE");
        assert_eq!(s.menu, ShopMenu::FullGame);
    }

    #[test]
    fn quit_ship_specs_and_options_entries() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 0, 0);
        s.cur_sel[0] = 7;
        assert_eq!(s.key(&t, ShopKey::Select), ShopAction::QuitRequest, "Quit Game");
        assert_eq!(s.key(&t, ShopKey::Escape), ShopAction::QuitRequest);
        s.cur_sel[0] = 3;
        assert_eq!(s.key(&t, ShopKey::Select), ShopAction::FullGameEntry(3), "Ship Specs");
        s.cur_sel[0] = 5;
        assert_eq!(s.key(&t, ShopKey::Select), ShopAction::FullGameEntry(5), "Options (save screen)");
        assert_eq!(s.menu, ShopMenu::FullGame);
    }

    /// `menu_row_under_mouse`: column x 171..307, rows of 16 px from
    /// y = 38 (truncated division: y 23..37 also targets the first), "Quit"
    /// offset by 16 px reached by rank 8 brought back to 7.
    #[test]
    fn row_under_mouse_follows_game_menu_geometry() {
        let Some(t) = tables() else { return };
        let s = session(&t, 10_000, 0);
        assert_eq!(s.row_under_mouse(200, 40, 0), Some(2));
        assert_eq!(s.row_under_mouse(200, 25, 0), Some(2));
        assert_eq!(s.row_under_mouse(200, 20, 0), None);
        assert_eq!(s.row_under_mouse(170, 40, 0), None);
        assert_eq!(s.row_under_mouse(308, 40, 0), None);
        assert_eq!(s.row_under_mouse(200, 38 + 16 * 4, 0), Some(6));
        assert_eq!(s.row_under_mouse(200, 38 + 16 * 6 + 2, 0), Some(7));
        assert_eq!(s.row_under_mouse(200, 190, 0), Some(7));
    }

    /// Click: another row is chosen (S_CLICK), the chosen row is
    /// activated ("Upgrade Ship" opens MENU_UPGRADES), "Done" of a
    /// category pays for the kept item (S_ITEM), an item that is too expensive plays
    /// S_CLINK without being chosen.
    #[test]
    fn click_row_selects_then_activates_and_commits() {
        let Some(t) = tables() else { return };
        // 1_000_000 credits: all items of the game are reachable (the
        // Dragonwing costs 100_000, the Revulsor 200_000).
        let mut s = session(&t, 1_000_000, 0);
        assert_eq!(s.click_row(&t, 4), (ShopAction::None, ShopSound::Click));
        assert_eq!(s.cur_sel[ShopMenu::FullGame as usize], 4);
        assert_eq!(s.menu, ShopMenu::FullGame);
        assert_eq!(s.click_row(&t, 4), (ShopAction::None, ShopSound::Click));
        assert_eq!(s.menu, ShopMenu::Upgrades);
        // "Ship" is already the chosen row: the click opens the category.
        s.click_row(&t, 2);
        assert_eq!(s.menu, ShopMenu::UpgradeSub);
        let sub = ShopMenu::UpgradeSub as usize;
        let cash = s.cash;
        let done = s.menu_choices[sub];
        let other = if s.cur_sel[sub] == 2 { 3 } else { 2 };
        // Click on another item: bought right away (S_ITEM, buy_row).
        assert_eq!(s.click_row(&t, other).1, ShopSound::Item);
        assert_eq!(s.cur_sel[sub], other);
        assert_eq!(s.menu, ShopMenu::UpgradeSub);
        assert_eq!(s.click_row(&t, done), (ShopAction::None, ShopSound::Item));
        assert_eq!(s.menu, ShopMenu::Upgrades);
        assert_ne!(s.cash, cash);
        // Item more expensive than the available money: S_CLINK, not chosen.
        let mut poor = session(&t, 0, 0);
        open_category(&mut poor, &t, 3);
        let list = poor.category_items().to_vec();
        let pricey = (0..list.len()).find(|&i| {
            item_cost(&t, category(poor.upgrade_sel()), list[i] as u16) > poor.cash as i64
                && i as u8 + 2 != poor.cur_sel[sub]
        });
        if let Some(i) = pricey {
            let before = poor.cur_sel[sub];
            assert_eq!(poor.click_row(&t, i as u8 + 2).1, ShopSound::Clink);
            assert_eq!(poor.cur_sel[sub], before);
        }
    }

    /// Power arrows with the mouse: up, back down, refusal (S_CLINK)
    /// when the power is at the minimum or money is lacking.
    #[test]
    fn power_arrows_by_mouse() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 1_000_000, 0);
        open_category(&mut s, &t, 3);
        let sub = ShopMenu::UpgradeSub as usize;
        // A front cannon that upgrades: first row whose right arrow is active.
        let rows = s.menu_choices[sub];
        let mut found = false;
        for r in 2..rows {
            s.hover_row(&t, r);
            if s.right_power {
                found = true;
                break;
            }
        }
        assert!(found, "no upgradable weapon in the front category");
        let row = s.cur_sel[sub];
        let port = (s.upgrade_sel() - 3) as usize;
        let before = s.items.weapon[port].power;
        assert_eq!(s.click_power_arrow(&t, false), Some(false), "minimum power: refused");
        assert_eq!(s.click_power_arrow(&t, true), Some(true));
        assert_eq!(s.items.weapon[port].power, before + 1);
        assert_eq!(s.temp_weapon_power[(row - 2) as usize], before + 1);
        assert_eq!(s.click_power_arrow(&t, false), Some(true));
        assert_eq!(s.items.weapon[port].power, before);
        // Insufficient funds: the right arrow is refused.
        let mut poor = session(&t, 0, 0);
        open_category(&mut poor, &t, 3);
        poor.right_power_afford = false;
        assert_ne!(poor.click_power_arrow(&t, true), Some(true));
        // Outside a weapon category: no effect.
        let mut other = session(&t, 1_000_000, 0);
        open_category(&mut other, &t, 2);
        assert_eq!(other.click_power_arrow(&t, true), None);
    }

    /// Hover: the hovered row becomes the selection, without activating.
    #[test]
    fn hover_row_moves_selection_only() {
        let Some(t) = tables() else { return };
        let mut s = session(&t, 10_000, 0);
        assert!(s.hover_row(&t, 5));
        assert_eq!(s.cur_sel[ShopMenu::FullGame as usize], 5);
        assert!(!s.hover_row(&t, 5));
        assert_eq!(s.menu, ShopMenu::FullGame);
    }
}
