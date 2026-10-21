# Playback of a replay chosen in the Replays menu (title_screen.c:
# replay_browser, Enter): the file goes through the level scene
# (TyrianSim.load_replay, same engine as --rep= and as the Rust test
# tyrian-golden/tests/replays.rs) and must reach the same final state as the
# C version's playback (golden master NAME.play.gm), then hand control back
# to the title like tyrian2.c (replay_stop).
extends GdUnitTestSuite

const REPLAYS_SCENE := "res://scenes/replays_screen.tscn"

var _dir := ""


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	_dir = ProjectSettings.globalize_path("user://replay_playback_test")
	DirAccess.make_dir_recursive_absolute(_dir)
	Capture._options["replay-dir"] = _dir


func after() -> void:
	Capture._options.erase("replay-dir")
	for f in DirAccess.get_files_at(_dir):
		DirAccess.remove_absolute(_dir.path_join(f))
	DirAccess.remove_absolute(_dir)
	Game.replay_file = ""
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	for f in DirAccess.get_files_at(_dir):
		DirAccess.remove_absolute(_dir.path_join(f))
	await get_tree().process_frame


func _golden_dir() -> String:
	return ProjectSettings.globalize_path("res://").path_join("../golden/replays")


## Last state line of the golden master (18 columns of write_tick) and
## tick count.
func _golden_end(name: String) -> Array:
	var last := ""
	var ticks := 0
	for line in FileAccess.get_file_as_string(_golden_dir().path_join(name + ".play.gm")).split("\n"):
		if not line.is_empty() and line[0].is_valid_int():
			last = line
			ticks += 1
	return [" ".join(Array(last.split(" ", false)).slice(0, 18)), ticks]


func _wait_idle(scene: Node) -> void:
	for i in 600:
		if scene.get("idle") == true:
			return
		await get_tree().process_frame


## Opens the Replays menu, chooses `name` and confirms; returns the level scene.
func _play_from_menu(name: String, others: Array) -> Node:
	for n in [name] + others:
		DirAccess.copy_absolute(_golden_dir().path_join(n + ".rep"), _dir.path_join(n + ".rep"))
	var menu: Node = Game.goto_scene(REPLAYS_SCENE)
	await _wait_idle(menu)
	# List sorted by descending name (replay_browser).
	var row: int = menu._names.find(name)
	assert_int(row).is_greater_equal(0)
	for i in row:
		menu.handle_action(TyrianControls.DOWN)
	menu.handle_action(TyrianControls.ACCEPT)
	await get_tree().process_frame
	var level: Node = Effects.current_game_scene()
	assert_str(level.scene_file_path).is_equal(Game.SCENE_LEVEL)
	return level


func _check_replay(name: String, others: Array) -> void:
	var level: Node = await _play_from_menu(name, others)
	assert_bool(level.replaying).is_true()
	assert_object(level.sim).is_not_null()
	# File consumed: the next game will not reread it.
	assert_str(Game.replay_file).is_empty()
	# Simulation driven tick by tick (like the capture tool): the
	# masks passed are ignored, the inputs come from the .rep.
	level.interactive = false
	var expected: Array = _golden_end(name)
	level.advance(int(expected[1]) + 10, func(_t: int) -> int: return 0x10)
	assert_bool(level.sim.level_ended()).override_failure_message("%s: playback not finished" % name).is_true()
	assert_int(level.sim.tick()).is_equal(expected[1])
	assert_str(level.sim.frame_line()).override_failure_message("%s: final state differs from the C" % name).is_equal(
		expected[0]
	)
	# End of playback: back to the title (tyrian2.c, replay_stop).
	level.interactive = true
	for i in 10:
		await get_tree().physics_frame
	assert_str(Effects.current_game_scene().scene_file_path).is_equal(Game.SCENE_TITLE)


## Exhausted file ("end replay"): chosen at the second rank of the list.
func test_replay_from_menu_reaches_c_end_state_replay_exhausted() -> void:
	await _check_replay("e1-tyrian-sweep", ["e2-first-sweep"])


## Level end ("end level"), other episode.
func test_replay_from_menu_reaches_c_end_state_level_end() -> void:
	await _check_replay("e2-first-sweep", [])


## Two players (game_mode 2): same path.
func test_replay_from_menu_two_players() -> void:
	await _check_replay("e1-2p-sweep", [])


## One-player arcade (game_mode 1, onePlayerAction): read like the others.
func test_replay_from_menu_arcade_one_player() -> void:
	await _check_replay("e1-arcade-sweep", [])


## Replay without a frame: refused in the list ("replay_empty"), we stay there.
func test_empty_replay_is_refused() -> void:
	var src := FileAccess.get_file_as_bytes(_golden_dir().path_join("e2-first-sweep.rep"))
	# v4 header (replay.c): 18 + mode 1 + items 13 + cash 4 + mutators 8
	# + P2 items 13 = 57 bytes, then the frame count (u32): set to 0,
	# frames cut.
	const FRAME_COUNT_AT := 57
	var empty := src.slice(0, FRAME_COUNT_AT + 4)
	empty.encode_u32(FRAME_COUNT_AT, 0)
	var f := FileAccess.open(_dir.path_join("empty.rep"), FileAccess.WRITE)
	f.store_buffer(empty)
	f.close()
	var menu: Node = Game.goto_scene(REPLAYS_SCENE)
	await _wait_idle(menu)
	assert_int(menu._names.size()).is_equal(1)
	menu.handle_action(TyrianControls.ACCEPT)
	await get_tree().process_frame
	assert_object(Effects.current_game_scene()).is_same(menu)
	assert_str(menu._status).is_equal(TyrianText.L("replay_empty"))
	assert_str(Game.replay_file).is_empty()


## enemy_render.c:221: JE_textShade(..., "PLAY", ...) hard-coded, outside the locale;
## in German as elsewhere, playback shows "PLAY" (the recording's "REC"
## is too).
func test_replay_label_is_play_in_every_language() -> void:
	var saved := TyrianText.language
	TyrianText.language = "de"
	var level: Node = await _play_from_menu("e2-first-sweep", [])
	TyrianText.language = saved
	assert_str(level._hud_vga.replay_label).is_equal("PLAY")
