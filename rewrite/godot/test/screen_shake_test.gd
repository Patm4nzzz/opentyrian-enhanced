# Screen shakes (screenshake.c): carried into the rendering, turned off by the
# Visual FX preset (visual_fx.c: Off = {false, false, false, 1}).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")
const FIRE := 16
const LEFT := 4
const RIGHT := 8


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	Effects.shake_px = Vector2i.ZERO
	Effects.kill_slowmo = -1.0
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	await get_tree().process_frame


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()


func _start_level() -> Node:
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 7
	Game.difficulty = 2
	Game.loadout = PackedInt32Array([1, 4, 7, 0, 1, 4, 2, 0, 0, 0, 10000])
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	level.interactive = false
	await get_tree().process_frame
	return level


## Integer offsets seen during `ticks` ticks (permanent fire, sweep).
func _offsets(level: Node, ticks: int) -> Array:
	var seen: Array = []
	for t in ticks:
		level.step_sim(FIRE | (LEFT if (t / 90) % 2 == 0 else RIGHT))
		seen.append(Effects.shake_px)
	return seen


func test_preset_sets_screenshake_like_the_c() -> void:
	# visual_fx.c:preset_table : Off {false}, Basic {true}, Extended {true}.
	SetupScreen.settings.visual_fx = 1
	SetupScreen.apply_settings()
	assert_bool(SetupScreen.settings.screenshake_enabled).is_true()
	SetupScreen.settings.visual_fx = 0
	SetupScreen.apply_settings()
	assert_bool(SetupScreen.settings.screenshake_enabled).is_false()
	assert_int(Game.processor_type).is_equal(1)
	SetupScreen.settings.visual_fx = 2
	SetupScreen.apply_settings()
	assert_bool(SetupScreen.settings.screenshake_enabled).is_true()
	# Saved with the rest of Setup.
	SetupScreen.settings.visual_fx = 0
	SetupScreen.apply_settings()
	SetupScreen.save_settings()
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	SetupScreen.load_settings()
	assert_bool(SetupScreen.settings.screenshake_enabled).is_false()


func test_level_shakes_and_settles_when_enabled() -> void:
	var level := await _start_level()
	var seen := _offsets(level, 1500)
	var moved := seen.filter(func(o: Vector2i) -> bool: return o != Vector2i.ZERO)
	assert_int(moved.size()).override_failure_message("no shake in 1500 ticks").is_greater(0)
	# The next level starts from zero (screenshake_reset).
	level.queue_free()
	await get_tree().process_frame
	assert_object(Effects.shake_px).is_equal(Vector2i.ZERO)


func test_no_shake_when_preset_off() -> void:
	SetupScreen.settings.visual_fx = 0
	SetupScreen.apply_settings()
	var level := await _start_level()
	var seen := _offsets(level, 1500)
	assert_bool(seen.all(func(o: Vector2i) -> bool: return o == Vector2i.ZERO)).is_true()


## mainint.c:JE_inGameSetup presents the screen every turn (JE_showVGA) and video.c:449
## decays the shake at each presentation: a pause menu opened during a shake
## falls back to 0 instead of staying offset (player-smoke's Quit box offset by 4 px).
func test_shake_settles_while_pause_menu_is_open() -> void:
	var level := await _start_level()
	var t := 0
	while Effects.shake_px == Vector2i.ZERO and t < 1500:
		level.step_sim(FIRE | (LEFT if (t / 90) % 2 == 0 else RIGHT))
		t += 1
	assert_object(Effects.shake_px).override_failure_message("no shake in 1500 ticks").is_not_equal(Vector2i.ZERO)
	level.interactive = true
	level._entry_fade = null  # level launched non-interactively: no fade-in running
	level.open_pause_menu()
	for i in 60:
		level._physics_process(1.0 / 35.0)
	assert_object(Effects.shake_px).override_failure_message("pause menu still offset").is_equal(Vector2i.ZERO)


## video.c:449-455: the shake offsets the image by `shake_px` WINDOW pixels, whatever the
## scaler factor and the mode (the offset must not be multiplied by content_scale).
func test_shake_offset_on_screen_is_in_window_pixels() -> void:
	for mode in [0, 1, 2, 5]:
		for scaler_index in [0, 5]:
			Effects.scaler = scaler_index
			Effects.scaling_mode = mode
			Effects.shake_px = Vector2i.ZERO
			Effects.shake_px = Vector2i(12, -6)
			var shown: Vector2 = Effects.shake_window_offset()
			(
				assert_float(shown.x)
				. override_failure_message(
					"mode %d scaler %d: x = %s window px, expected 12" % [mode, scaler_index, shown.x]
				)
				. is_equal_approx(12.0, 0.01)
			)
			(
				assert_float(shown.y)
				. override_failure_message(
					"mode %d scaler %d: y = %s window px, expected -6" % [mode, scaler_index, shown.y]
				)
				. is_equal_approx(-6.0, 0.01)
			)
	Effects.shake_px = Vector2i.ZERO
