# Start screen: fullscreen by default, last mode kept (Alt+Enter,
# Setup), one-time migration of old profiles (docs/divergences.md no. 56).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")


func before_test() -> void:
	_remove_file()
	SetupScreen.reset_settings()


func after_test() -> void:
	_remove_file()
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func _remove_file() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))


func _config(display: Variant, marker: Variant = null) -> ConfigFile:
	var cfg := ConfigFile.new()
	if display != null:
		cfg.set_value("setup", "display", display)
	if marker != null:
		cfg.set_value("format", "display_default", marker)
	return cfg


func test_new_profile_starts_fullscreen_on_primary_screen() -> void:
	var r := SetupScreen.resolve_display(ConfigFile.new(), false, 0)
	assert_int(r.display).is_equal(1)
	assert_bool(r.migrated).is_false()
	assert_int(SetupScreen.resolve_display(ConfigFile.new(), false, 1).display).is_equal(2)
	# File present but without a display key: same default.
	assert_int(SetupScreen.resolve_display(_config(null), true, 0).display).is_equal(1)


func test_old_profile_with_display_zero_migrates_once() -> void:
	var r := SetupScreen.resolve_display(_config(0), true, 0)
	assert_int(r.display).is_equal(1)
	assert_bool(r.migrated).is_true()


func test_choice_is_respected_after_migration() -> void:
	# Marker present: window chosen by the player, never migrated again.
	var r := SetupScreen.resolve_display(_config(0, SetupScreen.DISPLAY_DEFAULT_FORMAT), true, 0)
	assert_int(r.display).is_equal(0)
	assert_bool(r.migrated).is_false()
	# Fullscreen on screen 2 chosen by the player.
	assert_int(SetupScreen.resolve_display(_config(2), true, 0).display).is_equal(2)


func test_remember_display_saves_and_reloads() -> void:
	SetupScreen.remember_display(0)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(SetupScreen.SETTINGS_PATH)).is_equal(OK)
	assert_int(int(cfg.get_value("setup", "display"))).is_equal(0)
	assert_int(int(cfg.get_value("format", "display_default"))).is_equal(SetupScreen.DISPLAY_DEFAULT_FORMAT)
	SetupScreen.reset_settings()
	SetupScreen.load_settings()
	assert_int(SetupScreen.settings.display).is_equal(0)
	SetupScreen.remember_display(1)
	SetupScreen.reset_settings()
	SetupScreen.load_settings()
	assert_int(SetupScreen.settings.display).is_equal(1)


func test_first_load_of_old_profile_writes_marker() -> void:
	var cfg := _config(0)
	cfg.save(SetupScreen.SETTINGS_PATH)
	SetupScreen.load_settings()
	assert_int(SetupScreen.settings.display).is_equal(1)
	var saved := ConfigFile.new()
	saved.load(SetupScreen.SETTINGS_PATH)
	assert_int(int(saved.get_value("format", "display_default", 0))).is_equal(SetupScreen.DISPLAY_DEFAULT_FORMAT)
	# The player goes back to windowed: read back as is at the next launch.
	SetupScreen.remember_display(0)
	SetupScreen.reset_settings()
	SetupScreen.load_settings()
	assert_int(SetupScreen.settings.display).is_equal(0)
