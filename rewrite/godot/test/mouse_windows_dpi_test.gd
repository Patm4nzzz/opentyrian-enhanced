## Mouse in a level on scaled Windows (125%, 150%). The C is not
## "DPI aware" (src/opentyrian2000.manifest): Windows reports its window
## in logical pixels, it places its image there (video.c:calc_dst_render_rect) and
## divides the raw mouse counts by that image
## (video.c:scaleWindowDistanceToScreen). Figures measured on the maintainer's
## Windows at 125% with the instrumented c-final C (1 px bursts at 1000 Hz).
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")


func test_logical_window_size_at_125_and_150_percent() -> void:
	assert_that(LEVEL.logical_output_size(Vector2i(1920, 1080), 120)).is_equal(Vector2i(1536, 864))
	assert_that(LEVEL.logical_output_size(Vector2i(1920, 1080), 144)).is_equal(Vector2i(1280, 720))
	assert_that(LEVEL.logical_output_size(Vector2i(960, 600), 96)).is_equal(Vector2i(960, 600))


## Fullscreen 1920x1080 at 125%, Integer: the C sees 1536x864 and a x4 image
## of 1280 px: 180 raw counts = 180 × 320 / 1280 = 45 game pixels.
## Measured: C 0.133 unit per count, Godot which divided by 1600: 0.103.
func test_maintainer_fullscreen_125_percent_divides_by_the_c_image() -> void:
	var c := LEVEL.c_output_rect_size(LEVEL.logical_output_size(Vector2i(1920, 1080), 120), 1, 1)
	assert_that(c).is_equal(Vector2i(1280, 800))
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(180, 0), c).x).is_equal(45)


## Window 1200x750 physical at 125% (960x600 logical for the C): the C
## divides by 960 (x3), as without scaling. Measured: C 0.175, Godot
## which divided by 900/1.25 = 720: 0.215 (23% too fast).
func test_window_125_percent_divides_by_960_like_the_c() -> void:
	var c := LEVEL.c_output_rect_size(LEVEL.logical_output_size(Vector2i(1200, 750), 120), 1, 1)
	assert_that(c).is_equal(Vector2i(960, 600))
	assert_int(LEVEL.scale_window_distance_to_screen(Vector2i(180, 0), c).x).is_equal(60)


## 150% (the maintainer's middle screen, not measured): fullscreen 1920x1080 =
## 1280x720 logical, Integer x3 image = 960 px.
func test_fullscreen_150_percent_c_image_is_x3() -> void:
	var c := LEVEL.c_output_rect_size(LEVEL.logical_output_size(Vector2i(1920, 1080), 144), 1, 1)
	assert_that(c).is_equal(Vector2i(960, 600))


## video.c:calc_dst_render_rect, other Scaling Modes (logical window 1536x864).
func test_c_output_rect_other_scaling_modes() -> void:
	var win := Vector2i(1536, 864)
	assert_that(LEVEL.c_output_rect_size(win, 0, 3)).is_equal(Vector2i(960, 600))
	assert_that(LEVEL.c_output_rect_size(win, 2, 1)).is_equal(Vector2i(1382, 864))
	assert_that(LEVEL.c_output_rect_size(win, 3, 1)).is_equal(Vector2i(1152, 864))
	assert_that(LEVEL.c_output_rect_size(win, 4, 1)).is_equal(Vector2i(1536, 864))
	assert_that(LEVEL.c_output_rect_size(win, 5, 1)).is_equal(win)
