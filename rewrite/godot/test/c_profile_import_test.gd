# C profile import: keys, mutators, music
# engine, sprites, achievements. The values come from the save_json.c format.
extends GdUnitTestSuite

var _dir := ""


func before_test() -> void:
	_dir = ProjectSettings.globalize_path("user://c_profile_import_test_%d" % Time.get_ticks_usec())
	DirAccess.make_dir_recursive_absolute(_dir)


func after_test() -> void:
	for f in DirAccess.get_files_at(_dir):
		DirAccess.remove_absolute(_dir.path_join(f))
	DirAccess.remove_absolute(_dir)


func test_sdl_key_names_become_godot_keycodes() -> void:
	assert_int(CProfile.key_from_sdl_name("Up")).is_equal(KEY_UP)
	assert_int(CProfile.key_from_sdl_name("Space")).is_equal(KEY_SPACE)
	assert_int(CProfile.key_from_sdl_name("Return")).is_equal(KEY_ENTER)
	assert_int(CProfile.key_from_sdl_name("Left Ctrl")).is_equal(KEY_CTRL)
	assert_int(CProfile.key_from_sdl_name("A")).is_equal(KEY_A)
	assert_int(CProfile.key_from_sdl_name("F5")).is_equal(KEY_F5)
	assert_int(CProfile.key_from_sdl_name("")).is_equal(0)
	assert_int(CProfile.key_from_sdl_name("not a key")).is_equal(0)


func test_c_keyboard_is_written_as_player_one_keys() -> void:
	var root := {"keyboard": {"up": "W", "fire": "Z", "enhanced": {"quick_save": "F6", "bullet_time": "Tab"}}}
	var path := _dir.path_join("controls.cfg")
	assert_bool(CProfile._write_controls(root, path)).is_true()
	var cfg := ConfigFile.new()
	assert_int(cfg.load(path)).is_equal(OK)
	assert_array(cfg.get_value("player1", "up_keys")).is_equal([KEY_W])
	assert_array(cfg.get_value("player1", "fire_keys")).is_equal([KEY_Z])
	assert_int(int(cfg.get_value("global", "key_quick_save_key"))).is_equal(KEY_F6)
	# Slow motion keeps Godot's key (divergence no. 31).
	assert_bool(cfg.has_section_key("global", "key_bullet_time_key")).is_false()
	# Keys not given keep their default: nothing is written for them.
	assert_bool(cfg.has_section_key("player1", "down_keys")).is_false()


func test_c_keyboard_import_really_rebinds_the_game_keys() -> void:
	var root := {"keyboard": {"fire": "Z"}}
	var path := _dir.path_join("controls.cfg")
	assert_bool(CProfile._write_controls(root, path)).is_true()
	var saved := TyrianControls.save_config(_dir.path_join("before.cfg"))
	assert_int(saved).is_equal(OK)
	assert_bool(TyrianControls.load_config(path)).is_true()
	var keys := []
	for ev in InputMap.action_get_events(TyrianControls.action_name(1, "fire")):
		if ev is InputEventKey:
			keys.append(ev.physical_keycode)
	TyrianControls.load_config(_dir.path_join("before.cfg"))
	assert_array(keys).is_equal([KEY_Z])


func test_c_mutators_and_music_engine_are_imported() -> void:
	var root := {
		"mutators": {"armored": true, "mirror": true},
		"features": {"music_backend": "classic", "opl_backend": "nuked"},
	}
	var path := _dir.path_join("setup.cfg")
	assert_bool(CProfile._write_setup(root, path)).is_true()
	var cfg := ConfigFile.new()
	assert_int(cfg.load(path)).is_equal(OK)
	assert_int(int(cfg.get_value("setup", "mutators"))).is_equal((1 << 1) | (1 << 6))
	# Classic + Nuked = Authentic; the C numbering is marked in the file.
	assert_int(int(cfg.get_value("setup", "music_engine"))).is_equal(Music.ENGINE_AUTHENTIC)
	assert_int(int(cfg.get_value("format", "music_engine"))).is_equal(2)


## M-06: save_json.c:load_mouse, left / right / middle buttons.
func test_c_mouse_buttons_are_imported() -> void:
	var root := {
		"mouse":
		{"left_mouse": "fire both sidekicks", "right_mouse": "change rear mode", "middle_mouse": "fire main weapon"}
	}
	var path := _dir.path_join("setup.cfg")
	assert_bool(CProfile._write_setup(root, path)).is_true()
	var cfg := ConfigFile.new()
	assert_int(cfg.load(path)).is_equal(OK)
	assert_array(cfg.get_value("setup", "mouse_buttons")).is_equal([4, 5, 1])


func test_music_engine_of_the_c_backends() -> void:
	assert_int(CProfile.music_engine_of("enhanced", "dosbox")).is_equal(Music.ENGINE_ENHANCED)
	assert_int(CProfile.music_engine_of("classic", "dosbox")).is_equal(Music.ENGINE_CLASSIC)
	assert_int(CProfile.music_engine_of("classic", "nuked")).is_equal(Music.ENGINE_AUTHENTIC)


func test_c_sprite_mode_is_imported_as_the_matching_art_mode() -> void:
	var path := _dir.path_join("display.cfg")
	assert_bool(CProfile._write_display({"features": {"sprite_mode": "remastered"}}, path)).is_true()
	var cfg := ConfigFile.new()
	assert_int(cfg.load(path)).is_equal(OK)
	assert_int(int(cfg.get_value("display", "art_mode"))).is_equal(DisplaySettings.ArtMode.HD)
	assert_bool(CProfile._write_display({"features": {"sprite_mode": "upscaled"}}, path)).is_true()
	cfg.load(path)
	assert_int(int(cfg.get_value("display", "art_mode"))).is_equal(DisplaySettings.ArtMode.SHARP)


## C screen (video.fullscreen, -1 = window): an explicit value >= 0 is kept
## (screen + 1); -1, the C default, leaves the rewrite's default (fullscreen).
func test_c_fullscreen_value_is_imported_only_when_explicit() -> void:
	var path := _dir.path_join("setup.cfg")
	assert_bool(CProfile._write_setup({"video": {"fullscreen": 1}, "audio": {"music_volume": 100}}, path)).is_true()
	var cfg := ConfigFile.new()
	assert_int(cfg.load(path)).is_equal(OK)
	assert_int(int(cfg.get_value("setup", "display"))).is_equal(2)
	assert_bool(CProfile._write_setup({"video": {"fullscreen": -1}, "audio": {"music_volume": 100}}, path)).is_true()
	cfg = ConfigFile.new()
	assert_int(cfg.load(path)).is_equal(OK)
	assert_bool(cfg.has_section_key("setup", "display")).is_false()
