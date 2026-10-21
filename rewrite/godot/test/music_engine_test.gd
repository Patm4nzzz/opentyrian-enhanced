# Music engine (Setup > Sound, point 8): choice Classic (OPL DOSBox) /
# Authentic (OPL Nuked, setup_sound_test.gd) / Enhanced (MIDI) saved
# in user://setup.cfg and read back at launch; a missing MIDI track or
# SoundFont replays the OPL version (warning), never a
# silence. Real playback is proven by the synth's cargo test
# (tyrian-godot midi::tests) and the saving of both modes.
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")
const SCRATCH := "user://music_engine_test"


## No track loaded: play_song only reloads if the track changes.
func _forget_song() -> void:
	Music._stop_midi()
	Music.song_playing = -1
	# SoundFont reloaded: each case chooses its data/ folder.
	Music._midi = null
	Music._midi_loaded_from = ""


## Previous suites may have released the streams (release_streams, program
## shutdown): this suite replays tracks, then releases them in turn.
func before() -> void:
	Music._released = false


func after() -> void:
	Music.release_streams()


func before_test() -> void:
	_forget_song()
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func after_test() -> void:
	Music.stop_song()
	_forget_song()
	Music.midi_data_dir = ""
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	_remove_scratch()


func _remove_scratch() -> void:
	var dir := ProjectSettings.globalize_path(SCRATCH)
	for sub in ["midi", ""]:
		var d := dir.path_join(sub)
		if not DirAccess.dir_exists_absolute(d):
			continue
		for f in DirAccess.get_files_at(d):
			DirAccess.remove_absolute(d.path_join(f))
		DirAccess.remove_absolute(d)


## Test data/ folder: the SoundFont (and `midi_files`) of the real data/.
func _scratch_data(with_soundfont: bool, midi_files: Array) -> String:
	var dir := ProjectSettings.globalize_path(SCRATCH)
	DirAccess.make_dir_recursive_absolute(dir.path_join("midi"))
	if with_soundfont:
		DirAccess.copy_absolute(Game.data_dir.path_join("soundfont.sf2"), dir.path_join("soundfont.sf2"))
	for f in midi_files:
		DirAccess.copy_absolute(Game.data_dir.path_join("midi").path_join(f), dir.path_join("midi").path_join(f))
	return dir


# loudness.c:40-45: with FluidSynth (published C versions), a fresh profile
# starts on the Enhanced engine.
func test_engine_defaults_to_enhanced_like_the_c() -> void:
	assert_int(SetupScreen.settings.music_engine).is_equal(Music.ENGINE_ENHANCED)
	assert_int(SetupScreen.DEFAULTS.music_engine).is_equal(Music.ENGINE_ENHANCED)


## Write, put the defaults back, reread the file: the MIDI choice comes back and
## is applied to the player.
func test_engine_choice_persisted_and_applied() -> void:
	SetupScreen.settings.music_engine = Music.ENGINE_MIDI
	SetupScreen.apply_settings()
	SetupScreen.save_settings()
	assert_int(Music.engine).is_equal(Music.ENGINE_MIDI)
	SetupScreen.reset_settings()
	SetupScreen.settings.music_engine = Music.ENGINE_CLASSIC
	SetupScreen.apply_settings()
	assert_int(Music.engine).is_equal(Music.ENGINE_OPL)
	var cfg := ConfigFile.new()
	assert_int(cfg.load(SetupScreen.SETTINGS_PATH)).is_equal(OK)
	assert_int(cfg.get_value("setup", "music_engine", -1)).is_equal(Music.ENGINE_MIDI)
	SetupScreen.load_settings()
	assert_int(SetupScreen.settings.music_engine).is_equal(Music.ENGINE_MIDI)
	assert_int(Music.engine).is_equal(Music.ENGINE_MIDI)


## The Sound menu row goes through the menu engine: left / right
## change the choice, which is saved right away.
func test_sound_menu_picker_switches_and_saves() -> void:
	var setup: Node = auto_free(load("res://scenes/setup.tscn").instantiate())
	var items: Array = setup._sound_items()
	var picker: Dictionary = items.filter(func(it): return it.label == "snd_music_engine")[0]
	# The default is Enhanced (like the C): the test starts from Classic.
	SetupScreen.settings.music_engine = Music.ENGINE_CLASSIC
	assert_int(picker.kind).is_equal(MenuEngine.Kind.PICKER)
	assert_array(picker.labels).contains_exactly(["snd_engine_classic", "snd_engine_authentic", "snd_engine_enhanced"])
	assert_str(MenuEngine.value_suffix(picker)).is_equal(TyrianText.L("snd_engine_classic"))
	# Left from Classic: the choice wraps to Enhanced (sound_menu.c, PICKER).
	MenuEngine.new(setup).adjust_horizontal(picker, -1, -1)
	assert_int(SetupScreen.settings.music_engine).is_equal(Music.ENGINE_MIDI)
	assert_str(MenuEngine.value_suffix(picker)).is_equal(TyrianText.L("snd_engine_enhanced"))
	SetupScreen.apply_settings()
	SetupScreen.save_settings()
	SetupScreen.reset_settings()
	SetupScreen.load_settings()
	assert_int(Music.engine).is_equal(Music.ENGINE_MIDI)


## MIDI mode with complete data/: the title goes through the synth; back to
## Original: the same track restarts in the OPL.
func test_midi_mode_plays_through_synth() -> void:
	Music.set_engine(Music.ENGINE_MIDI)
	Music.play_song(Music.SONG_TITLE)
	assert_bool(Music.is_midi_playing()).is_true()
	assert_bool(Music.is_playing()).is_true()
	Music.set_engine(Music.ENGINE_OPL)
	assert_bool(Music.is_midi_playing()).is_false()
	assert_int(Music.song_playing).is_equal(Music.SONG_TITLE)
	assert_object(Music._player.stream).is_not_null()


## Track without .mid: its OPL version plays, the next track that has its
## .mid goes back to MIDI (loudness.c: per-track fallback).
func test_missing_midi_file_falls_back_to_opl() -> void:
	Music.midi_data_dir = _scratch_data(true, ["29-tyrian-main-title.mid"])
	Music.set_engine(Music.ENGINE_MIDI)
	Music.play_song(Music.SONG_HIGH_SCORES)
	assert_bool(Music.is_midi_playing()).is_false()
	assert_object(Music._player.stream).is_not_null()
	assert_bool(Music._player.playing).is_true()
	Music.play_song(Music.SONG_TITLE)
	assert_bool(Music.is_midi_playing()).is_true()
	# The player's choice stays MIDI.
	assert_int(Music.engine).is_equal(Music.ENGINE_MIDI)


## SoundFont missing: everything goes through the OPL, the MIDI choice stays saved.
func test_missing_soundfont_falls_back_to_opl() -> void:
	Music.midi_data_dir = _scratch_data(false, ["29-tyrian-main-title.mid"])
	SetupScreen.settings.music_engine = Music.ENGINE_MIDI
	SetupScreen.apply_settings()
	Music.play_song(Music.SONG_TITLE)
	assert_bool(Music.is_midi_playing()).is_false()
	assert_object(Music._player.stream).is_not_null()
	assert_bool(Music._player.playing).is_true()
	assert_int(SetupScreen.settings.music_engine).is_equal(Music.ENGINE_MIDI)
