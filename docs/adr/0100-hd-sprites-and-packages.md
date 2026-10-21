# ADR 0100 — "HD sprites" mode (Real-ESRGAN) and exported packages

Date: 2026-10-04 — status: accepted (decision taken alone, export batch).

## Context
The mandate asks for an optional display mode using the sprites upscaled by Real-ESRGAN (block 11), and ready-to-play
Windows and Linux packages (blocks 13 and 15). The C game has no equivalent of the HD mode: it is an addition of the
rewrite, which must touch neither the simulation nor the golden masters.

## Decisions
1. **HD mode only changes textures.** `TyrianAssets` (assets.gd) loads the ×4 atlas from `upscaled/` in place of the
   original; `SpriteSheet`, `ShotBatch` and the level TileSet keep their regions in original pixels and apply a 1/4
   scale factor when drawing. The simulation, positions and HUD stay in logical 320×200.
2. **Rendering at 1280×800 in HD mode.** The game viewport (effects.gd) is rendered at 4× its logical size with
   `size_2d_override` = 320×200: without that, the added details disappear at 320×200 rendering. The texture filter
   stays "nearest neighbor" (no extra blur).
3. **Disabled by default**, persistent toggle in `user://display.cfg`, "Display options" screen
   (`scenes/display_options.tscn`, F10 from any screen; a separate screen so as not to conflict with the title menu
   rewrite by the visuals batch). `--hd-sprites[=0|1]` on the command line for captures. If `upscaled/` is missing,
   the options screen says so ("run scripts/upscale_realesrgan.*") and the game keeps the original sprites.
4. **Mirrored border per frame before upscaling** (`scripts/atlas_pad.py`). Upscaling an atlas in one block makes the
   network "bleed" one frame onto its neighbors: the scenery (24×28 tiles) and the bosses (12×14 pieces) being made of
   frames laid edge to edge, a grid appeared on screen (seen on the first captures). Each frame is now upscaled alone
   with a 6 px mirrored border, then glued back: the grid disappears on the bosses, but not on the scenery (the network
   invents two touching edges differently): see "Scenery upscaled in context" below.
5. **The upscaled PNGs are not in Git** (≈ 94 MiB): see "Reproducibility" below (release archive verified by
   SHA-256). A `.gdignore` keeps them out of the Godot import and the `.pck`.
6. **Packages**: `scripts/package.sh` exports the "Windows Desktop" and "Linux" presets (export_presets.cfg), and
   places `data/` (Tyrian 2000 freeware data) and `upscaled/` (always, from the verified archive) next to the
   executable. The imported PNGs (`assets/imported/`) are included in the `.pck` through `include_filter` because the
   client reads them with `FileAccess`. In an exported game, `game.gd` looks for `data/` next to the executable
   (before the repo-relative path).

## Reproducibility of the HD PNGs (2026-10-05, review item 18)

Finding: the upscaled PNGs only existed on the machine that had run `upscale_realesrgan.ps1`; `package.sh` only
copied them with `--with-hd`. The first published development builds **do not contain them** (archives
`OpenTyrianGodot-{windows,linux}-x86_64` downloaded and listed: 0 `upscaled/` entries), so HD mode showed
"not installed" there.

Options weighed:
- Git LFS: the account's storage and bandwidth quota, and every clone (CI included) pays 94 MiB; rejected.
- Generation in CI: `realesrgan-ncnn-vulkan` requires Vulkan; on GitHub runners (no GPU) it goes through Mesa
  lavapipe: ≈ 4.5 min for 44 atlases measured in WSL, more for the 72, and the output of a floating-point network is
  not guaranteed identical from one driver to another (NVIDIA GPU versus CPU): no deterministic rendering; rejected.
