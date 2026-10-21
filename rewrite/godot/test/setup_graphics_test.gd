# Setup graphics options (graphics_menu.c): Display, Scaler, Scaling
# Mode, CRT (six modes), Sprites and Visual FX, in the C's order. Each option
# is applied right away (shader, window scaling,
# `config.c:processorType` read by the simulation), saved to
# user://display.cfg (or user://setup.cfg for Visual FX) and read back at the next
# launch. The keys are real keyboard events (Input.parse_input_event).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")

## Rows of the Graphics submenu (graphics_menu.c:graphics_items).
const ROW_SCALER := 1
const ROW_SCALING_MODE := 2
const ROW_CRT := 3
const ROW_SPRITES := 4
const ROW_VISUAL_FX := 5


func before_test() -> void:
	_clean()


func after_test() -> void:
	DisplaySettings.UPSCALED = DisplaySettings._upscaled_dir()
	_clean()
	_clear_scene()
	await get_tree().process_frame


func _clean() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	DisplaySettings._loaded = false
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	Effects.crt_mode = DisplaySettings.CrtMode.OFF
	Effects.bullet_time = false
	Effects.apply_display_settings()


func _clear_scene() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)


## Wait bounded in real time until the menu is at rest
## (setup.gd:handle_action ignores keys during a submenu, `idle`).
func _wait_idle(s: Node) -> void:
	var end := Time.get_ticks_msec() + 5000
	while Time.get_ticks_msec() < end and s.get("idle") != true:
		await get_tree().process_frame
	await get_tree().process_frame


## A key pressed then released, delivered by Godot's input loop.
func _press(code: int, s: Node = null) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = code
	ev.keycode = code
	ev.pressed = true
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	await get_tree().process_frame
	var up := ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()
	await get_tree().process_frame
	if s != null:
		await _wait_idle(s)


## Opens Setup > Graphics (row 0 of Setup) and returns the screen.
func _open_graphics() -> Node:
	var s: Node = Game.goto_scene(Game.SCENE_SETUP)
	var end := Time.get_ticks_msec() + 8000
	while Time.get_ticks_msec() < end and s.get("idle") != true:
		await get_tree().process_frame
	assert_object(s).is_not_null()
	await _wait_idle(s)
	assert_bool(s.get("idle")).override_failure_message("Setup never idle").is_true()
	await _press(KEY_ENTER, s)  # Graphics... (row 0)
	assert_int(s.menus.stack.size()).is_equal(2)
	return s


## Goes down to the `row` row of the Graphics submenu.
func _goto_row(s: Node, row: int) -> void:
	for i in row:
		await _press(KEY_DOWN, s)


func test_graphics_menu_follows_c_order() -> void:
	var s: Node = await _open_graphics()
	var labels: Array = s._graphics_items().map(func(it): return it.label)
	assert_array(labels).contains_exactly(
		[
			"gfx_display",
			"gfx_scaler",
			"gfx_scaling_mode",
			"gfx_crt",
			"gfx_sprites",
			"gfx_visual_fx",
			"gfx_exfx",
			"setup_done"
		]
	)


## Scaler: the C's list (nine entries), applied to the shader, window factor
## (video_scale.c:scalers[]) and saved.
func test_scaler_applied_and_persisted() -> void:
	var s: Node = await _open_graphics()
	# The default is Scale3x (5); the scenario starts from "None".
	DisplaySettings.set_scaler(0)
	await _goto_row(s, ROW_SCALER)
	assert_int(DisplaySettings.scaler()).is_equal(0)
	await _press(KEY_RIGHT, s)  # None -> 2x
	assert_int(DisplaySettings.scaler()).is_equal(1)
	assert_int(DisplaySettings.scaler_factor()).is_equal(2)
	assert_int(Effects._material.get_shader_parameter("scaler")).is_equal(1)
	await _press(KEY_RIGHT, s)  # 2x -> Scale2x
	assert_int(DisplaySettings.scaler()).is_equal(2)
	assert_int(Effects._material.get_shader_parameter("scaler")).is_equal(2)
	# Left: back to the previous one (menu_tree.c:adjust_horizontal).
	await _press(KEY_LEFT, s)
	assert_int(DisplaySettings.scaler()).is_equal(1)
	# Saved: read back as at the next launch.
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.scaler()).is_equal(1)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(DisplaySettings.PATH)).is_equal(OK)
	assert_int(int(cfg.get_value("display", "scaler", -1))).is_equal(1)


