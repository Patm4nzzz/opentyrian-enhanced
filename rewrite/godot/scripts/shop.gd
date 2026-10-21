## Campaign shop (game_menu.c:JE_itemScreen), rendered in 8-bit
## (ADR 0110) by the shared menu engine (ADR 0121, 0123).
##
## All the logic of buying, selling, weapon power and navigation
## is in the TyrianShop GDExtension (tyrian-sim/shop.rs): this script
## forwards the keys and draws `_shop.state()`. Background: tyrian.pic no. 1
## (game_menu.c:320). Item names and icons: items.json, plus the
## mod objects (Mods.items()).
##
## Menus rendered: root "Game Menu" and "Upgrade Ship" (GAME_MENU style),
## item list of each category (ROWS style), "Next Level" (GAME_MENU
## style), data cube list (ROWS style) and cube reading.
## Side screens: Ship Specs (JE_doShipSpecs, enlarged green image), "quit"
## box (JE_quitRequest), Options = save screen (save_menu.gd).
## Animations of the inner loop of JE_itemScreen: firing demonstration of the
## weapon categories (shop_preview.c, simulation in TyrianShop) and navigation
## map of "Next Level" (game_menu_nav.c:JE_updateNavScreen).
extends TyrianScreen

## 8-bit rendering (ADR 0110): idle screen for the capture tool.
var idle := false
var _background := PackedByteArray()
## game_menu.c:shop_scroll (window of the item list, persistent).
var _shop_scroll := 0
## game_menu.c:UPGRADE_VISIBLE.
const UPGRADE_VISIBLE := 5
## game_menu.c:menuChoicesDefault (MENU_FULL_GAME 7, MENU_UPGRADES 9).
const MENU_CHOICES := {0: 7, 1: 9}
## helptext.c:menuHelp (rows MENU_FULL_GAME and MENU_UPGRADES), 1-based indices of mainMenuHelp.
const MENU_HELP := {0: [1, 34, 2, 3, 4, 5], 1: [6, 7, 8, 9, 10, 11, 11, 0, 0, 12]}
## helptext.c:JE_refreshLocalizedMenuText: menuInt[1] and menuInt[2] (locale keys).
const MENU_INT := {
	0: ["shop_game_menu", "shop_data", "shop_ship_specs", "shop_upgrade", "shop_options", "shop_start_level",
			"shop_quit_game"],
	1: ["shop_upgrade", "upg_ship", "upg_front_gun", "upg_rear_gun", "upg_shield", "upg_generator", "upg_left_side",
			"upg_right_side", "misc_done"],
}
## shop.h: curMenu indices played by TyrianShop.
const MENU_PLAY_NEXT_LEVEL := 3
const MENU_UPGRADE_SUB := 4
## sndmast.h: S_ITEM.
const S_ITEM := 18
const MENU_DATA_CUBES := 7
const MENU_DATA_CUBE_SUB := 8
## pcxmast.c:facepal: palette (0-based) of each cube's face.
const FACEPAL := [1, 2, 3, 4, 6, 9, 11, 12, 16, 13, 14, 15]
## game_menu.c: visible text lines of a cube (10) and their step (12 px).
const CUBE_LINES := 10
const CUBE_LINE_H := 12
## curSel[MENU_UPGRADES] (2..8) -> items.json table.
const CATEGORY_TABLES := {2: "ships", 3: "weapon_ports", 4: "weapon_ports", 5: "shields", 6: "generators",
		7: "sidekicks", 8: "sidekicks"}
## Categories with a weapon demonstration (game_menu.c: curSel[MENU_UPGRADES]
## in [3, 4, 6, 7, 8] -> JE_weaponSimUpdate).
const WEAPON_SIM_CATEGORIES := [3, 4, 6, 7, 8]
## Object type of Mods.items() per category (names of the items added by
## the mods, MODDING.md §5.9).
const MOD_KINDS := {2: "ship", 3: "weapon", 4: "weapon", 5: "shield", 7: "sidekick", 8: "sidekick"}

## TyrianShop session (GDExtension) and its last state.
var _shop: RefCounted
var _state: Dictionary = {}
## Shared menu engine (ADR 0121): rendering of all the JE_itemScreen menus.
var menus := MenuEngine.new(self)
## Mod objects: "table:id" -> dictionary (Mods.items()).
var _mod_items := {}
## PNG images of mod items to lay over the shop image (never in the
## 8-bit framebuffer, like the other mod additions): position -> texture.
var _mod_images := {}
var _mod_image_layer: Node2D
## game_menu.c:yLoc: scrolling (pixels) of the open cube.
var _cube_y := 0
## Palette of a cube's face set (game_menu.c paletteChanged / newPal).
var _face_palette := false
## Time accumulated for the continuous scrolling of a cube (setDelay(1) per pixel).
var _cube_clock := 0.0
## Cube lines left to scroll on wheel notches (negative = upward).
## Intentional divergence by the maintainer: the C code has no wheel.
var _cube_wheel_lines := 0
## game_menu.c:yChg: step of the scrolling still in progress after release
## (reset to 0 on a 12 px line boundary).
var _cube_chg := 0
## game_menu.c col / colC: value of the blinking cube (MENU_DATA_CUBES),
## reset to 0 / -1 by JE_menuFunction, advanced at each setDelay(2).
var _flash_col := 0
var _flash_col_c := -1
var _flash_clock := 0.0
## Side screen open (Ship Specs): the next key closes it.
var _specs_open := false
var _specs_key := false

## Hover in a purchase list (MENU_UPGRADE_SUB) only applies once the pointer
## has rested HOVER_DWELL_MS on the same row. Hovering there fits the item
## live (money shown, power arrows, demo): leaving the list diagonally toward
## the power arrows crossed the "None" / "Done" rows, which became the
## selection, refunded the gun and hid the arrows (owner's bug, 2026-10-20).
const HOVER_DWELL_MS := 150
## Row waiting for its dwell (-1: none), when it was entered, and the menu it
## belongs to.
var _hover_pending := -1
var _hover_since_ms := 0
var _hover_menu := -1
## Clock of the dwell (ms); tests replace it.
var hover_clock: Callable = func() -> int: return Time.get_ticks_msec()

## Animated inner loop of JE_itemScreen (firing demonstration: setDelay(3),
## map: setDelay(2)): `_anim_step` draws one turn, presented afterwards.
var _anim_step := Callable()
var _anim_delay := 0
var _anim_clock := 0.0
## Turns left before stopping (capture tool, --shop-sim-frames=N:
## as many turns per key as the C GOLDEN_SHOP script, which injects a
## key every 8 turns); -1: endless.
var _anim_left := -1
## Turn of the inner loop: the background is not restored (only the map is
## redrawn, the text on top).
var _redraw_only := false

## game_menu.c:planetX / planetY: position of the 21 planets on the map.
const PLANET_X := [200, 150, 240, 300, 270, 280, 320, 260, 220, 150, 160, 210, 80, 240, 220, 180, 310, 330, 150, 240,
		200]
const PLANET_Y := [40, 90, 90, 80, 170, 30, 50, 130, 120, 150, 220, 200, 80, 50, 160, 10, 55, 55, 90, 90, 40]
## varz.c:PGR: image (1-based) of PLANET_SHAPES for each planet.
const PGR := [4, 1, 2, 3, 20, 36, 52, 68, 84, 100, 116, 132, 151, 151, 151, 151, 52, 52, 1, 2, 4]
## game_menu_nav.c:PAni: animated planets (planetAni added to the image).
const PANI := [1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1]
## Map palette (game_menu.c: newPal = 18, 1-based).
const NAV_PALETTE := 17
const INSTRUCTIONS := preload("res://scripts/instructions.gd")
## game_menu.c: navX / navY (view center), newNavX / newNavY (target).
var _nav_x := 0.0
var _nav_y := 0.0
var _new_nav_x := 0.0
var _new_nav_y := 0.0
## planetAni / planetAniWait, currentDotNum / currentDotWait.
var _planet_ani := 0
var _planet_ani_wait := 0
var _dot_num := 0
var _dot_wait := 0
## JE_computeDots: dots of the path to each destination.
var _planet_dots := PackedInt32Array()
var _dot_x: Array[PackedInt32Array] = []
var _dot_y: Array[PackedInt32Array] = []
## Map palette set.
var _nav_palette := false

## Shop menus of the title modes (game_menu.c:JE_itemScreen, curMenu
## set according to twoPlayerMode / onePlayerAction / superTyrian): 9
## MENU_2_PLAYER_ARCADE, 10 MENU_1_PLAYER_ARCADE, 13 MENU_SUPER_TYRIAN; 0:
## full game shop (TyrianShop).
var _arcade_menu := 0
## curSel[curMenu] (2 = first row).
var _arcade_sel := 2
## helptext.c:menuInt[curMenu + 1] (keys), menuChoicesDefault and menuHelp.
const ARCADE_MENU_INT := {
	9: ["menuInt_10_0", "menuInt_10_1", "menuInt_10_2", "menuInt_10_3", "menuInt_10_4", "menuInt_10_5"],
	10: ["menuInt_11_0", "menuInt_11_1", "menuInt_11_2", "menuInt_11_3"],
	13: ["menuInt_14_0", "menuInt_14_1", "menuInt_14_2", "menuInt_14_3", "menuInt_14_4"],
}
const ARCADE_MENU_HELP := {9: [4, 30, 30, 3, 5], 10: [4, 37, 12], 13: [4, 34, 3, 5]}


