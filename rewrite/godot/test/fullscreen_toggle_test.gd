# keyboard.c:toggle_fullscreen (Alt+Enter): exclusive fullscreen (exact screen
# size), return to the previous size and position, forced redraw.
# Needs a real display: skipped in headless.
extends GdUnitTestSuite


func after_test() -> void:
	if DisplayServer.get_name() != "headless":
		Effects.set_fullscreen(false)


func test_fullscreen_is_exclusive_then_windowed_restores_rect_and_repaints() -> void:
	if DisplayServer.get_name() == "headless":
		return
	Effects.set_fullscreen(false)
	await get_tree().process_frame
	DisplayServer.window_set_size(Vector2i(960, 600))
	DisplayServer.window_set_position(Vector2i(100, 80))
	await get_tree().process_frame
	var size := DisplayServer.window_get_size()
	var pos := DisplayServer.window_get_position()
	var repaints := Effects.repaints_done

	Effects.toggle_fullscreen()
	await _wait_until(func(): return Effects.is_fullscreen())
	assert_int(DisplayServer.window_get_mode()).is_equal(DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN)
	assert_bool(Effects.is_fullscreen()).is_true()

	Effects.toggle_fullscreen()
	await _wait_until(func(): return Effects.repaints_done > repaints)
	# The window manager applies the position after the fact.
	await _wait_until(func(): return DisplayServer.window_get_position() == pos)
	assert_int(DisplayServer.window_get_mode()).is_equal(DisplayServer.WINDOW_MODE_WINDOWED)
	assert_vector(Vector2(DisplayServer.window_get_size())).is_equal(Vector2(size))
	assert_vector(Vector2(DisplayServer.window_get_position())).is_equal(Vector2(pos))
	assert_int(Effects.repaints_done).is_equal(repaints + 1)
	# The return is finished: a focus loss becomes a real Alt+Tab again (level pause).
	assert_bool(Effects.leaving_fullscreen).is_false()


## Windows + OpenGL only: the return to windowed goes through the minimized state
## (fake title bar otherwise, measured on the maintainer's machine).
func test_minimized_restore_only_for_windows_opengl() -> void:
	var expected := (
		OS.get_name() == "Windows" and RenderingServer.get_current_rendering_driver_name().begins_with("opengl3")
	)
	assert_bool(Effects._needs_minimized_restore()).is_equal(expected)
	if OS.get_name() != "Windows":
		assert_bool(Effects._needs_minimized_restore()).is_false()


# The window manager answers asynchronously: wait, bounded to 3 s.
func _wait_until(condition: Callable) -> void:
	var end := Time.get_ticks_msec() + 3000
	while not condition.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame
