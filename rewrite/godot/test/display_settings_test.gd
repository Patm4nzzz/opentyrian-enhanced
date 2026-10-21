# Display options (review point 17, ADR 0124): graphics rendering
# (Original / Sharp pixel art) is saved to user://display.cfg, read back
# at startup, and drives the xBRZ filter; HD (AI) is offered only
# if its images (upscaled/) are present (ADR 0100).
extends GdUnitTestSuite

var _saved: PackedByteArray
var _had_file := false


func before_test() -> void:
	_had_file = FileAccess.file_exists(DisplaySettings.PATH)
	if _had_file:
		_saved = FileAccess.get_file_as_bytes(DisplaySettings.PATH)
		DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	DisplaySettings._loaded = false


## Restores the file and the previous state (the suites share a process).
func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	if _had_file:
		var f := FileAccess.open(DisplaySettings.PATH, FileAccess.WRITE)
		f.store_buffer(_saved)
		f.close()
	DisplaySettings._loaded = false
	DisplaySettings.UPSCALED = DisplaySettings._upscaled_dir()
	Effects.apply_display_settings()


func test_default_is_original() -> void:
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)
	assert_bool(DisplaySettings.hd_sprites()).is_false()


func test_sharp_mode_persists_and_drives_xbrz() -> void:
	DisplaySettings.cycle_art_mode()
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.SHARP)
	assert_bool(Effects.xbrz).is_true()
	# Read back as at the next game launch.
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.SHARP)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(DisplaySettings.PATH)).is_equal(OK)
	assert_int(int(cfg.get_value("display", "art_mode", -1))).is_equal(DisplaySettings.ArtMode.SHARP)
	# Back to Original (after HD (AI) if its images are there): saved
	# too, filter off.
	DisplaySettings.cycle_art_mode()
	if DisplaySettings.hd_available():
		assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.HD)
		assert_bool(Effects.xbrz).is_false()
		DisplaySettings.cycle_art_mode()
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)
	assert_bool(Effects.xbrz).is_false()


func test_hd_mode_offered_only_with_its_images() -> void:
	var modes := DisplaySettings.screen_art_modes()
	assert_bool(modes.has(DisplaySettings.ArtMode.HD)).is_equal(DisplaySettings.hd_available())
	# A full cycle of the toggle goes through every offered mode, then Original.
	var seen := []
	for i in modes.size():
		DisplaySettings.cycle_art_mode()
		seen.append(DisplaySettings.art_mode())
	assert_array(seen).contains_exactly_in_any_order(modes)
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)


## Light package (no upscaled/, HD pack not installed): HD (AI) saved
## by a full version is rendered as Original, the screen shows it that way, the
## toggle does not offer it, and the saved choice stays for the pack.
func test_saved_hd_without_pack_renders_original() -> void:
	DisplaySettings.UPSCALED = ProjectSettings.globalize_path("user://no-hd-pack-here")
	var cfg := ConfigFile.new()
	cfg.set_value("display", "art_mode", DisplaySettings.ArtMode.HD)
	cfg.save(DisplaySettings.PATH)
	DisplaySettings._loaded = false
	assert_bool(DisplaySettings.hd_available()).is_false()
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.HD)
	assert_int(DisplaySettings.effective_art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)
	assert_bool(DisplaySettings.hd_sprites()).is_false()
	assert_bool(DisplaySettings.screen_art_modes().has(DisplaySettings.ArtMode.HD)).is_false()
	# Toggle from the rendered mode (Original): Sharp pixel art, then Original.
	DisplaySettings.cycle_art_mode()
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.SHARP)
	DisplaySettings.cycle_art_mode()
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)


func test_invalid_saved_value_falls_back_to_original() -> void:
	var cfg := ConfigFile.new()
	cfg.set_value("display", "art_mode", 99)
	cfg.save(DisplaySettings.PATH)
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.art_mode()).is_equal(DisplaySettings.ArtMode.ORIGINAL)


## CRT filter (F2 / option) saved to display.cfg, read back as at the
## next launch. Bullet time is never written there (bullet_time.c: level
## runtime state).
func test_crt_persists_and_bullet_time_is_never_saved() -> void:
	var crt_before: int = DisplaySettings.crt_mode()
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.OFF)
	Effects.toggle_crt()
	assert_bool(Effects.crt).is_true()
	assert_int(DisplaySettings.crt_mode()).is_equal(DisplaySettings.CrtMode.PLUS)
	# F4 outside a level: no effect, nothing saved.
	var f4 := InputEventKey.new()
	f4.physical_keycode = KEY_F4
	f4.pressed = true
	Effects._unhandled_input(f4)
	assert_bool(Effects.bullet_time).is_false()
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.crt_mode()).override_failure_message("CRT not read back").is_equal(
		DisplaySettings.CrtMode.PLUS
	)
	var cfg := ConfigFile.new()
	cfg.load(DisplaySettings.PATH)
	assert_bool(cfg.has_section_key("display", "crt_mode")).is_true()
	assert_bool(cfg.has_section_key("display", "bullet_time")).is_false()
	Effects.toggle_crt()
	DisplaySettings._loaded = false
	assert_bool(DisplaySettings.crt()).is_false()
	DisplaySettings.set_crt_mode(crt_before)


