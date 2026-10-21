## Mouse on the title modes' Game Menu (arcade 1 / 2 players, Super
## Tyrian): hover, click and wheel, like game_menu.c:JE_itemScreen.
## Regression: handle_mouse sent everything to the base screen when _shop is null.
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.inactive = true
	Game.campaign_modes = PackedInt32Array()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.inactive = true
	Game.campaign_modes = PackedInt32Array()


static func _motion(x: float, y: float) -> InputEventMouseMotion:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	ev.relative = Vector2(1, 0)
	return ev


static func _button(x: float, y: float, button: int) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = button
	ev.pressed = true
	return ev


## menu: 9 (2 players), 10 (1 player), 13 (Super Tyrian).
func _open_arcade(menu: int) -> Node:
	Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 15267])
	Game.campaign_active = true
	Game.episode = 1
	var modes := PackedInt32Array([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
	modes[{9: 0, 10: 2, 13: 4}[menu]] = 1
	Game.campaign_modes = modes
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	await get_tree().process_frame
	await get_tree().process_frame
	return shop


## Row i (0 = Start Level): y = 38 + 16 * i + 4 (x in the column 171..307).
static func _row_y(i: int, menu: int) -> int:
	var gap := 0
	if menu == 9:
		gap = (16 if i + 2 > 3 else 0) + (16 if i + 2 > 4 else 0)
	return 38 + 16 * i + gap + 4


func test_arcade_hover_selects_row() -> void:
	for menu in [10, 9, 13]:
		var shop: Node = await _open_arcade(menu)
		assert_int(int(shop.get("_arcade_menu"))).is_equal(menu)
		TyrianMouse.inactive = false
		shop.handle_mouse(_motion(220, _row_y(1, menu)))
		(
			assert_int(int(shop.get("_arcade_sel")))
			. override_failure_message("menu %d: hovering does not select row 1" % menu)
			. is_equal(3)
		)
		assert_int(shop.menus.selection()).is_equal(1)
		shop.handle_mouse(_motion(220, _row_y(0, menu)))
		assert_int(int(shop.get("_arcade_sel"))).is_equal(2)


func test_arcade_click_other_row_selects_without_acting() -> void:
	var shop: Node = await _open_arcade(10)
	TyrianMouse.inactive = false
	shop.handle_mouse(_button(220, _row_y(2, 10), MOUSE_BUTTON_LEFT))
	assert_int(int(shop.get("_arcade_sel"))).override_failure_message("the click does not select Quit Game").is_equal(4)
	assert_bool(shop.modal_active).is_false()


func test_arcade_click_selected_row_acts() -> void:
	var shop: Node = await _open_arcade(10)
	TyrianMouse.inactive = false
	# Row already chosen: the click runs JE_menuFunction (here Quit Game: Yes / No box).
	shop.handle_mouse(_motion(220, _row_y(2, 10)))
	shop.handle_mouse(_button(220, _row_y(2, 10), MOUSE_BUTTON_LEFT))
	await get_tree().process_frame
	assert_bool(shop.modal_active).override_failure_message("clicking the selected row does nothing").is_true()


func test_arcade_wheel_does_nothing() -> void:
	var shop: Node = await _open_arcade(10)
	TyrianMouse.inactive = false
	shop.handle_mouse(_button(220, 100, MOUSE_BUTTON_WHEEL_DOWN))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(2)
	shop.handle_mouse(_button(220, 100, MOUSE_BUTTON_WHEEL_UP))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(2)


func test_arcade_click_outside_column_does_nothing() -> void:
	var shop: Node = await _open_arcade(10)
	TyrianMouse.inactive = false
	shop.handle_mouse(_button(100, _row_y(1, 10), MOUSE_BUTTON_LEFT))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(2)


## Arcade 2 players: every drawn row can be hovered (game_menu.c:126-160).
## Regression: hovering "Quit Game" chose "Player 2 Input", hovering
## "Options" chose "Player 1 Input" (engine rank without the blanks, then remapped).
func test_arcade_2p_hover_every_drawn_row() -> void:
	var shop: Node = await _open_arcade(9)
	TyrianMouse.inactive = false
	shop.handle_mouse(_motion(220, _row_y(1, 9)))
	for i in 5:
		shop.handle_mouse(_motion(220, _row_y(i, 9)))
		(
			assert_int(int(shop.get("_arcade_sel")))
			. override_failure_message(
				"row %d hovered (y %d): selection %d" % [i, _row_y(i, 9), int(shop.get("_arcade_sel"))]
			)
			. is_equal(i + 2)
		)
	# Device rows under Player 1 / Player 2 Input: the C attaches them
	# to the command above ("selection > 3 -> selection--").
	shop.handle_mouse(_motion(220, 38 + 16 * 2 + 4))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(3)
	shop.handle_mouse(_motion(220, 38 + 16 * 4 + 4))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(4)
	# Below Quit Game (raw rank 9 -> 7 > menuChoices 6): nothing changes.
	shop.handle_mouse(_motion(220, 38 + 16 * 7 + 4))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(4)


func test_arcade_2p_click_quit_twice_opens_quit_box() -> void:
	var shop: Node = await _open_arcade(9)
	TyrianMouse.inactive = false
	shop.handle_mouse(_button(220, _row_y(4, 9), MOUSE_BUTTON_LEFT))
	assert_int(int(shop.get("_arcade_sel"))).is_equal(6)
	shop.handle_mouse(_button(220, _row_y(4, 9), MOUSE_BUTTON_LEFT))
	await get_tree().process_frame
	assert_bool(shop.modal_active).override_failure_message("two clicks on Quit Game do not open the dialog").is_true()


## Super Tyrian / Super Arcade: names of the weapon and special weapon in color 12
## (green), like the C: helptext.c:220 resets helpBoxColor to 12 after the ship
## name (game_menu.c:1084-1093). Regression: color 15 (orange).
func test_super_menu_weapon_names_in_colour_12() -> void:
	var shop: Node = await _open_arcade(13)
	await get_tree().process_frame
	var px: PackedByteArray = shop.vga.indices(0)
	var bank := {}
	for y in range(60, 68):
		for x in range(25, 145):
			var v := int(px[y * 320 + x])
			if v != 0:
				bank[v >> 4] = int(bank.get(v >> 4, 0)) + 1
	(
		assert_int(int(bank.get(12, 0)))
		. override_failure_message("weapon name missing in colour 12: %s" % str(bank))
		. is_greater(20)
	)
	assert_int(int(bank.get(15, 0))).override_failure_message("weapon name in colour 15: %s" % str(bank)).is_equal(0)


## game_menu.c:3096-3108 (joysticks == 0): Enter on "Player 1 Input"
## swaps keyboard and mouse between the two players; the screen shows it
## (game_menu.c:1193-1209) and the next level receives it (modes[8] / [9]).
func test_two_player_input_rows_swap_keyboard_and_mouse() -> void:
	var shop: Node = await _open_arcade(9)
	Game.campaign_modes = PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2])
	Game.modes = Game.campaign_modes.duplicate()
	assert_array(Array(shop.arcade_input_devices())).is_equal([1, 2])
	shop._arcade_sel = 3
	shop._arcade_function()
	assert_array(Array(shop.arcade_input_devices())).is_equal([2, 1])
	assert_int(int(Game.modes[8])).is_equal(2)
	assert_int(int(Game.modes[9])).is_equal(1)
	shop._arcade_sel = 4
	shop._arcade_function()
	assert_array(Array(shop.arcade_input_devices())).is_equal([1, 2])
	Game.modes = PackedInt32Array()


