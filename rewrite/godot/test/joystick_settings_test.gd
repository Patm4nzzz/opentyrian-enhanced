# Setup > Joystick (joystick_menu.c): entry put back at the Setup root,
# between Key Bindings and Mods; Gamepad screen with gamepad choice,
# analog, sensitivity, threshold, ten assignments and reset.
# Real events: keyboard in Setup, InputEventJoypadMotion /
# InputEventJoypadButton for the stick and captures; each setting acts
# on read_analog / read_mask and is read back from controls.cfg.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const SetupScreen := preload("res://scripts/setup.gd")

## Rows of joystick_items.
const ROW_ANALOG := 1
const ROW_SENS := 2
const ROW_THRESH := 3
const ROW_UP := 4
const ROW_FIRE := 8
const ROW_RESET := 14


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(C.CONFIG_PATH))
	C.install()
	C.reset_defaults()


func after_test() -> void:
	_axis(0, JOY_AXIS_LEFT_X, 0.0)
	_axis(0, JOY_AXIS_LEFT_Y, 0.0)
	_axis(1, JOY_AXIS_LEFT_X, 0.0)
	_axis(0, JOY_AXIS_RIGHT_X, 0.0)
	C.reset_defaults()
	DirAccess.remove_absolute(ProjectSettings.globalize_path(C.CONFIG_PATH))
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()


func _axis(device: int, axis: int, value: float) -> void:
	var ev := InputEventJoypadMotion.new()
	ev.device = device
	ev.axis = axis
	ev.axis_value = value
	Input.parse_input_event(ev)
	Input.flush_buffered_events()


## Stick pushed while Setup is open: the screen does not receive it
## (the left stick also navigates menus), only Input sees it.
func _stick(setup: Node, device: int, axis: int, value: float) -> void:
	setup.set_process_unhandled_input(false)
	_axis(device, axis, value)
	await get_tree().process_frame
	setup.set_process_unhandled_input(true)


func _press(code: int) -> void:
	for pressed in [true, false]:
		var ev := InputEventKey.new()
		ev.physical_keycode = code as Key
		ev.keycode = code as Key
		ev.pressed = pressed
		Input.parse_input_event(ev)
		Input.flush_buffered_events()
	await get_tree().process_frame


func _pad_button(device: int, b: int, pressed: bool) -> void:
	var ev := InputEventJoypadButton.new()
	ev.device = device
	ev.button_index = b as JoyButton
	ev.pressed = pressed
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	await get_tree().process_frame


## Setup open, Joystick chosen with the keyboard (5th row: Down × 4, Enter).
func _open_joystick() -> Node:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	for i in 120:
		await get_tree().process_frame
		if setup.idle:
			break
	for i in 4:
		await _press(KEY_DOWN)
	assert_str(setup.menus.current().items[setup.menus.current().sel].label).is_equal("setup_joystick")
	await _press(KEY_ENTER)
	assert_str(setup.menus.current().header).is_equal("joym_header")
	return setup


func _go_to(setup: Node, row: int) -> void:
	while setup.menus.current().sel != row:
		await _press(KEY_DOWN)


func _saved(section: String, key: String) -> Variant:
	var cfg := ConfigFile.new()
	assert_int(cfg.load(C.CONFIG_PATH)).is_equal(OK)
	return cfg.get_value(section, key)


func test_setup_root_has_joystick_between_key_bindings_and_mods() -> void:
	var setup: Node = auto_free(load("res://scenes/setup.tscn").instantiate())
	(
		assert_array(setup._setup_items().map(func(it): return it.label))
		. contains_exactly(
			[
				"setup_graphics",
				"setup_sound",
				"setup_gameplay",
				"setup_keybindings",
				"setup_joystick",
				"setup_mods",
				"setup_jukebox",
				"setup_done"
			]
		)
	)
	var labels: Array = setup.joystick.items().map(func(it): return it.label)
	assert_array(labels).contains_exactly(
		[
			"joym_device",
			"joym_analog",
			"joym_sensitivity",
			"joym_threshold",
			"joym_up",
			"joym_right",
			"joym_down",
			"joym_left",
			"joym_fire",
			"joym_change_fire",
			"joym_left_sidekick",
			"joym_right_sidekick",
			"joym_menu",
			"joym_pause",
			"joym_reset",
			"setup_done"
		]
	)


