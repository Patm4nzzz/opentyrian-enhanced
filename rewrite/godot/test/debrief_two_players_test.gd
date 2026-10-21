# M-08: two-player debriefing (mainint.c:2218-2223 and 2336): one score line per
# player (miscText[40 + i], y = 50 + 20 i) in place of "Current Score", and
# "Press a key" at y = 150 instead of 160.
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()
	TyrianControls.reset_defaults()


func after_test() -> void:
	Game.campaign_modes = PackedInt32Array()
	Game.campaign_active = false
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _wait(cond: Callable, ms := 20000) -> bool:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame
	return cond.call()


func _open_debrief(two_players: bool, cubes := 0) -> Node:
	Game.episode = 1
	Game.campaign_active = true
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.main_level = 3
	Game.campaign_modes = PackedInt32Array([1 if two_players else 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 1500,
		"cash2": 2500,
		"enemy_killed": 5,
		"total_enemy": 10,
		"cube_max": cubes,
		"all_players_alive": true,
		"damage_by_bay": PackedInt32Array([0, 0, 0, 0, 0]),
		"damage_taken": 0,
		"damage_blocked": 0,
		"credits_start": 0
	}
	var deb: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	assert_bool(await _wait(func(): return is_instance_valid(deb) and deb.idle)).is_true()
	return deb


## Bright lines of the zone y0..y1 (x 90..230) in the debriefing's 8-bit image.
func _lit_rows(deb: Node, y0: int, y1: int) -> int:
	var img: Image = deb.vga.image(0)
	var rows := 0
	for y in range(y0, y1):
		for x in range(90, 230):
			if img.get_pixel(x, y).v > 0.5:
				rows += 1
				break
	return rows


func test_two_player_debrief_shows_both_scores_and_raises_press_a_key() -> void:
	var deb: Node = await _open_debrief(true)
	var log: Array = deb.glow_log
	assert_array(log).contains(
		["%s %d" % [TyrianText.L("miscText_40"), 1500], "%s %d" % [TyrianText.L("miscText_41"), 2500]]
	)
	assert_array(log).not_contains(["%s %d" % [TyrianText.L("miscText_27"), 1500]])
	# "Press a key" at 150: the line 150..160 is lit, the zone 164..172 stays black.
	assert_int(_lit_rows(deb, 150, 160)).is_greater(3)
	assert_int(_lit_rows(deb, 164, 172)).is_equal(0)


func test_one_player_debrief_keeps_single_score_and_press_a_key_at_160() -> void:
	var deb: Node = await _open_debrief(false)
	var log: Array = deb.glow_log
	assert_array(log).contains(["%s %d" % [TyrianText.L("miscText_27"), 1500]])
	assert_array(log).not_contains(["%s %d" % [TyrianText.L("miscText_41"), 2500]])
	assert_int(_lit_rows(deb, 148, 156)).is_equal(0)
	assert_int(_lit_rows(deb, 160, 170)).is_greater(3)


## The maintainer's decision (2026-10-10, divergences no. 51): each line of the first screen is centered
## on the 320 px with its real width (x = (320 - width) / 2), one and two players.
func test_every_debrief_line_is_centred() -> void:
	for two_players in [false, true]:
		var deb: Node = await _open_debrief(two_players)
		var log: Array = deb.glow_log
		assert_int(log.size()).is_greater(3)
		for i in log.size():
			var expected := (320 - TyrianText.width(log[i], deb.GLOW_FONT)) / 2
			(
				assert_int(deb.glow_xs[i])
				. override_failure_message("row '%s' at x %d, expected %d" % [log[i], deb.glow_xs[i], expected])
				. is_equal(expected)
			)


## Same decision: the row of cubes is centered as a block (30 px step, y 135), from 1 to 4 cubes.
func test_cube_row_is_centred_for_every_count() -> void:
	for count in range(1, 5):
		var deb: Node = await _open_debrief(false, count)
		var cube_width: int = deb.vga.sprite_size(deb.OPTION_SHAPES, 25).x
		var row_width := 30 * (count - 1) + cube_width
		assert_int(deb.cube_xs.size()).is_equal(count)
		assert_int(deb.cube_xs[0]).is_equal((320 - row_width) / 2)
		for i in count:
			assert_int(deb.cube_xs[i]).is_equal(deb.cube_xs[0] + 30 * i)
		# Equal left and right margin (to within 1 px): the row is properly centered.
		assert_int(abs(deb.cube_xs[0] - (320 - (deb.cube_xs[count - 1] + cube_width)))).is_less_equal(1)
		await after_test()
