# Keyboard and gamepad controls (blocks 14/15, ADR 0120). Taken from
# tools/check_controls.gd: same 22 checks, grouped by topic,
# plus the reconfigurable help and capture cancellation.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const SAVE_PATH := "user://check_controls.cfg"


func before_test() -> void:
	C.install()
	C.reset_defaults()


func after_test() -> void:
	C.reset_defaults()


func _key(code: int, pressed: bool) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = code
	ev.pressed = pressed
	Input.parse_input_event(ev)
	Input.flush_buffered_events()


func _pad_button(device: int, b: int, pressed: bool) -> void:
	var ev := InputEventJoypadButton.new()
	ev.device = device
	ev.button_index = b
	ev.pressed = pressed
	Input.parse_input_event(ev)
	Input.flush_buffered_events()


func _pad_axis(device: int, axis: int, value: float) -> void:
	var ev := InputEventJoypadMotion.new()
	ev.device = device
	ev.axis = axis
	ev.axis_value = value
	Input.parse_input_event(ev)
	Input.flush_buffered_events()


func _pad_event(button: int) -> InputEventJoypadButton:
	var ev := InputEventJoypadButton.new()
	ev.button_index = button
	ev.pressed = true
	return ev


func test_actions_installed() -> void:
	(
		assert_bool(
			(
				InputMap.has_action("tyrian_fire")
				and InputMap.has_action("tyrian_p2_fire")
				and InputMap.has_action("tyrian_menu_accept")
			)
		)
		. is_true()
	)


func test_keyboard_players() -> void:
	_key(KEY_SPACE, true)
	_key(KEY_UP, true)
	assert_int(C.read_mask(1)).override_failure_message("p1 Space+Up").is_equal(C.MASK_FIRE | C.MASK_UP)
	assert_int(C.read_mask(2)).is_equal(0)
	_key(KEY_SPACE, false)
	_key(KEY_UP, false)
	_key(KEY_A, true)
	_key(KEY_F, true)
	assert_int(C.read_mask(2)).override_failure_message("j2 A+F").is_equal(C.MASK_LEFT | C.MASK_FIRE)
	assert_int(C.read_mask(1)).is_equal(0)
	_key(KEY_A, false)
	_key(KEY_F, false)


func test_gamepads() -> void:
	# Analog off (Setup > Joystick > Analog): the D-pad gives all-or-nothing
	# directions, at the keyboard's step (player_movement.c, non-analog branch).
	C.analog_enabled[1] = false
	C.analog_enabled[2] = false
	_pad_button(0, JOY_BUTTON_A, true)
	_pad_button(0, JOY_BUTTON_DPAD_RIGHT, true)
	assert_int(C.read_mask(1)).override_failure_message("gamepad 0 -> p1 (A + d-pad right)").is_equal(
		C.MASK_FIRE | C.MASK_RIGHT
	)
	assert_int(C.read_mask(2)).is_equal(0)
	_pad_button(0, JOY_BUTTON_A, false)
	_pad_button(0, JOY_BUTTON_DPAD_RIGHT, false)
	_pad_axis(1, JOY_AXIS_TRIGGER_RIGHT, 0.2)
	assert_int(C.read_mask(2)).override_failure_message("gamepad 1 trigger dead zone").is_equal(0)
	_pad_button(1, JOY_BUTTON_DPAD_UP, true)
	_pad_axis(1, JOY_AXIS_TRIGGER_RIGHT, 1.0)
	_pad_button(1, JOY_BUTTON_X, true)
	assert_int(C.read_mask(2)).override_failure_message("gamepad 1 -> p2 (d-pad up, trigger, X)").is_equal(
		C.MASK_UP | C.MASK_FIRE | C.MASK_LEFT_SIDEKICK
	)
	assert_int(C.read_mask(1)).is_equal(0)
	_pad_button(1, JOY_BUTTON_DPAD_UP, false)
	_pad_axis(1, JOY_AXIS_TRIGGER_RIGHT, 0.0)
	_pad_button(1, JOY_BUTTON_X, false)