## Threshold and sensitivity set with the keyboard: read_analog follows
## joystick_axis_reduce with the new values; read back from controls.cfg.
func test_threshold_and_sensitivity_apply_to_read_analog() -> void:
	var setup := await _open_joystick()
	C.analog_enabled[1] = true
	C.analog_enabled[2] = true
	# Stick at 30%: 0.3 × 32767 = 9830. Threshold 5, sensitivity 5:
	# (9830 - 5000) / 2000 = 2.
	await _stick(setup, 0, JOY_AXIS_LEFT_X, 0.3)
	assert_object(C.read_analog(1)).is_equal(Vector2i(2, 0))
	# Threshold 10 (Right × 5): 9830 <= 10000, dead zone.
	await _go_to(setup, ROW_THRESH)
	for i in 5:
		await _press(KEY_RIGHT)
	assert_int(C.analog_threshold[1]).is_equal(10)
	assert_object(C.read_analog(1)).override_failure_message("threshold 10: dead zone").is_equal(Vector2i.ZERO)
	# Upper bound: one more Right stays at 10.
	await _press(KEY_RIGHT)
	assert_int(C.analog_threshold[1]).is_equal(10)
	# Threshold 0 (Left × 10): 9830 / 2000 = 4.
	for i in 10:
		await _press(KEY_LEFT)
	assert_int(C.analog_threshold[1]).is_equal(0)
	assert_object(C.read_analog(1)).is_equal(Vector2i(4, 0))
	# Sensitivity 10: divisor 3000 - 2000 = 1000 -> 9.
	await _press(KEY_UP)
	assert_int(setup.menus.current().sel).is_equal(ROW_SENS)
	for i in 5:
		await _press(KEY_RIGHT)
	assert_int(C.analog_sensitivity[1]).is_equal(10)
	assert_object(C.read_analog(1)).is_equal(Vector2i(9, 0))
	# Player 2's gamepad keeps its settings.
	await _stick(setup, 1, JOY_AXIS_LEFT_X, 0.3)
	assert_object(C.read_analog(2)).is_equal(Vector2i(2, 0))
	assert_int(_saved("joystick", "threshold_1")).is_equal(0)
	assert_int(_saved("joystick", "sensitivity_1")).is_equal(10)
	# Rebuilt actions forget the tilt: the stick is sent again.
	C.reset_defaults()
	C.analog_enabled[1] = true
	await _stick(setup, 0, JOY_AXIS_LEFT_X, 0.3)
	assert_object(C.read_analog(1)).is_equal(Vector2i(2, 0))
	assert_bool(C.load_config()).is_true()
	await _stick(setup, 0, JOY_AXIS_LEFT_X, 0.3)
	assert_object(C.read_analog(1)).is_equal(Vector2i(9, 0))


## Analog off: no more proportional movement, the stick sets the
## direction bits beyond mid-travel (joystick.c:poll_joystick).
func test_analog_toggle_switches_to_digital_directions() -> void:
	var setup := await _open_joystick()
	C.analog_enabled[1] = true
	await _go_to(setup, ROW_ANALOG)
	await _stick(setup, 0, JOY_AXIS_LEFT_X, 0.8)
	assert_int(C.read_analog(1).x).is_greater(0)
	assert_int(C.read_mask(1) & C.MASK_RIGHT).is_equal(0)
	await _press(KEY_ENTER)
	assert_bool(C.analog_enabled[1]).is_false()
	assert_object(C.read_analog(1)).is_equal(Vector2i.ZERO)
	assert_int(C.read_mask(1) & C.MASK_RIGHT).is_equal(C.MASK_RIGHT)
	await _stick(setup, 0, JOY_AXIS_LEFT_X, 0.4)
	assert_int(C.read_mask(1) & C.MASK_RIGHT).override_failure_message("below mid-travel").is_equal(0)
	assert_bool(_saved("joystick", "analog_1")).is_false()
	C.reset_defaults()
	C.load_config()
	assert_bool(C.analog_enabled[1]).is_false()


## Existing profile: a controls.cfg that enabled analog keeps it;
## a profile without the setting stays off like the C.
func test_saved_analog_choice_is_kept() -> void:
	assert_bool(C.analog_enabled[1]).is_false()
	C.analog_enabled[1] = true
	C.save_config()
	C.reset_defaults()
	assert_bool(C.analog_enabled[1]).is_false()
	assert_bool(C.load_config()).is_true()
	assert_bool(C.analog_enabled[1]).is_true()
	assert_bool(C.analog_enabled[2]).is_false()


