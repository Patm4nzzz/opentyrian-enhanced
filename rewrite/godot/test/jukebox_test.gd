# Jukebox (jukebox.c) opened by Setup -> Jukebox: track order
# (Part 1 before Part 2), navigation with wraparound, stop, track kept for
# levels (G, toggle), effects, exit by fade to Setup on the
# Jukebox row.
extends GdUnitTestSuite

const JUKEBOX_SCENE := "res://scenes/jukebox.tscn"


func before() -> void:
	EndlessSession.stop()
	Game.campaign_active = false
	# No transition left by a previous suite (fade to another
	# track); music turned off like audio_disabled: with no stream
	# (headless), Music.is_playing() is false and the C loop would move
	# to the next track every frame.
	Music.release_streams()
	Music.enabled = false


func after() -> void:
	Music.enabled = true
	Music.ingame_song_override = -1
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	Music.ingame_song_override = -1
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _key(jb: Node, code: int) -> void:
	jb.handle_event(TyrianControls.key_event(code))
	# Key handled after drawing the next frame, like the C.
	for i in 30:
		await get_tree().process_frame
		if jb._key == KEY_NONE:
			return


func _open() -> Node:
	Music.play_song(Music.SONG_TITLE)
	var jb: Node = Game.goto_scene(JUKEBOX_SCENE)
	for i in 60:
		await get_tree().process_frame
		if jb.idle:
			break
	return jb


func test_navigation_order_and_wrap() -> void:
	var jb: Node = await _open()
	# Enter on the current track (title, no. 29: position 30 of 41).
	assert_int(jb._jb_pos).is_equal(29)
	await _key(jb, KEY_RIGHT)
	assert_int(Music.song_playing).is_equal(30)
	# Start of list: "Asteroid Dance, Part 1" (track 1) before Part 2.
	jb._jb_pos = 0
	await _key(jb, KEY_LEFT)
	assert_int(jb._jb_pos).is_equal(Music.song_count() - 1)
	await _key(jb, KEY_DOWN)
	assert_int(Music.song_playing).is_equal(1)
	await _key(jb, KEY_RIGHT)
	assert_int(Music.song_playing).is_equal(0)
	assert_str(Music.title(1)).contains("Part 1")


## Without OPL rendering (music.json missing: build without --music, parity
## inventory environment): 41 tracks and their titles like the C
## (musmast.c:musicTitle), same navigation order; no automatic
## chaining (jukebox.c, audio_disabled).
func test_navigation_without_rendered_music() -> void:
	var saved: Array = Music._songs
	Music._songs = []
	Music.enabled = true
	assert_bool(Music.has_audio()).is_false()
	assert_int(Music.song_count()).is_equal(41)
	assert_str(Music.title(29)).is_equal("Tyrian - Main Title")
	var jb: Node = await _open()
	assert_int(jb._jb_pos).is_equal(29)
	await _key(jb, KEY_RIGHT)
	assert_int(Music.song_playing).is_equal(30)
	assert_int(jb._jb_pos).is_equal(30)
	# Several frames without a key: the track does not change by itself.
	for i in 10:
		await get_tree().process_frame
	assert_int(Music.song_playing).is_equal(30)
	jb._jb_pos = 0
	await _key(jb, KEY_LEFT)
	assert_int(jb._jb_pos).is_equal(40)
	Music._songs = saved
	Music.enabled = false


func test_stop_ingame_override_and_sfx() -> void:
	var jb: Node = await _open()
	await _key(jb, KEY_S)
	assert_bool(jb._stopped).is_true()
	assert_bool(Music.music_stopped).is_true()
	await _key(jb, KEY_R)
	assert_bool(jb._stopped).is_false()
	# G keeps the track for levels, G again cancels it.
	await _key(jb, KEY_G)
	assert_int(Music.ingame_song_override).is_equal(Music.song_playing)
	await _key(jb, KEY_G)
	assert_int(Music.ingame_song_override).is_equal(-1)
	# / : effects; "," from the first wraps to the last (40).
	await _key(jb, KEY_SLASH)
	await _key(jb, KEY_COMMA)
	assert_int(jb._fx_num).is_equal(jb.SOUND_TITLES.size() - 1)
	# G has no effect in effects mode.
	await _key(jb, KEY_G)
	assert_int(Music.ingame_song_override).is_equal(-1)


func test_escape_returns_to_setup_on_jukebox_row() -> void:
	var jb: Node = await _open()
	jb.handle_event(TyrianControls.key_event(KEY_ESCAPE))
	var setup: Node = null
	for i in 200:
		await get_tree().process_frame
		setup = Effects.current_game_scene()
		if setup != null and setup.scene_file_path == Game.SCENE_SETUP and setup.get("idle") == true:
			break
	assert_str(setup.scene_file_path).is_equal(Game.SCENE_SETUP)
	var rows: Array = setup.menus.current().items
	assert_str(rows[setup.menus.current().sel].label).is_equal("setup_jukebox")
