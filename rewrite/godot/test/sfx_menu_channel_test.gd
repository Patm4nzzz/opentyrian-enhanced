# nortsong.c:287: JE_playSampleNum always plays on channel 0 and replaces the
# current sound (loudness.c:867, multiSamplePlayPanPitch). In a level, channel 0
# is also the front gun's (shots.c, soundChannel[0]): a sound played that way is
# cut by the next shot. Godot used to spread these sounds over the 8 channels.
# The achievement sound (S_SOUL_OF_ZINGLON), the first victim of this cut, is no
# longer played in a level (divergence 63, achievement_notify_test.gd).
extends GdUnitTestSuite


func _new_sfx() -> Node:
	var sfx: Node = load("res://scripts/sfx.gd").new()
	sfx.play_in_headless = true
	add_child(sfx)
	return sfx


func _sample_on(sfx: Node, chan: int) -> int:
	return int(str(sfx._players[chan].stream.resource_path).get_file().get_basename())


func test_menu_sounds_share_channel_zero_and_replace_each_other() -> void:
	var sfx := _new_sfx()
	sfx.play(21)
	sfx.play(21)
	sfx.play(28)
	var busy := []
	for i in sfx._players.size():
		if sfx._players[i].playing:
			busy.append(i)
	assert_array(busy).is_equal([0])
	assert_int(_sample_on(sfx, 0)).is_equal(28)
	sfx.free()


func test_level_shot_on_channel_zero_cuts_a_menu_sound() -> void:
	var sfx := _new_sfx()
	sfx.play(21)
	# Front gun shot: channel 0 of the level queue (soundChannel[0]).
	sfx.play_queued(0, 1, 160, 0)
	assert_int(_sample_on(sfx, 0)).is_equal(1)
	for i in range(1, sfx._players.size()):
		assert_bool(sfx._players[i].playing).is_false()
	sfx.free()
