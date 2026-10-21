//! Godot bridge for the interactive shop (`tyrian_sim::shop::ShopSession`,
//! port of `game_menu.c:JE_itemScreen` / `JE_menuFunction`). All the
//! logic (resale, weapon power, confirmation) is in Rust; `shop.gd`
//! forwards the keys and displays the state returned by `state()`.
//!
//! Equipment: same order as `TyrianSim.set_loadout` (ship, front,
//! front power, rear, rear power, shield, generator,
//! left sidekick, right sidekick, special, money).

use std::path::Path;
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId};

use crate::vga::{with_assets, TyrianVga};
use tyrian_render::Surface;

use godot::prelude::*;
use tyrian_formats::crypt::EncryptedStringFile;
use tyrian_formats::cube::{self, Cube};
use tyrian_formats::hdt::HdtFile;
use tyrian_formats::lvl::LvlFile;
use tyrian_render::font;
use tyrian_sim::shop::{
    item_cost, item_value, weapon_power_costs, AlwaysAvailable, ShopAction, ShopCategory, ShopKey, ShopSession,
    ShopSound,
};
use tyrian_sim::shop_preview::WeaponPreview;
use tyrian_sim::tables::Tables;
use tyrian_sim::world::{Items, WeaponSlot};
use tyrian_sim::WeaponId;

#[derive(GodotClass)]
#[class(base = RefCounted, init)]
pub struct TyrianShop {
    tables: Option<Tables>,
    session: Option<ShopSession>,
    /// Fire demonstration for the weapon categories (`shop_preview.c`).
    preview: Option<WeaponPreview>,
    special: u8,
    /// `cube[]`: readable data cubes (`load_cubes`), `cubeMax` at most.
    cubes: Vec<Cube>,
    /// Mods attached by `set_mods`: tables and `always_available` items.
    mods: Option<Gd<crate::TyrianMods>>,
    #[var]
    last_error: GString,
}

/// Item tables of the episode (`episodes.c:JE_loadItemDat`), like
/// `tyrian_sim::setup::load_level`.
pub(crate) fn load_tables(data_dir: &Path, episode: i32) -> Result<Tables, String> {
    let lvl_path = data_dir.join(format!("tyrian{episode}.lvl"));
    let bytes = std::fs::read(&lvl_path).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
    let lvl = LvlFile::parse(&bytes).map_err(|e| format!("{}: {e}", lvl_path.display()))?;
    let items = match lvl.item_tables() {
        Some(t) => t.map_err(|e| e.to_string())?,
        None => {
            let hdt = data_dir.join("tyrian.hdt");
            let b = std::fs::read(&hdt).map_err(|e| format!("{}: {e}", hdt.display()))?;
            HdtFile::parse(&b).map_err(|e| e.to_string())?.items
        }
    };
    Ok(Tables::new(&items))
}

/// `fonthand.c:TINY_FONT` (cube font, `JE_textWidth(..., TINY_FONT)`).
const TINY_FONT: usize = 2;

fn shop_category(cat: u8) -> ShopCategory {
    match cat {
        2 => ShopCategory::Ship,
        3 => ShopCategory::FrontWeapon,
        4 => ShopCategory::RearWeapon,
        5 => ShopCategory::Shield,
        6 => ShopCategory::Generator,
        7 => ShopCategory::LeftSidekick,
        _ => ShopCategory::RightSidekick,
    }
}

fn key_from_name(name: &str) -> Option<ShopKey> {
    Some(match name {
        "up" => ShopKey::Up,
        "down" => ShopKey::Down,
        "left" => ShopKey::Left,
        "right" => ShopKey::Right,
        "select" => ShopKey::Select,
        "escape" => ShopKey::Escape,
        "slash" => ShopKey::Slash,
        _ => return None,
    })
}

#[godot_api]
impl TyrianShop {
    /// Mods applied to the next shop openings (null = none).
    #[func]
    fn set_mods(&mut self, mods: Option<Gd<crate::TyrianMods>>) {
        self.mods = mods;
    }