func _ready() -> void:
	# game_menu.c:JE_itemScreen: play_song(songBuy) (]i[ of the section,
	# otherwise DEFAULT_SONG_BUY, level_load.c:JE_loadMap).
	Music.play_song(Music.shop_song(Game.episode, Game.shop_section))
	var items_key := "episodes_1_3" if Game.episode <= 3 else "episode_%d" % Game.episode
	_items_json = TyrianAssets.read_json("items.json")[items_key]
	_load_mod_items()
	_mod_image_layer = Node2D.new()
	_mod_image_layer.name = "ModImages"
	overlay.add_child(_mod_image_layer)
	overlay.move_child(_mod_image_layer, 0)
	presented.connect(_sync_mod_images)
	_arcade_menu = Game.shop_start_menu()
	var resume_sel := Game.shop_resume_sel
	Game.shop_resume_sel = 0
	if resume_sel == 0:
		# New shop visit (not a return from Options): JE_itemScreen
		# resets curSel[] of each screen to the first row.
		Game.shop_menu_rows = {}
	if _arcade_menu != 0 and vga != null:
		if resume_sel > 2:
			_arcade_sel = resume_sel
		_open_arcade()
		return
	_open_session()
	if _shop != null:
		# Return from Options: the previous selection (game_menu.c keeps curSel),
		# root and "Upgrade Ship".
		if resume_sel != 0 and Game.shop_menu_rows.has("upgrades"):
			_shop.restore_upgrade_row(int(Game.shop_menu_rows["upgrades"]))
		while resume_sel > 2 and int(_state.get("sel", 2)) < resume_sel:
			_shop.key("down")
			_state = _shop.state()
	if _shop == null or vga == null:
		# Shop outside a campaign or without GDExtension: nothing to buy.
		Game.goto_scene.call_deferred(Game.SCENE_TITLE)
		return
	# game_menu.c:JE_itemScreen: JE_loadPic(VGAScreen, 1) + set_palette,
	# copy into VGAScreen2 (background restored at each loop turn).
	vga.load_pic(VGA_SCREEN, 1, true)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	# Own copy: the VGA is shared and the previous screen (title),
	# freed in a deferred way, may still write into VGAScreen2.
	_background = vga.indices(VGA_SCREEN)
	_render()


## game_menu.c:JE_itemScreen: opening of the Rust session with the end-of-level
## equipment and the ]I[ lists rendered by the episode script.
func _open_session() -> void:
	if not Game.campaign_active or not ClassDB.class_exists("TyrianShop"):
		push_error("Shop opened outside a campaign or without the TyrianShop GDExtension")
		return
	_shop = ClassDB.instantiate("TyrianShop")
	# Achievement tracking exists from the shop on: purchases (Shopaholic, Weapon
	# Collector, Full House) counted before any level.
	if ClassDB.class_exists("TyrianVga"):
		ClassDB.class_call_static("TyrianVga", "ach_ensure", Game.ach_dat_path())
	# Tables and "always_available" items of the mods (INJECT_AVAIL of the C).
	if Mods.api != null:
		_shop.set_mods(Mods.api)
	if not _shop.open(Game.data_dir, Game.episode, Game.loadout, Game.shop_items_available, Game.map_sections.size()):
		push_error("TyrianShop.open: %s" % _shop.last_error)
		_shop = null
		return
	# level_load.c ]? : cubes of the section (cubeList); cubeMax after the
	# script directives (Game.shop_cube_max); game_menu.c:load_cubes.
	_shop.set_cubes(Game.data_dir, Game.episode, Game.shop_cube_list, Game.shop_cube_max, TyrianText.language)
	_state = _shop.state()


## Mod objects (names of the items they add).
func _load_mod_items() -> void:
	for it in Mods.items():
		for cat in MOD_KINDS:
			if String(it["kind"]) == MOD_KINDS[cat]:
				_mod_items["%s:%d" % [CATEGORY_TABLES[cat], int(it["id"])]] = it


## Help opened by F1 (built-in instructions.gd), null otherwise.
var _help: Node


## game_menu.c:1791-1820 (F1, non-network): fade_black(10), JE_helpSystem(2)
## (help on topic 2, page 1), then the shop background is reloaded with the
## current menu's palette and a fade-in (backFromHelp). The shop's
## song is restarted (play_song(songBuy)).
func _open_help() -> void:
	idle = false
	var help: Node = INSTRUCTIONS.new()
	help.name = "ShopHelp"
	help.z_index = 10
	help.embedded = true
	help.start_topic = 2
	help.closed.connect(_close_help)
	_help = help
	await fade_black(10)
	overlay.visible = false
	add_child(help)


func _close_help() -> void:
	_help.queue_free()
	_help = null
	overlay.visible = true
	Music.play_song(Music.shop_song(Game.episode, Game.shop_section))
	var menu := int(_state.get("menu", 0)) if _arcade_menu == 0 else 0
	vga.load_palette(NAV_PALETTE if menu == MENU_PLAY_NEXT_LEVEL else 0)
	vga.set_colors(Color.BLACK, 0, 255)
	_render()
	# game_menu.c:1418-1433: the return fade (backFromHelp) follows JE_mouseStart,
	# the pointer stays drawn there.
	await fade_palette_over_pointer(10, 0, 255)


## Key forwarded to TyrianShop, then possible action (exit, subscreen).
func _shop_key(name: String) -> void:
	if name == "select":
		# JE_menuFunction: col = 0, colC = -1.
		_flash_col = 0
		_flash_col_c = -1
	var before := int(_state.get("menu", 0))
	var r: Dictionary = _shop.key(name)
	# game_menu.c:258 shop_commit_done: S_ITEM after the S_CLICK of the confirmation.
	if bool(r.get("committed", false)):
		Sfx.play(S_ITEM)
	_shop_after(r, before)


## State and action after a shop key or click (the click follows the
## same path, game_menu.c:JE_itemScreen).
func _shop_after(r: Dictionary, before: int) -> void:
	_state = _shop.state()
	if before != MENU_PLAY_NEXT_LEVEL and int(_state["menu"]) == MENU_PLAY_NEXT_LEVEL:
		_nav_open()
	match str(r.get("action", "")):
		"leave":
			Game.campaign_leave_shop(int(r["choice"]), _shop.loadout())
			return
		"quit_request":
			_quit_request()
			return
		"read_cube":
			# JE_menuFunction, MENU_DATA_CUBES: yLoc = yChg = 0.
			_cube_y = 0
			_cube_wheel_lines = 0
			_cube_chg = 0
		"entry":
			# Options: save / load (save_menu.gd, ADR 0129), with
			# the current equipment; Ship Specs: enlarged green screen.
			if int(r["entry"]) == 5:
				Game.loadout = _shop.loadout()
				Game.shop_resume_sel = 5
				Game.shop_menu_rows["upgrades"] = int(_state.get("category", 2))
				Game.goto_scene(Game.SCENE_SAVE_MENU)
				return
			if int(r["entry"]) == 3:
				_ship_specs()
				return
	_render()


## game_menu.c:JE_quitRequest (Esc at the root or "Quit Game"): Yes / No
## box on the current screen; Yes abandons the game (back to the title).
func _quit_request() -> void:
	idle = false
	if await quit_request_msg(TyrianText.L("miscText_28"), TyrianText.L("miscText_30")):
		Game.campaign_active = false
		Game.goto_scene(Game.SCENE_TITLE)
		return
	_render()


## Menu action (keyboard, gamepad or capture tool --keys; TyrianScreen
## routes the events, ADR 0120). Space and Enter = "select" like the C code.
func handle_action(action: StringName) -> void:
	if (_shop == null and _arcade_menu == 0) or modal_input(action):
		return
	if _specs_open:
		# JE_doShipSpecs: wait_input, any key.
		_specs_key = true
		return
	if _arcade_menu != 0:
		_arcade_action(action)
		return
	if int(_state.get("menu", 0)) == MENU_DATA_CUBE_SUB and action in [TyrianControls.UP, TyrianControls.DOWN]:
		# The text scrolls as long as the key is held (_process); the press
		# itself does nothing visible (the C's curSel[MENU_DATA_CUBE_SUB]).
		return
	const NAMES := {TyrianControls.UP: "up", TyrianControls.DOWN: "down", TyrianControls.LEFT: "left",
		TyrianControls.RIGHT: "right", TyrianControls.ACCEPT: "select", TyrianControls.SELECT: "select",
		TyrianControls.BACK: "escape"}
	if NAMES.has(action):
		# game_menu.c:2813: JE_menuFunction plays S_CLICK at each confirmation;
		# Up / Down S_CURSOR (game_menu.c:1826, 1857); Esc S_SPRING (1761);
		# Left / Right S_CURSOR only in a purchase list here
		# (game_menu.c:1949-1954, 2041-2046; Options is save_menu.gd), silent elsewhere.
		var name: String = NAMES[action]
		var menu := int(_state.get("menu", 0))
		match name:
			"select":
				Sfx.play(Sfx.CLICK)
			"escape":
				Sfx.play(Sfx.SPRING)
			"left", "right":
				if menu == MENU_UPGRADE_SUB:
					Sfx.play(Sfx.CURSOR)
			_:
				Sfx.play(Sfx.CURSOR)
		_shop_key(name)


## "/" (SDL_SCANCODE_SLASH, no menu action): mode of the rear weapon
## in its category; the other events follow TyrianScreen.
func handle_event(event: InputEvent) -> void:
	if _help != null:
		return
	if (
		TyrianControls.global_pressed(event, TyrianControls.HELP)
		and idle
		and not modal_active
		and not _specs_open
		and (_shop != null or _arcade_menu != 0)
	):
		_open_help()
		return
	if event is InputEventKey and event.pressed and not event.echo and event.physical_keycode == KEY_SLASH \
			and _shop != null and not modal_active and not _specs_open:
		_shop_key("slash")
		return
	# game_menu.c:1884-1892: Home / End in an open cube = top / bottom of the
	# text. The C code writes (last_line - 9) * 12 into a uint16_t: on a cube of
	# fewer than 10 lines, End emptied the text; clamped to 0 here.
	if event is InputEventKey and event.pressed and not event.echo \
			and event.physical_keycode in [KEY_HOME, KEY_END] and _shop != null and idle \
			and not modal_active and not _specs_open and int(_state.get("menu", 0)) == MENU_DATA_CUBE_SUB:
		var lines: int = _shop.cube_text(int(_state.get("cube_sel", 2)) - 2).size()
		_cube_y = 0 if event.physical_keycode == KEY_HOME else maxi(0, (lines - 9) * CUBE_LINE_H)
		_cube_chg = 0
		_cube_wheel_lines = 0
		_render()
		return
	super(event)


