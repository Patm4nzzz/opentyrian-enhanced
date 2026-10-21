# ADR 0090 — C version `.rep` replays played back by the rewrite

Status: accepted ("replay" batch, block 9).

## Context

`src/replay.c` records, at each iteration of `tyrian2.c:level_loop`
(`replay_capture_frame`, right after `service_SDL_events_full`), player 1's
position and four buttons (fire, fire mode switch, sidekicks). On playback,
`replay_inject_frame` FORCES this position at the top of the iteration and sets the
buttons in `keysactive[]`; the directions stay released. At the start of a level,
`replay_level_start_hook` reseeds both generators
(`reseed_for_level`) from the file's seed.

Measured consequence: C playback of a replay does not always reproduce the
recorded run (shots start from the position forced at the top of the frame,
not from the one reached after the movement). Measured on our recordings:
`e1-tyrian-sweep` diverges at tick 190, `e1-tyrian-still` at tick 340;
`e2-first-sweep` (169 ticks) is identical.

## Decision

1. The parity reference for a replay is **the C playback** of the same `.rep`,
   not the recorded run: that is what a player sees when opening the replay
   in the Replays menu. The fidelity defect of `replay.c` is outside the
   rewrite's scope (the C code must not be modified).
2. The instrumented binary (`make GOLDEN=1`) records a real `.rep` through the
   F10 path (`GOLDEN_REPLAY_REC`, `replay_start_recording`) with the script's
   keys set where SDL sets them (`golden_master_frame_top`),
   then plays it back through a copy of the Replays menu launcher (`title_screen.c`,
   `GOLDEN_REPLAY_PLAY`), writing a `NAME.play.gm` golden master. The
   additions in `src/tyrian2.c` are all under `#ifdef GOLDEN_MASTER`.
3. `tyrian-sim::replay::ReplayRun` reproduces the same path: level loaded,
   generators `reseed_for_level` + 400 `mt_rand` draws from
   `backgrnd.c:initialize_starfield`, then for each frame: forced
   position, loop head, loop end with the buttons. The tick is closed
   the way the C code closes it (before `autopilot_drive`, hence after the injection and the
   head of the next iteration; as-is when the file is exhausted,
   `golden_master_replay_exhausted`).
4. Section → level: `level_load.c:JE_loadMap` reads from the requested
   section up to the first `]L[`, including beyond the following `*`
   (`lvl_file_of_section`).
5. Not simulated (explicitly rejected by `ReplayRun::new`): two players,
   arcade, mutators — the core does not simulate them yet.

## Verification

`utils/golden/record_replay.sh` then `tyrian-replay --data ../data NAME.rep
NAME.play.gm`; test `crates/tyrian-golden/tests/replays.rs` on
`rewrite/golden/replays/`.
