## Windows pointer gain applied to relative mode (SDL_windowsmouse.c,
## SDL_mouse.c): expected values computed with the SDL 2.30 formulas on
## The maintainer's real setting (speed 10, "Enhance pointer precision" ticked,
## Windows default curves).
extends GdUnitTestSuite

const MAINTAINER_SETTINGS := {
	"MouseSensitivity": "10",
	"MouseSpeed": "1",
	"SmoothMouseXCurve": "0000000000000000156E000000000000004001000000000029DC0300000000000000280000000000",
	"SmoothMouseYCurve": "0000000000000000FD11010000000000002404000000000000FC12000000000000C0BB0100000000",
}


func after_test() -> void:
	MouseSystemScale.values = PackedFloat32Array()


func _sum(step: int, count: int) -> int:
	var total := 0
	for i in count:
		total += MouseSystemScale.apply(Vector2i(step, 0)).x
	return total


## Enhanced curve: 180 raw px give 100 / 124 / 144 / 173 / 207 px depending on
## the step size (1, 2, 3, 6, 12 px), like SDL.
func test_enhanced_curve_matches_sdl() -> void:
	var reg := MouseSystemScale.parse_registry(
		(
			"    MouseSensitivity    REG_SZ    10\n    MouseSpeed    REG_SZ    1\n"
			+ (
				"    SmoothMouseXCurve    REG_BINARY    %s\n    SmoothMouseYCurve    REG_BINARY    %s\n"
				% [MAINTAINER_SETTINGS["SmoothMouseXCurve"], MAINTAINER_SETTINGS["SmoothMouseYCurve"]]
			)
		)
	)
	assert_str(String(reg.get("MouseSpeed", ""))).is_equal("1")
	MouseSystemScale.set_from_registry(reg)
	assert_int(MouseSystemScale.values.size()).is_equal(10)
	assert_int(_sum(1, 180)).is_equal(100)
	MouseSystemScale.set_from_registry(reg)
	assert_int(_sum(2, 90)).is_equal(124)
	MouseSystemScale.set_from_registry(reg)
	assert_int(_sum(3, 60)).is_equal(144)
	MouseSystemScale.set_from_registry(reg)
	assert_int(_sum(6, 30)).is_equal(173)
	MouseSystemScale.set_from_registry(reg)
	assert_int(_sum(12, 15)).is_equal(207)


## Without "Enhance pointer precision": fixed speed gain (10 -> 1, 15 -> 2.25).
func test_linear_speed() -> void:
	MouseSystemScale.set_from_registry({"MouseSensitivity": "10", "MouseSpeed": "0"})
	assert_int(_sum(1, 180)).is_equal(180)
	MouseSystemScale.set_from_registry({"MouseSensitivity": "15", "MouseSpeed": "0"})
	assert_int(_sum(2, 90)).is_equal(405)


## Outside Windows (no value): the raw movement passes through as is.
func test_no_scale_passes_through() -> void:
	MouseSystemScale.values = PackedFloat32Array()
	assert_vector(Vector2(MouseSystemScale.apply(Vector2i(3, -2)))).is_equal(Vector2(3, -2))
