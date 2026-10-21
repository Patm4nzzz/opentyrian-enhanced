## Mouse pointer screen by screen, like the C (c-final): the C only shows the
## pointer in the images presented between JE_mouseStart (or
## JE_mouseStartFilter) and JE_mouseReplace (mouse.c:117-170). Full table and
## sources: local audit report (not versioned).
extends GdUnitTestSuite

const TitleScreen := preload("res://scripts/title.gd")
const NewGame := preload("res://scripts/new_game.gd")
const ControlsOptions := preload("res://scripts/controls_options.gd")

## Screens and expected pointer once the screen is ready (true: the C calls
## JE_mouseStart in the screen's loop).
const SCREENS := {
	# title_screen.c:572-574 (JE_mouseStartFilter(0xF0)).
	"res://scenes/title.tscn": true,
	# menus.c:111, 332, 520, 786 (gameplaySelect, episodeSelect…).
	"res://scenes/new_game.tscn": true,
	# setup_menu.c -> menu_tree.c:627-629 (run_menu).
	"res://scenes/setup.tscn": true,
	# highscore.c:222-224 (JE_highScoreScreen).
	"res://scenes/high_scores.tscn": true,
	# mainint.c:771-773 (JE_loadScreen).
	"res://scenes/load_screen.tscn": true,
	# mainint.c:286-288, 533-535 (JE_helpSystem, helpSystemPage).
	"res://scenes/instructions.tscn": true,
	# game_menu.c:1418-1428 (JE_itemScreen, menu Options).
	"res://scenes/save_menu.tscn": true,
	# keybindings_menu.c -> menu_tree.c:627-629.
	"res://scenes/controls_options.tscn": true,
	# Rewrite screens with no 8-bit equivalent: pointer kept.
	"res://scenes/modes_menu.tscn": true,
	"res://scenes/display_options.tscn": true,
	# No JE_mouseStart: achievements.c, title_screen.c:replay_browser,
	# jukebox.c, destruct.c, mod_manager.c.
	"res://scenes/achievements_screen.tscn": false,
	"res://scenes/replays_screen.tscn": false,
	"res://scenes/jukebox.tscn": false,
	"res://scenes/destruct.tscn": false,
	"res://scenes/mods.tscn": false,
}

## Screens whose `_run` requires a game state: only `_ready` is played (the
## flag is set there), the screen is freed before its deferred `_run`.
## No JE_mouseStart: mainint.c:JE_endLevelAni (debriefing), story
## and ending screens (tyrian2.c, varz.c, highscore.c outside name entry),
## endless.c:endless_on_level_end (Endless game over).
const SCREENS_READY_ONLY := {
	"res://scenes/debrief.tscn": false,
	"res://scenes/story_screens.tscn": false,
	"res://scenes/endless_end.tscn": false,
	# Rewrite addition: the anomaly draw and the resume are also played
	# with the mouse (hover, click); the C (endless.c, anomalies_ui.c) only
	# reads keys. Pointer kept to aim at the cards.
	"res://scenes/endless_prompt.tscn": true,
}

## Screens opened by the level (`new()` without a scene): pause menu
## (mainint.c:JE_inGameSetup -> menu_tree.c:627-629) and in-game help
## (mainint.c:1696-1698), both with JE_mouseStart.
const SCRIPTS_NEW := {
	"res://scripts/pause_menu.gd": true,
	"res://scripts/in_game_help.gd": true,
}


func before() -> void:
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.reset()


func before_test() -> void:
	# Mouse active, inside the image: only the screen decides.
	TyrianMouse.inactive = false
	TyrianMouse.position = Vector2i(160, 100)
	TitleScreen.intro_shown = true
	TitleScreen.move_tyrian_logo_up = false


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.reset()
	TitleScreen.intro_shown = true
	TitleScreen.move_tyrian_logo_up = false


## Pointer drawn: a visible MouseCursor in the scene.
static func _cursor_visible(scene: Node) -> bool:
	for c in scene.find_children("*", "MouseCursor", true, false):
		if c.is_visible_in_tree():
			return true
	return false


func _close(scene: Node) -> void:
	Effects.game_viewport.remove_child(scene)
	scene.queue_free()
	await get_tree().process_frame


