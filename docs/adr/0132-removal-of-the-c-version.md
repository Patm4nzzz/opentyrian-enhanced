# ADR 0132 — Removal of the C version from the repository

Status: accepted (2026-10-08, decision by the maintainer: "Retire C"). Replaces point 3 of ADR 0131.

## Context
The Godot + Rust version is the project's version (ADR 0131). The C version only served as a comparison oracle: the golden masters, replays and interface fingerprints it produced are already recorded in the repository, and since ADR 0127 the rewrite can rewrite these files byte for byte.

## Decision
1. **The last commit that contains the C version is the tag `c-final`** (published on the remote repository). No existing release, tag or branch is deleted: the C history remains intact, and the C packages already published remain on the releases page.
2. **Removed from `rewrite/godot-rust` and then from `master`**: `src/` (C sources and headers), `Makefile`, `tests/` (C tests), `linux/` and `packaging/` (C package), the `build.yml` workflow (build, tests, ASan, cppcheck, Wine, C packages), `CHANGELOG.md` and `CONFIG.md` (history and settings of the C version), `docs/game_harness.md`, `docs/video.md` and `docs/arch/` (documents of the C version), the tools that drove or instrumented the C binary (`utils/game_*.sh`, `win_smoke.sh`, `utils/golden/record*.sh`, `capture_c.sh`, `utils/money/`, `rewrite/scripts/visual_capture_*c*.sh`, `golden_pair.sh`, `side_by_side_video.sh`…), the asset and mod tools of the C format (`dat_*`, `mod_*`, `asset_build_hd_*`, C xBRZ), and the regeneration of the interface references on the C side.
3. **Kept**: the game data (`data/`, including the mods in the C format, which the Godot engine lists as an error to convert), the licenses (`COPYING`, `rewrite/licenses/`), all of `rewrite/`, `docs/` (except the C documents), the locale and font checks in `utils/`, the guide `docs/legacy/MODDING-c-version.md` (useful for converting a C mod).
4. **Three dependencies on C code are frozen, not removed**:
   - `utils/c_tables.json`: tables that the locale checks read in `src/` (UTF-8 → CP437 conversion, `font_ascii`, roles of the menu keys, keys drawn in large font, buffer capacities, keys used as a printf format, weapons assigned by the code). Extracted as is from the `c-final` tag.
   - `rewrite/tools/music-render/c/`: the 13 C files (OPL emulators, LDS player, song titles) that `render.sh` compiles to render the game's OPL music. Exact copies of the files from `c-final`; it is the only C code in the repository.
   - The golden masters, replays and interface fingerprints (`rewrite/golden/`) remain **frozen files**.
5. **New golden masters**: they can no longer be recorded in this repository. To produce one, you must check out the `c-final` tag (`git worktree add <folder> c-final`), build the C version there with `make GOLDEN=1` and use the `utils/golden/record*.sh` scripts of that tag, then copy the resulting file into `rewrite/golden/`. The rewrite does not produce a "new" golden master: it compares itself with the frozen C version or with itself (`tyrian-write-references --check`, ADR 0127).
6. **Strict parity**: `tyrian-golden --strict` and the `replays` test replay the frozen files without the C version; CI runs them on every tag and on demand.
7. **Publishing**: the `OpenTyrian Godot` workflow (`.github/workflows/rewrite.yml`) builds the Windows and Linux packages and publishes the release on the `v*` tag. No C package is published anymore.

## Consequences
- The C interface references (`rewrite/golden/ui/*.sha1`) are no longer regenerated. A screen that deliberately changes moves to a "godot" sequence with a reviewed Godot reference.
- Mentions of the C version in the code and the historical ADRs describe the state of the repository at `c-final`; they remain valid as provenance.
