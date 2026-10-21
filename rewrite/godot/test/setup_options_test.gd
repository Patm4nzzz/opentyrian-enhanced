# Setup options (point 4): each displayed option acts on the game, is
# saved to user://setup.cfg and read back at launch; options with no
# effect have disappeared from the screen (docs/setup-options.md).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	TyrianText.language = "en"


## Writes `value`, saves, resets the defaults, rereads: the value comes back.
func _roundtrip(key: String, value: Variant) -> void:
	SetupScreen.settings[key] = value
	SetupScreen.apply_settings()
	SetupScreen.save_settings()
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	assert_that(SetupScreen.settings[key]).override_failure_message("%s reset to default" % key).is_not_equal(value)
	SetupScreen.load_settings()
	assert_that(SetupScreen.settings[key]).override_failure_message("%s read back" % key).is_equal(value)


func test_bullet_time_persisted_and_applied() -> void:
	_roundtrip("bullet_time_enabled", false)
	assert_bool(Effects.bullet_time_allowed).is_false()
	# Forbidden: the option cuts the running slow motion and refuses its trigger.
	Effects.bullet_time = true
	Effects.bullet_time_allowed = false
	assert_bool(Effects.bullet_time).is_false()
	assert_bool(Effects.bullet_time_activate()).is_false()
	assert_bool(Effects.bullet_time).is_false()
	assert_float(Effects.tick_speed()).is_equal(1.0)


func test_combo_persisted_and_applied() -> void:
	_roundtrip("combo_enabled", false)
	assert_bool(Game.combos_enabled).is_false()


func test_auto_fire_persisted_and_applied() -> void:
	_roundtrip("auto_fire_enabled", true)
	assert_bool(Game.auto_fire).is_true()
	assert_int(Game.read_input_mask() & Game.MASK_FIRE).is_equal(Game.MASK_FIRE)
	assert_int(Game.read_input_mask_p2() & Game.MASK_FIRE).is_equal(Game.MASK_FIRE)


func test_language_persisted_and_applied() -> void:
	_roundtrip("current_language", 1)
	assert_str(TyrianText.language).is_equal("fr")


func test_music_volume_and_mute_persisted_and_applied() -> void:
	_roundtrip("music_volume", 95)
	assert_int(Music.user_volume).is_equal(95)
	assert_int(Music.music_volume).is_equal(95)
	_roundtrip("music_disabled", true)
	assert_bool(Music.muted).is_true()
	assert_float(Music._player.volume_db).is_equal(-80.0)


func test_sfx_volume_mute_pitch_persisted_and_applied() -> void:
	_roundtrip("fx_volume", 71)
	assert_int(Sfx.volume).is_equal(71)
	assert_float(Sfx._players[0].volume_db).is_less(-6.0)
	_roundtrip("samples_disabled", true)
	assert_bool(Sfx.enabled).is_false()
	_roundtrip("sfx_pitch_variation", false)
	assert_bool(Sfx.pitch_variation).is_false()


func test_display_persisted() -> void:
	# Fullscreen: no effect in headless (dummy DisplayServer), read back anyway.
	_roundtrip("display", 2)


## The screen only shows wired-up options (no removed row comes back).
func test_setup_screen_lists_only_wired_options() -> void:
	var setup: Node = auto_free(load("res://scenes/setup.tscn").instantiate())
	var labels := func(items: Array) -> Array: return items.map(func(it): return it.label)
	assert_array(labels.call(setup._setup_items())).contains_exactly(
		[
			"setup_graphics",
			"setup_sound",
			"setup_gameplay",
			"setup_keybindings",
			"setup_joystick",
			"setup_mods",
			"setup_jukebox",
			"setup_done"
		]
	)
	assert_array(labels.call(setup._gameplay_items())).contains_exactly(
		[
			"gp_bullet_time",
			"gp_freeze_frames",
			"gp_damage_numbers",
			"gp_combo",
			"gp_auto_fire",
			"gp_rumble",
			"gp_language",
			"gp_mutators",
			"setup_done"
		]
	)
	assert_array(labels.call(setup._sound_items())).contains_exactly(
		[
			"snd_music",
			"snd_sfx",
			"snd_stereo",
			"snd_sfx_pitch",
			"snd_sfx_impact",
			"snd_music_engine",
			"snd_music_widen",
			"setup_done"
		]
	)
	assert_array(labels.call(setup._graphics_items())).contains_exactly(
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
	# Each remaining toggle / selector reads and writes a saved setting.
	for items in [setup._gameplay_items(), setup._sound_items(), setup._graphics_items()]:
		for it in items:
			if it.kind in [MenuEngine.Kind.TOGGLE, MenuEngine.Kind.PICKER]:
				assert_bool(it.has("get") and it.has("set")).is_true()