    /// Opens the shop (`JE_itemScreen`): `items_available` = the 9
    /// arrays returned by `TyrianCampaign.run_script` (stop "shop"),
    /// `destinations` = number of `mapSection` entries.
    #[func]
    fn open(
        &mut self,
        data_dir: GString,
        episode: i32,
        loadout: PackedInt32Array,
        items_available: VarArray,
        destinations: i32,
    ) -> bool {
        let v = loadout.as_slice();
        if v.len() < 11 {
            self.last_error = "loadout: 11 values expected".into();
            return false;
        }
        let mut tables = match load_tables(Path::new(&data_dir.to_string()), episode) {
            Ok(t) => t,
            Err(e) => {
                self.last_error = e.as_str().into();
                return false;
            }
        };
        let mut always = AlwaysAvailable::default();
        if let Some((mod_tables, mod_always)) = self.mods.as_mut().and_then(|m| m.bind_mut().shop_data(episode)) {
            tables = mod_tables;
            always = mod_always;
        }
        let b = |i: usize| v[i].clamp(0, 255) as u8;
        let items = Items {
            ship: ShipId(b(0)),
            weapon: [
                WeaponSlot { id: WeaponId(b(1)), power: b(2).max(1) },
                WeaponSlot { id: WeaponId(b(3)), power: b(4).max(1) },
            ],
            shield: ShieldId(b(5)),
            generator: GeneratorId(b(6)),
            sidekick: [SidekickId(b(7)), SidekickId(b(8))],
            special: SpecialId(b(9)),
            ..Default::default()
        };
        let avail: Vec<Vec<i32>> = items_available
            .iter_shared()
            .map(|r| r.try_to::<PackedInt32Array>().map(|a| a.to_vec()).unwrap_or_default())
            .collect();
        self.special = b(9);
        self.session = Some(ShopSession::with_always_available(
            &tables,
            items,
            v[10].max(0) as u64,
            &avail,
            destinations.max(0) as usize,
            &always,
        ));
        crate::ach_api::shop_opened();
        self.tables = Some(tables);
        // Stars of the demonstration: `JE_initWeaponView` calls
        // `initialize_starfield`, which draws from the global `mt_rand`. The C does not
        // reseed between the level and the shop (the same generator
        // thus has the end-of-level state); here the stream starts from the default
        // seed, as at game launch -- it is the only state that is
        // reproducible without replaying the level in the shop. The shots,
        // the energy bar and the mode indicator, however, are identical to the C.
        let stars = crate::STARFIELD_MT.with(|mt| {
            tyrian_sim::world::initialize_starfield(crate::PROCESSOR_TYPE.with(|p| p.get()), &mut mt.borrow_mut())
        });
        // `World::start_level` needs a `LevelSetup`; the demonstration only
        // reads its tables (no map, no event): level 1 of the
        // episode is enough and keeps a single loading path.
        let lo = tyrian_sim::Loadout {
            ship: ShipId(b(0)),
            front_weapon: WeaponId(b(1)),
            front_power: b(2).max(1),
            rear_weapon: WeaponId(b(3)),
            rear_power: b(4).max(1),
            shield: ShieldId(b(5)),
            generator: GeneratorId(b(6)),
            left_sidekick: SidekickId(b(7)),
            right_sidekick: SidekickId(b(8)),
            special: SpecialId(b(9)),
            cash: v[10].max(0) as u32,
        };
        let dir = Path::new(&data_dir.to_string()).to_path_buf();
        match tyrian_sim::setup::load_level(&dir, episode.clamp(1, 5) as u8, 1, 1, 2, lo, 1) {
            Ok(mut setup) => {
                if let Some(m) = self.mods.as_mut() {
                    m.bind_mut().apply_to(&mut setup, 1);
                }
                self.preview = Some(WeaponPreview::new(&setup, stars));
            }
            Err(e) => {
                self.last_error = e.as_str().into();
                return false;
            }
        }
        true
    }

