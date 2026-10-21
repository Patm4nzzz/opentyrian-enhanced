## Saving and loading the campaign from the shop (game_menu.c,
## MENU_OPTIONS then MENU_LOAD_SAVE; mainint.c:JE_operation), in the rewrite's
## native format (ADR 0129, Game.save_campaign / campaign_slots).
##
## Opened by the shop's "Options" entry (shop.gd) with the current equipment
## in Game.loadout. Menus drawn by the shared engine (ADR 0121):
## "Options" (Load Game, Save Game, Done; GAME_MENU style) then the list of the
## 11 slots (ROWS style, geometry of JE_drawMenuChoices for
## MENU_LOAD_SAVE). The rest of the C Options menu (volumes, keyboard, gamepad,
## mouse) is in Setup. Saving: name entry (JE_operation) then
## confirmation (JE_saveRequest). Slot 11 is the "LAST LEVEL"
## autosave: it can be loaded but not overwritten (C:
## `slot % 11 != 0`).
extends TyrianScreen

## mainint.c:JE_operation: 14 characters at most.
const NAME_MAX := 14
## Buttons of the name box (mainint.c:JE_operation, miscText[9] / [10]):
## helptext.c:567-568 replaces them with misc_yes / misc_no ("YES" / "NO"),
## not with the raw miscText_9 / _10 ("OK" / "CANCEL").
const SAVE_BOX_YES := "misc_yes"
const SAVE_BOX_NO := "misc_no"
## game_menu.c:JE_drawMenuChoices, MENU_LOAD_SAVE: x = 163, y = 38 + 11·i;
## level at x = 245, episode at x = 297.
const X_NAME := 163
const X_LEVEL := 245
const X_EPISODE := 293
const Y_ROWS := 38
const DY_ROWS := 11
## helptext.c:menuHelp, MENU_OPTIONS (Load, Save, Done) and MENU_LOAD_SAVE (1-based).
const HELP_LOAD := 13
const HELP_SAVE := 14
const HELP_DONE := 12
const HELP_SLOTS := 21
## game_menu.c:MENU_OPTIONS cases 6/7/8 + helptext.c:467-529. Unlike the
## constants above (1-based), these three are already the mmhelp_N key number.
const HELP_JOYSTICK := 15
const HELP_KEYBOARD := 16
const HELP_MOUSE := 34
## helptext.c:menuHelp[MENU_MOUSE_CONFIG] = {35, 35, 35, 36, 12} (mmhelp_N keys: N - 1).
const HELP_MOUSE_BUTTON := 34
const HELP_MOUSE_RESET := 35
const HELP_VOLUME := 14
const SETUP := preload("res://scripts/setup.gd")
const SHOP := preload("res://scripts/shop.gd")
## game_menu.c:JE_itemScreen, MENU_OPTIONS: volume bars at x 225, rows
## Music (y 70) and Sound (y 86), one notch per 12 units; clickable area x >= 221
## (225 - 4), 13 px high. Rank (0-based) of the rows in the menu: 2 and 3.
const VOLUME_BAR_X := 225
const VOLUME_CLICK_X := 221
const VOLUME_BAR_Y := [70, 86]
const VOLUME_KEYS := ["music_volume", "fx_volume"]
const VOLUME_DISABLED_KEYS := ["music_disabled", "samples_disabled"]
const ROW_VOLUME_FIRST := 2
## game_menu.c:mouseSelectionY[MENU_MOUSE_CONFIG] = 24.
const MOUSE_ROW_STEP := 24
## game_menu.c:JE_itemScreen, MENU_MOUSE_CONFIG: value of button i at (215, 51 + 24 i).
const MOUSE_VALUE_X := 215
const MOUSE_VALUE_Y := 51

## game_menu.c:MENU_KEYBOARD_CONFIG: ten rows (eight keys, Reset, Done), 12 px step.
const KEY_ROWS := 10
const KEY_ROW_RESET := 8
const KEY_ROW_DONE := 9
const KEYBOARD_ROW_STEP := 12
## game_menu.c:MENU_JOYSTICK_CONFIG: sixteen rows (4 settings, 10 assignments, Reset, Done), 8 px step.
const JOY_ROWS := 16
const JOY_ROW_RESET := 14
const JOY_ROW_DONE := 15
const JOY_ROW_STEP := 8
## Columns of the two settings screens: labels x 166, values x 236 (game_menu.c).
const X_CONFIG_LABEL := 166
const KEY_VALUE_X := 236
## keyName[] of the gamepad assignment rows: up, right, down, left, fire,
## fire switch, left ally, right ally (game_menu.c:menu_item).
const JOY_KEY_NAMES := [1, 4, 2, 3, 5, 6, 7, 8]

## Options row to find again on returning from the settings screen.
static var entry_row := 0
var idle := false
var menus := MenuEngine.new(self)
var _background := PackedByteArray()
## Slots read (Game.campaign_slots) when the list is opened.
var _slots: Array = []
var _saving := false
## Help line message after an action (locale key), empty otherwise.
var _message := ""
## Name entry in progress (handle_event captures it); keys received, in
## order: character (String) or action (StringName).
var _typing := false
## Key or button capture in progress (_wait_capture): `_capture_wants_pad`
## true for a gamepad assignment; `_capture_event`: the event retained.
var _capturing := false
var _capture_wants_pad := false
var _capture_event: Variant = null


