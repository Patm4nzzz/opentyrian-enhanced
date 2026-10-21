//! `tyrian-write-references`: writes the parity references from
//! `tyrian-sim` (ADR 0127): golden masters (`.gm`), replay golden masters
//! (`.play.gm`) and recorded replays (`.rep`).
//!
//! `--check`: rewrites each reference and compares it byte for byte to the
//! tracked file (non-zero exit code at the first difference, continuous
//! integration). `--out DIR`: writes the rewritten references into DIR, under the
//! same name (after an intended gameplay change, when the C code is gone).

use anyhow::{anyhow, bail, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use tyrian_golden::writer::{first_difference, read_input_script, tick_count, write_recorded_replay, write_reference};

#[derive(Parser)]
#[command(about = "Writes the golden masters, readbacks and replays from tyrian-sim")]
struct Args {
    /// `data/` folder of the original game.
    #[arg(long, default_value = "../data")]
    data: PathBuf,
    /// Compares each rewritten reference to the tracked file, byte for byte.
    #[arg(long)]
    check: bool,
    /// Folder where to write the rewritten references.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Recorded replay: `NAME.rep=SCRIPT.in` (header taken from NAME.rep,
    /// frames replayed from the input script of `c-final:utils/golden/record_replay.sh`).
    #[arg(long = "record", value_name = "REP=SCRIPT")]
    record: Vec<String>,
    /// References: `.gm` (scenario taken from the file) or `.play.gm` (replay of the neighboring `.rep`).
    files: Vec<PathBuf>,
}

/// Rewritten `.rep`: header of the tracked file, frames replayed from the script.
fn write_rep(spec: &str, data: &Path) -> Result<(PathBuf, Vec<u8>)> {
    let (rep, script) = spec.split_once('=').ok_or_else(|| anyhow!("--record {spec}: REP=SCRIPT expected"))?;
    let rep = PathBuf::from(rep);
    let bytes = std::fs::read(&rep).map_err(|e| anyhow!("{}: {e}", rep.display()))?;
    let header = tyrian_formats::replay::Replay::parse(&bytes).map_err(|e| anyhow!("{}: {e}", rep.display()))?;
    let masks = read_input_script(Path::new(script))?;
    Ok((rep, write_recorded_replay(&header, &masks, data)?))
}

fn main() -> Result<()> {
    let a = Args::parse();
    if a.files.is_empty() && a.record.is_empty() {
        bail!("no reference given");
    }
    if !a.check && a.out.is_none() {
        bail!("specify --check or --out DIR");
    }
    if let Some(dir) = &a.out {
        std::fs::create_dir_all(dir)?;
    }
    let total = a.files.len() + a.record.len();
    let (mut same, mut ticks, mut bytes) = (0usize, 0usize, 0usize);
    let mut failed = Vec::new();
    let mut outputs: Vec<(PathBuf, Result<Vec<u8>>)> = Vec::new();
    for f in &a.files {
        outputs.push((f.clone(), write_reference(f, &a.data).map(String::into_bytes)));
    }
    for spec in &a.record {
        match write_rep(spec, &a.data) {
            Ok((rep, out)) => outputs.push((rep, Ok(out))),
            Err(e) => outputs.push((PathBuf::from(spec), Err(e))),
        }
    }
    for (f, written) in outputs {
        let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let written = match written {
            Ok(t) => t,
            Err(e) => {
                println!("{name}: FAILED {e:#}");
                failed.push(name);
                continue;
            }
        };
        if let Some(dir) = &a.out {
            std::fs::write(dir.join(&name), &written)?;
        }
        if a.check {
            let tracked = std::fs::read(&f)?;
            let is_text = name.ends_with(".gm");
            let n = if is_text { tick_count(&String::from_utf8_lossy(&written)) } else { 0 };
            if tracked == written {
                same += 1;
                ticks += n;
                bytes += tracked.len();
                println!("{name}: identical ({n} ticks, {} bytes)", tracked.len());
            } else {
                let at = if is_text {
                    format!(
                        "line {}",
                        first_difference(&String::from_utf8_lossy(&tracked), &String::from_utf8_lossy(&written))
                            .unwrap_or(0)
                    )
                } else {
                    format!(
                        "byte {}",
                        tracked
                            .iter()
                            .zip(written.iter())
                            .position(|(x, y)| x != y)
                            .unwrap_or(tracked.len().min(written.len()))
                    )
                };
                println!("{name}: DIFFERENT from {at}");
                failed.push(name);
            }
        }
    }
    if a.check {
        println!("total: {same}/{total} references identical byte for byte, {ticks} ticks, {bytes} bytes");
    }
    if !failed.is_empty() {
        bail!("{} failed reference(s): {}", failed.len(), failed.join(", "));
    }
    Ok(())
}
