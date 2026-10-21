## Title achievements screen (title_screen.c:836-871, achievements.c:1536-1550):
## Up / Down, Left / Right play no sound (no JE_playSampleNum in
## the loop); Godot played the cursor sound on every row.
extends GdUnitTestSuite


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func test_scrolling_is_silent_like_c() -> void:
	var scene: Node = Game.goto_scene("res://scenes/achievements_screen.tscn")
	for i in 600:
		await get_tree().process_frame
		if scene.idle:
			break
	assert_bool(scene.idle).override_failure_message("Achievements screen ready").is_true()
	var mark: int = Sfx.played.size()
	scene.handle_action(TyrianControls.DOWN)
	scene.handle_action(TyrianControls.DOWN)
	scene.handle_action(TyrianControls.UP)
	assert_int(scene.cursor).override_failure_message("Down, Down, Up -> row 1").is_equal(1)
	scene.handle_action(TyrianControls.RIGHT)
	scene.handle_action(TyrianControls.LEFT)
	assert_array(Sfx.played.slice(mark)).override_failure_message("no sound, like the C").is_empty()
