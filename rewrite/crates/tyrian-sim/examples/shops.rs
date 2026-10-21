// Inventory of the shops (`]I[`) of each episode: used to choose the
// golden master scenarios that exercise sidekicks and special shots.
use tyrian_formats::episode::EpisodeScript;

fn main() {
    let d = std::path::Path::new("../data");
    for ep in 1..=5 {
        let sc = EpisodeScript::parse(&std::fs::read(d.join(format!("levels{ep}.dat"))).unwrap()).unwrap();
        for s in sc.sections() {
            for shop in &s.shops {
                let cats: Vec<String> =
                    shop.categories.iter().map(|c| format!("{}={:?}", c.label.trim(), c.items)).collect();
                println!("ep{ep} section {} : {}", s.number, cats.join(" | "));
            }
        }
    }
}
