//! Dump of the sections of a levelsN.dat. Usage: episode_dump FILE [SECTION...]
use tyrian_formats::episode::EpisodeScript;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let s = EpisodeScript::parse(&std::fs::read(&args[1]).unwrap()).unwrap();
    let want: Vec<usize> = args[2..].iter().map(|a| a.parse().unwrap()).collect();
    for sec in s.sections() {
        if !want.is_empty() && !want.contains(&sec.number) {
            continue;
        }
        println!("== section {}: {}", sec.number, sec.title);
        for l in &sec.lines {
            if l.starts_with(']') {
                println!("   {l}");
            }
        }
    }
}
