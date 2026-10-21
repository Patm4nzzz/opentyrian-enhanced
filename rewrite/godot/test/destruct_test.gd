# Destruct mini-game (destruct.c, ADR 0132): BEHAVIOR test with real
# keyboard events, not a proof that the code exists.
#
# The flow (introduction -> mode menu -> round -> exit) is that of
# destruct.c:JE_destructGame / JE_destructMain; the logic is in tyrian-sim
# (destruct.rs), covered by cargo test, and image parity with the C by
# rewrite/scripts/destruct_parity.sh.
extends GdUnitTestSuite

const SCENE := "res://scenes/destruct.tscn"
## destruct.c : MODE_5CARDWAR, MODE_TRADITIONAL.
const MODE_5CARDWAR := 0


func before_test() -> void:
	# The hidden configuration file is specific to each test: it is read
	# then rewritten by the screen (destruct.c:JE_destructGame).
	if FileAccess.file_exists("user://destruct.cfg"):
		DirAccess.remove_absolute(ProjectSettings.globalize_path("user://destruct.cfg"))


func after() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
	Music.release_streams()
	Sfx.release_streams()


func _press(key: int, held := false) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = key as Key
	ev.keycode = key as Key
	ev.pressed = true
	Input.parse_input_event(ev)
	if not held:
		var up := InputEventKey.new()
		up.physical_keycode = key as Key
		up.keycode = key as Key
		up.pressed = false
		Input.parse_input_event(up)


func _key(key: int) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = key as Key
	ev.keycode = key as Key
	ev.pressed = true
	Effects.game_viewport.push_input(ev)


func test_destruct_enters_rounds_and_quits() -> void:
	var scene: Node = auto_free(load(SCENE).instantiate())
	Effects.game_viewport.add_child(scene)
	# The screen creates the GDExtension class TyrianDestruct (tyrian-sim:destruct).
	await get_tree().process_frame
	assert_object(scene.game).override_failure_message("TyrianDestruct unavailable").is_not_null()

	# Introduction (destruct.c:JE_introScreen): wait until the scene is
	# in the "intro" phase (fade finished, _wait_newkey pending), then
	# send Enter to advance. Sending the key before _wait_newkey
	# is reached would not trigger the advance.
	await _wait_phase(scene, "intro")
	# The configuration file is created on load (before the intro).
	(
		assert_bool(FileAccess.file_exists("user://destruct.cfg"))
		. override_failure_message("destruct.cfg not created")
		. is_true()
	)
	_key(KEY_ENTER)
	await _wait_phase(scene, "menu")

	# Mode menu: one key per notch, Enter chooses.
	assert_int(scene.menus.selection()).is_equal(0)
	_key(KEY_DOWN)
	await get_tree().process_frame
	assert_int(scene.menus.selection()).is_equal(1)
	_key(KEY_UP)
	await get_tree().process_frame
	_key(KEY_ENTER)
	await _wait_phase(scene, "play")

	# Round: a shot from the left player (X by default, destruct.c:defaultKeyConfig).
	_key(KEY_X)
	for i in 3:
		await get_tree().process_frame
	# Esc returns to the mode menu (STATE_INIT).
	_key(KEY_ESCAPE)
	await _wait_phase(scene, "menu")
	# Esc quits to the title (MODE_NONE); the scene change is asynchronous.
	_key(KEY_ESCAPE)
	await _wait_title(300)


func _wait_phase(scene: Node, phase: String) -> void:
	for i in 600:
		if scene.phase == phase:
			return
		await get_tree().process_frame
	assert_str(scene.phase).override_failure_message("phase %s never reached" % phase).is_equal(phase)


## Waits until the current game scene is the title or absent (max frames).
func _wait_title(max_frames: int) -> void:
	for i in max_frames:
		var cur := Effects.current_game_scene()
		if cur == null or cur.scene_file_path == "res://scenes/title.tscn":
			return
		await get_tree().process_frame
	var cur := Effects.current_game_scene()
	var path: String = cur.scene_file_path if cur != null else "(null)"
	assert_str(path).override_failure_message("return to title expected").is_equal("res://scenes/title.tscn")