## Analog stick (joystick.c:joystick_axis_reduce, threshold 5, sensitivity 5):
## no direction bit, a movement proportional to the tilt, zero
## in the dead zone, and specific to each player's gamepad.
func test_analog_stick() -> void:
	C.analog_enabled[1] = true
	C.analog_enabled[2] = true
	assert_int(C.analog_axis_reduce(5000)).is_equal(0)
	assert_int(C.analog_axis_reduce(5001)).is_equal(0)
	assert_int(C.analog_axis_reduce(7000)).is_equal(1)
	assert_int(C.analog_axis_reduce(-7000)).is_equal(-1)
	assert_int(C.analog_axis_reduce(32767)).is_equal(13)
	assert_int(C.analog_axis_reduce(-32767)).is_equal(-13)
	_pad_axis(0, JOY_AXIS_LEFT_X, 0.1)
	assert_object(C.read_analog(1)).override_failure_message("dead zone").is_equal(Vector2i.ZERO)
	_pad_axis(0, JOY_AXIS_LEFT_X, 0.5)
	var half := C.read_analog(1)
	_pad_axis(0, JOY_AXIS_LEFT_X, 1.0)
	_pad_axis(0, JOY_AXIS_LEFT_Y, -1.0)
	var full := C.read_analog(1)
	assert_int(half.x).override_failure_message("mid-travel: %s" % half).is_between(4, 6)
	assert_object(full).is_equal(Vector2i(13, -13))
	assert_int(C.read_mask(1)).override_failure_message("the stick no longer sets a direction bit").is_equal(0)
	assert_object(C.read_analog(2)).override_failure_message("gamepad 0 -> player 1 only").is_equal(Vector2i.ZERO)
	_pad_axis(0, JOY_AXIS_LEFT_X, 0.0)
	_pad_axis(0, JOY_AXIS_LEFT_Y, 0.0)


## Fresh profile: analog off for both gamepads, like the C
## (joystick.c:507, `analog = false` for lack of a key in the configuration);
## the D-pad then gives the keyboard step, not the stick at full tilt.
func test_analog_off_by_default_like_c() -> void:
	assert_bool(C.analog_enabled[1]).is_false()
	assert_bool(C.analog_enabled[2]).is_false()
	_pad_button(0, JOY_BUTTON_DPAD_RIGHT, true)
	assert_object(C.read_analog(1)).is_equal(Vector2i.ZERO)
	assert_int(C.read_mask(1)).is_equal(C.MASK_RIGHT)
	_pad_button(0, JOY_BUTTON_DPAD_RIGHT, false)


## Analog on: the D-pad counts as the stick pushed fully
## (joystick.c:check_assigned returns joystick_analog_max for a hat),
## i.e. 13 units per tick at the default setting, and not the one-pixel step
## of the keyboard. The keyboard key, for its part, keeps its step.
func test_dpad_is_full_stick_when_analog() -> void:
	C.analog_enabled[1] = true
	_pad_button(0, JOY_BUTTON_DPAD_RIGHT, true)
	assert_object(C.read_analog(1)).is_equal(Vector2i(13, 0))
	assert_int(C.read_mask(1)).override_failure_message("the d-pad sets no bit in analog mode").is_equal(0)
	_key(KEY_RIGHT, true)
	assert_int(C.read_mask(1)).override_failure_message("the keyboard keeps its bit").is_equal(C.MASK_RIGHT)
	_key(KEY_RIGHT, false)
	_pad_button(0, JOY_BUTTON_DPAD_RIGHT, false)
	assert_object(C.read_analog(1)).is_equal(Vector2i.ZERO)


func test_menu_actions() -> void:
	var down := _pad_event(JOY_BUTTON_DPAD_DOWN)
	assert_int(C.menu_key_for(down)).override_failure_message("gamepad -> menu key").is_equal(KEY_DOWN)
	assert_str(C.menu_action(down)).override_failure_message("gamepad -> menu action").is_equal(C.DOWN)
	assert_str(C.menu_action(C.key_event(KEY_ENTER))).is_equal(C.ACCEPT)
	assert_str(C.menu_action(C.key_event(KEY_KP_ENTER))).is_equal(C.ACCEPT)
	assert_str(C.menu_action(C.key_event(KEY_SPACE))).is_equal(C.SELECT)
	assert_str(C.menu_action(C.key_event(KEY_ESCAPE))).is_equal(C.BACK)
	assert_str(C.menu_action(C.key_event(KEY_TAB))).is_equal(C.TOGGLE)
	assert_str(C.menu_action(C.key_event(KEY_Q))).override_failure_message("key without action -> empty").is_equal(&"")
	assert_str(C.menu_action(_pad_event(JOY_BUTTON_B))).override_failure_message("gamepad B -> back").is_equal(C.BACK)


