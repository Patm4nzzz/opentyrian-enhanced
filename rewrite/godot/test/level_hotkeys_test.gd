# C level shortcuts (input_map.c:enhanced_keys[], mainint.c) with real
# keyboard events passed through Input.parse_input_event: slow motion on left Shift
# (and not on right Shift, nor on Tab), achievements F3, statistics F4, DPS F6,
# screenshot F12, gamma F11, quick save F5 / load F9.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")

var _level: Node


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()


func after() -> void:
	C.reset_defaults()
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func before_test() -> void:
	Effects.bullet_time_allowed = true
	Effects.bullet_time = false
	Effects._bullet_time_cooldown_end = 0
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	DisplaySettings._loaded = false


func after_test() -> void:
	C.reset_defaults()
	Effects.bullet_time = false
	Effects._bullet_time_cooldown_end = 0
	Game.campaign_active = false
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	await get_tree().process_frame
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	DisplaySettings._loaded = false


func _key(code: int, location := KEY_LOCATION_UNSPECIFIED) -> void:
	var ev := C.key_event(code)
	ev.location = location
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEventKey = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func _start_level() -> Node:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.difficulty = 2
	Game.loadout = PackedInt32Array(Game.NEW_GAME_LOADOUT)
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	_level = Game.goto_scene(Game.SCENE_LEVEL)
	await get_tree().process_frame
	await get_tree().process_frame
	return _level


func _messages() -> Array:
	var out := []
	for m in _level._hud_vga._messages:
		if m[2] > 0:
			out.append(m[0])
	return out


func _hud_hash() -> int:
	_level.hotkeys.overlays.draw(_level._hud_vga.vga, _level.sim.ach_stats(), _level.sim.ach_unlocked_flags())
	return hash(_level._hud_vga.vga.indices(_level._hud_vga.OVER_A))


func test_shift_left_starts_bullet_time_but_tab_and_shift_right_do_not() -> void:
	await _start_level()
	_key(KEY_TAB)
	assert_bool(Effects.bullet_time).override_failure_message("Tab triggers slow motion").is_false()
	_key(KEY_SHIFT, KEY_LOCATION_RIGHT)
	assert_bool(Effects.bullet_time).override_failure_message("Right Shift triggers slow motion").is_false()
	_key(KEY_SHIFT, KEY_LOCATION_LEFT)
	assert_bool(Effects.bullet_time).override_failure_message("Left Shift does not trigger slow motion").is_true()
	assert_float(Effects.tick_speed()).is_equal(0.25)
	assert_array(_messages()).contains([TyrianText.L("msg_bullet_time")])
	# Second press during slow motion: "active" message, no new use.
	var uses := Effects.bullet_time_uses
	_key(KEY_SHIFT, KEY_LOCATION_LEFT)
	assert_int(Effects.bullet_time_uses).is_equal(uses)
	assert_array(_messages()).contains([TyrianText.L("msg_bullet_time_active")])


## dev_console.c: with --devmode, Ctrl+Shift+letter is a cheat; the C has
## slow motion on Tab, so this prefix never triggers it (player-smoke sends
## Ctrl+Shift+H every second and slow motion shifted the Quit box image).
func test_ctrl_shift_dev_cheat_does_not_start_bullet_time() -> void:
	await _start_level()
	var DevConsole := preload("res://scripts/dev_console.gd")
	DevConsole.set_available(true)
	var ev := C.key_event(KEY_SHIFT)
	ev.location = KEY_LOCATION_LEFT
	ev.ctrl_pressed = true
	ev.shift_pressed = true
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEventKey = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()
	DevConsole.set_available(false)
	(
		assert_bool(Effects.bullet_time)
		. override_failure_message("Ctrl+Shift (--devmode cheat) triggers slow motion")
		. is_false()
	)


