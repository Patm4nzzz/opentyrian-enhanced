# Interface defects from the player audits: New Game's clickable area, Esc from the
# Mods menu, click in the Jukebox, demo timer, level name of a
# replay, debriefing background.
extends GdUnitTestSuite

const SETUP := preload("res://scripts/setup.gd")


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.inactive = true


func after_test() -> void:
	Game.debrief = {}
	Game.debrief_frozen = false
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.inactive = true


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


func test_new_game_row_is_hit_only_on_its_text() -> void:
	var ng_script := load("res://scripts/new_game.gd")
	ng_script.flow = ng_script.Flow.NEW_GAME
	var ng: Node = Game.goto_scene(Game.SCENE_NEW_GAME)
	for i in 120:
		await get_tree().process_frame
		if ng.current_screen == "gameplay" and not ng.menus.stack.is_empty():
			break
	assert_str(ng.current_screen).is_equal("gameplay")
	var m: Dictionary = ng.menus.current()
	var label: String = m.items[0].label
	var w := TyrianText.width(label, TyrianScreen.NORMAL_FONT)
	# menus.c:gameplaySelect: first row at y = 50, height 13, centered.
	assert_int(ng.menus.row_at(m, Vector2i(160, 55))).is_equal(0)
	assert_int(ng.menus.row_at(m, Vector2i(160 - w / 2 - 6, 55))).is_equal(-1)
	assert_int(ng.menus.row_at(m, Vector2i(160 + w / 2 + 6, 55))).is_equal(-1)
	assert_int(ng.menus.row_at(m, Vector2i(160, 50 + 13))).is_equal(-1)


func test_mods_menu_leave_returns_to_setup() -> void:
	var mods: Node = Game.goto_scene(SETUP.SCENE_MODS)
	await get_tree().process_frame
	mods.leave()
	assert_int(SETUP.entry_row).is_equal(5)
	var scene: Node = Effects.current_game_scene()
	assert_str(str(scene.get_script().resource_path)).is_equal("res://scripts/setup.gd")
	SETUP.entry_row = 0


func test_jukebox_mouse_click_quits() -> void:
	var jb: Node = Game.goto_scene(SETUP.SCENE_JUKEBOX)
	await get_tree().process_frame
	assert_bool(jb.get("_trigger_quit")).is_false()
	jb.handle_mouse(_motion(100, 100))
	assert_bool(jb.get("_trigger_quit")).is_false()
	jb.handle_mouse(_left_press(100, 100))
	assert_bool(jb.get("_trigger_quit")).is_true()


func test_title_mouse_motion_resets_demo_idle_timer() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	await get_tree().process_frame
	title.set("_idle_ms", 20000.0)
	title.handle_mouse(_motion(10, 10))
	assert_float(title.get("_idle_ms")).is_equal(0.0)
	title.set("_idle_ms", 20000.0)
	title.handle_mouse(_left_press(10, 10))
	assert_float(title.get("_idle_ms")).is_equal(0.0)


func test_replay_level_name_skips_the_shop_section() -> void:
	# A TYRIAN level replay records mainLevel = 2 (the shop that
	# precedes it): the name must come from the first level that follows.
	assert_str(Game.replay_level_name(1, 2, 2)).is_equal("TYRIAN")
	assert_str(Game.replay_level_name(1, 3, 2)).is_equal("TYRIAN")


func test_debrief_keeps_frozen_level_image_and_uses_the_big_font() -> void:
	var debrief_script := load("res://scripts/debrief.gd")
	assert_int(debrief_script.GLOW_FONT).is_equal(TyrianScreen.NORMAL_FONT)
	Game.campaign_active = false
	Game.debrief = {
		"all_players_alive": true,
		"level_name": "TEST",
		"cash": 100,
		"total_enemy": 10,
		"enemy_killed": 5,
		"cube_max": 0
	}
	Game.debrief_frozen = true
	var screen: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	# Frozen image set before the scene entry: the debriefing does not replace it with black.
	var marker := PackedByteArray()
	marker.resize(320 * 200)
	marker.fill(7)
	screen.vga.set_indices(TyrianScreen.VGA_SCREEN, marker)
	await get_tree().process_frame
	await get_tree().process_frame
	assert_bool(Game.debrief_frozen).is_false()
	var seen: PackedByteArray = screen.vga.indices(TyrianScreen.VGA_SCREEN)
	assert_int(seen[320 * 190 + 300]).is_equal(7)
