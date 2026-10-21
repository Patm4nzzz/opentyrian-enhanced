## Row chosen at the title (title_screen.c:482, `selectedIndex = MENU_ITEM_NEW_GAME`,
## local variable of titleScreen): a return from a title subscreen
## (`restart = true`) keeps it; a new entry into titleScreen, after
## a game, a demo, a replay or Destruct (opentyr.c:406), starts again on
## "Start New Game".
extends GdUnitTestSuite

const TitleScreen := preload("res://scripts/title.gd")


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TitleScreen.selected = 0


## Title opened right after a `previous` scene (simulated by an empty node
## carrying its path: only Effects.previous_scene_path counts).
func _title_after(previous: String) -> Node:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	var stand_in := Node.new()
	stand_in.scene_file_path = previous
	Effects.game_viewport.add_child(stand_in)
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	return title


func test_new_entry_starts_on_new_game() -> void:
	for previous in [Game.SCENE_LEVEL, "res://scenes/destruct.tscn"]:
		TitleScreen.selected = 6  # Replays, Demo...
		await _title_after(previous)
		(
			assert_int(TitleScreen.selected)
			. override_failure_message("return from %s: Start New Game expected" % previous)
			. is_equal(0)
		)


func test_sub_screen_return_keeps_row() -> void:
	for previous in ["res://scenes/setup.tscn", "res://scenes/new_game.tscn", "res://scenes/high_scores.tscn"]:
		TitleScreen.selected = 4
		await _title_after(previous)
		assert_int(TitleScreen.selected).override_failure_message("return from %s: row kept" % previous).is_equal(4)
