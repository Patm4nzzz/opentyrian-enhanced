# Setup gameplay effects (gameplay_menu.c, graphics_menu.c, mutators_menu.c):
# Freeze Frames (hitstop.c), Damage Numbers (damagenumbers.c), Rumble
# (rumble.c), Explosions (particles.c:explosion_style) and Mutators. Each
# option is set with real keys in Setup, saved to
# user://setup.cfg, then its effect is measured in a level played tick by
# tick (same inputs, same seed).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")
const C := preload("res://scripts/controls.gd")

## Permanent fire and left/right sweep (tyrian-sim, tests/determinism.rs).
const FIRE := 16
const LEFT := 4
const RIGHT := 8


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	Game.mutators = 0
	Effects.kill_slowmo = -1.0
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	await get_tree().process_frame
	await get_tree().process_frame


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()


# --- Setup driven by keyboard ----------------------------------------------------


## Key pressed then released, through the real input path
## (Effects passes it to the game screen).
func _key(code: int) -> void:
	var ev := C.key_event(code)
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEventKey = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()
	await get_tree().process_frame


func _open_setup() -> Node:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	for i in 300:
		await get_tree().process_frame
		if setup.idle:
			break
	assert_bool(setup.idle).override_failure_message("Setup never ready").is_true()
	return setup


func _current_label(setup: Node) -> String:
	var m: Dictionary = setup.menus.current()
	return m.items[m.sel].label


## Goes down to the `label` row of the current menu.
func _goto_row(setup: Node, label: String) -> void:
	for i in 20:
		if _current_label(setup) == label:
			return
		await _key(KEY_DOWN)
	assert_str(_current_label(setup)).is_equal(label)


func _labels(items: Array) -> Array:
	return items.map(func(it): return it.label)


## Rereads the saved file (load_settings) after resetting to defaults.
func _reload_settings() -> void:
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	SetupScreen.load_settings()


# --- Level played tick by tick ---------------------------------------------------


func _sim() -> Object:
	for n in Effects.game_viewport.find_children("TyrianSim", "", true, false):
		return n
	return null


## Episode 1 level (TYRIAN), fixed seed, driven by step_sim alone.
## C equipment (mainint.c:JE_initPlayerData) like the
## determinism tests; `level_num`/`power` for a level that fires harder.
func _start_level(seed := 7, level_num := 9, power := 1) -> Node:
	Game.episode = 1
	Game.lvl_file_num = level_num
	Game.start_cur_loc = 0
	Game.rng_seed = seed
	Game.difficulty = 2
	Game.loadout = PackedInt32Array([1, 4, power, 0, 1, 4, 2, 0, 0, 0, 10000])
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	level.interactive = false
	await get_tree().process_frame
	return level


func _mask(t: int) -> int:
	return FIRE | (LEFT if (t / 90) % 2 == 0 else RIGHT)


func _is_expected_rumble(call: Array) -> bool:
	return is_equal_approx(call[1], 0.2) and is_equal_approx(call[2], 0.2) and is_equal_approx(call[3], 0.1)


## Plays `ticks` ticks; `each` (optional) receives (tick, sim) after each one.
func _play(level: Node, ticks: int, each := Callable()) -> void:
	for t in ticks:
		level.step_sim(_mask(t))
		if each.is_valid():
			each.call(t, level.sim)


# --- Tests ---


## Freeze Frames: 3-frame freeze on a big target (hitstop.c), the
## enemies do not move during the freeze; disabled, no freeze and
## simulation identical to that without any effect (golden master).
func test_freeze_frames_freeze_enemies() -> void:
	var runs := {}
	for on in [true, false]:
		SetupScreen.settings.hitstop_enabled = on
		# Retro+ style (C default): the Modern style changes the simulation
		# (angle sprites of big explosions), not the freeze.
		SetupScreen.settings.explosion_style = 1
		SetupScreen.apply_settings()
		# Level 3 of episode 1, powerful weapon: big hits arrive
		# early (measured: first freeze at tick 111).
		var level: Node = await _start_level(7, 3, 11)
		var frozen_updates := [0]
		var seq := []
		for t in 2500:
			level.step_sim(_mask(t))
			var f: int = level.sim.fx_stats()[5]
			if f > 0:
				frozen_updates[0] += level.sim.fx_stats()[6]
			if f > 0 or (seq.size() > 0 and seq.back() > 0):
				seq.append(f)
		runs[on] = [level.sim.fx_stats()[6], seq, level.sim.state_hash()]
		Game.goto_scene(Game.SCENE_TITLE)
		await get_tree().process_frame
	prints("freeze: frozen updates", runs[true][0], "sequence", runs[true][1].slice(0, 12))
	assert_int(runs[true][0]).override_failure_message("no frozen enemy").is_greater(0)
	assert_int(runs[false][0]).is_equal(0)
	# A 2- or 3-frame freeze counts down one frame per tick down to 0.
	var seq: Array = runs[true][1]
	assert_bool(seq.has(3) or seq.has(2)).override_failure_message("no 2- or 3-frame freeze").is_true()
	var found_decrement := false
	for i in seq.size() - 1:
		if seq[i] >= 2 and seq[i + 1] == seq[i] - 1:
			found_decrement = true
	(
		assert_bool(found_decrement)
		. override_failure_message("the freeze does not count down by one frame per tick")
		. is_true()
	)
	assert_int(runs[true][2]).override_failure_message("the freeze changed nothing").is_not_equal(runs[false][2])
	# Without hitstop: same game as a Sim without effects (tools, golden master).
	var bare: Object = ClassDB.instantiate("TyrianSim")
	bare.set_loadout(Game.loadout)
	assert_bool(bare.load_level(Game.data_dir, 1, 3, 2, 7)).is_true()
	for t in 2500:
		bare.step(_mask(t), 0)
	assert_int(bare.state_hash()).override_failure_message("effects without hitstop: simulation changed").is_equal(
		runs[false][2]
	)
	bare.free()


