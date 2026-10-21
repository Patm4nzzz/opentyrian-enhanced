# ">> SLOW MOTION <<" indicator (enemy_render.c:213): below the top text
# and below the achievement banner, which sets hud_top_text_bottom = notify_y + 9
# (achievements.c:1587-1588).
extends GdUnitTestSuite


func test_without_banner_follows_top_text() -> void:
	assert_int(HudRenderer.slow_motion_y(-1, -1)).is_equal(22)
	assert_int(HudRenderer.slow_motion_y(30, -1)).is_equal(32)


func test_banner_at_top_keeps_y_22() -> void:
	# Banner at y 3 (common case): 3 + 9 + 2 = 14 < 22.
	assert_int(HudRenderer.slow_motion_y(-1, 3)).is_equal(22)


func test_banner_pushed_below_top_text_pushes_slow_motion_below_it() -> void:
	# "Secret Level" (bottom 30): banner y 32, slow motion y 43 like the C.
	assert_int(HudRenderer.slow_motion_y(30, 32)).is_equal(43)
	# Boss bar (bottom 12): banner y 14, slow motion y 25.
	assert_int(HudRenderer.slow_motion_y(12, 14)).is_equal(25)
	# Countdown (bottom 14): banner y 16, slow motion y 27.
	assert_int(HudRenderer.slow_motion_y(14, 16)).is_equal(27)


func test_low_banner_with_overlay_open_does_not_move_slow_motion() -> void:
	assert_int(HudRenderer.slow_motion_y(12, LevelOverlays.NOTIFY_Y_WITH_OVERLAY)).is_equal(22)