## joystick.c:push_joysticks_as_keyboard (l. 264-267): buttons 0 and 4 (A, LB)
## = Enter, buttons 1 and 5 (B, RB) = Esc; Start (button 7) is nothing
## in a menu of the C.
func test_menu_pad_buttons_like_c() -> void:
	(
		assert_str(C.menu_action(_pad_event(JOY_BUTTON_LEFT_SHOULDER)))
		. override_failure_message("LB -> confirm (C button 4)")
		. is_equal(C.ACCEPT)
	)
	(
		assert_str(C.menu_action(_pad_event(JOY_BUTTON_RIGHT_SHOULDER)))
		. override_failure_message("RB -> back (C button 5)")
		. is_equal(C.BACK)
	)
	(
		assert_str(C.menu_action(_pad_event(JOY_BUTTON_START)))
		. override_failure_message("Start does nothing in a menu")
		. is_equal(&"")
	)
	assert_int(C.menu_key_for(_pad_event(JOY_BUTTON_LEFT_SHOULDER))).is_equal(KEY_ENTER)
	assert_int(C.menu_key_for(_pad_event(JOY_BUTTON_RIGHT_SHOULDER))).is_equal(KEY_ESCAPE)
	assert_int(C.menu_key_for(_pad_event(JOY_BUTTON_START))).is_equal(0)


func test_rebind_saved_and_reloaded() -> void:
	C.rebind_key(1, "fire", KEY_X)
	C.rebind_button(1, "fire", JOY_BUTTON_Y)
	assert_int(C.save_config(SAVE_PATH)).is_equal(OK)
	C.reset_defaults()
	assert_bool(C.load_config(SAVE_PATH)).is_true()
	_key(KEY_X, true)
	assert_int(C.read_mask(1)).override_failure_message("rebound key read back (X = fire)").is_equal(C.MASK_FIRE)
	_key(KEY_X, false)
	_key(KEY_SPACE, true)
	assert_int(C.read_mask(1)).override_failure_message("old key removed (Space)").is_equal(0)
	_key(KEY_SPACE, false)
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SAVE_PATH))


## M-07: a controls.cfg from before format 2 keeps LB / RB as sidekicks;
## loading removes them (joystick.c:499-503) and RB pauses.
func test_old_controls_file_loses_shoulder_sidekicks() -> void:
	assert_int(C.save_config(SAVE_PATH)).is_equal(OK)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(SAVE_PATH)).is_equal(OK)
	cfg.set_value("player1", "left_sidekick_buttons", [JOY_BUTTON_X, JOY_BUTTON_LEFT_SHOULDER])
	cfg.set_value("player1", "right_sidekick_buttons", [JOY_BUTTON_Y, JOY_BUTTON_RIGHT_SHOULDER])
	cfg.set_value("global", "pause_buttons", [JOY_BUTTON_START])
	cfg.erase_section("format")
	assert_int(cfg.save(SAVE_PATH)).is_equal(OK)
	C.reset_defaults()
	assert_bool(C.load_config(SAVE_PATH)).is_true()
	_pad_button(0, JOY_BUTTON_LEFT_SHOULDER, true)
	assert_int(C.read_mask(1)).override_failure_message("LB is no longer a sidekick").is_equal(0)
	_pad_button(0, JOY_BUTTON_LEFT_SHOULDER, false)
	_pad_button(0, JOY_BUTTON_X, true)
	assert_int(C.read_mask(1)).override_failure_message("X remains the left sidekick").is_equal(C.MASK_LEFT_SIDEKICK)
	_pad_button(0, JOY_BUTTON_X, false)
	assert_bool(C._buttons_of(C.PAUSE).has(JOY_BUTTON_RIGHT_SHOULDER)).override_failure_message("RB pauses").is_true()
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SAVE_PATH))


func test_steam_deck_profile() -> void:
	assert_bool(C.load_config(C.STEAM_DECK_PROFILE)).is_true()
	_pad_button(0, JOY_BUTTON_Y, true)
	assert_int(C.read_mask(1)).override_failure_message("Steam Deck: Y = right sidekick").is_equal(
		C.MASK_RIGHT_SIDEKICK
	)
	_pad_button(0, JOY_BUTTON_Y, false)
	# The shoulders are the C's menu (L1) and pause (R1), not sidekicks.
	_pad_button(0, JOY_BUTTON_RIGHT_SHOULDER, true)
	assert_int(C.read_mask(1)).is_equal(0)
	_pad_button(0, JOY_BUTTON_RIGHT_SHOULDER, false)
	_pad_axis(0, JOY_AXIS_TRIGGER_RIGHT, 1.0)
	assert_int(C.read_mask(1)).override_failure_message("Steam Deck: R2 = fire").is_equal(C.MASK_FIRE)
	_pad_axis(0, JOY_AXIS_TRIGGER_RIGHT, 0.0)


