# Level-end music: the C only does fade_song() after the summary
# (tyrian2.c:320, after JE_endLevelAni); the level-end song plays
# during the whole summary.
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")


class EndedSim:
	extends Node

	func take_music_cues() -> PackedInt32Array:
		return PackedInt32Array()

	func music_level_song_branch() -> bool:
		return false

	func level_ended() -> bool:
		return true


func before_test() -> void:
	Music._fade_elapsed = -1.0


func after_test() -> void:
	Music._fade_elapsed = -1.0


func test_level_end_does_not_fade_music_before_debrief() -> void:
	var level: Node = auto_free(LEVEL.new())
	level.sim = auto_free(EndedSim.new())
	level._level_song = 0
	level._music_tick()
	level._music_tick()
	assert_float(Music._fade_elapsed).override_failure_message("fade started before the summary").is_equal(-1.0)


func test_debrief_fades_music_after_statistics() -> void:
	var src := FileAccess.get_file_as_string("res://scripts/debrief.gd")
	var stats := src.find("await _level_statistics(d)")
	var fade := src.find("Music.fade_song()", stats)
	var black := src.find("await fade_black(15)", stats)
	(
		assert_bool(stats > 0 and fade > stats and fade < black)
		. override_failure_message("fade_song after the summary, before the blackout")
		. is_true()
	)
