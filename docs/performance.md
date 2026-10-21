# Performance with thousands of projectiles (block 12)

Goal: stable 60 frames/s and the original simulation rate (35 ticks/s, ADR
0003) with thousands of projectiles.

Machine: AMD Ryzen 9 9950X3D (16 cores / 32 threads), NVIDIA GeForce RTX 5090
(driver 616.64), Windows 11. Godot 4.7.2, "Compatibility" renderer (`opengl3`,
the project's).

**Two measurement environments, not to be confused:**

- **Native Windows, RTX 5090**: Windows Godot (`Godot_v4.7.2-stable_win64`)
  with the cross-compiled GDExtension `tyrian_godot.dll`, real 960×600 window.
  These are the figures that count for the 60 FPS goal;
- **WSL + Xvfb**: **software** rendering (Mesa llvmpipe), no graphics card.
  The FPS figures do not represent a gaming PC; the simulation and node update
  times (CPU) remain comparable.

## Test bench

The "bullet hell" mutator of the C version (`enemy_render.c:JE_drawEnemy`,
`mut_bullet_hell`) is not ported ("modes" batch) and is in any case capped at
`ENEMY_SHOT_MAX` = 60 shots (`varz.h`). The bench therefore injects N enemy
shots through a test API:

- `tyrian-sim/src/bench.rs`: `bench_set_enemy_shot_capacity(n)` enlarges the
  enemy shot array; `bench_fill_enemy_shots(n, seed)` tops up to N shots in
  flight (graphics of the level's enemy weapons, no damage, its own generator
  that does not touch the game's). The simulation now iterates over
  `enemy_shots.len()` instead of the constant: 60 without the bench, hence
  behavior identical to the C (golden masters 10/10, 52,957 ticks);
- `tyrian-godot/src/bench.rs`: same functions exposed to Godot;
- `godot/tools/bench.gd` (`Bench` autoload, inactive without `--bench=`):
  plays level 1 (episode 1, TYRIAN, autopilot assists enabled so that the ship
  survives) at 35 ticks/s, continuous fire, vertical sync off; measures per
  frame: `sim_us` (injection + `step`), `refresh_us` (reading the state and
  updating the nodes, `level.gd:_refresh`), `shots_us` (of which projectiles),
  `frame_ms` (interval between two `_process` calls), the engine's CPU/GPU
  render time (`RenderingServer.viewport_get_measured_render_time_*`).
  `--bench-legacy-shots` forces the old GDScript projectile path;
- `scripts/bench_godot.sh` (WSL/Xvfb); on Windows, same command as below;
- `cargo run --release -p tyrian-sim --example bench_shots`: simulation only.

Commands:

```sh
# WSL (software rendering):
rewrite/scripts/bench_godot.sh /tmp/bench.json 0,60,500,2000,5000,10000,20000 8 [--bench-legacy-shots]
# native Windows (project imported with the DLL in bin/):
Godot_v4.7.2-stable_win64_console.exe --rendering-driver opengl3 --path <godot> -- \
    --data=<repo>/data --bench=<output.json> --bench-counts=0,60,500,2000,5000,10000,20000 --bench-seconds=8
# Simulation only:
cargo run --release -p tyrian-sim --example bench_shots -- ../data 3000
```

Raw results: `docs/perf/*.json`.

## Measurement before: where the time goes

First pass (original code, WSL, `docs/perf/wsl-llvmpipe-original.json`): the
simulation stays under 125 µs per tick at 5,000 shots; the cost is in the
Godot node updates, and it hides a defect:

- `shot_batch.gd` capped each batch at **512 instances**: beyond ~1,000 enemy
  shots (two sheets), the excess shots **were not drawn**. The "before"
  figures of the original code beyond 1,000 shots therefore measure an
  incomplete rendering;
- once the cap is lifted (capacity doubled on demand), the GDScript path —
  one instance at a time (`set_instance_transform_2d` +
  `set_instance_custom_data`, filter by `Callable`) — costs about 0.8 µs per
  shot: ~8 ms per tick at 10,000 shots, ~16 ms at 20,000. This is the
  `--bench-legacy-shots` mode below ("before", for equal work).

## Optimization

- `tyrian-godot/src/render.rs:shot_instances` builds the complete MultiMesh
  buffer in Rust (12 floats per instance: Transform2D + UV region) from
  `SimState`, using a frame table prepared once by `ShotBatch`;
  `ShotBatch.update_from_sim` sets it in a single call (`MultiMesh.buffer`).
  Breakdown at 10,000 shots (WSL, `--bench-probe`): construction 36 µs,
  resizing 23 µs, assignment 87 µs;
- `TyrianSim.counts()`: sounds and sparks only need the numbers of shots and
  explosions; `level.gd` no longer copies three arrays per tick for that;
- rendering verified pixel-identical: `--replay` captures of the golden master
  `e1-tyrian-autopilot.gm` at ticks 61, 4,221 and 6,801, old and new code,
  sparks (CPUParticles2D, random from one capture to the next) disabled for
  the comparison: **0 pixels different** on the three frames.

The simulation was not optimized: 0.15 ms per tick at 20,000 shots, that is
0.6% of the budget of a tick at 35 Hz (28.6 ms).

## Results — native Windows, RTX 5090 (vsync off)

Before (GDScript path, no cap) — `docs/perf/win-rtx5090-legacy.json`:

| shots | FPS | ticks/s | frame p50 / p99 / max (ms) | sim p50 / p99 (µs) | nodes p50 / p99 (µs) | of which shots p50 (µs) | render CPU / GPU p50 (ms) |
|---:|---:|---:|---|---|---|---:|---|
| 0 | 2010 | 35.00 | 0.43 / 1.53 / 35.59 | 18 / 35 | 211 / 983 | 49 | 0.15 / 0.15 |
| 60 | 1559 | 34.98 | 0.56 / 2.31 / 12.33 | 20 / 27 | 342 / 1914 | 105 | 0.20 / 0.16 |
| 500 | 749 | 34.99 | 1.12 / 5.73 / 14.89 | 27 / 39 | 1127 / 6057 | 731 | 0.43 / 0.29 |
| 2000 | 847 | 35.00 | 0.94 / 5.55 / 14.29 | 40 / 86 | 2614 / 7194 | 2266 | 0.37 / 0.22 |
| 5000 | 1162 | 35.00 | 0.66 / 5.69 / 15.24 | 52 / 66 | 4213 / 9552 | 3965 | 0.25 / 0.14 |
| 10000 | 864 | 35.00 | 0.70 / 10.75 / 18.70 | 86 / 119 | 8395 / 15235 | 8126 | 0.26 / 0.15 |
| 20000 | 502 | 35.00 | 0.66 / **22.50** / 31.97 | 147 / 252 | 16096 / 27370 | 15837 | 0.25 / 0.12 |

After (buffer built in Rust) — `docs/perf/win-rtx5090-rust.json`:

| shots | FPS | ticks/s | frame p50 / p99 / max (ms) | sim p50 / p99 (µs) | nodes p50 / p99 (µs) | of which shots p50 (µs) | render CPU / GPU p50 (ms) |
|---:|---:|---:|---|---|---|---:|---|
| 0 | 2091 | 35.00 | 0.41 / 1.38 / 10.33 | 15 / 22 | 180 / 900 | 36 | 0.15 / 0.15 |
| 60 | 1724 | 35.00 | 0.52 / 1.57 / 12.84 | 16 / 21 | 224 / 458 | 42 | 0.19 / 0.15 |
| 500 | 1308 | 35.00 | 0.69 / 2.48 / 11.78 | 21 / 28 | 313 / 489 | 49 | 0.25 / 0.16 |
| 2000 | 1323 | 35.00 | 0.68 / 3.07 / 11.47 | 32 / 47 | 314 / 530 | 72 | 0.26 / 0.16 |
| 5000 | 1419 | 35.00 | 0.63 / 2.49 / 11.40 | 49 / 90 | 344 / 645 | 131 | 0.24 / 0.14 |
| 10000 | 1347 | 34.97 | 0.66 / 2.63 / 11.91 | 84 / 122 | 486 / 3290 | 259 | 0.26 / 0.15 |
| 20000 | 1362 | 34.98 | 0.63 / **2.79** / 9.99 | 149 / 207 | 1048 / 5421 | 813 | 0.25 / 0.15 |

Reading:

- **original rate held** in all cases: 35.00 ticks/s;
- the per-tick cost of the projectiles goes from 8.1 to 0.26 ms at 10,000
  shots (×31) and from 15.8 to 0.81 ms at 20,000 (×19);
- the slowest frame at the 99th percentile goes from 22.5 ms to 2.8 ms at
  20,000 shots: before, one frame in a hundred exceeded the 60 FPS budget
  (16.7 ms); after, the margin is ×6;
- the **maximum** stays around 10-12 ms in all cases, including with no shots
  at all: it does not depend on the projectiles (first frames, Windows
  scheduler); it stays under 16.7 ms;
- the GPU is not a factor (0.15 ms per frame).

The FPS figures are measured with vsync off (the engine renders as fast as
possible); with vsync, the limit is the display. Not measured: a real 60 Hz
display with vsync, and the Forward+/Vulkan renderer (the project uses
Compatibility).

## Results — WSL + Xvfb, llvmpipe software rendering

Before (GDScript path) — `docs/perf/wsl-llvmpipe-legacy.json`:

| shots | FPS | ticks/s | frame p50 / p99 (ms) | sim p50 (µs) | nodes p50 (µs) | of which shots p50 (µs) | render CPU / GPU p50 (ms) |
|---:|---:|---:|---|---:|---:|---:|---|
| 0 | 115 | 34.96 | 8.19 / 15.48 | 19 | 201 | 51 | 4.34 / 5.28 |
| 500 | 101 | 34.92 | 9.35 / 16.79 | 24 | 660 | 388 | 4.98 / 5.94 |
| 2000 | 87 | 34.87 | 10.68 / 23.34 | 35 | 1660 | 1407 | 5.73 / 6.69 |
| 5000 | 71 | 34.86 | 14.74 / 25.64 | 57 | 3805 | 3480 | 7.12 / 8.13 |
| 10000 | 49 | 35.03 | 20.91 / 35.81 | 95 | 7335 | 6951 | 9.26 / 10.27 |
| 20000 | 15 | 35.19 | 64.31 / 105.61 | 164 | 14408 | 14051 | 29.61 / 31.03 |

After — `docs/perf/wsl-llvmpipe-rust.json`:

| shots | FPS | ticks/s | frame p50 / p99 (ms) | sim p50 (µs) | nodes p50 (µs) | of which shots p50 (µs) | render CPU / GPU p50 (ms) |
|---:|---:|---:|---|---:|---:|---:|---|
| 0 | 109 | 34.90 | 8.59 / 17.09 | 19 | 189 | 40 | 4.55 / 5.55 |
| 500 | 100 | 35.03 | 9.43 / 18.27 | 24 | 305 | 52 | 5.09 / 6.06 |
| 2000 | 95 | 35.03 | 10.05 / 17.23 | 36 | 321 | 80 | 5.74 / 6.73 |
| 5000 | 84 | 35.06 | 11.32 / 21.47 | 57 | 412 | 167 | 7.02 / 8.02 |
| 10000 | 70 | 35.17 | 13.65 / 24.56 | 95 | 580 | 313 | 9.04 / 10.06 |
| 20000 | 49 | 35.03 | 17.86 / 46.33 | 161 | 828 | 551 | 12.77 / 13.85 |

Under software rendering, the llvmpipe rasterizer dominates (5 to 14 ms per
frame): 60 FPS at the 99th percentile is not attainable there, even with no
projectiles. It is not a gaming PC; the environment serves for CPU
measurement and CI.

## Simulation only

`cargo run --release -p tyrian-sim --example bench_shots -- ../data 3000`
(WSL, 3,000 ticks per step, level 1, assists enabled):

| shots | median (µs) | p99 (µs) | max (µs) | share of a 35 Hz tick (p99) |
|---:|---:|---:|---:|---:|
| 0 | 1.1 | 1.9 | 42.3 | 0.01% |
| 60 | 1.5 | 2.3 | 78.7 | 0.01% |
| 500 | 4.2 | 6.7 | 1,732 | 0.02% |
| 1,000 | 7.3 | 11.1 | 2,981 | 0.04% |
| 2,000 | 13.6 | 24.1 | 7,763 | 0.08% |
| 5,000 | 33.4 | 48.0 | 10,484 | 0.17% |
| 10,000 | 68.0 | 87.8 | 141.6 | 0.31% |
| 20,000 | 137.9 | 166.9 | 238.6 | 0.58% |

Linear cost (~7 ns per shot per tick). The isolated maxima of 2 to 10 ms (one
tick in several thousand) vary from one run to the next and do not appear at
10,000/20,000 shots: process preemption under WSL, not a simulation cost.

## What remains

- Explosions and enemies still go through individual nodes (pooled
  `Sprite2D`, `_draw`): not measured in bulk (the C caps them at
  `MAX_EXPLOSIONS` = 200 and `ENEMY_MAX` = 100).
- The "bullet hell" mutator itself ("modes" batch): with it, the game would
  stay at 60 enemy shots at most, far from the volumes measured here.
