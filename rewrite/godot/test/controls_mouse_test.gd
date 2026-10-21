## Controls screen (Setup > Key Bindings, F9), equivalent of keybindings_menu.c
## on menu_tree: hover and opening under the pointer like menu_tree.c:run_menu.
extends GdUnitTestSuite

const ControlsOptions := preload("res://scripts/controls_options.gd")


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.inactive = true
	TyrianMouse.position = Vector2i(-1, -1)


static func _motion(x: float, y: float) -> InputEventMouseMotion:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	ev.relative = Vector2(1, 0)
	return ev


## Opened with the mouse (click on "Key Bindings"): the row under the pointer is
## chosen right away, like run_menu, with S_CLICK.
func test_pointer_row_selected_at_open() -> void:
	TyrianMouse.inactive = false
	TyrianMouse.position = Vector2i(100, ControlsOptions.ROW_Y + 5 * ControlsOptions.ROW_STEP + 3)
	var mark: int = Sfx.played.size()
	var scene: Node = Game.goto_scene(ControlsOptions.SCENE_PATH)
	await get_tree().process_frame
	assert_int(scene.selected).override_failure_message("row under the pointer selected on opening").is_equal(5)
	assert_array(Sfx.played.slice(mark)).override_failure_message("hover on opening: S_CLICK").contains([Sfx.CLICK])


## Keyboard (mouse inactive): opening keeps the first row.
func test_keyboard_open_keeps_first_row() -> void:
	TyrianMouse.inactive = true
	TyrianMouse.position = Vector2i(100, ControlsOptions.ROW_Y + 5 * ControlsOptions.ROW_STEP + 3)
	var scene: Node = Game.goto_scene(ControlsOptions.SCENE_PATH)
	await get_tree().process_frame
	assert_int(scene.selected).is_equal(0)


## Hover on another row: S_CLICK (menu_tree.c:580-584), not the cursor sound.
func test_hover_plays_click() -> void:
	TyrianMouse.inactive = true
	var scene: Node = Game.goto_scene(ControlsOptions.SCENE_PATH)
	await get_tree().process_frame
	TyrianMouse.inactive = false
	var mark: int = Sfx.played.size()
	scene.handle_mouse(_motion(100, ControlsOptions.ROW_Y + 2 * ControlsOptions.ROW_STEP + 3))
	assert_int(scene.selected).is_equal(2)
	assert_array(Sfx.played.slice(mark)).override_failure_message("hover: S_CLICK").is_equal([Sfx.CLICK])
