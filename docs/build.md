# Building and running the Godot client (Rust rewrite)

This document covers the rewrite (`rewrite/`): the Rust simulation core
(`tyrian-sim`), the `tyrian-godot` GDExtension and the Godot 4.7 project
`rewrite/godot/`. The C version has been removed from the repository (ADR 0132): its last state
is the `c-final` tag. The golden masters it produced are frozen files in
`rewrite/golden/`.

## Prerequisites

| Tool | Version | Role |
|---|---|---|
| Rust (rustup) | ≥ 1.94 | `tyrian-sim`, `tyrian-godot`, `tyrian-import` |
| Godot | 4.7.2 stable (standard, not .NET) | engine, level editor |
| `data/` files | Tyrian 2000 | input of `tyrian-import` |
| WSL / Linux: `xvfb`, `imagemagick`, Mesa | — | headless screenshots (`capture_godot.sh`) |
| Windows: Visual Studio Build Tools (C++) | 2022 | MSVC linker for the DLL |
| Optional: `mingw-w64` (Linux) | — | Windows DLL built from WSL |
| Optional: NVIDIA GPU, Python 3.11 + torch CUDA, spandrel, NumPy, Pillow | — | HD mode images (`hd_upscale.py`) |
| Optional: `zip`, Godot 4.7.2 export templates | — | packages (`package.sh`) |

Rust dependencies added to `tyrian-godot` for MIDI music (ADR 0130): `rustysynth` 1.3.7
(MIT, SoundFont synthesizer) and `midly` 0.5.3 (Unlicense, tempo map), pulled from crates.io by
`Cargo.lock` like the others; no system library (no FluidSynth). The
`cargo test -p tyrian-godot midi` test renders the tracks in memory with `data/soundfont.sf2`, without
an audio device.

The Godot project loads `res://bin/libtyrian_godot.so` (Linux) or
`res://bin/tyrian_godot.dll` (Windows), declared in `rewrite/godot/tyrian.gdextension`.
The generated folders — `godot/bin/`, `godot/assets/imported/`, `godot/levels/`,
`godot/assets/upscaled/`, `godot/.godot/` — are ignored by git and are
regenerated with the scripts below.

## WSL / Linux

```bash
cd rewrite
scripts/build-linux.sh             # .so + tyrian-import + Godot import + level scenes
scripts/build-linux.sh --windows   # same + cross-built Windows DLL (x86_64-pc-windows-gnu)
godot --path godot                 # run the game
godot -e --path godot              # open the editor
```

Script steps: `cargo build --release -p tyrian-godot -p tyrian-import`,
copying the library into `godot/bin/`, `tyrian-import --data ../data --out
godot/assets/imported`, `godot --headless --import`, then
`godot --headless res://editor/build_levels.tscn`, which writes the 70
scenes `res://levels/epN_NN.tscn` (+ `epN_NN_events.tres` and the TileSets
`res://levels/tilesets/*.tres`).

To build the Windows DLL from WSL (verified: the resulting DLL loads
in Godot 4.7.2 for Windows and gives the same `state_hash` as the Linux version
for the same inputs):

```bash
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu
cargo build --release -p tyrian-godot --target x86_64-pc-windows-gnu
cp $CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/tyrian_godot.dll godot/bin/
```

With a custom `CARGO_TARGET_DIR`, the scripts honor it.

## Windows (PowerShell)

```powershell
cd rewrite
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Godot C:\path\to\Godot_v4.7.2-stable_win64_console.exe
& C:\path\to\Godot_v4.7.2-stable_win64.exe --path godot
```