func test_slow_motion_label_is_drawn_while_active() -> void:
	await _start_level()
	var vga: RefCounted = _level._hud_vga.vga
	_level._hud_vga.refresh(_level.sim)
	var before: int = hash(vga.indices(_level._hud_vga.OVER_A))
	_key(KEY_SHIFT, KEY_LOCATION_LEFT)
	_level._hud_vga.refresh(_level.sim)
	(
		assert_int(hash(vga.indices(_level._hud_vga.OVER_A)))
		. override_failure_message("SLOW MOTION label missing")
		. is_not_equal(before)
	)


func test_bullet_time_unlocks_the_achievement_and_counts_uses() -> void:
	await _start_level()
	_key(KEY_SHIFT, KEY_LOCATION_LEFT)
	assert_int(_level.sim.ach_counter("bullet_time_uses")).is_equal(Effects.bullet_time_uses)
	var flags: PackedByteArray = _level.sim.ach_unlocked_flags()
	# achievements.h: ACH_BULLET_TIME is index 4 in the C's alphabetical order.
	var unlocked := 0
	for f in flags:
		unlocked += f
	assert_int(unlocked).override_failure_message("no achievement unlocked by slow motion").is_greater(0)


func test_f3_and_f4_toggle_and_close_each_other() -> void:
	await _start_level()
	var overlays: LevelOverlays = _level.hotkeys.overlays
	_key(KEY_F4)
	assert_bool(overlays.stats_active).is_true()
	_key(KEY_F3)
	assert_bool(overlays.achievements_active).is_true()
	assert_bool(overlays.stats_active).override_failure_message("F3 does not close the statistics").is_false()
	_key(KEY_F4)
	assert_bool(overlays.stats_active).is_true()
	assert_bool(overlays.achievements_active).override_failure_message("F4 does not close the achievements").is_false()
	_key(KEY_F4)
	assert_bool(overlays.stats_active).is_false()


func test_overlays_actually_draw_something() -> void:
	await _start_level()
	var base := _hud_hash()
	_key(KEY_F4)
	var stats := _hud_hash()
	assert_int(stats).override_failure_message("F4 draws nothing").is_not_equal(base)
	_key(KEY_F3)
	var achievements := _hud_hash()
	assert_int(achievements).is_not_equal(stats)
	_key(KEY_F3)
	_key(KEY_F6)
	var dps := _hud_hash()
	assert_int(dps).override_failure_message("F6 draws nothing").is_not_equal(base)


func test_achievement_list_navigation_with_arrows() -> void:
	await _start_level()
	var overlays: LevelOverlays = _level.hotkeys.overlays
	_key(KEY_F3)
	_key(KEY_DOWN)
	_key(KEY_DOWN)
	assert_int(overlays.cursor).is_equal(2)
	_key(KEY_UP)
	assert_int(overlays.cursor).is_equal(1)
	_key(KEY_RIGHT)
	assert_int(overlays.category).is_equal(1)
	assert_int(overlays.cursor).override_failure_message("cursor not reset to zero").is_equal(0)
	_key(KEY_LEFT)
	assert_int(overlays.category).is_equal(0)


func test_f12_writes_a_bmp_and_counts_the_screenshot() -> void:
	await _start_level()
	var image := Image.create(64, 40, false, Image.FORMAT_RGBA8)
	image.fill(Color(0.2, 0.4, 0.8))
	var dir := ProjectSettings.globalize_path("user://hotkeys_test")
	DirAccess.make_dir_recursive_absolute(dir)
	LevelHotkeys._screenshot_next = -1
	var number := LevelHotkeys.save_screenshot(image, dir)
	assert_int(number).is_equal(1)
	assert_bool(FileAccess.file_exists(dir.path_join("screenshot_0000.bmp"))).is_true()
	var bytes := FileAccess.get_file_as_bytes(dir.path_join("screenshot_0000.bmp"))
	assert_str(bytes.slice(0, 2).get_string_from_ascii()).is_equal("BM")
	# Numbering: the next capture does not replace the previous one.
	assert_int(LevelHotkeys.save_screenshot(image, dir)).is_equal(2)
	assert_bool(FileAccess.file_exists(dir.path_join("screenshot_0001.bmp"))).is_true()
	for f in DirAccess.get_files_at(dir):
		DirAccess.remove_absolute(dir.path_join(f))
	DirAccess.remove_absolute(dir)
	# With the key: message and lifetime counter.
	var before: int = _level.sim.ach_counter("screenshots_taken")
	_key(KEY_F12)
	if DisplayServer.get_name() != "headless":
		assert_int(_level.sim.ach_counter("screenshots_taken")).is_equal(before + 1)


