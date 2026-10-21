//! Writing of the parity references by the rewrite (ADR 0127).
//!
//! A `.gm` golden master mixes two kinds of lines: the scenario (header
//! written at the start of the level, input mask and mouse of each tick) and
//! the state sampled at each tick (positions, cash, counters, fingerprints).
//! [`write_golden_master`] keeps the scenario and recomputes all the state with
//! `tyrian-sim`, in the exact format of `src/golden_master.c` (`write_tick`,
//! `finish`). [`write_replay_master`] does the same for the golden master
//! of a `.rep` replay (`NAME.play.gm`). As long as the C version exists,
//! the output must be byte-for-byte identical to the files it
//! recorded (`tyrian-write-references --check`).

use anyhow::{anyhow, bail, Result};
use std::fmt::Write as _;
use std::path::Path;
use tyrian_sim::{FrameRecord, PlayerInput};

use crate::GoldenMaster;

/// Scenario lines of a `.gm`: everything before the first tick line.
fn header_lines(text: &str) -> Vec<&str> {
    text.lines().take_while(|l| !l.starts_with(|c: char| c.is_ascii_digit())).collect()
}

/// `golden_master.c:write_tick`, columns of the original format.
fn tick_line(out: &mut String, r: &FrameRecord) {
    out.push_str(&r.golden_line());
}

/// `.gm` golden master rewritten from `tyrian-sim`: header and inputs
/// taken from the file `path`, state of each tick recomputed.
pub fn write_golden_master(path: &Path, data_dir: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    let gm = GoldenMaster::load_chained(path, data_dir)?;
    let mut sim = gm.make_sim(data_dir)?;
    let mut out = String::new();
    for l in header_lines(&text) {
        out.push_str(l);
        out.push('\n');
    }
    let n = gm.ticks.len();
    for (k, exp) in gm.ticks.iter().enumerate() {
        if sim.state().level_ended {
            bail!(
                "{}: level ended on the Rust side at tick {}, before the end of the script",
                path.display(),
                exp.tick
            );
        }
        // golden_master_inject: script exhausted, the open tick is rewritten
        // without a new iteration (same rule as `compare`).
        let script_tail = gm.end_reason == "script" && k + 1 == n && k > 0;
        let mode = gm.mode_ticks.get(k);
        if !script_tail {
            if let Some(m) = mode {
                sim.set_mouse(tyrian_sim::modes::MouseInput { mask: m.mouse_mask, dx: m.mouse_dx, dy: m.mouse_dy });
            }
            sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        }
        let mut r = sim.frame_record();
        r.tick = k as u64;
        if script_tail {
            r.mask = exp.mask;
        }
        tick_line(&mut out, &r);
        if let Some(m) = mode {
            // Format v2: mouse injected (scenario), player 2 and hash_mode recomputed.
            let got = sim.mode_record();
            let _ = write!(
                out,
                " {:02x} {} {} {} {} {:016x}",
                m.mouse_mask, m.mouse_dx, m.mouse_dy, got.p2x, got.p2y, got.h_mode
            );
        }
        out.push('\n');
    }
    if gm.endless_end.is_some() {
        let w = sim.world();
        let lives = w.endless_lives.ok_or_else(|| anyhow!("{}: missing Endless lives", path.display()))?;
        // golden_master.c:finish writes endless.c:endless_score, cumulated since the
        // first wave: a chained wave adds the score of the previous ones.
        let earlier = GoldenMaster::chain_predecessor(path)
            .filter(|p| p.exists())
            .and_then(|p| GoldenMaster::load(&p).ok())
            .and_then(|g| g.endless_end.map(|e| e.score))
            .unwrap_or(0);
        let score =
            if gm.header.endless.is_some() && earlier > 0 { earlier + w.endless_score } else { w.endless_score };
        let _ = writeln!(out, "endless_end score {} lives {}:{}", score, lives[0], lives[1]);
    }
    let why = if sim.state().level_ended { "level" } else { gm.end_reason.as_str() };
    if why == "level" && gm.end_reason != "level" || why != "level" && gm.end_reason == "level" {
        bail!("{}: end '{why}' on the Rust side, '{}' in the script", path.display(), gm.end_reason);
    }
    let _ = writeln!(out, "end {why} ticks {n}");
    Ok(out)
}

