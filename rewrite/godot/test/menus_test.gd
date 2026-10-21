# Screens moved to the common menu engine (ADR 0121):
# side modes, mods, shop (categories with weapon demonstration and
# "Next Level" rendered in 8 bits by MenuEngine).
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()


func after() -> void:
	Game.mutators = 0
	Game.anomalies = 0
	Game.campaign_active = false
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func test_modes_menu_navigation() -> void:
	Game.mutators = 0
	Game.anomalies = 0
	var menu: Node = Game.goto_scene(Game.SCENE_MODES)
	assert_object(menu.menus).is_not_null()
	assert_int(menu.menus.current().items.size()).is_equal(1 + menu.MUTATORS.size() + menu.ANOMALIES.size())
	menu.handle_action(TyrianControls.RIGHT)
	assert_int(menu.mode_index).is_equal(1)
	menu.handle_action(TyrianControls.LEFT)
	menu.handle_action(TyrianControls.LEFT)
	assert_int(menu.mode_index).is_equal(menu.MODES.size() - 1)
	menu.handle_action(TyrianControls.SELECT)  # Space = Right
	assert_int(menu.mode_index).is_equal(0)
	menu.handle_action(TyrianControls.DOWN)
	menu.handle_action(TyrianControls.DOWN)
	assert_int(menu.row()).is_equal(2)
	menu.handle_action(TyrianControls.SELECT)
	assert_int(Game.mutators).is_equal(1 << 1)
	menu.handle_action(TyrianControls.RIGHT)
	assert_int(Game.mutators).is_equal(0)
	# Up from row 0: last anomaly (engine wraparound).
	for i in 3:
		menu.handle_action(TyrianControls.UP)
	assert_int(menu.row()).is_equal(menu.menus.current().items.size() - 1)
	menu.handle_action(TyrianControls.LEFT)
	assert_int(Game.anomalies).is_equal(1 << (menu.ANOMALIES.size() - 1))
	Game.anomalies = 0


func test_mods_menu_on_engine() -> void:
	var menu: Node = Game.goto_scene("res://scenes/mods.tscn")
	assert_object(menu.menus).is_not_null()
	# One row per mod, then "Open the mods folder" and "Install".
	var rows: int = menu.mods.size() + menu.ACTION_ROWS.size()
	assert_int(menu.menus.current().items.size()).is_equal(rows)
	menu.handle_action(TyrianControls.DOWN)
	assert_int(menu.index).is_equal(1)
	# Up / Down bounded, no wraparound (mod_manager.c:1691-1702).
	menu.handle_action(TyrianControls.UP)
	menu.handle_action(TyrianControls.UP)
	assert_int(menu.index).is_equal(0)
	for i in rows + 2:
		menu.handle_action(TyrianControls.DOWN)
	assert_int(menu.index).is_equal(rows - 1)
	# Esc: back to the Setup menu (setup_menu.c:action_mods).
	menu.handle_action(TyrianControls.BACK)
	assert_str(Effects.current_game_scene().scene_file_path).is_equal(Game.SCENE_SETUP)


## Shop of section 4 of episode 1 (same state as shop-1/shop-2 of
## ui_regression.sh): each menu of the session is rendered by MenuEngine.
func _open_shop() -> Node:
	Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 15267])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.shop_section = int(r.get("section", 4))
	Game.shop_items_available = r.get("items_available", [])
	var m: PackedInt32Array = r.get("map", PackedInt32Array())
	Game.map_sections = PackedInt32Array()
	Game.map_planets = PackedInt32Array()
	for i in range(1, m.size(), 2):
		Game.map_planets.append(m[i - 1])
		Game.map_sections.append(m[i])
	Game.debrief = {"cube_max": 0}
	return Game.goto_scene(Game.SCENE_SHOP)


func test_shop_weapon_category_and_next_level() -> void:
	var shop: Node = _open_shop()
	assert_object(shop.get("_shop")).override_failure_message("no TyrianShop session").is_not_null()
	var before: PackedByteArray = shop.vga.indices(0)
	# Upgrade Ship (sel 4), then Front Gun (sel 3): category with demonstration.
	for a in [
		TyrianControls.DOWN, TyrianControls.DOWN, TyrianControls.ACCEPT, TyrianControls.DOWN, TyrianControls.ACCEPT
	]:
		shop.handle_action(a)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_UPGRADE_SUB)
	assert_int(int(shop._state["category"])).is_equal(3)
	assert_int(shop.menus.current().options.style).is_equal(MenuEngine.Style.ROWS)
	assert_bool(shop._state.has("upgrade_cost")).is_true()
	assert_bool(shop.idle).is_true()
	# Demonstration area emptied (JE_weaponViewFrame: x 8..143, y 8..182).
	assert_int(shop.vga.get_pixel(0, 70, 60)).is_equal(0)
	assert_bool(shop.vga.indices(0) != before).is_true()
	# Back to the root, then "Start Level" (sel 6): Next Level.
	shop.handle_action(TyrianControls.BACK)
	shop.handle_action(TyrianControls.BACK)
	assert_int(int(shop._state["menu"])).is_equal(0)
	for a in [TyrianControls.DOWN, TyrianControls.DOWN, TyrianControls.ACCEPT]:
		shop.handle_action(a)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_PLAY_NEXT_LEVEL)
	var items: Array = shop.menus.current().items
	assert_int(items.size()).is_equal(Game.map_sections.size() + 1)
	assert_str(items[0].label).is_equal(TyrianText.L("planet_%d" % (Game.map_planets[0] - 1)))
	assert_str(items[-1].label).is_equal(TyrianText.L("miscText_5"))
	assert_int(shop.menus.current().options.style).is_equal(MenuEngine.Style.GAME_MENU)
	Game.campaign_active = false


