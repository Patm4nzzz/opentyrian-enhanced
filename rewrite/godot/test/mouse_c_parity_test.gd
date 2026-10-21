## Mouse like the C (menus track): reading a cube with the mouse
## alone (game_menu.c:1262-1278, 1529-1532, 1597-1604), screens without a mouse in
## the C (achievements, replays, destruct), key capture that ignores the mouse.
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.reset()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.reset()


static func _motion(x: float, y: float) -> InputEventMouseMotion:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	ev.relative = Vector2(1, 0)
	TyrianMouse.note(ev)  # Effects feeds it before the screen
	return ev


static func _button(x: float, y: float, button: int, pressed := true) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = button
	ev.pressed = pressed
	TyrianMouse.note(ev)
	return ev


## Shop with two cubes, first cube open (same state as menus_test).
func _open_cube_reading() -> Node:
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
	shop.handle_action(TyrianControls.ACCEPT)
	shop.handle_action(TyrianControls.ACCEPT)
	return shop


## M07: button held in the lower half of the text = scroll down.
func test_cube_reading_scrolls_with_mouse_held() -> void:
	var shop: Node = _open_cube_reading()
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBE_SUB)
	shop.handle_mouse(_motion(230, 130))
	assert_int(shop.mouse_pointer).is_equal(MouseCursor.DOWN)
	shop.handle_mouse(_button(230, 130, MOUSE_BUTTON_LEFT))
	shop._process(shop.DELAY_PERIOD * 3.5)
	assert_int(shop._cube_y).is_equal(3)
	shop.handle_mouse(_button(230, 130, MOUSE_BUTTON_LEFT, false))
	# Released: like on the keyboard, yChg stays set until the end of the
	# 12 px line (game_menu.c:1529-1535 then 1328).
	shop._process(shop.DELAY_PERIOD * 4)
	assert_int(shop._cube_y).is_equal(7)
	for i in 20:
		shop._process(shop.DELAY_PERIOD)
	assert_int(shop._cube_y).is_equal(12)


## M07: upper half = up-arrow pointer; outside the text = normal pointer.
func test_cube_reading_pointer_up_and_normal() -> void:
	var shop: Node = _open_cube_reading()
	shop.handle_mouse(_motion(230, 60))
	assert_int(shop.mouse_pointer).is_equal(MouseCursor.UP)
	shop.handle_mouse(_motion(100, 60))
	assert_int(shop.mouse_pointer).is_equal(MouseCursor.NORMAL)


## M07: click on EXIT (258 < x < 290, 159 < y < 171) goes back to the list.
func test_cube_reading_exit_click_returns_to_list() -> void:
	var shop: Node = _open_cube_reading()
	shop.handle_mouse(_button(270, 165, MOUSE_BUTTON_LEFT))
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_DATA_CUBES)


## M01: while waiting for a key, hover and left click are ignored; the
## right click cancels like Esc (the maintainer's decision, 2026-10-12).
func test_key_bindings_capture_ignores_mouse_except_right_click_cancel() -> void:
	var scene: Node = Game.goto_scene(ControlsOptions.SCENE_PATH)
	await get_tree().process_frame
	scene.waiting = true
	var before_sel: int = scene.selected
	scene._unhandled_input(_motion(100, 60))
	scene._unhandled_input(_button(100, 60, MOUSE_BUTTON_LEFT))
	assert_bool(scene.waiting).is_true()
	assert_int(scene.selected).is_equal(before_sel)
	scene._unhandled_input(_button(100, 60, MOUSE_BUTTON_RIGHT))
	assert_bool(scene.waiting).is_false()
	assert_int(scene.selected).is_equal(before_sel)


## M-A: Achievements without a mouse, like the C (no cursor, no hover).
func test_achievements_screen_ignores_mouse() -> void:
	var scene: Node = Game.goto_scene("res://scenes/achievements_screen.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	assert_bool(scene.mouse_cursor).is_false()
	var before_sel: int = scene.cursor
	TyrianMouse.inactive = false
	scene.handle_mouse(_motion(100, 100))
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_LEFT))
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_RIGHT))
	assert_int(scene.cursor).is_equal(before_sel)


## Divergence 54 (maintainer, 2026-10-10): the wheel scrolls the
## achievements list one row per notch, like Up / Down; a click still does nothing.
func test_achievements_screen_wheel_scrolls() -> void:
	var scene: Node = Game.goto_scene("res://scenes/achievements_screen.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	scene.idle = true
	scene._busy = false
	var start: int = scene.cursor
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_WHEEL_DOWN))
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_WHEEL_DOWN))
	assert_int(scene.cursor).is_equal(start + 2)
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_WHEEL_UP))
	assert_int(scene.cursor).is_equal(start + 1)
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_WHEEL_DOWN, false))
	assert_int(scene.cursor).is_equal(start + 1)


## M-B: Replays without a mouse, like the C.
func test_replays_screen_has_no_mouse() -> void:
	var scene: Node = Game.goto_scene("res://scenes/replays_screen.tscn")
	await get_tree().process_frame
	await get_tree().process_frame
	assert_bool(scene.mouse_cursor).is_false()
	scene.handle_mouse(_button(100, 100, MOUSE_BUTTON_RIGHT))
	assert_object(Effects.current_game_scene()).is_same(scene)


## M13: one wheel notch does not answer Yes in the pause quit box.
func test_wheel_never_counts_as_click() -> void:
	assert_bool(TyrianMouse.is_button_press(_button(100, 100, MOUSE_BUTTON_WHEEL_DOWN))).is_false()


