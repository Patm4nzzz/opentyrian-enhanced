# Debriefing left before the end key: the wait loop
# ("while (!newkey)" of mainint.c:JE_endLevelAni) must stop when
# the screen leaves the tree. wait_units returns right away outside
# the tree; without this exit, the loop spun forever in place, never
# yielding to the engine (gdUnit4 stayed frozen in minigames_test).
extends GdUnitTestSuite


func before_test() -> void:
	Game.campaign_active = false
	Game.debrief = {
		"all_players_alive": true,
		"level_name": "TEST",
		"cash": 100,
		"total_enemy": 10,
		"enemy_killed": 5,
		"cube_max": 0
	}


func after_test() -> void:
	Game.debrief = {}
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	await get_tree().process_frame


func test_debrief_wait_ends_when_screen_leaves_tree() -> void:
	# Same sequence as minigames_test: the scene is replaced in the same
	# frame as its creation, before _run (deferred call) starts; the
	# debriefing then runs outside the tree. Without an exit when the screen
	# leaves the tree, the key wait loop would never yield.
	var screen: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	Effects.game_viewport.remove_child(screen)
	screen.queue_free()
	await get_tree().process_frame
	await get_tree().process_frame
	assert_bool(is_instance_valid(screen)).is_false()
