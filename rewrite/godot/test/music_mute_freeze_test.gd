# Muting the music (Setup > Sound) freezes the track like the C
# (loudness.c:audioCallback no longer draws anything if music_disabled): it resumes
# at the same point instead of continuing in silence.
extends GdUnitTestSuite


func _music() -> Node:
	var m: Node = load("res://scripts/music.gd").new()
	add_child(m)
	return m


func test_mute_pauses_streams_and_unmute_resumes() -> void:
	var m := _music()
	m.play_song(3)
	assert_bool(m._player.stream_paused).is_false()
	m.muted = true
	assert_bool(m._player.stream_paused).override_failure_message("muted music must freeze the track").is_true()
	m.muted = false
	assert_bool(m._player.stream_paused).is_false()
	m.free()


func test_unmute_keeps_a_stopped_song_paused() -> void:
	var m := _music()
	m.play_song(3)
	m.stop_song()
	m.muted = true
	m.muted = false
	assert_bool(m._player.stream_paused).is_true()
	m.free()
