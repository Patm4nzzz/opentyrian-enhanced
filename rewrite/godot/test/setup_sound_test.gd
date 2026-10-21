# Setup > Sound (sound_menu.c:sound_items): Stereo, SFX Impact Scaling,
# three-choice music engine (Classic / Authentic / Enhanced) and Music
# Widen. Each row is reached by real keyboard events in
# Setup, its effect is measured on the audio output (dummy driver: the
# mixer runs in headless) or on the player, and the setting is read back.
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")

## Rows of _sound_items (C order).
const ROW_STEREO := 2
const ROW_IMPACT := 4
const ROW_ENGINE := 5
const ROW_WIDEN := 6


func before() -> void:
	Music._released = false
	Sfx._released = false


func after() -> void:
	Sfx.play_in_headless = false
	Music.release_streams()
	Sfx.release_streams()


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	Sfx.play_in_headless = true
	Sfx._released = false
	Music._released = false


func after_test() -> void:
	Sfx.play_in_headless = false
	for p in Sfx._players:
		p.stop()
	Music.stop_song()
	Music.song_playing = -1
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


## Real key (Input.parse_input_event -> Setup's _unhandled_input).
func _press(code: int) -> void:
	for pressed in [true, false]:
		var ev := InputEventKey.new()
		ev.physical_keycode = code as Key
		ev.keycode = code as Key
		ev.pressed = pressed
		Input.parse_input_event(ev)
		Input.flush_buffered_events()
	await get_tree().process_frame


## Setup open, Sound menu opened with Down, Down, Enter; cursor on `row`.
func _open_sound(row: int) -> Node:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	for i in 120:
		await get_tree().process_frame
		if setup.idle:
			break
	assert_bool(setup.idle).override_failure_message("Setup ready").is_true()
	await _press(KEY_DOWN)
	await _press(KEY_ENTER)
	assert_str(setup.menus.current().header).is_equal("snd_header")
	for i in row:
		await _press(KEY_DOWN)
	assert_int(setup.menus.current().sel).is_equal(row)
	return setup


## Rereads the file saved by Setup.
func _saved(key: String) -> Variant:
	var cfg := ConfigFile.new()
	assert_int(cfg.load(SetupScreen.SETTINGS_PATH)).is_equal(OK)
	return cfg.get_value("setup", key)


## Left / right energy of a bus, measured over `frames` mixed frames.
func _measure(bus: StringName, seconds := 0.25) -> Vector2:
	var idx := AudioServer.get_bus_index(bus)
	var cap := AudioEffectCapture.new()
	AudioServer.add_bus_effect(idx, cap)
	cap.clear_buffer()
	var energy := Vector2.ZERO
	var t := 0.0
	while t < seconds:
		await get_tree().process_frame
		t += get_process_delta_time()
		var n := cap.get_frames_available()
		if n > 0:
			for f in cap.get_buffer(n):
				energy += Vector2(absf(f.x), absf(f.y))
	AudioServer.remove_bus_effect(idx, AudioServer.get_bus_effect_count(idx) - 1)
	return energy


func test_sound_menu_lists_the_c_rows() -> void:
	var setup: Node = auto_free(load("res://scenes/setup.tscn").instantiate())
	var labels: Array = setup._sound_items().map(func(it): return it.label)
	assert_array(labels).contains_exactly(
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


## Stereo: an effect on the left of the screen comes out on the left; Stereo off
## (Enter on the row), it is centered. Panning from loudness.c.
func test_stereo_pans_effects_by_screen_position() -> void:
	await _open_sound(ROW_STEREO)
	assert_bool(Sfx.stereo).is_true()
	# Enemy explosion at x = 30 (left edge of the play area), channel 6.
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 30, 0)
	var g := Sfx.channel_gains(6)
	assert_float(g.x).is_equal(1.0)
	assert_float(g.y).is_less(0.1)
	var e := await _measure(&"Sfx6")
	assert_float(e.x).override_failure_message("sound heard on the left: %s" % e).is_greater(0.0)
	assert_float(e.y).override_failure_message("almost nothing on the right: %s" % e).is_less(e.x * 0.1)
	# Effect on the right (x = 280): the reverse.
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 280, 0)
	e = await _measure(&"Sfx6")
	assert_float(e.x).is_less(e.y * 0.1)
	await _press(KEY_ENTER)
	assert_bool(SetupScreen.settings.stereo_enabled).is_false()
	assert_bool(Sfx.stereo).is_false()
	assert_bool(_saved("stereo_enabled")).is_false()
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 30, 0)
	assert_object(Sfx.channel_gains(6)).is_equal(Vector2.ONE)
	e = await _measure(&"Sfx6")
	assert_float(absf(e.x - e.y)).override_failure_message("centred: %s" % e).is_less(e.x * 0.01)
	# Read back at launch.
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	assert_bool(Sfx.stereo).is_true()
	SetupScreen.load_settings()
	assert_bool(Sfx.stereo).is_false()