## Mouse (game_menu.c:JE_itemScreen, "mouseButton > 0" branch): hover
## (menu_row_under_mouse), click (any button acts like the left one: the selected
## row activates, another selects); wheel = one row, like the
## arrows (SDL has no wheel: rewrite addition).
func handle_mouse(event: InputEvent) -> void:
	if _help != null:
		return
	if _arcade_menu != 0 and _shop == null and not modal_active:
		_arcade_mouse(event)
		return
	if _shop == null or modal_active:
		super(event)
		return
	if _specs_open:
		# JE_doShipSpecs: wait_input, any button.
		if TyrianMouse.is_button_press(event):
			_specs_key = true
		return
	if TyrianMouse.is_wheel(event):
		# Wheel (maintainer, 2026-10-10): the text of an open cube scrolls by one
		# line per notch, like pressing Up / Down; no effect elsewhere.
		if int(_state.get("menu", 0)) == MENU_DATA_CUBE_SUB:
			_cube_wheel_lines += TyrianMouse.wheel_step(event)
		return
	var p := Vector2i(event.position.floor())
	if int(_state.get("menu", 0)) == MENU_DATA_CUBE_SUB:
		_cube_mouse(event, p)
		return
	mouse_pointer = MouseCursor.NORMAL
	if (
		int(_state.get("menu", 0)) == MENU_DATA_CUBES
		and PackedStringArray(_state.get("cube_titles", PackedStringArray())).is_empty()
	):
		# game_menu.c:1594-1599: empty list, any button goes back to the menu.
		if TyrianMouse.is_button_press(event):
			Sfx.play(Sfx.SPRING)
			_shop_key("escape")
		return
	var row: int = _shop.row_under_mouse(p.x, p.y, _shop_scroll)
	if event is InputEventMouseMotion:
		if not TyrianMouse.inactive and event.relative != Vector2.ZERO:
			if int(_state.get("menu", 0)) == MENU_UPGRADE_SUB:
				_hover_note(row, hover_clock.call())
			else:
				_apply_hover(row)
		return
	if not TyrianMouse.is_button_press(event):
		return
	# A click acts on its own row (or arrow): a pending hover is dropped.
	_hover_pending = -1
	# game_menu.c:JE_itemScreen: power arrows (front and rear weapons),
	# tested outside the choices column, after the click on a row.
	if row < 0 and _click_power_arrow(p):
		return
	if row < 0:
		return
	# JE_menuFunction: col = 0, colC = -1; the clicked row selects then acts.
	_flash_col = 0
	_flash_col_c = -1
	var before := int(_state.get("menu", 0))
	var r: Dictionary = _shop.click_row(row)
	# game_menu.c: S_CLICK (S_CLINK item too expensive, S_ITEM purchase paid).
	match str(r.get("sound", "")):
		"click":
			Sfx.play(Sfx.CLICK)
		"clink":
			Sfx.play(Sfx.CLINK)
		"item":
			Sfx.play(S_ITEM)
	_shop_after(r, before)


## Hover (game_menu.c:JE_itemScreen, mouse moved with no button): row `row`
## becomes the selection (S_CURSOR) if it changes it.
func _apply_hover(row: int) -> void:
	if _shop.hover_row(row):
		Sfx.play(Sfx.CURSOR)
		# State re-read: without it _render redrew the old selection.
		_state = _shop.state()
		_render()


## Purchase list: the pointer is on `row` (-1 outside the choices column) at
## `now` ms. A new row starts its dwell; the hover applies in `hover_tick`.
func _hover_note(row: int, now: int) -> void:
	var menu := int(_state.get("menu", 0))
	if row != _hover_pending or menu != _hover_menu:
		_hover_pending = row
		_hover_since_ms = now
		_hover_menu = menu
	hover_tick(now)


## Applies the pending hover once its row has been held HOVER_DWELL_MS
## (called on each movement and each frame). Dropped if the menu changed or
## the keyboard took over (mouseInactive).
func hover_tick(now: int) -> void:
	if _hover_pending < 0:
		return
	if TyrianMouse.inactive or _shop == null or int(_state.get("menu", 0)) != _hover_menu:
		_hover_pending = -1
		return
	if now - _hover_since_ms < HOVER_DWELL_MS:
		return
	var row := _hover_pending
	_hover_pending = -1
	_apply_hover(row)


## Reading a cube with the mouse (game_menu.c:1262-1278, 1529-1532, 1597-1604):
## inside the text (164 < x < 299, 47 < y < 153) the pointer becomes an up
## (y <= 100) or down arrow, button held = continuous scrolling (_process);
## click on "EXIT" (258 < x < 290, 159 < y < 171) = back to the list.
func _cube_mouse(event: InputEvent, p: Vector2i) -> void:
	if p.x > 164 and p.x < 299 and p.y > 47 and p.y < 153:
		mouse_pointer = MouseCursor.UP if p.y <= 100 else MouseCursor.DOWN
	else:
		mouse_pointer = MouseCursor.NORMAL
	if TyrianMouse.is_button_press(event) and p.x > 258 and p.x < 290 and p.y > 159 and p.y < 171:
		Sfx.play(Sfx.SPRING)
		_shop_key("escape")
		# game_menu.c: hover restarts from the last known point, the cube list
		# immediately takes the row under the pointer (here "Exit to Game Menu").
		var row: int = _shop.row_under_mouse(p.x, p.y, _shop_scroll)
		if row >= 0 and _shop.hover_row(row):
			Sfx.play(Sfx.CURSOR)
			_state = _shop.state()
			_render()


## Mouse scroll direction in a cube (0: none).
func _cube_mouse_dir() -> int:
	if not TyrianMouse.any_button_down() or TyrianMouse.inactive:
		return 0
	if mouse_pointer == MouseCursor.UP:
		return -1
	if mouse_pointer == MouseCursor.DOWN:
		return 1
	return 0


## Remapping of the clicked rank of the 2-player arcade Game Menu (game_menu.c:142-148).
static func _arcade_row_2p(selection: int) -> int:
	if selection > 5:
		selection -= 1
	if selection > 3:
		selection -= 1
	return selection


## game_menu.c:menu_row_under_mouse for MENU_2_PLAYER_ARCADE: raw rank of
## the screen, (mouseY - 38) / 16 + 2, remapped ("selection > 5 -> selection--"
## then "selection > 3 -> selection--", 16 px blanks of the drawing), then clamped
## to menuChoices. The menu engine's rank ignores the blanks: starting from it
## missed "Options" and "Quit Game" (hovering Quit = Player 2 Input).
## Returns the row index (0 = Start Level) or -1.
static func _arcade_row_2p_at(p: Vector2i, menu_choices: int) -> int:
	if p.x <= 170 or p.x >= 308 or p.y <= 20:
		return -1
	var selection := _arcade_row_2p((p.y - 38) / 16 + 2)
	if selection < 2 or selection > menu_choices:
		return -1
	return selection - 2


## Mouse on the title mode menus (1 / 2-player arcade, Super Tyrian):
## same "mouseButton > 0" branch of JE_itemScreen as the shop. Hover =
## row selected (S_CURSOR); click, any button (S_CLICK): the selected row acts,
## another is selected; wheel = one row (rewrite addition).
func _arcade_mouse(event: InputEvent) -> void:
	if get("_busy") == true or not idle:
		return
	if TyrianMouse.is_wheel(event):
		return
	if not (event is InputEventMouse):
		return
	TyrianMouse.note(event)
	var p := Vector2i(event.position.floor())
	var row: int = menus.row_at(menus.current(), p)
	if _arcade_menu == 9:
		row = _arcade_row_2p_at(p, menus.current().items.size() + 1)
	if event is InputEventMouseMotion:
		if not TyrianMouse.inactive and event.relative != Vector2.ZERO and row >= 0 and row + 2 != _arcade_sel:
			Sfx.play(Sfx.CURSOR)
			_arcade_sel = row + 2
			_state["sel"] = _arcade_sel
			_render()
		return
	if not TyrianMouse.is_button_press(event) or row < 0:
		return
	# JE_itemScreen: S_CLICK in both cases, then JE_menuFunction if the row was already selected.
	Sfx.play(Sfx.CLICK)
	if row + 2 == _arcade_sel:
		_arcade_function()
		return
	_arcade_sel = row + 2
	_state["sel"] = _arcade_sel
	_render()


## game_menu.c:JE_itemScreen: x 23-36 lowers the power (leftPower), x 119-131
## raises it (rightPower && rightPowerAfford), y 149-168; S_CURSOR on click,
## S_CLINK if the power cannot change. True if the click was on an arrow.
func _click_power_arrow(p: Vector2i) -> bool:
	if int(_state.get("menu", 0)) != MENU_UPGRADE_SUB or not int(_state.get("category", 0)) in [3, 4]:
		return false
	if p.y < 149 or p.y > 168:
		return false
	var raise := p.x >= 119 and p.x <= 131
	if not raise and not (p.x >= 23 and p.x <= 36):
		return false
	Sfx.play(Sfx.CURSOR)
	var result: String = _shop.click_power_arrow(raise)
	if result == "refused":
		Sfx.play(Sfx.CLINK)
	_state = _shop.state()
	_render()
	return true


## Mod PNG image for item `id` of `item_type` (1 ship, 2-3 weapons,
## 4 shield), or null: then the original illustration (original tables) applies.
func _mod_image(item_type: int, id: int) -> Texture2D:
	var table: String = CATEGORY_TABLES.get(item_type + 1, "")
	var it: Dictionary = _mod_items.get("%s:%d" % [table, id], {})
	if it.is_empty():
		return null
	match item_type:
		1:
			return Mods.texture("ships", int(it.get("shipgraphic", 0)))
		2, 3:
			return Mods.texture("shop_weapons", int(it.get("itemgraphic", 0)))
		4:
			return Mods.texture("shop_shields", int(it.get("itemgraphic", 0)))
	return null


