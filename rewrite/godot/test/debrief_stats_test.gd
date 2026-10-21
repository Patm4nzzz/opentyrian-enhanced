# Debriefing (mainint.c:JE_endLevelAni): detailed statistics screen
# ("Level Statistics Screen", Enhanced addition) after "Press a key", values
# from the simulation summary (stats.c:level_stats), closed by Confirm or
# Back on the keyboard AND the gamepad (input_held(INPUT_MENU_CONFIRM / CANCEL),
# push_joysticks_as_keyboard).
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()
	TyrianControls.reset_defaults()


func after() -> void:
	Game.campaign_active = false
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _send(ev: InputEvent) -> void:
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEvent = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func _key(code: int) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.keycode = code
	ev.physical_keycode = code
	ev.pressed = true
	return ev


func _pad(button: int) -> InputEventJoypadButton:
	var ev := InputEventJoypadButton.new()
	ev.button_index = button
	ev.pressed = true
	return ev


func _wait(cond: Callable, ms := 20000) -> bool:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame
	return cond.call()


## TYRIAN debriefing with the e1-option-plasma-pulse summary recorded on the
## Enhanced C (tyrian-golden/tests/level_stats.rs); "Press a key" passed
## with the gamepad A button, statistics shown, nothing happens on any
## other key, then B (Back) closes the screen and the campaign continues.
func test_statistics_screen_values_and_gamepad_close() -> void:
	Game.episode = 1
	Game.campaign_active = true
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.main_level = 3
	Game.level_start_ms = Time.get_ticks_msec() - 83000  # 1:23 of play
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 12371,
		"enemy_killed": 108,
		"total_enemy": 374,
		"cube_max": 0,
		"all_players_alive": true,
		"damage_by_bay": PackedInt32Array([626, 0, 111, 125, 0]),
		"damage_taken": 222,
		"damage_blocked": 202,
		"credits_start": 10000
	}
	var deb: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.idle)).is_true()
	_send(_pad(JOY_BUTTON_A))
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.stats_shown)).is_true()
	# Menu background (image 2) and text: the top of the screen is no longer black.
	var img: Image = deb.vga.image(0)
	var lit := 0
	for y in range(10, 30):
		for x in range(100, 220):
			if img.get_pixel(x, y).v > 0.5:
				lit += 1
	assert_int(lit).is_greater(50)
	# Displayed values (same formats as the C).
	assert_int(deb._stats_lines.size()).is_greater(0)
	assert_array(deb._stats_lines).contains(
		[
			["ls_enemies", "108 / 374  (28%)"],
			["ls_front_gun", "72%"],
			["ls_l_sidekick", "12%"],
			["ls_r_sidekick", "14%"],
			["ls_damage_taken", "222  (%s 90%%)" % TyrianText.L("ls_shield")],
			["ls_credits", "+2371"]
		]
	)
	# SDL_GetTicks() - start_ticks: the C also counts the debriefing animation.
	var time: String = deb._stats_lines.filter(func(l): return l[0] == "ls_time")[0][1]
	var secs := int(time.get_slice(":", 0)) * 60 + int(time.get_slice(":", 1))
	assert_int(secs).is_between(83, 83 + 30)
	# Any key does not close the screen (only Confirm / Back).
	_send(_key(KEY_X))
	await get_tree().process_frame
	await get_tree().process_frame
	assert_bool(deb.stats_shown).is_true()
	_send(_pad(JOY_BUTTON_B))
	assert_bool(await _wait(func(): return Game.shop_section == 4)).is_true()


## Keyboard: Enter (Confirm) closes the screen; bays under 5% hidden and
## "Damage taken: 0" always shown.
func test_statistics_screen_keyboard_close() -> void:
	Game.episode = 1
	Game.campaign_active = true
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.main_level = 3
	Game.level_start_ms = Time.get_ticks_msec()
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 14983,
		"enemy_killed": 311,
		"total_enemy": 473,
		"cube_max": 0,
		"all_players_alive": true,
		"damage_by_bay": PackedInt32Array([2397, 0, 0, 0, 3]),
		"damage_taken": 0,
		"damage_blocked": 0,
		"credits_start": 10000
	}
	var deb: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.idle)).is_true()
	_send(_key(KEY_SPACE))
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.stats_shown)).is_true()
	assert_array(deb._stats_lines).contains(
		[["ls_front_gun", "99%"], ["ls_damage_taken", "0"], ["ls_credits", "+4983"]]
	)
	assert_array(deb._stats_lines.map(func(l): return l[0])).not_contains(["ls_special"])
	_send(_key(KEY_ENTER))
	assert_bool(await _wait(func(): return Game.shop_section == 4)).is_true()


