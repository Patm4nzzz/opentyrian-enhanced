# Development console (dev_console.c): Ctrl+Shift+G/H/C/N/U, active
# only with --devmode (params.c). Each key is sent as a real keyboard
# event and its effect is read in the simulation at the next tick.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const ES := preload("res://scripts/endless_session.gd")
const DEV := preload("res://scripts/dev_console.gd")


func before() -> void:
	ES.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()


func after() -> void:
	DEV.set_available(false)
	DEV.god_mode = false
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	DEV.set_available(false)
	DEV.god_mode = false
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	await get_tree().process_frame


func _sim() -> Object:
	for n in Effects.game_viewport.find_children("TyrianSim", "", true, false):
		return n
	return null


func _wait_until(cond: Callable, ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame


func _wait_ticks(sim: Object, n: int) -> void:
	var t: int = sim.tick()
	await _wait_until(func() -> bool: return sim.tick() >= t + n, 3000)


## Ctrl+Shift+letter, modifiers held like on the keyboard.
func _cheat(code: int, ctrl := true, shift := true) -> void:
	var ev := C.key_event(code)
	ev.ctrl_pressed = ctrl
	ev.shift_pressed = shift
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEventKey = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func _start_level() -> Node:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	var sim: Object = _sim()
	await _wait_until(func() -> bool: return sim != null and sim.tick() >= 20, 5000)
	return level


## Without --devmode: no key acts (dev_console_available false).
func test_cheats_need_devmode() -> void:
	DEV.set_available(false)
	await _start_level()
	var sim: Object = _sim()
	var cash := int(sim.player_dict(0).cash)
	_cheat(KEY_C)
	_cheat(KEY_G)
	await _wait_ticks(sim, 3)
	assert_int(int(sim.player_dict(0).cash)).override_failure_message("+10000 without --devmode").is_equal(cash)
	assert_bool(sim.dev_god_mode()).is_false()


func test_cash_armor_and_god_mode() -> void:
	DEV.set_available(true)
	await _start_level()
	var sim: Object = _sim()
	var cash := int(sim.player_dict(0).cash)
	_cheat(KEY_C)
	await _wait_ticks(sim, 2)
	assert_int(int(sim.player_dict(0).cash)).override_failure_message("Ctrl+Shift+C").is_equal(cash + 10000)
	# Ctrl alone or Shift alone: nothing (mod_pressed requires both).
	_cheat(KEY_C, true, false)
	_cheat(KEY_C, false, true)
	await _wait_ticks(sim, 2)
	assert_int(int(sim.player_dict(0).cash)).is_equal(cash + 10000)
	_cheat(KEY_H)
	await _wait_ticks(sim, 1)
	assert_int(int(sim.player_dict(0).armor)).override_failure_message("Ctrl+Shift+H").is_equal(28)
	_cheat(KEY_G)
	assert_bool(sim.dev_god_mode()).override_failure_message("Ctrl+Shift+G").is_true()
	assert_bool(DEV.god_mode).is_true()
	var x := int(sim.player_dict(0).x)
	await _wait_ticks(sim, 10)
	assert_int(int(sim.player_dict(0).x)).override_failure_message("constantPlay: ship drift").is_less(x)
	assert_int(sim.player_shots().size()).override_failure_message("constantPlay: constant fire").is_greater(0)
	# Kept at the next level (C global), turned off by a second press.
	await _start_level()
	sim = _sim()
	assert_bool(sim.dev_god_mode()).override_failure_message("god mode lost on the next level").is_true()
	_cheat(KEY_G)
	assert_bool(sim.dev_god_mode()).is_false()


## Ctrl+Shift+N: successful level end (not abandonment): the level ends
## with the player alive.
func test_complete_level() -> void:
	DEV.set_available(true)
	await _start_level()
	Game.debrief = {}
	_cheat(KEY_N)
	# Non-campaign level finished alive: the scene goes to the debriefing, which
	# receives the summary (the simulation is freed with the level).
	await _wait_until(func() -> bool: return not Game.debrief.is_empty(), 10000)
	assert_bool(Game.debrief.is_empty()).override_failure_message("Ctrl+Shift+N does not end the level").is_false()
	assert_bool(bool(Game.debrief.get("all_players_alive", false))).is_true()
	Game.debrief = {}


## Ctrl+Shift+U: all achievements unlocked and saved (JE_saveConfiguration).
func test_unlock_all_achievements() -> void:
	DEV.set_available(true)
	var level: Node = await _start_level()
	var path: String = ProjectSettings.globalize_path("user://dev_console_test.dat")
	DirAccess.remove_absolute(path)
	assert_bool(_sim().dev_unlock_all_achievements(path)).is_true()
	var unlocked: PackedByteArray = ClassDB.class_call_static("TyrianVga", "achievements_unlocked_from_dat", path)
	assert_int(unlocked.size()).is_greater(0)
	assert_int(unlocked.count(0)).override_failure_message("achievements still locked").is_equal(0)
	DirAccess.remove_absolute(path)
	# By key: writes the player's file (user://opentyrian.dat).
	var dat: String = level._ach_dat_path()
	_cheat(KEY_U)
	unlocked = ClassDB.class_call_static("TyrianVga", "achievements_unlocked_from_dat", dat)
	assert_int(unlocked.size()).is_greater(0)
	assert_int(unlocked.count(0)).override_failure_message("Ctrl+Shift+U").is_equal(0)
	DirAccess.remove_absolute(dat)
