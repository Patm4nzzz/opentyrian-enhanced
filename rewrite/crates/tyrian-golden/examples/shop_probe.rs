//! Shop diagnostic: for each chained level, end-of-level cash of the
//! previous level (simulated), equipment before/after the shop (C headers) and
//! cash obtained by applying `shop::enter_category` + `shop::buy` to each
//! changed item. Usage: shop_probe N-1.gm N.gm
use std::path::Path;
use tyrian_golden::GoldenMaster;
use tyrian_sim::shop::{buy, enter_category, ShopCategory as C};
use tyrian_sim::PlayerInput;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let data = Path::new("../data");
    let prev = GoldenMaster::load_chained(Path::new(&a[1]), data)?;
    let next = GoldenMaster::load(Path::new(&a[2]))?;
    let mut sim = prev.make_sim(data)?;
    for t in &prev.ticks {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(t.mask), PlayerInput(0)]);
    }
    let w = sim.world();
    let p = &w.players[0];
    let (o, n) = (&p.items, &next.header.loadout);
    println!("end N: cash {}; header N+1: cash {}", p.cash, next.header.campaign.unwrap().cash);
    println!(
        "before: front {:?} rear {:?} shield {} gen {} sk {:?}",
        o.weapon[0], o.weapon[1], o.shield, o.generator, o.sidekick
    );
    println!(
        "after: front {}:{} rear {}:{} shield {} gen {} sk {}:{}",
        n.front_weapon,
        n.front_power,
        n.rear_weapon,
        n.rear_power,
        n.shield,
        n.generator,
        n.left_sidekick,
        n.right_sidekick
    );
    let t = &w.tables;
    let mut cash = p.cash;
    let steps: [(C, u16, u8, u16, u8); 6] = [
        (C::Generator, o.generator.0 as u16, 1, n.generator.0 as u16, 1),
        (C::Shield, o.shield.0 as u16, 1, n.shield.0 as u16, 1),
        (C::FrontWeapon, o.weapon[0].id.0 as u16, o.weapon[0].power, n.front_weapon.0 as u16, o.weapon[0].power),
        (C::RearWeapon, o.weapon[1].id.0 as u16, o.weapon[1].power, n.rear_weapon.0 as u16, o.weapon[1].power),
        (C::LeftSidekick, o.sidekick[0].0 as u16, 1, n.left_sidekick.0 as u16, 1),
        (C::RightSidekick, o.sidekick[1].0 as u16, 1, n.right_sidekick.0 as u16, 1),
    ];
    for (cat, own, op, new, np) in steps {
        if own != new {
            let c2 = enter_category(t, cash, cat, own, op);
            cash = buy(t, c2, cat, new, np).unwrap_or(cash);
            println!("{cat:?} {own} -> {new}: cash {cash}");
        }
    }
    println!("computed cash (interactive shop) {cash}");
    // Autopilot (autopilot.c:autopilot_buy_upgrades): cash -= JE_getCost(new item).
    let mut auto = p.cash as i64;
    for (cat, own, _, new, _) in steps {
        if own != new {
            auto -= tyrian_sim::shop::item_cost(t, cat, new);
        }
    }
    println!("computed cash (autopilot) {auto}");
    Ok(())
}
