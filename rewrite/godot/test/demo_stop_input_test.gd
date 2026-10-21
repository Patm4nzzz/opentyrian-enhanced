# Title demo: a gamepad direction stops it too (tyrian2.c:1524-1540,
# joystick[j].input_pressed covers buttons AND directions, stick beyond
# half of its travel, joystick.c:179).
extends GdUnitTestSuite

const Level := preload("res://scripts/level.gd")


func _motion(v: float) -> InputEventJoypadMotion:
	var ev := InputEventJoypadMotion.new()
	ev.axis = JOY_AXIS_LEFT_X
	ev.axis_value = v
	return ev


func test_stick_past_half_stops_demo() -> void:
	assert_bool(Level.demo_stop_event(_motion(0.8))).is_true()
	assert_bool(Level.demo_stop_event(_motion(-0.9))).is_true()


func test_stick_resting_does_not_stop_demo() -> void:
	assert_bool(Level.demo_stop_event(_motion(0.2))).is_false()


func test_key_and_button_still_stop_demo() -> void:
	var k := InputEventKey.new()
	k.pressed = true
	k.keycode = KEY_A
	assert_bool(Level.demo_stop_event(k)).is_true()
	var b := InputEventJoypadButton.new()
	b.pressed = true
	b.button_index = JOY_BUTTON_A
	assert_bool(Level.demo_stop_event(b)).is_true()
