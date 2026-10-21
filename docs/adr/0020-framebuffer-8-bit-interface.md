# ADR 0020 — Interface rendered into an 8-bit framebuffer identical to the C version

## Context

Block 2 (extended to the whole interface) calls for pixel-level visual parity
with the C version: title screen, fades, fonts, menus, shop, HUD… The
first Godot version drew the interface with RGBA nodes (Sprite2D,
approximated `TyrianText`, modulated colors): font shadows, the
highlight (`value + 4`), palette fades and the `blit_sprite_*_blend` blends
(which operate on palette **indices**) cannot be reproduced exactly in
RGBA.

## Decision

1. New crate `tyrian-render` (no Godot): 320×200 8-bit surfaces
   (`video.c:VGAScreen`, `VGAScreen2`, `game_screen`), the blitters of `sprite.c`
   with the same pointer arithmetic (including overflow from one line to the
   next), the fonts of `font.c`/`fonthand.c`, the hardware palette and the integer
   fades of `palette.c` (`step_fade_palette`, truncated division).
2. GDExtension class `TyrianVga` (`tyrian-godot/src/vga.rs`) that exposes it to
   GDScript; `TyrianScreen` (GDScript) displays the surface converted through the
   palette and calls `present()` at every `JE_showVGA()` of the C code.
3. The ported interface scenes (title first) call the same
   functions, in the same order, as the C code they reproduce; each
   rule cites its C file and function.
4. Verification: the `make GOLDEN=1` build writes every presented image
   (`GOLDEN_FRAMES`, OTFR format: indices + palette + RGBA layer);
   `TyrianScreen` writes the same format with `--frames`. `visual_diff.py
   seqdiff` compares the two sequences **rank by rank**. The real waits
   (`setDelay`, `wait_delayorinput`) are removed during capture: only the order
   of the images matters, not their timestamps.

## Consequences

- Exact parity verifiable image by image, independent of scaling,
  filtering and the X server.
- The Godot filters (CRT, xBRZ, Real-ESRGAN) apply afterwards, on the RGB
  image, like the C scalers after `JE_showVGA`.
- The game itself (`level` scene) remains in Godot nodes for now; moving it
  to the 8-bit framebuffer is the rest of the work.
- `palette.c:JE_gammaCorrect` is not reproduced: gamma is 0 by default.