## game_menu.c:JE_drawItem: a mod item that provides a PNG is laid on
## the mod layer; the others follow the original drawing.
func _draw_shop_item(item_type: int, id: int, x: int, y: int, surf := VGA_SCREEN) -> void:
	var tex := _mod_image(item_type, id) if id > 0 else null
	if tex == null:
		super(item_type, id, x, y, surf)
		return
	if surf == VGA_SCREEN:
		_mod_images[Vector2i(x, y)] = tex


## Rebuilds the mod image layer after each presented frame.
func _sync_mod_images(_surf: int) -> void:
	if _mod_image_layer == null:
		return
	var n := _mod_images.size()
	while _mod_image_layer.get_child_count() > n:
		var extra := _mod_image_layer.get_child(_mod_image_layer.get_child_count() - 1)
		_mod_image_layer.remove_child(extra)
		extra.queue_free()
	while _mod_image_layer.get_child_count() < n:
		var sprite := Sprite2D.new()
		sprite.centered = false
		sprite.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		_mod_image_layer.add_child(sprite)
	var i := 0
	for pos in _mod_images:
		var sprite := _mod_image_layer.get_child(i) as Sprite2D
		sprite.texture = _mod_images[pos]
		sprite.position = Vector2(pos)
		i += 1


func _item(cat: int, id: int) -> Dictionary:
	var table: String = CATEGORY_TABLES.get(cat, "")
	var mod_key := "%s:%d" % [table, id]
	if _mod_items.has(mod_key):
		return _mod_items[mod_key]
	var rows: Array = _items_json.get(table, [])
	if id >= 0 and id < rows.size():
		return rows[id]
	return {}


# --- 8-bit rendering of game_menu.c:JE_itemScreen (ADR 0110) -------------------

## Current menu of the session, drawn by the shared engine in
## `external` mode: it does not navigate, it draws the state of TyrianShop.
func _render() -> void:
	idle = false
	_mod_images.clear()
	if int(_state.get("menu", 0)) != MENU_DATA_CUBE_SUB:
		mouse_pointer = MouseCursor.NORMAL
	_anim_delay = 0
	_redraw_only = false
	if _arcade_menu != 0:
		_render_arcade()
		return
	var menu := int(_state["menu"])
	var sel := int(_state["sel"])
	var lo: PackedInt32Array = _state["loadout"]
	if menu in [MENU_DATA_CUBES, MENU_DATA_CUBE_SUB]:
		_render_cubes(menu)
		return
	if _face_palette:
		# Exit from the cubes: newPal = 1 (shop palette).
		_face_palette = false
		vga.load_palette(0)
		vga.set_palette(0, 255)
	if _nav_palette != (menu == MENU_PLAY_NEXT_LEVEL):
		# Map: newPal = 18; return to the menu: newPal = 1.
		_nav_palette = menu == MENU_PLAY_NEXT_LEVEL
		vga.load_palette(NAV_PALETTE if _nav_palette else 0)
		vga.set_palette(0, 255)
	if menu == MENU_UPGRADE_SUB:
		vga.set_indices(VGA_SCREEN, _background)
		_render_category(sel, lo)
		return
	var header := ""
	var items := []
	if menu == MENU_PLAY_NEXT_LEVEL:
		# JE_itemScreen, each turn of the outer loop: animation restarted.
		_planet_ani = 0
		_planet_ani_wait = 3
		_dot_num = 0
		_dot_wait = 8
		# game_menu.c:JE_menuFunction (case 6): menuInt[4] = "Next Level",
		# planet names (pName[mapPlanet - 1]), "Exit to Game Menu";
		# JE_drawMenuChoices shifts the last row by 16 px.
		header = TyrianText.L("shop_next_level")
		for i in Game.map_sections.size():
			var planet := Game.map_planets[i] if i < Game.map_planets.size() else 1
			items.append(MenuEngine.item(TyrianText.L("planet_%d" % (planet - 1)), "", MenuEngine.Kind.ACTION))
		items.append(MenuEngine.item(TyrianText.L("miscText_5"), "", MenuEngine.Kind.ACTION, {"gap": 16}))
	else:
		# Root and "Upgrade Ship" (ADR 0123).
		header = TyrianText.L(MENU_INT[menu][0])
		for x in range(2, MENU_CHOICES[menu] + 1):
			items.append(MenuEngine.item(TyrianText.L(MENU_INT[menu][x - 1]), "", MenuEngine.Kind.ACTION,
				{"gap": 16 if menu == 0 and x == 7 else 0}))
	menus.stack.clear()
	var background := func() -> void:
		if not _redraw_only:
			vga.set_indices(VGA_SCREEN, _background)
		if menu == MENU_PLAY_NEXT_LEVEL:
			# JE_updateNavScreen, before JE_drawMenuHeader / JE_drawMenuChoices.
			_nav_update()
	menus.open(header, items, {
		"style": MenuEngine.Style.GAME_MENU, "external": true, "sel": sel - 2, "draw": false,
		"background": background,
		"decorate": _decorate_menu.bind(menu, sel, lo),
	})
	if menu == MENU_PLAY_NEXT_LEVEL:
		# Outer loop (background, map, menu, help), then inner loop:
		# map and text redrawn on the current image at each turn.
		menus.render()
		_redraw_only = true
		_anim_begin(2, menus.render)
		return
	menus.draw()


## Elements of JE_itemScreen around the choices: data cubes, money,
## bars, illustration, help (MENU_FULL_GAME, MENU_UPGRADES); help of
## MENU_PLAY_NEXT_LEVEL (the map frame is in the background).
func _decorate_menu(menu: int, sel: int, lo: PackedInt32Array) -> void:
	if menu == MENU_PLAY_NEXT_LEVEL:
		# JE_drawMainMenuHelpText: Exit -> mainMenuHelp[12-1], otherwise [17 + 3 - 3].
		var last := sel - 2 == Game.map_sections.size()
		vga.text_shade(VGA_SCREEN, 10, 187, TyrianText.L("mmhelp_%d" % (11 if last else 17)), 14, 1, 2)
		# game_menu.c: "if (extraGame) JE_dString(170, 140, miscText[68 - 1],
		# FONT_SHAPES)" — ]x[: a map destination is a mini-game.
		if Game.extra_game:
			vga.dstring(VGA_SCREEN, 170, 140, TyrianText.L("miscText_67"), LARGE_FONT)
		return
	if menu == 0:
		# Data cube icons after "Data".
		var cubes := _cube_count()
		var icon_x := 166 + TyrianText.width(TyrianText.L(MENU_INT[0][1]), NORMAL_FONT) + 8
		icon_x = maxi(icon_x, 208)
		var max_x := 318 - 14 - (cubes - 1) * 18
		if cubes > 0 and icon_x > max_x:
			icon_x = max_x
		for i in cubes:
			vga.blit_sprite_dark(VGA_SCREEN, icon_x + i * 18 + 2, 37 + 1, OPTION_SHAPES, 34, false)
			vga.blit_sprite(VGA_SCREEN, icon_x + i * 18, 37, OPTION_SHAPES, 34)
	# Money, armor, shield (curMenu <= MENU_OPTIONS, one player).
	var ship := _base_item("ships", lo[0])
	var shield := _base_item("shields", lo[5])
	vga.text_shade(VGA_SCREEN, 65, 173, str(int(_state["cash"])), 1, 6, 2)
	bar_draw_shadow(42, 152, 3, 14, int(ship.get("dmg", 0)), 2, 13)
	bar_draw_shadow(104, 152, 4, 14, int(shield.get("mpwr", 0)) * 2, 2, 13)
	_draw_ship_illustration(lo, ship, shield)
	# JE_drawMainMenuHelpText (curMenu < MENU_PLAY_NEXT_LEVEL).
	var help: int = MENU_HELP[menu][sel - 2]
	vga.text_shade(VGA_SCREEN, 10, 187, TyrianText.L("mmhelp_%d" % (help - 1)), 14, 1, 2)


## Row of items.json (game tables, without the mods): bars and illustration.
func _base_item(table: String, id: int) -> Dictionary:
	var rows: Array = _items_json.get(table, [])
	return rows[id] if id >= 0 and id < rows.size() else {}


## C cubeMax: cubes readable at this shop (TyrianShop.set_cubes).
func _cube_count() -> int:
	return clampi(int(_state.get("cube_max", 0)), 0, 4)


## game_menu.c:JE_itemScreen, curMenu == MENU_UPGRADE_SUB (item list).
func _render_category(sel: int, lo: PackedInt32Array) -> void:
	var cat := int(_state["category"])
	var choices := int(_state["menu_choices"])
	var total := choices - 1
	# Item list + "Done": shared engine, ROWS style (ADR 0121,
	# 0123); selection and purchases held by TyrianShop.
	var rows := []
	for w in range(1, choices):
		rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"w": w, "draw": _draw_category_row}))
	menus.stack.clear()
	menus.open(TyrianText.L(MENU_INT[1][cat - 1]), rows, {
		"style": MenuEngine.Style.ROWS, "external": true, "draw": false,
		"sel": clampi(sel - 2, 0, maxi(total - 1, 0)),
		"row_x": 160, "row_y": 40, "row_step": 26, "visible": UPGRADE_VISIBLE,
		"background": func() -> void: vga.set_indices(VGA_SCREEN, _background),
		"header_draw": func() -> void: menus.draw_game_menu_header(menus.current().header),
		# game_menu.c: shop_scroll follows curSel - 2, clamped to the list.
		"window": func(_engine_sel: int, _n: int) -> int:
			if total <= UPGRADE_VISIBLE:
				_shop_scroll = 0
			else:
				var c0 := sel - 2
				if c0 < _shop_scroll:
					_shop_scroll = c0
				elif c0 >= _shop_scroll + UPGRADE_VISIBLE:
					_shop_scroll = c0 - UPGRADE_VISIBLE + 1
				_shop_scroll = clampi(_shop_scroll, 0, total - UPGRADE_VISIBLE)
			return _shop_scroll,
	})
	menus.render()
	_draw_scroll_arrows(total)
	var ship := _base_item("ships", lo[0])
	var shield := _base_item("shields", lo[5])
	var weapon_sim := cat in WEAPON_SIM_CATEGORIES
	# Armor and shield bars: curSel[MENU_UPGRADES] from 1 to 6.
	if cat <= 6:
		bar_draw_shadow(42, 152, 3, 14, int(ship.get("dmg", 0)), 2, 13)
		bar_draw_shadow(104, 152, 4, 14, int(shield.get("mpwr", 0)) * 2, 2, 13)
	if not weapon_sim:
		_draw_ship_illustration(lo, ship, shield)
		# JE_drawScore: JE_cashLeft(). Called twice before the first
		# JE_showVGA (step VI, then the inner loop setDelay(2)): the DARKEN
		# shadow is darkened twice.
		for i in 2:
			vga.text_shade(VGA_SCREEN, 65, 173, str(int(_state["cash_left"])), 1, 6, 2)
	# JE_drawMainMenuHelpText: power arrows -> mainMenuHelp[24-1];
	# Done -> [12-1]; otherwise [17 + 4 - 3].
	var help := 18
	if weapon_sim and (_state.get("left_power", false) or _state.get("right_power", false)):
		help = 23
	elif sel - 2 == choices - 2:
		help = 11
	vga.text_shade(VGA_SCREEN, 10, 187, TyrianText.L("mmhelp_%d" % help), 14, 1, 2)
	if weapon_sim:
		# Inner loop, setDelay(3): JE_weaponSimUpdate then JE_drawScore.
		_anim_begin(3, _weapon_sim_frame.bind(cat, lo))
		return
	present()
	# Inner loop (setDelay(2), without JE_weaponSimUpdate): each turn
	# redraws JE_drawScore without background, the shadow darkens to black;
	# identical images discarded by the capture deduplication.
	for i in 3:
		vga.text_shade(VGA_SCREEN, 65, 173, str(int(_state["cash_left"])), 1, 6, 2)
		present()
	idle = true


