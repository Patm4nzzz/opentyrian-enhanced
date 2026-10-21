# Shop > Options > Joystick / Keyboard: the C's settings screens
# (game_menu.c:2896-2903, menus 12 and 5) open in the shop, not the rewrite's
# "Controls" screen.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")

var _menu: Node


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	C.install()
	C.reset_defaults()


func after() -> void:
	C.reset_defaults()
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	C.reset_defaults()
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _open_options() -> void:
	Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 15267])
	Game.campaign_active = true
	Game.episode = 1
	Game.goto_scene(Game.SCENE_SAVE_MENU)
	_menu = await _wait_scene(Game.SCENE_SAVE_MENU)
	assert_object(_menu).is_not_null()
	await get_tree().process_frame


func _wait_scene(path: String) -> Node:
	var t0 := Time.get_ticks_msec()
	while Time.get_ticks_msec() - t0 < 15000:
		var cur: Node = Effects.current_game_scene()
		if cur != null and cur.scene_file_path == path and cur.is_inside_tree():
			return cur
		await get_tree().process_frame
	return null


func _press_row(row: int) -> void:
	_menu.menus.current().sel = row
	_menu.handle_action(C.ACCEPT)
	await get_tree().process_frame


# Options : Load, Save, Music, Sound, Joystick (4), Keyboard (5), Mouse, Done.
func test_options_keyboard_opens_the_shop_key_config() -> void:
	await _open_options()
	await _press_row(5)
	assert_str(Effects.current_game_scene().scene_file_path).is_equal(Game.SCENE_SAVE_MENU)
	var m: Dictionary = _menu.menus.current()
	assert_str(m.header).is_equal(TyrianText.L("keyName_0"))
	assert_int(m.items.size()).is_equal(10)


func test_options_joystick_opens_the_shop_joystick_config() -> void:
	await _open_options()
	await _press_row(4)
	assert_str(Effects.current_game_scene().scene_file_path).is_equal(Game.SCENE_SAVE_MENU)
	var m: Dictionary = _menu.menus.current()
	assert_str(m.header).is_equal(TyrianText.L("menuInt_13_0"))
	assert_int(m.items.size()).is_equal(16)


func test_key_config_reset_restores_default_keys() -> void:
	await _open_options()
	await _press_row(5)
	C.rebind_key(1, "fire", KEY_Z)
	assert_str(_menu._key_text(4)).is_equal(C.key_name(KEY_Z))
	await _press_row(_menu.KEY_ROW_RESET)
	assert_str(_menu._key_text(4)).is_equal(C.key_name(KEY_SPACE))


func test_key_config_done_returns_to_options() -> void:
	await _open_options()
	await _press_row(5)
	await _press_row(_menu.KEY_ROW_DONE)
	assert_int(_menu.menus.stack.size()).is_equal(1)
	assert_str(_menu.menus.current().header).is_equal(TyrianText.L("opt_options"))
