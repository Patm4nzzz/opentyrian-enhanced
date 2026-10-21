# M-09: a shop always opens on row 2 (game_menu.c:337, curSel = 2);
# the row remembered for the return from Options does not survive a new game.
extends GdUnitTestSuite


func after_test() -> void:
	Game.shop_resume_sel = 0
	Game.campaign_active = false
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func test_new_campaign_forgets_options_row() -> void:
	Game.shop_resume_sel = 5
	Game.start_campaign(1, 2)
	assert_int(Game.shop_resume_sel).is_equal(0)


func test_loading_a_slot_forgets_options_row() -> void:
	Game.shop_resume_sel = 5
	Game.load_campaign_slot(
		{
			"episode": 1,
			"initial_difficulty": 2,
			"difficulty": 2,
			"loadout": PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100]),
			"section": 4,
			"last_level": "TYRIAN"
		}
	)
	assert_int(Game.shop_resume_sel).is_equal(0)


func test_next_episode_forgets_options_row() -> void:
	Game.shop_resume_sel = 5
	Game.campaign_next_episode(1, true)
	assert_int(Game.shop_resume_sel).is_equal(0)
