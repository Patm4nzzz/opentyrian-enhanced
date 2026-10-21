# In-game pause menu (mainint.c:JE_inGameSetup), in-game help
# (JE_inGameHelp) and game speed (config.c:JE_setNewGameSpeed), driven
# by real events: Esc opens the menu, Up/Down/Left/Right/Enter
# act, the mouse clicks the rows. The simulation does not run during
# the menu and resumes without a lost tick.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const ES := preload("res://scripts/endless_session.gd")
const SETUP := preload("res://scripts/setup.gd")
const GAME_SPEED := preload("res://scripts/game_speed.gd")

var _saved_settings := {}


func before() -> void:
	ES.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()
	_saved_settings = SETUP.settings.duplicate()


func after() -> void:
	SETUP.settings = _saved_settings
	SETUP.apply_settings()
	SETUP.save_settings()
	GAME_SPEED.restore()
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func before_test() -> void:
	SETUP.reset_settings()
	SETUP.apply_settings()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	ES.stop()
	GAME_SPEED.restore()
	await get_tree().process_frame
	await get_tree().process_frame


func _sim() -> Object:
	for n in Effects.game_viewport.find_children("TyrianSim", "", true, false):
		return n
	return null


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


func _press(code: int, ctrl := false) -> void:
	var ev := C.key_event(code)
	ev.ctrl_pressed = ctrl
	_send(ev)


func _pad(button: int) -> void:
	var ev := InputEventJoypadButton.new()
	ev.button_index = button
	ev.pressed = true
	_send(ev)


## Click in the game's SubViewport (320×200 coordinates).
func _click(x: int, y: int, button := MOUSE_BUTTON_LEFT) -> void:
	for pressed in [true, false]:
		var ev := InputEventMouseButton.new()
		ev.button_index = button
		ev.pressed = pressed
		ev.position = Vector2(x, y)
		ev.global_position = ev.position
		Effects.game_viewport.push_input(ev, true)


func _start_level() -> Node:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	var sim: Object = _sim()
	await _wait_until(func() -> bool: return sim != null and sim.tick() >= 20, 5000)
	return level


func _open_menu(level: Node) -> Node:
	await _wait_ms(100)
	_press(KEY_ESCAPE)
	await _wait_until(func() -> bool: return level._pause_menu != null and level._pause_menu.idle, 2000)
	return level._pause_menu


func _assert_frozen(sim: Object, ms: int, what: String) -> void:
	var tick: int = sim.tick()
	var hash: int = sim.state_hash()
	await _wait_ms(ms)
	assert_int(sim.tick()).override_failure_message("%s: tick advanced" % what).is_equal(tick)
	assert_int(sim.state_hash()).override_failure_message("%s: state modified" % what).is_equal(hash)


func _saved(key: String) -> Variant:
	var cfg := ConfigFile.new()
	assert_int(cfg.load(SETUP.SETTINGS_PATH)).is_equal(OK)
	return cfg.get_value("setup", key)


## "Achievement" banner visible when the menu opens: it is not
## in the frozen image under the menu (in Original mode the Rust core composes it
## into the game image), and comes back on resume.
func test_achievement_banner_not_in_frozen_image_and_returns_after_resume() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	if level._playfield_bg == null or not ClassDB.class_has_method("TyrianSim", "set_playfield_banner"):
		return
	level._ach_notify_id = 0
	level._ach_notify_start = sim.ach_clock_ms()
	await _wait_ms(100)
	assert_str(level._hud_banner_text()).override_failure_message("banner not shown").is_not_empty()
	var menu: Node = await _open_menu(level)
	var px: PackedByteArray = menu.vga.indices(menu.VGA_SCREEN)
	ClassDB.class_call_static("TyrianSim", "set_playfield_banner", "")
	var clean: PackedByteArray = sim.playfield_indices(Game.data_dir)
	for y in range(3, 12):
		(
			assert_array(px.slice(y * 320, y * 320 + 264))
			. override_failure_message("banner drawn under the pause menu (y %d)" % y)
			. is_equal(clean.slice(y * 320 + 24, y * 320 + 288))
		)
	_press(KEY_ESCAPE)
	await _wait_until(func() -> bool: return level._pause_menu == null, 2000)
	await _wait_ms(200)
	assert_str(level._hud_banner_text()).override_failure_message("banner not resumed").is_not_empty()