func test_f11_cycles_gamma_over_four_levels_and_saves_it() -> void:
	await _start_level()
	assert_int(DisplaySettings.gamma()).is_equal(0)
	_key(KEY_F11)
	assert_int(DisplaySettings.gamma()).is_equal(1)
	assert_float(float(Effects._material.get_shader_parameter("gamma_gain"))).is_equal_approx(1.1, 0.0001)
	_key(KEY_F11)
	_key(KEY_F11)
	assert_int(DisplaySettings.gamma()).is_equal(3)
	_key(KEY_F11)
	assert_int(DisplaySettings.gamma()).override_failure_message("gamma does not wrap around").is_equal(0)
	assert_float(float(Effects._material.get_shader_parameter("gamma_gain"))).is_equal_approx(1.0, 0.0001)
	_key(KEY_F11)
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.gamma()).override_failure_message("gamma not saved").is_equal(1)


func test_f7_toggles_damage_numbers() -> void:
	await _start_level()
	var SetupScreen := preload("res://scripts/setup.gd")
	var before: bool = SetupScreen.settings.damage_numbers_enabled
	_key(KEY_F7)
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_equal(not before)
	var on_key := "msg_damage_numbers_on" if not before else "msg_damage_numbers_off"
	assert_array(_messages()).override_failure_message("F7 without toggle message").contains([TyrianText.L(on_key)])
	_key(KEY_F7)
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_equal(before)


func test_f10_arms_replay_recording_in_level() -> void:
	await _start_level()
	ReplayRecording.armed = false
	_key(KEY_F10)
	assert_bool(ReplayRecording.armed).is_true()
	assert_array(_messages()).contains([TyrianText.L("msg_replay_armed")])
	_key(KEY_F10)
	assert_bool(ReplayRecording.armed).is_false()


func test_shortcut_keys_are_rebindable_and_conflicts_are_refused() -> void:
	# F4 (statistics) becomes K; F4 does nothing anymore.
	assert_bool(C.rebind_global(C.KEY_STATS, KEY_K)).is_true()
	await _start_level()
	var overlays: LevelOverlays = _level.hotkeys.overlays
	_key(KEY_F4)
	assert_bool(overlays.stats_active).is_false()
	_key(KEY_K)
	assert_bool(overlays.stats_active).is_true()
	# A key already taken by another shortcut, or reserved, is refused.
	(
		assert_bool(C.rebind_global(C.KEY_DPS, KEY_F3))
		. override_failure_message("F3 already assigned to achievements")
		. is_false()
	)
	assert_bool(C.rebind_global(C.KEY_DPS, KEY_ESCAPE)).is_false()
	assert_bool(C.rebind_global(C.KEY_DPS, KEY_F1)).is_false()
	assert_bool(C.rebind_global(C.KEY_DPS, KEY_P)).is_false()
	# A player's game key is refused for a shortcut.
	assert_bool(C.rebind_global(C.KEY_DPS, KEY_SPACE)).is_false()


