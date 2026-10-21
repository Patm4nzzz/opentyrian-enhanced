## Mouse tests for the menu screens (ADR 0122):
## hover -> selection, left click -> activation,
## right click -> back/cancel, wheel -> list movement.
##
## All events are real InputEventMouse{Motion,Button}
## passed directly to the screens' handle_mouse.
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.inactive = true


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.inactive = true


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


static func _motion(x: float, y: float) -> InputEventMouseMotion:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	ev.relative = Vector2(1, 0)
	return ev


static func _left_press(x: float, y: float) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_LEFT
	ev.pressed = true
	return ev


static func _right_press(x: float, y: float) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_RIGHT
	ev.pressed = true
	return ev


static func _wheel_up(x: float, y: float) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_WHEEL_UP
	ev.pressed = true
	return ev


static func _wheel_down(x: float, y: float) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_WHEEL_DOWN
	ev.pressed = true
	return ev


# ---------------------------------------------------------------------------
# TyrianMouse: inactive toggle
# ---------------------------------------------------------------------------


## SDL_MOUSEBUTTONDOWN makes the mouse active (keyboard.c).
func test_mouse_click_activates() -> void:
	TyrianMouse.inactive = true
	TyrianMouse.note(_left_press(100, 100))
	assert_bool(TyrianMouse.inactive).is_false()


## The wheel does not change the toggle (it does not exist in the C).
func test_wheel_does_not_change_inactive_flag() -> void:
	TyrianMouse.inactive = true
	TyrianMouse.note(_wheel_up(100, 100))
	assert_bool(TyrianMouse.inactive).is_true()


## SDL_MOUSEMOTION with relative != 0 activates the mouse.
func test_mouse_motion_activates() -> void:
	TyrianMouse.inactive = true
	TyrianMouse.note(_motion(100, 100))
	assert_bool(TyrianMouse.inactive).is_false()


# ---------------------------------------------------------------------------
# quit_box_answer (tyrian_screen.gd)
# ---------------------------------------------------------------------------


func test_quit_box_answer_yes() -> void:
	var p_yes := Vector2(100, 136)
	assert_bool(TyrianScreen.quit_box_answer(p_yes) == true).is_true()


func test_quit_box_answer_no() -> void:
	var p_no := Vector2(200, 136)
	assert_bool(TyrianScreen.quit_box_answer(p_no) == false).is_true()


func test_quit_box_answer_outside() -> void:
	var p_out := Vector2(150, 100)
	assert_bool(TyrianScreen.quit_box_answer(p_out) == null).is_true()


# ---------------------------------------------------------------------------
# Title (TITLE style)
# ---------------------------------------------------------------------------


func test_title_hover_selects_row() -> void:
	var title_script: GDScript = load("res://scripts/title.gd")
	title_script.intro_shown = true
	title_script.move_tyrian_logo_up = false
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	await get_tree().process_frame
	TyrianMouse.inactive = false
	title.selected = 0
	var waited_3792 := 0
	while (title.menus.stack.is_empty()) and waited_3792 < 3000:
		await get_tree().process_frame
		waited_3792 += 1
	assert_bool(title.menus.stack.is_empty()).override_failure_message("screen never ready").is_false()
	# TITLE_Y_ITEMS = 88, TITLE_H_ITEM = 12 -> row 1: y=100.
	var items: Array = title.menus.current().items
	var waited_4030 := 0
	while (items.size() < 2) and waited_4030 < 3000:
		await get_tree().process_frame
		waited_4030 += 1
	assert_bool(items.size() < 2).override_failure_message("screen never ready").is_false()
	var w1 := TyrianText.width(items[1].label, TyrianScreen.NORMAL_FONT)
	var x1 := 160 - w1 / 2 + 2
	title.handle_mouse(_motion(x1, 100))
	assert_int(title.selected).is_equal(1)


func test_title_hover_ignored_when_inactive() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	TyrianMouse.inactive = true
	title.selected = 0
	title.handle_mouse(_motion(160, 100))
	assert_int(title.selected).is_equal(0)


## Click in the quit box when modal_active: modal_mouse receives
## the event and updates _modal_key (box resolution).
func test_title_modal_mouse_sets_answer() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	await get_tree().process_frame
	TyrianMouse.inactive = false
	# Manually activate the modal (simulates quit_request_msg pending).
	title.modal_active = true
	# Click No (x 151..237, y 123..149) -> _modal_key = NO.
	title.handle_mouse(_left_press(194, 136))
	assert_bool(title.get("_modal_key") == TyrianControls.NO).is_true()