## Key capture (game_menu.c:2960-2981): prompt and blinking frame
## presented without JE_mouseStart, no pointer. Gamepad capture (3205): nothing
## is presented, the menu image, pointer included, stays.
func pointer_shown() -> bool:
	return super() and not (_capturing and not _capture_wants_pad)
## game_menu.c:joystick_config: gamepad being configured (0 = player 1).
var _joy_config := 0
var _pending: Array = []
## Offset in _slots: 0 = 1 player (slots 1..11),
## 11 = 2 players (slots 12..22, config.c:(slot-1) > 10).
var _page_offset := 0


func _ready() -> void:
	var items_key := "episodes_1_3" if Game.episode <= 3 else "episode_%d" % Game.episode
	_items_json = TyrianAssets.read_json("items.json")[items_key]
	vga.load_pic(VGA_SCREEN, 1, true)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	_background = vga.indices(VGA_SCREEN)
	_open_options()


## game_menu.c:curSel[]: row of screen `screen` found again during the visit.
func _remembered_row(screen: String) -> int:
	return int(Game.shop_menu_rows.get(screen, 0))


func _remember_row(screen: String) -> void:
	Game.shop_menu_rows[screen] = menus.selection()


func _help(key: String) -> void:
	vga.text_shade(VGA_SCREEN, 10, 187, TyrianText.L(key), 14, 1, 2)
	_draw_status()


## game_menu.c:JE_itemScreen, MENU_OPTIONS and MENU_LOAD_SAVE: left column
## like the mode menus (money of both players, Super
## Arcade / Super Tyrian panel or ship illustration), then money, armor
## and shield gauges (one player).
func _draw_status() -> void:
	var lo := Game.loadout
	if lo.size() < 11:
		return
	SHOP.draw_left_side(self, lo, lo[10])
	if Game.modes.size() >= 1 and Game.modes[0] != 0:
		return
	var ships: Array = _items_json.get("ships", [])
	var shields: Array = _items_json.get("shields", [])
	var ship: Dictionary = ships[lo[0]] if lo[0] >= 0 and lo[0] < ships.size() else {}
	var shield: Dictionary = shields[lo[5]] if lo[5] >= 0 and lo[5] < shields.size() else {}
	vga.text_shade(VGA_SCREEN, 65, 173, str(lo[10]), 1, 6, 2)
	bar_draw_shadow(42, 152, 3, 14, int(ship.get("dmg", 0)), 2, 13)
	bar_draw_shadow(104, 152, 4, 14, int(shield.get("mpwr", 0)) * 2, 2, 13)


## game_menu.c:MENU_KEYBOARD_CONFIG (menu 5): eight keys of player 1, then
## "Reset to Defaults" and "Done" (12 px step, labels x 166, keys x 236).
func _open_keyboard_config() -> void:
	var rows := []
	for i in KEY_ROWS:
		rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"index": i, "draw": _draw_key_row}))
	menus.open(TyrianText.L("keyName_0"), rows,
			_config_menu_options("keyboard", KEYBOARD_ROW_STEP, _key_help, _key_activate))


## game_menu.c:MENU_JOYSTICK_CONFIG (menu 12): gamepad, analog, sensitivity,
## threshold, ten assignments, Reset, Done (8 px step, labels x 166, values x 236).
func _open_joystick_config() -> void:
	var rows := []
	for i in JOY_ROWS:
		rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"index": i, "draw": _draw_joy_row}))
	var options := _config_menu_options("joystick", JOY_ROW_STEP, _joy_help, _joy_activate)
	options["shortcuts"] = {
		TyrianControls.UP: _joy_move.bind(-1), TyrianControls.DOWN: _joy_move.bind(1),
		TyrianControls.LEFT: _joy_adjust.bind(-1), TyrianControls.RIGHT: _joy_adjust.bind(1),
	}
	menus.open(TyrianText.L("menuInt_13_0"), rows, options)


## Options common to the two settings screens (JE_itemScreen: column of
## choices x 170..308, y > 20, rank = (y - 38) / step).
func _config_menu_options(screen: String, step: int, help: Callable, activate: Callable) -> Dictionary:
	return {
		"sel": _remembered_row(screen),
		"style": MenuEngine.Style.ROWS, "row_x": X_CONFIG_LABEL, "row_y": Y_ROWS, "row_step": step,
		"hover_sfx": Sfx.CLICK, "right_click": "click", "click_selects_first": true,
		"hit_x": 171, "hit_w": 137, "hit_row_y": Y_ROWS, "hit_y0": 21, "row_h": step,
		"accept": [TyrianControls.ACCEPT, TyrianControls.SELECT], "move_sfx": Sfx.CURSOR,
		"background": func() -> void: vga.set_indices(VGA_SCREEN, _background),
		"header_draw": func() -> void: menus.draw_game_menu_header(menus.current().header),
		"decorate": func() -> void:
			_remember_row(screen)
			_help(help.call(menus.selection())),
		"on_activate": activate,
	}


