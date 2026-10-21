## Mouse speed in a level (keyboard.c, video.c, player_movement.c):
## - the RAW window pixels (screen_relative) add up over the tick;
## - a single conversion per tick: video.c:scaleWindowDistanceToScreen,
##   x = (2x + 1) × 320 / (2 × displayed width), C integer division;
## - 1 px steps are not lost (a 1000 Hz mouse sends them).
## Expected values = the C formula applied by hand (×3 window: 960×600).
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")
const OUT_X3 := Vector2i(960, 600)


func test_formula_matches_c_integer_division() -> void:
	# (2×0+1)×320/1920 = 0 ; (2×1+1)×320/1920 = 0 ; (2×3+1)×320/1920 = 1 ;
	# 6 -> 13×320/1920 = 2 ; 12 -> 25×320/1920 = 4 ; 360 -> 721×320/1920 = 120.
	var cases := {0: 0, 1: 0, 2: 0, 3: 1, 6: 2, 12: 4, 30: 10, 360: 120}
	for raw in cases:
		(
			assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(raw, 0), OUT_X3).x)
			. override_failure_message("raw %d" % raw)
			. is_equal(cases[raw])
		)
	# Negatives: C truncation toward zero, (2×-3+1)×320/1920 = -1600/1920 = 0.
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(-3, 0), OUT_X3).x).is_equal(0)
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(-6, 0), OUT_X3).x).is_equal(-1)
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(-360, 0), OUT_X3).x).is_equal(-119)
	# Height: (2×9+1)×200/1200 = 3.
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(0, 9), OUT_X3).y).is_equal(3)
	# ×1 window (320×200): the raw value passes through as is.
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(7, 0), Vector2i(320, 200)).x).is_equal(7)


## Twelve 1 px events in the same tick: 12 raw px, converted once
## (= 4 game pixels), and not twelve times 0.
func test_small_events_add_up_within_a_tick() -> void:
	var level: Node = auto_free(LEVEL.new())
	for i in 12:
		var mm := InputEventMouseMotion.new()
		mm.relative = Vector2(1.0 / 3.0, 0)
		mm.screen_relative = Vector2(1, 0)
		level._input(mm)
	assert_that(level._mouse_raw).is_equal(Vector2i(12, 0))
	var d := LEVEL.scale_window_distance_to_screen(level._mouse_raw, OUT_X3)
	assert_int(d.x).is_equal(4)


## One tick's movement does not spill over onto the next (the C accumulator
## restarts from zero after mouseGetRelativePosition).
func test_raw_accumulator_resets_each_tick() -> void:
	var level: Node = auto_free(LEVEL.new())
	var mm := InputEventMouseMotion.new()
	mm.screen_relative = Vector2(2, 0)
	level._input(mm)
	level._take_mouse_delta()
	assert_that(level._mouse_raw).is_equal(Vector2i.ZERO)


## D1 (Windows): in a level, movements arrive one by one (not grouped per
## frame), and the Windows gain applies to each like SDL. The raw sum
## of a gesture therefore does not depend on the frames per second.
func test_level_receives_raw_events_one_by_one() -> void:
	var level: Node = auto_free(LEVEL.new())
	Input.use_accumulated_input = true
	level._set_raw_mouse_events(true)
	(
		assert_bool(Input.use_accumulated_input)
		. override_failure_message("accumulated input still active in level")
		. is_false()
	)
	level._notification(NOTIFICATION_EXIT_TREE)
	assert_bool(Input.use_accumulated_input).override_failure_message("accumulated input not restored").is_true()


func test_windows_gain_applied_per_raw_event() -> void:
	var reg := {
		"MouseSensitivity": "10",
		"MouseSpeed": "1",
		"SmoothMouseXCurve": "0000000000000000156E000000000000004001000000000029DC0300000000000000280000000000",
		"SmoothMouseYCurve": "0000000000000000FD11010000000000002404000000000000FC12000000000000C0BB0100000000"
	}
	MouseSystemScale.set_from_registry(reg)
	var level: Node = auto_free(LEVEL.new())
	# 120 steps of 1 px (1000 Hz mouse): low gain of the curve, like SDL, whatever
	# the number of frames; 120 raw px give 66 px, not ~2 times more.
	for i in 120:
		var mm := InputEventMouseMotion.new()
		mm.screen_relative = Vector2(1, 0)
		level._input(mm)
	MouseSystemScale.values = PackedFloat32Array()
	assert_int(level._mouse_raw.x).is_between(60, 75)