    /// Data cubes of the shop (`level_load.c` `]?` then
    /// `game_menu.c:load_cubes`): `cube_list` (cube numbers of
    /// `cubetxt<episode>.dat`), `cube_max` (readable cubes, 0..4), language
    /// `lang` (`dc_*` texts of `locales/locale_<lang>_cubes.txt` when they
    /// exist, otherwise those of the encrypted file). Returns the number of cubes read.
    #[func]
    fn set_cubes(
        &mut self,
        data_dir: GString,
        episode: i32,
        cube_list: PackedInt32Array,
        cube_max: i32,
        lang: GString,
    ) -> i32 {
        self.cubes.clear();
        let Some(s) = self.session.as_mut() else { return 0 };
        let dir = Path::new(&data_dir.to_string()).to_path_buf();
        let file = std::fs::read(dir.join(format!("cubetxt{episode}.dat")))
            .ok()
            .and_then(|b| EncryptedStringFile::parse(&b).ok());
        let entries = std::fs::read_to_string(dir.join(format!("locales/locale_{lang}_cubes.txt")))
            .map(|t| cube::locale_entries(&t))
            .unwrap_or_default();
        let locale = |k: &str| entries.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        let width =
            |w: &str| with_assets(|a| font::text_width(a, &font::to_cp437(w), TINY_FONT)).unwrap_or(6 * w.len() as i32);
        let wanted = (cube_max.clamp(0, 4) as usize).min(cube_list.len());
        if let Some(file) = file.as_ref() {
            for &index in cube_list.as_slice().iter().take(wanted) {
                match cube::load(file, &locale, episode.clamp(1, 5) as u8, index.max(0) as usize, width) {
                    Some(c) => self.cubes.push(c),
                    None => break,
                }
            }
        }
        // A cube that cannot be found is not offered (the C would read past the end of the file).
        s.cube_max = self.cubes.len() as u8;
        s.cube_max as i32
    }

    /// Text lines of cube `n` (0-based, `cube[n].text[0..last_line]`).
    #[func]
    fn cube_text(&self, n: i32) -> PackedStringArray {
        let mut out = PackedStringArray::new();
        if let Some(c) = self.cubes.get(n.max(0) as usize) {
            for l in &c.text.lines {
                out.push(l.as_str());
            }
        }
        out
    }

    /// A key: "up", "down", "left", "right", "select", "escape".
    /// Returns the action to be run by the screen: "" (nothing), "leave" (with
    /// `choice`), "quit_request", "entry" (with `entry`: 3 Ship Specs,
    /// 5 Options) or "read_cube" (with `cube`, 0-based).
    #[func]
    fn key(&mut self, name: GString) -> VarDictionary {
        let mut d = VarDictionary::new();
        let (Some(t), Some(s), Some(k)) =
            (self.tables.as_ref(), self.session.as_mut(), key_from_name(&name.to_string()))
        else {
            d.set("action", "");
            return d;
        };
        let enters_category =
            matches!(k, ShopKey::Select) && s.menu == tyrian_sim::shop::ShopMenu::Upgrades && s.upgrade_sel() != 9;
        if enters_category {
            let stars = crate::STARFIELD_MT.with(|mt| {
                tyrian_sim::world::initialize_starfield(crate::PROCESSOR_TYPE.with(|p| p.get()), &mut mt.borrow_mut())
            });
            if let Some(p) = self.preview.as_mut() {
                p.init(stars);
            }
        }
        match s.key(t, k) {
            ShopAction::None => d.set("action", ""),
            ShopAction::Leave(i) => {
                d.set("action", "leave");
                d.set("choice", i as i32);
            }
            ShopAction::QuitRequest => d.set("action", "quit_request"),
            ShopAction::FullGameEntry(e) => {
                d.set("action", "entry");
                d.set("entry", e as i32);
            }
            ShopAction::ReadCube(c) => {
                d.set("action", "read_cube");
                d.set("cube", c as i32);
            }
        }
        // `game_menu.c:JE_itemScreen`:
        //  - "JE: {Reset player weapons}" resets `shotMultiPos` to zero at
        //    each pass through the outer loop, that is, at each
        //    key;
        //  - `JE_menuFunction` enters a category by calling
        //    `JE_initWeaponView` (ship position, power at 500,
        //    shots cleared, starfield drawn from the global `mt_rand`): a
        //    single `initialize_starfield` per entry, like the C.
        if let Some(p) = self.preview.as_mut() {
            p.reset_multi_pos();
            p.weapon_mode = s.weapon_mode;
        }
        if !s.purchases.is_empty() {
            crate::ach_api::note_purchases(&std::mem::take(&mut s.purchases));
        }
        // shop_commit_done: purchase paid, the screen plays S_ITEM.
        d.set("committed", s.committed);
        d
    }