## Esc: menu on the frozen (darkened) image, simulation frozen; Esc
## closes it and the game resumes at the tick where it had stopped, one tick per
## physics frame (no tick lost or played in excess).
func test_escape_opens_menu_freezes_and_resumes_without_lost_tick() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	var menu: Node = await _open_menu(level)
	assert_object(menu).override_failure_message("Escape does not open the pause menu").is_not_null()
	assert_object(level._quit_box).is_null()
	assert_int(menu.menus.selection()).is_equal(0)
	assert_int(menu.menus.current().items.size()).is_equal(6)
	var tick: int = sim.tick()
	var hash: int = sim.state_hash()
	await _assert_frozen(sim, 400, "pause menu")
	# Image: darkened panel (JE_barShade) over the level image, "*" before
	# the chosen row: the menu is not the game image.
	var px: PackedByteArray = menu.vga.indices(menu.VGA_SCREEN)
	var frozen: PackedByteArray = sim.playfield_indices(Game.data_dir)
	assert_int(px[100 * 320 + 4] & 0x0f).override_failure_message("panel not darkened").is_less_equal(
		maxi(frozen[100 * 320 + 4 + 24] & 0x0f, 1)
	)
	# Bottom of the dashboard (x 204..319, y 191..199): the level's, not the
	# black strip of leftover text from the settings screens' backdrop.
	var hud_px: PackedByteArray = level._hud_vga.vga.indices(HudRenderer.SEG)
	for y in range(191, 200):
		(
			assert_array(px.slice(y * 320 + 204, y * 320 + 320))
			. override_failure_message("black band on the dashboard while paused (y %d)" % y)
			. is_equal(hud_px.slice(y * 320 + 204, y * 320 + 320))
		)
	_press(KEY_DOWN)
	assert_int(menu.menus.selection()).is_equal(1)
	_press(KEY_UP)
	_press(KEY_UP)
	assert_int(menu.menus.selection()).override_failure_message("Up from the first row wraps to Quit").is_equal(5)
	_press(KEY_ESCAPE)
	assert_object(level._pause_menu).override_failure_message("Escape does not close the menu").is_null()
	assert_int(sim.tick()).is_equal(tick)
	assert_int(sim.state_hash()).override_failure_message("state changed by the menu").is_equal(hash)
	var f0 := Engine.get_physics_frames()
	await _wait_ms(500)
	var played: int = sim.tick() - tick
	var frames := Engine.get_physics_frames() - f0
	(
		assert_bool(is_instance_valid(level) and level.is_inside_tree())
		. override_failure_message("Escape left the level")
		. is_true()
	)
	assert_object(level._pause_menu).override_failure_message("the menu reopened").is_null()
	assert_int(played).override_failure_message("ticks played %d for %d frames" % [played, frames]).is_between(
		frames - 1, frames
	)
	# Back (gamepad, the C's action_pressed[4]) also opens the menu; "Return to Game" closes it.
	_pad(JOY_BUTTON_BACK)
	await _wait_until(func() -> bool: return level._pause_menu != null, 2000)
	assert_object(level._pause_menu).override_failure_message("Back does not open the menu").is_not_null()
	await _wait_until(func() -> bool: return level._pause_menu.idle, 2000)
	for i in 4:
		_press(KEY_DOWN)
	_press(KEY_ENTER)
	assert_object(level._pause_menu).override_failure_message("Return to Game does not close").is_null()


## Volume rows: Left/Right ±12 (JE_changeVolume), Enter mutes;
## setting applied to the sound right away and saved on close
## (same keys as Setup > Sound).
func test_volume_rows_apply_and_save() -> void:
	var level: Node = await _start_level()
	var menu: Node = await _open_menu(level)
	assert_int(int(SETUP.settings.music_volume)).is_equal(191)
	_press(KEY_RIGHT)
	assert_int(int(SETUP.settings.music_volume)).is_equal(203)
	assert_int(Music.user_volume).override_failure_message("music volume not applied").is_equal(203)
	_press(KEY_LEFT)
	_press(KEY_LEFT)
	assert_int(Music.user_volume).is_equal(179)
	_press(KEY_ENTER)
	assert_bool(Music.muted).override_failure_message("Enter does not mute the music").is_true()
	_press(KEY_ENTER)
	assert_bool(Music.muted).is_false()
	_press(KEY_DOWN)
	_press(KEY_RIGHT)
	_press(KEY_RIGHT)
	assert_int(Sfx.volume).override_failure_message("effects volume not applied").is_equal(215)
	for i in 5:
		_press(KEY_RIGHT)
	assert_int(Sfx.volume).override_failure_message("volume capped at 255").is_equal(255)
	# Bar redrawn: (255 + 6) / 12 = 21 notches starting at x = 120.
	var px: PackedByteArray = menu.vga.indices(menu.VGA_SCREEN)
	assert_int(px[(40 + 5) * 320 + 120 + 20 * 4 + 1]).override_failure_message("effects bar not drawn").is_equal(
		16 + 12
	)
	_press(KEY_ESCAPE)
	assert_int(int(_saved("music_volume"))).override_failure_message("music volume not saved").is_equal(179)
	assert_int(int(_saved("fx_volume"))).is_equal(255)