## Click Yes in the box -> _modal_key = YES.
func test_title_modal_mouse_yes() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	await get_tree().process_frame
	TyrianMouse.inactive = false
	# Manually activate the modal.
	title.modal_active = true
	# Click Yes (x 56..142, y 123..149) -> _modal_key = YES.
	title.handle_mouse(_left_press(100, 136))
	assert_bool(title.get("_modal_key") == TyrianControls.YES).is_true()


# ---------------------------------------------------------------------------
# Setup (style MENU_TREE)
# ---------------------------------------------------------------------------


func test_setup_hover_selects_row() -> void:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_6004 := 0
	while (setup.menus.stack.is_empty()) and waited_6004 < 600:
		await get_tree().process_frame
		waited_6004 += 1
	assert_bool(setup.menus.stack.is_empty()).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	setup.handle_action(TyrianControls.UP)
	# menu_tree.c : DEFAULT_Y_START_WITH_HEADER = 47, DEFAULT_STEP = 14.
	# Row 2: y = 47 + 2*14 = 75.
	setup.handle_mouse(_motion(100, 75))
	assert_int(setup.menus.selection()).override_failure_message("hovering over row 2 must select it").is_equal(2)


## The C has no wheel (no SDL_MOUSEWHEEL in src/): no effect.
func test_setup_wheel_does_nothing() -> void:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_6744 := 0
	while (setup.menus.stack.is_empty()) and waited_6744 < 600:
		await get_tree().process_frame
		waited_6744 += 1
	assert_bool(setup.menus.stack.is_empty()).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	setup.handle_action(TyrianControls.DOWN)
	var sel_before: int = setup.menus.selection()
	setup.handle_mouse(_wheel_up(100, 100))
	assert_int(setup.menus.selection()).is_equal(sel_before)


func test_setup_right_click_back() -> void:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_7255 := 0
	while (setup.menus.stack.is_empty()) and waited_7255 < 600:
		await get_tree().process_frame
		waited_7255 += 1
	assert_bool(setup.menus.stack.is_empty()).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	setup.handle_mouse(_right_press(100, 100))
	var cur := Effects.current_game_scene()
	assert_str(cur.scene_file_path).is_equal(Game.SCENE_TITLE)


# ---------------------------------------------------------------------------
# High Scores (own mouse handling)
# ---------------------------------------------------------------------------


func test_high_scores_right_click_back() -> void:
	var hs: Node = Game.goto_scene("res://scenes/high_scores.tscn")
	await get_tree().process_frame
	TyrianMouse.inactive = false
	hs.handle_mouse(_right_press(100, 100))
	assert_bool(hs.idle).is_false()


