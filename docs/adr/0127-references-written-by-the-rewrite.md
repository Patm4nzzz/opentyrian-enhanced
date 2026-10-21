# ADR 0127 — Parity references written by the rewrite

Date: 2026-10-05 (card t_401e6b69, point 7)

## Context

The parity references (`rewrite/golden/**/*.gm`, `rewrite/golden/replays/*.play.gm`
and `*.rep`) are recorded by the instrumented C binary (`make GOLDEN=1`,
`utils/golden/record*.sh`). The C version is to be removed after the first release: without it,
there is no longer any way to produce a reference after an intended change to the
gameplay, and the parity safety net would freeze.

A `.gm` mixes two kinds of lines: the **scenario** (header written at the start of the
level — episode, section, seed, equipment, modes — and, at each tick,
the input mask and the mouse that were injected) and the **state** sampled at each tick
(positions, armor, money, counters, seven fingerprints, player 2 and
`hash_mode` in v2, Endless score, end reason). A `.rep` carries a game
header and, per frame, the positions and buttons sampled at the top of the iteration.

## Decision

Binary `tyrian-write-references` in `tyrian-golden`
(`src/bin/write_references.rs`, functions in `src/writer.rs`):

- **`.gm`**: scenario taken from the file, state recomputed by `tyrian-sim`, in the exact
  format of `golden_master.c:write_tick` and `finish` (16-digit hexadecimal,
  `endless_end` line, `end level|script ticks N`); chained campaign levels
  take over the state of the previous level like the comparator
  (`GoldenMaster::load_chained`).
- **`.play.gm`**: header taken over, ticks recomputed by re-reading the neighboring `.rep`
  (`ReplayRun`), `level` or `replay` ending.
- **`.rep`** (`--record NAME.rep=SCRIPT.in`): header taken from the file,
  frames recomputed by playing the input script of `record_replay.sh`
  (`replay.c:replay_capture_frame`: positions of the two ships and four
  buttons at the top of the iteration, one more frame after the script is
  exhausted). In two-player mode, `initialDifficulty` is the `.rep` difficulty
  minus one (`title_screen.c` raises `difficultyLevel` after setting it):
  `ReplayCursor::start_with_initial_difficulty`.
- `--check` compares each output **byte for byte** with the tracked file;
  `--out DIR` writes the outputs.

What the tool does not produce: the scenario itself. The inputs of the "autopilot"
recordings come from the C autopilot; they are kept in the tracked `.gm` files
and replayed as is. A new scenario is written as an input script
(`utils/golden/inputs/*.in`) with a header copied from a neighboring reference.
The achievement logs (`golden/achievements/`) and the Endless `.egm`/`.agm` files
remain out of scope (checked by `tests/achievements.rs` and `tests/endless.rs`).

## Evidence

**Byte-for-byte** comparison (not just tick for tick), on the tracked references,
regenerated from the corrected C version (point 6, Zinglon):
76/76 identical — 66 golden masters (319,966 ticks), 5 `.play.gm` replays
(5,837 ticks) and 5 `.rep`, 52,754,263 bytes. Control test on five replays
recorded again by the C version the same day (seeds drawn from the clock,
hence different from the tracked files): 10/10 identical (`.rep` and
`.play.gm`). The CI (`rewrite.yml`, step "References rewritten by
tyrian-sim") reruns the proof on every push.

## Usage

```
cd rewrite
# proof (CI): rewrite and compare with the tracked files
cargo run --release -p tyrian-golden --bin tyrian-write-references -- --check --data ../data \
  --record golden/replays/e1-tyrian-sweep.rep=../utils/golden/inputs/sweep.in \
  golden/**/*.gm
# after an intended gameplay change, without the C version: write, then review and commit
cargo run --release -p tyrian-golden --bin tyrian-write-references -- --out /tmp/refs --data ../data golden/e1-nort-pulse.gm
```

After the removal of the C version, an intended change is handled as follows: `--out`, review
of the references diff (ticks affected, groups), copy into `rewrite/golden/`,
then `--check` goes green again. As long as the C version exists, its recordings
remain the source and `--check` proves the tool.

## Consequences

The safety net no longer depends on the C version to exist; it depends on `tyrian-sim`, which
makes it circular after the removal of the C version: an unintended change that
went through `--out` without review would be accepted. Reviewing the diff of the
references then replaces the comparison with the C version.