## SFX Impact Scaling: a downed boss (soundQueueDmg 3) explodes lower
## (0.65) and at full volume; off, like an ordinary effect.
func test_impact_scaling_follows_damage() -> void:
	Sfx.pitch_variation = false
	await _open_sound(ROW_IMPACT)
	assert_bool(Sfx.impact_scaling).is_true()
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 160, 0)
	var plain_db: float = Sfx._players[6].volume_db
	assert_float(Sfx._players[6].pitch_scale).is_equal(1.0)
	var plain := await _measure(&"Sfx6")
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 160, 3)
	assert_float(Sfx._players[6].pitch_scale).is_equal_approx(0.65, 0.0001)
	assert_float(Sfx._players[6].volume_db - plain_db).is_equal_approx(Sfx.level_db(4) - Sfx.level_db(2), 0.001)
	var boss := await _measure(&"Sfx6")
	assert_float(boss.x).override_failure_message("boss %s > ordinary %s" % [boss, plain]).is_greater(plain.x * 1.2)
	# Big enemy (dmg 2): 0.80 and level (2 + 4) / 2 = 3.
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 160, 2)
	assert_float(Sfx._players[6].pitch_scale).is_equal_approx(0.8, 0.0001)
	assert_float(Sfx._players[6].volume_db - plain_db).is_equal_approx(Sfx.level_db(3) - Sfx.level_db(2), 0.001)
	# Turned off by Left (menu engine toggle).
	await _press(KEY_LEFT)
	assert_bool(Sfx.impact_scaling).is_false()
	assert_bool(_saved("sfx_impact_scaling")).is_false()
	Sfx.play_queued(6, Sfx.EXPLOSION_9, 160, 3)
	assert_float(Sfx._players[6].pitch_scale).is_equal(1.0)
	assert_float(Sfx._players[6].volume_db).is_equal_approx(plain_db, 0.001)
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	SetupScreen.load_settings()
	assert_bool(Sfx.impact_scaling).is_false()
	Sfx.pitch_variation = true


## The simulation provides the queue (channel, sample, x, damage): a played
## level outputs shots at the ship's position and enemy explosions
## with their magnitude.
func test_level_sounds_come_from_simulation_queue() -> void:
	var sim: Node = auto_free(ClassDB.instantiate("TyrianSim"))
	assert_bool(sim.load_level(Game.data_dir, 1, 2, 2, 42)).is_true()
	var shots := 0
	var xs := {}
	var kills := 0
	for t in 1500:
		var dir := 4 if (t / 90) % 2 == 0 else 8
		sim.step(16 | dir, 0)
		var q: PackedInt32Array = sim.take_sounds()
		for k in range(0, q.size(), 4):
			if q[k] == 0:
				shots += 1
				xs[q[k + 2] / 40] = true
			if q[k] == 6 and q[k + 3] > 0:
				kills += 1
	assert_int(shots).is_greater(50)
	assert_int(xs.size()).override_failure_message("shots spread out: %s" % xs).is_greater(2)
	assert_int(kills).is_greater(0)


