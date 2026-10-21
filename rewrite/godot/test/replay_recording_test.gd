# Recording a replay in a level (replay.c: replay_start_recording,
# begin_recording, replay_capture_frame, replay_stop_recording). The F8 key
# (EKEY_REPLAY) arms recording for the next level, the next level
# opens it ("REC"), the same key closes it and writes `replays/YYYYMMDD-HHMMSS.rep`,
# which the Replays screen lists and which playback replays identically: two
# playbacks of the file give the same state fingerprint at every tick. The proof
# against the C binary's playback is in tyrian-golden (replays.rs), on the
# files stored in golden/replays/ (see replay_playback_test.gd).
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const REPLAYS_SCENE := "res://scenes/replays_screen.tscn"
const TICKS := 90

var _dir := ""


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()
	_dir = ProjectSettings.globalize_path("user://replay_recording_test")
	DirAccess.make_dir_recursive_absolute(_dir)
	Capture._options["replay-dir"] = _dir


func after() -> void:
	Capture._options.erase("replay-dir")
	_clean_dir()
	DirAccess.remove_absolute(_dir)
	ReplayRecording.armed = false
	Game.modes = PackedInt32Array()
	C.reset_defaults()
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	ReplayRecording.armed = false
	Game.modes = PackedInt32Array()
	C.reset_defaults()
	_clean_dir()
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	await get_tree().process_frame


func _clean_dir() -> void:
	for f in DirAccess.get_files_at(_dir):
		DirAccess.remove_absolute(_dir.path_join(f))


func _key(code: int, pressed: bool) -> void:
	var ev := C.key_event(code)
	ev.pressed = pressed
	Input.parse_input_event(ev)
	Input.flush_buffered_events()


func _press(code: int) -> void:
	_key(code, true)
	_key(code, false)


## Fresh level, no automatic physics: the test advances the ticks itself
## with the keys actually held (Game.read_input_mask).
func _start_level(episode := 1, lvl_file := 9) -> Node:
	Game.episode = episode
	Game.lvl_file_num = lvl_file
	# Difficulty and equipment set here: other suites leave them changed, and
	# the section written in the header depends on the difficulty (levelsN.dat).
	Game.difficulty = 2
	Game.loadout = PackedInt32Array(Game.NEW_GAME_LOADOUT)
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	level.set_physics_process(false)
	return level


## Plays `ticks` ticks: fire held, movement left then right.
func _play(level: Node, ticks: int) -> void:
	_key(KEY_SPACE, true)
	for t in ticks:
		if t == 0:
			_key(KEY_LEFT, true)
		elif t == ticks / 3:
			_key(KEY_LEFT, false)
			_key(KEY_RIGHT, true)
		elif t == 2 * ticks / 3:
			_key(KEY_RIGHT, false)
		level.step_sim(Game.read_input_mask())
	_key(KEY_SPACE, false)
	_key(KEY_RIGHT, false)
	_key(KEY_LEFT, false)


func _messages(level: Node) -> Array:
	var out := []
	for m in level._hud_vga._messages:
		if m[2] > 0:
			out.append(m[0])
	return out


## Rereads a .rep in a fresh simulation; returns the fingerprint of each tick.
func _play_back(path: String) -> PackedInt64Array:
	var sim: Node = ClassDB.instantiate("TyrianSim")
	add_child(sim)
	var hashes := PackedInt64Array()
	(
		assert_bool(sim.load_replay(Game.data_dir, path))
		. override_failure_message("playback impossible: %s" % sim.last_error())
		. is_true()
	)
	while not sim.level_ended() and hashes.size() < 100000:
		sim.step(0, 0)
		hashes.append(sim.state_hash())
	sim.queue_free()
	return hashes