func test_high_scores_wheel_does_nothing() -> void:
	var hs: Node = Game.goto_scene("res://scenes/high_scores.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	TyrianMouse.inactive = false
	hs.idle = true
	hs.set("_busy", false)
	var ep_before: int = hs.episode_index
	hs.handle_mouse(_wheel_down(100, 100))
	assert_int(hs.episode_index).is_equal(ep_before)


# ---------------------------------------------------------------------------
# Load Screen (style ROWS)
# ---------------------------------------------------------------------------


func test_load_screen_hover_selects_slot() -> void:
	var ls: Node = Game.goto_scene("res://scenes/load_screen.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_8749 := 0
	while (not ls.idle) and waited_8749 < 600:
		await get_tree().process_frame
		waited_8749 += 1
	assert_bool(not ls.idle).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	# LoadScreen: row_y=30 (Y_ITEMS), row_step=13 (DY_ITEMS) -> slot 2 a y=56.
	ls.handle_mouse(_motion(100, 56))
	assert_int(ls.selected).is_equal(2)


func test_load_screen_wheel_does_nothing() -> void:
	var ls: Node = Game.goto_scene("res://scenes/load_screen.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_9206 := 0
	while (not ls.idle or ls.menus.stack.is_empty()) and waited_9206 < 600:
		await get_tree().process_frame
		waited_9206 += 1
	assert_bool(not ls.idle or ls.menus.stack.is_empty()).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	ls.selected = 0
	var before: int = ls.selected
	ls.handle_mouse(_wheel_down(100, 56))
	assert_int(ls.selected).is_equal(before)


# ---------------------------------------------------------------------------
# Instructions (TOPICS style)
# ---------------------------------------------------------------------------


func test_instructions_hover_topic() -> void:
	var ins: Node = Game.goto_scene("res://scenes/instructions.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_9885 := 0
	while (ins.menus.stack.is_empty() or ins.get("_busy") == true) and waited_9885 < 600:
		await get_tree().process_frame
		waited_9885 += 1
	(
		assert_bool(ins.menus.stack.is_empty() or ins.get("_busy") == true)
		. override_failure_message("screen never ready")
		. is_false()
	)
	TyrianMouse.inactive = false
	assert_int(ins.page).is_equal(0)
	# TOPICS_Y_ITEMS = 60, TOPICS_DY_ITEMS = 20, TOPICS_H_ITEM = 13.
	# Topic 1: y = 60 + 1*20 = 80; horizontal center = 160.
	var items: Array = ins.menus.current().items
	var waited_10299 := 0
	while (items.size() < 2) and waited_10299 < 600:
		await get_tree().process_frame
		waited_10299 += 1
	assert_bool(items.size() < 2).override_failure_message("screen never ready").is_false()
	var w := TyrianText.width(items[1].label, TyrianScreen.NORMAL_FONT)
	var x := MenuEngine.TOPICS_X_CENTER - w / 2 + 2
	ins.handle_mouse(_motion(x, 80))
	assert_int(ins.selected_index).is_equal(1)


func test_instructions_right_click_back_to_topics() -> void:
	var ins: Node = Game.goto_scene("res://scenes/instructions.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_10795 := 0
	while (ins.menus.stack.is_empty() or ins.get("_busy") == true) and waited_10795 < 600:
		await get_tree().process_frame
		waited_10795 += 1
	(
		assert_bool(ins.menus.stack.is_empty() or ins.get("_busy") == true)
		. override_failure_message("screen never ready")
		. is_false()
	)
	TyrianMouse.inactive = false
	ins.handle_action(TyrianControls.ACCEPT)
	assert_bool(ins.page > 0).is_true()
	ins.handle_mouse(_right_press(100, 100))
	assert_int(ins.page).is_equal(0)


# ---------------------------------------------------------------------------
# Shop (GAME_MENU style)
# ---------------------------------------------------------------------------


func _open_shop() -> Node:
	Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 15267])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.shop_section = int(r.get("section", 4))
	var mi := 0
	while r.has("planet%d" % mi):
		Game.map_planets.append(r["planet%d" % mi])
		mi += 1
	Game.debrief = {"cube_max": 0}
	return Game.goto_scene(Game.SCENE_SHOP)


## The shop hovers a row: hover_row(2) returns true (the selection changes),
## which updates _state["sel"] via the Rust simulation.
func test_shop_hover_selects_row() -> void:
	var shop: Node = _open_shop()
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_12082 := 0
	while (shop.get("_shop") == null) and waited_12082 < 600:
		await get_tree().process_frame
		waited_12082 += 1
	assert_bool(shop.get("_shop") == null).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	# GAME_MENU: x > 170, x < 308, y > 20. row = (y - 38) / 16.
	# Row 2: y = 38 + 2*16 = 70.
	# Row 0 first, to have a known starting state.
	shop.handle_mouse(_motion(220, 38))
	await get_tree().process_frame
	var sel_before: int = int(shop.get("_state").get("sel", 0))
	# Hover row 2.
	shop.handle_mouse(_motion(220, 70))
	await get_tree().process_frame
	var sel_after: int = int(shop.get("_state").get("sel", 0))
	assert_bool(sel_after != sel_before or sel_after == 2).is_true()
	# The hovered row is also the one the menu engine draws as chosen.
	Effects.game_viewport.push_input(_motion(221, 86))  # row 5
	await get_tree().process_frame
	assert_int(int(shop.get("_state").get("sel", 0))).is_equal(5)
	assert_int(shop.menus.selection()).override_failure_message("the hovered row is not highlighted").is_equal(3)


func test_shop_wheel_does_nothing() -> void:
	var shop: Node = _open_shop()
	await get_tree().process_frame
	TyrianMouse.inactive = false
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.DOWN)
	var before: int = shop.menus.selection()
	shop.handle_mouse(_wheel_up(220, 70))
	assert_int(shop.menus.selection()).is_equal(before)


## The shop treats the right click like the left (game_menu.c: any
## button activates the row). A click at x < 170 or outside the area (row = -1)
## does nothing (handle_mouse returns early).
func test_shop_click_outside_does_nothing() -> void:
	var shop: Node = _open_shop()
	await get_tree().process_frame
	await get_tree().process_frame
	var waited_13769 := 0
	while (shop.get("_shop") == null) and waited_13769 < 600:
		await get_tree().process_frame
		waited_13769 += 1
	assert_bool(shop.get("_shop") == null).override_failure_message("screen never ready").is_false()
	TyrianMouse.inactive = false
	var state_before: Variant = shop.get("_state").duplicate()
	# Click outside the column (x < 170): row = -1, no action.
	shop.handle_mouse(_left_press(100, 70))
	# The menu state does not change.
	assert_bool(int(shop.get("_state").get("menu", 0)) == int(state_before.get("menu", 0))).is_true()


# ---------------------------------------------------------------------------
# DisplaySettings (Node2D with handle_action)
# ---------------------------------------------------------------------------


func test_display_settings_hover() -> void:
	var ds: Node = Game.goto_scene(DisplaySettings.SCENE_PATH)
	await get_tree().process_frame
	TyrianMouse.inactive = false
	ds.selected = 0
	# y0 = 60, step = 14: row 1 is at y = 74.
	ds.handle_mouse(_motion(100, 74))
	assert_int(ds.selected).is_equal(1)


func test_display_settings_wheel_does_nothing() -> void:
	var ds: Node = Game.goto_scene(DisplaySettings.SCENE_PATH)
	await get_tree().process_frame
	TyrianMouse.inactive = false
	ds.selected = 0
	ds.handle_mouse(_wheel_down(100, 74))
	assert_int(ds.selected).is_equal(0)


func test_display_settings_right_click_back() -> void:
	var ds: Node = Game.goto_scene(DisplaySettings.SCENE_PATH)
	await get_tree().process_frame
	TyrianMouse.inactive = false
	ds.handle_mouse(_right_press(100, 100))
	var cur := Effects.current_game_scene()
	assert_str(cur.scene_file_path).is_equal(Game.SCENE_TITLE)


# ---------------------------------------------------------------------------
# Jukebox (style ROWS)
# ---------------------------------------------------------------------------


## Right click in the jukebox -> back to Setup (handle_action BACK).
func test_jukebox_right_click_back() -> void:
	var jb: Node = Game.goto_scene("res://scenes/jukebox.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	# The palette fade may never finish without a display: the loop runs, that is enough.
	var waited := 0
	while not jb._running and waited < 600:
		await get_tree().process_frame
		waited += 1
	assert_bool(jb._running).override_failure_message("jukebox never started").is_true()
	TyrianMouse.inactive = false
	jb.handle_mouse(_right_press(100, 100))
	# The exit goes through a palette fade before handing control back to Setup.
	var left := 0
	while Effects.current_game_scene() == jb and left < 1200:
		await get_tree().process_frame
		left += 1
	var cur := Effects.current_game_scene()
	# The jukebox goes back to Setup (jukebox.gd: on_back -> Game.SCENE_SETUP).
	assert_str(cur.scene_file_path).is_equal(Game.SCENE_SETUP)


# ---------------------------------------------------------------------------
# Shop, Data list: hover only chooses (game_menu.c:JE_itemScreen)
# ---------------------------------------------------------------------------


## Pointer moved over each cube of the list, then outside the rows: the list
## stays displayed, the hovered row is chosen, no cube opens.
func test_shop_data_cubes_hover_only_selects() -> void:
	Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 14983])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true, 2)
	Game.set_shop(r, 2)
	var m: PackedInt32Array = r.get("map", PackedInt32Array())
	Game.map_sections = PackedInt32Array()
	Game.map_planets = PackedInt32Array()
	for i in range(1, m.size(), 2):
		Game.map_planets.append(m[i - 1])
		Game.map_sections.append(m[i])
	Game.debrief = {"cube_max": 3}
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	await get_tree().process_frame
	await get_tree().process_frame
	shop.handle_action(TyrianControls.ACCEPT)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBES)
	TyrianMouse.inactive = false
	# Cubes: rows at y = 38 + 28 i; hover at 46, 62, 78, 94 as on screen.
	for y in [46, 62, 78, 94, 100, 62]:
		Effects.game_viewport.push_input(_motion(220, y))
		await get_tree().process_frame
		assert_int(int(shop._state["menu"])).override_failure_message("hover y=%d: the playback opened" % y).is_equal(
			shop.MENU_DATA_CUBES
		)
	# A click, on the other hand, does open the reader.
	Effects.game_viewport.push_input(_left_press(220, 62))
	await get_tree().process_frame
	Effects.game_viewport.push_input(_left_press(220, 62))
	await get_tree().process_frame
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBE_SUB)
	Game.shop_cube_list = PackedInt32Array()
	Game.shop_cube_max = 0


