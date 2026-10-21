# Pause in a level (mainint.c:JE_pauseGame) and the title's B key (Endless
# ranking, title_screen.c SDL_SCANCODE_B). Pause is not a simulation input:
# while it lasts, no tick is played and the state fingerprint does not
# move; the game then resumes exactly where it was.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const ES := preload("res://scripts/endless_session.gd")


func before() -> void:
	ES.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()


func after() -> void:
	C.reset_defaults()
	ES.stop()
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	C.reset_defaults()
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	ES.stop()
	await get_tree().process_frame
	await get_tree().process_frame


func _sim() -> Object:
	for n in Effects.game_viewport.find_children("TyrianSim", "", true, false):
		return n
	return null


## Waits bounded in real time, not in frames.
func _wait_ms(ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while Time.get_ticks_msec() < end:
		await get_tree().physics_frame


func _wait_until(cond: Callable, ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame


func _send(ev: InputEvent) -> void:
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEvent = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func _press(code: int) -> void:
	_send(C.key_event(code))


func _pad(button: int) -> void:
	var ev := InputEventJoypadButton.new()
	ev.button_index = button
	ev.pressed = true
	_send(ev)


func _start_level() -> Node:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	var sim: Object = _sim()
	await _wait_until(func() -> bool: return sim != null and sim.tick() >= 20, 5000)
	return level


## Freeze for `ms`: tick and fingerprint identical before and after.
func _assert_frozen(sim: Object, ms: int, what: String) -> void:
	var tick: int = sim.tick()
	var hash: int = sim.state_hash()
	await _wait_ms(ms)
	assert_int(sim.tick()).override_failure_message("%s: tick advanced during pause" % what).is_equal(tick)
	assert_int(sim.state_hash()).override_failure_message("%s: state modified during pause" % what).is_equal(hash)


func test_pause_key_freezes_and_resumes() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	assert_int(sim.tick()).override_failure_message("the level does not advance").is_greater_equal(20)
	_press(KEY_P)
	assert_bool(level.paused).override_failure_message("P does not pause").is_true()
	# mainint.c:JE_pauseGame: JE_dString in the 8-bit screen (level palette),
	# hence through the dashboard, not through a white RGBA label.
	assert_str(level._hud_vga.pause_text).override_failure_message("'PAUSED' missing").is_equal(
		TyrianText.L("miscText_22")
	)
	assert_bool(level._pause_label.visible).override_failure_message("extra RGBA label displayed").is_false()
	# fonthand.c:208 / sprite.c:507: the "PAUSED" shadow darkens the frozen image
	# (low nibble / 2), it is not black. Shadow pixels = opaque in the
	# overlay, outside the glyph; none must be black on a background that is not.
	var hud = level._hud_vga
	var pf: PackedByteArray = sim.playfield_indices(Game.data_dir)
	var a: PackedByteArray = hud.vga.indices(hud.OVER_A)
	var b: PackedByteArray = hud.vga.indices(hud.OVER_B)
	var x0: int = hud.PAUSE_TEXT_X + hud.PLAY_AREA_X
	var black_on_lit := 0
	var shadow := 0
	for y in range(hud.PAUSE_TEXT_Y, hud.PAUSE_TEXT_Y + 22):
		for x in range(x0, x0 + 90):
			var i := y * 320 + x
			if a[i] != b[i] or a[i] >= 0xF0:
				continue
			var under: int = pf[i]
			if a[i] == ((under & 0xF0) | ((under & 0x0F) >> 1)):
				shadow += 1
			elif a[i] == 0 and under != 0:
				black_on_lit += 1
	assert_int(shadow).override_failure_message("no darkened shadow under 'PAUSED'").is_greater(30)
	(
		assert_int(black_on_lit)
		. override_failure_message("'PAUSED' shadow black (%d px) instead of darkening the game" % black_on_lit)
		. is_equal(0)
	)
	await _assert_frozen(sim, 500, "P")
	var tick: int = sim.tick()
	# Esc resumes (any key, like JE_pauseGame) without leaving the level.
	_press(KEY_ESCAPE)
	assert_bool(level.paused).is_false()
	assert_str(level._hud_vga.pause_text).override_failure_message("'PAUSED' remained after resuming").is_empty()
	await _wait_until(func() -> bool: return sim.tick() > tick + 5, 5000)
	(
		assert_bool(is_instance_valid(level) and level.is_inside_tree())
		. override_failure_message("resume Escape left the level")
		. is_true()
	)
	assert_int(sim.tick()).override_failure_message("the game did not resume").is_greater(tick + 5)


## joystick.c:reset_joystick_assignments: button 4 (LB) = in-game menu,
## button 5 (RB) = pause; Start and Back remain.
func test_shoulder_buttons_are_menu_and_pause() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	_pad(JOY_BUTTON_RIGHT_SHOULDER)
	assert_bool(level.paused).override_failure_message("RB does not pause").is_true()
	await _assert_frozen(sim, 300, "RB")
	_pad(JOY_BUTTON_A)
	assert_bool(level.paused).is_false()
	assert_object(level._pause_menu).is_null()
	for i in 5:
		await get_tree().physics_frame
	_pad(JOY_BUTTON_LEFT_SHOULDER)
	await _wait_until(func() -> bool: return level._pause_menu != null and level._pause_menu.idle, 2000)
	assert_object(level._pause_menu).override_failure_message("LB does not open the in-game menu").is_not_null()
	assert_bool(level.paused).is_false()


func test_shoulder_buttons_are_not_sidekicks() -> void:
	for b in [JOY_BUTTON_LEFT_SHOULDER, JOY_BUTTON_RIGHT_SHOULDER]:
		assert_bool(C.DEFAULT_PAD_BUTTONS["left_sidekick"].has(b)).is_false()
		assert_bool(C.DEFAULT_PAD_BUTTONS["right_sidekick"].has(b)).is_false()


func test_start_button_and_rebound_key() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	_pad(JOY_BUTTON_START)
	assert_bool(level.paused).override_failure_message("Start does not pause").is_true()
	await _assert_frozen(sim, 300, "Start")
	_pad(JOY_BUTTON_A)
	assert_bool(level.paused).override_failure_message("a button does not resume").is_false()
	# Reconfigured key: O pauses, and P too (mainint.c: "keysactive[P] ||
	# input_held(INPUT_PAUSE)"); Start stays.
	assert_bool(C.rebind_global(C.PAUSE, KEY_O)).is_true()
	await _wait_ms(100)
	_press(KEY_F2)
	assert_bool(level.paused).override_failure_message("the old F2 key still pauses").is_false()
	_press(KEY_O)
	assert_bool(level.paused).override_failure_message("O does not pause").is_true()
	_press(KEY_SPACE)
	_press(KEY_P)
	assert_bool(level.paused).override_failure_message("P no longer pauses").is_true()
	_press(KEY_SPACE)
	_pad(JOY_BUTTON_START)
	assert_bool(level.paused).override_failure_message("Start lost after the new key").is_true()


## F9 during the pause: the controls screen opens, closing it returns the
## level still paused (music included), then a key resumes it.
func test_pause_survives_f9_overlay() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	_press(KEY_P)
	Effects.open_options("res://scenes/controls_options.tscn")
	assert_bool(Effects.level_paused()).is_true()
	await _assert_frozen(sim, 200, "controls screen")
	_press(KEY_ESCAPE)  # closes the controls screen
	await _wait_ms(200)
	assert_object(Effects.current_game_scene()).is_same(level)
	assert_bool(level.paused).override_failure_message("closing F9 lifted the pause").is_true()
	await _assert_frozen(sim, 300, "after F9")
	var tick: int = sim.tick()
	_press(KEY_SPACE)
	await _wait_until(func() -> bool: return sim.tick() > tick + 3, 5000)
	assert_int(sim.tick()).is_greater(tick + 3)


func test_pause_key_refuses_reserved_keys() -> void:
	# Esc, F1, P (reserved), F9 / F10 / F12 (other shortcuts) and Space (fire).
	for k in [KEY_ESCAPE, KEY_F9, KEY_F10, KEY_F12, KEY_SPACE, KEY_F1, KEY_P]:
		(
			assert_bool(C.rebind_global(C.PAUSE, k))
			. override_failure_message("key %s accepted" % OS.get_keycode_string(k))
			. is_false()
		)
	assert_int(C.global_key(C.PAUSE)).is_equal(KEY_F2)
	# B is no longer a menu action (Endless ranking in High Scores).
	assert_str(C.menu_action(C.key_event(KEY_B))).is_equal(&"")


## The Endless ranking is the last page of High Scores (no more B key at the title).
func test_high_scores_endless_page_is_last_and_circular() -> void:
	var hs: Node = Game.goto_scene("res://scenes/high_scores.tscn")
	await _wait_until(func() -> bool: return hs.idle, 15000)
	assert_int(hs.PAGE_COUNT).is_equal(hs.EPISODE_COUNT + 1)
	hs.handle_action(C.LEFT)
	assert_int(hs.episode_index).override_failure_message("Left on page 1: not the Endless page").is_equal(
		hs.ENDLESS_PAGE
	)
	hs.handle_action(C.RIGHT)
	assert_int(hs.episode_index).override_failure_message("Right on the Endless page: not page 1").is_equal(0)
	assert_str(C.menu_action(C.key_event(KEY_B))).is_equal(&"")


## JE_pauseGame: set_volume(tyrMusicVolume / 2) during the pause, the music
## continues (no suspension), then tyrMusicVolume on resume.
func test_pause_halves_music_volume() -> void:
	var level: Node = await _start_level()
	var full: int = Music.user_volume
	_press(KEY_P)
	assert_bool(level.paused).is_true()
	assert_int(Music.music_volume).override_failure_message("volume during pause").is_equal(full / 2)
	(
		assert_bool(Music._player.stream_paused and not Music.music_stopped)
		. override_failure_message("the music is suspended")
		. is_false()
	)
	_press(KEY_SPACE)
	assert_bool(level.paused).is_false()
	assert_int(Music.music_volume).override_failure_message("volume after resuming").is_equal(full)


## Esc in a level: the C's pause menu (tyrian2.c, INPUT_MENU_CANCEL ->
## JE_inGameSetup), simulation frozen; Esc again resumes. "Quit"
## raises the "Are you sure you want to exit?" box (JE_quitRequest), "Yes"
## quits to the title (pause_menu_test.gd covers the rest of the menu).
func test_escape_asks_before_leaving() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	await _wait_ms(100)
	_press(KEY_ESCAPE)
	await _wait_until(func() -> bool: return level._pause_menu != null, 2000)
	assert_object(level._pause_menu).override_failure_message("Escape does not open the pause menu").is_not_null()
	await _assert_frozen(sim, 300, "pause menu")
	# Esc again: the game resumes, still in the level.
	await _wait_until(func() -> bool: return level._pause_menu.idle, 2000)
	_press(KEY_ESCAPE)
	assert_object(level._pause_menu).is_null()
	var tick: int = sim.tick()
	await _wait_until(func() -> bool: return sim.tick() > tick + 5, 5000)
	(
		assert_bool(is_instance_valid(level) and level.is_inside_tree())
		. override_failure_message("Escape left the level")
		. is_true()
	)
	assert_int(sim.tick()).is_greater(tick + 5)
	# Back (gamepad) also opens the menu; Quit, Left then Enter = "Yes".
	_pad(JOY_BUTTON_BACK)
	await _wait_until(func() -> bool: return level._pause_menu != null and level._pause_menu.idle, 2000)
	assert_object(level._pause_menu).override_failure_message("Back does not open the menu").is_not_null()
	var menu: Node = level._pause_menu
	_press(KEY_UP)
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return menu.modal_active, 2000)
	_press(KEY_LEFT)
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return Effects.current_game_scene().scene_file_path == Game.SCENE_TITLE, 5000)
	assert_str(Effects.current_game_scene().scene_file_path).is_equal(Game.SCENE_TITLE)
