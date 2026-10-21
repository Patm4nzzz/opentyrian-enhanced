# M-03: the wheel is not a button (keyboard.c:service_SDL_events_ex only handles
# SDL_MOUSEBUTTONDOWN, no SDL_MOUSEWHEEL). Godot emits a "press" for buttons
# 4 to 7: neither the pause resume, nor the demo exit, nor the title's inactivity
# counter must take it for a click.
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")
const TITLE := preload("res://scripts/title.gd")


func _button(index: int, pressed := true) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(100, 100)
	ev.button_index = index
	ev.pressed = pressed
	return ev


func test_wheel_does_not_resume_pause() -> void:
	for wheel in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN, MOUSE_BUTTON_WHEEL_LEFT, MOUSE_BUTTON_WHEEL_RIGHT]:
		(
			assert_bool(LEVEL._resumes_pause(_button(wheel)))
			. override_failure_message("wheel %d resumes the pause" % wheel)
			. is_false()
		)


func test_real_click_resumes_pause() -> void:
	assert_bool(LEVEL._resumes_pause(_button(MOUSE_BUTTON_LEFT))).is_true()
	assert_bool(LEVEL._resumes_pause(_button(MOUSE_BUTTON_RIGHT))).is_true()
	assert_bool(LEVEL._resumes_pause(_button(MOUSE_BUTTON_LEFT, false))).is_false()


func test_wheel_does_not_reset_title_idle_counter() -> void:
	var title := TITLE.new()
	title._idle_ms = 12000.0
	title.handle_event(_button(MOUSE_BUTTON_WHEEL_DOWN))
	assert_float(title._idle_ms).is_equal(12000.0)
	title.handle_event(_button(MOUSE_BUTTON_LEFT))
	assert_float(title._idle_ms).is_equal(0.0)
	title.free()


## mainint.c:3284: the pause resume ignores Ctrl and Alt only; Shift alone resumes.
func test_shift_alone_resumes_pause_but_not_ctrl_or_alt() -> void:
	for code in [KEY_CTRL, KEY_ALT]:
		var ev := InputEventKey.new()
		ev.physical_keycode = code
		ev.pressed = true
		assert_bool(LEVEL._resumes_pause(ev)).override_failure_message("key %d resumes the pause" % code).is_false()
	var shift := InputEventKey.new()
	shift.physical_keycode = KEY_SHIFT
	shift.pressed = true
	assert_bool(LEVEL._resumes_pause(shift)).override_failure_message("Shift alone does not resume the pause").is_true()


## title_screen.c: only a change of mouse position (mouseMoved) restarts the demo delay.
func test_title_idle_reset_only_on_real_mouse_motion() -> void:
	var title: Object = TITLE.new()
	title._idle_ms = 500.0
	var still := InputEventMouseMotion.new()
	still.relative = Vector2.ZERO
	title.handle_event(still)
	assert_float(title._idle_ms).override_failure_message("zero movement: counter reset to zero").is_equal(500.0)
	var moved := InputEventMouseMotion.new()
	moved.relative = Vector2(1, 0)
	title.handle_event(moved)
	assert_float(title._idle_ms).override_failure_message("real movement: counter not reset to zero").is_equal(0.0)
	title.free()


## Divergence 54: only the vertical wheel gives a scroll step, on
## press; neither the horizontal wheel nor a real button.
func test_wheel_step_only_vertical_wheel() -> void:
	assert_int(TyrianMouse.wheel_step(_button(MOUSE_BUTTON_WHEEL_DOWN))).is_equal(1)
	assert_int(TyrianMouse.wheel_step(_button(MOUSE_BUTTON_WHEEL_UP))).is_equal(-1)
	assert_int(TyrianMouse.wheel_step(_button(MOUSE_BUTTON_WHEEL_DOWN, false))).is_equal(0)
	assert_int(TyrianMouse.wheel_step(_button(MOUSE_BUTTON_WHEEL_LEFT))).is_equal(0)
	assert_int(TyrianMouse.wheel_step(_button(MOUSE_BUTTON_LEFT))).is_equal(0)