## Game speed: Slug, Slower, Slow, Normal, Turbo (config.c); ticks per
## second measured in play for each notch, Normal = 35 (parity), setting
## saved (save_json.c "game_speed"), Normal rhythm outside a level.
func test_game_speed_rows_set_ticks_per_second() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	assert_int(Engine.physics_ticks_per_second).override_failure_message("Normal = 35 ticks/s").is_equal(35)
	var expected := {1: 14, 2: 23, 3: 28, 4: 35, 5: 49}
	# Left from Normal: Slow, Slower, Slug; then Right up to Turbo.
	var order := [
		[KEY_LEFT, 3], [KEY_LEFT, 2], [KEY_LEFT, 1], [KEY_RIGHT, 2], [KEY_RIGHT, 3], [KEY_RIGHT, 4], [KEY_RIGHT, 5]
	]
	var measured := {}
	for step in order:
		await _open_menu(level)
		for i in 3:
			_press(KEY_DOWN)
		_press(step[0])
		assert_int(int(SETUP.settings.game_speed)).is_equal(step[1])
		_press(KEY_ESCAPE)
		assert_int(Engine.physics_ticks_per_second).override_failure_message("speed %d" % step[1]).is_equal(
			expected[step[1]]
		)
		if measured.has(step[1]):
			continue
		await _wait_ms(200)
		var t0: int = sim.tick()
		var ms0 := Time.get_ticks_msec()
		await _wait_ms(1500)
		var rate := float(sim.tick() - t0) * 1000.0 / float(Time.get_ticks_msec() - ms0)
		measured[step[1]] = rate
		(
			assert_float(rate)
			. override_failure_message(
				"speed %d: %.1f ticks/s measured, %d expected" % [step[1], rate, expected[step[1]]]
			)
			. is_between(expected[step[1]] * 0.8, expected[step[1]] * 1.15)
		)
	print("pause_menu_test: ticks/s measured", measured)
	assert_int(int(_saved("game_speed"))).override_failure_message("speed not saved").is_equal(5)
	# Right on Turbo wraps to Slug (speed_row_act).
	await _open_menu(level)
	for i in 3:
		_press(KEY_DOWN)
	_press(KEY_RIGHT)
	assert_int(int(SETUP.settings.game_speed)).is_equal(1)
	_press(KEY_ESCAPE)
	Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	await get_tree().process_frame
	assert_int(Engine.physics_ticks_per_second).override_failure_message("outside a level: Normal pace").is_equal(35)


## Visual effects: Off, Basic, Extended in a loop (visual_fx.c), advanced
## lighting (RGBA overlay) only with Extended; saved.
func test_visual_fx_row_cycles_presets() -> void:
	var level: Node = await _start_level()
	await _open_menu(level)
	_press(KEY_DOWN)
	_press(KEY_DOWN)
	assert_int(int(SETUP.settings.visual_fx)).is_equal(2)
	_press(KEY_RIGHT)
	assert_int(int(SETUP.settings.visual_fx)).override_failure_message("Extended -> Off").is_equal(0)
	assert_bool(ClassDB.class_call_static("TyrianSim", "advanced_lighting")).is_false()
	_press(KEY_LEFT)
	_press(KEY_LEFT)
	assert_int(int(SETUP.settings.visual_fx)).is_equal(1)
	_press(KEY_ENTER)
	assert_int(int(SETUP.settings.visual_fx)).is_equal(2)
	assert_bool(ClassDB.class_call_static("TyrianSim", "advanced_lighting")).is_true()
	_press(KEY_LEFT)
	_press(KEY_ESCAPE)
	assert_int(int(_saved("visual_fx"))).is_equal(1)