## Bare 8-bit screen (default pointer), for the phases common to all screens.
func _bare_screen() -> TyrianScreen:
	var s := TyrianScreen.new()
	Effects.game_viewport.add_child(s)
	return s


func _wait_ready(scene: Node, frames := 240) -> void:
	for i in frames:
		await get_tree().process_frame
		if scene.get("idle") == true and scene.get("_busy") != true:
			break
	await get_tree().process_frame


## The level (and the Game Over screen, tyrian2.c) has no pointer.
func test_level_has_no_pointer() -> void:
	var scene: Node = load(Game.SCENE_LEVEL).instantiate()
	assert_bool(scene.find_children("*", "MouseCursor", true, false).is_empty()).is_true()
	scene.free()


## title_screen.c:intro_logos (1098-1125) and the logo rising (511-531):
## JE_showVGA and fades without JE_mouseStart; the pointer only appears at the menu.
func test_intro_logos_and_logo_rise_have_no_pointer() -> void:
	TitleScreen.intro_shown = false
	TitleScreen.move_tyrian_logo_up = true
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	var shown_frames := 0
	var frames := 0
	while not title.idle and frames < 2000:
		await get_tree().process_frame
		frames += 1
		if title.idle:
			break
		if title.pointer_shown():
			shown_frames += 1
		# Shortens the waits of 200 units (wait_delayorinput) like a key.
		title._input_seen = true
	assert_bool(title.idle).override_failure_message("title menu reached").is_true()
	assert_int(shown_frames).override_failure_message("frames with pointer before the menu").is_equal(0)
	await get_tree().process_frame
	assert_bool(_cursor_visible(title)).override_failure_message("pointer at the menu (title_screen.c:572)").is_true()
	assert_int(title.mouse_filter_hue).is_equal(0xF0)


## level_helpers.c:JE_whoa (ENGAGE code): 300 JE_showVGA without a pointer.
func test_whoa_has_no_pointer() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await _wait_ready(title)
	title.whoa()
	await get_tree().process_frame
	assert_bool(title.pointer_shown()).is_false()
	title._input_seen = true
	for i in 3:
		await get_tree().process_frame
	assert_bool(title.pointer_shown()).is_true()


## palette.c:141-176: each fade step is presented without a pointer, except
## the fades done between JE_mouseStart and JE_mouseReplace (shop on
## return from help, game_menu.c:1333-1342; name entry, highscore.c:475-482).
func test_fades_hide_pointer_except_over_pointer() -> void:
	var setup := _bare_screen()
	await get_tree().process_frame
	assert_bool(setup.pointer_shown()).is_true()
	# wait_units deadline reset to zero: each fade step waits.
	setup._delay_next_ms = -1.0
	setup.fade_black(5)
	await get_tree().process_frame
	assert_bool(setup.pointer_shown()).override_failure_message("fade_black").is_false()
	await _wait_pointer(setup)
	setup._delay_next_ms = -1.0
	setup.fade_palette(5)
	await get_tree().process_frame
	assert_bool(setup.pointer_shown()).override_failure_message("fade_palette").is_false()
	await _wait_pointer(setup)
	setup._delay_next_ms = -1.0
	setup.fade_palette_over_pointer(5)
	await get_tree().process_frame
	(
		assert_bool(setup.pointer_shown())
		. override_failure_message("fade between JE_mouseStart and JE_mouseReplace")
		. is_true()
	)
	await _wait_ms(300)