## Player 1's key for command `row` (input_key_name_localized).
func _key_text(row: int) -> String:
	for ev in InputMap.action_get_events(TyrianControls.action_name(1, TyrianControls.COMMANDS[row][0])):
		if ev is InputEventKey:
			return TyrianControls.key_name(ev.physical_keycode)
	return ""


## game_menu.c:JE_itemScreen, MENU_KEYBOARD_CONFIG: label keyName[x - 1]
## (15 / 28), key at x 236 (252 / 250); Reset and Done without a value.
func _draw_key_row(it: Dictionary, x: int, y: int, sel: bool) -> void:
	var i: int = it.index
	var c_name := 15 if sel else 28
	TyrianText.text_shade(self, x, y, TyrianText.L("keyName_%d" % (i + 1)), c_name / 16, c_name % 16 - 8,
			TyrianText.DARKEN)
	if i < TyrianControls.COMMANDS.size():
		var c_key := 252 if sel else 250
		TyrianText.text_shade(self, KEY_VALUE_X, y, _key_text(i), c_key / 16, c_key % 16 - 8, TyrianText.DARKEN)


## game_menu_nav.c:JE_drawMainMenuHelpText for MENU_KEYBOARD_CONFIG.
func _key_help(row: int) -> String:
	if row == KEY_ROW_RESET:
		return "mmhelp_24"
	if row == KEY_ROW_DONE:
		return "mmhelp_11"
	return "mmhelp_19"


func _key_activate(row: int) -> void:
	Sfx.play(Sfx.CLICK)
	if row == KEY_ROW_RESET:
		for c in TyrianControls.COMMANDS:
			_set_default_key(c[0])
		TyrianControls.save_config()
		menus.draw()
	elif row == KEY_ROW_DONE:
		menus.back()
	else:
		await _capture_key(row)


## game_menu.c:defaultKeySettings for one command: only player 1's key
## is restored (the gamepad and player 2 keep their settings).
func _set_default_key(command: String) -> void:
	var action := TyrianControls.action_name(1, command)
	for ev in InputMap.action_get_events(action):
		if ev is InputEventKey:
			InputMap.action_erase_event(action, ev)
	for code in TyrianControls.DEFAULT_KEYS[1].get(command, []):
		InputMap.action_add_event(action, TyrianControls.key_event(code))


## game_menu.c:JE_menuFunction, MENU_KEYBOARD_CONFIG, "change key": shaded
## key, blinking frame (colors 243..248) until a key, a click or
## a button. Same rules as Setup > Key Bindings: reserved key refused,
## key already taken by another command swapped, hotkey refused.
func _capture_key(row: int) -> void:
	var y := Y_ROWS + KEYBOARD_ROW_STEP * row
	TyrianText.text_shade(self, KEY_VALUE_X, y, _key_text(row), 15, 6, TyrianText.DARKEN)
	present()
	var event = await _wait_capture(false,
			func(col: int) -> void: vga.rectangle(VGA_SCREEN, 230, y - 2, 300, y + 7, col))
	if event is InputEventKey:
		var code: int = event.physical_keycode
		if not code in TyrianControls.RESERVED_KEYS and not TyrianControls.global_pressed(event,
				TyrianControls.CAPTURE_CANCEL):
			if _assign_game_key(row, code):
				Sfx.play(Sfx.CLICK)
				if row < TyrianControls.COMMANDS.size() - 1:
					menus.current().sel = row + 1
	menus.draw()


## Sets `code` on player 1's command `row`: swap with the command that
## had it, refused if a hotkey has it. True if the key was set.
func _assign_game_key(row: int, code: int) -> bool:
	var command: String = TyrianControls.COMMANDS[row][0]
	for other in TyrianControls.COMMANDS:
		if other[0] != command and _key_code(other[0]) == code:
			TyrianControls.rebind_key(1, other[0], _key_code(command))
			TyrianControls.rebind_key(1, command, code)
			TyrianControls.save_config()
			return true
	if TyrianControls.shortcut_key_in_use(code):
		return false
	TyrianControls.rebind_key(1, command, code)
	TyrianControls.save_config()
	return true


func _key_code(command: String) -> int:
	for ev in InputMap.action_get_events(TyrianControls.action_name(1, command)):
		if ev is InputEventKey:
			return ev.physical_keycode
	return 0


## Waits for a capture event (key, click, button or gamepad axis).
## `flash`: called every frame with the color of the blinking frame
## (invalid Callable(): fixed frame already drawn).
func _wait_capture(pad: bool, flash: Callable) -> Variant:
	_capturing = true
	_capture_wants_pad = pad
	_capture_event = null
	idle = false
	var col := 248
	var col_c := 1
	while _capture_event == null and is_inside_tree():
		if flash.is_valid():
			col += col_c
			if col < 243 or col > 248:
				col_c = -col_c
			flash.call(col)
		present()
		await wait_units(1)
	_capturing = false
	return _capture_event