func test_quick_save_then_quick_load_round_trip() -> void:
	Game.campaign_active = true
	await _start_level()
	var slot_before = Game.campaign_slots()[LevelHotkeys.QUICK_SLOT_ONE_PLAYER - 1]
	if slot_before != null:
		# A quick save left by another suite: F9 would already ask the question.
		_key(KEY_F9)
		assert_bool(_level.hotkeys.quick_load_pending).is_true()
		_key(KEY_N)
		slot_before = null
	_key(KEY_F9)
	# No quick save yet (or an old one): if absent, the message says so.
	if slot_before == null:
		assert_bool(_level.hotkeys.quick_load_pending or _messages().has(TyrianText.L("msg_quick_load_none"))).is_true()
		if _level.hotkeys.quick_load_pending:
			_key(KEY_N)
	_key(KEY_F5)
	assert_array(_messages()).contains([TyrianText.L("msg_quick_saved")])
	var slot = Game.campaign_slots()[LevelHotkeys.QUICK_SLOT_ONE_PLAYER - 1]
	assert_object(slot).override_failure_message("F5 wrote nothing").is_not_null()
	assert_str(str(slot.get("name", ""))).is_equal(TyrianText.L("save_quick_save"))
	# F9 asks the pinned question; N cancels it.
	_key(KEY_F9)
	assert_bool(_level.hotkeys.quick_load_pending).is_true()
	assert_str(_level._hud_vga.pinned_message).is_equal(TyrianText.L("msg_quick_load_prompt"))
	_key(KEY_N)
	assert_bool(_level.hotkeys.quick_load_pending).is_false()
	assert_str(_level._hud_vga.pinned_message).is_empty()
	assert_array(_messages()).contains([TyrianText.L("msg_cancelled")])


## mainint.c:3107-3110: Esc during the F9 question cancels and is consumed;
## it does not also open the pause menu (G-02).
func test_escape_on_quick_load_question_does_not_open_pause_menu() -> void:
	Game.campaign_active = true
	await _start_level()
	_key(KEY_F5)
	_key(KEY_F9)
	assert_bool(_level.hotkeys.quick_load_pending).is_true()
	var ev := C.key_event(KEY_ESCAPE)
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	await get_tree().physics_frame
	await get_tree().physics_frame
	assert_bool(_level.hotkeys.quick_load_pending).is_false()
	assert_object(_level._pause_menu).override_failure_message("Escape also opened the pause").is_null()
	var up: InputEventKey = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func test_no_quick_save_in_endless_or_replay() -> void:
	Game.campaign_active = true
	await _start_level()
	_level.replaying = true
	_key(KEY_F5)
	assert_array(_messages()).not_contains([TyrianText.L("msg_quick_saved")])


## Level launched outside the campaign (side modes): F5 and F9 are ignored, but the
## player is informed.
func test_quick_keys_outside_campaign_show_a_message() -> void:
	Game.campaign_active = false
	await _start_level()
	_key(KEY_F5)
	assert_array(_messages()).contains([TyrianText.L("msg_quick_campaign_only")])
	assert_array(_messages()).not_contains([TyrianText.L("msg_quick_saved")])


## Level of a side mode finished alive: debriefing, not a frozen image
## (tyrian2.c: JE_endLevelAni follows every level finished without death).
func test_side_mode_level_finished_alive_reaches_debrief() -> void:
	Game.campaign_active = false
	Game.debrief = {}
	await _start_level()
	_level.sim.dev_complete_level()
	for i in 900:
		await get_tree().physics_frame
		if not Game.debrief.is_empty():
			break
	assert_bool(Game.debrief.is_empty()).override_failure_message("level finished: no debriefing").is_false()
	assert_bool(bool(Game.debrief.get("all_players_alive", false))).is_true()
	Game.debrief = {}


# config.c:lastLevelName: F5 writes the last COMPLETED level ("NEW GAME" at the start),
# not the current level ("TYRIAN"), like the C (input_map.c:input_action_quick_save).
func test_quick_save_writes_last_finished_level_not_current() -> void:
	Game.campaign_active = true
	await _start_level()
	Game.last_level_name = "NEW GAME"
	Game.level_name = "TYRIAN"
	_key(KEY_F5)
	var slot = Game.campaign_slots()[LevelHotkeys.QUICK_SLOT_ONE_PLAYER - 1]
	assert_object(slot).is_not_null()
	assert_str(str(slot.get("last_level", ""))).is_equal("NEW GAME")