## Quit: "Are you sure you want to exit?" box (JE_quitRequest), "No"
## preselected = back to the game; "Yes" = exit to the title.
func test_quit_asks_then_leaves_to_title() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	var menu: Node = await _open_menu(level)
	_press(KEY_UP)
	assert_int(menu.menus.selection()).is_equal(5)
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return menu.modal_active, 2000)
	assert_bool(menu.modal_active).override_failure_message("Quit does not open the confirmation").is_true()
	await _assert_frozen(sim, 200, "confirmation")
	_press(KEY_ENTER)  # 'No'
	await _wait_until(func() -> bool: return level._pause_menu == null, 2000)
	assert_object(level._pause_menu).override_failure_message("'No' does not return to the game").is_null()
	var tick: int = sim.tick()
	await _wait_until(func() -> bool: return sim.tick() > tick + 3, 3000)
	assert_bool(is_instance_valid(level) and level.is_inside_tree()).is_true()
	menu = await _open_menu(level)
	_press(KEY_UP)
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return menu.modal_active, 2000)
	_press(KEY_LEFT)
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return Effects.current_game_scene().scene_file_path == Game.SCENE_TITLE, 5000)
	(
		assert_str(Effects.current_game_scene().scene_file_path)
		. override_failure_message("'Yes' does not return to the title")
		. is_equal(Game.SCENE_TITLE)
	)


## Mouse (menu_tree.c:run_menu): click on the Game Speed row = next
## notch, click in the volume bar = volume at that point, right click = exit.
func test_mouse_clicks_rows() -> void:
	var level: Node = await _start_level()
	var menu: Node = await _open_menu(level)
	_click(30, 20 + 3 * 20 + 4)
	assert_int(int(SETUP.settings.game_speed)).override_failure_message("click on Game Speed").is_equal(5)
	assert_int(menu.menus.selection()).is_equal(3)
	_click(10 + 110 + 44, 20 + 4)
	assert_int(int(SETUP.settings.music_volume)).override_failure_message("click in the bar").is_equal(
		44 * 255 / (menu.VOLUME_BAR_W - 1)
	)
	_click(100, 100, MOUSE_BUTTON_RIGHT)
	assert_object(level._pause_menu).override_failure_message("right click does not close").is_null()


## F1: the C's in-game help (panels on a black background), simulation frozen, any
## key closes it; Ctrl+F1: the rewrite's technical help (tick, curLoc).
func test_f1_shows_in_game_help() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	_press(KEY_F1)
	await _wait_until(func() -> bool: return level._help_screen != null, 2000)
	var help: Node = level._help_screen
	assert_object(help).override_failure_message("F1 does not open help").is_not_null()
	await _assert_frozen(sim, 300, "help")
	var px: PackedByteArray = help.vga.indices(help.VGA_SCREEN)
	assert_int(px[100 * 320 + 250]).override_failure_message("black help background").is_equal(0)
	var lit := 0
	for y in range(10, 30):
		for x in range(40, 250):
			if px[y * 320 + x] != 0:
				lit += 1
	assert_int(lit).override_failure_message("energy bar text missing").is_greater(50)
	assert_bool(level._debug_text.visible).is_false()
	var tick: int = sim.tick()
	_press(KEY_SPACE)
	(
		assert_object(level._help_screen)
		. override_failure_message("a key moves on to the keys and mouse page")
		. is_not_null()
	)
	assert_int(level._help_screen._panel).is_equal(2)
	_press(KEY_SPACE)
	assert_object(level._help_screen).override_failure_message("the second key does not close help").is_null()
	await _wait_until(func() -> bool: return sim.tick() > tick + 3, 3000)
	assert_int(sim.tick()).is_greater(tick + 3)
	_press(KEY_F1, true)
	assert_bool(level._debug_text.visible).override_failure_message("Ctrl+F1: technical help").is_true()
	assert_object(level._help_screen).is_null()


## --speed=N (params.c, debugging): overrides the setting, not saved.
func test_speed_override() -> void:
	GAME_SPEED.speed_override = 5
	assert_int(GAME_SPEED.fast_play(4)).is_equal(1)
	assert_int(GAME_SPEED.ticks_per_second(GAME_SPEED.fast_play(1))).is_equal(49)
	GAME_SPEED.speed_override = 1
	assert_int(GAME_SPEED.ticks_per_second(GAME_SPEED.fast_play(4))).is_equal(14)
	GAME_SPEED.speed_override = -1
	assert_int(GAME_SPEED.fast_play(4)).override_failure_message("without --speed: setting").is_equal(0)