The script builds with the `x86_64-pc-windows-msvc` target (install the
toolchain: `rustup toolchain install stable-x86_64-pc-windows-msvc`), copies
`tyrian_godot.dll` into `godot\bin\`, runs `tyrian-import.exe`, the Godot
import and `build_levels.gd`. Use the `_console` executable to see the
messages of the headless steps.

## Game controls

| Key | Effect |
|---|---|
| Arrow keys | movement (bits 0-3 of the mask) |
| Space | fire (bit 4) |
| Enter | change fire mode (bit 5) |
| Ctrl / Alt | left / right sidekick (bits 6, 7) |
| Esc | back to the menu |
| F1 | tick / `cur_loc` info |
| F2 | CRT filter |
| F3 | edge smoothing (xBRZ-style) |
| Left Shift | in-level bullet time: 4 s at `Engine.time_scale` = 0.25 (no tint, like the C version), 15 s cooldown (bullet_time.c) |
| F4 | STATISTICS panel over the level (F3: achievements) |
| F10 | display options (HD sprites) |

The simulation advances one tick per `_physics_process` at 35 Hz (ADR 0003).
Bullet time slows the engine clock: fewer ticks per real second,
the simulation itself stays identical.

User arguments (after `--`): `--level=ep1_09`, `--start-loc=<cur_loc>`,
`--seed=<n>`, `--data=<data folder>` (otherwise the `TYRIAN_DATA` variable, otherwise
`data/` next to the executable for an exported game, otherwise `<repo>/data`),
`--hd-sprites[=0|1]`.

Christmas mode (opentyr.c, xmas.c): in December, the title screen first shows the prompt
"Christmas has been detected. Activate Christmas?" (Yes / No, keyboard and
mouse). Yes replaces the shots, ships and bonuses with those of tyrianc.shp and the
voices with those of voicesc.snd. `--xmas` (-X) enables it without a prompt, `--no-xmas` (-x)
turns it off. To see the prompt out of season: `--unix-time=1797336000` (15 December
2026); the option freezes the date read by the game and has no effect without it.

## Level editor (ADR 0005)

Open `res://levels/epN_NN.tscn` in the Godot editor: three `TileMapLayer` nodes
(`Layer1` 14×300, `Layer2` 14×600, `Layer3` 15×600) and the
`LevelEventResource` resource (`events` property of the root). The "Tyrian" dock
(`addons/tyrian_level_editor` plugin, enabled in `project.godot`) offers:

- **Start row (layer 1)**: row of layer 1 to place at the top of the screen;
- **Row at the top of the 2D view**: takes the row at the top of the view;
- **Play from here**: launches `res://scenes/level.tscn`, which loads the level
  then calls `TyrianSim.fast_forward_to(cur_loc)`.

The game menu also offers "Level editor": pick a level and a
starting row, same launch.

## Screenshots (phase 7)

```bash
cd rewrite
scripts/capture_godot.sh            # -> docs/screenshots/godot-*.png
```

Each capture launches Godot under `xvfb-run` with
`-- --capture=<scene>:<output.png>:<ticks>[:<inputs>]` (see the header of
`godot/tools/capture.gd` for the input syntax, e.g. `up*60,fire~35`),
then the editor is opened on `ep1_09.tscn` and the X screen captured with
`import -window root` (`EDITOR_SETTLE` seconds, 35 by default). The script
fails if an image has an average brightness ≤ 0.02.

## HD mode (AI): images upscaled ×4 (ADR 0100)