## One turn of the firing demonstration (shop_preview.c:JE_weaponSimUpdate):
## JE_weaponViewFrame (TyrianShop: stars, shots, frame, rear-mode
## indicator, energy bar), power interface of the front /
## rear weapons, motionless ship at (72, 110), then JE_drawScore.
func _weapon_sim_frame(cat: int, lo: PackedInt32Array) -> void:
	var sim_time: int = _shop.weapon_view_frame(vga)
	var left: bool = _state.get("left_power", false)
	var right: bool = _state.get("right_power", false)
	if left or right:
		if not left:
			vga.blit_sprite(VGA_SCREEN, 24, 149, OPTION_SHAPES, 13)  # downgrade impossible
		if not right or not _state.get("right_power_afford", false):
			vga.blit_sprite(VGA_SCREEN, 119, 149, OPTION_SHAPES, 14)  # upgrade impossible
		var power := lo[2] if cat == 3 else lo[4]
		if cat == 4 and int(_state.get("rear_mode_count", 1)) == 2 and sim_time >= 75:
			# Every other half-period: "[/] Rear Weapon Mode".
			vga.out_text(VGA_SCREEN, 28, 137, TyrianText.L("miscText_70"), 1, 4)
		else:
			if left:
				vga.out_text(VGA_SCREEN, 26, 137, str(int(_state.get("downgrade_cost", 0))), 1, 4)
			if right:
				vga.out_text(VGA_SCREEN, 108, 137, str(int(_state.get("upgrade_cost", 0))),
						1 if _state.get("right_power_afford", false) else 7, 4)
			vga.out_text(VGA_SCREEN, 58, 137, "%s %d" % [TyrianText.L("miscTextB_5"), power], 15, 4)
		for x in range(1, power + 1):
			vga.fill_rectangle_xy(VGA_SCREEN, 39 + x * 6, 151, 39 + x * 6 + 4, 151, 251)
			vga.set_pixel(VGA_SCREEN, 39 + x * 6, 151, 252)
			vga.fill_rectangle_xy(VGA_SCREEN, 39 + x * 6, 152, 39 + x * 6 + 4, 164, 250)
			vga.fill_rectangle_xy(VGA_SCREEN, 39 + x * 6, 165, 39 + x * 6 + 4, 165, 249)
	else:
		vga.blit_sprite(VGA_SCREEN, 20, 146, OPTION_SHAPES, 17)  # power interface hidden
	_draw_shop_item(1, lo[0], 72 - 5, 110 - 7)
	vga.text_shade(VGA_SCREEN, 65, 173, str(int(_state["cash_left"])), 1, 6, 2)


# --- Animated inner loop -----------------------------------------------------

## First turn of the inner loop (presented), then one turn every
## `delay` setDelay periods in _process.
func _anim_begin(delay: int, step: Callable) -> void:
	_anim_step = step
	_anim_delay = delay
	_anim_clock = 0.0
	var n := int(Capture.option("shop-sim-frames")) if Capture != null else 0
	_anim_left = n if n > 0 else -1
	_anim_frame()


func _anim_frame() -> void:
	_anim_step.call()
	present()
	if _anim_left > 0:
		_anim_left -= 1
	idle = _anim_left <= 0


## One turn per `_anim_delay` periods, never two per displayed frame.
func _anim_tick(delta: float) -> void:
	_anim_clock = minf(_anim_clock + delta, 2 * _anim_delay * DELAY_PERIOD)
	if _anim_clock >= _anim_delay * DELAY_PERIOD:
		_anim_clock -= _anim_delay * DELAY_PERIOD
		_anim_frame()


# --- "Next Level" map (game_menu_nav.c) ------------------------------------------

## JE_menuFunction, MENU_FULL_GAME entry 6: JE_computeDots, view centered on
## the starting planet (mapOrigin).
func _nav_open() -> void:
	var o := _planet_index(Game.map_origin)
	_planet_dots = PackedInt32Array()
	_dot_x.clear()
	_dot_y.clear()
	for planet in Game.map_planets:
		var p := _planet_index(planet)
		var dx: int = PLANET_X[p] - PLANET_X[o]
		var dy: int = PLANET_Y[p] - PLANET_Y[o]
		var dots := 0
		if absi(dx) + absi(dy) != 0:
			dots = mini(roundi(sqrt(sqrt(float(dx * dx + dy * dy)))) - 1, 10)
		var xs := PackedInt32Array()
		var ys := PackedInt32Array()
		for y in dots:
			# JE_partWay: (finish - start) / (dots + 2) * (dist + 1) + start.
			# (integer division truncated toward zero, like in C).
			xs.append(dx / (dots + 2) * (y + 1) + PLANET_X[o])
			ys.append(dy / (dots + 2) * (y + 1) + PLANET_Y[o])
		_planet_dots.append(dots)
		_dot_x.append(xs)
		_dot_y.append(ys)
	_nav_x = PLANET_X[o]
	_nav_y = PLANET_Y[o]
	_new_nav_x = _nav_x
	_new_nav_y = _nav_y


## 1-based planet of ]G[ -> table index (0..20).
func _planet_index(planet: int) -> int:
	return clampi(planet - 1, 0, PLANET_X.size() - 1)


## JE_updateNavScreen: map (grid, paths, planets), frame, view
## sliding toward the middle of the selected path, animations.
func _nav_update() -> void:
	var tx := roundi(_nav_x)
	var ty := roundi(_nav_y)
	vga.fill_rectangle_xy(VGA_SCREEN, 19, 16, 135, 169, 2)
	_draw_nav_lines(tx, ty, true)
	_draw_nav_lines(tx, ty, false)
	# JE_drawDots: lit dots (30) up to currentDotNum on the chosen path.
	var sel := int(_state["sel"]) - 2
	for x in _planet_dots.size():
		for y in _planet_dots[x]:
			var px: int = _dot_x[x][y] - tx + 66 - 2
			var py: int = _dot_y[x][y] - ty + 85 - 2
			if px > 0 and px < 140 and py > 0 and py < 168:
				vga.blit_sprite(VGA_SCREEN, px, py, OPTION_SHAPES, 30 if x == sel and y < _dot_num else 29)
	for p in 11:
		_draw_planet(p, tx, ty)
	# Destinations and start outside the first 11 planets. The C code iterates
	# one cell too many (menuChoices - 1 = mapPNum + 1) and may draw a
	# planet left over from a previous map: only the mapPNum cells here.
	for planet in Game.map_planets:
		if planet > 11:
			_draw_planet(_planet_index(planet), tx, ty)
	if Game.map_origin > 11:
		_draw_planet(_planet_index(Game.map_origin), tx, ty)
	vga.blit_sprite(VGA_SCREEN, 0, 0, OPTION_SHAPES, 28)  # map frame
	if sel < Game.map_planets.size():
		var o := _planet_index(Game.map_origin)
		var d := _planet_index(Game.map_planets[sel])
		var os: Vector2i = TyrianVga.sprite_size(PLANET_SHAPES, PGR[o] - 1) / 2
		var ds: Vector2i = TyrianVga.sprite_size(PLANET_SHAPES, PGR[d] - 1) / 2
		_new_nav_x = (PLANET_X[o] - os.x + PLANET_X[d] - ds.x) / 2.0
		_new_nav_y = (PLANET_Y[o] - os.y + PLANET_Y[d] - ds.y) / 2.0
	_nav_x += (_new_nav_x - _nav_x) / 2.0
	_nav_y += (_new_nav_y - _nav_y) / 2.0
	if absf(_new_nav_x - _nav_x) < 1:
		_nav_x = _new_nav_x
	if absf(_new_nav_y - _nav_y) < 1:
		_nav_y = _new_nav_y
	vga.fill_rectangle_xy(VGA_SCREEN, 314, 0, 319, 199, 230)
	if _planet_ani_wait > 0:
		_planet_ani_wait -= 1
	else:
		_planet_ani = 0 if _planet_ani >= 14 else _planet_ani + 1
		_planet_ani_wait = 3
	if _dot_wait > 0:
		_dot_wait -= 1
	else:
		if sel < _planet_dots.size() and _dot_num < _planet_dots[sel]:
			_dot_num += 1
		_dot_wait = 5


