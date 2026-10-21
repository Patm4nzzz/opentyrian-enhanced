# video.c:init_scaler: changing the scaler resizes the window then
# recenters it (window_center_in_display). Without recentering, the shrunken window
# stayed stuck to the corner where it was. Needs a real display:
# skipped in headless.
extends GdUnitTestSuite

var _scaler := 0


func before_test() -> void:
	_scaler = DisplaySettings.scaler()


func after_test() -> void:
	DisplaySettings.set_scaler(_scaler)
	Effects.apply_display_settings()


func test_scaler_change_recenters_the_window() -> void:
	if DisplayServer.get_name() == "headless":
		return
	var w := get_tree().root
	w.mode = Window.MODE_WINDOWED
	DisplayServer.window_set_size(Vector2i(1280, 800))
	DisplayServer.window_set_position(Vector2i(0, 0))
	DisplaySettings.set_scaler(1)  # 2x : 640 x 400
	Effects.apply_display_settings()
	var area := DisplayServer.screen_get_usable_rect(DisplayServer.window_get_current_screen())
	var expected := area.position + (area.size - Vector2i(640, 400)) / 2
	assert_vector(Vector2(DisplayServer.window_get_position())).is_equal_approx(Vector2(expected), Vector2(2, 2))


# F3 (art mode) never touches the window size: the C only
# resizes it on scaler change.
func test_art_mode_change_keeps_window_size() -> void:
	if DisplayServer.get_name() == "headless":
		return
	var w := get_tree().root
	w.mode = Window.MODE_WINDOWED
	DisplaySettings.set_scaler(0)
	Effects.apply_display_settings()
	DisplayServer.window_set_size(Vector2i(1280, 800))
	var art := DisplaySettings.art_mode()
	for i in 3:
		DisplaySettings.cycle_art_mode()
	assert_vector(Vector2(DisplayServer.window_get_size())).is_equal(Vector2(1280, 800))
	DisplaySettings.set_art_mode(art)
	Effects.apply_display_settings()
