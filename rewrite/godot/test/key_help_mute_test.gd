# Up-to-date key help and sound mute key (divergence no. 57).
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const SetupScreen := preload("res://scripts/setup.gd")
const MENU_KEY_CODES := [KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_ENTER, KEY_SPACE, KEY_ESCAPE, KEY_TAB]


func before_test() -> void:
	C.install()
	C.reset_defaults()
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func after_test() -> void:
	C.reset_defaults()
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


## Each key named by the help is really bound to a command, and the
## text carries the key's name (never "%s" nor a stale hard-coded name).
func test_every_key_named_in_help_is_bound() -> void:
	for line in C.HELP_KEYS:
		var action: StringName = line[1]
		if action == &"":
			continue
		var code := C.global_key(action)
		assert_int(code).override_failure_message("%s: no key bound" % line[0]).is_not_equal(KEY_NONE)
		var found := false
		for g in C.GLOBAL_COMMANDS:
			found = found or g[0] == action
		assert_bool(found).override_failure_message("%s: not a common command" % line[0]).is_true()
		for lang in ["en", "fr", "de", "es", "it", "pt"]:
			TyrianText.language = lang
			var text := C.help_line(line[0], action)
			assert_str(text).override_failure_message("%s/%s: %s" % [lang, line[0], text]).not_contains("%s")
			assert_str(text).override_failure_message("%s/%s: key not translated" % [lang, line[0]]).not_contains(
				"keyhelp_"
			)
	TyrianText.language = "en"


## The help keys follow the bindings: changing the key changes the help.
func test_help_follows_rebinding() -> void:
	TyrianText.language = "en"
	assert_str(C.help_line("quick_save", C.KEY_QUICK_SAVE)).contains("F5")
	assert_bool(C.rebind_global(C.KEY_QUICK_SAVE, KEY_F13)).is_true()
	assert_str(C.help_line("quick_save", C.KEY_QUICK_SAVE)).contains("F13")
	assert_str(C.help_line("bullet_time", C.KEY_BULLET_TIME)).contains("Left Shift")


## No more stale hint (B key, Tab for slow motion) in any language.
func test_help_has_no_stale_keys() -> void:
	for lang in ["en", "fr", "de", "es", "it", "pt"]:
		TyrianText.language = lang
		for line in C.HELP_KEYS:
			var text := C.help_line(line[0], line[1])
			assert_str(text).override_failure_message(text).not_contains("~B~").not_contains("Tab")
	TyrianText.language = "en"


## The mute key is that of no other shortcut, no game key
## nor menu key, and appears in the controls screen.
func test_mute_key_is_free_everywhere() -> void:
	var mute := C.global_key(C.KEY_MUTE)
	for g in C.GLOBAL_COMMANDS:
		if g[0] != C.KEY_MUTE:
			assert_int(C.global_key(g[0])).override_failure_message(str(g[0])).is_not_equal(mute)
	assert_bool(C.is_game_key(mute)).is_false()
	assert_bool(mute in MENU_KEY_CODES).is_false()
	for action in C.MENU_ACTIONS:
		for k in C.MENU_ACTIONS[action][0]:
			assert_int(k).override_failure_message(str(action)).is_not_equal(mute)
	assert_bool(C.GLOBAL_COMMANDS.any(func(g: Array) -> bool: return g[0] == C.KEY_MUTE)).is_true()


## Mute: separate flag, volumes and "disabled" settings unchanged,
## exact restoration, state kept after rereading the file.
func test_mute_is_separate_flag_and_restores_sound() -> void:
	SetupScreen.settings.music_volume = 120
	SetupScreen.settings.fx_volume = 90
	SetupScreen.apply_settings()
	var music_db: float = Music._player.volume_db
	var sfx_db: float = Sfx._players[0].volume_db
	assert_bool(Music.muted).is_false()
	assert_bool(Sfx.enabled).is_true()
	Effects.set_muted(true)
	assert_bool(Music.muted).is_true()
	assert_float(Music._player.volume_db).is_equal(-80.0)
	assert_bool(Sfx.enabled).is_false()
	assert_int(SetupScreen.settings.music_volume).is_equal(120)
	assert_int(SetupScreen.settings.fx_volume).is_equal(90)
	assert_bool(SetupScreen.settings.music_disabled).is_false()
	assert_bool(SetupScreen.settings.samples_disabled).is_false()
	# Kept after relaunch.
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	SetupScreen.load_settings()
	assert_bool(SetupScreen.settings.sound_muted).is_true()
	assert_bool(Music.muted).is_true()
	Effects.toggle_mute()
	assert_bool(Music.muted).is_false()
	assert_bool(Sfx.enabled).is_true()
	assert_float(Music._player.volume_db).is_equal(music_db)
	assert_float(Sfx._players[0].volume_db).is_equal(sfx_db)


## Restoring the sound does not turn back on what the player had turned off in Setup.
func test_unmute_keeps_setup_disabled_flags() -> void:
	SetupScreen.settings.music_disabled = true
	SetupScreen.settings.samples_disabled = true
	SetupScreen.apply_settings()
	Effects.set_muted(true)
	Effects.set_muted(false)
	assert_bool(Music.muted).is_true()
	assert_bool(Sfx.enabled).is_false()
	SetupScreen.settings.music_disabled = false
	SetupScreen.settings.samples_disabled = false
	SetupScreen.apply_settings()
	assert_bool(Music.muted).is_false()
	assert_bool(Sfx.enabled).is_true()


## Pressing the key (real event) mutes then restores.
func test_mute_key_press_toggles() -> void:
	var ev := C.key_event(C.global_key(C.KEY_MUTE))
	Effects._unhandled_input(ev)
	assert_bool(Music.muted).is_true()
	Effects._unhandled_input(ev)
	assert_bool(Music.muted).is_false()