## JE_drawNavLines: grid scrolling with the view (half speed), in
## two passes (shadow offset by one pixel, then light lines); crosses at the
## intersections (JE_pix3).
func _draw_nav_lines(tx: int, ty: int, dark: bool) -> void:
	var ox := tx >> 1
	var oy := ty >> 1
	for x in range(1, 21):
		var lx := x * 15 - ox
		if lx > 18 and lx < 135:
			if dark:
				vga.rectangle(VGA_SCREEN, lx + 1, 16, lx + 1, 169, 1)
			else:
				vga.rectangle(VGA_SCREEN, lx, 16, lx, 169, 5)
	for y in range(1, 21):
		var ly := y * 15 - oy
		if ly > 15 and ly < 169:
			if dark:
				vga.rectangle(VGA_SCREEN, 19, ly + 1, 135, ly + 1, 1)
			else:
				vga.rectangle(VGA_SCREEN, 8, ly, 160, ly, 5)
			for x in range(1, 21):
				var lx := x * 15 - ox
				if lx > 18 and lx < 135:
					for d in [Vector2i.ZERO, Vector2i.LEFT, Vector2i.RIGHT, Vector2i.UP, Vector2i.DOWN]:
						vga.set_pixel(VGA_SCREEN, lx + d.x, ly + d.y, 7)


## JE_drawPlanet: shadow offset by (3, 3) then planet, if it is in view.
func _draw_planet(p: int, tx: int, ty: int) -> void:
	var z: int = PGR[p] - 1
	var size := TyrianVga.sprite_size(PLANET_SHAPES, z)
	var x: int = PLANET_X[p] + 66 - tx - size.x / 2
	var y: int = PLANET_Y[p] + 85 - ty - size.y / 2
	if x > -7 and x + size.x < 170 and y > 0 and y < 160:
		if PANI[p]:
			z += _planet_ani
		vga.blit_sprite_dark(VGA_SCREEN, x + 3, y + 3, PLANET_SHAPES, z, false)
		vga.blit_sprite(VGA_SCREEN, x, y, PLANET_SHAPES, z)


## A row of the item list (game_menu.c:JE_itemScreen,
## MENU_UPGRADE_SUB): "owned" mark, name, JE_drawItem icon, cost.
func _draw_category_row(it: Dictionary, _x: int, y: int, _selected: bool) -> void:
	var w: int = it.w
	var sel := int(_state["sel"])
	var cat := int(_state["category"])
	var ids: PackedInt32Array = _state["items"]
	var costs: PackedInt32Array = _state["costs"]
	var choices := int(_state["menu_choices"])
	var cash := int(_state["cash"])
	var owned := int(_state["owned_item"])
	var chosen := int(_state["chosen_item"])
	var done := w == choices - 1
	var cost := costs[w - 1] if not done and w - 1 < costs.size() else 0
	var shade := 4 if cost > cash else 0
	var item := ids[w - 1] if w - 1 < ids.size() else 0
	var color := 15 if w == sel - 1 else 28
	if sel == choices and not done and item == chosen:
		color = 15
	if item == owned and item != 0 and not done:
		vga.fill_rectangle_xy(VGA_SCREEN, 160, y + 7, 300, y + 11, 227)
		vga.blit_sprite2(VGA_SCREEN, 298, y + 2, "newsh1.shp", 247, 0, 0, false)
	var name := TyrianText.L("misc_done2") if done else String(_item(cat, item).get("name", ""))
	vga.text_shade(VGA_SCREEN, 185, y, name, color / 16, color % 16 - 8 - shade, 2)
	if not done:
		_draw_shop_item(cat - 1, item, 160, y - 4)
	color = 15 if w == sel - 1 else 28
	if not done:
		vga.text_shade(VGA_SCREEN, 187, y + 10, "%s %d" % [TyrianText.L("shop_cost"), cost], color / 16,
				color % 16 - 8 - shade, 2)


## Scroll arrows of the list (game_menu.c, triangles pixel by pixel).
func _draw_scroll_arrows(total: int) -> void:
	if _shop_scroll > 0:
		for row in 5:
			var span := 7 - row * 2
			if span < 1:
				break
			for dx in span:
				vga.set_pixel(VGA_SCREEN, 290 - span / 2 + dx, 30 + row, 250)
		for dx in [-1, 0, 1]:
			vga.set_pixel(VGA_SCREEN, 290 + dx, 35, 227)
	if _shop_scroll + UPGRADE_VISIBLE < total:
		for row in 5:
			var span := 7 - row * 2
			if span < 1:
				break
			for dx in span:
				vga.set_pixel(VGA_SCREEN, 290 - span / 2 + dx, 180 - row + 4, 250)
		for dx in [-1, 0, 1]:
			vga.set_pixel(VGA_SCREEN, 290 + dx, 179, 227)


# --- Data cubes (game_menu.c, MENU_DATA_CUBES / MENU_DATA_CUBE_SUB) --------------

## JE_itemScreen for the two cube menus: portrait on the left (face
## of the chosen cube, facepal palette), on the right the cube list (shared
## engine, ROWS style) or the text of the open cube.
func _render_cubes(menu: int) -> void:
	var headers: PackedStringArray = _state.get("cube_headers", PackedStringArray())
	var faces: PackedInt32Array = _state.get("cube_faces", PackedInt32Array())
	var cube_max := int(_state.get("cube_max", 0))
	var cube := int(_state.get("cube_sel", 2)) - 2
	var rows := []
	if menu == MENU_DATA_CUBES:
		for i in cube_max + 1:
			rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"index": i, "draw": _draw_cube_row}))
	var header := (
		TyrianText.L(MENU_INT[0][1])
		if menu == MENU_DATA_CUBES
		else (headers[cube] if cube < headers.size() else "")
	)
	menus.stack.clear()
	menus.open(header, rows, {
		"style": MenuEngine.Style.ROWS, "external": true, "draw": false,
		"sel": clampi(cube, 0, maxi(rows.size() - 1, 0)),
		# One row per cube (y = 38 + 28·i); "Exit" below the last one, or at
		# y = 160 if there are none.
		"row_x": 166, "row_y": 38, "row_step": 28,
		# Portrait before the cubes, like the C code: its right edge covers x = 166-169.
		"background": _cubes_background,
		"header_draw": func() -> void: menus.draw_game_menu_header(header),
		"decorate": _decorate_cubes.bind(menu, cube, cube_max, faces, headers),
	})
	menus.render()
	if menu == MENU_DATA_CUBE_SUB:
		_draw_cube_text(cube)
	elif cube < cube_max:
		# First turn of the inner loop: blinking cube.
		_flash_cube_step(cube)
	_flash_clock = 0.0
	present()
	idle = true


## Cube background: shop screen, portrait (OPTION_SHAPES 20).
## Then, after the rows: face and its palette, help, cube header
## under the face; empty list: "No Data is available at this time.".
func _cubes_background() -> void:
	vga.set_indices(VGA_SCREEN, _background)
	vga.fill_rectangle_xy(VGA_SCREEN, 1, 1, 145, 170, 0)
	vga.blit_sprite(VGA_SCREEN, 1, 1, OPTION_SHAPES, 20)


func _decorate_cubes(menu: int, cube: int, cube_max: int, faces: PackedInt32Array, headers: PackedStringArray) -> void:
	if menu == MENU_DATA_CUBES and cube_max == 0:
		TyrianText.help_box(self, 166, 80, TyrianText.L("miscText_15"), 30)
	var reading := cube < cube_max
	if reading and cube < faces.size() and faces[cube] >= 0:
		var face := faces[cube]
		var size: Vector2i = TyrianVga.sprite_size(FACE_SHAPES, face)
		vga.blit_sprite(VGA_SCREEN, 77 - size.x / 2, 92 - size.y / 2, FACE_SHAPES, face)
		# colors[1..255-48] = palettes[facepal[face]] (the end of the palette
		# stays the shop's: interface, text).
		vga.load_palette(0)
		vga.load_palette_range(FACEPAL[face] if face < FACEPAL.size() else 0, 1, 255 - 3 * 16)
		vga.set_palette(0, 255)
		_face_palette = true
	# JE_drawMainMenuHelpText: Exit or no cube -> mainMenuHelp[12-1],
	# otherwise [17 + curMenu - 3] (21: choose a cube, 22: Esc).
	var help := 17 + menu - 3
	if menu == MENU_DATA_CUBES and not reading:
		help = 11
	vga.text_shade(VGA_SCREEN, 10, 187, TyrianText.L("mmhelp_%d" % help), 14, 1, 2)
	if reading and cube < headers.size():
		vga.text_shade(VGA_SCREEN, 75 - TyrianText.width(headers[cube], SMALL_FONT) / 2, 173, headers[cube], 14, 3, 2)


## A row of MENU_DATA_CUBES: cube (JE_drawCube, filter 13), title in
## 24-column JE_helpBox (252 chosen, 250 otherwise, DARKEN); "Exit" at the end.
func _draw_cube_row(it: Dictionary, x: int, y: int, selected: bool) -> void:
	var titles: PackedStringArray = _state.get("cube_titles", PackedStringArray())
	var c := 252 if selected else 250
	var i: int = it.index
	if i < titles.size():
		vga.blit_sprite_dark(VGA_SCREEN, x + 4, y + 4, OPTION_SHAPES, 25, false)
		vga.blit_sprite_dark(VGA_SCREEN, x + 3, y + 3, OPTION_SHAPES, 25, false)
		vga.blit_sprite_hv(VGA_SCREEN, x, y, OPTION_SHAPES, 25, 13, 0)
		TyrianText.help_box(self, 192, y + 6, titles[i], 24, c / 16, c % 16 - 8, TyrianText.DARKEN)
		return
	var ey := 160 if titles.is_empty() else y + 6
	vga.text_shade(VGA_SCREEN, 172, ey, TyrianText.L("miscText_5"), c / 16, c % 16 - 8, 2)


## "Read N%" of the open cube (game_menu.c:1306-1314), clamped to 100.
func cube_read_percent(lines: int) -> int:
	return 100 if lines <= 9 else mini(100, _cube_y * 100 / ((lines - 9) * CUBE_LINE_H))