## B-06: Timed Battle, "Time Bonus" = (levelTimerCountdown / 10) × 100 and "Lives Bonus" =
## lives × 1000 added to the money; no "Cubes" block (mainint.c:JE_endLevelAni).
func test_timed_battle_time_and_lives_bonus() -> void:
	Game.episode = 1
	Game.campaign_active = true
	Game.time_battle_selection = 1
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.main_level = 3
	Game.level_start_ms = Time.get_ticks_msec()
	Game.loadout = PackedInt32Array([8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 1000])
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 1000,
		"enemy_killed": 5,
		"total_enemy": 10,
		"cube_max": 2,
		"all_players_alive": true,
		"timer_countdown": 235,
		"lives": 2,
		"damage_by_bay": PackedInt32Array([0, 0, 0, 0, 0]),
		"damage_taken": 0,
		"damage_blocked": 0,
		"credits_start": 0
	}
	var deb: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.idle)).is_true()
	var log: Array = deb.glow_log
	var time_line := "%s %d" % [TyrianText.L("miscTextB_6"), 2300]
	var lives_line := "%s %d" % [TyrianText.L("miscTextB_7"), 2000]
	assert_array(log).override_failure_message("Time Bonus missing: %s" % [log]).contains([time_line])
	assert_array(log).override_failure_message("Lives Bonus missing: %s" % [log]).contains([lives_line])
	assert_array(log).override_failure_message("no Cubes block in Timed Battle").not_contains(
		[TyrianText.L("miscText_3")]
	)
	assert_int(int(Game.loadout[10])).override_failure_message("money = 1000 + 2300 + 2000").is_equal(5300)
	# mainint.c:2425-2429: "Credits" of the statistics screen, bonus included.
	_send(_key(KEY_SPACE))
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.stats_shown)).is_true()
	assert_array(deb._stats_lines).override_failure_message("credits %s" % [deb._stats_lines]).contains(
		[["ls_credits", "+5300"]]
	)
	Game.time_battle_selection = 0
	_send(_key(KEY_ESCAPE))


## M-08: two players, one money line per player (miscText 40 and 41 at y = 50 and 70,
## mainint.c:2218-2223), no Cubes block, "Press a key" at y = 150 (mainint.c:2336).
func test_two_player_debrief_has_both_cash_lines_and_press_key_at_150() -> void:
	Game.episode = 1
	Game.campaign_active = true
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.main_level = 3
	Game.level_start_ms = Time.get_ticks_msec()
	var modes := PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 0, 0])
	Game.campaign_modes = modes
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 1200,
		"cash2": 3400,
		"enemy_killed": 5,
		"total_enemy": 10,
		"cube_max": 0,
		"all_players_alive": true,
		"damage_by_bay": PackedInt32Array([0, 0, 0, 0, 0]),
		"damage_taken": 0,
		"damage_blocked": 0,
		"credits_start": 0
	}
	var deb: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.idle)).is_true()
	var log: Array = deb.glow_log
	assert_array(log).contains(
		["%s %d" % [TyrianText.L("miscText_40"), 1200], "%s %d" % [TyrianText.L("miscText_41"), 3400]]
	)
	assert_array(log).not_contains(["%s %d" % [TyrianText.L("miscText_27"), 1200]])
	assert_array(log).not_contains([TyrianText.L("miscText_3")])
	assert_int(deb.glow_ys[log.find(TyrianText.L("misc_press_key"))]).is_equal(150)
	Game.campaign_modes = PackedInt32Array()
	_send(_key(KEY_ESCAPE))


## mainint.c:2199-2202: the "Level completed" voice everywhere, except episode 4
## when the level timer (levelTimer) has reached 0: play_song(21).
func test_timed_episode_4_end_plays_song_21_not_voice() -> void:
	var D := preload("res://scripts/debrief.gd")
	assert_int(D.DEBRIEF_TIMED_SONG).is_equal(21)
	assert_bool(D.debrief_plays_song({"level_timer": true, "timer_countdown": 0}, 4)).is_true()
	assert_bool(D.debrief_plays_song({"level_timer": true, "timer_countdown": 3}, 4)).is_false()
	assert_bool(D.debrief_plays_song({"level_timer": false, "timer_countdown": 0}, 4)).is_false()
	assert_bool(D.debrief_plays_song({"level_timer": true, "timer_countdown": 0}, 1)).is_false()