**Reference version (what the packages use)**: the archive of the `hd-assets-5` release, downloaded and
verified by SHA-256. The release is published in this repository (`HD_REPO` in `hd_assets.sh`; the same
bytes are also kept in the former repository `Patm4nzzz/opentyrian-enhanced-archive`): the `gh` login,
or `GH_TOKEN` in CI (the workflow's default token is enough), must be able to read this repository.

```bash
scripts/hd_assets.sh fetch      # gh logged in; fails if the checksum, the imported sprites or the maps differ
```

Regenerate the images (NVIDIA GPU, Python 3.11 with CUDA `torch`, `spandrel`, `numpy`, `Pillow`):

```bash
python scripts/hd_upscale.py sprites godot/assets/imported 4x-PixelPerfectV4.pth <output>
python scripts/hd_upscale.py render  godot/assets/imported 4x-PixelPerfectV4.pth <output>
python scripts/hd_upscale.py measure godot/assets/imported <output> [level…]    # seam score
scripts/hd_assets.sh pack <output> opentyrian-hd-assets-N.tar.gz
```

The `4x-PixelPerfectV4` model (mutin-choler, WTFPL license, <https://openmodeldb.info/models/4x-PixelPerfectV4>,
SHA-256 `52bcda86…eb5f`, verified by the script) is not versioned. Publish the archive in an
`hd-assets-N` release, then copy the tag, name and hash into `hd_assets.sh`.

- **Sprites**: each atlas frame is upscaled on its own with a 6 px mirrored border (`scripts/atlas_pad.py`), otherwise
  bosses made of pieces placed edge to edge show a grid. The rectangles of the atlas JSON files remain valid ×4.
- **Short effects** (explosions, smoke, debris: `newsh6`; shots: `main_07`, `main_11`): xBRZ ×4 (`xbrz.py`,
  AGPL, generation tool only), not the model, which turns them into round bubbles. `SHARP_SHEETS` list of
  `hd_upscale.py`; mean difference from the nearest-neighbor upscaled original (`hd_upscale.py fidelity`): explosions
  3.69 versus 7.96 for the model (7.95 with grain restoration).
- **Background**: each layer of each level is assembled into a single image, upscaled in one go (no more tiles
  in HD mode: no grid possible), then cut into WebP strips of 4 tile rows
  (`upscaled/maps/<key>/NNN.webp`). The game only displays the strips close to the screen
  (`godot/scripts/hd_map_layer.gd`). The fine grain that the model smooths out (grass) is restored from the original wherever
  the upscaled image lost more than half of it.
- A layer whose grid changes (mod level) has no HD image: the game keeps the original tiles there.
- Whatever is neither background nor sprite stays pixel-exact, drawn by the extension as in Original mode: starfield
  (between layers 1 and 2), level texts, boss bars, sparks, halos (`level.gd`,
  `_update_hd_overlays`).
- What the C version does on palette indices is redone on the upscaled images (`scripts/hd_playfield_effects.gd`,
  `shaders/hd_index_effects.gdshader`): each color is mapped back to the nearest index, the C operation
  is applied, and the HD detail is kept as an offset from the index color. Ships of both players
  (Dragonwing included), shadows, sidekicks, enemies and shots come from the lists of what the C version drew at that
  frame (`TyrianSim.hd_player_sprites`, `hd_enemy_sprites`, `hd_shot_sprites`); blended explosions; level
  filter (event 44), lamp and flipped image over the whole play area (`hd_screen_effects`), in the C drawing
  order, which changes with `background2over`, `background3over`, `topEnemyOver` and `skyEnemyOverAll`.
- **Interface** (menus, shop, dashboard, exit box, level texts): the drawing remains the C version's in
  8-bit VGA (parity and OTFR screenshots unchanged); each pixel keeps track of the original image it comes from
  (`tyrian-render`: `Surface::origin`), and `TyrianVga.hd_image` puts the 4×4 block of the upscaled image in its place
  (`pics/` backgrounds, frames, icons, planets, portraits), recolored if the current palette differs from the atlas one.
  The rest (texts, digits, flat areas) is upscaled sharply by xBRZ (`shaders/hd_interface.gdshader`,
  `scripts/hd_interface.gd`). Fonts (`main_00` to `main_02`) in xBRZ frame by frame, not by the model.
  Green labels embedded in the dashboard frames (`PIC_LABELS` in `hd_ui.rs`): xBRZ too. Only the
  blocks of changed pixels are recomposed; a fade recolors without recomposing (ADR 0100, same result as the full
  composition).
  Planets, options and weapons are also exported by `tyrian-import` in the palettes where the C version draws them
  (`sprites/palNN/`), portraits one by one with their palette (`sprites/main_faces/`):
  `hd_upscale.py interface <imported> <model> <output>` upscales them along with the `pics/` images. The planet and
  portrait atlases in the game palette (`main_03`, `main_04`, `estpa`), which the C version never draws that way, are
  not upscaled (`SKIP_SHEETS`).
- Original versus HD check, frame by frame, of all the sheets (exact ×4, 1 to 3 px offset, frame
  blended with another, holes): `python3 scripts/hd_upscale.py frames godot/assets/imported godot/assets/upscaled`
  (exit code 1 at the first defect; `FRAME_LIMITS` thresholds).
- HD background: only the cells that the C version draws are displayed (`TyrianSim.hd_layer_cells`, rows of
  `backgrnd.c` with their overflow of one map line onto the next).

Missing-layer check (same tick, Original versus HD; difference in share of pixels or of lit 8×8 cells
in the play area, and largest area of differing color, thresholds calibrated in the script header; without arguments,
the four defects from the chief's review: two linked players, Super Arcade, TYRIAN difficult, Timed):

```bash
python3 scripts/hd_layer_check.py capture godot <output> e5-first-autopilot.gm:2000 e1-asteroid1-autopilot.gm:1200 \
    e3-first-autopilot.gm:1500 mode-squadron.gm:2500 mode-galaga.gm:1000 endless/endless-arcade-seed1.gm:1500
python3 scripts/hd_layer_check.py capture godot <output>
```

In game: **Graphics** in F10 (or F3) cycles through Original, Sharp pixel art and HD (AI), remembered in
`user://display.cfg`. HD (AI) is only offered if `upscaled/` is present. When active, the play area is rendered at
1280×800 (logical 320×200 coordinates unchanged, simulation untouched). Command line: `--hd-sprites[=0|1]`.

## Ready-to-play packages (Windows, Linux / Steam Deck)

```bash
cd rewrite
scripts/package.sh                   # WSL: full build + export for both platforms (HD PNGs included)
scripts/package.sh --skip-build --only=linux
scripts/package.sh --lite --hd-pack  # lite packages (without upscaled/) + separate HD pack
```

**Version and archive names.** The single source of the version is `application/config/version` in
`godot/project.godot` (`v1.0-rc1`), shown at the bottom left of the title screen. `package.sh` checks
that the Windows `file_version` / `product_version` of `godot/export_presets.cfg` follow it as four
numbers, major.minor.patch.rc (`v1.0-rc1` → `1.0.0.1`; a final `v1.0` would be `1.0.0.0`), and names
the archives after it: `OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64.zip`, `…-linux-x86_64.tar.gz`,
`…-windows-x86_64-lite.zip`, `…-linux-x86_64-lite.tar.gz` and `…-hd-pack.zip` (the folder inside stays
`OpenTyrian2000-Enhanced/`). The Cargo workspace carries the same version in semver form (`1.0.0-rc.1`).
A release attaches these five files.

Lite variant (`--lite`): same export, without
`upscaled/`, in `build/{windows,linux}-lite/`; archives `…-windows-x86_64-lite.zip` (197.1 MB = 188 MiB) and
`…-linux-x86_64-lite.tar.gz` (185.5 MB = 177 MiB), measured on v1.0-rc1
(MB = 10^6 bytes, the size a browser shows; MiB = 2^20 bytes).
The HD archive is then not
downloaded. `--hd-pack` adds `build/hd-pack/OpenTyrian2000-Enhanced-v1.0-rc1-hd-pack.zip` (506.5 MB = 483 MiB:
`upscaled/` and `HD-PACK-README.txt`), to be unzipped into the game folder. Without the pack, HD (AI)
is not offered; a saved HD (AI) choice is displayed and rendered as Original, with a
warning in the log.

The script installs the Godot 4.7.2 export templates if needed (official
`.tpz` archive, 1.28 GiB, in `~/.local/share/godot/export_templates/4.7.2.stable/`),
exports the presets of `godot/export_presets.cfg`, then assembles
`build/{windows,linux}/OpenTyrian2000-Enhanced/`: executable, `.pck`, GDExtension
library (`tyrian_godot.dll` mingw cross-built / `libtyrian_godot.so`),
`data/` (Tyrian 2000 data, plus `midi/NN-*.mid` and `soundfont.sf2` for the MIDI music),
`COPYING`, `DATA-NOTICES.txt` and `upscaled/` (verified HD archive, always included).
Archives: `OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64.zip` (703.8 MB = 671 MiB with HD) and
`OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64.tar.gz` (683.2 MB = 652 MiB with HD), measured on v1.0-rc1. The exported game
looks for `data/` and `upscaled/` next to the executable.

Launch tests in a clean folder:

```powershell
# Windows: extraction into %TEMP%\ot-clean-<date>\, reduced environment
# (no dev PATH/TYRIAN_DATA/CARGO), launch, screenshots.
powershell -ExecutionPolicy Bypass -File rewrite\scripts\test-package-windows.ps1 `
  -Zip build\windows\OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64.zip -Out docs\screenshots\export
```
```bash
# Linux: extraction into /tmp, env -i, xvfb-run.
mkdir /tmp/clean && tar -xzf build/linux/OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64.tar.gz -C /tmp/clean
cd /tmp/clean/OpenTyrian2000-Enhanced && env -i HOME=/tmp/clean PATH=/usr/bin:/bin \
  xvfb-run -a -s "-screen 0 1280x800x24" ./OpenTyrian2000-Enhanced.x86_64 --rendering-driver opengl3 \
  -- --capture=menu:/tmp/clean/title.png:30
```

Steam Deck: the Linux x86_64 package targets SteamOS (Linux x86_64, "GL Compatibility"
renderer). Add it to Steam as a "non-Steam game" pointing to
`OpenTyrian2000-Enhanced.x86_64`. Not tested on a real Deck; the controls profile
belongs to the modes work package.

## macOS package (experimental, unsigned)

No Mac and no paid Apple Developer account: the macOS app is built by the **`macos` job** of
`.github/workflows/rewrite.yml` (manual `workflow_dispatch` only, `macos-14` runner, Apple Silicon)
and signed **ad-hoc** (`codesign --sign -`): it runs on any Mac once the player gets past Gatekeeper
(right-click › Open, see the README), but it is neither Developer ID signed nor notarized.
It is not attached to releases yet.

The job runs, on the runner:

1. `brew install sdl2 ffmpeg coreutils` (music rendering, `sha256sum` for `hd_assets.sh`), Rust stable
   with the `aarch64-apple-darwin` and `x86_64-apple-darwin` targets;
2. Godot 4.7.2 macOS editor (`Godot_v4.7.2-stable_macos.universal.zip`, official SHA-512 of the
   release's `SHA512-SUMS.txt` checked);
3. `scripts/package.sh --only=macos` (refused on any other host), which runs:
   - `scripts/build-macos.sh`: `tyrian-godot` built in release for both targets, merged by `lipo` into a
     universal `godot/bin/libtyrian_godot.dylib` (declared as `macos.debug` / `macos.release` in
     `godot/tyrian.gdextension`), then `build-linux.sh --no-gdextension` (tyrian-import, music,
     Godot import, level scenes: plain POSIX steps);
   - export templates: the official `.tpz` (SHA-512 checked), only `templates/macos.zip` extracted into
     `~/Library/Application Support/Godot/export_templates/4.7.2.stable/` (cached by the job);
   - HD (AI) archive (`hd_assets.sh fetch`, read with the default `GITHUB_TOKEN`): **included, as in
     every default package**; a failed download or checksum fails the job (`--lite` exists but is never
     used by the CI);
   - export of the **`macOS`** preset (`godot/export_presets.cfg`): universal binary (x86_64 + arm64),
     bundle identifier `com.tyrian-enhanced.opentyrian2000`, `CFBundleShortVersionString` 1.0.0 and
     `CFBundleVersion` 1.0.0.1 (checked against `application/config/version`, like the Windows fields),
     icon `godot/icon/tyrian2000.icns` (made from `tyrian2000.ico`), Retina (`display/high_res`), category
     arcade games, built-in ad-hoc signing, notarization off, no microphone/camera/location/network
     entitlement nor usage description, no sandbox (gamepads need no entitlement). A universal or arm64
     export requires `rendering/textures/vram_compression/import_etc2_astc=true` in `project.godot`; the
     project has no VRAM-compressed texture, so Windows and Linux exports are unchanged;
   - `data/`, example `mods/`, `upscaled/`, `COPYING`, `DATA-NOTICES.txt`, `licenses/` copied into
     `OpenTyrian2000 Enhanced.app/Contents/Resources/` (same `copy_data` as the other packages), Rust
     library stripped, then the whole bundle signed again ad-hoc
     (`codesign --force --deep --sign - --entitlements packaging/macos/entitlements.plist`; the only
     entitlement is `disable-library-validation`, which Godot requires to load a GDExtension in an ad-hoc
     signed app), checked by `codesign --verify --deep --strict` and `lipo -info` (both architectures);
   - `ditto -c -k --keepParent` → `build/macos/OpenTyrian2000-Enhanced-v1.0-rc1-macos-universal.zip`;
4. `scripts/test-package-macos.sh`: extraction by `ditto` into a blank folder, signature and `lipo`
   checks, then without display: startup (`--headless --quit-after`), startup with `--hd-sprites` (the
   bundle's HD images must be found), **strict replay** of `golden/e1-tyrian-autopilot.gm` inside the
   exported app (checks the GDExtension, `data/` in `Contents/Resources` and the simulation), the Intel
   slice under Rosetta 2 if the runner has it, and a check that nothing was written into the bundle;
5. artifacts: `OpenTyrian2000-Enhanced-macos-universal` (the `.zip`, 7 days) and `macos-test-logs`.

**Paths on macOS** (`godot/scripts/game_paths.gd`, `user_data.gd`, tested by
`godot/test/game_paths_test.gd` for the three systems): the files shipped with the game are read from
`Contents/Resources/` (the executable is in `Contents/MacOS/`); the player data (settings, saves,
replays, screenshots, mods, log) always goes to `~/Library/Application Support/OpenTyrian2000 Enhanced/`
(`application/config/use_custom_user_dir.macos=true` in `project.godot`: macOS only, Windows and Linux
keep `userdata/` next to the executable and Godot's usual user folder). Nothing is ever written into the
bundle: it is signed, and Gatekeeper may run it translocated (read-only copy). HD (AI) images:
`Contents/Resources/upscaled` first (shipped), then `upscaled/` in the player data folder (where a separate
HD pack would go, never into the bundle).

Under WSL, only the Rust side can be checked (no Apple linker or SDK):

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo check -p tyrian-godot --target aarch64-apple-darwin
cargo check -p tyrian-godot --target x86_64-apple-darwin
```

A run takes about 25–35 min the first time (Homebrew, two Rust release builds, music rendering,
Godot import, 1.3 GB templates download, 0.5 GB HD archive), less with the caches. macOS minutes are
free on a public repository and count ten times on a private one. Tester checklist:
[macos-test-checklist.md](macos-test-checklist.md).

## Player check before every push

**Every push to `master` first passes `rewrite/scripts/player-journeys.sh --core` (all PASS), then
`rewrite/scripts/player-smoke.sh`** locally (GitHub Actions CI is unavailable: billing).
The player journeys verify the effect of each action (money debited, weapon upgraded, option kept);
player-smoke compares images and fails if the journeys did not all pass on the same head.
See [testing.md](testing.md). Both play the exported package, not the Godot project: that is
what the player receives.

```bash
sudo apt install xvfb xdotool imagemagick python3-pil python3-numpy   # once
rewrite/scripts/player-journeys.sh --build --core   # builds the Linux package, then the core journeys
rewrite/scripts/player-smoke.sh            # reuses build/linux/OpenTyrian2000-Enhanced
```

Duration: about 4 minutes. Exit code 0 (OK) or 1; report `build/player-smoke/report.md`
(references, captures obtained and side-by-side differences). Only one game lives at a time, Xvfb and the game
are killed at the end; `Dummy` audio, `--max-fps 70`, nothing is displayed on the desktop.

Checks:

1. **Package contents**: `demo.1-5`, `cubetxt1-5.dat`, levels, SoundFont, MIDI, icons,
   native library, locales.
2. **Strict replay of 2 golden masters** (`e1-tyrian-autopilot`, `e2-first-autopilot`) in the
   package: every tick compared to the C version (18 columns, seven state hashes).
3. **Played game** under Xvfb, 1280x684 window (320x200 game letterboxed), real X inputs:
   title, new game, shop (hover and click on Ship Specs, Upgrade Ship, Options),
   level 1 with keyboard then with mouse, pause, pause menu, Quit box (hover Yes / No),
   back to the shop, exit, relaunch, title, demo, Load, Setup (hover over the 8 lines), Jukebox,
   exit via the title.
4. **Images**: each capture is cropped to the exact 320x200 and compared with
   `rewrite/golden/player-smoke/*.png` (fraction of pixels whose difference exceeds a threshold; see
   `player_smoke_compare.py`). Animated screens (level, demo, Jukebox) are only compared as
   "not black"; the level must in addition react to the keyboard and mouse.
5. **Godot log**: fails on `SCRIPT ERROR`, `ERROR:`, file not found, crash; fails
   if the game stops before the end or does not close with exit code 0.

The references were frozen with Mesa llvmpipe (software rendering, deterministic). After an intended
display change: `player-smoke.sh --update-refs`, **review** the replaced images
(`git diff --stat rewrite/golden/player-smoke`), then commit them with the change.
Limits: the check does not compare with the C version image by image in the menus (the references are
captures of the Godot package, reviewed by hand); level parity relies on the replays.

## Troubleshooting

- **"GDExtension tyrian-godot absente"** when launching a level:
  the library is not in `godot/bin/`. Rerun the build script, or
  copy `libtyrian_godot.so` / `tyrian_godot.dll`.
- **Textures or levels not found**: `godot/assets/imported/` is empty;
  run `tyrian-import` (step 3 of the script) then `godot --headless --import`.
- **`build_levels: … not imported`**: the generator needs the imported
  textures; run `godot --headless --path godot --import` first.
- **Project on `/mnt/d` (WSL)**: the `.godot/` cache works on this
  file system in our tests. In case of slowness or import errors,
  copy the project elsewhere (`rsync -a --exclude .godot rewrite/godot/ ~/godot-work/`)
  and run from the copy.
- **`Started the engine as root`** under WSL: harmless warning;
  `GODOT_SILENCE_ROOT_WARNING=1` hides it.
- **Black screen under xvfb**: force `--rendering-driver opengl3` (Mesa
  llvmpipe); the default Vulkan driver is not available under xvfb.
- **Windows DLL rejected** ("not a valid Win32 application"): check the
  `x86_64` target and that the DLL does not come from a Linux build.
- **Scrolling does not exactly match the C game**: as long as
  `tyrian-sim` does not publish `layers[]` (values at 0), the client displays an
  approximation (default speeds from `tyrian2.c`, without the speed
  events); F1 indicates it ("defilement approx.").
