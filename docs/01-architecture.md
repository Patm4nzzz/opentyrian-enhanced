# Phase 1 — Architecture of the Godot 4 + Rust rewrite

Date: 2026-10-04. All versions below were verified the same day (GitHub releases, crates.io).

## 1. Selected versions

| Component | Version | Verification source |
|---|---|---|
| Godot | **4.7.2-stable** (released 2026-08-18) | `api.github.com/repos/godotengine/godot/releases/latest` |
| godot-rust / gdext (crate `godot`) | **0.5.5** (2026-08-09), feature `api-4-7`, MSRV Rust 1.94 | crates.io + GitHub release `v0.5.5` |
| Rust | 1.99.0 (rustup, stable channel) | `cargo --version` |
| `png` | 0.18 | crates.io |
| `serde` / `serde_json` | 1.0 | crates.io |
| `clap` | 4.6 | crates.io |
| `anyhow` | 1.0 | crates.io |

The simulation core has **no external dependency** (no Godot, no floats, no randomness crate).

## 2. Directory layout

All new code lives under `rewrite/`; the C version (`src/`, `Makefile`, `data/`…) is untouched, except for the phase 2 instrumentation behind a compile flag.

```
rewrite/
  Cargo.toml                  workspace Cargo
  crates/
    tyrian-formats/           byte-exact reading/writing of the original formats (lib)
    tyrian-sim/               deterministic simulation core, no Godot (lib)
    tyrian-import/            CLI: data/ → Godot resources (PNG + JSON)
    tyrian-golden/            CLI: replays a C golden master in the Rust core and compares
    tyrian-godot/             GDExtension (cdylib): gdext bridge Godot ↔ tyrian-sim
  godot/                      Godot 4.7 project (rendering, UI, audio, level editor)
    project.godot
    tyrian.gdextension
    scenes/  scripts/  shaders/  editor/  assets/imported/ (generated, git-ignored)
  golden/                     reference replays produced by the instrumented C version
  scripts/                    Linux/WSL and Windows builds, captures, Real-ESRGAN
```

## 3. Simulation core (`tyrian-sim`)

- **Cadence**: one `step` = one tick of the original game, i.e. ≈ 34.8 Hz at Normal speed (`config.c:JE_setNewGameSpeed`, `speed = 0x4300`, `frameCountMax = 2`). Godot calls `step` from `_physics_process` with `physics_ticks_per_second = 35`; bullet time slows the **calling clock**, never the logic (ADR 0003).
- **Arithmetic**: integers only. The few floats in the C code (Link Gun, orbiting sidekicks, firing directions, difficulty multipliers) are replaced by integer tables and exact fractions; every deviation is recorded in `docs/divergences.md`.
- **RNG**: hand-written xoshiro256**, same algorithm as `src/rng.c`, so that the C draws can be reproduced; explicit seed. Visual effects never touch it (Godot has its own cosmetic randomness).
- **API**:
  - `Sim::new(LevelData, Loadout, seed) -> Sim`
  - `Sim::step(&mut self, inputs: [PlayerInput; 2])`
  - `Sim::state(&self) -> &SimState` (read-only for Godot)
  - `Sim::snapshot() -> Vec<u8>` / `Sim::restore(&[u8])`
  - `Sim::state_hash() -> u64` (FNV-1a 64 over a canonical serialization) and `Sim::frame_record()`, which exposes the same fields as the C golden master, for a field-by-field comparison.
- **Per-tick order**: reproduces that of `tyrian2.c:JE_main` (`level_loop`): events → scroll → ground enemies → random spawn → sky/top enemies → player shots → player collision → player movement → enemy shots → explosions.

## 4. Godot layer

- Godot **reads** the state (`SimState`) on every render frame and owns none of it. The `TyrianSim` node (GDExtension) exposes `load_level`, `step`, `get_player`, `get_enemies` (PackedInt32Array), `get_shots`, `get_scroll`, `state_hash`.
- Rendering: 3 background layers as `TileMapLayer` (24×28 tiles), enemies as pooled `Sprite2D`, projectiles as `MultiMeshInstance2D`. Logical resolution 320×200, integer scaling through `viewport` stretch.
- Level editor: the Godot editor itself. Each imported level becomes a `.tscn` scene (3 `TileMapLayer`) plus a `LevelEvents` resource (`.tres`, custom resource `LevelEventResource`). A plugin button launches the game at a given scroll position (ADR 0005).
- Compatibility rendering (`gl_compatibility`) by default: works under Xvfb/Mesa for captures, and on any GPU.

## 5. Import (`tyrian-formats` + `tyrian-import`)

- `tyrian-formats` reads and rewrites every format described in `docs/formats-spec.md`. Areas the C code does not read (tails, residual bytes, unused table entries) are kept as-is. Mandatory test: `read → write` identical byte for byte on **all** files in `data/`.
- `tyrian-import` produces, in `rewrite/godot/assets/imported/`: PNG tile atlases (per `shapesX.dat` × palette), PNG sprite sheets, `levels/epN_lvlM.json` (layers, `mapSh`, events, enemies), `items.json` (HDT tables), sounds as WAV. A headless Godot pass (`editor/build_levels.gd`) turns the JSON files into `.tscn` scenes + `.tres` resources.

## 6. Audio strategy (ADR 0004)

- **Effects**: `tyrian.snd` / `voices.snd` converted to WAV (signed 8-bit 11,025 Hz PCM → unsigned 8-bit WAV), played by `AudioStreamPlayer`.
- **Music**: `music.mus` is Loudness (OPL2). Godot has no OPL emulator; the target is an offline OPL → OGG render by a small separate tool (reusing `lds_play.c` + `nuked_opl3.c` without modifying the C version). At the player's choice (Setup > Sound), the MIDI files in `data/midi/` are played with `data/soundfont.sf2` by rustysynth in the extension (ADR 0130); their rights status remains unclear (see `formats-spec.md` §13).

## 7. Build

- Linux/WSL (reference, used for all tests and captures): `rewrite/scripts/build-linux.sh` → `cargo build --release` then copy `libtyrian_godot.so` into `godot/bin/`.
- Windows: `rewrite/scripts/build-windows.ps1` (`cargo build --release --target x86_64-pc-windows-msvc`, or cross-compilation from WSL to `x86_64-pc-windows-gnu` with mingw-w64). See `docs/build.md`.

## 8. ADRs

- [0001](adr/0001-oracle-master.md) — `master` is the oracle.
- [0002](adr/0002-workspace-layout.md) — new code under `rewrite/`, Cargo workspace with five crates.
- [0003](adr/0003-fixed-tick-35hz.md) — fixed step at the original tick (≈ 34.8 Hz), integers only, xoshiro256** RNG reproduced.
- [0004](adr/0004-audio.md) — SFX converted to WAV, OPL music rendered offline, MIDI set aside.
- [0130](adr/0130-midi-music-rustysynth.md) — optional MIDI music (rustysynth + SoundFont), per-track OPL fallback.
- [0005](adr/0005-level-editor.md) — the Godot editor as level editor (generated TileMapLayer scenes).
- [0006](adr/0006-golden-master-format.md) — golden master by inputs + field-by-field state record.
