# Memory of the window size and position (divergence no. 58).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")
const SCREENS := [Rect2i(0, 0, 1920, 1080), Rect2i(1920, 0, 1280, 1024)]


func before_test() -> void:
	SetupScreen.reset_settings()


func after_test() -> void:
	SetupScreen.reset_settings()


func test_no_saved_rect_means_nothing_to_restore() -> void:
	assert_vector(Vector2(SetupScreen.saved_window_rect().size)).is_equal(Vector2.ZERO)


func test_saved_rect_roundtrip_through_settings() -> void:
	SetupScreen.settings["window_rect"] = [300, 200, 1100, 700]
	assert_object(SetupScreen.saved_window_rect()).is_equal(Rect2i(300, 200, 1100, 700))


func test_unreadable_saved_rect_is_ignored() -> void:
	for bad in [[1, 2, 3], "x", [1, 2, "a", 4], null]:
		SetupScreen.settings["window_rect"] = bad
		assert_vector(Vector2(SetupScreen.saved_window_rect().size)).is_equal(Vector2.ZERO)


func test_rect_on_a_connected_screen_is_kept() -> void:
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(300, 200, 1100, 700), SCREENS)).is_true()
	# Second screen.
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(2000, 100, 800, 600), SCREENS)).is_true()
	# Half off screen, but the title bar remains grabbable.
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(-300, 50, 900, 600), SCREENS)).is_true()


func test_rect_off_every_screen_is_refused() -> void:
	# Screen unplugged: the rectangle was on the second screen.
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(2000, 100, 800, 600), [SCREENS[0]])).is_false()
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(5000, 4000, 900, 600), SCREENS)).is_false()
	# Reduced resolution: the title bar would be below the bottom of the screen.
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(100, 900, 900, 600), [Rect2i(0, 0, 1280, 720)])).is_false()
	# Absurd size.
	assert_bool(SetupScreen.window_rect_on_screen(Rect2i(10, 10, 0, 0), SCREENS)).is_false()


func test_remember_window_writes_the_settings_file() -> void:
	SetupScreen.remember_window(Rect2i(120, 80, 1000, 650))
	SetupScreen.reset_settings()
	SetupScreen.load_settings()
	assert_object(SetupScreen.saved_window_rect()).is_equal(Rect2i(120, 80, 1000, 650))
