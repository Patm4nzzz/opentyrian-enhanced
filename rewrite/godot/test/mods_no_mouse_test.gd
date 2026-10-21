# M-12: mod_manager.c has no mouse access (grep mouse = 0); the Mods screen
# reacts neither to hover nor to click nor draws a cursor.
extends GdUnitTestSuite

const MODS_MENU := preload("res://scripts/mods_menu.gd")


func test_mods_screen_ignores_mouse() -> void:
	var m: Node = MODS_MENU.new()
	add_child(m)
	await get_tree().process_frame
	var before: int = m.index
	var motion := InputEventMouseMotion.new()
	motion.position = Vector2(60, 40 + 12 * 3)
	motion.relative = Vector2(1, 1)
	m._unhandled_input(motion)
	var click := InputEventMouseButton.new()
	click.position = motion.position
	click.button_index = MOUSE_BUTTON_LEFT
	click.pressed = true
	m._unhandled_input(click)
	assert_int(m.index).is_equal(before)
	var cursors := m.get_children().filter(func(c): return c is MouseCursor)
	assert_array(cursors).is_empty()
	m.queue_free()