# ---------------------------------------------------------------------------
# Cursor sound on every selection move (keyboard and mouse)
# ---------------------------------------------------------------------------


func _count_cursor(from: int) -> int:
	var n := 0
	for i in range(from, Sfx.played.size()):
		if Sfx.played[i] == Sfx.CURSOR:
			n += 1
	return n


## MENU_TREE screens (Setup, controls): Up / Down silent like the C
## (menu_tree.c:488, boss's decision).
func test_keyboard_move_is_silent_in_setup_and_controls() -> void:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	var waited := 0
	while not setup.idle and waited < 600:
		await get_tree().process_frame
		waited += 1
	assert_bool(setup.idle).override_failure_message("Setup never ready").is_true()
	var mark: int = Sfx.played.size()
	Effects.game_viewport.push_input(_key(KEY_DOWN))
	Effects.game_viewport.push_input(_key(KEY_DOWN))
	await get_tree().process_frame
	assert_int(_count_cursor(mark)).override_failure_message("Setup: Up/Down silent like the C").is_equal(0)
	var ctl: Node = Game.goto_scene("res://scenes/controls_options.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	mark = Sfx.played.size()
	ctl.handle_action(TyrianControls.DOWN)
	ctl.handle_action(TyrianControls.UP)
	assert_int(_count_cursor(mark)).override_failure_message("Controls: Up/Down silent like the C").is_equal(0)


static func _key(code: int) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.physical_keycode = code
	ev.keycode = code
	ev.pressed = true
	return ev


func test_cursor_sound_on_shop_hover_and_keys() -> void:
	var shop: Node = _open_shop()
	await get_tree().process_frame
	await get_tree().process_frame
	TyrianMouse.inactive = false
	var mark: int = Sfx.played.size()
	Effects.game_viewport.push_input(_motion(221, 86))
	await get_tree().process_frame
	assert_int(_count_cursor(mark)).override_failure_message("shop: hover silent").is_equal(1)
	mark = Sfx.played.size()
	shop.handle_action(TyrianControls.DOWN)
	assert_int(_count_cursor(mark)).override_failure_message("shop: Down silent").is_equal(1)


# ---------------------------------------------------------------------------
# Buying a front weapon with the mouse alone (choice, power, confirmation)
# ---------------------------------------------------------------------------


func _click(_shop: Node, x: float, y: float) -> void:
	Effects.game_viewport.push_input(_left_press(x, y))
	await get_tree().process_frame


func _wait_shop_idle(shop: Node) -> void:
	var w := 0
	while not shop.idle and w < 600:
		await get_tree().process_frame
		w += 1


## Data: Upgrade Ship > front weapon, 40000 credits. Click on another weapon =
## it is chosen (S_CLICK) without buying anything; right arrow = power + 1;
## left arrow = power - 1; arrow refused at power 1 = S_CLINK;
## click on "Done" = purchase, money goes down.
func test_buy_front_weapon_with_mouse_only() -> void:
	Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 40000])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.shop_section = int(r.get("section", 4))
	Game.map_planets = PackedInt32Array()
	var mi := 0
	while r.has("planet%d" % mi):
		Game.map_planets.append(r["planet%d" % mi])
		mi += 1
	Game.set_shop(r, 0)
	Game.debrief = {"cube_max": 0}
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	await _wait_shop_idle(shop)
	TyrianMouse.inactive = false
	# Root: Upgrade Ship is row 4 (y = 38 + 16 * 2).
	var upgrade_row := 4
	Effects.game_viewport.push_input(_motion(220, 38 + 16 * (upgrade_row - 2)))
	await get_tree().process_frame
	await _click(shop, 220, 38 + 16 * (upgrade_row - 2))
	await _click(shop, 220, 38 + 16 * (upgrade_row - 2))
	assert_int(int(shop._state["menu"])).override_failure_message("Upgrade Ship not open").is_equal(1)
	# "Front weapon" category: row 3 of this menu (y = 38 + 16 * 1).
	var cat_y := 38 + 16 * 1
	Effects.game_viewport.push_input(_motion(220, cat_y))
	await get_tree().process_frame
	await _click(shop, 220, cat_y)
	assert_int(int(shop._state["menu"])).override_failure_message("front weapon category not open").is_equal(
		shop.MENU_UPGRADE_SUB
	)
	await _wait_shop_idle(shop)
	var items_before: PackedInt32Array = shop._shop.loadout()
	# Choosing another weapon: hover then click on the second row.
	var row_y := func(row: int) -> float: return 38.0 + 26.0 * (row - 2) + 1.0
	Effects.game_viewport.push_input(_motion(220, row_y.call(4)))
	await get_tree().process_frame
	var mark: int = Sfx.played.size()
	await _click(shop, 220, row_y.call(4))
	assert_int(int(shop._state["sel"])).override_failure_message("the clicked row is not selected").is_equal(4)
	# Right arrow: power + 1 (or S_CLINK refusal if too expensive / at the maximum).
	var power_before: int = int(shop._shop.loadout()[2])
	await _click(shop, 125, 158)
	var power_after: int = int(shop._shop.loadout()[2])
	var heard_clink: bool = Sfx.played.slice(mark).has(Sfx.CLINK)
	(
		assert_bool(power_after == power_before + 1 or heard_clink)
		. override_failure_message("right arrow with no effect or refusal (%d -> %d)" % [power_before, power_after])
		. is_true()
	)
	# Left arrow: back to the starting power.
	if power_after > power_before:
		await _click(shop, 30, 158)
		assert_int(int(shop._shop.loadout()[2])).override_failure_message("left arrow with no effect").is_equal(
			power_before
		)
	# Left arrow at power 1: refusal.
	mark = Sfx.played.size()
	await _click(shop, 30, 158)
	await _click(shop, 30, 158)
	(
		assert_bool(Sfx.played.slice(mark).has(Sfx.CLINK))
		. override_failure_message("downgrade impossible without S_CLINK")
		. is_true()
	)
	# Purchase: click on "Done" (last row) = the chosen weapon is paid for.
	var cash_before_buy: int = int(items_before[10])
	var weapon_before: int = int(items_before[1])
	var done_sel: int = shop.menus.current().items.size() + 1
	var found_done := false
	var yy := 40
	for step in range(38):
		yy = 40 + step * 4
		Effects.game_viewport.push_input(_motion(220, yy))
		await get_tree().process_frame
		# Purchase list: the hover applies once the row is held (HOVER_DWELL_MS).
		shop.hover_tick(int(shop.hover_clock.call()) + shop.HOVER_DWELL_MS)
		if int(shop._state["menu"]) == shop.MENU_UPGRADE_SUB and int(shop._state["sel"]) == done_sel:
			found_done = true
			break
	assert_bool(found_done).override_failure_message("'Done' row not found on hover").is_true()
	if found_done:
		await _click(shop, 220, yy)
		assert_int(int(shop._state["menu"])).override_failure_message("'Done' did not confirm the purchase").is_equal(1)
		(
			assert_int(int(shop._shop.loadout()[1]))
			. override_failure_message(
				(
					"front weapon unchanged after purchase: %s -> %s, money %d -> %d"
					% [items_before, shop._shop.loadout(), cash_before_buy, int(shop._shop.loadout()[10])]
				)
			)
			. is_not_equal(weapon_before)
		)
		(
			assert_bool(int(shop._shop.loadout()[10]) < cash_before_buy)
			. override_failure_message("money not debited")
			. is_true()
		)
	Game.campaign_active = false
	await get_tree().process_frame


