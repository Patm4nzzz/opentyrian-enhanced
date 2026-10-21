# ADR 0001 — `master` is the oracle, not `wip/sept-2026`

Date: 2026-10-04 — status: accepted (decision made alone, autonomous mandate).

## Context
The mandate asks to start from the current C version. Two candidates: `master` (394b1fb7, v0.9.13-3, released) and `wip/sept-2026` (backup of the uncommitted work in the WSL copy, pushed on 2026-10-04, 245 modified files, 128 of them in `src/`).

## Decision
The `rewrite/godot-rust` branch starts from `master`. The C version of `master` is the behavior reference (oracle) for the golden masters. `wip/sept-2026` is neither merged nor modified.

## Reasons
- `master` compiles as is and matches a released version; `wip` is a snapshot of work in progress, not validated.
- The format audit verified that the data readers and the binary files in `data/` are identical between the two: the choice does not affect the import.
- The golden master covers classic gameplay; the differences in `wip` (separate generator for effects, re-enabled anomalies) concern the Enhanced modes.

## Consequences
If `wip` is merged into `master` later, regenerate the reference replays (phase 2) and check that they stay identical in classic mode.