## Scaler, Scaling Mode and the six CRT modes: saved to display.cfg,
## read back at the next launch, applied by Effects (screen factor, filter).
func test_scaler_scaling_mode_and_crt_modes_persist() -> void:
	DisplaySettings.set_scaler(5)  # Scale3x, C default (video_scale.c:scaler)
	assert_int(DisplaySettings.scaler()).is_equal(5)
	assert_int(DisplaySettings.scaler_factor()).is_equal(3)
	DisplaySettings.set_scaling_mode(3)  # Fit 4:3
	assert_int(DisplaySettings.scaling_mode()).is_equal(3)
	DisplaySettings._loaded = false
	assert_int(DisplaySettings.scaler()).is_equal(5)
	assert_int(DisplaySettings.scaling_mode()).is_equal(3)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(DisplaySettings.PATH)).is_equal(OK)
	assert_int(int(cfg.get_value("display", "scaler", -1))).is_equal(5)
	assert_int(int(cfg.get_value("display", "scaling_mode", -1))).is_equal(3)
	# The six CRT modes read back identically.
	for mode in DisplaySettings.CrtMode.size():
		DisplaySettings.set_crt_mode(mode)
		DisplaySettings._loaded = false
		assert_int(DisplaySettings.crt_mode()).override_failure_message("mode %d" % mode).is_equal(mode)
	DisplaySettings.set_crt_mode(0)


# F10: Right and Left cycle the row's value in both directions
# (like Setup > Graphics), without touching "Back".
func test_display_screen_left_right_cycle_value() -> void:
	var screen := DisplaySettings.new()
	add_child(screen)
	screen.selected = 1  # CRT filter
	var start := DisplaySettings.crt_mode()
	screen.handle_action(TyrianControls.RIGHT)
	assert_int(DisplaySettings.crt_mode()).is_equal((start + 1) % DisplaySettings.CrtMode.size())
	screen.handle_action(TyrianControls.LEFT)
	assert_int(DisplaySettings.crt_mode()).is_equal(start)
	screen.handle_action(TyrianControls.LEFT)
	assert_int(DisplaySettings.crt_mode()).is_equal(
		(start + DisplaySettings.CrtMode.size() - 1) % DisplaySettings.CrtMode.size()
	)
	screen.selected = 4  # Back: no effect, we stay on the screen
	screen.handle_action(TyrianControls.RIGHT)
	assert_bool(screen.is_inside_tree()).is_true()
	screen.queue_free()


## D3: Fit 16:9 and Fit 4:3 use a 320x180 / 320x240 logical base; the
## pointer position is mapped back to the game's 320x200 (video.c:mapWindowPointToScreen).
func test_fit_modes_map_pointer_to_game_pixels() -> void:
	var saved: Vector2 = Effects._screen.size
	var ev := InputEventMouseMotion.new()
	for case in [
		[Vector2(320, 180), Vector2(160, 90), Vector2(160, 100)],
		[Vector2(320, 180), Vector2(160, 170), Vector2(160, 188.888)],
		[Vector2(320, 240), Vector2(160, 120), Vector2(160, 100)],
		[Vector2(320, 200), Vector2(160, 150), Vector2(160, 150)]
	]:
		Effects._screen.size = case[0]
		ev.position = case[1]
		ev.global_position = case[1]
		var mapped: InputEventMouseMotion = Effects.mouse_event_in_game_pixels(ev)
		assert_float(mapped.position.y).override_failure_message("base %s" % case[0]).is_equal_approx(case[2].y, 0.01)
		assert_float(mapped.position.x).is_equal_approx(case[2].x, 0.01)
	Effects._screen.size = saved


## Center mode in a window resized by hand (3840x1812, x3 scaler): the
## 960x600 image is centered at (1440, 606). The window position is mapped
## from the image's corner, not from the window's (the hover and the drawn
## pointer were off by (480, 202) game pixels, i.e. out of the image).
func test_center_mode_maps_pointer_from_the_centered_image() -> void:
	var saved_size: Vector2 = Effects._screen.size
	var saved_base: Vector2 = Effects._screen_base
	Effects._screen.size = Vector2(960, 600)
	Effects._screen_base = (Vector2(3840, 1812) - Vector2(960, 600)) * 0.5
	var ev := InputEventMouseMotion.new()
	for case in [
		[Vector2(1920, 906), Vector2(160, 100)],
		[Vector2(1440, 606), Vector2.ZERO],
		[Vector2(1440 + 3 * 126, 606 + 3 * 158), Vector2(126, 158)]
	]:
		ev.position = case[0]
		ev.global_position = case[0]
		ev.relative = Vector2(3, 0)
		var mapped: InputEventMouseMotion = Effects.mouse_event_in_game_pixels(ev)
		assert_vector(mapped.position).is_equal_approx(case[1], Vector2(0.01, 0.01))
		assert_vector(mapped.relative).is_equal_approx(Vector2(1, 0), Vector2(0.01, 0.01))
	# x1 scaler: 320x200 image, offset still removed.
	Effects._screen.size = Vector2(320, 200)
	Effects._screen_base = Vector2(100, 50)
	ev.position = Vector2(260, 150)
	assert_vector(Effects.mouse_event_in_game_pixels(ev).position).is_equal(Vector2(160, 100))
	Effects._screen.size = saved_size
	Effects._screen_base = saved_base