## game_menu.c:JE_drawMenuChoices, MENU_JOYSTICK_CONFIG: the C labels (the
## first four, MENU and PAUSE are hardcoded in game_menu.c).
func _joy_label(i: int) -> String:
	# gdlint: ignore=max-line-length
	const FIXED := ["JOYSTICK", "ANALOG AXES", " SENSITIVITY", " THRESHOLD"]  # locale-exempt: hardcoded labels from game_menu.c
	if i < 4:
		return FIXED[i]
	if i == 12:
		return "MENU"  # locale-exempt: hardcoded label from game_menu.c
	if i == 13:
		return "PAUSE"  # locale-exempt: hardcoded label from game_menu.c
	if i == JOY_ROW_RESET:
		return TyrianText.L("keyName_9")
	if i == JOY_ROW_DONE:
		return TyrianText.L("keyName_10")
	# up, right, down, left (order of the assignments), then fire, fire switch, allies.
	return TyrianText.L("keyName_%d" % JOY_KEY_NAMES[i - 4])


func _draw_joy_row(it: Dictionary, x: int, y: int, sel: bool) -> void:
	var i: int = it.index
	var c_name := 15 if sel else 28
	TyrianText.text_shade(self, x, y, _joy_label(i), c_name / 16, c_name % 16 - 8, TyrianText.DARKEN)
	var c_val := 252 if sel else 250
	var value := ""
	if _joysticks() == 0 and i < 14:
		value = "-"
	elif i == 0:
		value = str(_joy_config + 1)
	elif i == 1:
		# gdlint: ignore=max-line-length
		value = "TRUE" if TyrianControls.analog_enabled[_joy_player()] else "FALSE"  # locale-exempt: hardcoded values from game_menu.c
	elif i < 4:
		if not TyrianControls.analog_enabled[_joy_player()]:
			c_val -= 3
		value = str(
			TyrianControls.analog_sensitivity[_joy_player()]
			if i == 2
			else TyrianControls.analog_threshold[_joy_player()]
		)
	elif i < 14:
		value = TyrianControls.pad_row_text(_joy_player(), i - 4)
	TyrianText.text_shade(self, KEY_VALUE_X, y, value, c_val / 16, c_val % 16 - 8, TyrianText.DARKEN)


## game_menu_nav.c:JE_drawMainMenuHelpText for MENU_JOYSTICK_CONFIG.
func _joy_help(row: int) -> String:
	if row == JOY_ROW_RESET:
		return "mmhelp_24"
	if row == JOY_ROW_DONE:
		return "mmhelp_11"
	return "mmhelp_15"


## Number of gamepads (the C's joysticks): those of players 1 and 2 plugged in.
func _joysticks() -> int:
	var n := 0
	for p in TyrianControls.PAD_DEVICE:
		if TyrianControls.PAD_DEVICE[p] in Input.get_connected_joypads():
			n += 1
	return n


func _joy_player() -> int:
	return _joy_config + 1


## game_menu.c:SDL_SCANCODE_UP / DOWN: the cursor skips sensitivity and threshold
## when the gamepad is not analog.
func _joy_move(dir: int) -> bool:
	var m := menus.current()
	var n: int = m.items.size()
	m.sel = (m.sel + dir + n) % n
	if _joysticks() > 0 and not TyrianControls.analog_enabled[_joy_player()] and m.sel in [2, 3]:
		m.sel = 4 if dir > 0 else 1
	Sfx.play(Sfx.CURSOR)
	return true


## game_menu.c:SDL_SCANCODE_LEFT / RIGHT on the first four rows
## (gamepad, analog, sensitivity, threshold 0..10 wrapping).
func _joy_adjust(dir: int) -> bool:
	# game_menu.c:1893-2046: Left / Right are silent on the gamepad screen
	# (S_CURSOR only in Options, Upgrade and Limited Options).
	if _joysticks() == 0:
		return true
	var p := _joy_player()
	match menus.selection():
		0:
			_joy_config = (_joy_config + dir + _joysticks()) % _joysticks()
		1:
			TyrianControls.analog_enabled[p] = not TyrianControls.analog_enabled[p]
		2:
			TyrianControls.analog_sensitivity[p] = (int(TyrianControls.analog_sensitivity[p]) + dir + 11) % 11
		3:
			TyrianControls.analog_threshold[p] = (int(TyrianControls.analog_threshold[p]) + dir + 11) % 11
		_:
			return true
	TyrianControls.save_config()
	return true


func _joy_activate(row: int) -> void:
	Sfx.play(Sfx.CLICK)
	if row == JOY_ROW_DONE:
		menus.back()
		return
	if _joysticks() == 0:
		return
	var p := _joy_player()
	match row:
		0:
			_joy_config = (_joy_config + 1) % _joysticks()
		1:
			TyrianControls.analog_enabled[p] = not TyrianControls.analog_enabled[p]
		2:
			if TyrianControls.analog_enabled[p]:
				TyrianControls.analog_sensitivity[p] = (int(TyrianControls.analog_sensitivity[p]) + 1) % 11
		3:
			if TyrianControls.analog_enabled[p]:
				TyrianControls.analog_threshold[p] = (int(TyrianControls.analog_threshold[p]) + 1) % 11
		JOY_ROW_RESET:
			TyrianControls.reset_pad(p)
		_:
			await _capture_pad(row)
			return
	TyrianControls.save_config()
	menus.draw()