## Scaling Mode: the C's modes applied to the window (video.c:calc_dst_render_rect).
func test_scaling_mode_applied_and_persisted() -> void:
	var s: Node = await _open_graphics()
	await _goto_row(s, ROW_SCALING_MODE)
	assert_int(DisplaySettings.scaling_mode()).is_equal(1)  # Integer
	await _press(KEY_RIGHT, s)  # Integer -> Fit 8:5
	assert_int(DisplaySettings.scaling_mode()).is_equal(2)
	var w := Effects.get_window()
	assert_int(w.content_scale_size.x).is_equal(320)
	assert_int(w.content_scale_size.y).is_equal(200)
	await _press(KEY_RIGHT, s)  # Fit 8:5 -> Fit 4:3 (logical base 320x240)
	assert_int(DisplaySettings.scaling_mode()).is_equal(3)
	assert_int(w.content_scale_size.y).is_equal(240)
	await _press(KEY_RIGHT, s)  # Fit 16:9
	assert_int(w.content_scale_size.y).is_equal(180)
	await _press(KEY_RIGHT, s)  # Fill: whole window, ratio ignored
	assert_int(w.content_scale_aspect).is_equal(Window.CONTENT_SCALE_ASPECT_IGNORE)
	# Saved and read back.
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.scaling_mode()).is_equal(5)
	await _press(KEY_RIGHT, s)  # Fill -> Center: native size centered
	assert_int(DisplaySettings.scaling_mode()).is_equal(0)
	assert_int(w.content_scale_mode).is_equal(Window.CONTENT_SCALE_MODE_DISABLED)
	await _press(KEY_RIGHT, s)  # Center -> Integer (wraps around over six modes)
	assert_int(DisplaySettings.scaling_mode()).is_equal(1)
	assert_int(w.content_scale_mode).is_equal(Window.CONTENT_SCALE_MODE_CANVAS_ITEMS)


## CRT: the C's six modes, each with its shader parameters
## (shader.c:shader_render), saved and read back.
func test_crt_six_modes_applied_and_persisted() -> void:
	var s: Node = await _open_graphics()
	await _goto_row(s, ROW_CRT)
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.OFF)
	assert_bool(Effects._material.get_shader_parameter("crt_enabled")).is_false()
	var expected := [
		[0.5, 0.0, 0.0, 0.0, 0.0, 0.0],  # Scanlines
		[0.8, 0.3, 0.0, 0.0, 0.0, 1.0],  # Plus
		[0.85, 0.35, 0.35, 0.0015, 0.0, 1.0],  # Premium
		[0.2, 0.0, 0.2, 0.0, 1.0, 1.0],  # Aperture
		[0.10, 0.0, 0.55, 0.0, 0.0, 1.0],  # Soft
	]
	for mode in expected.size():
		await _press(KEY_RIGHT, s)
		assert_int(DisplaySettings.crt_mode()).override_failure_message("mode %d" % (mode + 1)).is_equal(mode + 1)
		assert_bool(Effects._material.get_shader_parameter("crt_enabled")).is_true()
		assert_float(Effects._material.get_shader_parameter("scanline_intensity")).is_equal_approx(
			expected[mode][0], 1e-5
		)
		assert_float(Effects._material.get_shader_parameter("curvature")).is_equal_approx(expected[mode][1], 1e-5)
		assert_float(Effects._material.get_shader_parameter("phosphor_glow")).is_equal_approx(expected[mode][2], 1e-5)
		assert_float(Effects._material.get_shader_parameter("chroma_shift")).is_equal_approx(expected[mode][3], 1e-5)
		assert_float(Effects._material.get_shader_parameter("aperture_strength")).is_equal_approx(
			expected[mode][4], 1e-5
		)
		assert_float(Effects._material.get_shader_parameter("vignette_strength")).is_equal_approx(
			expected[mode][5], 1e-5
		)
	assert_int(Effects._material.get_shader_parameter("crt_mode")).is_equal(DisplaySettings.CrtMode.SOFT)
	await _press(KEY_RIGHT, s)  # Soft -> Off (six cyclable notches)
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.OFF)
	assert_bool(Effects._material.get_shader_parameter("crt_enabled")).is_false()
	# Saved: read back as at the next launch (display.cfg).
	await _press(KEY_RIGHT, s)
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.SCANLINES)
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.SCANLINES)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(DisplaySettings.PATH)).is_equal(OK)
	assert_int(int(cfg.get_value("display", "crt_mode", -1))).is_equal(DisplaySettings.CrtMode.SCANLINES)


