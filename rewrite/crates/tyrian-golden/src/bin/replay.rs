//! `tyrian-replay --data ../data NAME.rep NAME.play.gm`: replays a replay of the
//! C version in `tyrian-sim` and compares each tick to the golden master of the
//! C replay of the same file (ADR 0090). Non-zero exit code if a single
//! tick or the end differs.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tyrian_golden::{compare_replay, GoldenMaster};

#[derive(Parser)]
#[command(about = "Replays a C .rep in tyrian-sim and compares it to the golden master of the C replay")]
struct Args {
    #[arg(long, default_value = "../data")]
    data: PathBuf,
    rep: PathBuf,
    gm: PathBuf,
}

fn main() -> Result<()> {
    let a = Args::parse();
    let gm = GoldenMaster::load(&a.gm)?;
    let name = a.rep.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (r, hash) = compare_replay(&a.rep, &gm, &a.data, &name)?;
    let ok = r.matched_prefix == r.ticks && r.end_matches;
    println!(
        "{name}: {}/{} identical ticks, end {} (C: {}), final Rust hash {hash:016x}, divergences {:?}",
        r.matched_prefix,
        r.ticks,
        if r.end_matches { "identical" } else { "DIFFERENT" },
        gm.end_reason,
        r.first_divergence.iter().filter_map(|(g, t)| t.map(|t| format!("{g}@{t}"))).collect::<Vec<_>>()
    );
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}
