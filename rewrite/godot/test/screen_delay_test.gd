## C's setDelay(n) + wait (nortsong.c:57-84): successive waits of the
## 8-bit screens follow a cumulative deadline, whatever the frame rate.
## Regression: one timer per call cost two frames at 70 fps; the ENGAGE effect
## (300 steps, level_helpers.c:JE_whoa) lasted 9 s instead of 4.3 s.
extends GdUnitTestSuite

var _old_fps := 0


func before() -> void:
	_old_fps = Engine.max_fps


func after() -> void:
	Engine.max_fps = _old_fps


func _steps_ms(screen: TyrianScreen, steps: int) -> float:
	var t0 := Time.get_ticks_usec()
	for i in steps:
		await screen.wait_units(1)
	return (Time.get_ticks_usec() - t0) / 1000.0


func test_wait_units_follows_c_period_at_70_fps() -> void:
	Engine.max_fps = 70
	var screen: TyrianScreen = auto_free(TyrianScreen.new())
	add_child(screen)
	await get_tree().process_frame
	var want := 70.0 * TyrianScreen.DELAY_PERIOD * 1000.0
	var got: float = await _steps_ms(screen, 70)
	assert_float(got).override_failure_message("40 steps: %.0f ms, C: %.0f ms" % [got, want]).is_between(
		want * 0.9, want * 1.15
	)


func test_wait_units_or_input_shares_the_deadline() -> void:
	Engine.max_fps = 70
	var screen: TyrianScreen = auto_free(TyrianScreen.new())
	add_child(screen)
	await get_tree().process_frame
	var want := 40.0 * TyrianScreen.DELAY_PERIOD * 1000.0
	var t0 := Time.get_ticks_usec()
	for i in 20:
		await screen.wait_units(1)
		await screen.wait_units_or_input(1)
	var got := (Time.get_ticks_usec() - t0) / 1000.0
	assert_float(got).override_failure_message("40 steps: %.0f ms, C: %.0f ms" % [got, want]).is_between(
		want * 0.9, want * 1.15
	)


## Glow of the story texts (fonthand.c:JE_outCharGlow, setDelay per step):
## End of episode 4 (levels4.dat section 48, ]Wn 03[): 3 units per step, 43 ms; 40 steps last
## 120 units at 70 fps, not 4 frames (57 ms) each.
func test_story_glow_steps_follow_c_period_at_70_fps() -> void:
	Engine.max_fps = 70
	Game.story_script = {}
	var s = auto_free(preload("res://scripts/story_screens.gd").new())
	add_child(s)
	await get_tree().process_frame
	var want := 120.0 * TyrianScreen.DELAY_PERIOD * 1000.0
	var t0 := Time.get_ticks_usec()
	for i in 40:
		await s._wait_with_warning(3)
	var got := (Time.get_ticks_usec() - t0) / 1000.0
	remove_child(s)
	assert_float(got).override_failure_message("40 steps: %.0f ms, C: %.0f ms" % [got, want]).is_between(
		want * 0.9, want * 1.15
	)