    /// Row (`curSel`, 2 = first) under point (x, y) of the current menu,
    /// -1 outside the choices column (`game_menu.c:menu_row_under_mouse`);
    /// `scroll`: `shop_scroll` of the screen (item list).
    #[func]
    fn row_under_mouse(&self, x: i32, y: i32, scroll: i32) -> i32 {
        self.session.as_ref().and_then(|s| s.row_under_mouse(x, y, scroll)).map_or(-1, |r| r as i32)
    }

    /// Hover over a row (`JE_itemScreen`, mouse with no button); true if the
    /// selection changed.
    #[func]
    fn hover_row(&mut self, row: i32) -> bool {
        let (Some(t), Some(s)) = (self.tables.as_ref(), self.session.as_mut()) else {
            return false;
        };
        let changed = s.hover_row(t, row.clamp(0, 255) as u8);
        if changed {
            if let Some(p) = self.preview.as_mut() {
                p.weapon_mode = s.weapon_mode;
            }
        }
        changed
    }

    /// Click on a power arrow (`JE_itemScreen`, zones x 23-36 and
    /// x 119-131, y 149-168): "applied" (power changed), "refused"
    /// (S_CLINK) or "" (outside a weapon category).
    #[func]
    fn click_power_arrow(&mut self, raise: bool) -> GString {
        let (Some(t), Some(s)) = (self.tables.as_ref(), self.session.as_mut()) else {
            return GString::new();
        };
        let result = s.click_power_arrow(t, raise);
        if let Some(p) = self.preview.as_mut() {
            p.reset_multi_pos();
            p.weapon_mode = s.weapon_mode;
        }
        if !s.purchases.is_empty() {
            crate::ach_api::note_purchases(&std::mem::take(&mut s.purchases));
        }
        match result {
            Some(true) => "applied".into(),
            Some(false) => "refused".into(),
            None => GString::new(),
        }
    }

    /// Click on a row (`JE_itemScreen`, `mouseButton > 0`): same
    /// dictionary as `key`, plus `sound` ("click", "clink", "item" or "").
    #[func]
    fn click_row(&mut self, row: i32) -> VarDictionary {
        let mut d = VarDictionary::new();
        let (Some(t), Some(s)) = (self.tables.as_ref(), self.session.as_mut()) else {
            d.set("action", "");
            d.set("sound", "");
            return d;
        };
        let row = row.clamp(0, 255) as u8;
        let enters_category = s.menu == tyrian_sim::shop::ShopMenu::Upgrades
            && s.cur_sel[tyrian_sim::shop::ShopMenu::Upgrades as usize] == row
            && row != 9;
        if enters_category {
            let stars = crate::STARFIELD_MT.with(|mt| {
                tyrian_sim::world::initialize_starfield(crate::PROCESSOR_TYPE.with(|p| p.get()), &mut mt.borrow_mut())
            });
            if let Some(p) = self.preview.as_mut() {
                p.init(stars);
            }
        }
        let (action, sound) = s.click_row(t, row);
        d.set(
            "sound",
            match sound {
                ShopSound::None => "",
                ShopSound::Click => "click",
                ShopSound::Clink => "clink",
                ShopSound::Item => "item",
            },
        );
        match action {
            ShopAction::None => d.set("action", ""),
            ShopAction::Leave(i) => {
                d.set("action", "leave");
                d.set("choice", i as i32);
            }
            ShopAction::QuitRequest => d.set("action", "quit_request"),
            ShopAction::FullGameEntry(e) => {
                d.set("action", "entry");
                d.set("entry", e as i32);
            }
            ShopAction::ReadCube(c) => {
                d.set("action", "read_cube");
                d.set("cube", c as i32);
            }
        }
        if let Some(p) = self.preview.as_mut() {
            p.reset_multi_pos();
            p.weapon_mode = s.weapon_mode;
        }
        if !s.purchases.is_empty() {
            crate::ach_api::note_purchases(&std::mem::take(&mut s.purchases));
        }
        d
    }

