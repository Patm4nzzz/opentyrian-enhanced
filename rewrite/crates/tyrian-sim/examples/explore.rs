// One-off data exploration (not versioned).
use tyrian_formats::episode::EpisodeScript;
use tyrian_formats::hdt::HdtFile;
use tyrian_formats::sprite::MainShapes;

fn name(n: &tyrian_formats::hdt::PascalName) -> String {
    String::from_utf8_lossy(&n.bytes[..(n.len as usize).min(30)]).to_string()
}

fn main() {
    let d = std::path::Path::new("../data");
    let hdt = HdtFile::parse(&std::fs::read(d.join("tyrian.hdt")).unwrap()).unwrap();
    let it = &hdt.items;
    println!("== sidekicks");
    for (i, o) in it.sidekicks.iter().enumerate() {
        println!(
            "{i} {} tr={} opt={} wport={} wpnum={} ammo={} pwr={} cost={}",
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
    println!("== specials");
    for (i, s) in it.specials.iter().enumerate() {
        println!("{i} {} stype={} wpn={} pwr={}", name(&s.name), s.stype, s.wpn, s.pwr);
    }
    println!("== weapons sg>60000");
    for id in 0..3000 {
        if let Some(w) = it.weapon(id) {
            if w.sg.iter().any(|&g| g > 60000) {
                println!("{id} sg={:?} max={} multi={}", w.sg, w.max, w.multi);
            }
        }
    }
    let shp = MainShapes::parse(&std::fs::read(d.join("tyrian.shp")).unwrap()).unwrap();
    for t in [5usize, 6] {
        let sizes: Vec<String> = shp.simple[t]
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| match &e.sprite {
                Some(s) => format!("{i}:{}x{}", s.width, s.height),
                None => format!("{i}:-"),
            })
            .collect();
        println!("== table {t} ({} entries) {}", sizes.len(), sizes.join(" "));
    }
    for ep in 1..=5 {
        let sc = EpisodeScript::parse(&std::fs::read(d.join(format!("levels{ep}.dat"))).unwrap()).unwrap();
        println!("== ep{ep}");
        for (i, s) in sc.sections().iter().enumerate().take(6) {
            println!("section {} : {:?}", i + 1, s);
        }
    }
}