## Migrates the old boolean `[display] crt` (true -> closest mode).
func test_legacy_crt_bool_migrates() -> void:
	var cfg := ConfigFile.new()
	cfg.set_value("display", "crt", true)
	cfg.save(DisplaySettings.PATH)
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.PLUS)


## Sprites: same setting as the Display screen (a single source of truth).
func test_sprites_drive_art_mode() -> void:
	var s: Node = await _open_graphics()
	await _goto_row(s, ROW_SPRITES)
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)
	await _press(KEY_RIGHT, s)  # Original -> Remastered (HD (AI)) or Upscaled (xBRZ)
	var expected_next := DisplaySettings.ArtMode.HD if DisplaySettings.hd_available() else DisplaySettings.ArtMode.SHARP
	assert_int(DisplaySettings.art_mode()).is_equal(expected_next)
	assert_bool(Effects.xbrz).is_equal(expected_next == DisplaySettings.ArtMode.SHARP)
	# Saved: read back as at the next launch.
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.art_mode()).is_equal(expected_next)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(DisplaySettings.PATH)).is_equal(OK)
	assert_int(int(cfg.get_value("display", "art_mode", -1))).is_equal(expected_next)


## Light package (HD pack absent): Sprites only offers Original and
## Upscaled (xBRZ); a saved HD (AI) is displayed and rendered as Original,
## and the arrow moves to Upscaled without error.
func test_sprites_without_hd_pack() -> void:
	DisplaySettings.UPSCALED = ProjectSettings.globalize_path("user://no-hd-pack-here")
	var cfg := ConfigFile.new()
	cfg.set_value("display", "art_mode", DisplaySettings.ArtMode.HD)
	cfg.save(DisplaySettings.PATH)
	DisplaySettings._loaded = false
	var s: Node = await _open_graphics()
	var item: Dictionary = s._sprite_items()
	assert_int(item["count"]).is_equal(2)
	assert_array(item["labels"]).contains_exactly(["gfx_sprite_original", "gfx_sprite_upscaled"])
	assert_int(item["get"].call()).is_equal(0)
	await _goto_row(s, ROW_SPRITES)
	await _press(KEY_RIGHT, s)
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.SHARP)
	assert_bool(Effects.xbrz).is_true()


## Visual Effects: three notches (Off/Basic/Extended) -> config.c:processorType.
func test_visual_fx_drives_processor_type() -> void:
	var s: Node = await _open_graphics()
	await _goto_row(s, ROW_VISUAL_FX)
	assert_int(SetupScreen.settings.visual_fx).is_equal(2)
	assert_int(Game.processor_type).is_equal(4)
	await _press(KEY_RIGHT, s)  # Extended -> Off (wraparound `(get + dir + n) % n`)
	assert_int(SetupScreen.settings.visual_fx).is_equal(0)
	assert_int(Game.processor_type).is_equal(1)
	await _press(KEY_RIGHT, s)  # Off -> Basic
	assert_int(Game.processor_type).is_equal(3)
	await _press(KEY_RIGHT, s)  # Basic -> Extended
	assert_int(Game.processor_type).is_equal(4)
	# Saved: read back as at the next launch.
	SetupScreen.settings.visual_fx = 1
	SetupScreen.apply_settings()
	SetupScreen.save_settings()
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	SetupScreen.load_settings()
	assert_int(SetupScreen.settings.visual_fx).is_equal(1)
	assert_int(Game.processor_type).is_equal(3)


## processorType reaches the simulation: `TyrianSim.set_processor_type`.
func test_processor_type_reaches_sim() -> void:
	var sim = ClassDB.instantiate("TyrianSim")
	add_child(sim)
	assert_int(sim.processor_type()).is_equal(4)
	sim.set_processor_type(1)
	assert_int(sim.processor_type()).is_equal(1)
	sim.set_processor_type(99)  # clamped 1..6 (config.c:JE_initProcessorType)
	assert_int(sim.processor_type()).is_equal(6)
	var ok: bool = sim.load_level(Game.data_dir, 1, 17, 3, 7)
	assert_bool(ok).override_failure_message(sim.last_error()).is_true()
	assert_int(sim.processor_type()).is_equal(6)
	sim.set_processor_type(4)
	assert_int(sim.processor_type()).is_equal(4)
	sim.free()
