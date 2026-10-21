# Unreadable scene (.pck package replaced under a running game, crash of
# 2026-10-11 06:03): the exported binary hit SIGSEGV on
# `(null as PackedScene).instantiate()`. The current screen must stay displayed.
extends GdUnitTestSuite

const MISSING := "res://scenes/does_not_exist.tscn"


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	await get_tree().process_frame


func test_show_scene_keeps_current_screen_when_scene_is_unreadable() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	assert_object(Game.goto_scene(MISSING)).is_null()
	assert_object(Effects.current_game_scene()).is_same(title)


func test_open_options_keeps_current_screen_when_scene_is_unreadable() -> void:
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	assert_object(Effects.open_options(MISSING)).is_null()
	assert_object(Effects.current_game_scene()).is_same(title)


func test_instantiate_scene_returns_null_for_unreadable_scene() -> void:
	assert_object(Effects.instantiate_scene(MISSING)).is_null()
