//! Campaign non-regression: each level of the chained C campaigns
//! (`rewrite/golden/campaign/NAME-K.gm`, recorded by
//! `c-final:utils/golden/record_campaign_chain.sh`: level → debriefing → episode
//! script → shop → next level, episode transition included) is
//! replayed from the header written by the C code and from the state inherited from the
//! previous level, and must stay identical tick for tick.

use std::path::{Path, PathBuf};
use tyrian_golden::{compare, GoldenMaster, GROUPS};
use tyrian_sim::world::WeaponSlot;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn chained_campaign_levels_match_c() {
    let data = root().join("data");
    let dir = root().join("rewrite/golden/campaign");
    if !data.join("tyrian.hdt").exists() || !dir.exists() {
        eprintln!("data or campaign golden masters missing: test skipped");
        return;
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "gm"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for f in files {
        let gm = GoldenMaster::load_chained(&f, &data).expect("golden master");
        assert!(gm.header.campaign.is_some(), "{}: missing campaign line", f.display());
        if let Some(d) = gm.derived_difficulty {
            assert_eq!(d, gm.header.difficulty, "{}: difficulty (adjust_difficulty)", f.display());
            eprintln!("{}: difficulty {d} recovered", f.display());
        }
        // Campaigns with a scripted shop (`NAME.keys`, GOLDEN_SHOP): checked
        // by `scripted_shop_matches_c` (resale included), not by the
        // no-resale purchase model of the autopilot.
        let scripted = keys_file(&f).is_some_and(|k| k.exists());
        if let Some(prev) = GoldenMaster::chain_predecessor(&f).filter(|p| p.exists() && !scripted) {
            check_shop_cash(&prev, &gm, &data);
        }
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let r = compare(&gm, &data, &name, |_, _, _| {}).expect("comparison");
        for g in GROUPS {
            assert_eq!(r.first_divergence[g], None, "{name}: group {g}");
        }
        assert!(r.end_matches && r.ended_early_at.is_none(), "{name}: level end");
        assert_eq!(r.matched_prefix, r.ticks, "{name}");
    }
}

/// Shop between two levels: the C campaign is driven by
/// `autopilot.c:autopilot_buy_upgrades`, which debits `JE_getCost` of each
/// new item without reselling the old one. The end-of-level cash of level N
/// (simulated in Rust) minus the `shop::item_cost` prices of the changed items
/// must give back the cash of the N+1 header written by the C code.
fn check_shop_cash(prev: &Path, gm: &GoldenMaster, data: &Path) {
    use tyrian_sim::shop::{item_cost, ShopCategory as C};
    use tyrian_sim::PlayerInput;
    let prev_gm = GoldenMaster::load_chained(prev, data).expect("previous level");
    let mut sim = prev_gm.make_sim(data).expect("sim");
    for t in &prev_gm.ticks {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
    }
    let w = sim.world();
    let p = &w.players[0];
    let (o, n) = (&p.items, &gm.header.loadout);
    let changes = [
        (C::Generator, o.generator.0, n.generator.0),
        (C::Shield, o.shield.0, n.shield.0),
        (C::FrontWeapon, o.weapon[0].id.0, n.front_weapon.0),
        (C::RearWeapon, o.weapon[1].id.0, n.rear_weapon.0),
        (C::LeftSidekick, o.sidekick[0].0, n.left_sidekick.0),
        (C::RightSidekick, o.sidekick[1].0, n.right_sidekick.0),
    ];
    let mut cash = p.cash as i64;
    for (cat, own, new) in changes {
        if own != new {
            cash -= item_cost(&w.tables, cat, new as u16);
        }
    }
    let want = gm.header.campaign.unwrap().cash as i64;
    assert_eq!(cash, want, "{}: money after the shop", prev.display());
}

/// `NAME-K.gm` -> `NAME.keys` (GOLDEN_SHOP script of the campaign).
fn keys_file(gm: &Path) -> Option<PathBuf> {
    let stem = gm.file_stem()?.to_str()?;
    let (prefix, _) = stem.rsplit_once('-')?;
    Some(gm.with_file_name(format!("{prefix}.keys")))
}

/// Interactive shop: the C campaign `e1-shop` goes through the shop of
/// section 4 with a key script (GOLDEN_SHOP) that resells the
/// previously owned cannon and buys another cannon while raising its power. The same
/// keys, played by `tyrian_sim::shop::ShopSession` on the end-of-level-1
/// state simulated in Rust, must give back the equipment and cash of the
/// level 2 header written by the C code.
#[test]
fn scripted_shop_matches_c() {
    use tyrian_formats::episode::EpisodeScript;
    use tyrian_sim::campaign::{run_script, ScriptContext, ScriptStop};
    use tyrian_sim::shop::{ShopAction, ShopKey, ShopSession};
    use tyrian_sim::PlayerInput;
    let data = root().join("data");
    let dir = root().join("rewrite/golden/campaign");
    if !data.join("tyrian.hdt").exists() || !dir.join("e1-shop.keys").exists() {
        eprintln!("data or e1-shop campaign missing: test skipped");
        return;
    }
    let first = GoldenMaster::load_chained(&dir.join("e1-shop-1.gm"), &data).unwrap();
    let second = GoldenMaster::load(&dir.join("e1-shop-2.gm")).unwrap();
    let mut sim = first.make_sim(&data).unwrap();
    for t in &first.ticks {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
    }
    let w = sim.world();
    let p = &w.players[0];
    // Episode script: TYRIAN (section 3) leads to the shop of section 4.
    let script = EpisodeScript::parse(&std::fs::read(data.join("levels1.dat")).unwrap()).unwrap();
    let ctx =
        ScriptContext { initial_difficulty: 2, all_players_alive: true, ship: p.items.ship.0, ..Default::default() };
    let (stop, fx) = run_script(&script, 1, 4, &ctx, [true; 5]);
    let ScriptStop::Shop { items_available, .. } = stop else { panic!("{stop:?}") };
    let mut shop = ShopSession::new(&w.tables, p.items, p.cash, &items_available, fx.map.len());
    let keys = std::fs::read_to_string(dir.join("e1-shop.keys")).unwrap();
    for c in keys.chars().filter(|c| !c.is_whitespace()) {
        let a = shop.key(&w.tables, ShopKey::from_script(c).expect("key"));
        assert_eq!(a, ShopAction::None, "key {c}");
    }
    // The autopilot then exits via "Play Next Level", the first destination.
    shop.cur_sel[0] = 6;
    assert_eq!(shop.key(&w.tables, ShopKey::Select), ShopAction::None);
    assert_eq!(shop.key(&w.tables, ShopKey::Select), ShopAction::Leave(0));
    let want = &second.header.loadout;
    let got = &shop.items;
    assert_eq!(
        (got.weapon[0], got.weapon[1], got.shield, got.generator, got.sidekick),
        (
            WeaponSlot { id: want.front_weapon, power: want.front_power },
            WeaponSlot { id: want.rear_weapon, power: want.rear_power },
            want.shield,
            want.generator,
            [want.left_sidekick, want.right_sidekick]
        ),
        "equipment after the shop"
    );
    assert_eq!(shop.cash, second.header.campaign.unwrap().cash as u64, "money after the shop (resale included)");
    eprintln!("scripted shop: {:?} cash {} (level end {} -> {})", got.weapon[0], shop.cash, p.cash, shop.cash);
}