## game_menu.c:JE_menuFunction, MENU_JOYSTICK_CONFIG, assignment: fixed frame
## (235, 21 + 8 · row) then detect_joystick_assignment; a key or a click
## cancels. The cursor advances one row except after the last assignment.
func _capture_pad(row: int) -> void:
	var y := 21 + 8 * (row + 2)
	vga.rectangle(VGA_SCREEN, 235, y, 310, y + 9, 248)
	present()
	var event = await _wait_capture(true, Callable())
	var detected = TyrianControls.pad_capture_event(event) if event != null else null
	if detected != null:
		var assign_row := row - 4
		TyrianControls.pad_assign_toggle(_joy_player(), assign_row, detected)
		TyrianControls.save_config()
		Sfx.play(Sfx.CLICK)
		if assign_row + 1 < TyrianControls.PAD_ROWS.size():
			menus.current().sel = row + 1
	menus.draw()


## game_menu.c:MENU_OPTIONS (cases 2=Load, 3=Save, 4=Music, 5=Sound, 6=Joystick,
## 7=Keyboard, 8=Mouse, 9=Done). Music and Sound are adjusted with the left / right
## arrows and by clicking their bar (handle_mouse).
func _open_options() -> void:
	var items := [
		MenuEngine.item(TyrianText.L("opt_load_game"), "mmhelp_%d" % (HELP_LOAD - 1), MenuEngine.Kind.ACTION,
				{"action": _open_slots.bind(false)}),
		MenuEngine.item(TyrianText.L("opt_save_game"), "mmhelp_%d" % (HELP_SAVE - 1), MenuEngine.Kind.ACTION,
				{"action": _open_slots.bind(true)}),
		MenuEngine.item(TyrianText.L("opt_music_vol"), "mmhelp_%d" % HELP_VOLUME, MenuEngine.Kind.ACTION,
				{"action": func() -> void: pass}),
		MenuEngine.item(TyrianText.L("opt_sfx_vol"), "mmhelp_%d" % HELP_VOLUME, MenuEngine.Kind.ACTION,
				{"action": func() -> void: pass}),
		MenuEngine.item(TyrianText.L("opt_joystick"), "mmhelp_%d" % HELP_JOYSTICK, MenuEngine.Kind.ACTION,
				{"action": _open_joystick_config}),
		MenuEngine.item(TyrianText.L("opt_keyboard"), "mmhelp_%d" % HELP_KEYBOARD, MenuEngine.Kind.ACTION,
				{"action": _open_keyboard_config}),
		MenuEngine.item(TyrianText.L("opt_mouse"), "mmhelp_%d" % HELP_MOUSE, MenuEngine.Kind.ACTION,
				{"action": _open_mouse_buttons}),
		MenuEngine.item(TyrianText.L("opt_done"), "mmhelp_%d" % (HELP_DONE - 1), MenuEngine.Kind.BACK),
	]
	menus.stack.clear()
	var first_row := entry_row if entry_row != 0 else _remembered_row("options")
	entry_row = 0
	menus.open(TyrianText.L("opt_options"), items, {
		"sel": first_row,
		"style": MenuEngine.Style.GAME_MENU, "accept": [TyrianControls.ACCEPT, TyrianControls.SELECT],
		"move_sfx": Sfx.CURSOR, "activate_sfx": Sfx.CLICK,
		# game_menu.c:mouseSelectionY[MENU_OPTIONS] = 16; hover and click like
		# the shop (any button acts like the left one, S_CLICK on hover).
		"hover_sfx": Sfx.CLICK, "right_click": "click", "click_selects_first": true,
		"shortcuts": {
			TyrianControls.LEFT: _volume_key.bind(-12),
			TyrianControls.RIGHT: _volume_key.bind(12),
		},
		"background": func() -> void: vga.set_indices(VGA_SCREEN, _background),
		"decorate": func() -> void:
			_remember_row("options")
			_draw_volume_bars()
			_help(menus.current().items[menus.selection()].help),
		"on_back": _leave,
	})


## game_menu.c:JE_itemScreen: Music / Sound volume bars ("Changing the
## volume?": JE_barDrawShadow, one notch per 12 units, grayed out if muted).
func _draw_volume_bars() -> void:
	for i in 2:
		var disabled: bool = SETUP.settings[VOLUME_DISABLED_KEYS[i]]
		bar_draw_shadow(VOLUME_BAR_X, VOLUME_BAR_Y[i], 1, 12 if disabled else 16,
				int(SETUP.settings[VOLUME_KEYS[i]]) / 12, 3, 13)


## Left / right arrows on Music (rank 2) or Sound (rank 3): ±12, S_CLINK
## at the limit (nortsong.c:JE_changeVolume), the muted channel is restored
## (menu_enable_music / menu_enable_samples). False on the other rows.
func _volume_key(delta: int) -> bool:
	var row := menus.selection() - ROW_VOLUME_FIRST
	if row < 0 or row > 1:
		return false
	Sfx.play(Sfx.CURSOR)
	_set_volume(row, int(SETUP.settings[VOLUME_KEYS[row]]) + delta, true)
	return true


## Sets the volume of channel `row` (0 music, 1 effects) clamped to 0..255;
## `clink`: S_CLINK when the limit is reached (arrows only).
func _set_volume(row: int, value: int, clink: bool) -> void:
	if clink and (value > 255 or value < 0):
		Sfx.play(Sfx.CLINK)
	SETUP.settings[VOLUME_KEYS[row]] = clampi(value, 0, 255)
	SETUP.settings[VOLUME_DISABLED_KEYS[row]] = false
	SETUP.apply_settings()
	SETUP.save_settings()
	menus.draw()


