// One-off exploration: weapons of the specials exercised by the golden masters
// e1-special-* (Pearl Wind 620, Blade Field 623, Dual Vulcan 658).
use tyrian_formats::hdt::HdtFile;

fn main() {
    let d = std::path::Path::new("../data");
    let hdt = HdtFile::parse(&std::fs::read(d.join("tyrian.hdt")).unwrap()).unwrap();
    for id in [620usize, 623, 658] {
        if let Some(w) = hdt.items.weapon(id) {
            println!("{id} {:?}", w);
        }
    }
}
