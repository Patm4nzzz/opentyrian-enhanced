# ADR 0123 — Interactive shop on the shared menu engine (block 20.6)

Status: accepted (block 20, refactoring approved by the maintainer).

## Context

The shop (`shop.gd`, ADR 0110) displays the state of `TyrianShop` (GDExtension, `tyrian-sim`:
`ShopSession`), which holds all the logic for buying, selling, weapon power and navigation
(`game_menu.c:JE_itemScreen`). Its menus were drawn by code specific to `shop.gd`, and its
keys read as `KEY_*`.

## Decision

- Inputs: shared InputMap actions (ADR 0120) translated into `TyrianShop.key` keys
  (`up/down/left/right/select/escape`); Space and Enter = `select` as in the C version.
- Rendering: "Game Menu" root and "Upgrade Ship" through `MenuEngine` with the `GAME_MENU` style
  (`JE_drawMenuHeader`, `JE_drawMenuChoices`); a category's item list through the
  `ROWS` style, CUSTOM rows (`_draw_category_row`: owned mark, name, `JE_drawItem` icon, cost) and
  the C `shop_scroll` window.
- The engine is in `external` mode: it does not navigate, it draws the state of the Rust session. The
  logic remains single (Rust); no shop rule in GDScript.
- The elements around the menus (data cubes, money, armor and shield bars,
  ship illustration, help) stay in `shop.gd` (`_decorate_menu`), called by the
  `decorate` option.

## Consequences

- Proof: sequences shop-1 (10 images compared with the C version) and shop-2 (14): 0 rows different from the C version
  and from the Godot reference; `campaign_check OK`; golden masters unchanged.
- All the categories (including Front Gun, Rear Gun and sidekicks, formerly in the temporary
  `_redraw_session` display) and "Next Level" (`]G[` destinations: planet names, "Exit to
  Game Menu") are rendered in 8 bits by the engine; the temporary node display is removed.
  The weapon power costs (`downgradeCost` / `upgradeCost`) come from `TyrianShop.state()`
  (Rust), not from a GDScript computation.
- Remaining: animated firing simulation (`shop_preview.c:JE_weaponSimUpdate`: starfield, shots) and
  animated Next Level map (`JE_updateNavScreen`: lines, dots, planets); areas left
  empty; Data Cubes, Ship Specs, Options not ported.