## game_menu.c:JE_itemScreen, "mouseButton > 0" branch: click on a volume
## bar (x >= 221, y 70..82 or 86..98) = volume (x - 221) / 4 * 12,
## row selected, S_CURSOR; the normal row click follows (S_CLICK).
func _click_volume_bar(p: Vector2i) -> void:
	if p.x < VOLUME_CLICK_X:
		return
	for row in 2:
		var y: int = VOLUME_BAR_Y[row]
		if p.y >= y and p.y <= y + 12:
			menus.current().sel = ROW_VOLUME_FIRST + row
			_set_volume(row, (p.x - VOLUME_CLICK_X) / 4 * 12, false)
			Sfx.play(Sfx.CURSOR)


## game_menu.c:MENU_MOUSE_CONFIG: three buttons (value 1..5: main fire,
## left sidekick, right, both, rear fire switch), Reset, Done.
func _open_mouse_buttons() -> void:
	var items := []
	for i in 3:
		items.append(MenuEngine.item(TyrianText.L("menuInt_15_%d" % (i + 1)), "mmhelp_%d" % HELP_MOUSE_BUTTON,
				MenuEngine.Kind.ACTION, {"gap": 8 * i, "action": _cycle_mouse_button.bind(i)}))
	items.append(MenuEngine.item(TyrianText.L("menuInt_15_4"), "mmhelp_%d" % HELP_MOUSE_RESET,
			MenuEngine.Kind.ACTION, {"gap": 8 * 3, "action": _reset_mouse_buttons}))
	items.append(MenuEngine.item(TyrianText.L("menuInt_15_5"), "mmhelp_%d" % (HELP_DONE - 1),
			MenuEngine.Kind.BACK, {"gap": 8 * 4}))
	menus.open(TyrianText.L("menuInt_15_0"), items, {
		"sel": _remembered_row("mouse"),
		"style": MenuEngine.Style.GAME_MENU, "accept": [TyrianControls.ACCEPT, TyrianControls.SELECT],
		"move_sfx": Sfx.CURSOR, "activate_sfx": Sfx.CLICK,
		"hover_sfx": Sfx.CLICK, "right_click": "click", "click_selects_first": true,
		# mouseSelectionY[MENU_MOUSE_CONFIG] = 24: (y - 38) / 24, column 170..308.
		"hit": func(p: Vector2i) -> int:
			if p.x <= 170 or p.x >= 308 or p.y <= 20:
				return -1
			var row := (p.y - 38) / MOUSE_ROW_STEP
			return row if row < items.size() else -1,
		"background": func() -> void: vga.set_indices(VGA_SCREEN, _background),
		"decorate": func() -> void:
			var buttons: Array = SETUP.settings["mouse_buttons"]
			_remember_row("mouse")
			for i in 3:
				TyrianText.text_shade(self, MOUSE_VALUE_X, MOUSE_VALUE_Y + 24 * i,
						TyrianText.L("joyButton_%d" % (int(buttons[i]) - 1)), 15, 2, TyrianText.PART_SHADE)
			_help(menus.current().items[menus.selection()].help),
	})


## game_menu.c:JE_menuFunction, MENU_MOUSE_CONFIG cases 2..4: next value (5 then 1).
func _cycle_mouse_button(index: int) -> void:
	var buttons: Array = SETUP.settings["mouse_buttons"].duplicate()
	buttons[index] = int(buttons[index]) % 5 + 1
	SETUP.settings["mouse_buttons"] = buttons
	SETUP.save_settings()


## game_menu.c:JE_menuFunction, MENU_MOUSE_CONFIG case 5: defaultMouseSettings.
func _reset_mouse_buttons() -> void:
	SETUP.settings["mouse_buttons"] = SETUP.DEFAULTS["mouse_buttons"].duplicate()
	SETUP.save_settings()


## MENU_LOAD_SAVE: 11 slots and "Exit to Game Menu".
func _open_slots(save: bool) -> void:
	_saving = save
	_message = ""
	_slots = Game.campaign_slots()
	# config.c:(slot-1) > 10 = two players: offset 11 for the 2-player page.
	_page_offset = 11 if (Game.modes.size() >= 1 and Game.modes[0] != 0) else 0
	var rows := []
	for i in Game.SAVE_SLOTS_PER_PAGE + 1:
		rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"index": i, "draw": _draw_row}))
	# Load and Save are the same C screen (MENU_LOAD_SAVE): a single row kept.
	menus.open(TyrianText.L("sl_save" if save else "sl_load"), rows, {
		"sel": _remembered_row("slots"),
		"style": MenuEngine.Style.ROWS, "row_x": X_NAME, "row_y": Y_ROWS, "row_step": DY_ROWS,
		# game_menu.c:mouseSelectionY[MENU_LOAD_SAVE] = 11, column 170..308,
		# hover S_CLICK, click that selects then acts (any button).
		"hover_sfx": Sfx.CLICK, "right_click": "click", "click_selects_first": true,
		# mouse_in_menu_column: y > 20 and 170 < x < 308; (y - 38) / 11 truncated
		# toward zero, so y 21..37 targets the first slot (M12).
		"hit_x": 171, "hit_w": 137, "hit_row_y": Y_ROWS, "hit_y0": 21, "row_h": DY_ROWS,
		"accept": [TyrianControls.ACCEPT, TyrianControls.SELECT], "move_sfx": Sfx.CURSOR,
		"background": func() -> void: vga.set_indices(VGA_SCREEN, _background),
		# game_menu.c:507 then game_menu.c:627, in the loop over the 12 rows:
		# the title is redrawn 13 times per frame, its shadow (blit_sprite_dark)
		# darkens at each pass.
		"header_draw": func() -> void:
			for n in Game.SAVE_SLOTS_PER_PAGE + 2:
				menus.draw_game_menu_header(menus.current().header),
		"decorate": func() -> void:
			_remember_row("slots")
			_help(_message if _message != "" else "mmhelp_%d" % (HELP_SLOTS - 1)),
		"on_activate": _choose,
	})


