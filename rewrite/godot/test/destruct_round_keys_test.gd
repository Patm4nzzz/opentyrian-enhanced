# Destruct, round keys. The human / computer toggle of the right-hand player follows the
# reconfigurable Gamma key (destruct.c:1755, input_held(INPUT_GAMMA)), F11 by default,
# and not a hard-coded F11.
extends GdUnitTestSuite

const SCENE := "res://scenes/destruct.tscn"


func before_test() -> void:
	if FileAccess.file_exists("user://destruct.cfg"):
		DirAccess.remove_absolute(ProjectSettings.globalize_path("user://destruct.cfg"))


func after_test() -> void:
	TyrianControls.rebind_global(TyrianControls.GAMMA, KEY_F11)
	for child in Effects.game_viewport.get_children():
		child.queue_free()
	Music.release_streams()
	Sfx.release_streams()


func _tap(key: int) -> void:
	for pressed in [true, false]:
		var ev := InputEventKey.new()
		ev.physical_keycode = key as Key
		ev.keycode = key as Key
		ev.pressed = pressed
		Effects.game_viewport.push_input(ev)


func _wait_phase(scene: Node, phase: String) -> void:
	for i in 600:
		if scene.phase == phase:
			return
		await get_tree().process_frame
	assert_str(scene.phase).is_equal(phase)


## A key typed during the round, then a few ticks.
func _tap_in_round(key: int) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = key as Key
	ev.keycode = key as Key
	ev.pressed = true
	Effects.game_viewport.push_input(ev)
	for i in 6:
		await get_tree().process_frame
	ev = ev.duplicate()
	ev.pressed = false
	Effects.game_viewport.push_input(ev)
	for i in 3:
		await get_tree().process_frame


func test_right_player_toggle_follows_gamma_binding() -> void:
	var scene: Node = auto_free(load(SCENE).instantiate())
	Effects.game_viewport.add_child(scene)
	await get_tree().process_frame
	await _wait_phase(scene, "intro")
	_tap(KEY_ENTER)
	await _wait_phase(scene, "menu")
	_tap(KEY_ENTER)
	await _wait_phase(scene, "play")
	var start: bool = scene.game.is_cpu(1)
	# Default key: F11.
	await _tap_in_round(KEY_F11)
	(
		assert_bool(scene.game.is_cpu(1))
		. override_failure_message("F11 by default must toggle the right-hand player")
		. is_equal(not start)
	)
	# Gamma bound to Origin: Origin toggles, F11 does nothing anymore.
	assert_bool(TyrianControls.rebind_global(TyrianControls.GAMMA, KEY_HOME)).is_true()
	await _tap_in_round(KEY_HOME)
	assert_bool(scene.game.is_cpu(1)).override_failure_message("the rebound Gamma key (Origin) must toggle").is_equal(
		start
	)
	await _tap_in_round(KEY_F11)
	(
		assert_bool(scene.game.is_cpu(1))
		. override_failure_message("F11 must no longer toggle once Gamma is bound to Origin")
		. is_equal(start)
	)


## Gamepad: the exit button (Back) during the round returns to the mode
## choice, like Esc (the C: gamepad button changed to Esc, destruct.c:1772).
func test_pad_exit_button_leaves_round() -> void:
	var scene: Node = auto_free(load(SCENE).instantiate())
	Effects.game_viewport.add_child(scene)
	await get_tree().process_frame
	await _wait_phase(scene, "intro")
	_tap(KEY_ENTER)
	await _wait_phase(scene, "menu")
	_tap(KEY_ENTER)
	await _wait_phase(scene, "play")
	for pressed in [true, false]:
		var ev := InputEventJoypadButton.new()
		ev.device = 0
		ev.button_index = JOY_BUTTON_BACK
		ev.pressed = pressed
		Effects.game_viewport.push_input(ev)
		await get_tree().process_frame
	await _wait_phase(scene, "menu")