## Reading a cube (inner loop of JE_itemScreen): ten lines starting
## at yLoc, masking bands, percentage read and "EXIT".
func _draw_cube_text(cube: int) -> void:
	var lines: PackedStringArray = _shop.cube_text(cube)
	vga.fill_rectangle_xy(VGA_SCREEN, 160, 49, 310, 158, 228)
	var first := _cube_y / CUBE_LINE_H
	var y := 38 + CUBE_LINE_H - _cube_y % CUBE_LINE_H
	for i in range(first, mini(first + CUBE_LINES, lines.size())):
		vga.out_text_and_darken(VGA_SCREEN, 161, y, lines[i], 14, 3, SMALL_FONT)
		y += CUBE_LINE_H
	vga.fill_rectangle_xy(VGA_SCREEN, 160, 39, 310, 48, 228)
	vga.fill_rectangle_xy(VGA_SCREEN, 160, 157, 310, 166, 228)
	var percent := cube_read_percent(lines.size())
	vga.out_text_and_darken(VGA_SCREEN, 176, 160, "%s %d%%" % [TyrianText.L("miscText_11"), percent], 14, 1, SMALL_FONT)
	var exit := TyrianText.L("miscText_12")
	vga.dstring(VGA_SCREEN, maxi(312 - TyrianText.width(exit, NORMAL_FONT), 200), 160, exit, NORMAL_FONT)


## Reading a cube: inner loop of JE_itemScreen, one turn per
## setDelay(1). Up / Down held (keysactive, d-pad or stick) move yLoc
## by one pixel per turn, Page Up / Down by two; stops at the top of the text
## and when the last page is reached (yLoc / 12 > last_line - 10).
func _process(delta: float) -> void:
	if _shop != null and _anim_delay > 0 and _anim_left != 0 and not _specs_open and not modal_active:
		_anim_tick(delta)
	if _hover_pending >= 0 and _shop != null and _help == null and not _specs_open and not modal_active:
		hover_tick(hover_clock.call())
	if _shop == null or _specs_open or modal_active or not idle:
		_cube_clock = 0.0
		return
	var menu := int(_state.get("menu", 0))
	if menu == MENU_DATA_CUBES:
		_flash_cube_tick(delta)
		return
	if menu != MENU_DATA_CUBE_SUB:
		_cube_clock = 0.0
		_cube_wheel_lines = 0
		_cube_chg = 0
		return
	var dir := 0
	if Input.is_action_pressed(TyrianControls.UP):
		dir = -1
	if Input.is_action_pressed(TyrianControls.DOWN):
		dir = 1
	if Input.is_action_pressed(&"tyrian_menu_page_up"):
		dir = -2
	if Input.is_action_pressed(&"tyrian_menu_page_down"):
		dir = 2
	if dir == 0:
		dir = _cube_mouse_dir()
	if dir != 0:
		_cube_wheel_lines = 0
	var wheel := dir == 0 and _cube_wheel_lines != 0
	if wheel:
		dir = signi(_cube_wheel_lines)
		_cube_chg = 0
	# game_menu.c:1328: yChg stays set after release until
	# yLoc % 12 == 0; a brief press therefore scrolls a whole line.
	var held := dir != 0 and not wheel
	if dir == 0 and _cube_chg != 0:
		dir = _cube_chg
	if dir == 0:
		_cube_clock = 0.0
		return
	_cube_clock += delta
	var moved := false
	while _cube_clock >= DELAY_PERIOD:
		_cube_clock -= DELAY_PERIOD
		if not wheel:
			var step := dir
			if not held:
				# End of line: the step of 2 (Page) does not skip the 12 px boundary.
				var left := (CUBE_LINE_H - _cube_y % CUBE_LINE_H) if dir > 0 else _cube_y % CUBE_LINE_H
				step = signi(dir) * mini(absi(dir), left)
			if not _scroll_cube(step):
				_cube_chg = 0
				break
			moved = true
			_cube_chg = 0 if _cube_y % CUBE_LINE_H == 0 else dir
			if not held and _cube_chg == 0:
				break
			continue
		if wheel:
			# One notch = one 12 px line, one pixel per turn, like Up / Down
			# (game_menu.c:1328, yChg reset to 0 when yLoc % 12 == 0).
			if not _scroll_cube(dir):
				_cube_wheel_lines = 0
				break
			moved = true
			if _cube_y % CUBE_LINE_H == 0:
				_cube_wheel_lines -= dir
				if _cube_wheel_lines == 0:
					break
	if moved:
		_render()


## Cube list: one inner-loop turn per setDelay(2), the chosen cube
## is re-blended on the current image (the blend accumulates as
## in the C code, which does not restore the background between two turns).
func _flash_cube_tick(delta: float) -> void:
	var cube := int(_state.get("cube_sel", 2)) - 2
	var cube_max := PackedStringArray(_state.get("cube_titles", PackedStringArray())).size()
	if cube >= cube_max:
		return
	# One turn = one presented image (JE_showVGA): never two steps per image,
	# even if the image fell behind (the intermediate hues remain).
	_flash_clock = minf(_flash_clock + delta, 2 * DELAY_PERIOD)
	if _flash_clock >= 2 * DELAY_PERIOD:
		_flash_clock = 0.0
		_flash_cube_step(cube)
		present()


## col += colC, direction reversed outside [-2, 6], then blit_sprite_hv_blend of the
## cube (OPTION_SHAPES 25, filter 13) at (166, 38 + 28·rank).
func _flash_cube_step(cube: int) -> void:
	_flash_col += _flash_col_c
	if _flash_col < -2 or _flash_col > 6:
		_flash_col_c = -_flash_col_c
	vga.blit_sprite_hv_blend(VGA_SCREEN, 166, 38 + cube * 28, OPTION_SHAPES, 25, 13, _flash_col)


## One turn of the inner loop: yLoc += yChg, clamped like the C code. True if
## the text moved.
func _scroll_cube(dir: int) -> bool:
	var lines: int = _shop.cube_text(int(_state.get("cube_sel", 2)) - 2).size()
	if dir < 0 and _cube_y == 0 or dir > 0 and _cube_y / CUBE_LINE_H > lines - 10:
		return false
	_cube_y = maxi(0, _cube_y + dir)
	return true


# --- Ship Specs (game_menu_nav.c:JE_doShipSpecs) -----------------------------

## Image of the specifications in GAME_SCREEN (JE_drawShipSpecs), enlarged
## from the center (JE_scaleInPicture), then wait for a key.
func _ship_specs() -> void:
	_specs_open = true
	_specs_key = false
	idle = false
	var lo: PackedInt32Array = _state["loadout"]
	_draw_ship_specs(lo[0])
	# JE_loadPic(VGAScreen2, 1): VGAScreen2 was used as a buffer.
	vga.set_indices(VGA_SCREEN2, _background)
	Sfx.play(Sfx.SPRING)
	for i in range(2, 161, 2):
		if _specs_key:
			break
		vga.scale_bitmap(VGA_SCREEN, GAME_SCREEN, 160 - i, 0, 160 + i - 1, 100 + roundi(i * 0.625) - 1)
		present()
		await get_tree().process_frame
	_specs_key = false
	idle = true
	while not _specs_key and is_inside_tree():
		await get_tree().process_frame
	_specs_open = false
	_specs_key = false
	_render()


## game_menu_nav.c:JE_drawShipSpecs: grid, name, two help texts
## (verticalHeight 9), "Press a key", then the ship in green relief.
func _draw_ship_specs(ship_id: int) -> void:
	vga.clear(GAME_SCREEN, 0)
	_draw_spec_lines(true)
	_draw_spec_lines(false)
	vga.rectangle(GAME_SCREEN, 0, 0, 319, 199, 37)
	vga.rectangle(GAME_SCREEN, 1, 1, 318, 198, 35)
	var ship := _item(2, ship_id)
	vga.out_text(GAME_SCREEN, 10, 2, String(ship.get("name", "")), 12, 3)
	if ship_id >= 1 and ship_id <= 20:
		TyrianText.vertical_height = 9
		TyrianText.help_box(self, 100, 20, TyrianText.L("shipInfo_%d_0" % (ship_id - 1)), 40, 12, 1,
				TyrianText.FULL_SHADE, GAME_SCREEN)
		TyrianText.help_box(self, 100, 100, TyrianText.L("shipInfo_%d_1" % (ship_id - 1)), 40, 12, 1,
				TyrianText.FULL_SHADE, GAME_SCREEN)
		TyrianText.vertical_height = 7
	var press := TyrianText.L("miscText_4")
	vga.out_text(GAME_SCREEN, TyrianText.font_center(press, SMALL_FONT), 190, press, 12, 2)
	vga.clear(VGA_SCREEN2, 0)
	if ship_id >= 19 and ship_id <= 32:
		# Mod ship: its in-game sprite (no OPTION_SHAPES illustration).
		_draw_shop_item(1, ship_id, 36, 48, VGA_SCREEN2)
	else:
		var big := int(_base_item("ships", ship_id).get("bigshipgraphic", 32))
		var xy: Array = {28: [31, 36], 33: [31, 35], 45: [36, 33], 46: [30, 30]}.get(big, [35, 33])
		vga.blit_sprite(VGA_SCREEN2, xy[0] - 30, xy[1], OPTION_SHAPES, big - 1)
	vga.greenify(GAME_SCREEN, VGA_SCREEN2)


## game_menu_nav.c:JE_drawLines: 15 px grid (columns 25..130),
## dark lines offset by one pixel then light ones, JE_pix3 dots.
func _draw_spec_lines(dark: bool) -> void:
	for k in range(1, 21):
		var x := 15 * k + 10
		if x > 18 and x < 135:
			if dark:
				vga.rectangle(GAME_SCREEN, x + 1, 0, x + 1, 199, 32 + 3)
			else:
				vga.rectangle(GAME_SCREEN, x, 0, x, 199, 32 + 5)
	for k in range(1, 21):
		var y := 15 * k
		if y > 15 and y < 169:
			if dark:
				vga.rectangle(GAME_SCREEN, 0, y + 1, 319, y + 1, 32 + 3)
			else:
				vga.rectangle(GAME_SCREEN, 0, y, 319, y, 32 + 5)
			for j in range(1, 21):
				var x := 15 * j + 10
				if x > 18 and x < 135:
					for d in [Vector2i(0, 0), Vector2i(-1, 0), Vector2i(1, 0), Vector2i(0, -1), Vector2i(0, 1)]:
						vga.set_pixel(GAME_SCREEN, x + d.x, y + d.y, 32 + 6)