## A MENU_LOAD_SAVE row (colors of JE_drawMenuChoices: 15 / 28
## for the name, 252 / 250 for the level and episode, DARKEN shadow).
func _draw_row(it: Dictionary, x: int, y: int, sel: bool) -> void:
	var i: int = it.index
	var c_name := 15 if sel else 28
	var c_info := 252 if sel else 250
	if i == Game.SAVE_SLOTS_PER_PAGE:
		TyrianText.text_shade(self, x, y, TyrianText.L("miscText_5"), c_name / 16, c_name % 16 - 8, TyrianText.DARKEN)
		return
	var s = _slots[i + _page_offset] if (i + _page_offset) < _slots.size() else null
	var label: String
	var level := "-----"
	var ep := ""
	if s == null:
		label = TyrianText.L("misc_empty")
	elif str(s.get("error", "")) != "":
		label = TyrianText.L("save_unreadable")
	else:
		label = str(s["name"])
		level = str(s["last_level"])
		if level == "Completed":
			level = TyrianText.L("save_completed")
		ep = "%s%d" % [TyrianText.L("miscTextB_0"), int(s["episode"])]
	TyrianText.text_shade(self, x, y, label, c_name / 16, c_name % 16 - 8, TyrianText.DARKEN)
	TyrianText.text_shade(self, X_LEVEL, y, level, c_info / 16, c_info % 16 - 8, TyrianText.DARKEN)
	if ep != "":
		TyrianText.text_shade(self, X_EPISODE, y, ep, c_info / 16, c_info % 16 - 8, TyrianText.DARKEN)


## game_menu.c, MENU_LOAD_SAVE: exit, load or save.
func _choose(sel: int) -> void:
	if sel == Game.SAVE_SLOTS_PER_PAGE:
		Sfx.play(Sfx.CLICK)
		menus.back()
		return
	var s = _slots[sel + _page_offset] if (sel + _page_offset) < _slots.size() else null
	if not _saving:
		if s == null:
			Sfx.play(23)  # S_CLINK
		elif str(s.get("error", "")) != "":
			Sfx.play(23)
			_message = "save_refused"
			push_warning("Slot %d unreadable: %s" % [sel + 1 + _page_offset, s["error"]])
		else:
			Sfx.play(Sfx.CLICK)
			idle = false
			await fade_black(15)
			Game.load_campaign_slot(s)
			return
		menus.draw()
		return
	if (
		sel + 1 + _page_offset == Game.AUTOSAVE_SLOT
		or sel + 1 + _page_offset == Game.AUTOSAVE_SLOT + Game.SAVE_SLOTS_PER_PAGE
	):
		Sfx.play(23)
		_message = "save_slot_reserved"
		menus.draw()
		return
	Sfx.play(Sfx.CLICK)
	var name = await _prompt_name(str(s["name"]) if s != null and str(s.get("error", "")) == "" else "")
	# Confirmation box (JE_saveRequest): waits for a key.
	idle = name != null
	if name == null:
		menus.draw()
		return
	# Intentional deviation (docs/divergences.md): the C code does not redraw the screen, its box covers the name box
	# and leftovers of the list protrude below Yes / No. The list is redrawn before the box.
	menus.draw()
	vga.bar_shade(VGA_SCREEN, 0, 0, 319, 199)
	var original := str(s["name"]) if s != null and str(s.get("error", "")) == "" else ""
	var info := {"name": name, "original": original}
	# JE_saveRequest: Yes preselected.
	if await quit_request_msg(TyrianText.L("miscText_68"), "", true, info):
		_message = "miscText_60" if Game.save_campaign(sel + 1 + _page_offset, name) else "save_failed"
		_slots = Game.campaign_slots()
	menus.draw()