## game_menu.c:1927-1944 and 2025-2036: Left / Right on "Player 1/2 Input"
## also swap the devices; elsewhere they do nothing.
func test_two_player_input_rows_left_right_swap() -> void:
	var shop: Node = await _open_arcade(9)
	Game.campaign_modes = PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2])
	shop._arcade_sel = 2
	shop._arcade_action(TyrianControls.LEFT)
	assert_array(Array(shop.arcade_input_devices())).is_equal([1, 2])
	shop._arcade_sel = 3
	shop._arcade_action(TyrianControls.LEFT)
	assert_array(Array(shop.arcade_input_devices())).is_equal([2, 1])
	shop._arcade_sel = 4
	shop._arcade_action(TyrianControls.RIGHT)
	assert_array(Array(shop.arcade_input_devices())).is_equal([1, 2])


## game_menu.c:2813: JE_menuFunction plays S_CLICK on every confirmation, also in
## the Arcade / Super Tyrian menus (Enter, Space); "Player 1/2 Input" adds
## S_CURSOR (game_menu.c:3097).
func test_arcade_menu_accept_plays_click_like_c() -> void:
	var shop: Node = await _open_arcade(9)
	Game.campaign_modes = PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2])
	shop._arcade_sel = 3
	var mark: int = Sfx.played.size()
	shop._arcade_action(TyrianControls.ACCEPT)
	var sounds: Array = Sfx.played.slice(mark)
	assert_array(sounds).contains([Sfx.CLICK, Sfx.CURSOR])
	assert_array(sounds).not_contains([Sfx.SELECT])


## Left column (x 0..150, y 20..145) of the current screen.
static func _left_column(screen: Node) -> PackedByteArray:
	var px: PackedByteArray = screen.vga.indices(screen.VGA_SCREEN)
	var out := PackedByteArray()
	for y in range(20, 146):
		out.append_array(px.slice(y * 320, y * 320 + 151))
	return out


## game_menu.c:1065-1100: Options / Load / Save of a Super Tyrian,
## Super Arcade or two-player game show the same left column as the mode's
## menu (ship-weapon-special weapon panel, both players' money), not
## the ship illustration of the full game.
func test_options_left_column_matches_mode_menu() -> void:
	for menu in [13, 9]:
		var shop: Node = await _open_arcade(menu)
		var expected := _left_column(shop)
		var options: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
		await get_tree().process_frame
		await get_tree().process_frame
		(
			assert_bool(_left_column(options) == expected)
			. override_failure_message("menu %d: Options left column differs from the mode menu" % menu)
			. is_true()
		)
		after_test()
		await get_tree().process_frame
