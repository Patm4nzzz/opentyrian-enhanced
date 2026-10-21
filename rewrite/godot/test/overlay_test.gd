# F9 / F10 during a level: the options
# screen opens on top of the game, the simulation is suspended (tick and
# state fingerprint unchanged while it is displayed), then the game resumes
# as is on close (Esc).
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


## Empties the game scene and lets the deferred free through: gdUnit4
## counts orphan nodes as soon as the test case ends.
func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	await get_tree().process_frame
	await get_tree().process_frame


func _sim() -> Object:
	for n in Effects.game_viewport.find_children("TyrianSim", "", true, false):
		return n
	return null


## Wait bounded in real time (not in frames: see the review skill).
func _wait_ms(ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while Time.get_ticks_msec() < end:
		await get_tree().physics_frame


func _wait_until(cond: Callable, ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().physics_frame


func _press(code: int) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = code
	ev.keycode = code
	ev.pressed = true
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up := ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func _overlay_round_trip(scene_path: String) -> void:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	var sim: Object = _sim()
	assert_object(sim).override_failure_message("no TyrianSim").is_not_null()
	if sim == null:
		return
	# The game is running: a few ticks played.
	await _wait_until(func() -> bool: return sim.tick() >= 20, 5000)
	assert_int(sim.tick()).override_failure_message("the level does not advance").is_greater_equal(20)

	Effects.open_options(scene_path)
	var overlay: Node = Effects.current_game_scene()
	assert_str(overlay.scene_file_path).is_equal(scene_path)
	assert_bool(Effects.level_paused()).is_true()
	assert_bool(is_instance_valid(level) and level.is_inside_tree()).override_failure_message("level left").is_true()
	var tick: int = sim.tick()
	var hash: int = sim.state_hash()
	# Screen open 500 ms (~17 ticks at 35 Hz): no tick played.
	await _wait_ms(500)
	assert_int(sim.tick()).override_failure_message("tick advanced during pause").is_equal(tick)
	assert_int(sim.state_hash()).override_failure_message("state modified during pause").is_equal(hash)

	# Esc: back to the game, in the same state.
	_press(KEY_ESCAPE)
	assert_object(Effects.current_game_scene()).is_same(level)
	assert_int(sim.tick()).is_equal(tick)
	assert_int(sim.state_hash()).is_equal(hash)
	# Then the game resumes (and Esc did not quit it).
	await _wait_until(func() -> bool: return sim.tick() > tick + 5, 5000)
	(
		assert_bool(is_instance_valid(level) and level.is_inside_tree())
		. override_failure_message("Escape left the level")
		. is_true()
	)
	assert_int(sim.tick()).override_failure_message("the game did not resume").is_greater(tick + 5)
	assert_bool(Effects.level_paused()).is_false()


## In a level, F9 and F10 are the C's keys (quick load, replay): they
## no longer open an options screen.
func test_f9_f10_in_level_do_not_open_option_screens() -> void:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	var sim: Object = _sim()
	await _wait_until(func() -> bool: return sim.tick() >= 10, 5000)
	_press(KEY_F9)
	_press(KEY_F10)
	assert_object(Effects.current_game_scene()).is_same(level)
	assert_bool(Effects.level_paused()).is_false()
	ReplayRecording.armed = false


func test_f9_controls_over_level() -> void:
	await _overlay_round_trip("res://scenes/controls_options.tscn")


func test_f10_display_over_level() -> void:
	await _overlay_round_trip("res://scenes/display_options.tscn")


## At the title, F10 replaces the screen (Esc: back to the title).
func test_f10_on_title_replaces_screen() -> void:
	Game.goto_scene(Game.SCENE_TITLE)
	_press(KEY_F10)
	assert_bool(Effects.level_paused()).is_false()
	assert_str(Effects.current_game_scene().scene_file_path).is_equal("res://scenes/display_options.tscn")
	assert_int(Effects.game_viewport.get_child_count()).is_equal(1)


## Elsewhere (shop, menus), F9 / F10 open on top of the screen and Esc
## goes back to it: from the shop, the old replacement sent back to the title and
## the game in progress was lost (seen on the maintainer's Windows).
func _f_key_round_trip_over(scene_path: String, key: int) -> void:
	if scene_path == Game.SCENE_SHOP:
		# Shop of a campaign in progress (episode 1, first section).
		Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 15267])
		Game.campaign_active = true
		Game.episode = 1
		Game.campaign_modes = PackedInt32Array([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
	var under: Node = Game.goto_scene(scene_path)
	await get_tree().process_frame
	_press(key)
	assert_object(Effects.current_game_scene()).override_failure_message("no screen open").is_not_same(under)
	(
		assert_bool(is_instance_valid(under) and under.is_inside_tree())
		. override_failure_message("original screen replaced")
		. is_true()
	)
	_press(KEY_ESCAPE)
	await get_tree().process_frame
	(
		assert_object(Effects.current_game_scene())
		. override_failure_message("Escape did not return to the original screen")
		. is_same(under)
	)
	if not is_instance_valid(under):
		return
	await _wait_until(
		func() -> bool: return is_instance_valid(under) and under.process_mode == Node.PROCESS_MODE_INHERIT, 2000
	)
	assert_int(under.process_mode).override_failure_message("original screen left suspended").is_equal(
		Node.PROCESS_MODE_INHERIT
	)
	Game.campaign_active = false


func test_f10_in_shop_returns_to_shop() -> void:
	await _f_key_round_trip_over(Game.SCENE_SHOP, KEY_F10)


func test_f9_in_shop_returns_to_shop() -> void:
	await _f_key_round_trip_over(Game.SCENE_SHOP, KEY_F9)


## F10 on the Display screen already open: nothing more (no stacked screen).
func test_f10_twice_opens_one_screen() -> void:
	Game.goto_scene(Game.SCENE_TITLE)
	_press(KEY_F10)
	var first: Node = Effects.current_game_scene()
	_press(KEY_F10)
	assert_object(Effects.current_game_scene()).is_same(first)
	assert_int(Effects.game_viewport.get_child_count()).is_equal(1)


func test_f10_in_menu_returns_to_menu() -> void:
	await _f_key_round_trip_over(Game.SCENE_MODES, KEY_F10)