## mainint.c:JE_operation: name entry box (blinking cursor,
## uppercase letters, Backspace, Enter confirms, Esc cancels).
## Returns the name, or null if cancelled.
func _prompt_name(initial: String) -> Variant:
	var name := initial.left(NAME_MAX)
	var flash := 8 * 16 + 10
	vga.bar_shade(VGA_SCREEN, 65, 55, 255, 155)
	_typing = true
	_pending.clear()
	var result: Variant = null
	while true:
		vga.blit_sprite(VGA_SCREEN, 50, 50, OPTION_SHAPES, 35)
		TyrianText.text_shade(self, 60, 55, TyrianText.L("miscText_0"), 11, 4, TyrianText.DARKEN)
		TyrianText.text_shade(self, 70, 70, Game.level_name, 11, 4, TyrianText.DARKEN)
		flash = 8 * 16 + 2 if flash == 8 * 16 + 10 else 8 * 16 + 10
		var line := TyrianText.L("miscText_1") + name
		TyrianText.out_text(self, 65, 89, line, 8, 3)
		var w := 65 + TyrianText.width(line.replace("~", ""), SMALL_FONT)
		vga.bar_shade(VGA_SCREEN, w + 2, 90, w + 6, 95)
		vga.fill_rectangle_xy(VGA_SCREEN, w + 1, 89, w + 5, 94, flash)
		var ok := TyrianText.L(SAVE_BOX_YES)
		var cancel := TyrianText.L(SAVE_BOX_NO)
		TyrianText.out_text_adjust(self, 54 + 45 - TyrianText.width(ok, LARGE_FONT) / 2, 128, ok, 15, -5, LARGE_FONT,
				true)
		TyrianText.out_text_adjust(self, 149 + 45 - TyrianText.width(cancel, LARGE_FONT) / 2, 128, cancel, 15, -5,
				LARGE_FONT, true)
		present()
		# 14 × setDelay(1), interrupted by a key.
		idle = _pending.is_empty()
		for i in 14:
			if not _pending.is_empty():
				break
			await wait_units(1)
		idle = false
		var done := false
		while not _pending.is_empty() and not done:
			var k = _pending.pop_front()
			if k is StringName:
				match k:
					TyrianControls.DELETE:
						if name.length() > 0:
							name = name.left(name.length() - 1)
							Sfx.play(Sfx.CLICK)
					TyrianControls.ACCEPT:
						result = name.strip_edges(false, true)
						done = true
					TyrianControls.BACK:
						Sfx.play(Sfx.SPRING)
						done = true
			elif name.length() < NAME_MAX and (k == " " or TyrianText.sprite_id(String(k).unicode_at(0)) >= 0):
				Sfx.play(Sfx.CURSOR)
				name += k
		if done:
			break
	_typing = false
	return result


## Name entry: typed character (uppercase) or control key, whatever
## the action bound to the letter (M, E, R, Y, N... are also menu
## hotkeys). Without `unicode` (capture tool --keys), the letter
## or digit of the key.
func handle_event(event: InputEvent) -> void:
	if _capturing:
		_capture_input(event)
		return
	if not _typing:
		super.handle_event(event)
		return
	# joystick.c:push_joysticks_as_keyboard (game_menu.c, name entry):
	# gamepad confirm and cancel become RETURN and ESCAPE.
	if event is InputEventJoypadButton and event.pressed:
		var pad := TyrianControls.menu_action(event)
		if pad in [TyrianControls.ACCEPT, TyrianControls.BACK]:
			idle = false
			_pending.append(pad)
		return
	var k := event as InputEventKey
	if k == null or not k.pressed or k.echo:
		return
	idle = false
	var action := TyrianControls.menu_action(event)
	if action in [TyrianControls.ACCEPT, TyrianControls.BACK, TyrianControls.DELETE]:
		_pending.append(action)
		return
	var code := k.unicode
	if (
		code == 0
		and (
			k.keycode >= KEY_A and k.keycode <= KEY_Z
			or k.keycode >= KEY_0 and k.keycode <= KEY_9
			or k.keycode == KEY_SPACE
		)
	):
		code = k.keycode
	if code >= 32 and code < 127:
		_pending.append(char(code).to_upper())


func handle_action(action: StringName) -> void:
	if modal_input(action) or _typing or _capturing or menus.stack.is_empty():
		return
	menus.handle_action(action)


## Mouse: name entry (mainint.c:JE_operation: click on "OK" x 56..142,
## "Cancel" x 151..237, y 123..149) or menus (shared engine).
func handle_mouse(event: InputEvent) -> void:
	if modal_active:
		super(event)
		return
	if _capturing:
		if TyrianMouse.is_button_press(event):
			_capture_event = event
		return
	if menus.stack.size() == 1 and not _typing and TyrianMouse.is_button_press(event):
		_click_volume_bar(Vector2i(event.position.floor()))
	if _typing:
		if not TyrianMouse.is_button_press(event):
			return
		var p: Vector2 = event.position
		if p.y > 123 and p.y < 149 and p.x > 56 and p.x < 142:
			_pending.append(TyrianControls.ACCEPT)
		elif p.y > 123 and p.y < 149 and p.x > 151 and p.x < 237:
			_pending.append(TyrianControls.BACK)
		return
	super(event)


## Event received during a capture (keyboard.c: newkey, mousedown, joydown).
## Gamepad button or axis: retained by the gamepad capture, ends the key one.
func _capture_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		_capture_event = event
	elif event is InputEventJoypadButton and event.pressed:
		_capture_event = event
	elif event is InputEventJoypadMotion and _capture_wants_pad and TyrianControls.pad_capture_event(event) != null:
		_capture_event = event


## "Done": back to the shop, which reopens with Game.loadout.
func _leave() -> void:
	idle = false
	Game.goto_scene(Game.SCENE_SHOP)