func test_key_arms_and_disarms_for_next_level() -> void:
	var level := await _start_level()
	assert_bool(ReplayRecording.armed).is_false()
	_press(KEY_F10)
	assert_bool(ReplayRecording.armed).override_failure_message("the key does not arm").is_true()
	assert_array(_messages(level)).contains([TyrianText.L("msg_replay_armed")])
	# Armed, but the running level does not record: nothing in the simulation.
	assert_bool(level.sim.is_recording()).is_false()
	_press(KEY_F10)
	assert_bool(ReplayRecording.armed).override_failure_message("the second press does not disarm").is_false()
	assert_array(_messages(level)).contains([TyrianText.L("msg_replay_disarmed")])


func test_recording_writes_file_that_replays_identically() -> void:
	var level := await _start_level()
	_press(KEY_F10)
	assert_bool(ReplayRecording.armed).is_true()
	# Next level: the recording opens, "REC" is displayed.
	level = await _start_level()
	assert_bool(ReplayRecording.armed).override_failure_message("the request is not consumed").is_false()
	assert_bool(level.sim.is_recording()).override_failure_message("recording did not start").is_true()
	assert_str(level._hud_vga.replay_label).is_equal("REC")
	assert_array(_messages(level)).contains([TyrianText.L("msg_replay_started")])
	_play(level, TICKS)
	assert_int(level.sim.recording_frames()).is_equal(TICKS)
	# Same key: closes and writes the file.
	_press(KEY_F10)
	assert_bool(level.sim.is_recording()).is_false()
	assert_str(level._hud_vga.replay_label).is_empty()
	var path: String = level.last_recording_path
	assert_str(path).override_failure_message("no file written").is_not_empty()
	assert_str(path.get_base_dir()).is_equal(_dir)
	assert_bool(path.get_file().is_valid_filename()).is_true()
	var re := RegEx.create_from_string("^\\d{8}-\\d{6}\\.rep$")
	(
		assert_object(re.search(path.get_file()))
		. override_failure_message("unexpected name: %s" % path.get_file())
		. is_not_null()
	)
	# Header: episode 1, level played, normal game, TICKS frames.
	var h: PackedInt64Array = TyrianVga.replay_header(path)
	assert_int(h.size()).is_equal(5)
	assert_int(h[0]).is_equal(1)
	assert_int(h[3]).is_equal(0)
	assert_int(h[4]).is_equal(TICKS)
	# Two playbacks of the file: same fingerprint at every tick.
	var a := _play_back(path)
	var b := _play_back(path)
	assert_int(a.size()).is_equal(TICKS)
	assert_array(Array(a)).is_equal(Array(b))
	# The Replays screen lists it.
	var menu: Node = Game.goto_scene(REPLAYS_SCENE)
	for i in 600:
		if menu.get("idle") == true:
			break
		await get_tree().process_frame
	assert_array(menu._names).contains([path.get_file().get_basename()])


## An armed recording dies with the level: leaving the level closes it and
## writes what was captured (tyrian2.c: level end).
func test_leaving_the_level_saves_the_recording() -> void:
	var level := await _start_level()
	_press(KEY_F10)
	level = await _start_level()
	assert_bool(level.sim.is_recording()).is_true()
	_play(level, 40)
	level.stop_recording()
	assert_int(DirAccess.get_files_at(_dir).size()).is_equal(1)
	var path: String = _dir.path_join(DirAccess.get_files_at(_dir)[0])
	assert_int(TyrianVga.replay_header(path)[4]).is_equal(40)


## The modes that replay.c cannot replay (here Galaga) refuse
## recording at the start of the level, with a message, without a file.
func test_unreplayable_mode_refuses_recording() -> void:
	var level := await _start_level()
	_press(KEY_F10)
	Game.modes = PackedInt32Array([0, 1, 1, 0, 1, 0, 0, 0, 1, 1])
	level = await _start_level(1, 18)
	assert_bool(level.sim.is_recording()).is_false()
	assert_array(_messages(level)).contains([TyrianText.L("msg_replay_refused")])
	_play(level, 20)
	level.stop_recording()
	assert_int(DirAccess.get_files_at(_dir).size()).is_equal(0)