# --- Title mode menus (game_menu.c, MENU_2_PLAYER_ARCADE,
# MENU_1_PLAYER_ARCADE, MENU_SUPER_TYRIAN) ------------------------------------

## JE_itemScreen with a mode's curMenu: tyrian.pic no. 1 background, short menu
## (Start Level, entries / options, Ship Specs, Quit Game), no purchases.
func _open_arcade() -> void:
	var lo := Game.loadout
	_state = {"menu": _arcade_menu, "sel": 2, "loadout": lo, "cash": lo[10] if lo.size() > 10 else 0}
	vga.load_pic(VGA_SCREEN, 1, true)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	_background = vga.indices(VGA_SCREEN)
	_render()


## One frame of the menu: header and choices (JE_drawMenuHeader /
## JE_drawMenuChoices, MENU_2_PLAYER_ARCADE: offsets after rows 3 and
## 4), money and bars (one player), left column, devices of the
## two players, help.
func _render_arcade() -> void:
	var keys: Array = ARCADE_MENU_INT[_arcade_menu]
	var items := []
	for x in range(2, keys.size() + 1):
		var gap := 0
		if _arcade_menu == 9:
			gap = (16 if x > 3 else 0) + (16 if x > 4 else 0)
		items.append(MenuEngine.item(TyrianText.L(keys[x - 1]), "", MenuEngine.Kind.ACTION, {"gap": gap}))
	menus.stack.clear()
	menus.open(TyrianText.L(keys[0]), items, {
		"style": MenuEngine.Style.GAME_MENU, "external": true, "sel": _arcade_sel - 2, "draw": false,
		"background": func() -> void: vga.set_indices(VGA_SCREEN, _background),
		"decorate": _decorate_arcade,
	})
	menus.draw()


func _decorate_arcade() -> void:
	var lo: PackedInt32Array = _state["loadout"]
	var two_players := _arcade_menu == 9
	if not two_players:
		# "(curMenu >= MENU_1_PLAYER_ARCADE) && !twoPlayerMode": money, armor, shield.
		var ship := _base_item("ships", lo[0])
		var shield := _base_item("shields", lo[5])
		vga.text_shade(VGA_SCREEN, 65, 173, str(int(_state["cash"])), 1, 6, 2)
		bar_draw_shadow(42, 152, 3, 14, int(ship.get("dmg", 0)), 2, 13)
		bar_draw_shadow(104, 152, 4, 14, int(shield.get("mpwr", 0)) * 2, 2, 13)
	draw_left_side(self, lo, int(_state["cash"]))
	if two_players:
		# game_menu.c:1193-1209: devices of the two players (inputDevice:
		# 1 keyboard, 2 mouse), read from campaign_modes[8] / [9].
		var devices := arcade_input_devices()
		for i in 2:
			vga.dstring(VGA_SCREEN, 186, 38 + 2 * (i + 1) * 16, TyrianText.L("inputDevice_%d" % (devices[i] - 1)),
					NORMAL_FONT)
	# JE_drawMainMenuHelpText: curMenu >= MENU_2_PLAYER_ARCADE.
	var help: int = ARCADE_MENU_HELP[_arcade_menu][_arcade_sel - 2]
	vga.text_shade(VGA_SCREEN, 10, 187, TyrianText.L("mmhelp_%d" % (help - 1)), 14, 1, 2)


## game_menu.c:1065-1100, "Draw crap on the left side of the screen": money
## of the two players (two-player game), Super Arcade / Super Tyrian panel
## (ship, weapon, special weapon), otherwise the ship illustration. Same
## drawing in the mode menus and in Options / Load / Save (save_menu.gd).
static func draw_left_side(screen: TyrianScreen, lo: PackedInt32Array, cash: int) -> void:
	var vga = screen.vga
	var m := Game.campaign_modes
	var row := func(table: String, id: int) -> Dictionary:
		var rows: Array = screen._items_json.get(table, [])
		return rows[id] if id >= 0 and id < rows.size() else {}
	if m.size() >= 10 and m[0] != 0:
		# miscText[40 + i] and money of the two players.
		var cash2 := 0
		if Game.campaign_modes_extra.size() >= 2:
			cash2 = int(Game.campaign_modes_extra[1])
		var cashes := [cash, cash2]
		for i in 2:
			vga.text_shade(VGA_SCREEN, 25, 50 + 10 * i, "%s %d" % [TyrianText.L("miscText_%d" % (40 + i)), cashes[i]],
					15, 0, TyrianText.FULL_SHADE)
	elif m.size() >= 10 and (m[3] != 0 or m[4] != 0):
		# Super Arcade / Super Tyrian: ship name, weapon, special weapon.
		var name_key := "superShip_12" if m[4] != 0 else "superShip_%d" % int(m[3])
		TyrianText.help_box(screen, 35, 25, TyrianText.L(name_key), 18, 15, 4)
		vga.text_shade(VGA_SCREEN, 25, 50, TyrianText.L("superShip_10"), 15, 0, TyrianText.FULL_SHADE)
		# helptext.c:220: JE_helpBox resets helpBoxColor to 12 on exit; the two
		# following weapon names are therefore in color 12 (green), brightness 1.
		TyrianText.help_box(screen, 25, 60, str(row.call("weapon_ports", lo[1]).get("name", "")), 22, 12, 1)
		vga.text_shade(VGA_SCREEN, 25, 120, TyrianText.L("superShip_11"), 15, 0, TyrianText.FULL_SHADE)
		TyrianText.help_box(screen, 25, 130, str(row.call("specials", lo[9]).get("name", "")), 22, 12, 1)
	else:
		screen._draw_ship_illustration(lo, row.call("ships", lo[0]), row.call("shields", lo[5]))


## JE_itemScreen keys on these menus, then JE_menuFunction.
func _arcade_action(action: StringName) -> void:
	var choices: int = ARCADE_MENU_INT[_arcade_menu].size()
	match action:
		TyrianControls.UP:
			Sfx.play(Sfx.CURSOR)
			_arcade_sel = choices if _arcade_sel <= 2 else _arcade_sel - 1
		TyrianControls.DOWN:
			Sfx.play(Sfx.CURSOR)
			_arcade_sel = 2 if _arcade_sel >= choices else _arcade_sel + 1
		TyrianControls.ACCEPT, TyrianControls.SELECT:
			# game_menu.c:2813: JE_menuFunction plays S_CLICK on any confirmation.
			Sfx.play(Sfx.CLICK)
			_arcade_function()
			return
		TyrianControls.LEFT, TyrianControls.RIGHT:
			# game_menu.c:1927-1944 and 2025-2036: Left / Right on "Player 1/2
			# Input" cycle the device (S_CURSOR); without a gamepad, both
			# directions swap keyboard and mouse.
			if _arcade_menu != 9 or not _arcade_sel in [3, 4]:
				return
			Sfx.play(Sfx.CURSOR)
			swap_arcade_input_devices()
		TyrianControls.BACK:
			# menuEsc[curMenu] == 0: JE_quitRequest.
			Sfx.play(Sfx.SPRING)
			_quit_request()
			return
		_:
			return
	_state["sel"] = _arcade_sel
	_render()


## game_menu.c:JE_menuFunction, cases MENU_2_PLAYER_ARCADE (2 Start Level,
## 3/4 player input, 5 Options, 6 Quit), MENU_1_PLAYER_ARCADE (2 Start
## Level, 3 Options, 4 Quit) and MENU_SUPER_TYRIAN (2 Start Level, 3 Ship
## Specs, 4 Options, 5 Quit). "Start Level": mainLevel = mapSection[mapPNum-1].
## Options: save screen, like the shop (save_menu.gd).
func _arcade_function() -> void:
	var options_row: int = {9: 5, 10: 3, 13: 4}[_arcade_menu]
	var quit_row: int = {9: 6, 10: 4, 13: 5}[_arcade_menu]
	if _arcade_sel == 2:
		Game.campaign_leave_shop(maxi(Game.map_sections.size() - 1, 0), _state["loadout"])
	elif _arcade_sel == quit_row:
		_quit_request()
	elif _arcade_sel == options_row:
		Game.shop_resume_sel = options_row
		Game.goto_scene(Game.SCENE_SAVE_MENU)
	elif _arcade_menu == 13 and _arcade_sel == 3:
		_ship_specs()
	else:
		# game_menu.c:3096-3108: Enter on "Player 1/2 Input" switches to the
		# next device and gives the other to the second player (without a gamepad:
		# keyboard and mouse swap). Godot has no SDL gamepad number
		# (divergence 45): only keyboard (1) and mouse (2) cycle.
		Sfx.play(Sfx.CURSOR)
		swap_arcade_input_devices()
		_render()


## Devices [player 1, player 2] of the two-player game (config.c:187: 1, 2 by default).
static func arcade_input_devices() -> PackedInt32Array:
	var m := Game.campaign_modes
	if m.size() >= 10 and int(m[8]) in [1, 2] and int(m[9]) in [1, 2] and m[8] != m[9]:
		return PackedInt32Array([int(m[8]), int(m[9])])
	return PackedInt32Array([1, 2])


## Keyboard / mouse swap between the two players (game_menu.c:3103-3108, joysticks == 0),
## for the game (campaign_modes) and the next level (modes).
static func swap_arcade_input_devices() -> void:
	var d := arcade_input_devices()
	if Game.campaign_modes.size() >= 10:
		var c := Game.campaign_modes
		c[8] = d[1]
		c[9] = d[0]
		Game.campaign_modes = c
	if Game.modes.size() >= 10:
		var m := Game.modes
		m[8] = d[1]
		m[9] = d[0]
		Game.modes = m