- **Chosen: versioned archive as a release asset, verified by hash.** The `hd-assets-1` release (pre-release, not a
  game version) carries `opentyrian-hd-assets-1.tar.gz` (98,713,718 bytes, SHA-256
  `4dd2ab19a31be33f67ceca43574da31672839dc1829276836269d6f0f043f9c8`). `scripts/hd_assets.sh fetch` downloads it
  (`gh release download`, private repo; cache `~/.cache/opentyrian-hd`), refuses any other hash, extracts it into
  `godot/assets/upscaled/`, then checks `sources.sha256`: the hash of each imported PNG from which an upscaled PNG
  derives. A change to `tyrian-import` (frames, palette) therefore makes CI fail instead of shipping stale images.
- `hd_assets.sh pack` rebuilds the archive reproducibly (sorted order, dates, owners and gzip header fixed: two
  successive builds give the same hash) from the output of `upscale_realesrgan.ps1`. Publishing a new archive = a new
  `hd-assets-N` release + the new hash in `hd_assets.sh`, in the same commit.
- `package.sh` calls `hd_assets.sh fetch` and always includes `upscaled/` (the `--with-hd` option is removed); the
  `rewrite` CI also runs `fetch`. No package depends any more on the GPU of the machine that builds it.

Licenses: `realesrgan-ncnn-vulkan` v0.2.5.0 is under the MIT license (Xintao Wang; code taken from realsr-ncnn-vulkan,
MIT, nihui); Real-ESRGAN and its `realesrgan-x4plus-anime` weights are under the 3-clause BSD license (Xintao Wang).
Neither the tool nor the model is redistributed: only their outputs are, derived from the Tyrian data (same caveat as
the data, `DATA-NOTICES.txt` of the package, which now cites the tool and the model).

## Scenery upscaled in context (2026-10-05, review item 16)

Finding: with tiles upscaled in isolation, every tile edge is a break (squares of different contrast on the walls,
`docs/screenshots/export/hd-compare-zoom.png`).

Decision (`scripts/hd_tiles.py`, called by `upscale_realesrgan.{ps1,sh}` after the atlases):
- each layer of each level is rebuilt as an image (106 distinct layers out of the 70 levels, cut into 396 strips of
  100 tile rows with 4 rows of overlap), upscaled ×4 in one block, then cut back up;
- the game stays at **one atlas per tile set** (nothing changes on the Godot side): a tile seen in several contexts
  takes the **per-pixel median** of its upscaled occurrences (at most 64, deterministic sampling). 1,438 tiles used
  across 5 sets. Measured alternative: one atlas per level and layer (3,224 tiles, 2.2× the size, one TileSet per
  level to load) gains almost nothing (ratio 2.01 versus 2.05 below): rejected;
- moving sprites unchanged (atlas_pad.py); tiles that no map uses: isolated version.
- `hd_assets.sh` also checks `tile_maps.sha256` (hash of the tile grids): a modified map makes the archive stale.
  Archive `hd-assets-2` (SHA-256 `73382416…f4a2`), 349.9 s on the RTX 5090.

Measurement (`hd_tiles.py measure`): mean color jump between the two pixels on either side of a tile edge, divided by
the same jump 2-3 px from the edge, over all the recomposed maps (1 = no break specific to the edges):

| image | jump at the edge | near the edge | ratio |
|---|---|---|---|
| map upscaled in one block (reference, no seam possible) | 5.33 | 3.78 | 1.41 |
| isolated tiles (before, `hd-assets-1`) | 14.82 | 2.90 | **5.11** |
| tiles in context, per-set median (`hd-assets-2`) | 7.16 | 3.29 | **2.18** |

On real Godot captures (`e1-tyrian-autopilot` replay, ticks 1500/2600/4221, 1280×800), discontinuity peak at the tile
period, relative to the median: 5.46 / 3.07 / 3.62 before versus 2.20 / 1.81 / 2.60 after (x).
Boards: `docs/screenshots/export/hd-grid-context-{1500,4221}.png` (original, isolated, context).

**The grid is reduced (÷2.3), not eliminated**: to the eye, seams remain visible on textured surfaces (ice, rock) when
a tile appears there in different contexts; the median softens them without erasing them. Recalibrating the mean
colors against the original was tried and worsens the measurement (2.38). In line with the maintainer's rule, **the HD (AI)
mode is not offered in the options screen** for now (ADR 0124, addendum); the pipeline and the archive remain in
place for a resumption (lead: upscale the whole map at run time, or one atlas per level with a less "painterly"
model).

