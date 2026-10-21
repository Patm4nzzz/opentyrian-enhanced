# ADR 0002 — New code under `rewrite/`, a Cargo workspace of eight crates

Date: 2026-10-04 — status: accepted (decision made alone). Updated on 2026-10-05 (m5 review): the
workspace has eight crates, no longer five.

## Context
The C version must stay intact and buildable at the root (`src/`, `Makefile`, `data/`). The new code must not be mixed into it.

## Decision
All new code lives under `rewrite/`: a Cargo workspace and the Godot project `rewrite/godot/`. The original data is still read from `data/` (no versioned copy); the imported resources are generated in `rewrite/godot/assets/imported/`, which is ignored by git.

The workspace members (`rewrite/Cargo.toml`) are eight crates:

| Crate | Role |
|---|---|
| `tyrian-formats` | byte-exact reading and writing of the original formats |
| `tyrian-sim` | deterministic simulation core, with no Godot and no hardware floating point (ADR 0003) |
| `tyrian-render` | palettized 8-bit rendering identical to the C framebuffer (ADR 0020) |
| `tyrian-mods` | mod format and validation (ADR 0080 to 0083); `tyrian-mods` binary |
| `tyrian-import` | conversion of the original data into Godot resources |
| `tyrian-golden` | comparison against the golden masters (ADR 0006); `tyrian-golden`, `tyrian-replay` and `tyrian-write-references` binaries (references written by the rewrite, ADR 0127) |
| `tyrian-robustness` | robustness bot for the core; `tyrian-robustness` binary |
| `tyrian-godot` | GDExtension, the bridge between Godot and `tyrian-sim` |

## Consequences
- One repository, two products; the existing C CI is untouched.
- Under WSL, the Cargo target directory is placed on the Linux disk (`CARGO_TARGET_DIR=$HOME/target-ot`): `/mnt/d` is slow and supports neither FIFOs nor symbolic links.
