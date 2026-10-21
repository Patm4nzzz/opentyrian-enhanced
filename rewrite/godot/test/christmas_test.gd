# Christmas prompt (xmas.c:xmas_prompt) and Christmas mode (opentyr.c: xmas).
extends GdUnitTestSuite

const XmasPrompt := preload("res://scripts/xmas_prompt.gd")

var _saved_active := false
var _saved_options := {}


func before_test() -> void:
	_saved_active = Christmas.active
	_saved_options = Capture._options.duplicate()


func after_test() -> void:
	Christmas.active = _saved_active
	Capture._options = _saved_options


func _screen() -> Node:
	return XmasPrompt.new()


# --- Season -------------------------------------------------------------------


func test_december_is_in_season() -> void:
	# December 15, 2026, noon UTC.
	Capture._options["unix-time"] = "1797336000"
	assert_bool(Christmas.in_season()).is_true()


func test_other_months_are_out_of_season() -> void:
	# November 15, 2026 and January 15, 2027, noon UTC.
	for t in ["1794744000", "1799928000"]:
		Capture._options["unix-time"] = t
		assert_bool(Christmas.in_season()).is_false()


# --- Mouse zones (xmas.c: Yes 120..159, No 160..199, y 120..132) --------


func test_choice_zones() -> void:
	assert_int(XmasPrompt.choice_at(Vector2(120, 120))).is_equal(0)
	assert_int(XmasPrompt.choice_at(Vector2(159, 132))).is_equal(0)
	assert_int(XmasPrompt.choice_at(Vector2(160, 126))).is_equal(1)
	assert_int(XmasPrompt.choice_at(Vector2(199, 126))).is_equal(1)


func test_outside_choice_zones() -> void:
	assert_int(XmasPrompt.choice_at(Vector2(119, 126))).is_equal(-1)
	assert_int(XmasPrompt.choice_at(Vector2(200, 126))).is_equal(-1)
	assert_int(XmasPrompt.choice_at(Vector2(140, 119))).is_equal(-1)
	assert_int(XmasPrompt.choice_at(Vector2(140, 133))).is_equal(-1)


# --- Keyboard (xmas.c: left / right wrap around, Enter / Space confirm, Esc cancels) --


func test_accept_on_default_choice_is_yes() -> void:
	var s := _screen()
	s.handle_action(TyrianControls.ACCEPT)
	assert_int(s.answer).is_equal(1)
	s.free()


func test_left_and_right_wrap_between_the_two_choices() -> void:
	var s := _screen()
	s.handle_action(TyrianControls.RIGHT)
	assert_int(s.selected).is_equal(1)
	s.handle_action(TyrianControls.RIGHT)
	assert_int(s.selected).is_equal(0)
	s.handle_action(TyrianControls.LEFT)
	assert_int(s.selected).is_equal(1)
	s.handle_action(TyrianControls.ACCEPT)
	assert_int(s.answer).is_equal(0)
	s.free()


func test_back_cancels_even_on_yes() -> void:
	var s := _screen()
	s.handle_action(TyrianControls.BACK)
	assert_int(s.answer).is_equal(0)
	s.free()


func test_first_answer_wins() -> void:
	var s := _screen()
	s.handle_action(TyrianControls.BACK)
	s.handle_action(TyrianControls.ACCEPT)
	assert_int(s.answer).is_equal(0)
	s.free()


# --- Mouse -------------------------------------------------------------------


func _click(button: MouseButton, pos: Vector2) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.button_index = button
	ev.pressed = true
	ev.position = pos
	return ev


func test_hover_selects_the_word_under_the_pointer() -> void:
	var s := _screen()
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(180, 126)
	ev.relative = Vector2(3, 0)
	s.handle_mouse(ev)
	assert_int(s.selected).is_equal(1)
	assert_int(s.answer).is_equal(-1)
	s.free()


func test_left_click_on_a_word_validates_it() -> void:
	var s := _screen()
	s.handle_mouse(_click(MOUSE_BUTTON_LEFT, Vector2(180, 126)))
	assert_int(s.answer).is_equal(0)
	var t := _screen()
	t.handle_mouse(_click(MOUSE_BUTTON_LEFT, Vector2(140, 126)))
	assert_int(t.answer).is_equal(1)
	s.free()
	t.free()


func test_left_click_elsewhere_does_nothing() -> void:
	var s := _screen()
	s.handle_mouse(_click(MOUSE_BUTTON_LEFT, Vector2(40, 40)))
	assert_int(s.answer).is_equal(-1)
	s.free()


func test_right_click_cancels_anywhere() -> void:
	var s := _screen()
	s.handle_mouse(_click(MOUSE_BUTTON_RIGHT, Vector2(40, 40)))
	assert_int(s.answer).is_equal(0)
	s.free()


# --- Christmas resources ---------------------------------------------------------


func test_voices_come_from_voicesc_only_when_active() -> void:
	Christmas.active = false
	assert_str(Christmas.sfx_path(33)).is_equal("sfx/33.wav")
	Christmas.active = true
	assert_str(Christmas.sfx_path(33)).is_equal("sfx/christmas/33.wav")
	# The tyrian.snd sounds (1..31) do not change.
	assert_str(Christmas.sfx_path(5)).is_equal("sfx/5.wav")


func test_christmas_sheets_replace_shots_ships_and_powerups() -> void:
	Christmas.active = false
	var plain := TyrianAssets.sheet("main_08_player_ships")
	Christmas.active = true
	var festive := TyrianAssets.sheet("main_08_player_ships")
	assert_object(festive).is_not_same(plain)
