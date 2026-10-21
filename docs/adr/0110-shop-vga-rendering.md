# ADR 0110 — Shop rendered on TyrianScreen, menu by menu

Status: accepted (ui batch, 2026-10-04).

## Context
`shop.gd` displayed the state of `TyrianShop` (the Rust `ShopSession` logic) with temporary
text nodes. The C version (`game_menu.c:JE_itemScreen`) redraws the whole 8-bit
framebuffer on every loop iteration. The time available in the round does not allow
porting all the sub-screens at once (category with the weapon simulation
`shop_preview.c`, Next Level with the map, Data Cubes, Options).

## Decision
- `shop.gd` inherits from `TyrianScreen`. When the Rust session is open and the
  current menu is MENU_FULL_GAME or MENU_UPGRADES, the screen is redrawn
  in the VGA like `JE_itemScreen` (pic 1 background, `JE_drawMenuHeader`,
  `JE_drawMenuChoices`, cubes, money, `JE_barDrawShadow` gauges,
  `draw_ship_illustration`, `JE_drawMainMenuHelpText`) and then `present()` is called.
- The other menus keep the temporary display (nodes) until they are ported.
- The logic (keys → `TyrianShop.key`) does not change.
- Font mapping: the C `FONT_SHAPES` (table 0) = `LARGE_FONT` on the
  TyrianScreen side, `SMALL_FONT_SHAPES` (table 1) = `NORMAL_FONT`.
- The background is kept in a private copy (`_background`) rather than in
  VGAScreen2: the VGA is shared and the title screen, freed with a delay, may
  still write to it.

## Consequences
Root and "Upgrade Ship": 0 pixels different from the C version.
Category sub-screen, Next Level and the others: still temporary.