    /// State to display: `menu` (0 Full Game, 1 Upgrade Ship, 3 Next Level, 4 category, 7 Data Cubes, 8 reading a cube), `sel`
    /// (`curSel` of the menu, 2 = first row), `cube_sel` (`curSel` of Data
    /// Cubes), `cube_max`, `cube_titles`, `cube_headers`, `cube_faces`,
    /// `category` (`curSel[MENU_UPGRADES]`, 2..8), `items` (items of the
    /// category), `costs` (`JE_getCost` of each), `cash`, `loadout`
    /// (hovered item fitted live, like the C), `left_power`,
    /// `right_power`, `right_power_afford`.
    #[func]
    fn state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let (Some(t), Some(s)) = (self.tables.as_ref(), self.session.as_ref()) else {
            return d;
        };
        d.set("menu", s.menu as i32);
        d.set("sel", s.cur_sel[s.menu as usize] as i32);
        let cat = s.upgrade_sel();
        d.set("category", cat as i32);
        let items: Vec<i32> = s.category_items().iter().map(|&v| v as i32).collect();
        let shop_cat = shop_category(cat);
        let costs: Vec<i32> = items.iter().map(|&i| item_cost(t, shop_cat, i as u16) as i32).collect();
        d.set("items", &PackedInt32Array::from(items.as_slice()));
        d.set("costs", &PackedInt32Array::from(costs.as_slice()));
        d.set("cash", s.cash as i64);
        d.set("loadout", &self.loadout());
        d.set("left_power", s.left_power);
        d.set("right_power", s.right_power);
        d.set("right_power_afford", s.right_power_afford);
        // shop_preview.c:JE_weaponSimUpdate: costs displayed on either side
        // of "POWER: n" (mainint.c:JE_getCost, downgradeCost / upgradeCost)
        // for the hovered weapon, fitted live into the equipment.
        if matches!(cat, 3 | 4) {
            let slot = s.items.weapon[(cat - 3) as usize];
            let (down, up) = weapon_power_costs(t, slot.id.0 as u16, slot.power);
            d.set("downgrade_cost", down);
            d.set("upgrade_cost", up);
        }
        // Category display (game_menu.c, MENU_UPGRADE_SUB):
        // JE_cashLeft, owned item (old_items) and selected item (chosen_items).
        d.set("cash_left", s.cash_left(t));
        let (owned, chosen) = s.owned_and_chosen();
        d.set("owned_item", owned as i32);
        d.set("chosen_item", chosen as i32);
        d.set("menu_choices", s.menu_choices[s.menu as usize] as i32);
        // Trade-in value of the owned item of the category
        // (mainint.c:JE_getValue, power included for weapons).
        if (2..=8).contains(&cat) {
            let (owned, _) = s.owned_and_chosen();
            let power = if matches!(cat, 3 | 4) { s.old_power((cat - 3) as usize) } else { 1 };
            d.set("owned_value", item_value(t, shop_cat, owned as u16, power));
        }
        // Number of rear weapon modes (`weaponPort[...].opnum`):
        // "[/] Rear Weapon Mode" only appears if there are two.
        d.set("rear_mode_count", s.rear_mode_count(t) as i32);
        d.set("cube_sel", s.cur_sel[7] as i32);
        d.set("cube_max", s.cube_max as i32);
        let mut titles = PackedStringArray::new();
        let mut headers = PackedStringArray::new();
        let mut faces = PackedInt32Array::new();
        for c in &self.cubes {
            titles.push(c.title.as_str());
            headers.push(c.header.as_str());
            faces.push(c.face_sprite);
        }
        d.set("cube_titles", &titles);
        d.set("cube_headers", &headers);
        d.set("cube_faces", &faces);
        d
    }

    /// `shop_preview.c:JE_weaponViewFrame` in the VGAScreen of `vga`: starfield,
    /// simulated shots, frame, mode indicator and energy bar.
    /// Returns `weaponSimTime` after increment (`JE_weaponSimUpdate`), on which
    /// the alternation of the rear mode text depends.
    #[func]
    fn weapon_view_frame(&mut self, mut vga: Gd<TyrianVga>) -> i32 {
        let (Some(s), Some(p)) = (self.session.as_ref(), self.preview.as_mut()) else {
            return 0;
        };
        let sprites = p.frame(&s.items, s.weapon_mode);
        let stars = p.stars;
        let power = p.power();
        let sim_time = p.sim_time;
        let mut v = vga.bind_mut();
        let surf = &mut v.surfaces[0];
        surf.fill_rectangle_xy(8, 8, 143, 182, 0);
        // `backgrnd.c:update_and_draw_starfield(VGAScreen, 1)`: the positions
        // have already advanced in `frame()`.
        let mut px = std::mem::take(&mut surf.pixels);
        crate::playfield::draw_stars(&mut px, &stars);
        surf.pixels = px;
        // Shots: `blit_sprite2` of spriteSheet8 (`main:7`) or 12 (`main:11`).
        with_assets(|a| {
            for sp in &sprites {
                let frame = sp.frame as u32;
                let (sheet, idx) = if frame > 500 { ("main:11", frame - 500) } else { ("main:7", frame) };
                if let Some(data) = a.sheet(sheet) {
                    surf.blit_sprite2(sp.x, sp.y, data, idx);
                }
            }
        });
        // `blit_sprite(VGAScreenSeg, 0, 0, OPTION_SHAPES, 12)`: frame, then
        // rear mode indicator (18 on / 19 off).
        with_assets(|a| {
            let idx = if s.weapon_mode == 1 { (18, 19) } else { (19, 18) };
            if let Some(sprite) = a.sprite(tyrian_render::assets::OPTION_SHAPES, 12) {
                surf.blit_sprite(0, 0, sprite);
            }
            if let Some(sprite) = a.sprite(tyrian_render::assets::OPTION_SHAPES, idx.0) {
                surf.blit_sprite(3, 56, sprite);
            }
            if let Some(sprite) = a.sprite(tyrian_render::assets::OPTION_SHAPES, idx.1) {
                surf.blit_sprite(3, 64, sprite);
            }
        });
        draw_power_bar(surf, power as i32);
        sim_time as i32
    }

    /// "Upgrade Ship" row found again on return from Options (the scene is
    /// reloaded; `game_menu.c` keeps `curSel[MENU_UPGRADES]` for the whole visit).
    #[func]
    fn restore_upgrade_row(&mut self, row: i32) {
        let Some(s) = self.session.as_mut() else { return };
        let m = tyrian_sim::shop::ShopMenu::Upgrades as usize;
        s.cur_sel[m] = row.clamp(2, s.menu_choices[m] as i32) as u8;
    }

    /// Current equipment (order of `TyrianSim.set_loadout`).
    #[func]
    fn loadout(&self) -> PackedInt32Array {
        let Some(s) = self.session.as_ref() else {
            return PackedInt32Array::new();
        };
        let i = &s.items;
        PackedInt32Array::from(
            &[
                i.ship.0 as i32,
                i.weapon[0].id.0 as i32,
                i.weapon[0].power as i32,
                i.weapon[1].id.0 as i32,
                i.weapon[1].power as i32,
                i.shield.0 as i32,
                i.generator.0 as i32,
                i.sidekick[0].0 as i32,
                i.sidekick[1].0 as i32,
                self.special as i32,
                s.cash.min(i32::MAX as u64) as i32,
            ][..],
        )
    }
}