## Scenery displayed as a whole map (2026-10-06, HD comparison card, step 2)

The cause of the grid was the method, not the model: a tile atlas cannot reconcile the edges of two neighboring tiles,
which the network upscales in different contexts. Decision (the maintainer, 2026-10-06): the HD scenery is no longer a tile
atlas but **the whole map of each level layer**, upscaled in one block offline, and the game displays it **in strips
as it scrolls**.

- `scripts/hd_upscale.py render`: for each of the 106 distinct layers of the 70 levels, the map is rebuilt (empty
  tiles = transparent, according to `blank_tiles`), upscaled ×4 by `4x-PixelPerfectV4` (spandrel + PyTorch, fp32,
  RTX 5090; model hash verified at load), then cut into strips of 4 tile rows (448 HD px) encoded as WebP q90: 8,492
  strips. 55 min of rendering. `--quality` lowers the quality if size is preferred: q80 shrinks the archive by about
  40% on the measured layers (ep1_09: 12.97 → 7.77 MiB) with no visible difference at the zooms examined
  (`hd-whole-map-quality.png`).
- `HdMapLayer` (`godot/scripts/hd_map_layer.gd`) replaces the emptied `TileMapLayer`: same parent, hence same
  scrolling; 1/4 scale; nearest filter. Only the visible strips and their neighbors stay in memory; the next one on
  each side is decoded **and uploaded** in a thread (`WorkerThreadPool`; the rendering API is safe from another
  thread), which brings the median update down to 0.01 ms. The name of a layer's folder is the SHA-256 of its tile
  grid: a map modified by a mod has no images and the layer keeps its original tiles (dedicated test,
  `hd_map_layer_test.gd`).
- **Each strip contains 16 HD px of its neighbors, which the game does not display** (`region_rect`). Without this
  margin, a WebP strip joins the next one poorly: the seam ratio at strip edges goes from 1.03 (never compressed) to
  1.37–1.43 at q90, and returns to 1.04–1.05 with the margin. Cost: +7% pixels (+6% bytes on ep1_09).
- **Grass grain** (`restore_grain`): the model renders textured surfaces smooth. Where the upscaled image has lost
  most of the fine detail of the original (grass, bushes), that detail is given back to it from the original ×4;
  stone, metal and trees do not move (`hd-whole-map-grass.png`). No seam added: the "+ grain" column of the table
  below stays at the whole-map level.
- Moving sprites: same model (`hd_upscale.py sprites`, per-frame mirrored border kept); the boss in pieces no longer
  shows a grid (`hd-whole-map-ingame.png`).

Measurement (`hd_upscale.py verify --atlas <hd-assets-N folder> <level…>`): mean color jump |Δ| at tile edges divided
by the mean jump at the other edges of ×4 pixels. 1.00 = tile edges are indistinguishable from the others. The value
depends on the content (art with sharp edges at the grid gives more than 1): compare the columns with one another, not
the score with 1.

| level (layer) | original art ×4 | HD atlas (hd-assets-2) | model, whole map | + grain | WebP strips |
|---|---|---|---|---|---|
| ep1_01 L1 | 4.14 | 5.80 | 5.45 | 5.43 | 5.03 |
| ep1_06 L1 | 5.59 | 11.81 | 6.46 | 6.47 | 6.26 |
| ep1_06 L2 | 46.47 | 100.03 | 55.00 | 54.99 | 49.82 |
| ep1_09 L1 | 0.86 | 1.64 | 0.88 | 0.90 | 0.91 |
| ep1_09 L2 | 3.65 | 6.61 | 3.28 | 3.28 | 3.07 |
| ep1_09 L3 | 7.20 | 14.30 | 7.57 | 7.57 | 7.50 |
| ep2_01 L1 | 0.99 | 1.42 | 0.90 | 0.92 | 0.93 |
| ep5_04 L1 | 0.93 | 1.17 | 0.89 | 0.90 | 0.96 |
| ep5_04 L2 | 26.66 | 25.28 | 27.43 | 27.27 | 16.85 |