## Player bug of 2026-10-10: in Upgrade Ship > Front Gun, Enter then a click
## on another weapon must equip it ("owned" mark moved, cash register
## debited), the cursor staying on the row (divergence no. 53).
func test_shop_enter_and_click_buy_a_weapon() -> void:
	Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 40000])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.shop_section = int(r.get("section", 4))
	Game.map_planets = PackedInt32Array()
	Game.set_shop(r, 0)
	Game.debrief = {"cube_max": 0}
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	await _wait_shop_idle(shop)
	# Upgrade Ship (row 4) then Front Gun (row 3).
	for a in [
		TyrianControls.DOWN, TyrianControls.DOWN, TyrianControls.ACCEPT, TyrianControls.DOWN, TyrianControls.ACCEPT
	]:
		shop.handle_action(a)
	var st: Dictionary = shop.get("_state")
	assert_int(int(st["menu"])).is_equal(4)
	var ids: PackedInt32Array = st["items"]
	var owned := int(st["owned_item"])
	var cash_start := int(shop._shop.loadout()[10])
	# Keyboard: Down to another affordable weapon (the too-expensive ones are skipped), then Enter.
	var choices := int(st["menu_choices"])
	for _i in choices:
		shop.handle_action(TyrianControls.DOWN)
		var s := int(shop.get("_state")["sel"])
		if s < choices and ids[s - 2] != owned:
			break
	var sel := int(shop.get("_state")["sel"])
	assert_int(sel).override_failure_message("no other purchasable weapon: %s" % [shop.get("_state")]).is_less(choices)
	var target := ids[sel - 2]
	assert_int(target).is_not_equal(owned)
	shop.handle_action(TyrianControls.ACCEPT)
	st = shop.get("_state")
	assert_int(int(st["menu"])).override_failure_message("Enter left the list").is_equal(4)
	assert_int(int(st["owned_item"])).override_failure_message("Enter did not equip the weapon").is_equal(target)
	# Exact price and resume: Rust tests (tyrian-sim, enter_on_an_item_buys_it_and_moves_the_owned_marker).
	# Mouse: click on another row (26 px step from y = 38).
	var row := 2
	while ids[row - 2] == target or ids[row - 2] == 0:
		row += 1
	TyrianMouse.inactive = false
	shop.handle_mouse(_left_press(230, 38 + 26 * (row - 2) + 10))
	st = shop.get("_state")
	assert_int(int(st["owned_item"])).override_failure_message("the click did not equip the weapon").is_equal(
		ids[row - 2]
	)
	assert_int(int(st["sel"])).is_equal(row)
	# Esc after the purchase: the weapon stays mounted (nothing is taken back).
	shop.handle_action(TyrianControls.BACK)
	assert_int(int(shop._shop.loadout()[1])).override_failure_message("Escape resumed the purchase").is_equal(
		ids[row - 2]
	)
	assert_int(int(shop._shop.loadout()[10])).override_failure_message("cash unchanged").is_not_equal(cash_start)
	Game.campaign_active = false