## Music engine: Right goes from Classic to Authentic (Nuked rendering, stream
## loaded and different), then Enhanced (MIDI); saved with the new
## numbering and read back.
func test_engine_picker_three_choices_and_authentic_stream() -> void:
	(
		assert_bool(Music.has_authentic())
		. override_failure_message("Nuked render missing (assets/imported/music_nuked)")
		. is_true()
	)
	# The default is Enhanced (like the C): the test starts from Classic.
	SetupScreen.settings.music_engine = Music.ENGINE_CLASSIC
	SetupScreen.apply_settings()
	Music.play_song(Music.SONG_TITLE)
	var classic: AudioStream = Music._player.stream
	assert_object(classic).is_not_null()
	var setup := await _open_sound(ROW_ENGINE)
	assert_str(MenuEngine.value_suffix(setup.menus.current().items[ROW_ENGINE])).is_equal(
		TyrianText.L("snd_engine_classic")
	)
	await _press(KEY_RIGHT)
	assert_int(Music.engine).is_equal(Music.ENGINE_AUTHENTIC)
	assert_int(_saved("music_engine")).is_equal(1)
	var authentic: AudioStream = Music._player.stream
	assert_object(authentic).is_not_null()
	assert_bool(authentic == classic).is_false()
	assert_str(authentic.resource_path).contains("music_nuked")
	assert_bool(Music._player.playing).is_true()
	# Different samples: the two OGGs are not the same file.
	var a := FileAccess.get_file_as_bytes(Music.MUSIC_DIR.path_join("song_29.ogg"))
	var b := FileAccess.get_file_as_bytes(Music.MUSIC_NUKED_DIR.path_join("song_29.ogg"))
	assert_bool(a.size() > 0 and b.size() > 0 and a != b).is_true()
	await _press(KEY_RIGHT)
	assert_int(Music.engine).is_equal(Music.ENGINE_ENHANCED)
	assert_int(_saved("music_engine")).is_equal(2)
	await _press(KEY_RIGHT)
	assert_int(Music.engine).is_equal(Music.ENGINE_CLASSIC)
	await _press(KEY_LEFT)
	assert_int(Music.engine).is_equal(Music.ENGINE_ENHANCED)
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	assert_int(Music.engine).is_equal(Music.ENGINE_ENHANCED)
	SetupScreen.load_settings()
	assert_int(Music.engine).is_equal(Music.ENGINE_ENHANCED)


## File written before this change (0 Original OPL, 1 MIDI, no [format]):
## MIDI becomes Enhanced (2), OPL stays Classic (0); re-saved in format 2.
func test_old_music_engine_value_is_migrated() -> void:
	for pair in [[1, Music.ENGINE_ENHANCED], [0, Music.ENGINE_CLASSIC]]:
		var cfg := ConfigFile.new()
		cfg.set_value("setup", "music_engine", pair[0])
		cfg.save(SetupScreen.SETTINGS_PATH)
		SetupScreen.reset_settings()
		SetupScreen.load_settings()
		assert_int(SetupScreen.settings.music_engine).is_equal(pair[1])
		assert_int(Music.engine).is_equal(pair[1])
	SetupScreen.settings.music_engine = Music.ENGINE_AUTHENTIC
	SetupScreen.save_settings()
	var cfg := ConfigFile.new()
	cfg.load(SetupScreen.SETTINGS_PATH)
	assert_int(cfg.get_value("format", "music_engine", 0)).is_equal(SetupScreen.MUSIC_ENGINE_FORMAT)
	SetupScreen.reset_settings()
	SetupScreen.load_settings()
	assert_int(Music.engine).is_equal(Music.ENGINE_AUTHENTIC)


## Music Widen: the OPL (mono, left = right) comes out with a left / right
## difference (loudness.c:ps_widen); off, left = right.
func test_music_widen_makes_left_and_right_differ() -> void:
	# The default is Enhanced (like the C); the widening measured here is the OPL's.
	SetupScreen.settings.music_engine = Music.ENGINE_CLASSIC
	SetupScreen.apply_settings()
	Music.play_song(Music.SONG_TITLE)
	await _open_sound(ROW_WIDEN)
	var idx := AudioServer.get_bus_index(Music.BUS_OPL)
	assert_bool(AudioServer.is_bus_effect_enabled(idx, 0)).is_false()
	var diff := await _measure_lr_difference()
	assert_float(diff.x).override_failure_message("mono without widening: %s" % diff).is_less(diff.y * 0.001)
	await _press(KEY_ENTER)
	assert_bool(Music.widen).is_true()
	assert_bool(_saved("music_pseudo_stereo")).is_true()
	assert_bool(AudioServer.is_bus_effect_enabled(idx, 0)).is_true()
	diff = await _measure_lr_difference()
	assert_float(diff.x).override_failure_message("widened: %s" % diff).is_greater(diff.y * 0.02)
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	assert_bool(Music.widen).is_false()
	SetupScreen.load_settings()
	assert_bool(Music.widen).is_true()


## (Σ|l - r|, Σ|l| + |r|) on the output of the OPL music bus.
func _measure_lr_difference() -> Vector2:
	var idx := AudioServer.get_bus_index(Music.BUS_OPL)
	var cap := AudioEffectCapture.new()
	AudioServer.add_bus_effect(idx, cap)
	cap.clear_buffer()
	var out := Vector2.ZERO
	var t := 0.0
	while t < 0.4:
		await get_tree().process_frame
		t += get_process_delta_time()
		var n := cap.get_frames_available()
		if n > 0:
			for f in cap.get_buffer(n):
				out += Vector2(absf(f.x - f.y), absf(f.x) + absf(f.y))
	AudioServer.remove_bus_effect(idx, AudioServer.get_bus_effect_count(idx) - 1)
	return out