The HD atlas doubles the break of the original art (5.59 → 11.81; 3.65 → 6.61; 7.20 → 14.30); the whole map brings it
back to the level of the art itself, sometimes better (ep5_04 L2: 26.66 → 16.85), and the strips add nothing
measurable (under 5%). In game, `e1-tyrian-autopilot` replay (curLoc and position identical to the C version): no more
grid on the ice or the rock, boss without a seam. Boards: `docs/screenshots/export/hd-whole-map-levels.png` (four
levels, three states, markers of the old tile edges), `hd-whole-map-ingame.png`, `hd-whole-map-grass.png`.

Loading and memory, largest level (ep1_09, three layers, 1280×800, RTX 5090): 409 ms to enter the level (of which
46 ms for the first two frames), +41.6 MiB of textures; full scroll of the three layers simulated (10,500 positions):
5 strips per layer at most, +48.2 MiB at most, worst update 19.4 ms, median 0.01 ms, 99th percentile 4.54 ms.

HD (AI) mode is now offered in the options screen (hot toggle included, during a level); the archive is
`hd-assets-3` (484,472,483 bytes, SHA-256 `e8e0bfea45aa45e7b510096380cb4b1d3686d9e8f16ee1dc2a60377f91421ed8`),
built twice in a row identically.

## Layer 2 blended like the C (2026-10-06, HD mode review)
The C draws layer 2 through `draw_background_2_blend` (`wild` true, default value): each pixel keeps the hue (high
nibble) of layer 2 and takes a brightness (low nibble) halfway between its own and that of what it covers
(`backgrnd.c:blit_background_row_blend`). That is what makes the ice of ep1_09 translucent light blue, the water
visible beneath. The HD scenery laid layer 2 opaque: white ice, no water.

Fix: `shaders/hd_layer_blend.gdshader` on HD layer 2 brings each pixel (layer 2 and the image already drawn beneath)
back to the nearest index of the game palette through two 64³ tables (`TyrianVga.nearest_index_lut`), applies the C
formula, then reads the palette back. For layer 2, the search is limited to the palette rows that make up at least 2%
of its tiles (`TyrianSim.layer2_hue_rows`): without that limit, the upscaled glints of the ice blue fall on other hues
(dark spots). The blend follows the published state (`TyrianSim.layer2_blended`: switched off by event 48 or
`background2over == 3`). Layer 3 is opaque in the C (`draw_background_3`), hence unchanged.

Measurement (ep1_09, layer 2 over layer 1, ×4 image brought back to ×1 and compared with the exact blend by index,
mean RGB difference 0-255): opaque layer 49.9; 50% alpha 42.4; nearest index over the whole palette 5.35; layer 2
rows only 3.30 (floor: layer 2 alone versus the original, 4.9). Board `docs/screenshots/export/hd-layer2-blend.png`
(same C replay ticks on the three columns; the HD captures have their scenery aligned with the original, best offset
0 px at the three ticks). The "tick 61" row of the previous board actually showed tick 1500 in the first two columns
(captures from another series); it is replaced by tick 1500 on the three columns. Measured cost (`--bench` bench,
ep1_09, HD): 0.36 ms per median frame versus 0.41 ms before, within measurement noise.

## HD interface and the hd-assets-4 archive (2026-10-07)
In HD mode, the scenery and sprites were smooth but the dashboard, menus and texts stayed in big 320×200 pixels. The
interface goes HD without touching its drawing: screens are still composed by the C code in 8-bit VGA (parity and
interface references of Original mode unchanged). Each pixel keeps track of the original image it comes from;
`TyrianVga.hd_image` puts the 4×4 block of the upscaled image in its place, and the rest (texts, digits, flat areas) is
upscaled sharply by xBRZ in `shaders/hd_interface.gdshader`.

- Images by the model (`hd_upscale.py interface`): 14 full-screen backgrounds from `tyrian.pic` (title, menus, shop,
  dashboard frames for 1 and 2 players), 36 portraits each in its palette, 8 atlases exported in the palette of their
  screen (`sprites/palNN/`: planets, options, weapons).
