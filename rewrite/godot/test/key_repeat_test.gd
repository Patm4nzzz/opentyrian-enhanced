## Key repeat tests (keyboard.c, joystick.c):
## - mouseInactive: any keyboard key makes the mouse inactive;
##   a real movement (relative != 0) reactivates it.
## - OS repeat (echo = true): accepted for directions (Down),
##   ignored for Enter (TyrianControls.REPEATABLE).
## - Gamepad (MenuRepeat): presses at 0 / 300 / 600 ms with an
##   injectable clock (Effects.repeat_clock).
extends GdUnitTestSuite


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	Effects.repeat_clock = func() -> int: return Time.get_ticks_msec()
	Effects.menu_repeat.clear()
	TyrianMouse.inactive = true


## keyboard.c: SDL_KEYDOWN makes the mouse inactive.
func test_keydown_makes_mouse_inactive() -> void:
	TyrianMouse.inactive = false
	var kev := InputEventKey.new()
	kev.physical_keycode = KEY_DOWN
	kev.keycode = KEY_DOWN
	kev.pressed = true
	TyrianMouse.note(kev)
	assert_bool(TyrianMouse.inactive).is_true()


## keyboard.c: SDL_MOUSEMOTION relative!=0 reactivates the mouse.
func test_mouse_motion_reactivates() -> void:
	TyrianMouse.inactive = true
	var mev := InputEventMouseMotion.new()
	mev.relative = Vector2(2, 0)
	TyrianMouse.note(mev)
	assert_bool(TyrianMouse.inactive).is_false()


## keyboard.c: relative=0 does not change inactive.
func test_zero_motion_keeps_inactive() -> void:
	TyrianMouse.inactive = true
	var mev := InputEventMouseMotion.new()
	mev.relative = Vector2.ZERO
	TyrianMouse.note(mev)
	assert_bool(TyrianMouse.inactive).is_true()


## Held gamepad makes the mouse inactive.
func test_joypad_makes_mouse_inactive() -> void:
	TyrianMouse.inactive = false
	var jev := InputEventJoypadButton.new()
	jev.pressed = true
	TyrianMouse.note(jev)
	assert_bool(TyrianMouse.inactive).is_true()


## Down echo=true accepted (REPEATABLE).
func test_echo_down_accepted() -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_DOWN
	ev.keycode = KEY_DOWN
	ev.pressed = true
	ev.echo = true
	var action := TyrianControls.menu_action(ev)
	assert_str(action).is_equal(TyrianControls.DOWN)


## Enter echo=true ignored.
func test_echo_enter_ignored() -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_ENTER
	ev.keycode = KEY_ENTER
	ev.pressed = true
	ev.echo = true
	var action := TyrianControls.menu_action(ev)
	assert_str(action).is_equal(&"")


## Esc echo=true ignored.
func test_echo_escape_ignored() -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_ESCAPE
	ev.keycode = KEY_ESCAPE
	ev.pressed = true
	ev.echo = true
	var action := TyrianControls.menu_action(ev)
	assert_str(action).is_equal(&"")


## PageUp echo=true accepted.
func test_echo_page_up_accepted() -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_PAGEUP
	ev.keycode = KEY_PAGEUP
	ev.pressed = true
	ev.echo = true
	var action := TyrianControls.menu_action(ev)
	assert_str(action).is_equal(&"tyrian_menu_page_up")


## First feed -> immediate edge, repeat at 300 ms.
func test_menu_repeat_first_and_delay() -> void:
	var mr := MenuRepeat.new()
	var jev := InputEventJoypadButton.new()
	jev.button_index = JOY_BUTTON_DPAD_DOWN
	jev.pressed = true
	var is_new := mr.feed(jev, 0)
	assert_bool(is_new).is_true()
	assert_bool(mr.tick(299).is_empty()).is_true()
	var due := mr.tick(300)
	assert_bool(due.is_empty()).is_false()
	assert_str(due[0]).is_equal(&"tyrian_menu_down")


## Second repeat at 600 ms.
func test_menu_repeat_second_at_600ms() -> void:
	var mr := MenuRepeat.new()
	var jev := InputEventJoypadButton.new()
	jev.button_index = JOY_BUTTON_DPAD_DOWN
	jev.pressed = true
	mr.feed(jev, 0)
	mr.tick(300)
	assert_bool(mr.tick(599).is_empty()).is_true()
	assert_bool(mr.tick(600).is_empty()).is_false()


## Release: stop.
func test_menu_repeat_release_stops() -> void:
	var mr := MenuRepeat.new()
	var press := InputEventJoypadButton.new()
	press.button_index = JOY_BUTTON_DPAD_DOWN
	press.pressed = true
	mr.feed(press, 0)
	var release := InputEventJoypadButton.new()
	release.button_index = JOY_BUTTON_DPAD_DOWN
	release.pressed = false
	mr.feed(release, 100)
	assert_bool(mr.tick(300).is_empty()).is_true()


## Effects.repeat_tick with an injectable clock.
func test_effects_repeat_clock_injectable() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	assert_object(title).is_not_null()
	await get_tree().process_frame
	var t := 0
	Effects.repeat_clock = func() -> int: return t
	Effects.menu_repeat.clear()
	Effects.repeat_pushed = 0
	var jev := InputEventJoypadButton.new()
	jev.button_index = JOY_BUTTON_DPAD_DOWN
	jev.pressed = true
	Effects.menu_repeat.feed(jev, t)
	t = 299
	Effects.repeat_tick(t)
	assert_int(Effects.repeat_pushed).is_equal(0)
	t = 300
	Effects.repeat_tick(t)
	assert_int(Effects.repeat_pushed).is_equal(1)
	t = 600
	Effects.repeat_tick(t)
	assert_int(Effects.repeat_pushed).is_equal(2)