/// Golden master of the replay of a `.rep` (`NAME.play.gm`) rewritten from
/// `tyrian-sim`: header taken from `play_gm`, state recomputed by replaying `rep`.
pub fn write_replay_master(rep: &Path, play_gm: &Path, data_dir: &Path) -> Result<String> {
    let text = std::fs::read_to_string(play_gm).map_err(|e| anyhow!("{}: {e}", play_gm.display()))?;
    let mut run = tyrian_sim::replay::ReplayRun::load(data_dir, rep).map_err(|e| anyhow!(e))?;
    let mut out = String::new();
    for l in header_lines(&text) {
        out.push_str(l);
        out.push('\n');
    }
    let mut n = 0u64;
    while run.step() {
        let mut r = run.sim.frame_record();
        r.tick = n;
        tick_line(&mut out, &r);
        out.push('\n');
        n += 1;
    }
    // golden_master.c: "level" (reallyEndLevel) or "replay" (file exhausted).
    let why = if run.sim.state().level_ended { "level" } else { "replay" };
    let _ = writeln!(out, "end {why} ticks {n}");
    Ok(out)
}

/// The `.rep` of which `play_gm` is the replay (`NAME.play.gm` → `NAME.rep`).
pub fn replay_of(play_gm: &Path) -> Option<std::path::PathBuf> {
    let name = play_gm.file_name()?.to_str()?;
    let stem = name.strip_suffix(".play.gm")?;
    Some(play_gm.with_file_name(format!("{stem}.rep")))
}

/// A tracked reference (`.gm` or `.play.gm`) rewritten from `tyrian-sim`.
pub fn write_reference(path: &Path, data_dir: &Path) -> Result<String> {
    match replay_of(path) {
        Some(rep) => write_replay_master(&rep, path, data_dir),
        None => write_golden_master(path, data_dir),
    }
}

/// Number of tick lines of a golden master text.
pub fn tick_count(text: &str) -> usize {
    text.lines().filter(|l| l.starts_with(|c: char| c.is_ascii_digit())).count()
}

/// Line of the first difference between two texts (1-based), `None` if identical.
pub fn first_difference(a: &str, b: &str) -> Option<usize> {
    if a == b {
        return None;
    }
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    Some(la.iter().zip(lb.iter()).position(|(x, y)| x != y).unwrap_or(la.len().min(lb.len())) + 1)
}

/// Input script from `utils/golden/inputs/*.in` ("ticks mask", `#` = comment).
pub fn read_input_script(path: &Path) -> Result<Vec<u8>> {
    let text = std::fs::read_to_string(path).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    let mut masks = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut t = line.split_whitespace();
        let (Some(n), Some(m)) = (t.next(), t.next()) else { bail!("{}: invalid line '{line}'", path.display()) };
        let n: usize = n.parse()?;
        let m = u8::from_str_radix(m, 16)?;
        masks.extend(std::iter::repeat_n(m, n));
    }
    Ok(masks)
}

/// `.rep` recorded from `tyrian-sim` (`replay.c:replay_capture_frame`):
/// header (version, level, seed, equipment, mutators) taken from
/// `header`, frames recomputed by playing the `masks` script on the keyboard.
/// Each frame samples, at the top of the iteration, the position of the two ships
/// and the four buttons; when the script is exhausted, the next iteration
/// is still sampled with the last keys, then the recording
/// stops (`golden_master_inject` → `finish("script")`).
pub fn write_recorded_replay(
    header: &tyrian_formats::replay::Replay,
    masks: &[u8],
    data_dir: &Path,
) -> Result<Vec<u8>> {
    use tyrian_formats::replay::ReplayFrame;
    // title_screen.c: in two-player, `initialDifficulty = difficultyLevel`
    // then `difficultyLevel++`; the `.rep` only keeps the second.
    let initial_difficulty = if header.game_mode == 2 { header.difficulty - 1 } else { header.difficulty };
    let (_, mut sim) =
        tyrian_sim::replay::ReplayCursor::start_with_initial_difficulty(data_dir, header.clone(), initial_difficulty)
            .map_err(|e| anyhow!(e))?;
    let p = &sim.world().players;
    let (x, y) = (p[0].x, p[0].y);
    sim.replay_begin(x, y);
    let mut frames = Vec::new();
    let mut last = 0u8;
    for k in 0..=masks.len() {
        if sim.state().level_ended {
            break;
        }
        let mask = masks.get(k).copied().unwrap_or(last);
        last = mask;
        let w = sim.world();
        let buttons = (mask >> 4) & 0x0f;
        frames.push(ReplayFrame {
            x: w.players[0].x as i16,
            y: w.players[0].y as i16,
            buttons,
            x2: w.players[1].x as i16,
            y2: w.players[1].y as i16,
            buttons2: buttons,
        });
        if k == masks.len() {
            break;
        }
        sim.step([PlayerInput(mask), PlayerInput(0)]);
    }
    let rep = tyrian_formats::replay::Replay { frames, ..header.clone() };
    Ok(rep.write())
}