## Every list screen plays the cursor when Down or Up moves the selection
## (the replays screen is excluded: with no .rep file there is nothing to browse).
func test_cursor_sound_on_arrow_keys_in_every_list_screen() -> void:
	var silent: Array[String] = []
	for path in [
		"res://scenes/title.tscn",
		"res://scenes/save_menu.tscn",
		"res://scenes/high_scores.tscn",
		"res://scenes/instructions.tscn",
		"res://scenes/display_options.tscn",
		"res://scenes/load_screen.tscn"
	]:
		if not ResourceLoader.exists(path):
			continue
		var scene: Node = Game.goto_scene(path)
		var w := 0
		while w < 400 and (scene.get("idle") == false or scene.get("_busy") == true):
			await get_tree().process_frame
			w += 1
		await get_tree().process_frame
		if not scene.has_method("handle_action"):
			continue
		var mark: int = Sfx.played.size()
		# high_scores: the cursor is horizontal (episodes), same sound.
		var next := TyrianControls.RIGHT if path.ends_with("high_scores.tscn") else TyrianControls.DOWN
		scene.handle_action(next)
		scene.handle_action(TyrianControls.UP if next == TyrianControls.DOWN else TyrianControls.LEFT)
		scene.handle_action(next)
		if _count_cursor(mark) == 0:
			silent.append(path)
	assert_array(silent).override_failure_message("screens silent on arrows: %s" % [silent]).is_empty()
	# MENU_TREE screens of the C (menu_tree.c:488) and the title's achievements list
	# (title_screen.c:855-871): Up / Down silent.
	for path in [
		"res://scenes/modes_menu.tscn",
		"res://scenes/mods.tscn",
		"res://scenes/controls_options.tscn",
		"res://scenes/achievements_screen.tscn"
	]:
		var scene: Node = Game.goto_scene(path)
		var w := 0
		while w < 400 and (scene.get("idle") == false or scene.get("_busy") == true):
			await get_tree().process_frame
			w += 1
		await get_tree().process_frame
		var mark: int = Sfx.played.size()
		scene.handle_action(TyrianControls.DOWN)
		scene.handle_action(TyrianControls.UP)
		assert_int(_count_cursor(mark)).override_failure_message("%s: Up/Down silent like the C" % path).is_equal(0)


