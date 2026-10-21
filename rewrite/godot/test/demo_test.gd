# Title demonstration (title_screen.c: "Demo" entry and 30 s
# of inactivity; mainint.c:load_next_demo / replay_demo_keys; tyrian2.c,
# end of demo): level and equipment from demo.N, keys replayed by the
# Rust simulation tick-exact (C golden master rewrite/golden/demo), any
# key returns to the title, demo_num cycles from 1 to 5.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const TITLE := preload("res://scripts/title.gd")


func before() -> void:
	EndlessSession.stop()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()


func after() -> void:
	TITLE.demo_idle_ms = 30000
	Game.demo_num = 0
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	TITLE.demo_idle_ms = 30000
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	await get_tree().process_frame


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


func _scene() -> Node:
	return Effects.current_game_scene()


func _sim(level: Node) -> Object:
	return level.get("sim")


## C state at program launch: demo_num 0, player[0].cash 0,
## initialDifficulty 0, no inherited friction.
func _fresh_program() -> void:
	Game.demo_num = 0
	Game.demo_cash = 0
	Game.c_initial_difficulty = 0
	Game.demo_carry = PackedInt32Array()
	Game.rng_seed = 1


func _title() -> Node:
	TITLE.intro_shown = true
	TITLE.move_tyrian_logo_up = false
	TITLE.selected = 0
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await _wait_until(func() -> bool: return title.idle, 15000)
	assert_bool(title.idle).override_failure_message("title never idle").is_true()
	return title


func _wait_demo_level() -> Node:
	await _wait_until(
		func() -> bool:
			var s := _scene()
			return s != null and s.scene_file_path == Game.SCENE_LEVEL and s.get("demo") == true and _sim(s) != null,
		15000
	)
	var level := _scene()
	assert_str(level.scene_file_path).is_equal(Game.SCENE_LEVEL)
	assert_bool(level.demo).is_true()
	return level


## State lines of a demo golden master (tick mask px py armor shield cash curloc ...).
func _golden_rows(n: int) -> Array:
	var path := ProjectSettings.globalize_path("res://").path_join("../golden/demo/demo-%d.gm" % n).simplify_path()
	var rows := []
	for line in FileAccess.get_file_as_string(path).split("\n"):
		if line.is_empty() or not line[0].is_valid_int():
			continue
		rows.append(line.split(" ", false))
	return rows


## Title "Demo" entry (Up, Up, Enter): demo.1 (TYRIAN, lvl 9,
## equipment from the header), keys from the file; the game follows the golden
## master of the C binary tick by tick (position, money, scroll).
func test_demo_menu_entry_plays_demo_1_like_c() -> void:
	_fresh_program()
	await _title()
	_send(C.key_event(KEY_UP))
	_send(C.key_event(KEY_UP))
	assert_int(TITLE.selected).is_equal(TITLE.DEMO)
	_send(C.key_event(KEY_ENTER))
	var level := await _wait_demo_level()
	assert_int(Game.demo_num).is_equal(1)
	assert_int(Game.episode).is_equal(1)
	assert_int(Game.lvl_file_num).is_equal(9)
	assert_str(Game.level_name).is_equal("TYRIAN")
	assert_bool(Game.campaign_active).is_false()
	var sim: Object = _sim(level)
	assert_bool(sim.is_demo()).is_true()
	# Equipment of demo.1: ship 1, front 7, rear 29, shield 6, generator 5, sidekicks 21 / 20.
	var lo: PackedInt32Array = sim.level_stats()["loadout"]
	assert_array(Array(lo.slice(0, 9))).is_equal([1, 7, 1, 29, 1, 6, 5, 21, 20])
	# The scene no longer plays by itself: ticks advanced here, compared with the C.
	level.interactive = false
	var rows := _golden_rows(1)
	var target := 1500
	while sim.tick() < target and not sim.level_ended():
		level.step_sim(0, 0)
	var row: PackedStringArray = rows[target - 1]
	var p: Dictionary = sim.player_dict(0)
	(
		assert_str("%d %d %d %d" % [int(p["x"]), int(p["y"]), int(p["cash"]), sim.cur_loc()])
		. override_failure_message("tick %d: Godot ≠ C (%s)" % [target - 1, " ".join(row)])
		. is_equal("%s %s %s %s" % [row[2], row[3], row[6], row[7]])
	)


## Title left without input: the demo starts (delay shortened for the test);
## a key stops it, back to the title with the logo moving back up
## (moveTyrianLogoUp); the next demo is demo.2 (SAVARA).
func test_idle_title_starts_demo_and_key_returns() -> void:
	_fresh_program()
	await _title()
	TITLE.demo_idle_ms = 400
	var level := await _wait_demo_level()
	assert_int(Game.demo_num).is_equal(1)
	var sim: Object = _sim(level)
	await _wait_until(func() -> bool: return sim.tick() > 10, 5000)
	assert_int(sim.tick()).is_greater(10)
	# Key: "input kills demo".
	TITLE.demo_idle_ms = 30000
	_send(C.key_event(KEY_SPACE))
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_TITLE, 5000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_TITLE)
	assert_bool(TITLE.move_tyrian_logo_up).override_failure_message("logo not restarted").is_true()
	var title := _scene()
	await _wait_until(func() -> bool: return title.idle, 15000)
	assert_bool(TITLE.move_tyrian_logo_up).is_false()
	# Next demo: demo.2.
	TITLE.demo_idle_ms = 400
	level = await _wait_demo_level()
	TITLE.demo_idle_ms = 30000
	assert_int(Game.demo_num).is_equal(2)
	assert_str(Game.level_name).is_equal("SAVARA")
	assert_int(Game.lvl_file_num).is_equal(12)
	# Gamepad button: also stops the demo.
	var ev := InputEventJoypadButton.new()
	ev.button_index = JOY_BUTTON_A
	ev.pressed = true
	_send(ev)
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_TITLE, 5000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_TITLE)


## An input at the title resets the delay to zero; demo_num loops 5 -> 1.
func test_title_input_resets_idle_and_demo_num_wraps() -> void:
	_fresh_program()
	var title: Node = await _title()
	TITLE.demo_idle_ms = 600
	for i in 4:
		await get_tree().create_timer(0.3).timeout
		_send(C.key_event(KEY_DOWN))
		_send(C.key_event(KEY_UP))
	assert_object(_scene()).override_failure_message("the demo started despite the keys").is_same(title)
	TITLE.demo_idle_ms = 30000
	Game.demo_num = 5
	Game.start_demo()
	await _wait_demo_level()
	assert_int(Game.demo_num).is_equal(1)
	assert_str(Game.level_name).is_equal("TYRIAN")


## End of the demo (demo.1 to the end, 6649 ticks in the C): same level end
## and same money as the C binary's golden master, back to the
## title without a "Game Over" screen; the player's money is kept for the
## next demo (the C does not reset player[0].cash to zero).
func test_demo_end_returns_to_title() -> void:
	_fresh_program()
	Game.start_demo()
	var level := await _wait_demo_level()
	var sim: Object = _sim(level)
	level.interactive = false
	var rows := _golden_rows(1)
	while not sim.level_ended() and sim.tick() < 10000:
		level.step_sim(0, 0)
	assert_bool(sim.level_ended()).is_true()
	assert_int(sim.tick()).is_equal(rows.size())
	assert_int(int(sim.player_dict(0)["cash"])).is_equal(int(rows[-1][6]))
	level.interactive = true
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_TITLE, 5000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_TITLE)
	assert_int(Game.demo_cash).is_equal(int(rows[-1][6]))
