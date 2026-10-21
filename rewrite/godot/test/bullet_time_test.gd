# Bullet time (bullet_time.c): 4 s slow motion in levels only, then
# 15 s cooldown; reset on leaving the level; never saved;
# never a tint outside a level, whatever the old files contain.
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")

var _now := 0
var _level: Node


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	DisplaySettings._loaded = false
	_now = 100000
	Effects.bullet_time_clock = func() -> int: return _now
	Effects.bullet_time_allowed = true
	Effects.bullet_time = false
	Effects._bullet_time_end = 0
	Effects._bullet_time_cooldown_end = 0
	Effects._bullet_time_last_update = _now
	_level = null


func after_test() -> void:
	Effects.bullet_time_clock = func() -> int: return Time.get_ticks_msec()
	Effects.bullet_time = false
	Effects._bullet_time_cooldown_end = 0
	Effects._paused_level = null
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	DisplaySettings._loaded = false
	for child in Effects.game_viewport.get_children():
		Effects.game_viewport.remove_child(child)
		child.free()
	await get_tree().process_frame


## Stand-in for a level for Effects: same scene path.
func _enter_level() -> void:
	for child in Effects.game_viewport.get_children():
		Effects.game_viewport.remove_child(child)
		child.free()
	_level = Node.new()
	_level.scene_file_path = "res://scenes/level.tscn"
	Effects.game_viewport.add_child(_level)


## Slow-motion trigger; the key (left Shift, gamepad) is tested on a
## real level in level_hotkeys_test.gd.
func _press(code: int) -> void:
	if code == KEY_F4:
		Effects.bullet_time_activate()


## The C has no slow-motion tint (bullet_time.c: ">> SLOW MOTION <<" and
## the game at 0.25): the shader no longer has a "bullet_time" parameter.
func _tint() -> float:
	var v: Variant = Effects._material.get_shader_parameter("bullet_time")
	return 0.0 if v == null else float(v)


func test_slow_motion_has_no_color_tint() -> void:
	var src := FileAccess.get_file_as_string("res://shaders/post.gdshader")
	assert_bool("bullet_time" in src).override_failure_message("slow-motion tint absent from the C").is_false()
	_enter_level()
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_true()
	assert_object(Effects._material.get_shader_parameter("bullet_time")).is_null()


## Advances the clock in 100 ms steps, like frames.
func _advance(ms: int) -> void:
	var left := ms
	while left > 0:
		var step := mini(left, 100)
		_now += step
		left -= step
		Effects.bullet_time_update()


func test_old_display_cfg_with_bullet_time_gives_untinted_title_and_is_cleaned() -> void:
	var cfg := ConfigFile.new()
	cfg.set_value("display", "bullet_time", true)
	cfg.set_value("display", "crt_mode", 0)
	cfg.save(DisplaySettings.PATH)
	DisplaySettings._loaded = false
	DisplaySettings.art_mode()
	var back := ConfigFile.new()
	back.load(DisplaySettings.PATH)
	assert_bool(back.has_section_key("display", "bullet_time")).override_failure_message("old key kept").is_false()
	# Startup: saved settings applied, title (outside a level) on screen.
	Effects.crt_mode = DisplaySettings.crt_mode()
	Effects.bullet_time_update()
	assert_bool(Effects.bullet_time).is_false()
	assert_float(_tint()).is_equal(0.0)
	assert_float(Effects.tick_speed()).is_equal(1.0)


func test_activation_lasts_4s_then_15s_cooldown() -> void:
	_enter_level()
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_true()
	assert_float(_tint()).is_equal(0.0)
	assert_float(Effects.tick_speed()).is_equal(0.25)
	assert_int(Effects.bullet_time_uses).is_greater(0)
	_advance(3900)
	assert_bool(Effects.bullet_time).is_true()
	_advance(200)
	assert_bool(Effects.bullet_time).override_failure_message("ends at 4 s").is_false()
	assert_float(_tint()).is_equal(0.0)
	assert_float(Effects.tick_speed()).is_equal(1.0)
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).override_failure_message("recharge ignored").is_false()
	assert_int(Effects.bullet_time_cooldown_seconds()).is_greater(0)
	_advance(14000)
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_false()
	_advance(1100)
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_true()


func test_second_press_during_slowmo_does_not_extend_it() -> void:
	_enter_level()
	_press(KEY_F4)
	_advance(2000)
	var uses := Effects.bullet_time_uses
	_press(KEY_F4)
	assert_int(Effects.bullet_time_uses).is_equal(uses)
	_advance(2100)
	assert_bool(Effects.bullet_time).is_false()


func test_not_active_outside_a_level() -> void:
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_false()
	assert_float(_tint()).is_equal(0.0)
	# Outside a level, the Shift key does not trigger anything either (real event).
	var shift := InputEventKey.new()
	shift.physical_keycode = KEY_SHIFT
	shift.location = KEY_LOCATION_LEFT
	shift.pressed = true
	Effects._unhandled_input(shift)
	assert_bool(Effects.bullet_time).is_false()


func test_leaving_the_level_resets_it() -> void:
	_enter_level()
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_true()
	Effects.show_scene("res://scenes/title.tscn")
	assert_bool(Effects.bullet_time).is_false()
	assert_float(_tint()).is_equal(0.0)
	assert_float(Effects.tick_speed()).is_equal(1.0)


func test_pause_time_is_not_counted() -> void:
	_enter_level()
	_press(KEY_F4)
	_advance(1000)
	# Level suspended under an options screen for 30 s: the budget waits.
	Effects._paused_level = _level
	_advance(30000)
	Effects._paused_level = null
	assert_bool(Effects.bullet_time).override_failure_message("budget consumed while paused").is_true()
	_advance(2800)
	assert_bool(Effects.bullet_time).is_true()
	_advance(300)
	assert_bool(Effects.bullet_time).is_false()


func test_nothing_is_saved() -> void:
	_enter_level()
	_press(KEY_F4)
	_advance(4200)
	Effects.toggle_crt()
	Effects.toggle_crt()
	var cfg := ConfigFile.new()
	cfg.load(DisplaySettings.PATH)
	assert_bool(cfg.has_section_key("display", "bullet_time")).is_false()
	var setup := ConfigFile.new()
	if setup.load(SetupScreen.SETTINGS_PATH) == OK and setup.has_section("setup"):
		for key in setup.get_section_keys("setup"):
			assert_str(key).is_not_equal("bullet_time")


## tyrian2.c:741: the loop delay is divided by game_speed. At 0.25 the level
## plays 1 tick out of 4 (8.75 ticks/s out of 35); Engine.time_scale did not do that.
func test_slow_motion_plays_one_tick_in_four() -> void:
	_enter_level()
	_press(KEY_F4)
	assert_bool(Effects.bullet_time).is_true()
	var played := 0
	for i in 140:
		if Effects.consume_tick():
			played += 1
	assert_int(played).override_failure_message("4 s of slow motion: 35 ticks out of 140").is_equal(35)
	# Boss slow motion: 0.3 -> 1, the level plays 3 ticks out of 10 at 0.3.
	Effects.bullet_time = false
	Effects.kill_slowmo = 0.7
	played = 0
	for i in 100:
		if Effects.consume_tick():
			played += 1
	assert_int(played).is_equal(70)
	Effects.kill_slowmo = -1.0
	for i in 5:
		assert_bool(Effects.consume_tick()).is_true()
