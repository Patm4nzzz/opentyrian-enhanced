// Prices and weapons of the shop items used by the golden master scenarios
// "shop / sidekicks / special shots" (exploration).
use tyrian_formats::hdt::{HdtFile, ItemTables};
use tyrian_formats::lvl::LvlFile;

fn name(n: &tyrian_formats::hdt::PascalName) -> String {
    String::from_utf8_lossy(&n.bytes[..(n.len as usize).min(30)]).to_string()
}

fn dump(label: &str, it: &ItemTables) {
    println!("=== {label}");
    for i in [9usize, 16, 20, 30, 31] {
        let o = &it.sidekicks[i];
        println!(
            "option {i} {} tr={} opt={} wport={} wpnum={} ammo={} pwr={} cost={}",
            name(&o.name),
            o.tr,
            o.option,
            o.wport,
            o.wpnum,
            o.ammo,
            o.pwr,
            o.cost
        );
    }
    for i in 0..7 {
        println!("generator {i} cost={} power={}", it.generators[i].cost, it.generators[i].power);
    }
    for i in 0..10 {
        println!("shield {i} cost={}", it.shields[i].cost);
    }
    for i in [1usize, 2, 7, 10, 11, 13, 29, 41, 45] {
        let p = &it.weapon_ports[i];
        println!("port {i} {} cost={}", name(&p.name), p.cost);
    }
    for id in [470usize, 88] {
        if let Some(w) = it.weapon(id) {
            println!("weapon {id} sg={:?} max={} multi={} del={}", w.sg, w.max, w.multi, w.shotrepeat);
        }
    }
}

fn main() {
    let d = std::path::Path::new("../data");
    let hdt = HdtFile::parse(&std::fs::read(d.join("tyrian.hdt")).unwrap()).unwrap();
    dump("hdt", &hdt.items);
    let lvl = LvlFile::parse(&std::fs::read(d.join("tyrian5.lvl")).unwrap()).unwrap();
    if let Some(t) = lvl.item_tables() {
        dump("tyrian5.lvl", &t.unwrap());
    }
}
