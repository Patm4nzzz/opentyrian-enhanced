# Level ending of the side modes:
# - debriefing of a mode started from "Side modes" (campaign_active false, settings in
#   Game.modes): rows for both players, Time/Lives Bonus in Timed Battle, no Cubes block
#   in arcade (mainint.c:JE_endLevelAni);
# - quitting from the pause menu (tyrian2.c:330-336): Timed Battle = title, two players =
#   save 22 (not slot 11).
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	TyrianControls.install()
	TyrianControls.reset_defaults()


func after() -> void:
	Game.campaign_active = false
	Game.reset_game_modes()
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	Game.reset_game_modes()
	Game._two_player_last_level = {}
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _wait(cond: Callable, ms := 20000) -> bool:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame
	return cond.call()


## Side mode as modes_menu.gd sets it: modes only, no campaign.
func _open_side_mode_debrief(flags: Array) -> Node:
	Game.reset_game_modes()
	Game.episode = 1
	Game.modes = PackedInt32Array(flags + [0, 0, 0, 0])
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.main_level = 3
	Game.loadout = PackedInt32Array([8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 1000])
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 1500,
		"cash2": 2500,
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
	return deb


func test_side_mode_two_players_debrief_has_both_scores() -> void:
	var deb: Node = await _open_side_mode_debrief([1, 0, 0, 0, 0, 0])
	var log: Array = deb.glow_log
	assert_array(log).contains(
		["%s %d" % [TyrianText.L("miscText_40"), 1500], "%s %d" % [TyrianText.L("miscText_41"), 2500]]
	)
	assert_array(log).not_contains([TyrianText.L("miscText_3")])


func test_side_mode_arcade_debrief_has_no_cubes_block() -> void:
	var deb: Node = await _open_side_mode_debrief([0, 0, 1, 0, 0, 0])
	var log: Array = deb.glow_log
	assert_array(log).contains(["%s %d" % [TyrianText.L("miscText_27"), 1500]])
	assert_array(log).not_contains([TyrianText.L("miscText_3")])


func test_side_mode_timed_battle_debrief_has_time_and_lives_bonus() -> void:
	var deb: Node = await _open_side_mode_debrief([0, 0, 1, 0, 0, 1])
	var log: Array = deb.glow_log
	assert_array(log).contains(["%s %d" % [TyrianText.L("miscTextB_6"), 2300]])
	assert_array(log).contains(["%s %d" % [TyrianText.L("miscTextB_7"), 2000]])
	assert_array(log).not_contains([TyrianText.L("miscText_3")])


func test_abandon_in_timed_battle_returns_to_title() -> void:
	Game.start_new_game()
	Game.time_battle_selection = 1
	Game.campaign_abandon_level()
	await get_tree().process_frame
	var scene: Node = Effects.current_game_scene()
	(
		assert_bool(scene != null and scene.scene_file_path == Game.SCENE_TITLE)
		. override_failure_message("Timed Battle quit: title expected (%s)" % [scene.scene_file_path if scene else ""])
		. is_true()
	)
	assert_bool(Game.campaign_active).is_false()


func test_abandon_in_two_player_game_uses_two_player_save_and_keeps_both_alive() -> void:
	Game.start_new_game()
	var modes := PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2])
	Game.campaign_modes = modes
	Game.modes = modes
	Game._two_player_last_level = {
		"episode": 1,
		"section": 2,
		"last_level": "TYRIAN",
		"difficulty": 2,
		"initial_difficulty": 2,
		"cubes": 0,
		"loadout": PackedInt32Array([8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 777])
	}
	Game.campaign_abandon_level()
	await get_tree().process_frame
	var scene: Node = Effects.current_game_scene()
	(
		assert_bool(scene != null and scene.scene_file_path == Game.SCENE_SHOP)
		. override_failure_message(
			"two-player game quit: shop expected (%s)" % [scene.scene_file_path if scene else ""]
		)
		. is_true()
	)
	assert_bool(Game.two_player_game()).override_failure_message("the game stays two-player").is_true()
	assert_int(int(Game.loadout[10])).is_equal(777)
	assert_bool(bool(Game.debrief.get("all_players_alive", true))).is_true()