## Player regression (hover on the "Are you sure you want to exit?" box): the
## pointer on "Yes" or "No" moves the selection, as in the other
## menus. Pause > Quit opens the TyrianScreen box; the image changes when the
## highlight goes from "No" to "Yes".
func test_quit_box_hover_moves_selection_after_pause_quit() -> void:
	var level: Node = await _start_level()
	var menu: Node = await _open_menu(level)
	menu._quit_action()
	await _wait_until(func() -> bool: return menu.modal_active, 2000)
	assert_bool(menu.modal_active).is_true()
	await _wait_ms(150)
	var closed_before: PackedByteArray = menu.vga.indices(menu.VGA_SCREEN).slice(128 * 320 + 60, 128 * 320 + 140)
	TyrianMouse.inactive = false
	var ev := InputEventMouseMotion.new()
	ev.relative = Vector2(1, 0)
	ev.position = Vector2(100, 136)
	ev.global_position = ev.position
	Effects.game_viewport.push_input(ev, true)
	await _wait_ms(150)
	var hovered_yes: PackedByteArray = menu.vga.indices(menu.VGA_SCREEN).slice(128 * 320 + 60, 128 * 320 + 140)
	(
		assert_bool(hovered_yes != closed_before)
		. override_failure_message("hovering over 'Yes' does not change the selection")
		. is_true()
	)
	# Back on "No": the "Yes" image goes back to the original.
	var back := InputEventMouseMotion.new()
	back.relative = Vector2(1, 0)
	back.position = Vector2(200, 136)
	back.global_position = back.position
	Effects.game_viewport.push_input(back, true)
	await _wait_ms(150)
	var hovered_no: PackedByteArray = menu.vga.indices(menu.VGA_SCREEN).slice(128 * 320 + 60, 128 * 320 + 140)
	(
		assert_bool(hovered_no == closed_before)
		. override_failure_message("hovering over 'No' does not return the selection to 'No'")
		. is_true()
	)
	TyrianMouse.inactive = true


## Player regression (mouse on the ship): with the window enlarged ×3 each
## event arrives as a fraction of a game pixel (1/3). The level accumulates these
## fractions and takes integers at each tick; the old code truncated
## each event to 0 and the ship never moved.
func test_level_accumulates_fractional_mouse_motion() -> void:
	var level: Node = await _start_level()
	var sim: Object = _sim()
	await _wait_until(func() -> bool: return sim.tick() >= 200, 8000)
	var start_x: int = sim.player_dict(0)["x"]
	for i in 90:
		var ev := InputEventMouseMotion.new()
		ev.relative = Vector2(1.0 / 3.0, 0)
		ev.position = Vector2(160, 100)
		level._input(ev)
		await get_tree().physics_frame
	await _wait_ms(200)
	var end_x: int = sim.player_dict(0)["x"]
	(
		assert_int(end_x)
		. override_failure_message("90 events of 1/3 px must move the ship (%d -> %d)" % [start_x, end_x])
		. is_greater(start_x + 5)
	)


## menu_tree.c:run_menu: hover only changes the selection when the mouse is active (mouseInactive false).
func test_pause_menu_hover_ignored_while_mouse_inactive() -> void:
	var level: Node = await _start_level()
	var menu: Node = await _open_menu(level)
	var m: Dictionary = menu.menus.current()
	var start_sel: int = m.sel
	var target := -1
	var pos := Vector2.ZERO
	for y in range(20, 190):
		for x in [100, 160]:
			var row: int = menu.menus.hittest_row(x, y)
			if row >= 0 and row != start_sel:
				target = row
				pos = Vector2(x, y)
				break
		if target >= 0:
			break
	assert_int(target).override_failure_message("no other row found").is_greater_equal(0)
	var ev := InputEventMouseMotion.new()
	ev.relative = Vector2(1, 0)
	ev.position = pos
	ev.global_position = pos
	TyrianMouse.inactive = true
	menu.mouse_event(ev)
	assert_int(m.sel).override_failure_message("hover taken into account when mouse inactive").is_equal(start_sel)
	TyrianMouse.inactive = false
	menu.mouse_event(ev)
	assert_int(m.sel).override_failure_message("hover ignored while mouse active").is_equal(target)