## Damage Numbers: numbers shown on impacts, shot tint in
## colored mode, 15 (white) in "on" mode, none in "off"; drawn in
## the 8-bit image of the play area.


## Damage Numbers: numbers shown on impacts, shot tint in
## colored mode, 15 (white) in "on" mode, none in "off"; drawn in
## the 8-bit image of the play area.
func test_damage_numbers_shown_with_color() -> void:
	var seen := {}
	for mode in [0, 1, 2]:
		SetupScreen.settings.damage_numbers_enabled = mode > 0
		SetupScreen.settings.colored_damage_numbers = mode == 2
		SetupScreen.settings.explosion_style = 0
		SetupScreen.apply_settings()
		var level: Node = await _start_level()
		var hues := {}
		var count := [0]
		await _play(
			level,
			600,
			func(_t: int, sim: Object) -> void:
				var d: PackedInt32Array = sim.damage_numbers()
				count[0] += d.size() / 4
				for k in range(0, d.size(), 4):
					hues[d[k + 3]] = true
		)
		seen[mode] = [count[0], hues.keys()]
		Game.goto_scene(Game.SCENE_TITLE)
		await get_tree().process_frame
	prints("damage numbers (frames×numbers, tints):", seen)
	assert_int(seen[0][0]).is_equal(0)
	assert_int(seen[1][0]).is_greater(0)
	assert_array(seen[1][1]).contains_exactly([15])
	assert_int(seen[2][0]).is_greater(0)
	assert_bool(seen[2][1].any(func(h): return h != 15)).override_failure_message("no shot tint").is_true()


## F7 in a level (input_map.c:input_action_toggle_damage_numbers): real
## key, setting toggled and saved.


## F7 in a level (input_map.c:input_action_toggle_damage_numbers): real
## key, setting toggled and saved.
func test_f7_toggles_damage_numbers() -> void:
	var level: Node = await _start_level()
	level.interactive = true
	await _key(KEY_F7)
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_false()
	_reload_settings()
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_false()


## Rumble: Input.start_joy_vibration intercepted; enemy killed 0.2 / 100 ms
## (rumble.c:on_enemy_killed), nothing when the option is off.


## Rumble: Input.start_joy_vibration intercepted; enemy killed 0.2 / 100 ms
## (rumble.c:on_enemy_killed), nothing when the option is off.
func test_rumble_calls_vibration() -> void:
	for on in [true, false]:
		SetupScreen.settings.rumble_enabled = on
		SetupScreen.apply_settings()
		var level: Node = await _start_level()
		var calls := []
		level.joy_devices = func() -> Array: return [0]
		level.joy_vibrate = func(device: int, weak: float, strong: float, duration: float) -> void:
			calls.append([device, weak, strong, duration])
		await _play(level, 900)
		if on:
			prints("rumble calls:", calls.size(), calls.slice(0, 3))
			assert_int(calls.size()).is_greater(0)
			assert_bool(calls.any(_is_expected_rumble)).is_true()
		else:
			assert_int(calls.size()).is_equal(0)
		Game.goto_scene(Game.SCENE_TITLE)
		await get_tree().process_frame


## Setting changed during the level: applied right away (C globals).


## Setting changed during the level: applied right away (C globals).
func test_setting_applied_to_running_level() -> void:
	SetupScreen.settings.explosion_style = 0
	SetupScreen.apply_settings()
	var level: Node = await _start_level()
	await _play(level, 300)
	var before: int = level.sim.fx_stats()[1]
	SetupScreen.settings.explosion_style = 2
	SetupScreen.apply_settings()
	var fb := [0]
	await _play(level, 600, func(_t: int, sim: Object) -> void: fb[0] = maxi(fb[0], sim.fx_stats()[1]))
	assert_int(before).is_equal(0)
	assert_int(fb[0]).is_greater(0)
