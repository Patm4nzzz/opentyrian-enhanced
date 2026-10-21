# ADR 0122 — Shared interface layer above the 8-bit framebuffer (block 20.3)

Status: accepted (block 20, refactoring approved by the maintainer).

## Context

The out-of-game screens draw into a shared 8-bit framebuffer (`TyrianScreen` + `TyrianVga`, ADR
0020) that is compared pixel for pixel with the C version. Mods (their own interface elements, MODDING.md) and HD
sprites (ADR 0040) must be able to add elements to these screens without breaking this parity, and
without each screen having to make room for them. Some C drawing helpers were also duplicated between
screens (`nortvars.c:JE_barDrawShadow` in `setup.gd` and `shop.gd`).

## Decision

- `TyrianScreen.overlay`: a `Node2D` (z_index 1) above the `Sprite2D` that displays the framebuffer.
  Everything that does not exist in the C version (mod element, HD sprite, rewrite help) goes into
  this layer, **never** into the framebuffer: the OTFR images (`--frames`) remain those of the 8-bit
  rendering, so pixel parity does not depend on the additions.
- `TyrianScreen.add_overlay_provider(Callable)`: a provider (mod autoload, HD pack) is called
  with each screen on entry (`NOTIFICATION_READY`) and places its nodes there; signal `presented(surf)`
  on each `JE_showVGA` to follow the displayed image.
- Shared C drawing helpers in `TyrianScreen`: `bar_draw_shadow` (`nortvars.c:JE_barDrawShadow`,
  moved out of `setup.gd` and `shop.gd`), next to `quit_request_msg`, the fades and the waits; the
  menu drawings are in `MenuEngine` (ADR 0121), the text in `TyrianText`.

## Consequences

- Proof: `tools/capture.gd --overlay-probe` installs a test provider (rectangle + text on
  each screen, checked on Title then Setup); Setup sequence 206/206 images identical to the
  pre-refactoring reference.
- Not done: wiring a real provider (mods: `Mods`, HD pack: `DisplaySettings`) — `display_*`
  is being refactored by another agent; the API is ready for it.