/// `vga256d.c:JE_pix`, without writing outside the surface.
fn pix(surf: &mut Surface, x: i32, y: i32, c: i32) {
    if x >= 0 && y >= 0 && x < surf.w as i32 && y < surf.h as i32 {
        surf.pixels[y as usize * surf.w + x as usize] = c as u8;
    }
}

/// `shop_preview.c:JE_weaponViewFrame`, "Power Bar" block: power divided
/// by ten, one step every nine pixels plus a six-pixel gradient.
/// `power` is already clamped to 900 by the caller.
fn draw_power_bar(surf: &mut Surface, power: i32) {
    let temp = power / 10;
    for y in (147 - temp)..=146 {
        let mut temp2 = 113 + (146 - y) / 9 + 2;
        let temp3 = (y + 1) % 6;
        if temp3 == 1 {
            temp2 += 3;
        } else if temp3 != 0 {
            temp2 += 2;
        }
        pix(surf, 141, y, temp2 - 3);
        pix(surf, 142, y, temp2 - 3);
        pix(surf, 143, y, temp2 - 2);
        pix(surf, 144, y, temp2 - 1);
        surf.fill_rectangle_xy(145, y, 149, y, temp2 as u8);
    }
    let top = 147 - temp;
    let c = 113 + (146 - top) / 9 + 4;
    for x in 141..=144 {
        pix(surf, x, top - 1, c - 1);
    }
    surf.fill_rectangle_xy(145, top - 1, 149, top - 1, c as u8);
}