## Replays screen with three .rep files from the repository: Down / Up play the cursor.
func test_cursor_sound_on_replays_screen_with_files() -> void:
	var dir := ProjectSettings.globalize_path("res://").path_join("../golden/replays").simplify_path()
	Capture._options["replay-dir"] = dir
	var scene: Node = Game.goto_scene("res://scenes/replays_screen.tscn")
	var w := 0
	while w < 400 and (scene.get("idle") == false or scene.get("_busy") == true):
		await get_tree().process_frame
		w += 1
	(
		assert_bool(scene._names.size() >= 2)
		. override_failure_message("repository replays not listed (%d)" % scene._names.size())
		. is_true()
	)
	var mark: int = Sfx.played.size()
	scene.handle_action(TyrianControls.DOWN)
	scene.handle_action(TyrianControls.UP)
	Capture._options.erase("replay-dir")
	assert_int(_count_cursor(mark)).override_failure_message("replays: arrows silent").is_equal(2)


## Weapon preview in HD mode (×4 viewport): same rhythm as the C, one turn per
## 3 setDelay periods, measured in real time over two seconds.
func test_weapon_preview_rate_in_hd_mode() -> void:
	if not DisplaySettings.hd_available():
		return
	var previous := DisplaySettings.art_mode()
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	Effects.apply_display_settings()
	Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 40000])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.set_shop(r, 0)
	Game.debrief = {"cube_max": 0}
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	await _wait_shop_idle(shop)
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	await get_tree().process_frame
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	await get_tree().process_frame
	var count := {"n": 0}
	var original: Callable = shop._anim_step
	shop._anim_step = func() -> void:
		count.n += 1
		original.call()
	var t0 := Time.get_ticks_msec()
	while Time.get_ticks_msec() - t0 < 2000:
		await get_tree().process_frame
	var expected: float = 2.0 / (3.0 * float(shop.DELAY_PERIOD))
	DisplaySettings.set_art_mode(previous)
	Effects.apply_display_settings()
	Game.campaign_active = false
	(
		assert_bool(absf(count.n - expected) <= expected * 0.15)
		. override_failure_message("HD preview: %d turns in 2 s, expected %.1f" % [count.n, expected])
		. is_true()
	)