## M12: Save/Load, y 28..37 already targets the first slot; x = 170 is refused.
func test_save_slots_hit_box_matches_c() -> void:
	var menus := MenuEngine.new(null)
	var m := {
		"items": [{}, {}, {}],
		"top": 0,
		"options":
		{
			"style": MenuEngine.Style.ROWS,
			"row_x": 0,
			"row_y": 38,
			"row_step": 11,
			"hit_x": 171,
			"hit_w": 137,
			"hit_row_y": 38,
			"hit_y0": 21,
			"row_h": 11
		}
	}
	assert_int(menus.row_at(m, Vector2i(200, 30))).is_equal(0)
	assert_int(menus.row_at(m, Vector2i(170, 45))).is_equal(-1)
	assert_int(menus.row_at(m, Vector2i(171, 45))).is_equal(0)


## M04: hover over a MENU_TREE menu (Setup) = S_CLICK (menu_tree.c:536-538).
func test_menu_tree_hover_sound_is_click() -> void:
	var menus := MenuEngine.new(null)
	var m := {"options": {}}
	assert_int(menus._default_hover_sfx(m)).is_equal(Sfx.CLICK)


## M11: 2-player arcade Game Menu, row remapping (game_menu.c:142-148).
func test_arcade_two_player_row_remap() -> void:
	var shop_script: GDScript = load("res://scripts/shop.gd")
	assert_int(shop_script._arcade_row_2p(3)).is_equal(3)
	assert_int(shop_script._arcade_row_2p(4)).is_equal(3)
	assert_int(shop_script._arcade_row_2p(5)).is_equal(4)
	assert_int(shop_script._arcade_row_2p(6)).is_equal(4)
	assert_int(shop_script._arcade_row_2p(7)).is_equal(5)


## D2/D3: the pointer jump caused by the relative-mode change is neither a
## hover nor a "real movement" (menus.c:115-127); a real movement after
## the tolerance window is.
func test_synthetic_motion_does_not_wake_the_mouse() -> void:
	TyrianMouse.reset()
	TyrianMouse.expect_synthetic_motion()
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(100, 100)
	ev.relative = Vector2(5, 0)
	assert_bool(TyrianMouse.is_synthetic_motion(ev)).is_true()
	TyrianMouse.synthetic_until_ms = 0
	assert_bool(TyrianMouse.is_synthetic_motion(ev)).is_false()


## M05: without a header, MENU_TREE rows target y_start = 30, not 36.
func test_menu_tree_without_header_uses_real_rows() -> void:
	var menus := MenuEngine.new(null)
	var items := [
		{"kind": MenuEngine.Kind.ACTION, "label": "a", "help": ""},
		{"kind": MenuEngine.Kind.ACTION, "label": "b", "help": ""}
	]
	var m := {"header": "", "items": items, "top": 0, "options": {"style": MenuEngine.Style.MENU_TREE}}
	assert_int(menus.row_at(m, Vector2i(40, 30))).is_equal(0)
	assert_int(menus.row_at(m, Vector2i(40, 28))).is_equal(0)  # y - 2
	assert_int(menus.row_at(m, Vector2i(40, 45))).is_equal(1)
	m.header = "h"
	assert_int(menus.row_at(m, Vector2i(40, 30))).is_equal(-1)  # 36 - 2 = 34 > 30
	assert_int(menus.row_at(m, Vector2i(40, 36))).is_equal(0)


## M10: mouse button settings (keyboard.c:mouseSettings).
func test_mouse_button_functions_follow_settings() -> void:
	var level_script: GDScript = load("res://scripts/level.gd")
	var saved: Array = SetupScreen.settings["mouse_buttons"].duplicate()
	SetupScreen.settings["mouse_buttons"] = [1, 4, 5]
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_LEFT)).is_equal(Game.MASK_FIRE)
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_RIGHT)).is_equal(
		Game.MASK_LEFT_SIDEKICK | Game.MASK_RIGHT_SIDEKICK
	)
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_MIDDLE)).is_equal(Game.MASK_CHANGE_FIRE)
	SetupScreen.settings["mouse_buttons"] = [2, 3, 1]
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_LEFT)).is_equal(Game.MASK_LEFT_SIDEKICK)
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_RIGHT)).is_equal(Game.MASK_RIGHT_SIDEKICK)
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_MIDDLE)).is_equal(Game.MASK_FIRE)
	assert_int(level_script.mouse_button_mask(MOUSE_BUTTON_WHEEL_UP)).is_equal(0)
	SetupScreen.settings["mouse_buttons"] = saved


## M09: click on the volume bar = (x - 221) / 4 * 12, capped at 255; outside the
## bar (x < 221 or y outside 70..82 / 86..98) no effect.
func test_options_volume_bar_click_sets_volume() -> void:
	var saved_music: int = SetupScreen.settings["music_volume"]
	var saved_fx: int = SetupScreen.settings["fx_volume"]
	var scene: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await get_tree().process_frame
	scene.handle_mouse(_button(221 + 40, 75, MOUSE_BUTTON_LEFT))
	assert_int(SetupScreen.settings["music_volume"]).is_equal(10 * 12)
	scene.handle_mouse(_button(221 + 100, 90, MOUSE_BUTTON_LEFT))
	assert_int(SetupScreen.settings["fx_volume"]).is_equal(255)
	scene.handle_mouse(_button(200, 75, MOUSE_BUTTON_LEFT))
	assert_int(SetupScreen.settings["music_volume"]).is_equal(10 * 12)
	scene.handle_mouse(_button(250, 60, MOUSE_BUTTON_LEFT))
	assert_int(SetupScreen.settings["music_volume"]).is_equal(10 * 12)
	SetupScreen.settings["music_volume"] = saved_music
	SetupScreen.settings["fx_volume"] = saved_fx
	SetupScreen.apply_settings()