## Gamepad choice: Right switches to player 2, whose settings
## change on their own.
func test_device_picker_selects_player_two_pad() -> void:
	var setup := await _open_joystick()
	await _press(KEY_RIGHT)
	assert_int(setup.joystick.player).is_equal(2)
	await _go_to(setup, ROW_THRESH)
	await _press(KEY_RIGHT)
	assert_int(C.analog_threshold[2]).is_equal(6)
	assert_int(C.analog_threshold[1]).is_equal(5)


## Assignment: Enter on "Fire", then a real gamepad button; the
## button fires, the code is shown ("BTN 4"), a second capture of the same
## button removes it (joystick_assign_toggle). An axis can be captured too.
func test_assignment_capture_toggle_and_reset() -> void:
	var setup := await _open_joystick()
	await _go_to(setup, ROW_FIRE)
	await _press(KEY_ENTER)
	assert_bool(setup.joystick.capturing()).is_true()
	await _pad_button(0, JOY_BUTTON_Y, true)
	await _pad_button(0, JOY_BUTTON_Y, false)
	assert_bool(setup.joystick.capturing()).is_false()
	assert_str(C.pad_row_text(1, ROW_FIRE - ROW_UP)).contains("BTN 4")
	await _pad_button(0, JOY_BUTTON_Y, true)
	assert_int(C.read_mask(1) & C.MASK_FIRE).is_equal(C.MASK_FIRE)
	await _pad_button(0, JOY_BUTTON_Y, false)
	assert_array(_saved("player1", "fire_buttons")).contains([JOY_BUTTON_Y])
	# Second capture: removed.
	await _press(KEY_ENTER)
	await _pad_button(0, JOY_BUTTON_Y, true)
	await _pad_button(0, JOY_BUTTON_Y, false)
	assert_str(C.pad_row_text(1, ROW_FIRE - ROW_UP)).not_contains("BTN 4")
	# "Up": right vertical axis pushed up (-1) -> "AX 4-".
	await _go_to(setup, ROW_UP)
	await _press(KEY_ENTER)
	_axis(0, JOY_AXIS_RIGHT_Y, -0.9)
	await get_tree().process_frame
	_axis(0, JOY_AXIS_RIGHT_Y, 0.0)
	assert_str(C.pad_row_text(1, 0)).contains("AX 4-")
	# Analog on: the assigned axis moves the ship up.
	C.analog_enabled[1] = true
	_axis(0, JOY_AXIS_RIGHT_Y, -1.0)
	assert_int(C.read_analog(1).y).is_equal(-13)
	_axis(0, JOY_AXIS_RIGHT_Y, 0.0)
	# A key cancels a capture without changing anything.
	var before := C.pad_row_text(1, 0)
	await _press(KEY_ENTER)
	await _press(KEY_SPACE)
	assert_bool(setup.joystick.capturing()).is_false()
	assert_str(C.pad_row_text(1, 0)).is_equal(before)
	C.reset_defaults()
	C.load_config()
	assert_str(C.pad_row_text(1, 0)).contains("AX 4-")
	# Reset: assignments, analog and settings back to defaults.
	C.analog_threshold[1] = 9
	await _go_to(setup, ROW_RESET)
	await _press(KEY_ENTER)
	assert_int(C.analog_threshold[1]).is_equal(C.ANALOG_THRESHOLD)
	assert_str(C.pad_row_text(1, 0)).not_contains("AX 4-")
	assert_str(C.pad_row_text(1, 0)).contains("AX 2-")
	C.reset_defaults()
	C.load_config()
	assert_str(C.pad_row_text(1, 0)).not_contains("AX 4-")


## Capture prompt and help line: in every language, the prompt fits in two
## lines in the box (260 px) and the help fits in the 296 px of the help strip.
func test_capture_prompt_and_help_fit_screen() -> void:
	for lang in ["en", "fr", "de", "es", "it", "pt"]:
		var lines := JoystickMenu.wrap_lines(TyrianText.L("joym_capture", lang), JoystickMenu.CAPTURE_TEXT_WIDTH, 1)
		assert_int(lines.size()).is_between(1, 2)
		for line in lines:
			assert_int(TyrianText.width(line, 1)).is_less_equal(JoystickMenu.CAPTURE_TEXT_WIDTH)
		assert_int(TyrianText.width(TyrianText.L("joym_assign_desc", lang), 2)).is_less_equal(296)