## Shop with two data cubes picked up (same state as the
## cubelist / cuberead capture compared with the C): list, reading, held scrolling,
## back; Ship Specs; "quit" box refused.
func test_shop_data_cubes_ship_specs_and_quit_box() -> void:
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
	Game.debrief = {"cube_max": 2}
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	assert_object(shop.get("_shop")).override_failure_message("no TyrianShop session").is_not_null()
	# Data (sel 2): two cubes + Exit, rendered as rows by MenuEngine.
	shop.handle_action(TyrianControls.ACCEPT)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBES)
	assert_int(shop.menus.current().items.size()).is_equal(3)
	assert_int(shop.menus.current().options.style).is_equal(MenuEngine.Style.ROWS)
	# Reading the first cube: non-empty text, scroll at 0.
	shop.handle_action(TyrianControls.ACCEPT)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBE_SUB)
	var lines: PackedStringArray = shop._shop.cube_text(0)
	assert_bool(lines.size() > 10).is_true()
	assert_int(shop._cube_y).is_equal(0)
	# Up at the start of the text: nothing; Down held: one pixel per setDelay(1).
	shop._scroll_cube(-1)
	assert_int(shop._cube_y).is_equal(0)
	Input.action_press(TyrianControls.DOWN)
	shop._process(shop.DELAY_PERIOD * 3.5)
	Input.action_release(TyrianControls.DOWN)
	assert_int(shop._cube_y).is_equal(3)
	# Released, the text finishes its 12 px line then stops (game_menu.c:1328,
	# yChg reset to 0 only when yLoc % 12 == 0): a short press = one line.
	shop._process(shop.DELAY_PERIOD * 4)
	assert_int(shop._cube_y).is_equal(7)
	for i in 20:
		shop._process(shop.DELAY_PERIOD)
	assert_int(shop._cube_y).is_equal(12)
	# Short Up: goes back up a whole line.
	Input.action_press(TyrianControls.UP)
	shop._process(shop.DELAY_PERIOD * 1.5)
	Input.action_release(TyrianControls.UP)
	for i in 20:
		shop._process(shop.DELAY_PERIOD)
	assert_int(shop._cube_y).is_equal(0)
	# End / Home (game_menu.c:1884-1892): last page, then top of the text.
	shop.handle_event(_key(KEY_END))
	assert_int(shop._cube_y).is_equal((lines.size() - 9) * shop.CUBE_LINE_H)
	assert_int(shop.cube_read_percent(lines.size())).is_equal(100)
	shop.handle_event(_key(KEY_HOME))
	assert_int(shop._cube_y).is_equal(0)
	# Esc: back to the list, shop palette restored at the root.
	shop.handle_action(TyrianControls.BACK)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBES)
	shop.handle_action(TyrianControls.BACK)
	assert_int(int(shop._state["menu"])).is_equal(0)
	assert_bool(shop._face_palette).is_false()
	# Ship Specs (sel 3): screen open until a key.
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	assert_bool(shop._specs_open).is_true()
	# JE_scaleInPicture: a key shortens the enlargement, a second one
	# (wait_input) closes the screen.
	shop.handle_action(TyrianControls.ACCEPT)
	await get_tree().process_frame
	assert_bool(shop._specs_open).is_true()
	shop.handle_action(TyrianControls.ACCEPT)
	await _wait_until(func() -> bool: return not shop._specs_open)
	assert_bool(shop._specs_open).is_false()
	assert_int(int(shop._state["menu"])).is_equal(0)
	# Esc at the root: "quit" box, No (default) keeps the shop.
	shop.handle_action(TyrianControls.BACK)
	await _wait_until(func() -> bool: return shop.modal_active)
	assert_bool(shop.modal_active).is_true()
	shop.handle_action(TyrianControls.ACCEPT)
	await _wait_until(func() -> bool: return not shop.modal_active)
	assert_bool(shop.modal_active).is_false()
	assert_str(Effects.current_game_scene().scene_file_path).is_equal(Game.SCENE_SHOP)
	Game.campaign_active = false
	Game.debrief = {}
	# Process shared between suites: cubes reset to zero.
	Game.shop_cube_list = PackedInt32Array()
	Game.shop_cube_max = 0


## Waits for a condition frame by frame: the duration of a frame varies with the
## machine load, a fixed 50 ms delay made the test unstable.
func _wait_until(cond: Callable, max_ms: int = 3000) -> void:
	var end := Time.get_ticks_msec() + max_ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame


func _key(code: Key) -> InputEventKey:
	var e := InputEventKey.new()
	e.physical_keycode = code
	e.keycode = code
	e.pressed = true
	return e