## Review m3: help (F1) and capture cancellation (Esc) are reconfigurable
## InputMap actions, saved with the other bindings.
func test_help_and_capture_cancel_are_rebindable_and_saved() -> void:
	(
		assert_bool(C.global_pressed(C.key_event(KEY_F1), C.HELP))
		. override_failure_message("F1 = help by default")
		. is_true()
	)
	assert_bool(C.global_pressed(C.key_event(KEY_ESCAPE), C.CAPTURE_CANCEL)).is_true()
	assert_bool(C.rebind_global(C.HELP, KEY_H)).is_true()
	assert_bool(C.rebind_global(C.CAPTURE_CANCEL, KEY_BACKSPACE)).is_true()
	(
		assert_bool(C.global_pressed(C.key_event(KEY_F1), C.HELP))
		. override_failure_message("F1 is no longer help")
		. is_false()
	)
	assert_int(C.save_config(SAVE_PATH)).is_equal(OK)
	C.reset_defaults()
	assert_int(C.global_key(C.HELP)).override_failure_message("default restored").is_equal(KEY_F1)
	assert_bool(C.load_config(SAVE_PATH)).is_true()
	assert_bool(C.global_pressed(C.key_event(KEY_H), C.HELP)).override_failure_message("help read back (H)").is_true()
	assert_int(C.global_key(C.CAPTURE_CANCEL)).override_failure_message("cancellation read back").is_equal(
		KEY_BACKSPACE
	)
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SAVE_PATH))


## The cancel key cannot become a key that would prevent starting
## or finishing a capture, nor a game key, nor the help key.
func test_capture_cancel_key_stays_usable() -> void:
	for k in [KEY_ENTER, KEY_KP_ENTER, KEY_UP, KEY_DOWN, KEY_TAB, KEY_DELETE, KEY_SPACE, KEY_W, KEY_F1]:
		(
			assert_bool(C.rebind_global(C.CAPTURE_CANCEL, k))
			. override_failure_message("key %s refused" % OS.get_keycode_string(k))
			. is_false()
		)
	assert_int(C.global_key(C.CAPTURE_CANCEL)).is_equal(KEY_ESCAPE)
	assert_bool(C.rebind_global(C.HELP, KEY_SPACE)).override_failure_message("help on a game key refused").is_false()


## Gamepad buttons carry the same number (starting at 1) on the controls
## screen and on the Gamepad screen.
func test_controls_describe_numbers_pad_buttons_like_joystick_screen() -> void:
	C.rebind_button(1, "fire", JOY_BUTTON_Y)
	var text := C.describe(1, "fire")
	assert_str(text).override_failure_message("button number:" + text).ends_with("BTN %d" % (JOY_BUTTON_Y + 1))


## Controls screen: right click cancels the capture like Esc.
func test_controls_screen_right_click_cancels_capture() -> void:
	var screen: ControlsOptions = auto_free(ControlsOptions.new())
	screen.selected = 4  # fire
	screen.waiting = true
	var click := InputEventMouseButton.new()
	click.button_index = MOUSE_BUTTON_RIGHT
	click.pressed = true
	screen._unhandled_input(click)
	assert_bool(screen.waiting).is_false()
	assert_str(C.describe(1, "fire")).override_failure_message("right click: fire unchanged").starts_with("Space")


## Controls screen: the configured cancel key cancels the capture
## (the old Esc becomes an ordinary key) and any key ends it.
func test_controls_screen_capture_uses_cancel_action() -> void:
	var screen: ControlsOptions = auto_free(load("res://scripts/controls_options.gd").new())
	C.rebind_global(C.CAPTURE_CANCEL, KEY_BACKSPACE)
	screen.selected = 4  # fire
	screen.waiting = true
	screen.capture(C.key_event(KEY_BACKSPACE))
	assert_bool(screen.waiting).is_false()
	assert_str(C.describe(1, "fire")).override_failure_message("cancelled: fire unchanged").starts_with("Space")
	screen.waiting = true
	screen.capture(C.key_event(KEY_ESCAPE))
	assert_str(C.describe(1, "fire")).override_failure_message("Escape no longer cancels: new key").starts_with(
		"Escape"
	)
	screen.selected = C.COMMANDS.size()  # Help row
	screen.waiting = true
	screen.capture(C.key_event(KEY_F14))
	assert_int(C.global_key(C.HELP)).is_equal(KEY_F14)
	screen.waiting = true
	screen.capture(C.key_event(KEY_ESCAPE))
	assert_bool(screen.refused).override_failure_message("help on the fire key refused").is_true()
	assert_int(C.global_key(C.HELP)).is_equal(KEY_F14)