- Sharp pixel art (xBRZ ×4, frame by frame): the 6 fonts (`main_00` to `main_02` and their Christmas version), in
  addition to explosions and shots. The green labels embedded in the dashboard frames (FRONT GUN, REAR GUN, MODE,
  SHIELD, ARMOR; `PIC_LABELS` in `hd_ui.rs`) also go through xBRZ: the model rendered them blurry and doubled.
- The planet and portrait atlases in the game palette (`main_03`, `main_04`, `estpa`), which the C never draws that
  way, are no longer upscaled.
- An interface image only recomposes the blocks of changed pixels and their neighbors; a fade recolors the blocks
  without recomposing them. Check: a variant that recomposes everything every frame and compares with the partial
  result gave 1,395 identical frames out of 1,395 (menus with fades, shop, Setup, dashboards for 1 and 2 players,
  Timed, achievement banner, Endless, Galaga).

Archive `hd-assets-4` (495,432,267 bytes, SHA-256 `440e0741d36a1b1f178bcfb07346c11287b402c18ce521de2c52010fece63b3d`),
built three times identically. It contains archive 3 unchanged for the scenery (8,492 strips), 62 sprite sheets and 58
interface images. Images regenerated from an empty folder: 58 out of 58 byte-for-byte identical (model, RTX 5090) and
11 xBRZ sheets out of 11. `sources.sha256` also covers the `pics/` backgrounds (130 entries). `package.sh` copies
`upscaled/pics` into the packages.

Cost, previous development build versus new, HD mode, 1280×800 window, median of 3 runs (Linux, Xvfb, software OpenGL; in Original mode, the
differences stay under 0.1 s and 8 MiB):

| screen | time to image previous → new | process memory | textures | median / 95th percentile game frame |
|---|---|---|---|---|
| title | 2.62 s → 3.71 s | 313 → 390 MiB | 15.8 → 19.7 MiB | - |
| shop | 2.45 s → 3.87 s | 428 → 504 MiB | 15.8 → 21.0 MiB | - |
| 1-player dashboard | 1.57 s → 2.50 s | 512 → 581 MiB | 73.4 → 78.5 MiB | 0.31 / 0.43 ms → 3.40 / 5.48 ms |
| 2-player dashboard | 1.41 s → 2.24 s | 475 → 555 MiB | 57.6 → 71.2 MiB | 0.37 / 0.47 ms → 2.99 / 4.76 ms |

The time to image includes the screen's fade-in: before partial composition, it was 5.9 s at the title and 6.0 s at
the shop. A full composition of the interface image costs about 20 ms; partial composition brings it down to 1.7 ms
median (dashboard), and no game frame exceeds 16.7 ms over 300 frames played (worst 15.5 ms, versus 13.1 ms in the previous build).

HD previous-build versus HD new boards at 1280×800 (dashboard for 1 and 2 players, shop, title, Setup, Mods, achievement banner,
Timed countdown, ×2 zooms on the texts) and proof that Sharp pixel art mode is unchanged: `hd-ui/` folder of the
card's deliverables. Sharp pixel art, previous build versus new: 6 screens out of 8 pixel-identical; the other two differ exactly
as in Original mode (money counter darkened by the lamp, em dash in a mod description converted like the C), parity
fixes of this version.

## Consequences
- Golden masters unchanged (10/10, 52,957 ticks): no simulation file touched.
- The package depends on no repo path nor development environment variable (tested in a blank folder, reduced
  environment, Windows and Linux).
- No Windows Sandbox on the reference machine: the Windows "clean machine" test is a temporary folder with a minimal
  environment, not a VM. It does not prove the absence of a dependency on an installed runtime; checked with objdump:
  the GDExtension DLL (mingw) only imports system DLLs (KERNEL32, msvcrt, ntdll, USERENV, WS2_32, bcryptprimitives,
  api-ms-win-core-synch), no VC++ runtime nor libgcc/libstdc++.