func _wait_ms(ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while Time.get_ticks_msec() < end:
		await get_tree().process_frame


## Waits for the end of a pointerless phase (2 s at most).
func _wait_pointer(screen: Node) -> void:
	var t0 := Time.get_ticks_msec()
	while not screen.pointer_shown() and Time.get_ticks_msec() - t0 < 2000:
		await get_tree().process_frame
	assert_bool(screen.pointer_shown()).override_failure_message("pointer back after the phase").is_true()


## game_menu.c:2583-2587, 2694-2698: the Quit box draws the pointer via
## JE_mouseStart, without the title's 0xF0 filter.
func test_quit_box_pointer_has_no_title_filter() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await _wait_ready(title)
	assert_int(title.mouse_filter_hue).is_equal(0xF0)
	var box := _bare_screen()
	await get_tree().process_frame
	box.mouse_filter_hue = 0xF0
	box.quit_request_msg("a", "b")
	await get_tree().process_frame
	assert_bool(box.pointer_shown()).is_true()
	assert_int(box.mouse_filter_hue).is_equal(0)
	box.modal_input(TyrianControls.NO)
	for i in 10:
		await get_tree().process_frame
	assert_int(box.mouse_filter_hue).is_equal(0xF0)


## title_screen.c:newSuperArcadeGame (1012-1015) and newSuperTyrianGame
## (1065-1069): JE_showVGA, fade then wait_input, without JE_mouseStart.
func test_super_arcade_and_super_tyrian_screens_have_no_pointer() -> void:
	NewGame.flow = NewGame.Flow.NEW_GAME
	var ng: Node = Game.goto_scene(Game.SCENE_NEW_GAME)
	await _wait_ready(ng)
	assert_bool(ng.pointer_shown()).is_true()
	ng.wait_input()
	await get_tree().process_frame
	assert_bool(ng.pointer_shown()).override_failure_message("waiting for a key").is_false()
	ng._input_seen = true
	await get_tree().process_frame
	await get_tree().process_frame
	assert_bool(ng.pointer_shown()).is_true()


## game_menu.c:2960-2981 (Options > Keyboard, new key): JE_showVGA without
## JE_mouseStart. Gamepad (3205): no presentation, the pointer image stays.
func test_shop_key_capture_has_no_pointer() -> void:
	var menu: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await _wait_ready(menu)
	assert_bool(menu.pointer_shown()).is_true()
	menu._capturing = true
	menu._capture_wants_pad = false
	assert_bool(menu.pointer_shown()).override_failure_message("key capture").is_false()
	menu._capture_wants_pad = true
	assert_bool(menu.pointer_shown()).override_failure_message("gamepad capture").is_true()
	menu._capturing = false


## joystick_menu.c:assign_act (110-116) and menu_tree.c:828-862
## (menu_tree_capture_scancode): prompt presented without JE_mouseStart.
func test_setup_captures_have_no_pointer() -> void:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	await _wait_ready(setup)
	setup.joystick.capture_row = 4
	await get_tree().process_frame
	assert_bool(_cursor_visible(setup)).override_failure_message("Setup gamepad capture").is_false()
	setup.joystick.capture_row = -1
	await get_tree().process_frame
	assert_bool(_cursor_visible(setup)).is_true()
	var keys: Node = Game.goto_scene(ControlsOptions.SCENE_PATH)
	await get_tree().process_frame
	assert_bool(_cursor_visible(keys)).is_true()
	keys.waiting = true
	await get_tree().process_frame
	assert_bool(_cursor_visible(keys)).override_failure_message("key capture (Key Bindings)").is_false()
	keys.waiting = false


func test_every_screen_matches_c_table() -> void:
	var wrong := []
	for path in SCREENS:
		var scene: Node = Game.goto_scene(path)
		await _wait_ready(scene)
		if _cursor_visible(scene) != SCREENS[path]:
			wrong.append("%s: %s" % [path.get_file(), "pointer shown" if not SCREENS[path] else "no pointer"])
		await _close(scene)
	for path in SCREENS_READY_ONLY:
		var scene: Node = load(path).instantiate()
		Effects.game_viewport.add_child(scene)
		if scene.pointer_shown() != SCREENS_READY_ONLY[path]:
			wrong.append("%s: pointer shown" % path.get_file())
		scene.free()
	for path in SCRIPTS_NEW:
		var screen: TyrianScreen = load(path).new()
		if screen.pointer_shown() != SCRIPTS_NEW[path]:
			wrong.append("%s: no pointer" % path.get_file())
		screen.free()
	# game_menu.c:1333-1342, 1386-1402, 1418-1428 (JE_itemScreen).
	var shop := await _open_shop()
	if not _cursor_visible(shop):
		wrong.append("shop.tscn: no pointer")
	await _close(shop)
	Game.campaign_active = false
	assert_array(wrong).override_failure_message("differences with the C: %s" % [wrong]).is_empty()


## Shop of section 4 of episode 1 (same state as menus_test).
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
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	await _wait_ready(shop)
	return shop