## Replays screen: the "up" indicator (the "^" glyph does not exist in
## the fonts) appears as soon as the list is scrolled and disappears on return.
func test_replays_up_indicator_shows_only_when_scrolled() -> void:
	var dir := ProjectSettings.globalize_path("res://").path_join("../golden/replays").simplify_path()
	Capture._options["replay-dir"] = dir
	var scene: Node = Game.goto_scene("res://scenes/replays_screen.tscn")
	var w := 0
	while w < 400 and (scene.get("idle") == false or scene.get("_busy") == true):
		await get_tree().process_frame
		w += 1
	Capture._options.erase("replay-dir")
	(
		assert_bool(scene._names.size() > scene.ITEMS_PER_PAGE)
		. override_failure_message("not enough replays to scroll (%d)" % scene._names.size())
		. is_true()
	)
	(
		assert_int(_up_indicator_pixels(scene))
		. override_failure_message("top indicator visible without scrolling")
		. is_equal(0)
	)
	for i in scene.ITEMS_PER_PAGE:
		scene.handle_action(TyrianControls.DOWN)
	scene.menus.render()
	assert_int(scene.menus.current().top).is_greater(0)
	(
		assert_int(_up_indicator_pixels(scene))
		. override_failure_message("top indicator missing after scrolling")
		. is_greater(0)
	)
	for i in scene.ITEMS_PER_PAGE:
		scene.handle_action(TyrianControls.UP)
	scene.menus.render()
	assert_int(scene.menus.current().top).is_equal(0)
	(
		assert_int(_up_indicator_pixels(scene))
		. override_failure_message("top indicator remained after returning to the top")
		. is_equal(0)
	)


## Non-black pixels in the area of the up indicator (290,30) of the replays screen.
func _up_indicator_pixels(scene: Node) -> int:
	scene.menus.render()
	var px: PackedByteArray = scene.vga.indices(0)
	var n := 0
	for y in range(28, 36):
		for x in range(286, 298):
			if px[y * 320 + x] != 0:
				n += 1
	return n


# ---------------------------------------------------------------------------
# CUSTOM rows without action (achievements, saves): click and Left/Right
# ---------------------------------------------------------------------------


## A real left click and Left/Right on a CUSTOM row that has neither
## "activate" nor "adjust" raise no script error: nothing happens.
func test_custom_row_without_handlers_ignores_click_and_arrows() -> void:
	var ach: Node = Game.goto_scene("res://scenes/achievements_screen.tscn")
	for _i in 300:
		if ach.idle:
			break
		await get_tree().process_frame
	assert_bool(ach.idle).is_true()
	TyrianMouse.inactive = false
	var menus: MenuEngine = ach.menus
	var sel_before: int = menus.selection()
	# First row of the list: row_x = X, row_y = Y_START + 4.
	ach.handle_mouse(_left_press(ach.X + 20, ach.Y_START + 6))
	await get_tree().process_frame
	menus.handle_action(TyrianControls.LEFT)
	menus.handle_action(TyrianControls.RIGHT)
	await get_tree().process_frame
	assert_bool(menus.stack.is_empty()).is_false()
	assert_int(menus.selection()).is_greater_equal(0)
	assert_int(sel_before).is_greater_equal(0)
